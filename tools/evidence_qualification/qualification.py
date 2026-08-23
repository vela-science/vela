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
LEGACY_PROVIDER_DELETIONS = {"uniqueItems": True}
PROVIDER_ADAPTERS = {
    "openai-responses-v1": ("openai", "responses/v1"),
    "anthropic-messages-v1": ("anthropic", "messages/v1"),
}
# Provider derivation is selected by the exact registered-schema bytes and
# adapter. Each entry is a closed ordered transformation, not permission to
# delete a keyword with the same spelling elsewhere.
NEUTRAL_REGISTERED_SCHEMA_SHA256 = (
    "sha256:d25a5b53c3e715806b38b7d63511ce5ac118137b5a42a3a3e2a81792218082ad"
)
STAGE_A_REGISTERED_SCHEMA_SHA256 = (
    "sha256:b2d9bee1c76bc1f25f134fd50697f4e4a820a36bd61a84081edd5c542d749268"
)
PROVIDER_SCHEMA_RULES = {
    NEUTRAL_REGISTERED_SCHEMA_SHA256: {
        "openai-responses-v1": (
            ("/properties/items/uniqueItems", "uniqueItems", True),
            ("/properties/items/minItems", "minItems", 3),
        ),
        "anthropic-messages-v1": (
            ("/properties/items/uniqueItems", "uniqueItems", True),
            ("/properties/items/minItems", "minItems", 3),
        ),
    },
    STAGE_A_REGISTERED_SCHEMA_SHA256: {
        "openai-responses-v1": (
            ("/properties/impact_closure/uniqueItems", "uniqueItems", True),
            (
                "/properties/impact_closure/items/properties/evidence_ids/minItems",
                "minItems",
                1,
            ),
            (
                "/properties/impact_closure/items/properties/evidence_ids/uniqueItems",
                "uniqueItems",
                True,
            ),
            ("/properties/uncertainty/uniqueItems", "uniqueItems", True),
        ),
        "anthropic-messages-v1": (
            ("/properties/impact_closure/uniqueItems", "uniqueItems", True),
            (
                "/properties/impact_closure/items/properties/evidence_ids/minItems",
                "minItems",
                1,
            ),
            (
                "/properties/impact_closure/items/properties/evidence_ids/uniqueItems",
                "uniqueItems",
                True,
            ),
            ("/properties/uncertainty/uniqueItems", "uniqueItems", True),
        ),
    },
}
TOOL_BOUNDARY_SCHEMA = "vela.tooling.read-only-offline-tool-boundary.v1"
TOOL_BOUNDARY_SCHEMA_V2 = "vela.tooling.read-only-offline-tool-boundary.v2"
TOOL_RECEIPT_SCHEMA = "vela.tooling.read-only-tool-receipt.v1"
RAW_PROVIDER_EVENT_SCHEMA = "vela.tooling.raw-provider-event.v1"
PROVIDER_EQUIVALENCE_SCHEMA = "vela.tooling.provider-equivalence.v1"
QUALIFIER = Path(__file__).resolve()
PERMIT_SCHEMA = "vela.tooling.closed-launch-permit.v1"
RUNNER_VERSION = "neutral-runner/1"
EVENT_SCHEMA = "vela.tooling.provider-event.v1"
LAUNCH_SCHEMA = "vela.tooling.neutral-launch.v1"
TERMINAL_SCHEMA = "vela.tooling.neutral-terminal-receipt.v1"
TEARDOWN_SCHEMA = "vela.tooling.neutral-teardown.v1"
TOOL_INPUT_SCHEMAS = {
    ("shell", "1"): {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": False,
        "required": ["argv", "cwd"],
        "properties": {
            "argv": {
                "type": "array",
                "minItems": 1,
                "items": {"type": "string", "minLength": 1},
            },
            "cwd": {"type": "string", "minLength": 1, "pattern": "^/"},
        },
    },
    ("read_file", "1"): {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": False,
        "required": ["operation", "path"],
        "properties": {
            "operation": {"type": "string", "enum": ["read", "list", "stat"]},
            "path": {"type": "string", "minLength": 1, "pattern": "^/"},
        },
    },
    ("read_file", "2"): {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": False,
        "required": ["operation", "path", "query"],
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["read", "list", "stat", "search"],
            },
            "path": {"type": "string", "minLength": 1, "pattern": "^/"},
            "query": {"type": "string", "maxLength": 256},
        },
    },
}
# This is the complete shell vocabulary needed by offline-shell-files/1.
# --no-optional-locks prevents Git's read path from refreshing the index.
SHELL_ARGV_VOCABULARIES = {
    "1": (("git", "--no-optional-locks", "status", "--short"),),
}
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
                if metadata.st_nlink != 1:
                    raise QualificationError("bundle_file_link_count_invalid")
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
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            raise QualificationError(f"{label}_not_regular")
        if metadata.st_nlink != 1:
            raise QualificationError(f"{label}_link_count_invalid")
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            raw = handle.read()
            final_metadata = os.fstat(handle.fileno())
            if (
                final_metadata.st_dev != metadata.st_dev
                or final_metadata.st_ino != metadata.st_ino
                or final_metadata.st_nlink != 1
            ):
                raise QualificationError(f"{label}_custody_changed_during_read")
            return raw
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
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            raise QualificationError(f"{label}_not_regular")
        if metadata.st_nlink != 1:
            raise QualificationError(f"{label}_link_count_invalid")
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            raw = handle.read()
            final_metadata = os.fstat(handle.fileno())
            if (
                final_metadata.st_dev != metadata.st_dev
                or final_metadata.st_ino != metadata.st_ino
                or final_metadata.st_nlink != 1
            ):
                raise QualificationError(f"{label}_custody_changed_during_read")
            return raw
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


def _pointer_parent(value: Any, pointer: str) -> tuple[dict[str, Any], str]:
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
    return parent, keyword


def _same_json_value(left: Any, right: Any) -> bool:
    return type(left) is type(right) and left == right


def _deletion_rule(
    rule: Any, adapter: str | None, registered_schema_sha256: str | None
) -> tuple[str, str, Any]:
    if isinstance(rule, str):
        if adapter is not None:
            raise QualificationError("provider_legacy_deletion_forbidden")
        parent_keyword = rule.rsplit("/", 1)[-1].replace("~1", "/").replace("~0", "~")
        if parent_keyword not in LEGACY_PROVIDER_DELETIONS:
            raise QualificationError("provider_deleted_keyword_not_proven")
        return rule, parent_keyword, LEGACY_PROVIDER_DELETIONS[parent_keyword]
    exact_keys(rule, {"pointer", "keyword", "expected_value"}, "provider_deletion")
    if adapter not in PROVIDER_ADAPTERS:
        raise QualificationError("provider_adapter_unknown")
    registry = PROVIDER_SCHEMA_RULES.get(registered_schema_sha256)
    if registry is None or adapter not in registry:
        raise QualificationError("provider_registered_schema_not_registered")
    pointer = rule["pointer"]
    keyword = rule["keyword"]
    expected = rule["expected_value"]
    if (
        not isinstance(pointer, str)
        or not isinstance(keyword, str)
        or pointer.rsplit("/", 1)[-1].replace("~1", "/").replace("~0", "~") != keyword
        or not any(
            pointer == registered_pointer
            and keyword == registered_keyword
            and _same_json_value(expected, registered_expected)
            for registered_pointer, registered_keyword, registered_expected in (
                registry[adapter]
            )
        )
    ):
        raise QualificationError("provider_deletion_rule_not_registered")
    return pointer, keyword, expected


