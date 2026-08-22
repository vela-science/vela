#!/usr/bin/env python3
"""Fail-closed qualification and custody for neutral evidence runtimes.

This module is tooling, not Protocol 1. It validates one pre-execution bundle
and one no-science capture fixture. It never invokes a provider, mints a
participant permit, opens protected answers, or changes Repository state.
"""

from __future__ import annotations

import argparse
import contextvars
import hashlib
import io
import json
import os
import re
import stat
import sys
import tarfile
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import ROUND_HALF_EVEN, Decimal, InvalidOperation
from pathlib import Path
from typing import Any

import jsonschema
from jsonschema import Draft202012Validator, FormatChecker

SCHEMA = "vela.tooling.evidence-qualification.v1"
RECEIPT_SCHEMA = "vela.tooling.evidence-qualification-receipt.v1"
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
FORBIDDEN_EVENT = re.compile(
    r"tool|command|patch|file_change|web_search|computer|compact|resume|continu",
    re.IGNORECASE,
)
NETWORK_PACKAGE_METADATA = re.compile(
    r"\b(?:apt-get|apt|apk|dnf|yum)\s+(?:update|install)\b"
)
PROVEN_PROVIDER_DELETIONS = {"uniqueItems": True}
QUALIFIER = Path(__file__).resolve()
PERMIT_SCHEMA = "vela.tooling.closed-launch-permit.v1"
RUNNER_VERSION = "neutral-runner/1"
EVENT_SCHEMA = "vela.tooling.provider-event.v1"
LAUNCH_SCHEMA = "vela.tooling.neutral-launch.v1"
TERMINAL_SCHEMA = "vela.tooling.neutral-terminal-receipt.v1"
TEARDOWN_SCHEMA = "vela.tooling.neutral-teardown.v1"
_ACTIVE_BUNDLE_ROOT: contextvars.ContextVar[Path | None] = contextvars.ContextVar(
    "evidence_qualification_bundle_root", default=None
)


class QualificationError(ValueError):
    """A closed qualification boundary did not validate."""


