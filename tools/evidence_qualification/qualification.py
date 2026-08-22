#!/usr/bin/env python3
"""Fail-closed qualification and custody for neutral evidence runtimes.

This module is tooling, not Protocol 1. It validates one pre-execution bundle
and one no-science capture fixture. It never invokes a provider, mints a
participant permit, opens protected answers, or changes Repository state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import tarfile
from collections.abc import Iterable
from dataclasses import dataclass
from decimal import ROUND_HALF_EVEN, Decimal, InvalidOperation
from pathlib import Path
from typing import Any

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
    resolved_root = root.resolve()
    try:
        resolved = path.resolve(strict=must_exist)
    except FileNotFoundError as error:
        raise QualificationError(f"{label}_missing") from error
    if resolved != resolved_root and resolved_root not in resolved.parents:
        raise QualificationError(f"{label}_escapes_bundle")
    if must_exist and path.is_symlink():
        raise QualificationError(f"{label}_symlink_forbidden")
    return path


def read_regular(path: Path, label: str) -> bytes:
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
        text = raw.decode("utf-8")
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


def consume_permit(directory: Path, run_id: str) -> Path:
    """Atomically consume one permit without an overwrite-capable rename."""
    if not re.fullmatch(r"[-a-z0-9]+", run_id):
        raise QualificationError("permit_run_id_invalid")
    source = directory / f"{run_id}.permit.json"
    consumed = directory / f"{run_id}.permit.consumed.json"
    if source.is_symlink() or not source.is_file():
        raise QualificationError("permit_source_missing_or_unsafe")
    try:
        os.link(source, consumed, follow_symlinks=False)
    except FileExistsError as error:
        raise QualificationError("permit_already_consumed") from error
    except OSError as error:
        raise QualificationError("permit_atomic_consume_failed") from error
    try:
        source.unlink()
    except OSError as error:
        consumed.unlink(missing_ok=True)
        raise QualificationError("permit_atomic_consume_failed") from error
    return consumed


def permit_identity(value: dict[str, Any]) -> dict[str, Any]:
    return {
        key: item for key, item in value.items() if key not in {"status", "expires_at"}
    }


def validate_events(raw: bytes, output_token_ceiling: int) -> dict[str, Any]:
    events = []
    for index, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        event = parse_json(line, f"provider_event_{index}")
        if not isinstance(event, dict):
            raise QualificationError("provider_event_not_object")
        event_type = str(event.get("type", ""))
        item = event.get("item")
        item_type = str(item.get("type", "")) if isinstance(item, dict) else ""
        if FORBIDDEN_EVENT.search(f"{event_type}:{item_type}"):
            raise QualificationError("provider_event_forbidden")
        events.append(event)
    types = [str(event.get("type", "")) for event in events]
    messages = [
        event["item"]
        for event in events
        if isinstance(event.get("item"), dict)
        and event["item"].get("type") in {"agent_message", "message"}
    ]
    if (
        types.count("thread.started"),
        types.count("turn.started"),
        types.count("turn.completed"),
        len(messages),
    ) != (1, 1, 1, 1):
        raise QualificationError("provider_event_sequence_invalid")
    usage_events = [
        event["usage"] for event in events if isinstance(event.get("usage"), dict)
    ]
    if not usage_events:
        raise QualificationError("provider_usage_missing")
    usage = usage_events[-1]
    for key, value in usage.items():
        if "token" in key and (
            isinstance(value, bool) or not isinstance(value, int) or value < 0
        ):
            raise QualificationError(f"provider_usage_invalid:{key}")
    for key in ("input_tokens", "cached_input_tokens", "output_tokens"):
        if key not in usage:
            raise QualificationError(f"provider_usage_missing:{key}")
    if usage["output_tokens"] > output_token_ceiling:
        raise QualificationError("provider_output_token_ceiling")
    return {"events": events, "messages": messages, "usage": usage}


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


def _oci_identity(raw: bytes) -> tuple[str, str, str]:
    import io

    try:
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:*") as archive:
            index_raw = archive.extractfile("index.json").read()  # type: ignore[union-attr]
            index = parse_json(index_raw, "oci_index")
            manifests = index.get("manifests") if isinstance(index, dict) else None
            if not isinstance(manifests, list) or len(manifests) != 1:
                raise QualificationError("oci_manifest_count_invalid")
            manifest_digest = manifests[0].get("digest")
            if not isinstance(manifest_digest, str) or not SHA256.fullmatch(
                manifest_digest
            ):
                raise QualificationError("oci_manifest_digest_invalid")
            manifest_raw = archive.extractfile(
                "blobs/sha256/" + manifest_digest.removeprefix("sha256:")
            ).read()  # type: ignore[union-attr]
            if digest(manifest_raw) != manifest_digest:
                raise QualificationError("oci_manifest_bytes_drift")
            manifest = parse_json(manifest_raw, "oci_manifest")
            config_digest = manifest.get("config", {}).get("digest")
            if not isinstance(config_digest, str) or not SHA256.fullmatch(
                config_digest
            ):
                raise QualificationError("oci_config_digest_invalid")
            config_raw = archive.extractfile(
                "blobs/sha256/" + config_digest.removeprefix("sha256:")
            ).read()  # type: ignore[union-attr]
            if digest(config_raw) != config_digest:
                raise QualificationError("oci_config_bytes_drift")
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
    return manifest_digest, config_digest, f"{operating_system}/{architecture}"


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
            or receipt["image_digest"] != identity[0]
            or receipt["config_digest"] != identity[1]
            or receipt["platform"] != identity[2]
            or receipt["oci_tar_bytes"] != digest(raw)
        ):
            raise QualificationError("oci_receipt_binding_invalid")
        builders.add(receipt["builder"])
    return identities[0][0], identities[0][1]


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
    dockerfile = read_regular(source_dir / "Dockerfile", "runtime_dockerfile").decode()
    if (
        "ARG SOURCE_DATE_EPOCH" not in dockerfile
        or "RUN --network=none" not in dockerfile
        or NETWORK_PACKAGE_METADATA.search(dockerfile)
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
        {"account", "fixed_day", "fixtures", "normalized_sha256"},
        "account_database",
    )
    if not isinstance(account["fixtures"], list) or len(account["fixtures"]) < 2:
        raise QualificationError("account_database_fixtures_invalid")
    normalized = [
        normalize_shadow_account(
            read_regular(
                safe_relative(root, fixture, "account_fixture"), "account_fixture"
            ),
            account["account"],
            account["fixed_day"],
        )
        for fixture in account["fixtures"]
    ]
    if (
        len(set(normalized)) != 1
        or digest(normalized[0]) != account["normalized_sha256"]
    ):
        raise QualificationError("account_database_not_date_invariant")
    return {
        "runtime_source_root": source_root,
        "build_inputs_root": build_inputs_root,
        "image_digest": image_digest,
        "image_config_digest": config_digest,
        "trust_bundle_sha256": digest(trust_raw),
    }


def _validate_configuration(root: Path, value: Any) -> dict[str, Any]:
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
        },
        "configuration_compatibility_receipt",
    )
    if (
        receipt["schema"] != "vela.tooling.strict-config-compatibility.v1"
        or receipt["strict_parse_passed"] is not True
        or receipt["provider_contact_possible"] is not False
        or receipt["accepted_arguments"] != value["strict_arguments"]
        or not SHA256.fullmatch(receipt["stderr_sha256"])
        or not SHA256.fullmatch(receipt["image_digest"])
    ):
        raise QualificationError("configuration_compatibility_invalid")
    return {
        "configuration_root": canonical_root(value),
        "output_token_ceiling": value["output_token_ceiling"],
        "image_digest": receipt["image_digest"],
    }


def _validate_participant_hold(
    root: Path, value: Any, runtime: dict[str, Any]
) -> dict[str, Any]:
    exact_keys(value, {"hold", "permit", "consumed_permit"}, "participant_permit")
    hold = load_json(
        safe_relative(root, value["hold"], "participant_hold"), "participant_hold"
    )
    permit = load_json(
        safe_relative(root, value["permit"], "participant_permit"), "participant_permit"
    )
    exact_keys(hold, {"schema", "status", "reason"}, "participant_hold")
    if not isinstance(permit, dict):
        raise QualificationError("participant_permit_not_object")
    if (
        hold["status"] != "hold"
        or permit.get("status") != "held"
        or permit.get("expires_at") != "not_authorized"
        or permit.get("image_digest") != runtime["image_digest"]
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
        },
        "neutral_fixture",
    )
    fixture_dir = safe_relative(root, value["directory"], "neutral_fixture_directory")
    paths = {
        key: safe_relative(root, value[key], f"neutral_fixture_{key}")
        for key in value
        if key != "directory"
    }
    if any(fixture_dir not in path.parents for path in paths.values()):
        raise QualificationError("neutral_fixture_path_outside_directory")
    template = load_json(paths["permit_template"], "neutral_fixture_permit_template")
    consumed = load_json(paths["consumed_permit"], "neutral_fixture_consumed_permit")
    if not isinstance(template, dict) or not isinstance(consumed, dict):
        raise QualificationError("neutral_fixture_permit_not_object")
    if (
        permit_identity(template) != permit_identity(consumed)
        or template.get("status") != "held"
        or consumed.get("status") != "authorized"
    ):
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
    exact_keys(launch, {"schema", "run_id", "permit_bytes"}, "neutral_fixture_launch")
    if launch["run_id"] != consumed.get("run_id") or launch["permit_bytes"] != digest(
        read_regular(paths["consumed_permit"], "neutral_fixture_consumed_permit")
    ):
        raise QualificationError("neutral_fixture_launch_binding_invalid")
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
        },
        "neutral_fixture_terminal_receipt",
    )
    expected = {
        "status": "completed",
        "permit_bytes": digest(
            read_regular(paths["consumed_permit"], "neutral_fixture_consumed_permit")
        ),
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
    if any(receipt.get(key) != item for key, item in expected.items()):
        raise QualificationError("neutral_fixture_terminal_receipt_drift")
    exact_keys(
        teardown,
        {
            "schema",
            "process_reaped",
            "network_disabled",
            "mounts_detached",
            "completed_at",
        },
        "neutral_fixture_teardown",
    )
    if (
        teardown["process_reaped"] is not True
        or teardown["network_disabled"] is not True
        or teardown["mounts_detached"] is not True
    ):
        raise QualificationError("neutral_fixture_teardown_incomplete")
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
    exact_keys(value, {"command", "qualifier_sha256"}, "self_verification")
    expected = [
        str(Path(sys.executable).resolve()),
        str(QUALIFIER),
        "--bundle",
        str(root.resolve()),
    ]
    if value["command"] != expected or value["qualifier_sha256"] != digest(
        read_regular(QUALIFIER, "qualifier")
    ):
        raise QualificationError(
            "self_verification_targets_predecessor_or_other_artifact"
        )
    return canonical_root(value)


def qualify_bundle(bundle: Path) -> dict[str, Any]:
    root = bundle.resolve(strict=True)
    if not root.is_dir() or root.is_symlink():
        raise QualificationError("bundle_directory_invalid")
    config = load_json(root / "qualification.json", "qualification")
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
    configuration = _validate_configuration(root, config["configuration"])
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
    if configuration["image_digest"] != runtime["image_digest"]:
        raise QualificationError("configuration_compatibility_image_drift")
    hold = _validate_participant_hold(root, config["participant_permit"], runtime)
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