def _delete_pointer(
    value: Any,
    rule: Any,
    adapter: str | None,
    registered_schema_sha256: str | None,
) -> None:
    pointer, keyword, expected = _deletion_rule(rule, adapter, registered_schema_sha256)
    parent, observed_keyword = _pointer_parent(value, pointer)
    if observed_keyword != keyword or not _same_json_value(parent[keyword], expected):
        raise QualificationError("provider_deleted_expected_value_drift")
    del parent[keyword]


def provider_derivative(
    registered: dict[str, Any],
    deletions: list[Any],
    adapter: str | None = None,
    registered_schema_sha256: str | None = None,
) -> dict[str, Any]:
    if not isinstance(deletions, list) or (not deletions and adapter is None):
        raise QualificationError("provider_deleted_pointers_invalid")
    identities = tuple(
        _deletion_rule(deletion, adapter, registered_schema_sha256)
        for deletion in deletions
    )
    registry = PROVIDER_SCHEMA_RULES.get(registered_schema_sha256, {})
    if adapter is not None and identities != registry.get(adapter):
        raise QualificationError("provider_deletion_sequence_not_registered")
    if len({canonical_json_bytes(identity) for identity in identities}) != len(
        identities
    ):
        raise QualificationError("provider_deleted_pointers_invalid")
    derived = parse_json(canonical_json_bytes(registered), "registered_schema_copy")
    for deletion in deletions:
        _delete_pointer(derived, deletion, adapter, registered_schema_sha256)
    return derived


