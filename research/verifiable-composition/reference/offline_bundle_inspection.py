#!/usr/bin/env python3
"""Offline Git-bundle and event-history inspection for ADR 0004.

This removable reader verifies and fetches one already-local bundle into a
disposable bare repository.  It derives commit trees and lineage with Git
plumbing, then derives snapshot and Vela-style event-log roots from one
committed inspection-state document at each selected commit.  No relation,
tree, snapshot root, or event-log root is accepted from the caller.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any


INSPECTION_STATE_SCHEMA = "vela.verifiable-composition.bundle-state.v0"
INSPECTION_RESULT_SCHEMA = "vela.verifiable-composition.delivery-inspection.v0"
INSPECTION_ENVELOPE_SCHEMA = (
    "vela.verifiable-composition.delivery-inspection-envelope.v0"
)
MAX_BUNDLE_BYTES = 128 * 1024 * 1024
MAX_STATE_BYTES = 4 * 1024 * 1024
MAX_EVENTS = 4096
MAX_SAFE_INTEGER = 2**53 - 1
GIT_OID = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
EVENT_ID = re.compile(r"^vev_[0-9a-f]{16}$")
SIGNATURE = re.compile(r"^(?:v1:)?[0-9a-f]{128}$")
EVENT_CONTENT_FIELDS = (
    "schema",
    "kind",
    "target",
    "actor",
    "timestamp",
    "reason",
    "before_hash",
    "after_hash",
    "payload",
    "caveats",
)


class InspectionError(ValueError):
    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(code)
        self.code = code
        self.detail = detail


def _duplicate_guard(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise InspectionError("duplicate:object_name", key)
        value[key] = item
    return value


def _reject_constant(token: str) -> None:
    raise InspectionError("invalid:nonfinite_number", token)


def validate_safe_json(value: Any, path: str = "$") -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_SAFE_INTEGER:
            raise InspectionError("invalid:unsafe_integer", path)
        return
    if isinstance(value, float):
        raise InspectionError("invalid:float", path)
    if isinstance(value, list):
        for index, item in enumerate(value):
            validate_safe_json(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise InspectionError("invalid:object_key", path)
            validate_safe_json(item, f"{path}.{key}")
        return
    raise InspectionError("invalid:json_type", path)


def canonical_bytes(value: Any) -> bytes:
    validate_safe_json(value)
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError, RecursionError) as error:
        raise InspectionError("invalid:canonical_json") from error


def sha256_root(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def strict_json(raw: bytes, *, label: str, limit: int) -> dict[str, Any]:
    if len(raw) > limit:
        raise InspectionError(f"oversized:{label}")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise InspectionError(f"invalid:{label}_utf8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_duplicate_guard,
            parse_constant=_reject_constant,
        )
    except InspectionError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise InspectionError(f"invalid:{label}_json") from error
    if not isinstance(value, dict):
        raise InspectionError(f"invalid:{label}_document")
    validate_safe_json(value)
    return value


def event_content_root(event: dict[str, Any]) -> str:
    if not isinstance(event, dict):
        raise InspectionError("invalid:event")
    required = set(EVENT_CONTENT_FIELDS) | {"id", "signature"}
    if set(event) != required:
        raise InspectionError("invalid:event_fields")
    identifier = event["id"]
    if not isinstance(identifier, str) or not EVENT_ID.fullmatch(identifier):
        raise InspectionError("invalid:event_id")
    signature = event["signature"]
    if not isinstance(signature, str) or not SIGNATURE.fullmatch(signature):
        raise InspectionError("invalid:event_signature")
    preimage = {field: event[field] for field in EVENT_CONTENT_FIELDS}
    root = sha256_root(canonical_bytes(preimage))
    if identifier != f"vev_{root[7:23]}":
        raise InspectionError("mismatch:event_id")
    return root


def event_log_root(events: list[dict[str, Any]]) -> str:
    stripped: list[dict[str, Any]] = []
    seen: set[str] = set()
    for event in events:
        event_content_root(event)
        if event["id"] in seen:
            raise InspectionError("duplicate:event_id")
        seen.add(event["id"])
        item = copy.deepcopy(event)
        item.pop("signature")
        stripped.append(item)
    stripped.sort(key=lambda item: item["id"])
    return sha256_root(canonical_bytes(stripped))


def state_from_document(raw: bytes) -> dict[str, Any]:
    document = strict_json(raw, label="inspection_state", limit=MAX_STATE_BYTES)
    if set(document) != {"schema", "snapshot", "events"}:
        raise InspectionError("invalid:inspection_state_fields")
    if document["schema"] != INSPECTION_STATE_SCHEMA:
        raise InspectionError("invalid:inspection_state_schema")
    snapshot = document["snapshot"]
    events = document["events"]
    if not isinstance(snapshot, dict):
        raise InspectionError("invalid:snapshot")
    if not isinstance(events, list) or len(events) > MAX_EVENTS:
        raise InspectionError("invalid:events")
    roots = [event_content_root(event) for event in events]
    if len(roots) != len(set(roots)):
        raise InspectionError("duplicate:event_content_root")
    return {
        "snapshot": copy.deepcopy(snapshot),
        "snapshot_root": sha256_root(canonical_bytes(snapshot)),
        "events": copy.deepcopy(events),
        "event_content_roots": roots,
        "event_log_root": event_log_root(events),
        "state_document_root": sha256_root(canonical_bytes(document)),
    }


def relation_from_commits(
    last_seen: str,
    delivered: str,
    merge_base: str,
) -> str:
    if last_seen == delivered:
        if merge_base != last_seen:
            raise InspectionError("mismatch:merge_base_same")
        return "same"
    if merge_base == last_seen:
        return "descendant"
    if merge_base == delivered:
        return "ancestor"
    return "forked"


def relation_from_events(last_seen: list[str], delivered: list[str]) -> str:
    if last_seen == delivered:
        return "same"
    if delivered[: len(last_seen)] == last_seen:
        return "descendant"
    if last_seen[: len(delivered)] == delivered:
        return "ancestor"
    return "forked"


def build_inspection_envelope(result: dict[str, Any]) -> dict[str, Any]:
    validate_inspection_result(result)
    return {
        "schema": INSPECTION_ENVELOPE_SCHEMA,
        "inspection_root": sha256_root(canonical_bytes(result)),
        "result": copy.deepcopy(result),
    }


def validate_inspection_result(value: Any) -> None:
    fields = {
        "schema",
        "verification",
        "bundle_root",
        "state_path",
        "last_seen_git_commit",
        "last_seen_git_tree",
        "delivered_git_commit",
        "delivered_git_tree",
        "merge_base",
        "git_relation",
        "event_relation",
        "last_seen_snapshot",
        "delivered_snapshot",
        "last_seen_events",
        "delivered_events",
        "last_seen_state_document_root",
        "delivered_state_document_root",
    }
    if not isinstance(value, dict) or set(value) != fields:
        raise InspectionError("invalid:inspection_result_fields")
    if value["schema"] != INSPECTION_RESULT_SCHEMA:
        raise InspectionError("invalid:inspection_result_schema")
    if value["verification"] != "verified":
        raise InspectionError("invalid:inspection_verification")
    _sha_root(value["bundle_root"], "bundle_root")
    _state_path(value["state_path"])
    for field in (
        "last_seen_git_commit",
        "last_seen_git_tree",
        "delivered_git_commit",
        "delivered_git_tree",
        "merge_base",
    ):
        _git_oid(value[field], field)
    if value["git_relation"] not in {"same", "descendant", "ancestor", "forked"}:
        raise InspectionError("invalid:git_relation")
    if value["event_relation"] not in {
        "same",
        "descendant",
        "ancestor",
        "forked",
    }:
        raise InspectionError("invalid:event_relation")
    for field in ("last_seen_snapshot", "delivered_snapshot"):
        if not isinstance(value[field], dict):
            raise InspectionError(f"invalid:{field}")
    for field in ("last_seen_events", "delivered_events"):
        events = value[field]
        if not isinstance(events, list) or len(events) > MAX_EVENTS:
            raise InspectionError(f"invalid:{field}")
        event_log_root(events)
    for field in (
        "last_seen_state_document_root",
        "delivered_state_document_root",
    ):
        _sha_root(value[field], field)
    derived_git = relation_from_commits(
        value["last_seen_git_commit"],
        value["delivered_git_commit"],
        value["merge_base"],
    )
    if value["git_relation"] != derived_git:
        raise InspectionError("mismatch:git_relation")
    last_roots = [event_content_root(event) for event in value["last_seen_events"]]
    delivered_roots = [event_content_root(event) for event in value["delivered_events"]]
    derived_events = relation_from_events(last_roots, delivered_roots)
    if value["event_relation"] != derived_events:
        raise InspectionError("mismatch:event_relation")
    compatible = {
        "same": {"same"},
        "descendant": {"same", "descendant"},
        "ancestor": {"same", "ancestor"},
        "forked": {"same", "descendant", "ancestor", "forked"},
    }
    if derived_events not in compatible[derived_git]:
        raise InspectionError("mismatch:git_event_continuity")


def inspect_bundle(
    bundle: Path,
    *,
    last_seen_commit: str,
    delivered_commit: str,
    state_path: str,
) -> dict[str, Any]:
    _git_oid(last_seen_commit, "last_seen_commit")
    _git_oid(delivered_commit, "delivered_commit")
    _state_path(state_path)
    bundle_raw = _read_regular(bundle, MAX_BUNDLE_BYTES)
    bundle_root = sha256_root(bundle_raw)
    with tempfile.TemporaryDirectory(prefix="vela-adr4-bundle-") as raw_temp:
        repository = Path(raw_temp) / "objects.git"
        _run(["git", "init", "--bare", "-q", str(repository)])
        _run(["git", "-C", str(repository), "bundle", "verify", str(bundle)])
        _run(
            [
                "git",
                "-C",
                str(repository),
                "fetch",
                "--quiet",
                "--no-tags",
                str(bundle),
                f"{last_seen_commit}:refs/inspection/last-seen",
            ]
        )
        if delivered_commit != last_seen_commit:
            _run(
                [
                    "git",
                    "-C",
                    str(repository),
                    "fetch",
                    "--quiet",
                    "--no-tags",
                    str(bundle),
                    f"{delivered_commit}:refs/inspection/delivered",
                ]
            )
        last_commit = _run_text(
            [
                "git",
                "-C",
                str(repository),
                "rev-parse",
                f"{last_seen_commit}^{{commit}}",
            ]
        )
        delivered = _run_text(
            [
                "git",
                "-C",
                str(repository),
                "rev-parse",
                f"{delivered_commit}^{{commit}}",
            ]
        )
        last_tree = _run_text(
            ["git", "-C", str(repository), "rev-parse", f"{last_commit}^{{tree}}"]
        )
        delivered_tree = _run_text(
            ["git", "-C", str(repository), "rev-parse", f"{delivered}^{{tree}}"]
        )
        merge_base = _run_text(
            [
                "git",
                "-C",
                str(repository),
                "merge-base",
                last_commit,
                delivered,
            ]
        )
        last_state = state_from_document(
            _run(
                [
                    "git",
                    "-C",
                    str(repository),
                    "show",
                    f"{last_commit}:{state_path}",
                ],
                stdout_limit=MAX_STATE_BYTES,
            ).stdout
        )
        delivered_state = state_from_document(
            _run(
                [
                    "git",
                    "-C",
                    str(repository),
                    "show",
                    f"{delivered}:{state_path}",
                ],
                stdout_limit=MAX_STATE_BYTES,
            ).stdout
        )
    result = {
        "schema": INSPECTION_RESULT_SCHEMA,
        "verification": "verified",
        "bundle_root": bundle_root,
        "state_path": state_path,
        "last_seen_git_commit": last_commit,
        "last_seen_git_tree": last_tree,
        "delivered_git_commit": delivered,
        "delivered_git_tree": delivered_tree,
        "merge_base": merge_base,
        "git_relation": relation_from_commits(last_commit, delivered, merge_base),
        "event_relation": relation_from_events(
            last_state["event_content_roots"],
            delivered_state["event_content_roots"],
        ),
        "last_seen_snapshot": last_state["snapshot"],
        "delivered_snapshot": delivered_state["snapshot"],
        "last_seen_events": last_state["events"],
        "delivered_events": delivered_state["events"],
        "last_seen_state_document_root": last_state["state_document_root"],
        "delivered_state_document_root": delivered_state["state_document_root"],
    }
    return build_inspection_envelope(result)


def _run(
    command: list[str],
    *,
    stdout_limit: int = 4 * 1024 * 1024,
) -> subprocess.CompletedProcess[bytes]:
    environment = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": "/nonexistent",
        "LANG": "C",
        "LC_ALL": "C",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
    }
    try:
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            timeout=30,
            env=environment,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise InspectionError("git:execution_failed", type(error).__name__) from error
    if len(result.stdout) > stdout_limit or len(result.stderr) > 1024 * 1024:
        raise InspectionError("git:output_oversized")
    if result.returncode != 0:
        raise InspectionError("git:command_failed", command[-1])
    return result


def _run_text(command: list[str]) -> str:
    raw = _run(command, stdout_limit=64 * 1024).stdout
    try:
        value = raw.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise InspectionError("git:output_invalid") from error
    _git_oid(value, "git_output")
    return value


def _read_regular(path: Path, limit: int) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise InspectionError("input:file_unavailable") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise InspectionError("input:file_not_regular")
    if metadata.st_size > limit:
        raise InspectionError("input:file_oversized")
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise InspectionError("input:file_unreadable") from error
    if len(raw) != metadata.st_size:
        raise InspectionError("input:file_changed_during_read")
    return raw


def _git_oid(value: Any, label: str) -> None:
    if not isinstance(value, str) or not GIT_OID.fullmatch(value):
        raise InspectionError(f"invalid:{label}")


def _sha_root(value: Any, label: str) -> None:
    if not isinstance(value, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", value):
        raise InspectionError(f"invalid:{label}")


def _state_path(value: Any) -> None:
    if not isinstance(value, str) or len(value.encode("utf-8")) > 1024:
        raise InspectionError("invalid:state_path")
    path = PurePosixPath(value)
    if (
        not value
        or value.startswith("/")
        or "\\" in value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise InspectionError("invalid:state_path")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Derive one content-addressed delivery inspection from an offline Git bundle"
    )
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--last-seen", required=True)
    parser.add_argument("--delivered", required=True)
    parser.add_argument("--state-path", required=True)
    return parser


def main() -> int:
    arguments = _parser().parse_args()
    try:
        result = inspect_bundle(
            arguments.bundle,
            last_seen_commit=arguments.last_seen,
            delivered_commit=arguments.delivered,
            state_path=arguments.state_path,
        )
    except InspectionError as error:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "unresolvable",
                    "code": f"unresolvable:{error.code}",
                    "detail": error.detail,
                },
                separators=(",", ":"),
                sort_keys=True,
            )
        )
        return 1
    print(canonical_bytes(result).decode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