def exact_keys(value: Any, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        raise QualificationError(f"{label}_fields_invalid")


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise QualificationError(f"duplicate_json_key:{key}")
        value[key] = item
    return value


def parse_json(raw: bytes, label: str) -> Any:
    try:
        return json.loads(
            raw,
            object_pairs_hook=_pairs,
            parse_float=Decimal,
            parse_constant=lambda value: (_ for _ in ()).throw(
                QualificationError(f"{label}_nonfinite_number:{value}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise QualificationError(f"{label}_json_invalid") from error


def load_json(path: Path, label: str) -> Any:
    return parse_json(read_regular(path, label), label)


def _decimal_text(value: Decimal) -> str:
    if not value.is_finite():
        raise QualificationError("canonical_number_nonfinite")
    if value == 0:
        return "0"
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text


def canonical_json_bytes(value: Any) -> bytes:
    """Serialize deterministically without routing Decimal through binary float."""

    def encode(item: Any) -> str:
        if item is None:
            return "null"
        if item is True:
            return "true"
        if item is False:
            return "false"
        if isinstance(item, str):
            return json.dumps(item, ensure_ascii=False, separators=(",", ":"))
        if isinstance(item, int) and not isinstance(item, bool):
            return str(item)
        if isinstance(item, Decimal):
            return _decimal_text(item)
        if isinstance(item, float):
            raise QualificationError("canonical_binary_float_forbidden")
        if isinstance(item, (list, tuple)):
            return "[" + ",".join(encode(child) for child in item) + "]"
        if isinstance(item, dict):
            if any(not isinstance(key, str) for key in item):
                raise QualificationError("canonical_key_not_string")
            return (
                "{"
                + ",".join(f"{encode(key)}:{encode(item[key])}" for key in sorted(item))
                + "}"
            )
        raise QualificationError(f"canonical_type_unsupported:{type(item).__name__}")

    return (encode(value) + "\n").encode()


def canonical_root(value: Any) -> str:
    return digest(canonical_json_bytes(value))


def rounded_decimal(value: Decimal, quantum: Decimal) -> Decimal:
    try:
        return value.quantize(quantum, rounding=ROUND_HALF_EVEN)
    except InvalidOperation as error:
        raise QualificationError("decimal_quantization_invalid") from error


def safe_relative(
    root: Path, value: Any, label: str, *, must_exist: bool = True
) -> Path:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise QualificationError(f"{label}_path_not_relative")
    relative = Path(value)
    if any(part in {"", ".", ".."} for part in relative.parts):
        raise QualificationError(f"{label}_path_unsafe")
    path = root / relative
    cursor = root
    parts = relative.parts if must_exist else relative.parts[:-1]
    for part in parts:
        cursor = cursor / part
        try:
            mode = os.lstat(cursor).st_mode
        except FileNotFoundError as error:
            raise QualificationError(f"{label}_missing") from error
        if stat.S_ISLNK(mode):
            raise QualificationError(f"{label}_symlink_forbidden")
    if must_exist and not path.exists():
        raise QualificationError(f"{label}_missing")
    return path


def validate_bundle_tree(root: Path) -> None:
    """Reject aliases before any role-specific file is trusted."""
    if not root.is_absolute() or root != Path(os.path.abspath(root)):
        raise QualificationError("bundle_root_not_canonical_absolute")
    try:
        root_mode = os.lstat(root).st_mode
    except FileNotFoundError as error:
        raise QualificationError("bundle_directory_invalid") from error
    if stat.S_ISLNK(root_mode) or not stat.S_ISDIR(root_mode):
        raise QualificationError("bundle_directory_invalid")
    identities: dict[tuple[int, int], str] = {}
    for directory, names, files in os.walk(root, followlinks=False):
        for name in [*names, *files]:
            path = Path(directory) / name
            metadata = os.lstat(path)
            if stat.S_ISLNK(metadata.st_mode):
                raise QualificationError("bundle_symlink_forbidden")
            if stat.S_ISREG(metadata.st_mode):
                identity = (metadata.st_dev, metadata.st_ino)
                relative = path.relative_to(root).as_posix()
                prior = identities.setdefault(identity, relative)
                if prior != relative:
                    raise QualificationError("bundle_file_alias_forbidden")


def parse_timestamp(value: Any, label: str) -> datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise QualificationError(f"{label}_timestamp_invalid")
    try:
        parsed = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as error:
        raise QualificationError(f"{label}_timestamp_invalid") from error
    if parsed.tzinfo != timezone.utc:
        raise QualificationError(f"{label}_timestamp_invalid")
    return parsed


def nonnegative_number(value: Any, label: str) -> Decimal:
    if isinstance(value, bool) or not isinstance(value, (int, Decimal)):
        raise QualificationError(f"{label}_invalid")
    decimal = Decimal(value)
    if not decimal.is_finite() or decimal < 0:
        raise QualificationError(f"{label}_invalid")
    return decimal


def read_regular(path: Path, label: str) -> bytes:
    root = _ACTIVE_BUNDLE_ROOT.get()
    if root is not None:
        try:
            relative = path.relative_to(root)
        except ValueError:
            relative = None
        if relative is not None:
            return _read_bundle_regular(root, relative, label)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise QualificationError(f"{label}_missing_or_unsafe") from error
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise QualificationError(f"{label}_not_regular")
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            return handle.read()
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _read_bundle_regular(root: Path, relative: Path, label: str) -> bytes:
    if relative.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise QualificationError(f"{label}_path_unsafe")
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | nofollow
    try:
        directory = os.open(root, directory_flags)
    except OSError as error:
        raise QualificationError(f"{label}_root_missing_or_unsafe") from error
    descriptor = -1
    try:
        for part in relative.parts[:-1]:
            next_directory = os.open(part, directory_flags, dir_fd=directory)
            os.close(directory)
            directory = next_directory
        descriptor = os.open(
            relative.parts[-1], os.O_RDONLY | nofollow, dir_fd=directory
        )
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise QualificationError(f"{label}_not_regular")
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            return handle.read()
    except OSError as error:
        raise QualificationError(f"{label}_missing_or_unsafe") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        os.close(directory)


def tree_manifest(directory: Path) -> list[dict[str, Any]]:
    if not directory.is_dir() or directory.is_symlink():
        raise QualificationError("runtime_source_directory_invalid")
    entries = []
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise QualificationError("runtime_source_symlink_forbidden")
        if path.is_file():
            raw = read_regular(path, "runtime_source")
            entries.append(
                {
                    "path": path.relative_to(directory).as_posix(),
                    "bytes": len(raw),
                    "sha256": digest(raw),
                }
            )
    return entries


def _delete_pointer(value: Any, pointer: str) -> None:
    if not isinstance(pointer, str) or not pointer.startswith("/"):
        raise QualificationError("provider_deleted_pointer_invalid")
    parts = [
        part.replace("~1", "/").replace("~0", "~") for part in pointer[1:].split("/")
    ]
    parent = value
    for part in parts[:-1]:
        if not isinstance(parent, dict) or part not in parent:
            raise QualificationError("provider_deleted_pointer_missing")
        parent = parent[part]
    keyword = parts[-1]
    if not isinstance(parent, dict) or keyword not in parent:
        raise QualificationError("provider_deleted_pointer_missing")
    if (
        keyword not in PROVEN_PROVIDER_DELETIONS
        or parent[keyword] is not PROVEN_PROVIDER_DELETIONS[keyword]
    ):
        raise QualificationError("provider_deleted_keyword_not_proven")
    del parent[keyword]


def provider_derivative(
    registered: dict[str, Any], pointers: list[str]
) -> dict[str, Any]:
    if not pointers or len(set(pointers)) != len(pointers):
        raise QualificationError("provider_deleted_pointers_invalid")
    derived = parse_json(canonical_json_bytes(registered), "registered_schema_copy")
    for pointer in pointers:
        _delete_pointer(derived, pointer)
    return derived


def validate_schema_boundary(
    registered: Any,
    provider: Any,
    deleted_pointers: Any,
    response: Any,
) -> None:
    if not isinstance(registered, dict) or not isinstance(provider, dict):
        raise QualificationError("response_schema_not_object")
    if registered.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        raise QualificationError("registered_schema_not_draft_2020_12")
    try:
        Draft202012Validator.check_schema(registered)
        Draft202012Validator.check_schema(provider)
    except Exception as error:
        raise QualificationError("response_schema_invalid") from error
    if not isinstance(deleted_pointers, list) or provider != provider_derivative(
        registered, deleted_pointers
    ):
        raise QualificationError("provider_schema_not_exact_derivative")
    errors = sorted(
        Draft202012Validator(registered, format_checker=FormatChecker()).iter_errors(
            response
        ),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        raise QualificationError("response_full_schema_invalid")


def normalize_closed_set(
    response: dict[str, Any], field: str, key: str, expected: list[str]
) -> dict[str, Any]:
    if not isinstance(field, str) or not isinstance(key, str):
        raise QualificationError("closed_set_contract_invalid")
    items = response.get(field)
    if not isinstance(items, list) or not all(isinstance(item, dict) for item in items):
        raise QualificationError("closed_set_not_array")
    observed = [item.get(key) for item in items]
    if any(not isinstance(item, str) or not item for item in observed):
        raise QualificationError("closed_set_identity_invalid")
    if len(set(observed)) != len(observed):
        raise QualificationError("closed_set_identity_duplicate")
    if set(observed) != set(expected) or len(observed) != len(expected):
        raise QualificationError("closed_set_identity_mismatch")
    by_identity = dict(zip(observed, items, strict=True))
    normalized = dict(response)
    normalized[field] = [by_identity[identity] for identity in sorted(expected)]
    return normalized


def normalize_shadow_account(raw: bytes, account: str, fixed_day: int) -> bytes:
    if not isinstance(fixed_day, int) or isinstance(fixed_day, bool) or fixed_day < 0:
        raise QualificationError("account_fixed_day_invalid")
    try:
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError as error:
            raise QualificationError("account_database_utf8_invalid") from error
    except UnicodeDecodeError as error:
        raise QualificationError("account_database_utf8_invalid") from error
    lines = text.splitlines()
    matches = [
        index for index, line in enumerate(lines) if line.split(":", 1)[0] == account
    ]
    if len(matches) != 1:
        raise QualificationError("account_record_count_invalid")
    index = matches[0]
    fields = lines[index].split(":")
    if len(fields) != 9:
        raise QualificationError("account_record_fields_invalid")
    if fields[1] != "!" or not fields[2].isdigit():
        raise QualificationError("account_record_shape_invalid")
    fields[2] = str(fixed_day)
    lines[index] = ":".join(fields)
    suffix = "\n" if text.endswith("\n") else ""
    return ("\n".join(lines) + suffix).encode()


def _permit_fields() -> set[str]:
    return {
        "schema",
        "registration_id",
        "assignment_id",
        "participant_id",
        "run_id",
        "condition",
        "attempt",
        "runner_version",
        "runtime_source_root",
        "configuration_root",
        "image_digest",
        "registered_schema_bytes",
        "provider_schema_bytes",
        "prompt_root",
        "packet_root",
        "timeout_seconds",
        "status",
        "issued_at",
        "consumed_at",
    }


def validate_permit(
    value: Any,
    expected: dict[str, Any],
    *,
    status: str,
) -> None:
    exact_keys(value, _permit_fields(), "permit")
    if value["schema"] != PERMIT_SCHEMA or value["status"] != status:
        raise QualificationError("permit_schema_or_status_invalid")
    for key, expected_value in expected.items():
        if value.get(key) != expected_value:
            raise QualificationError(f"permit_binding_invalid:{key}")
    if (
        isinstance(value["attempt"], bool)
        or not isinstance(value["attempt"], int)
        or value["attempt"] != 1
        or isinstance(value["timeout_seconds"], bool)
        or not isinstance(value["timeout_seconds"], int)
        or value["timeout_seconds"] <= 0
    ):
        raise QualificationError("permit_numeric_contract_invalid")
    for key in (
        "registration_id",
        "assignment_id",
        "participant_id",
        "run_id",
        "condition",
        "runner_version",
    ):
        if not isinstance(value[key], str) or not value[key]:
            raise QualificationError(f"permit_identity_invalid:{key}")
    for key in (
        "runtime_source_root",
        "configuration_root",
        "image_digest",
        "registered_schema_bytes",
        "provider_schema_bytes",
        "prompt_root",
        "packet_root",
    ):
        if not isinstance(value[key], str) or not SHA256.fullmatch(value[key]):
            raise QualificationError(f"permit_root_invalid:{key}")
    parse_timestamp(value["issued_at"], "permit_issued_at")
    if status == "held":
        if value["consumed_at"] is not None:
            raise QualificationError("permit_held_consumption_invalid")
    elif status == "consumed":
        consumed_at = parse_timestamp(value["consumed_at"], "permit_consumed_at")
        if consumed_at < parse_timestamp(value["issued_at"], "permit_issued_at"):
            raise QualificationError("permit_consumption_precedes_issuance")
    else:
        raise QualificationError("permit_status_invalid")


def consume_permit(directory: Path, run_id: str, expected: dict[str, Any]) -> Path:
    """Atomically consume one permit without an overwrite-capable rename."""
    if not re.fullmatch(r"[-a-z0-9]+", run_id):
        raise QualificationError("permit_run_id_invalid")
    if (
        not directory.is_absolute()
        or directory != directory.resolve()
        or directory.is_symlink()
        or not directory.is_dir()
    ):
        raise QualificationError("permit_directory_invalid")
    source_name = f"{run_id}.permit.json"
    consumed_name = f"{run_id}.permit.consumed.json"
    consumed = directory / consumed_name
    directory_descriptor = os.open(
        directory,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        source_descriptor = os.open(
            source_name,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=directory_descriptor,
        )
        try:
            if not stat.S_ISREG(os.fstat(source_descriptor).st_mode):
                raise QualificationError("permit_source_not_regular")
            with os.fdopen(source_descriptor, "rb") as handle:
                source_descriptor = -1
                raw = handle.read()
        finally:
            if source_descriptor >= 0:
                os.close(source_descriptor)
    except OSError as error:
        os.close(directory_descriptor)
        raise QualificationError("permit_source_missing_or_unsafe") from error
    try:
        try:
            permit = parse_json(raw, "permit_source")
            validate_permit(permit, expected, status="held")
            os.link(
                source_name,
                consumed_name,
                src_dir_fd=directory_descriptor,
                dst_dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileExistsError as error:
            raise QualificationError("permit_already_consumed") from error
        except OSError as error:
            raise QualificationError("permit_atomic_consume_failed") from error
        try:
            os.unlink(source_name, dir_fd=directory_descriptor)
        except OSError as error:
            try:
                os.unlink(consumed_name, dir_fd=directory_descriptor)
            except FileNotFoundError:
                pass
            raise QualificationError("permit_atomic_consume_failed") from error
    finally:
        os.close(directory_descriptor)
    return consumed


def permit_identity(value: dict[str, Any]) -> dict[str, Any]:
    return {
        key: item for key, item in value.items() if key not in {"status", "consumed_at"}
    }


def validate_events(raw: bytes, output_token_ceiling: int) -> dict[str, Any]:
    events = []
    for index, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        event = parse_json(line, f"provider_event_{index}")
        if not isinstance(event, dict):
            raise QualificationError("provider_event_not_object")
        event_type = event.get("type")
        if event.get("schema") != EVENT_SCHEMA or not isinstance(event_type, str):
            raise QualificationError("provider_event_schema_invalid")
        item = event.get("item")
        item_type = str(item.get("type", "")) if isinstance(item, dict) else ""
        if FORBIDDEN_EVENT.search(f"{event_type}:{item_type}"):
            raise QualificationError("provider_event_forbidden")
        events.append(event)
    types = [event["type"] for event in events]
    if types != ["thread.started", "turn.started", "item.completed", "turn.completed"]:
        raise QualificationError("provider_event_sequence_invalid")
    exact_keys(
        events[0], {"schema", "type", "run_id", "thread_id", "at"}, "thread_started"
    )
    exact_keys(
        events[1],
        {"schema", "type", "run_id", "thread_id", "turn_id", "at"},
        "turn_started",
    )
    exact_keys(
        events[2],
        {
            "schema",
            "type",
            "run_id",
            "thread_id",
            "turn_id",
            "response_id",
            "at",
            "item",
        },
        "item_completed",
    )
    exact_keys(
        events[3],
        {
            "schema",
            "type",
            "run_id",
            "thread_id",
            "turn_id",
            "response_id",
            "at",
            "usage",
        },
        "turn_completed",
    )
    item = events[2]["item"]
    exact_keys(item, {"type", "text"}, "provider_message")
    if item["type"] != "agent_message" or not isinstance(item["text"], str):
        raise QualificationError("provider_message_invalid")
    identities = {(event["run_id"], event["thread_id"]) for event in events}
    if len(identities) != 1 or any(
        not isinstance(item, str) or not item for item in next(iter(identities))
    ):
        raise QualificationError("provider_event_identity_drift")
    if (
        events[1]["turn_id"] != events[2]["turn_id"]
        or events[1]["turn_id"] != events[3]["turn_id"]
    ):
        raise QualificationError("provider_turn_identity_drift")
    if events[2]["response_id"] != events[3]["response_id"]:
        raise QualificationError("provider_response_identity_drift")
    timestamps = [parse_timestamp(event["at"], "provider_event") for event in events]
    if timestamps != sorted(timestamps):
        raise QualificationError("provider_event_time_not_monotone")
    usage = events[3]["usage"]
    exact_keys(
        usage,
        {"input_tokens", "cached_input_tokens", "output_tokens", "tool_call_count"},
        "provider_usage",
    )
    for key, value in usage.items():
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise QualificationError(f"provider_usage_invalid:{key}")
    if (
        usage["cached_input_tokens"] > usage["input_tokens"]
        or usage["tool_call_count"] != 0
    ):
        raise QualificationError("provider_usage_contract_invalid")
    if usage["output_tokens"] > output_token_ceiling:
        raise QualificationError("provider_output_token_ceiling")
    return {
        "events": events,
        "messages": [item],
        "usage": usage,
        "timestamps": timestamps,
    }


def _validate_mounts(mounts: Any) -> None:
    if not isinstance(mounts, list) or not mounts:
        raise QualificationError("runtime_mounts_invalid")
    targets = set()
    for mount in mounts:
        exact_keys(mount, {"source", "target", "read_only"}, "runtime_mount")
        if not isinstance(mount["source"], str) or not isinstance(mount["target"], str):
            raise QualificationError("runtime_mount_path_invalid")
        source = Path(mount["source"])
        target = Path(mount["target"])
        if (
            not source.is_absolute()
            or source != source.resolve()
            or not source.exists()
        ):
            raise QualificationError("runtime_mount_source_not_canonical_absolute")
        if not target.is_absolute() or ".." in target.parts or target in targets:
            raise QualificationError("runtime_mount_target_invalid")
        if mount["read_only"] is not True:
            raise QualificationError("runtime_mount_not_read_only")
        targets.add(target)


def _validate_build_inputs(root: Path, value: Any) -> str:
    exact_keys(value, {"schema", "inputs"}, "build_inputs")
    if value["schema"] != "vela.tooling.vendored-build-inputs.v1":
        raise QualificationError("build_inputs_schema_invalid")
    inputs = value["inputs"]
    if not isinstance(inputs, list) or not inputs:
        raise QualificationError("build_inputs_empty")
    paths = set()
    for entry in inputs:
        exact_keys(
            entry,
            {"path", "bytes", "sha256", "source_url", "source_sha256", "license_path"},
            "build_input",
        )
        path = safe_relative(root, entry["path"], "build_input")
        license_path = safe_relative(root, entry["license_path"], "build_input_license")
        raw = read_regular(path, "build_input")
        if (
            entry["path"] in paths
            or entry["bytes"] != len(raw)
            or entry["sha256"] != digest(raw)
        ):
            raise QualificationError("build_input_binding_invalid")
        if (
            entry["source_sha256"] != entry["sha256"]
            or not isinstance(entry["source_url"], str)
            or not entry["source_url"].startswith("https://")
            or entry["license_path"] == entry["path"]
        ):
            raise QualificationError("build_input_provenance_incomplete")
        if not read_regular(license_path, "build_input_license"):
            raise QualificationError("build_input_license_empty")
        paths.add(entry["path"])
    return canonical_root(value)


def effective_dockerfile_instructions(raw: bytes) -> list[tuple[str, str]]:
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise QualificationError("runtime_dockerfile_utf8_invalid") from error
    logical: list[str] = []
    current = ""
    for source_line in lines:
        stripped = source_line.strip()
        if not stripped or (not current and stripped.startswith("#")):
            continue
        current = (current + " " + stripped).strip()
        if current.endswith("\\"):
            current = current[:-1].rstrip()
            continue
        logical.append(current)
        current = ""
    if current:
        raise QualificationError("runtime_dockerfile_continuation_invalid")
    instructions = []
    for line in logical:
        match = re.fullmatch(r"([A-Za-z]+)\s+(.+)", line)
        if not match:
            raise QualificationError("runtime_dockerfile_instruction_invalid")
        instructions.append((match.group(1).upper(), match.group(2).strip()))
    return instructions


@dataclass(frozen=True)
class OciIdentity:
    manifest_digest: str
    config_digest: str
    platform: str
    layer_digests: tuple[str, ...]
    archive_digest: str
    layout_digest: str


def _descriptor(value: Any, label: str) -> tuple[str, int]:
    exact_keys(value, {"mediaType", "digest", "size"}, label)
    if (
        not isinstance(value["mediaType"], str)
        or not value["mediaType"]
        or not isinstance(value["digest"], str)
        or not SHA256.fullmatch(value["digest"])
        or isinstance(value["size"], bool)
        or not isinstance(value["size"], int)
        or value["size"] < 0
    ):
        raise QualificationError(f"{label}_invalid")
    return value["digest"], value["size"]


def _oci_identity(raw: bytes) -> OciIdentity:
    try:
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:*") as archive:
            members: dict[str, bytes] = {}
            for member in archive.getmembers():
                path = Path(member.name)
                if (
                    member.name in members
                    or path.is_absolute()
                    or any(part in {"", ".", ".."} for part in path.parts)
                    or not member.isfile()
                ):
                    raise QualificationError("oci_archive_member_invalid")
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise QualificationError("oci_archive_member_invalid")
                members[member.name] = extracted.read()
            if "index.json" not in members or "oci-layout" not in members:
                raise QualificationError("oci_archive_layout_incomplete")
            layout_raw = members["oci-layout"]
            layout = parse_json(layout_raw, "oci_layout")
            if layout != {"imageLayoutVersion": "1.0.0"}:
                raise QualificationError("oci_layout_invalid")
            index_raw = members["index.json"]
            index = parse_json(index_raw, "oci_index")
            exact_keys(index, {"schemaVersion", "manifests"}, "oci_index")
            if index["schemaVersion"] != 2:
                raise QualificationError("oci_index_schema_invalid")
            manifests = index.get("manifests") if isinstance(index, dict) else None
            if not isinstance(manifests, list) or len(manifests) != 1:
                raise QualificationError("oci_manifest_count_invalid")
            manifest_digest, manifest_size = _descriptor(
                manifests[0], "oci_manifest_descriptor"
            )
            manifest_path = "blobs/sha256/" + manifest_digest.removeprefix("sha256:")
            manifest_raw = members.get(manifest_path)
            if (
                manifest_raw is None
                or len(manifest_raw) != manifest_size
                or digest(manifest_raw) != manifest_digest
            ):
                raise QualificationError("oci_manifest_bytes_drift")
            manifest = parse_json(manifest_raw, "oci_manifest")
            exact_keys(manifest, {"schemaVersion", "config", "layers"}, "oci_manifest")
            if manifest["schemaVersion"] != 2 or not isinstance(
                manifest["layers"], list
            ):
                raise QualificationError("oci_manifest_schema_invalid")
            config_digest, config_size = _descriptor(
                manifest["config"], "oci_config_descriptor"
            )
            descriptors = [
                _descriptor(layer, "oci_layer_descriptor")
                for layer in manifest["layers"]
            ]
            if not descriptors or len({item[0] for item in descriptors}) != len(
                descriptors
            ):
                raise QualificationError("oci_layer_descriptors_invalid")
            config_path = "blobs/sha256/" + config_digest.removeprefix("sha256:")
            config_raw = members.get(config_path)
            if (
                config_raw is None
                or len(config_raw) != config_size
                or digest(config_raw) != config_digest
            ):
                raise QualificationError("oci_config_bytes_drift")
            expected_members = {"index.json", "oci-layout", manifest_path, config_path}
            for layer_digest, layer_size in descriptors:
                layer_path = "blobs/sha256/" + layer_digest.removeprefix("sha256:")
                layer_raw = members.get(layer_path)
                if (
                    layer_raw is None
                    or len(layer_raw) != layer_size
                    or digest(layer_raw) != layer_digest
                ):
                    raise QualificationError("oci_layer_bytes_drift")
                expected_members.add(layer_path)
            if set(members) != expected_members:
                raise QualificationError("oci_archive_members_not_exact")
            config = parse_json(config_raw, "oci_config")
            operating_system = config.get("os") if isinstance(config, dict) else None
            architecture = (
                config.get("architecture") if isinstance(config, dict) else None
            )
            if not isinstance(operating_system, str) or not isinstance(
                architecture, str
            ):
                raise QualificationError("oci_config_platform_invalid")
    except (tarfile.TarError, KeyError, AttributeError, json.JSONDecodeError) as error:
        raise QualificationError("oci_archive_invalid") from error
    return OciIdentity(
        manifest_digest,
        config_digest,
        f"{operating_system}/{architecture}",
        tuple(item[0] for item in descriptors),
        digest(raw),
        digest(layout_raw),
    )


def _validate_oci(
    root: Path, runtime: dict[str, Any], source_root: str, build_inputs_root: str
) -> tuple[str, str]:
    archives = [
        safe_relative(root, item, "oci_archive") for item in runtime["oci_archives"]
    ]
    receipts = [
        load_json(safe_relative(root, item, "oci_receipt"), "oci_receipt")
        for item in runtime["oci_receipts"]
    ]
    if len(archives) != 2 or len(receipts) != 2:
        raise QualificationError("oci_independent_builder_count_invalid")
    raw_archives = [read_regular(path, "oci_archive") for path in archives]
    if raw_archives[0] != raw_archives[1]:
        raise QualificationError("oci_archives_not_byte_identical")
    identities = [_oci_identity(raw) for raw in raw_archives]
    if identities[0] != identities[1]:
        raise QualificationError("oci_identity_drift")
    builders = set()
    for receipt, raw, identity in zip(receipts, raw_archives, identities, strict=True):
        exact_keys(
            receipt,
            {
                "schema",
                "builder",
                "empty_cache",
                "network_during_build",
                "platform",
                "source_date_epoch",
                "source_root",
                "build_inputs_root",
                "controls",
                "image_digest",
                "config_digest",
                "layer_digests",
                "oci_layout_bytes",
                "oci_tar_bytes",
            },
            "oci_receipt",
        )
        if receipt["schema"] != "vela.tooling.oci-build-receipt.v1":
            raise QualificationError("oci_receipt_schema_invalid")
        required_controls = {
            "no_cache": True,
            "provenance": False,
            "pull": False,
            "rewrite_timestamp": True,
        }
        if (
            not isinstance(receipt["builder"], str)
            or not receipt["builder"]
            or receipt["builder"] in builders
            or receipt["empty_cache"] is not True
            or receipt["network_during_build"] is not False
            or receipt["platform"] != runtime["platform"]
            or receipt["source_date_epoch"] != runtime["source_date_epoch"]
            or receipt["source_root"] != source_root
            or receipt["build_inputs_root"] != build_inputs_root
            or receipt["controls"] != required_controls
            or receipt["image_digest"] != identity.manifest_digest
            or receipt["config_digest"] != identity.config_digest
            or receipt["layer_digests"] != list(identity.layer_digests)
            or receipt["platform"] != identity.platform
            or receipt["oci_layout_bytes"] != identity.layout_digest
            or receipt["oci_tar_bytes"] != identity.archive_digest
        ):
            raise QualificationError("oci_receipt_binding_invalid")
        builders.add(receipt["builder"])
    return identities[0].manifest_digest, identities[0].config_digest


def _validate_runtime(root: Path, value: Any) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "source_dir",
            "source_manifest",
            "build_inputs",
            "oci_archives",
            "oci_receipts",
            "platform",
            "source_date_epoch",
            "trust_bundle",
            "trust_bundle_sha256",
            "trust_bundle_container_path",
            "ssl_cert_file",
            "mounts",
            "account_database",
        },
        "runtime",
    )
    if (
        not isinstance(value["platform"], str)
        or not value["platform"]
        or isinstance(value["source_date_epoch"], bool)
        or not isinstance(value["source_date_epoch"], int)
        or value["source_date_epoch"] <= 0
    ):
        raise QualificationError("runtime_build_identity_invalid")
    source_dir = safe_relative(root, value["source_dir"], "runtime_source_dir")
    manifest = load_json(
        safe_relative(root, value["source_manifest"], "runtime_source_manifest"),
        "runtime_source_manifest",
    )
    actual_manifest = tree_manifest(source_dir)
    if manifest != actual_manifest:
        raise QualificationError("runtime_source_manifest_drift")
    source_root = canonical_root(manifest)
    dockerfile_raw = read_regular(source_dir / "Dockerfile", "runtime_dockerfile")
    instructions = effective_dockerfile_instructions(dockerfile_raw)
    if (
        ("ARG", "SOURCE_DATE_EPOCH") not in instructions
        or not any(
            keyword == "RUN" and arguments.startswith("--network=none ")
            for keyword, arguments in instructions
        )
        or any(
            keyword == "RUN" and NETWORK_PACKAGE_METADATA.search(arguments)
            for keyword, arguments in instructions
        )
    ):
        raise QualificationError("runtime_dockerfile_not_reproducible")
    build_inputs = load_json(
        safe_relative(root, value["build_inputs"], "build_inputs"), "build_inputs"
    )
    build_inputs_root = _validate_build_inputs(root, build_inputs)
    image_digest, config_digest = _validate_oci(
        root, value, source_root, build_inputs_root
    )
    trust_path = safe_relative(root, value["trust_bundle"], "trust_bundle")
    trust_raw = read_regular(trust_path, "trust_bundle")
    if b"-----BEGIN CERTIFICATE-----" not in trust_raw or value[
        "trust_bundle_sha256"
    ] != digest(trust_raw):
        raise QualificationError("trust_bundle_invalid")
    if (
        value["trust_bundle_container_path"] != value["ssl_cert_file"]
        or not Path(value["trust_bundle_container_path"]).is_absolute()
    ):
        raise QualificationError("trust_bundle_runtime_binding_invalid")
    _validate_mounts(value["mounts"])
    account = value["account_database"]
    exact_keys(
        account,
        {
            "account",
            "expected_accounts",
            "fixed_day",
            "fixtures",
            "normalized_sha256",
        },
        "account_database",
    )
    fixtures = account["fixtures"]
    if (
        not isinstance(account["account"], str)
        or not isinstance(account["expected_accounts"], list)
        or len(set(account["expected_accounts"])) != len(account["expected_accounts"])
        or account["account"] not in account["expected_accounts"]
        or not isinstance(fixtures, list)
        or len(fixtures) != 2
    ):
        raise QualificationError("account_database_fixtures_invalid")
    raw_fixtures = []
    source_days = []
    paths = []
    normalized = []
    baseline_records: dict[str, list[str]] | None = None
    for fixture in fixtures:
        exact_keys(fixture, {"path", "source_day", "sha256"}, "account_fixture")
        path = safe_relative(root, fixture["path"], "account_fixture")
        raw = read_regular(path, "account_fixture")
        if fixture["sha256"] != digest(raw):
            raise QualificationError("account_fixture_bytes_drift")
        text = raw.decode("utf-8")
        records: dict[str, list[str]] = {}
        for line in text.splitlines():
            fields = line.split(":")
            if len(fields) != 9 or fields[0] in records:
                raise QualificationError("account_record_fields_invalid")
            records[fields[0]] = fields
        if set(records) != set(account["expected_accounts"]):
            raise QualificationError("account_database_accounts_invalid")
        target = records[account["account"]]
        if (
            target[1] != "!"
            or not target[2].isdigit()
            or isinstance(fixture["source_day"], bool)
            or not isinstance(fixture["source_day"], int)
            or int(target[2]) != fixture["source_day"]
        ):
            raise QualificationError("account_fixture_source_day_invalid")
        comparable = {key: value.copy() for key, value in records.items()}
        comparable[account["account"]][2] = "<source-day>"
        if baseline_records is not None and comparable != baseline_records:
            raise QualificationError("account_fixture_metadata_drift")
        baseline_records = comparable
        paths.append(fixture["path"])
        source_days.append(fixture["source_day"])
        raw_fixtures.append(raw)
        normalized.append(
            normalize_shadow_account(raw, account["account"], account["fixed_day"])
        )
    if (
        len(set(paths)) != 2
        or len(set(source_days)) != 2
        or len(set(raw_fixtures)) != 2
        or len(set(normalized)) != 1
        or digest(normalized[0]) != account["normalized_sha256"]
    ):
        raise QualificationError("account_database_not_date_invariant")
    return {
        "runtime_source_root": source_root,
        "build_inputs_root": build_inputs_root,
        "image_digest": image_digest,
        "image_config_digest": config_digest,
        "trust_bundle_sha256": digest(trust_raw),
        "dockerfile_bytes": digest(dockerfile_raw),
    }


def _validate_configuration(
    root: Path, value: Any, runtime: dict[str, Any]
) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "model",
            "reasoning_effort",
            "service_tier",
            "timeout_seconds",
            "output_token_ceiling",
            "attempt",
            "retries",
            "tools",
            "strict_arguments",
            "compatibility_receipt",
            "runner_version",
        },
        "configuration",
    )
    if (
        isinstance(value["timeout_seconds"], bool)
        or not isinstance(value["timeout_seconds"], int)
        or value["timeout_seconds"] <= 0
        or isinstance(value["output_token_ceiling"], bool)
        or not isinstance(value["output_token_ceiling"], int)
        or value["output_token_ceiling"] <= 0
        or isinstance(value["attempt"], bool)
        or value["attempt"] != 1
        or isinstance(value["retries"], bool)
        or value["retries"] != 0
        or value["tools"] != "none"
        or value["runner_version"] != RUNNER_VERSION
        or any(
            not isinstance(value[key], str) or not value[key]
            for key in ("model", "reasoning_effort", "service_tier")
        )
        or not isinstance(value["strict_arguments"], list)
        or not value["strict_arguments"]
        or any(
            not isinstance(item, str) or not item for item in value["strict_arguments"]
        )
        or len(set(value["strict_arguments"])) != len(value["strict_arguments"])
    ):
        raise QualificationError("configuration_contract_invalid")
    receipt = load_json(
        safe_relative(
            root, value["compatibility_receipt"], "configuration_compatibility_receipt"
        ),
        "configuration_compatibility_receipt",
    )
    exact_keys(
        receipt,
        {
            "schema",
            "runner_version",
            "strict_parse_passed",
            "provider_contact_possible",
            "accepted_arguments",
            "stderr_sha256",
            "image_digest",
            "configuration_root",
            "runtime_source_root",
            "dockerfile_bytes",
        },
        "configuration_compatibility_receipt",
    )
    configuration_root = canonical_root(value)
    if (
        receipt["schema"] != "vela.tooling.strict-config-compatibility.v1"
        or receipt["runner_version"] != value["runner_version"]
        or receipt["strict_parse_passed"] is not True
        or receipt["provider_contact_possible"] is not False
        or receipt["accepted_arguments"] != value["strict_arguments"]
        or not SHA256.fullmatch(receipt["stderr_sha256"])
        or receipt["image_digest"] != runtime["image_digest"]
        or receipt["configuration_root"] != configuration_root
        or receipt["runtime_source_root"] != runtime["runtime_source_root"]
        or receipt["dockerfile_bytes"] != runtime["dockerfile_bytes"]
    ):
        raise QualificationError("configuration_compatibility_invalid")
    return {
        "configuration_root": configuration_root,
        "output_token_ceiling": value["output_token_ceiling"],
        "image_digest": receipt["image_digest"],
        "runner_version": value["runner_version"],
        "timeout_seconds": value["timeout_seconds"],
    }


def _validate_participant_hold(
    root: Path,
    value: Any,
    runtime: dict[str, Any],
    configuration: dict[str, Any],
    schemas: dict[str, Any],
) -> dict[str, Any]:
    exact_keys(
        value,
        {"hold", "permit", "consumed_permit", "identity"},
        "participant_permit",
    )
    identity = value["identity"]
    exact_keys(
        identity,
        {
            "registration_id",
            "assignment_id",
            "participant_id",
            "run_id",
            "condition",
            "prompt_root",
            "packet_root",
        },
        "participant_permit_identity",
    )
    hold = load_json(
        safe_relative(root, value["hold"], "participant_hold"), "participant_hold"
    )
    permit = load_json(
        safe_relative(root, value["permit"], "participant_permit"), "participant_permit"
    )
    exact_keys(
        hold,
        {
            "schema",
            "status",
            "reason",
            "registration_id",
            "assignment_id",
        },
        "participant_hold",
    )
    expected = {
        **identity,
        "attempt": 1,
        "runner_version": configuration["runner_version"],
        "runtime_source_root": runtime["runtime_source_root"],
        "configuration_root": configuration["configuration_root"],
        "image_digest": runtime["image_digest"],
        "registered_schema_bytes": schemas["registered_bytes"],
        "provider_schema_bytes": schemas["provider_bytes"],
        "timeout_seconds": configuration["timeout_seconds"],
    }
    validate_permit(permit, expected, status="held")
    if (
        hold["schema"] != "vela.tooling.participant-hold.v1"
        or hold["status"] != "hold"
        or hold["reason"] != "qualification_incomplete"
        or hold["registration_id"] != identity["registration_id"]
        or hold["assignment_id"] != identity["assignment_id"]
    ):
        raise QualificationError("participant_permit_not_held")
    consumed_path = safe_relative(
        root, value["consumed_permit"], "participant_consumed_permit", must_exist=False
    )
    if consumed_path.exists() or consumed_path.is_symlink():
        raise QualificationError("participant_permit_already_consumed")
    return {
        "participant_permit_root": canonical_root(permit),
        "participant_hold_root": canonical_root(hold),
        "participant_permit_expected": expected,
    }


def _validate_capture_fixture(
    root: Path,
    value: Any,
    schemas: dict[str, Any],
    configuration: dict[str, Any],
    runtime: dict[str, Any],
) -> dict[str, Any]:
    exact_keys(
        value,
        {
            "directory",
            "permit_template",
            "consumed_permit",
            "launch",
            "events",
            "stderr",
            "raw_response",
            "terminal_receipt",
            "teardown_receipt",
            "capture_manifest",
            "identity",
        },
        "neutral_fixture",
    )
    fixture_dir = safe_relative(root, value["directory"], "neutral_fixture_directory")
    paths = {
        key: safe_relative(root, value[key], f"neutral_fixture_{key}")
        for key in value
        if key not in {"directory", "identity"}
    }
    if any(fixture_dir not in path.parents for path in paths.values()):
        raise QualificationError("neutral_fixture_path_outside_directory")
    template = load_json(paths["permit_template"], "neutral_fixture_permit_template")
    consumed = load_json(paths["consumed_permit"], "neutral_fixture_consumed_permit")
    identity = value["identity"]
    exact_keys(
        identity,
        {
            "registration_id",
            "assignment_id",
            "participant_id",
            "run_id",
            "condition",
            "prompt_root",
            "packet_root",
        },
        "neutral_fixture_identity",
    )
    expected_permit = {
        **identity,
        "attempt": 1,
        "runner_version": configuration["runner_version"],
        "runtime_source_root": runtime["runtime_source_root"],
        "configuration_root": configuration["configuration_root"],
        "image_digest": runtime["image_digest"],
        "registered_schema_bytes": schemas["registered_bytes"],
        "provider_schema_bytes": schemas["provider_bytes"],
        "timeout_seconds": configuration["timeout_seconds"],
    }
    validate_permit(template, expected_permit, status="held")
    validate_permit(consumed, expected_permit, status="consumed")
    if permit_identity(template) != permit_identity(consumed):
        raise QualificationError("neutral_fixture_permit_invalid")
    held_name = paths["consumed_permit"].name.replace(
        ".permit.consumed.json", ".permit.json"
    )
    if (paths["consumed_permit"].parent / held_name).exists():
        raise QualificationError("neutral_fixture_permit_not_atomically_consumed")
    launch_raw = read_regular(paths["launch"], "neutral_fixture_launch")
    events_raw = read_regular(paths["events"], "neutral_fixture_events")
    stderr_raw = read_regular(paths["stderr"], "neutral_fixture_stderr")
    response_raw = read_regular(paths["raw_response"], "neutral_fixture_raw_response")
    teardown_raw = read_regular(paths["teardown_receipt"], "neutral_fixture_teardown")
    launch = parse_json(launch_raw, "neutral_fixture_launch")
    response = parse_json(response_raw, "neutral_fixture_raw_response")
    receipt = load_json(paths["terminal_receipt"], "neutral_fixture_terminal_receipt")
    teardown = parse_json(teardown_raw, "neutral_fixture_teardown")
    event_summary = validate_events(events_raw, configuration["output_token_ceiling"])
    exact_keys(
        launch,
        {
            "schema",
            "run_id",
            "attempt",
            "runner_version",
            "permit_bytes",
            "configuration_root",
            "runtime_source_root",
            "image_digest",
            "started_at",
        },
        "neutral_fixture_launch",
    )
    permit_bytes = digest(
        read_regular(paths["consumed_permit"], "neutral_fixture_consumed_permit")
    )
    launch_started = parse_timestamp(launch["started_at"], "neutral_launch")
    if (
        launch["schema"] != LAUNCH_SCHEMA
        or launch["run_id"] != consumed["run_id"]
        or launch["attempt"] != consumed["attempt"]
        or isinstance(launch["attempt"], bool)
        or launch["runner_version"] != configuration["runner_version"]
        or launch["permit_bytes"] != permit_bytes
        or launch["configuration_root"] != configuration["configuration_root"]
        or launch["runtime_source_root"] != runtime["runtime_source_root"]
        or launch["image_digest"] != runtime["image_digest"]
    ):
        raise QualificationError("neutral_fixture_launch_binding_invalid")
    events = event_summary["events"]
    if (
        any(event["run_id"] != identity["run_id"] for event in events)
        or event_summary["timestamps"][0] < launch_started
    ):
        raise QualificationError("neutral_fixture_event_binding_invalid")
    validate_schema_boundary(
        schemas["registered"],
        schemas["provider"],
        schemas["deleted_pointers"],
        response,
    )
    normalized = normalize_closed_set(
        response,
        schemas["closed_set_field"],
        schemas["closed_set_key"],
        schemas["closed_set_expected"],
    )
    exact_keys(
        receipt,
        {
            "schema",
            "status",
            "permit_bytes",
            "launch_bytes",
            "provider_events_bytes",
            "provider_stderr_bytes",
            "raw_response_bytes",
            "teardown_receipt_bytes",
            "registered_schema_bytes",
            "provider_schema_bytes",
            "canonical_response_root",
            "configuration_root",
            "image_digest",
            "trust_bundle_sha256",
            "cumulative_provider_usage_is_telemetry_only",
            "credential_retained",
            "run_id",
            "attempt",
            "runner_version",
            "runtime_source_root",
            "started_at",
            "completed_at",
            "duration_seconds",
            "exit_code",
        },
        "neutral_fixture_terminal_receipt",
    )
    expected = {
        "schema": TERMINAL_SCHEMA,
        "status": "completed",
        "run_id": identity["run_id"],
        "attempt": 1,
        "runner_version": configuration["runner_version"],
        "runtime_source_root": runtime["runtime_source_root"],
        "exit_code": 0,
        "permit_bytes": permit_bytes,
        "launch_bytes": digest(launch_raw),
        "provider_events_bytes": digest(events_raw),
        "provider_stderr_bytes": digest(stderr_raw),
        "raw_response_bytes": digest(response_raw),
        "teardown_receipt_bytes": digest(teardown_raw),
        "registered_schema_bytes": schemas["registered_bytes"],
        "provider_schema_bytes": schemas["provider_bytes"],
        "canonical_response_root": canonical_root(normalized),
        "configuration_root": configuration["configuration_root"],
        "image_digest": runtime["image_digest"],
        "trust_bundle_sha256": runtime["trust_bundle_sha256"],
        "cumulative_provider_usage_is_telemetry_only": True,
        "credential_retained": False,
    }
    terminal_started = parse_timestamp(receipt["started_at"], "terminal_started")
    terminal_completed = parse_timestamp(receipt["completed_at"], "terminal_completed")
    duration = nonnegative_number(receipt["duration_seconds"], "terminal_duration")
    if (
        any(receipt.get(key) != item for key, item in expected.items())
        or isinstance(receipt["attempt"], bool)
        or isinstance(receipt["exit_code"], bool)
        or terminal_started != launch_started
        or terminal_completed < terminal_started
        or event_summary["timestamps"][-1] > terminal_completed
        or duration
        != Decimal(str((terminal_completed - terminal_started).total_seconds()))
    ):
        raise QualificationError("neutral_fixture_terminal_receipt_drift")
    exact_keys(
        teardown,
        {
            "schema",
            "process_reaped",
            "network_disabled",
            "mounts_detached",
            "completed_at",
            "run_id",
            "attempt",
            "status",
            "exit_code",
            "started_at",
            "duration_seconds",
            "permit_bytes",
            "launch_bytes",
            "provider_stderr_bytes",
        },
        "neutral_fixture_teardown",
    )
    if (
        teardown["schema"] != TEARDOWN_SCHEMA
        or teardown["run_id"] != identity["run_id"]
        or teardown["attempt"] != 1
        or isinstance(teardown["attempt"], bool)
        or teardown["status"] != "completed"
        or teardown["exit_code"] != 0
        or isinstance(teardown["exit_code"], bool)
        or teardown["permit_bytes"] != permit_bytes
        or teardown["launch_bytes"] != digest(launch_raw)
        or teardown["provider_stderr_bytes"] != digest(stderr_raw)
        or teardown["process_reaped"] is not True
        or teardown["network_disabled"] is not True
        or teardown["mounts_detached"] is not True
    ):
        raise QualificationError("neutral_fixture_teardown_incomplete")
    teardown_started = parse_timestamp(teardown["started_at"], "teardown_started")
    teardown_completed = parse_timestamp(teardown["completed_at"], "teardown_completed")
    teardown_duration = nonnegative_number(
        teardown["duration_seconds"], "teardown_duration"
    )
    if (
        teardown_started < terminal_completed
        or teardown_completed < teardown_started
        or teardown_duration
        != Decimal(str((teardown_completed - teardown_started).total_seconds()))
    ):
        raise QualificationError("neutral_fixture_teardown_time_invalid")
    message_text = event_summary["messages"][0].get("text")
    if (
        not isinstance(message_text, str)
        or parse_json(message_text.encode(), "provider_message") != response
    ):
        raise QualificationError("neutral_fixture_event_response_mismatch")
    manifest = load_json(paths["capture_manifest"], "neutral_fixture_capture_manifest")
    expected_entries = []
    for key in (
        "consumed_permit",
        "launch",
        "events",
        "stderr",
        "raw_response",
        "terminal_receipt",
        "teardown_receipt",
    ):
        raw = read_regular(paths[key], f"neutral_fixture_{key}")
        expected_entries.append(
            {
                "path": paths[key].relative_to(fixture_dir).as_posix(),
                "bytes": len(raw),
                "sha256": digest(raw),
            }
        )
    expected_entries.sort(key=lambda entry: entry["path"])
    expected_manifest = {
        "schema": "vela.tooling.neutral-capture-manifest.v1",
        "entries": expected_entries,
    }
    expected_manifest["capture_root"] = canonical_root(expected_manifest)
    if manifest != expected_manifest:
        raise QualificationError("neutral_fixture_capture_bridge_incomplete")
    return {
        "neutral_capture_root": manifest["capture_root"],
        "raw_response_bytes": digest(response_raw),
        "canonical_response_root": canonical_root(normalized),
        "input_tokens_telemetry": event_summary["usage"]["input_tokens"],
    }


@dataclass(frozen=True)
class FrozenSnapshot:
    root: str
    entries: tuple[tuple[str, bytes], ...]


def pre_key_snapshot(root: Path, manifest: dict[str, Any]) -> FrozenSnapshot:
    exact_keys(manifest, {"schema", "entries", "snapshot_root"}, "score_snapshot")
    if manifest["schema"] != "vela.tooling.pre-key-snapshot.v1":
        raise QualificationError("score_snapshot_schema_invalid")
    entries = manifest["entries"]
    if not isinstance(entries, list) or not entries:
        raise QualificationError("score_snapshot_entries_invalid")
    buffered = []
    derived = []
    seen = set()
    for entry in entries:
        exact_keys(entry, {"path", "bytes", "sha256"}, "score_snapshot_entry")
        path = safe_relative(root, entry["path"], "score_snapshot_entry")
        raw = read_regular(path, "score_snapshot_entry")
        if (
            entry["path"] in seen
            or entry["bytes"] != len(raw)
            or entry["sha256"] != digest(raw)
        ):
            raise QualificationError("score_snapshot_entry_drift")
        seen.add(entry["path"])
        buffered.append((entry["path"], raw))
        derived.append(dict(entry))
    body = {"schema": manifest["schema"], "entries": derived}
    if manifest["snapshot_root"] != canonical_root(body):
        raise QualificationError("score_snapshot_root_drift")
    return FrozenSnapshot(manifest["snapshot_root"], tuple(buffered))


def _validate_self_verification(root: Path, value: Any) -> str:
    exact_keys(
        value,
        {
            "command",
            "qualifier_sha256",
            "environment_prefix",
            "jsonschema_module",
        },
        "self_verification",
    )
    environment = Path(sys.prefix)
    executable = Path(sys.executable)
    module_path = Path(jsonschema.__file__ or "")
    if (
        sys.prefix == sys.base_prefix
        or environment not in executable.parents
        or environment not in module_path.parents
    ):
        raise QualificationError("self_verification_not_locked_environment")
    expected = [
        sys.executable,
        str(QUALIFIER),
        "--bundle",
        str(root),
    ]
    if (
        value["command"] != expected
        or value["environment_prefix"] != sys.prefix
        or value["jsonschema_module"] != str(module_path)
        or value["qualifier_sha256"] != digest(read_regular(QUALIFIER, "qualifier"))
    ):
        raise QualificationError(
            "self_verification_targets_predecessor_or_other_artifact"
        )
    return canonical_root(value)


def qualify_bundle(bundle: Path) -> dict[str, Any]:
    root = bundle if bundle.is_absolute() else Path.cwd() / bundle
    validate_bundle_tree(root)
    token = _ACTIVE_BUNDLE_ROOT.set(root)
    try:
        return _qualify_root(root)
    finally:
        _ACTIVE_BUNDLE_ROOT.reset(token)


def _qualify_root(root: Path) -> dict[str, Any]:
    config_path = safe_relative(root, "qualification.json", "qualification")
    config = load_json(config_path, "qualification")
    exact_keys(
        config,
        {
            "schema",
            "status",
            "configuration",
            "schemas",
            "runtime",
            "participant_permit",
            "neutral_fixture",
            "scoring_snapshot",
            "self_verification",
        },
        "qualification",
    )
    if config["schema"] != SCHEMA or config["status"] != "hold":
        raise QualificationError("qualification_not_held")
    schema_config = config["schemas"]
    exact_keys(
        schema_config,
        {"registered", "provider", "deleted_pointers", "valid_response", "closed_set"},
        "schemas",
    )
    registered_path = safe_relative(
        root, schema_config["registered"], "registered_schema"
    )
    provider_path = safe_relative(root, schema_config["provider"], "provider_schema")
    valid_response_path = safe_relative(
        root, schema_config["valid_response"], "valid_response"
    )
    registered_raw = read_regular(registered_path, "registered_schema")
    provider_raw = read_regular(provider_path, "provider_schema")
    valid_raw = read_regular(valid_response_path, "valid_response")
    registered = parse_json(registered_raw, "registered_schema")
    provider = parse_json(provider_raw, "provider_schema")
    valid_response = parse_json(valid_raw, "valid_response")
    closed = schema_config["closed_set"]
    exact_keys(closed, {"field", "key", "expected"}, "closed_set")
    validate_schema_boundary(
        registered, provider, schema_config["deleted_pointers"], valid_response
    )
    canonical_valid = normalize_closed_set(
        valid_response, closed["field"], closed["key"], closed["expected"]
    )
    schemas = {
        "registered": registered,
        "provider": provider,
        "deleted_pointers": schema_config["deleted_pointers"],
        "registered_bytes": digest(registered_raw),
        "provider_bytes": digest(provider_raw),
        "closed_set_field": closed["field"],
        "closed_set_key": closed["key"],
        "closed_set_expected": closed["expected"],
        "canonical_valid_response_root": canonical_root(canonical_valid),
    }
    runtime = _validate_runtime(root, config["runtime"])
    configuration = _validate_configuration(root, config["configuration"], runtime)
    if configuration["image_digest"] != runtime["image_digest"]:
        raise QualificationError("configuration_compatibility_image_drift")
    hold = _validate_participant_hold(
        root, config["participant_permit"], runtime, configuration, schemas
    )
    fixture = _validate_capture_fixture(
        root, config["neutral_fixture"], schemas, configuration, runtime
    )
    snapshot_manifest = load_json(
        safe_relative(root, config["scoring_snapshot"], "scoring_snapshot"),
        "scoring_snapshot",
    )
    snapshot = pre_key_snapshot(root, snapshot_manifest)
    self_root = _validate_self_verification(root, config["self_verification"])
    gates = {
        "configuration": True,
        "provider_schema_derivative": True,
        "draft_2020_12_local_validation": True,
        "runtime_reproducibility": True,
        "trust_and_mounts": True,
        "single_use_permit_hold": True,
        "neutral_capture_bridge": True,
        "order_independent_closed_set": True,
        "pre_key_snapshot": True,
        "canonical_decimal_serialization": True,
        "self_verification_target": True,
    }
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "status": "qualified_hold",
        "authority_effect": "none",
        "provider_calls": 0,
        "scientific_sessions": 0,
        "participant_permits_consumed": 0,
        "configuration_root": configuration["configuration_root"],
        "registered_schema_bytes": schemas["registered_bytes"],
        "provider_schema_bytes": schemas["provider_bytes"],
        "canonical_valid_response_root": schemas["canonical_valid_response_root"],
        **runtime,
        **hold,
        **fixture,
        "pre_key_snapshot_root": snapshot.root,
        "self_verification_root": self_root,
        "gates": gates,
    }
    receipt["qualification_root"] = canonical_root(receipt)
    return receipt


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bundle", type=Path, required=True)
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        receipt = qualify_bundle(args.bundle)
    except QualificationError as error:
        print(
            json.dumps({"status": "blocked", "error": str(error)}, sort_keys=True),
            file=sys.stderr,
        )
        return 2
    sys.stdout.buffer.write(canonical_json_bytes(receipt))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