def validate_schema_boundary(
    registered: Any,
    provider: Any,
    deleted_pointers: Any,
    response: Any,
    adapter: str | None = None,
    registered_schema_sha256: str | None = None,
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
        registered, deleted_pointers, adapter, registered_schema_sha256
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


def _closed_absolute_path(value: Any, label: str) -> Path:
    if not isinstance(value, str):
        raise QualificationError(f"{label}_invalid")
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts:
        raise QualificationError(f"{label}_invalid")
    return path


def _mount_content_root(source: Path) -> str:
    if source.is_symlink() or not source.exists():
        raise QualificationError("tool_mount_source_unsafe")
    if source.is_file():
        metadata = source.stat()
        if metadata.st_nlink != 1:
            raise QualificationError("tool_mount_hardlink_forbidden")
        return digest(read_regular(source, "tool_mount_source"))
    if not source.is_dir():
        raise QualificationError("tool_mount_source_unsafe")
    identities: set[tuple[int, int]] = set()
    entries = []
    for path in sorted(source.rglob("*")):
        metadata = os.lstat(path)
        if stat.S_ISLNK(metadata.st_mode):
            raise QualificationError("tool_mount_symlink_forbidden")
        if stat.S_ISREG(metadata.st_mode):
            if metadata.st_nlink != 1:
                raise QualificationError("tool_mount_hardlink_forbidden")
            identity = (metadata.st_dev, metadata.st_ino)
            if identity in identities:
                raise QualificationError("tool_mount_inode_reuse")
            identities.add(identity)
            raw = read_regular(path, "tool_mount_source")
            entries.append(
                {
                    "path": path.relative_to(source).as_posix(),
                    "bytes": len(raw),
                    "sha256": digest(raw),
                }
            )
    return canonical_root(entries)


def _validate_tool_input_schema(value: Any, name: str, version: str) -> str:
    if not isinstance(value, dict):
        raise QualificationError("tool_input_schema_invalid")
    if (
        value.get("$schema") != "https://json-schema.org/draft/2020-12/schema"
        or value.get("type") != "object"
        or value.get("additionalProperties") is not False
    ):
        raise QualificationError("tool_input_schema_not_closed")
    try:
        Draft202012Validator.check_schema(value)
    except Exception as error:
        raise QualificationError("tool_input_schema_invalid") from error
    expected = TOOL_INPUT_SCHEMAS.get((name, version))
    if expected is None or canonical_json_bytes(value) != canonical_json_bytes(
        expected
    ):
        raise QualificationError("tool_input_schema_not_registered")
    return canonical_root({"name": name, "schema": value})


def _validate_allowed_argv(value: Any, version: str) -> tuple[tuple[str, ...], ...]:
    if not isinstance(value, list) or not all(isinstance(argv, list) for argv in value):
        raise QualificationError("tool_allowed_argv_invalid")
    observed = tuple(tuple(argv) for argv in value)
    expected = SHELL_ARGV_VOCABULARIES.get(version)
    if expected is None or observed != expected:
        raise QualificationError("tool_allowed_argv_not_registered")
    return observed


def validate_tool_boundary(value: Any) -> dict[str, Any]:
    """Validate a maintained offline read-only tool vocabulary and workspace."""

    version_two = (
        isinstance(value, dict) and value.get("schema") == TOOL_BOUNDARY_SCHEMA_V2
    )
    fields = {
        "schema",
        "mode",
        "provider_adapter",
        "provider_organization",
        "api_version",
        "tool_protocol_version",
        "tools",
        "mounts",
        "network",
        "writes",
        "shell_interpolation",
        "max_output_bytes",
        "per_call_timeout_seconds",
        "lifecycle",
    }
    if version_two:
        fields.add("max_tool_calls")
    exact_keys(value, fields, "tool_boundary")
    adapter = value["provider_adapter"]
    expected_provider = PROVIDER_ADAPTERS.get(adapter)
    if (
        value["schema"] not in {TOOL_BOUNDARY_SCHEMA, TOOL_BOUNDARY_SCHEMA_V2}
        or value["mode"]
        != (
            "read_only_offline_files"
            if version_two
            else "read_only_offline_shell_files"
        )
        or expected_provider is None
        or (value["provider_organization"], value["api_version"]) != expected_provider
        or value["tool_protocol_version"]
        != ("offline-files/2" if version_two else "offline-shell-files/1")
        or value["network"] is not False
        or value["writes"] is not False
        or value["shell_interpolation"] is not False
        or isinstance(value["max_output_bytes"], bool)
        or not isinstance(value["max_output_bytes"], int)
        or value["max_output_bytes"] <= 0
        or isinstance(value["per_call_timeout_seconds"], bool)
        or not isinstance(value["per_call_timeout_seconds"], int)
        or value["per_call_timeout_seconds"] <= 0
        or value["lifecycle"]
        != [
            "thread.started",
            "turn.started",
            "tool.call",
            "tool.result",
            "item.completed",
            "turn.completed",
        ]
        or (
            version_two
            and (
                isinstance(value["max_tool_calls"], bool)
                or not isinstance(value["max_tool_calls"], int)
                or value["max_tool_calls"] < 1
                or value["max_tool_calls"] > 64
            )
        )
    ):
        raise QualificationError("tool_boundary_contract_invalid")
    tools = value["tools"]
    if not isinstance(tools, list) or len(tools) != (1 if version_two else 2):
        raise QualificationError("tool_boundary_tools_invalid")
    by_name: dict[str, dict[str, Any]] = {}
    schema_roots = []
    allowed_argv: tuple[tuple[str, ...], ...] = ()
    file_roots: tuple[Path, ...] = ()
    for tool in tools:
        exact_keys(
            tool,
            {
                "name",
                "version",
                "input_schema",
                "operations",
                "allowed_argv",
                "file_roots",
            },
            "tool_definition",
        )
        name = tool["name"]
        allowed_names = {"read_file"} if version_two else {"shell", "read_file"}
        if name in by_name or name not in allowed_names:
            raise QualificationError("tool_name_invalid")
        version = tool["version"]
        if version != ("2" if version_two else "1"):
            raise QualificationError("tool_version_invalid")
        roots = (
            tuple(
                _closed_absolute_path(item, "tool_file_root")
                for item in tool["file_roots"]
            )
            if isinstance(tool["file_roots"], list)
            else ()
        )
        if not roots:
            raise QualificationError("tool_file_roots_invalid")
        schema_roots.append(
            _validate_tool_input_schema(tool["input_schema"], name, version)
        )
        if name == "shell":
            if tool["operations"] != ["execute"]:
                raise QualificationError("tool_operations_invalid")
            allowed_argv = _validate_allowed_argv(tool["allowed_argv"], version)
        else:
            if (
                tool["operations"]
                != (
                    ["read", "list", "stat", "search"]
                    if version_two
                    else ["read", "list", "stat"]
                )
                or tool["allowed_argv"] != []
            ):
                raise QualificationError("tool_operations_invalid")
        if file_roots and roots != file_roots:
            raise QualificationError("tool_file_roots_not_equivalent")
        file_roots = roots
        by_name[name] = tool
    mounts = value["mounts"]
    if not isinstance(mounts, list) or not mounts:
        raise QualificationError("tool_mounts_invalid")
    targets = []
    for mount in mounts:
        exact_keys(
            mount, {"source", "target", "read_only", "content_root"}, "tool_mount"
        )
        source = _closed_absolute_path(mount["source"], "tool_mount_source")
        target = _closed_absolute_path(mount["target"], "tool_mount_target")
        if source != Path(os.path.abspath(source)) or source != source.resolve():
            raise QualificationError("tool_mount_source_not_canonical")
        if mount["read_only"] is not True or mount[
            "content_root"
        ] != _mount_content_root(source):
            raise QualificationError("tool_mount_binding_invalid")
        if target in targets:
            raise QualificationError("tool_mount_target_duplicate")
        targets.append(target)
    if tuple(targets) != file_roots:
        raise QualificationError("tool_mount_file_root_mismatch")
    workspace_content_root = canonical_root(
        [
            {"content_root": mount["content_root"], "target": mount["target"]}
            for mount in mounts
        ]
    )
    policy = {
        key: item
        for key, item in value.items()
        if key
        not in {"provider_adapter", "provider_organization", "api_version", "mounts"}
    }
    policy["mount_targets"] = [mount["target"] for mount in mounts]
    return {
        "version_two": version_two,
        "adapter": adapter,
        "provider_organization": value["provider_organization"],
        "tool_boundary_root": canonical_root(value),
        "tool_semantics_root": canonical_root(policy),
        "tool_policy_root": canonical_root(policy),
        "workspace_content_root": workspace_content_root,
        "tool_schema_roots": schema_roots,
        "allowed_argv": allowed_argv,
        "file_roots": file_roots,
        "max_output_bytes": value["max_output_bytes"],
        "per_call_timeout_seconds": value["per_call_timeout_seconds"],
        "max_tool_calls": value.get("max_tool_calls", 1),
    }


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


def _permit_fields(expected: dict[str, Any] | None = None) -> set[str]:
    fields = {
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
    optional = {
        "tool_boundary_root",
        "tool_policy_root",
        "workspace_content_root",
        "evidence_manifest_root",
        "workspace_preflight_root",
    }
    return fields | (
        {key for key in optional if key in expected} if expected else set()
    )


def validate_permit(
    value: Any,
    expected: dict[str, Any],
    *,
    status: str,
) -> None:
    exact_keys(value, _permit_fields(expected), "permit")
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
        "tool_boundary_root",
        "tool_policy_root",
        "workspace_content_root",
        "evidence_manifest_root",
        "workspace_preflight_root",
    ):
        if key in value and (
            not isinstance(value[key], str) or not SHA256.fullmatch(value[key])
        ):
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
    source_descriptor = -1
    consumed_descriptor = -1
    linked = False
    try:
        try:
            source_descriptor = os.open(
                source_name,
                os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=directory_descriptor,
            )
            validated_metadata = os.fstat(source_descriptor)
            if not stat.S_ISREG(validated_metadata.st_mode):
                raise QualificationError("permit_source_not_regular")
            if validated_metadata.st_nlink != 1:
                raise QualificationError("permit_source_link_count_invalid")
            raw = _read_open_descriptor(source_descriptor)
            after_read = os.fstat(source_descriptor)
            if (
                (after_read.st_dev, after_read.st_ino)
                != (validated_metadata.st_dev, validated_metadata.st_ino)
                or after_read.st_nlink != 1
                or after_read.st_size != len(raw)
            ):
                raise QualificationError("permit_source_changed_during_validation")
            permit = parse_json(raw, "permit_source")
            validate_permit(permit, expected, status="held")
            os.link(
                source_name,
                consumed_name,
                src_dir_fd=directory_descriptor,
                dst_dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
            linked = True
            consumed_descriptor = os.open(
                consumed_name,
                os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=directory_descriptor,
            )
            consumed_metadata = os.fstat(consumed_descriptor)
            source_after_link = os.fstat(source_descriptor)
            expected_inode = (validated_metadata.st_dev, validated_metadata.st_ino)
            if (
                (consumed_metadata.st_dev, consumed_metadata.st_ino) != expected_inode
                or (source_after_link.st_dev, source_after_link.st_ino)
                != expected_inode
                or consumed_metadata.st_nlink != 2
                or source_after_link.st_nlink != 2
                or _read_open_descriptor(consumed_descriptor) != raw
                or _read_open_descriptor(source_descriptor) != raw
            ):
                raise QualificationError("permit_consumed_inode_or_bytes_mismatch")
            named_source_descriptor = os.open(
                source_name,
                os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=directory_descriptor,
            )
            try:
                named_source_metadata = os.fstat(named_source_descriptor)
                if (
                    named_source_metadata.st_dev,
                    named_source_metadata.st_ino,
                ) != expected_inode or _read_open_descriptor(
                    named_source_descriptor
                ) != raw:
                    raise QualificationError("permit_source_replaced_before_unlink")
            finally:
                os.close(named_source_descriptor)
            os.unlink(source_name, dir_fd=directory_descriptor)
            linked = False
            final_metadata = os.fstat(consumed_descriptor)
            if (
                (final_metadata.st_dev, final_metadata.st_ino) != expected_inode
                or final_metadata.st_nlink != 1
                or _read_open_descriptor(consumed_descriptor) != raw
            ):
                raise QualificationError("permit_consumed_custody_invalid")
        except FileExistsError as error:
            raise QualificationError("permit_already_consumed") from error
        except OSError as error:
            raise QualificationError("permit_atomic_consume_failed") from error
        except QualificationError:
            if linked:
                try:
                    os.unlink(consumed_name, dir_fd=directory_descriptor)
                except FileNotFoundError:
                    pass
            raise
    finally:
        if consumed_descriptor >= 0:
            os.close(consumed_descriptor)
        if source_descriptor >= 0:
            os.close(source_descriptor)
        os.close(directory_descriptor)
    return consumed


def _read_open_descriptor(descriptor: int) -> bytes:
    try:
        os.lseek(descriptor, 0, os.SEEK_SET)
        chunks = []
        while True:
            chunk = os.read(descriptor, 1024 * 64)
            if not chunk:
                return b"".join(chunks)
            chunks.append(chunk)
    except OSError as error:
        raise QualificationError("permit_descriptor_read_failed") from error


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


def _parse_event_lines(raw: bytes, label: str) -> list[dict[str, Any]]:
    events = []
    for index, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        event = parse_json(line, f"{label}_{index}")
        if not isinstance(event, dict):
            raise QualificationError(f"{label}_not_object")
        events.append(event)
    return events


def _path_inside_roots(path: Path, roots: tuple[Path, ...]) -> bool:
    return any(path == root or root in path.parents for root in roots)


def _validate_tool_call(
    item: Any, boundary: dict[str, Any]
) -> tuple[str, str, dict[str, Any]]:
    exact_keys(item, {"type", "call_id", "tool_name", "arguments"}, "tool_call")
    if item["type"] != "tool_call":
        raise QualificationError("tool_call_type_invalid")
    call_id = item["call_id"]
    tool_name = item["tool_name"]
    arguments = item["arguments"]
    if (
        not isinstance(call_id, str)
        or not call_id
        or tool_name not in {"shell", "read_file"}
        or not isinstance(arguments, dict)
    ):
        raise QualificationError("tool_call_invalid")
    if tool_name == "shell":
        exact_keys(arguments, {"argv", "cwd"}, "shell_arguments")
        argv = arguments["argv"]
        cwd = _closed_absolute_path(arguments["cwd"], "shell_cwd")
        if (
            not isinstance(argv, list)
            or any(not isinstance(arg, str) or not arg for arg in argv)
            or tuple(argv) not in boundary["allowed_argv"]
            or not _path_inside_roots(cwd, boundary["file_roots"])
        ):
            raise QualificationError("shell_call_not_allowlisted")
    else:
        version_two = not boundary["allowed_argv"]
        exact_keys(
            arguments,
            {"operation", "path", "query"} if version_two else {"operation", "path"},
            "file_arguments",
        )
        path = _closed_absolute_path(arguments["path"], "file_path")
        operation = arguments["operation"]
        query = arguments.get("query")
        if (
            operation
            not in (
                {"read", "list", "stat", "search"}
                if version_two
                else {"read", "list", "stat"}
            )
            or not _path_inside_roots(path, boundary["file_roots"])
            or (
                version_two
                and (
                    not isinstance(query, str) or (operation == "search") != bool(query)
                )
            )
        ):
            raise QualificationError("file_call_not_allowlisted")
    return call_id, tool_name, arguments


def validate_tool_events(
    raw: bytes, output_token_ceiling: int, boundary: dict[str, Any]
) -> dict[str, Any]:
    events = _parse_event_lines(raw, "provider_event")
    types = [event.get("type") for event in events]
    if (
        len(events) < 6
        or types[:2] != ["thread.started", "turn.started"]
        or types[-2:] != ["item.completed", "turn.completed"]
        or len(types[2:-2]) % 2 != 0
        or any(
            pair != ["tool.call", "tool.result"]
            for pair in (
                types[index : index + 2] for index in range(2, len(types) - 2, 2)
            )
        )
    ):
        raise QualificationError("provider_tool_event_sequence_invalid")
    tool_count = len(types[2:-2]) // 2
    if tool_count < 1 or tool_count > boundary["max_tool_calls"]:
        raise QualificationError("provider_tool_event_count_invalid")
    for event in events:
        if event.get("schema") != EVENT_SCHEMA:
            raise QualificationError("provider_event_schema_invalid")
    exact_keys(
        events[0], {"schema", "type", "run_id", "thread_id", "at"}, "thread_started"
    )
    exact_keys(
        events[1],
        {"schema", "type", "run_id", "thread_id", "turn_id", "at"},
        "turn_started",
    )
    common_item_keys = {
        "schema",
        "type",
        "run_id",
        "thread_id",
        "turn_id",
        "response_id",
        "at",
        "item",
    }
    for index in range(2, len(events) - 1):
        exact_keys(
            events[index],
            common_item_keys,
            "item_completed" if index == len(events) - 2 else "tool_event",
        )
    exact_keys(
        events[-1],
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
    tool_calls = []
    tool_results = []
    call_ids = set()
    for index in range(2, len(events) - 2, 2):
        call_id, tool_name, arguments = _validate_tool_call(
            events[index]["item"], boundary
        )
        if call_id in call_ids:
            raise QualificationError("tool_call_id_duplicate")
        call_ids.add(call_id)
        result = events[index + 1]["item"]
        exact_keys(
            result,
            {
                "type",
                "call_id",
                "tool_name",
                "receipt_root",
                "stdout_bytes",
                "stdout_sha256",
                "stderr_bytes",
                "stderr_sha256",
                "exit_code",
            },
            "tool_result",
        )
        if (
            result["type"] != "tool_result"
            or result["call_id"] != call_id
            or result["tool_name"] != tool_name
            or not SHA256.fullmatch(str(result["receipt_root"]))
            or not SHA256.fullmatch(str(result["stdout_sha256"]))
            or not SHA256.fullmatch(str(result["stderr_sha256"]))
            or any(
                isinstance(result[key], bool)
                or not isinstance(result[key], int)
                or result[key] < 0
                for key in ("stdout_bytes", "stderr_bytes")
            )
            or result["stdout_bytes"] + result["stderr_bytes"]
            > boundary["max_output_bytes"]
            or isinstance(result["exit_code"], bool)
            or not isinstance(result["exit_code"], int)
        ):
            raise QualificationError("tool_result_invalid")
        tool_calls.append(
            {"call_id": call_id, "tool_name": tool_name, "arguments": arguments}
        )
        tool_results.append(result)
    message = events[-2]["item"]
    exact_keys(message, {"type", "text"}, "provider_message")
    if message["type"] != "agent_message" or not isinstance(message["text"], str):
        raise QualificationError("provider_message_invalid")
    identities = {(event["run_id"], event["thread_id"]) for event in events}
    if len(identities) != 1:
        raise QualificationError("provider_event_identity_drift")
    if any(event.get("turn_id") != events[1]["turn_id"] for event in events[2:]):
        raise QualificationError("provider_turn_identity_drift")
    if any(
        event.get("response_id") != events[2]["response_id"] for event in events[3:]
    ):
        raise QualificationError("provider_response_identity_drift")
    timestamps = [parse_timestamp(event["at"], "provider_event") for event in events]
    if timestamps != sorted(timestamps):
        raise QualificationError("provider_event_time_not_monotone")
    usage = events[-1]["usage"]
    exact_keys(
        usage,
        {"input_tokens", "cached_input_tokens", "output_tokens", "tool_call_count"},
        "provider_usage",
    )
    if any(
        isinstance(value, bool) or not isinstance(value, int) or value < 0
        for value in usage.values()
    ):
        raise QualificationError("provider_usage_invalid")
    if (
        usage["tool_call_count"] != tool_count
        or usage["cached_input_tokens"] > usage["input_tokens"]
        or usage["output_tokens"] > output_token_ceiling
    ):
        raise QualificationError("provider_usage_contract_invalid")
    return {
        "events": events,
        "messages": [message],
        "usage": usage,
        "timestamps": timestamps,
        "tool_call": tool_calls[0],
        "tool_result": tool_results[0],
        "tool_calls": tool_calls,
        "tool_results": tool_results,
        "normalized_tool_semantics_root": canonical_root(
            [
                {
                    "tool_name": call["tool_name"],
                    "arguments": call["arguments"],
                    "stdout_bytes": result["stdout_bytes"],
                    "stdout_sha256": result["stdout_sha256"],
                    "stderr_bytes": result["stderr_bytes"],
                    "stderr_sha256": result["stderr_sha256"],
                    "exit_code": result["exit_code"],
                }
                for call, result in zip(tool_calls, tool_results, strict=True)
            ]
        ),
    }


def validate_raw_provider_events(
    raw: bytes, normalized_raw: bytes, adapter: str
) -> str:
    raw_events = _parse_event_lines(raw, "raw_provider_event")
    normalized_lines = [line + b"\n" for line in normalized_raw.splitlines() if line]
    normalized_types = [
        parse_json(line, "normalized_provider_event")["type"]
        for line in normalized_raw.splitlines()
        if line
    ]
    tool_count = normalized_types.count("tool.call")
    expected_types = {
        "openai-responses-v1": [
            "response.created",
            "response.in_progress",
            *sum(
                (
                    ["response.function_call_arguments.done", "runner.tool_result"]
                    for _ in range(tool_count)
                ),
                [],
            ),
            "response.output_text.done",
            "response.completed",
        ],
        "anthropic-messages-v1": [
            "message_start",
            "message_delta.start",
            *sum(
                (
                    ["content_block_stop.tool_use", "runner.tool_result"]
                    for _ in range(tool_count)
                ),
                [],
            ),
            "content_block_stop.text",
            "message_stop",
        ],
    }.get(adapter)
    if expected_types is None or len(raw_events) != len(normalized_lines):
        raise QualificationError("raw_provider_event_count_invalid")
    for index, (event, normalized_line, event_type) in enumerate(
        zip(raw_events, normalized_lines, expected_types, strict=True)
    ):
        exact_keys(
            event,
            {
                "schema",
                "provider_adapter",
                "sequence",
                "provider_event_type",
                "provider_payload",
                "normalized_event_bytes",
            },
            "raw_provider_event",
        )
        if (
            event["schema"] != RAW_PROVIDER_EVENT_SCHEMA
            or event["provider_adapter"] != adapter
            or event["sequence"] != index
            or isinstance(event["sequence"], bool)
            or event["provider_event_type"] != event_type
            or not isinstance(event["provider_payload"], dict)
            or event["normalized_event_bytes"] != digest(normalized_line)
        ):
            raise QualificationError("raw_provider_event_binding_invalid")
    return canonical_root(raw_events)


def validate_tool_receipts(
    root: Path,
    value: Any,
    event_summary: dict[str, Any],
    boundary: dict[str, Any],
) -> str:
    calls = event_summary["tool_calls"]
    results = event_summary["tool_results"]
    if not isinstance(value, list) or len(value) != len(calls):
        raise QualificationError("tool_receipt_count_invalid")
    output_paths = set()
    prior_completed = None
    for receipt, call, result in zip(value, calls, results, strict=True):
        exact_keys(
            receipt,
            {
                "schema",
                "call_id",
                "tool_name",
                "arguments",
                "arguments_root",
                "stdout",
                "stdout_bytes",
                "stdout_sha256",
                "stderr",
                "stderr_bytes",
                "stderr_sha256",
                "exit_code",
                "network_disabled",
                "writes_disabled",
                "started_at",
                "completed_at",
                "timeout_seconds",
            },
            "tool_receipt",
        )
        stdout_path = safe_relative(root, receipt["stdout"], "tool_stdout")
        stderr_path = safe_relative(root, receipt["stderr"], "tool_stderr")
        if (
            stdout_path == stderr_path
            or stdout_path in output_paths
            or stderr_path in output_paths
        ):
            raise QualificationError("tool_receipt_output_paths_not_unique")
        output_paths |= {stdout_path, stderr_path}
        stdout = read_regular(stdout_path, "tool_stdout")
        stderr = read_regular(stderr_path, "tool_stderr")
        started = parse_timestamp(receipt["started_at"], "tool_started")
        completed = parse_timestamp(receipt["completed_at"], "tool_completed")
        if (
            receipt["schema"] != TOOL_RECEIPT_SCHEMA
            or receipt["call_id"] != call["call_id"]
            or receipt["tool_name"] != call["tool_name"]
            or receipt["arguments"] != call["arguments"]
            or receipt["arguments_root"] != canonical_root(call["arguments"])
            or receipt["stdout_bytes"] != len(stdout)
            or receipt["stdout_sha256"] != digest(stdout)
            or receipt["stderr_bytes"] != len(stderr)
            or receipt["stderr_sha256"] != digest(stderr)
            or receipt["exit_code"] != result["exit_code"]
            or receipt["stdout_bytes"] != result["stdout_bytes"]
            or receipt["stdout_sha256"] != result["stdout_sha256"]
            or receipt["stderr_bytes"] != result["stderr_bytes"]
            or receipt["stderr_sha256"] != result["stderr_sha256"]
            or canonical_root(receipt) != result["receipt_root"]
            or receipt["network_disabled"] is not True
            or receipt["writes_disabled"] is not True
            or receipt["timeout_seconds"] != boundary["per_call_timeout_seconds"]
            or completed < started
            or (prior_completed is not None and started < prior_completed)
        ):
            raise QualificationError("tool_receipt_binding_invalid")
        prior_completed = completed
    return canonical_root(value)


def validate_provider_equivalence(value: Any) -> str:
    exact_keys(value, {"schema", "providers"}, "provider_equivalence")
    providers = value["providers"]
    if (
        value["schema"] != PROVIDER_EQUIVALENCE_SCHEMA
        or not isinstance(providers, list)
        or len(providers) != 2
    ):
        raise QualificationError("provider_equivalence_invalid")
    adapters = set()
    organizations = set()
    comparable = []
    for provider in providers:
        exact_keys(
            provider,
            {
                "provider_adapter",
                "provider_organization",
                "tool_boundary_root",
                "tool_semantics_root",
                "participant_visible_atoms_root",
                "registered_schema_bytes",
                "provider_schema_bytes",
                "raw_provider_events_bytes",
                "normalized_events_bytes",
                "normalized_tool_semantics_root",
                "tool_receipts_root",
            },
            "provider_equivalence_entry",
        )
        adapter = provider["provider_adapter"]
        expected = PROVIDER_ADAPTERS.get(adapter)
        if expected is None or provider["provider_organization"] != expected[0]:
            raise QualificationError("provider_equivalence_adapter_invalid")
        roots = {
            key: item
            for key, item in provider.items()
            if key not in {"provider_adapter", "provider_organization"}
        }
        if any(
            not isinstance(item, str) or not SHA256.fullmatch(item)
            for item in roots.values()
        ):
            raise QualificationError("provider_equivalence_root_invalid")
        adapters.add(adapter)
        organizations.add(provider["provider_organization"])
        comparable.append(
            {
                key: provider[key]
                for key in (
                    "tool_semantics_root",
                    "participant_visible_atoms_root",
                    "registered_schema_bytes",
                    "normalized_tool_semantics_root",
                )
            }
        )
    if (
        adapters != set(PROVIDER_ADAPTERS)
        or len(organizations) != 2
        or comparable[0] != comparable[1]
    ):
        raise QualificationError("provider_participant_visible_equivalence_invalid")
    return canonical_root(value)


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
    base_fields = {
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
    }
    tool_mode = isinstance(value, dict) and value.get("tools") in {
        "read_only_offline_shell_files",
        "read_only_offline_files",
    }
    exact_keys(
        value,
        base_fields | ({"provider_adapter", "tool_boundary"} if tool_mode else set()),
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
        or value["tools"]
        not in {
            "none",
            "no_tools",
            "read_only_offline_shell_files",
            "read_only_offline_files",
        }
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
    boundary = None
    if tool_mode:
        boundary_value = load_json(
            safe_relative(root, value["tool_boundary"], "tool_boundary"),
            "tool_boundary",
        )
        boundary = validate_tool_boundary(boundary_value)
        if boundary["adapter"] != value["provider_adapter"]:
            raise QualificationError("configuration_provider_adapter_drift")
    elif (
        value["tools"] == "no_tools"
        and any(
            re.search(r"(?:^|[=])none$", item) for item in value["strict_arguments"]
        )
        is False
    ):
        raise QualificationError("configuration_no_tools_not_strict")
    receipt = load_json(
        safe_relative(
            root, value["compatibility_receipt"], "configuration_compatibility_receipt"
        ),
        "configuration_compatibility_receipt",
    )
    compatibility_fields = {
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
    }
    version_two = tool_mode and boundary["version_two"]
    if version_two:
        compatibility_fields |= {
            "provider_adapter",
            "tool_boundary_root",
            "tool_policy_root",
            "workspace_content_root",
        }
    elif tool_mode:
        compatibility_fields |= {"provider_adapter", "tool_boundary_root"}
    exact_keys(
        receipt,
        compatibility_fields,
        "configuration_compatibility_receipt",
    )
    configuration_root = (
        canonical_root(
            {
                "configuration": value,
                ("tool_policy_root" if version_two else "tool_boundary_root"): boundary[
                    "tool_policy_root" if version_two else "tool_boundary_root"
                ],
            }
        )
        if tool_mode
        else canonical_root(value)
    )
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
        or (
            tool_mode
            and (
                receipt["provider_adapter"] != boundary["adapter"]
                or receipt["tool_boundary_root"] != boundary["tool_boundary_root"]
                or (
                    version_two
                    and (
                        receipt["tool_policy_root"] != boundary["tool_policy_root"]
                        or receipt["workspace_content_root"]
                        != boundary["workspace_content_root"]
                    )
                )
            )
        )
    ):
        raise QualificationError("configuration_compatibility_invalid")
    return {
        "configuration_root": configuration_root,
        "output_token_ceiling": value["output_token_ceiling"],
        "image_digest": receipt["image_digest"],
        "runner_version": value["runner_version"],
        "timeout_seconds": value["timeout_seconds"],
        "tool_mode": (
            "no_tools" if value["tools"] in {"none", "no_tools"} else value["tools"]
        ),
        "tool_boundary": boundary,
        "provider_adapter": boundary["adapter"] if boundary else None,
    }


def _validate_participant_hold(
    root: Path,
    value: Any,
    runtime: dict[str, Any],
    configuration: dict[str, Any],
    schemas: dict[str, Any],
) -> dict[str, Any]:
    participant_fields = {"hold", "permit", "consumed_permit", "identity"}
    version_two = (
        configuration["tool_boundary"] is not None
        and configuration["tool_boundary"]["version_two"]
    )
    if version_two:
        participant_fields.add("workspace_preflight")
    exact_keys(value, participant_fields, "participant_permit")
    identity = value["identity"]
    identity_fields = {
        "registration_id",
        "assignment_id",
        "participant_id",
        "run_id",
        "condition",
        "prompt_root",
        "packet_root",
    }
    if version_two:
        identity_fields |= {
            "tool_boundary_root",
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
        }
    exact_keys(
        identity,
        identity_fields,
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
    if version_two:
        preflight_path = safe_relative(
            root, value["workspace_preflight"], "participant_workspace_preflight"
        )
        preflight_raw = read_regular(preflight_path, "participant_workspace_preflight")
        preflight = parse_json(preflight_raw, "participant_workspace_preflight")
        exact_keys(
            preflight,
            {
                "schema",
                "status",
                "bridge_receipt",
                "tool_boundary_root",
                "tool_policy_root",
                "workspace_content_root",
                "evidence_manifest_root",
            },
            "participant_workspace_preflight",
        )
        bridge_receipt = preflight["bridge_receipt"]
        exact_keys(
            bridge_receipt,
            {
                "schema",
                "status",
                "workspace_manifest_sha256",
                "evidence_manifest_root",
                "evidence_tree_root",
                "reachable_file_count",
                "operations",
                "network_contact",
                "writes",
            },
            "participant_workspace_bridge_receipt",
        )
        if (
            digest(preflight_raw) != identity["workspace_preflight_root"]
            or preflight["schema"]
            != "vela.anthropic-offline-workspace-bound-preflight.v1"
            or preflight["status"] != "pass"
            or preflight["tool_boundary_root"]
            != configuration["tool_boundary"]["tool_boundary_root"]
            or preflight["tool_policy_root"]
            != configuration["tool_boundary"]["tool_policy_root"]
            or preflight["workspace_content_root"]
            != configuration["tool_boundary"]["workspace_content_root"]
            or preflight["evidence_manifest_root"] != identity["evidence_manifest_root"]
            or bridge_receipt["schema"]
            != "vela.anthropic-offline-workspace-bridge-preflight.v1"
            or bridge_receipt["status"] != "pass"
            or bridge_receipt["evidence_manifest_root"]
            != identity["evidence_manifest_root"]
            or isinstance(bridge_receipt["reachable_file_count"], bool)
            or not isinstance(bridge_receipt["reachable_file_count"], int)
            or bridge_receipt["reachable_file_count"] < 1
            or bridge_receipt["operations"] != ["read", "list", "stat", "search"]
            or bridge_receipt["network_contact"] is not False
            or bridge_receipt["writes"] is not False
            or not SHA256.fullmatch(bridge_receipt["workspace_manifest_sha256"])
            or not SHA256.fullmatch(bridge_receipt["evidence_tree_root"])
        ):
            raise QualificationError("participant_workspace_preflight_invalid")
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
    tool_mode = configuration["tool_mode"] in {
        "read_only_offline_shell_files",
        "read_only_offline_files",
    }
    version_two = tool_mode and configuration["tool_boundary"]["version_two"]
    fixture_fields = {
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
    }
    if tool_mode:
        fixture_fields |= {"raw_provider_events", "tool_receipts"}
    exact_keys(
        value,
        fixture_fields,
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
    identity_fields = {
        "registration_id",
        "assignment_id",
        "participant_id",
        "run_id",
        "condition",
        "prompt_root",
        "packet_root",
    }
    if version_two:
        identity_fields |= {
            "tool_boundary_root",
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
        }
    exact_keys(
        identity,
        identity_fields,
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
    if tool_mode:
        event_summary = validate_tool_events(
            events_raw,
            configuration["output_token_ceiling"],
            configuration["tool_boundary"],
        )
        raw_provider_events = read_regular(
            paths["raw_provider_events"], "neutral_fixture_raw_provider_events"
        )
        raw_provider_events_root = validate_raw_provider_events(
            raw_provider_events, events_raw, configuration["provider_adapter"]
        )
        tool_receipts_raw = read_regular(
            paths["tool_receipts"], "neutral_fixture_tool_receipts"
        )
        tool_receipts = parse_json(tool_receipts_raw, "neutral_fixture_tool_receipts")
        tool_receipts_root = validate_tool_receipts(
            root, tool_receipts, event_summary, configuration["tool_boundary"]
        )
    else:
        event_summary = validate_events(
            events_raw, configuration["output_token_ceiling"]
        )
        raw_provider_events = None
        raw_provider_events_root = None
        tool_receipts_raw = None
        tool_receipts_root = None
    launch_fields = {
        "schema",
        "run_id",
        "attempt",
        "runner_version",
        "permit_bytes",
        "configuration_root",
        "runtime_source_root",
        "image_digest",
        "started_at",
    }
    if tool_mode:
        launch_fields |= {"tool_boundary_root", "provider_adapter"}
    if version_two:
        launch_fields |= {
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
        }
    exact_keys(
        launch,
        launch_fields,
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
        or (
            tool_mode
            and (
                launch["tool_boundary_root"]
                != configuration["tool_boundary"]["tool_boundary_root"]
                or launch["provider_adapter"] != configuration["provider_adapter"]
                or (
                    version_two
                    and (
                        launch["tool_policy_root"]
                        != configuration["tool_boundary"]["tool_policy_root"]
                        or launch["workspace_content_root"]
                        != configuration["tool_boundary"]["workspace_content_root"]
                        or launch["evidence_manifest_root"]
                        != identity["evidence_manifest_root"]
                        or launch["workspace_preflight_root"]
                        != identity["workspace_preflight_root"]
                    )
                )
            )
        )
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
        schemas["provider_adapter"],
        schemas["registered_bytes"],
    )
    normalized = normalize_closed_set(
        response,
        schemas["closed_set_field"],
        schemas["closed_set_key"],
        schemas["closed_set_expected"],
    )
    terminal_fields = {
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
    }
    if tool_mode:
        terminal_fields |= {
            "raw_provider_events_bytes",
            "raw_provider_events_root",
            "tool_receipts_bytes",
            "tool_receipts_root",
            "tool_boundary_root",
            "provider_adapter",
        }
    if version_two:
        terminal_fields |= {
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
        }
    exact_keys(
        receipt,
        terminal_fields,
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
    if tool_mode:
        expected.update(
            {
                "raw_provider_events_bytes": digest(raw_provider_events),
                "raw_provider_events_root": raw_provider_events_root,
                "tool_receipts_bytes": digest(tool_receipts_raw),
                "tool_receipts_root": tool_receipts_root,
                "tool_boundary_root": configuration["tool_boundary"][
                    "tool_boundary_root"
                ],
                "provider_adapter": configuration["provider_adapter"],
            }
        )
    if version_two:
        expected.update(
            {
                "tool_policy_root": configuration["tool_boundary"]["tool_policy_root"],
                "workspace_content_root": configuration["tool_boundary"][
                    "workspace_content_root"
                ],
                "evidence_manifest_root": identity["evidence_manifest_root"],
                "workspace_preflight_root": identity["workspace_preflight_root"],
            }
        )
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
    teardown_fields = {
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
    }
    if tool_mode:
        teardown_fields |= {"tool_boundary_root", "provider_adapter"}
    if version_two:
        teardown_fields |= {
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
        }
    exact_keys(
        teardown,
        teardown_fields,
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
        or (
            tool_mode
            and (
                teardown["tool_boundary_root"]
                != configuration["tool_boundary"]["tool_boundary_root"]
                or teardown["provider_adapter"] != configuration["provider_adapter"]
                or (
                    version_two
                    and (
                        teardown["tool_policy_root"]
                        != configuration["tool_boundary"]["tool_policy_root"]
                        or teardown["workspace_content_root"]
                        != configuration["tool_boundary"]["workspace_content_root"]
                        or teardown["evidence_manifest_root"]
                        != identity["evidence_manifest_root"]
                        or teardown["workspace_preflight_root"]
                        != identity["workspace_preflight_root"]
                    )
                )
            )
        )
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
        *(("raw_provider_events", "tool_receipts") if tool_mode else ()),
    ):
        raw = read_regular(paths[key], f"neutral_fixture_{key}")
        expected_entries.append(
            {
                "path": paths[key].relative_to(fixture_dir).as_posix(),
                "bytes": len(raw),
                "sha256": digest(raw),
            }
        )
    if tool_mode:
        for receipt_item in tool_receipts:
            for key in ("stdout", "stderr"):
                output_path = safe_relative(
                    root, receipt_item[key], f"neutral_fixture_tool_{key}"
                )
                if fixture_dir not in output_path.parents:
                    raise QualificationError(
                        "neutral_fixture_tool_output_outside_directory"
                    )
                output_raw = read_regular(output_path, f"neutral_fixture_tool_{key}")
                expected_entries.append(
                    {
                        "path": output_path.relative_to(fixture_dir).as_posix(),
                        "bytes": len(output_raw),
                        "sha256": digest(output_raw),
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
    result = {
        "neutral_capture_root": manifest["capture_root"],
        "raw_response_bytes": digest(response_raw),
        "canonical_response_root": canonical_root(normalized),
        "input_tokens_telemetry": event_summary["usage"]["input_tokens"],
    }
    if tool_mode:
        result.update(
            {
                "raw_provider_events_root": raw_provider_events_root,
                "tool_receipts_root": tool_receipts_root,
                "raw_provider_events_bytes": digest(raw_provider_events),
                "normalized_events_bytes": digest(events_raw),
                "normalized_tool_semantics_root": event_summary[
                    "normalized_tool_semantics_root"
                ],
                "participant_visible_atoms_root": identity["packet_root"],
            }
        )
    return result


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
    qualification_fields = {
        "schema",
        "status",
        "configuration",
        "schemas",
        "runtime",
        "participant_permit",
        "neutral_fixture",
        "scoring_snapshot",
        "self_verification",
    }
    if isinstance(config, dict) and "provider_equivalence" in config:
        qualification_fields.add("provider_equivalence")
    exact_keys(
        config,
        qualification_fields,
        "qualification",
    )
    if config["schema"] != SCHEMA or config["status"] != "hold":
        raise QualificationError("qualification_not_held")
    schema_config = config["schemas"]
    modern_schema = isinstance(schema_config, dict) and "deletions" in schema_config
    exact_keys(
        schema_config,
        {
            "registered",
            "provider",
            "valid_response",
            "closed_set",
            *(
                {"deletions", "provider_adapter"}
                if modern_schema
                else {"deleted_pointers"}
            ),
        },
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
    registered_schema_sha256 = digest(registered_raw)
    declared_tools = (
        config["configuration"].get("tools")
        if isinstance(config["configuration"], dict)
        else None
    )
    declared_tool_mode = (
        "no_tools" if declared_tools in {"none", "no_tools"} else declared_tools
    )
    if not modern_schema and (
        declared_tool_mode != "no_tools"
        or registered_schema_sha256 != NEUTRAL_REGISTERED_SCHEMA_SHA256
    ):
        raise QualificationError("provider_legacy_schema_binding_invalid")
    if declared_tool_mode in {
        "read_only_offline_shell_files",
        "read_only_offline_files",
    } and (
        not modern_schema
        or registered_schema_sha256 != STAGE_A_REGISTERED_SCHEMA_SHA256
    ):
        raise QualificationError("provider_tool_schema_binding_invalid")
    validate_schema_boundary(
        registered,
        provider,
        schema_config["deletions" if modern_schema else "deleted_pointers"],
        valid_response,
        schema_config["provider_adapter"] if modern_schema else None,
        registered_schema_sha256 if modern_schema else None,
    )
    canonical_valid = normalize_closed_set(
        valid_response, closed["field"], closed["key"], closed["expected"]
    )
    schemas = {
        "registered": registered,
        "provider": provider,
        "deleted_pointers": schema_config[
            "deletions" if modern_schema else "deleted_pointers"
        ],
        "provider_adapter": schema_config["provider_adapter"]
        if modern_schema
        else None,
        "registered_bytes": registered_schema_sha256,
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
    if (
        modern_schema
        and schemas["provider_adapter"] != configuration["provider_adapter"]
    ):
        raise QualificationError("provider_schema_adapter_drift")
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
    provider_equivalence_root = None
    provider_equivalence = None
    if "provider_equivalence" in config:
        provider_equivalence = load_json(
            safe_relative(root, config["provider_equivalence"], "provider_equivalence"),
            "provider_equivalence",
        )
        provider_equivalence_root = validate_provider_equivalence(provider_equivalence)
    if (
        configuration["tool_mode"]
        in {"read_only_offline_shell_files", "read_only_offline_files"}
        and provider_equivalence_root is None
    ) or (
        configuration["tool_mode"] == "no_tools"
        and provider_equivalence_root is not None
    ):
        raise QualificationError("provider_equivalence_mode_invalid")
    if provider_equivalence is not None:
        current = [
            item
            for item in provider_equivalence["providers"]
            if item["provider_adapter"] == configuration["provider_adapter"]
        ]
        expected_current = {
            "provider_adapter": configuration["provider_adapter"],
            "provider_organization": configuration["tool_boundary"][
                "provider_organization"
            ],
            "tool_boundary_root": configuration["tool_boundary"]["tool_boundary_root"],
            "tool_semantics_root": configuration["tool_boundary"][
                "tool_semantics_root"
            ],
            "participant_visible_atoms_root": fixture["participant_visible_atoms_root"],
            "registered_schema_bytes": schemas["registered_bytes"],
            "provider_schema_bytes": schemas["provider_bytes"],
            "raw_provider_events_bytes": fixture["raw_provider_events_bytes"],
            "normalized_events_bytes": fixture["normalized_events_bytes"],
            "normalized_tool_semantics_root": fixture["normalized_tool_semantics_root"],
            "tool_receipts_root": fixture["tool_receipts_root"],
        }
        if current != [expected_current]:
            raise QualificationError("provider_equivalence_capture_binding_invalid")
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
    if configuration["tool_mode"] in {
        "read_only_offline_shell_files",
        "read_only_offline_files",
    }:
        gates.update(
            {
                "read_only_offline_tool_boundary": True,
                "raw_provider_event_normalization": True,
                "provider_equivalence": provider_equivalence_root is not None,
            }
        )
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
    if configuration["tool_mode"] in {
        "read_only_offline_shell_files",
        "read_only_offline_files",
    }:
        receipt.update(
            {
                "tool_boundary_root": configuration["tool_boundary"][
                    "tool_boundary_root"
                ],
                "tool_semantics_root": configuration["tool_boundary"][
                    "tool_semantics_root"
                ],
                "tool_policy_root": configuration["tool_boundary"]["tool_policy_root"],
                "workspace_content_root": configuration["tool_boundary"][
                    "workspace_content_root"
                ],
                "provider_adapter": configuration["provider_adapter"],
                "provider_equivalence_root": provider_equivalence_root,
            }
        )
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
