"""Exact-checkout encoder and fail-closed resolver for ADR 0004 Phase 1.

This is removable experiment code.  It reads one already-present Git commit,
derives producer-provenance roots from current Vela objects, and never signs or
mutates frontier state.  A structural match is deliberately not an authority
verdict: current public Vela porcelain cannot verify one arbitrary historical
review decision and return its scoped authority result.
"""

from __future__ import annotations

import copy
import datetime as dt
import hashlib
import json
import os
import re
import selectors
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "clients/python"))
sys.path.insert(0, str(REPO_ROOT / "crates/vela-cli/resources"))

from receipt_json import (  # noqa: E402
    canonical_receipt_json_bytes,
    strict_receipt_json_load_bytes,
)
from vela_receipt_v1 import validate_receipt  # noqa: E402
from vela_verify_log import canonical_bytes as vela_canonical_bytes  # noqa: E402

try:
    from .dependency_observation import (
        ATTACHMENT_ID,
        AUTHORITY,
        EVENT_ID,
        FINDING_ID,
        FRONTIER_ID,
        MAX_AUTHORITY_BYTES,
        MAX_LIST_ITEMS,
        ROLES,
        SHA256,
        SIGNATURE,
        validate_observation,
    )
except ImportError:  # Direct execution by composition.py.
    from dependency_observation import (  # type: ignore[no-redef]
        ATTACHMENT_ID,
        AUTHORITY,
        EVENT_ID,
        FINDING_ID,
        FRONTIER_ID,
        MAX_AUTHORITY_BYTES,
        MAX_LIST_ITEMS,
        ROLES,
        SHA256,
        SIGNATURE,
        validate_observation,
    )


SELECTION_SCHEMA = "vela.experimental-dependency-selection.v0"
RESOLUTION_SCHEMA = "vela.experimental-dependency-resolution.v0"
CANONICAL_CUSTODY_SCHEMA = "vela.experimental-canonical-custody.v0"
SELECTION_FIELDS = {
    "schema",
    "frontier_path",
    "finding_id",
    "decision_event_id",
    "verifier_attachment_ids",
    "premise_path",
    "role",
}
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
FULL_GIT_OID = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
MAX_JSON_BYTES = 32 * 1024 * 1024
MAX_BLOB_BYTES = 64 * 1024 * 1024
MAX_TREE_BYTES = 256 * 1024 * 1024
MAX_TREE_FILES = 20_000


class CompositionError(ValueError):
    """A stable, typed, fail-closed experiment result."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(code)
        self.code = code
        self.detail = detail


def _run_bounded(
    command: list[str],
    *,
    environment: dict[str, str],
    cwd: str | Path | None = None,
    stdout_limit: int,
    stderr_limit: int,
    timeout: int,
    label: str,
) -> subprocess.CompletedProcess[bytes]:
    """Drain both process pipes concurrently and kill before either overflows."""

    try:
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
            cwd=cwd,
        )
    except OSError as error:
        raise CompositionError(
            f"{label}:execution_failed", type(error).__name__
        ) from error
    if process.stdout is None or process.stderr is None:
        process.kill()
        process.wait()
        raise CompositionError(f"{label}:pipe_failed")
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    limits = {"stdout": stdout_limit, "stderr": stderr_limit}
    deadline = time.monotonic() + timeout
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise CompositionError(f"{label}:execution_timeout")
            ready = selector.select(min(remaining, 0.25))
            for key, _ in ready:
                chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                buffer = buffers[key.data]
                buffer.extend(chunk)
                if len(buffer) > limits[key.data]:
                    raise CompositionError(f"{label}:output_oversized", key.data)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise CompositionError(f"{label}:execution_timeout")
        returncode = process.wait(timeout=remaining)
    except CompositionError:
        process.kill()
        process.wait()
        raise
    except subprocess.TimeoutExpired as error:
        process.kill()
        process.wait()
        raise CompositionError(f"{label}:execution_timeout") from error
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()
    return subprocess.CompletedProcess(
        command,
        returncode,
        bytes(buffers["stdout"]),
        bytes(buffers["stderr"]),
    )


def _duplicate_guard(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise CompositionError("invalid:duplicate_object_name")
        value[key] = item
    return value


def strict_json(raw: bytes, *, label: str, limit: int = MAX_JSON_BYTES) -> Any:
    if len(raw) > limit:
        raise CompositionError(f"oversized:{label}")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CompositionError(f"invalid:{label}_utf8") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_duplicate_guard,
            parse_constant=lambda token: (_ for _ in ()).throw(
                CompositionError(f"invalid:{label}_number", token)
            ),
        )
    except CompositionError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise CompositionError(f"invalid:{label}_json") from error


def canonical_bytes(value: Any) -> bytes:
    try:
        return vela_canonical_bytes(value)
    except (TypeError, ValueError, RecursionError) as error:
        raise CompositionError("invalid:canonical_json") from error


def sha256_bytes(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def sha256_json(value: Any) -> str:
    return sha256_bytes(canonical_bytes(value))


def _path(value: Any, *, field: str, allow_root: bool = False) -> str:
    if not isinstance(value, str):
        raise CompositionError(f"invalid:{field}")
    if value == "." and allow_root:
        return value
    if (
        not value
        or value.startswith("/")
        or value.endswith("/")
        or "\\" in value
        or len(value.encode("utf-8")) > 1024
        or any(ord(character) < 32 for character in value)
    ):
        raise CompositionError(f"invalid:{field}")
    parts = PurePosixPath(value).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        raise CompositionError(f"invalid:{field}")
    return "/".join(parts)


def _join(prefix: str, relative: str) -> str:
    return relative if prefix == "." else f"{prefix}/{relative}"


@dataclass(frozen=True)
class GitEntry:
    mode: str
    kind: str
    oid: str
    path: str


class ExactGitCheckout:
    """Read raw regular-file bytes from one exact, already-local Git commit."""

    def __init__(self, repo: str | Path, commit: str) -> None:
        self.repo = Path(repo).resolve()
        if not self.repo.is_dir():
            raise CompositionError("git:repository_missing")
        if not isinstance(commit, str) or not FULL_GIT_OID.fullmatch(commit):
            raise CompositionError("invalid:parent_git_commit")
        self.object_format = self._git_text(
            ["rev-parse", "--show-object-format"]
        ).strip()
        expected_length = {"sha1": 40, "sha256": 64}.get(self.object_format)
        if expected_length is None:
            raise CompositionError("git:unsupported_object_format")
        if len(commit) != expected_length:
            raise CompositionError("invalid:git_object_format")
        self.commit = commit
        if self._git_text(["cat-file", "-t", commit]).strip() != "commit":
            raise CompositionError("git:not_commit")
        commit_body = self._git_bytes(["cat-file", "commit", commit], MAX_BLOB_BYTES)
        tree_line = next(
            (line for line in commit_body.splitlines() if line.startswith(b"tree ")),
            None,
        )
        if tree_line is None:
            raise CompositionError("git:commit_without_tree")
        try:
            tree = tree_line[5:].decode("ascii")
        except UnicodeDecodeError as error:
            raise CompositionError("git:invalid_tree_oid") from error
        if not FULL_GIT_OID.fullmatch(tree) or len(tree) != expected_length:
            raise CompositionError("git:invalid_tree_oid")
        if self._git_text(["cat-file", "-t", tree]).strip() != "tree":
            raise CompositionError("git:not_tree")
        self.tree = tree
        self._entries: dict[str, GitEntry] | None = None

    @staticmethod
    def _git_environment() -> dict[str, str]:
        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": os.environ.get("HOME", "/nonexistent"),
            "LANG": "C",
            "LC_ALL": "C",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0",
        }
        return environment

    def _git_bytes(self, args: list[str], limit: int) -> bytes:
        command = [
            "git",
            "-c",
            "protocol.allow=never",
            "--literal-pathspecs",
            "-C",
            str(self.repo),
            *args,
        ]
        result = _run_bounded(
            command,
            environment=self._git_environment(),
            stdout_limit=limit,
            stderr_limit=64 * 1024,
            timeout=30,
            label="git",
        )
        if result.returncode != 0:
            detail = result.stderr[:512].decode("utf-8", "replace").strip()
            raise CompositionError("git:object_unavailable", detail)
        return result.stdout

    def _git_text(self, args: list[str]) -> str:
        raw = self._git_bytes(args, 64 * 1024)
        try:
            return raw.decode("utf-8")
        except UnicodeDecodeError as error:
            raise CompositionError("git:invalid_text_output") from error

    def entries(self) -> dict[str, GitEntry]:
        if self._entries is not None:
            return self._entries
        raw = self._git_bytes(
            ["ls-tree", "-r", "-z", "--full-tree", self.tree],
            MAX_TREE_BYTES,
        )
        entries: dict[str, GitEntry] = {}
        for record in raw.split(b"\0"):
            if not record:
                continue
            try:
                header, raw_path = record.split(b"\t", 1)
                mode_raw, kind_raw, oid_raw = header.split(b" ", 2)
                mode = mode_raw.decode("ascii")
                kind = kind_raw.decode("ascii")
                oid = oid_raw.decode("ascii")
                path = raw_path.decode("utf-8")
            except (ValueError, UnicodeDecodeError) as error:
                raise CompositionError("git:invalid_tree_entry") from error
            _path(path, field="git_tree_path")
            if path in entries:
                raise CompositionError("git:duplicate_tree_path")
            if not FULL_GIT_OID.fullmatch(oid) or len(oid) != len(self.commit):
                raise CompositionError("git:invalid_object_oid")
            entries[path] = GitEntry(mode=mode, kind=kind, oid=oid, path=path)
            if len(entries) > MAX_TREE_FILES:
                raise CompositionError("git:too_many_tree_entries")
        self._entries = entries
        return entries

    @staticmethod
    def _require_regular(entry: GitEntry) -> None:
        if entry.kind != "blob" or entry.mode not in {"100644", "100755"}:
            raise CompositionError(
                "git:non_regular_entry",
                f"{entry.path}:{entry.mode}:{entry.kind}",
            )

    def read_bytes(self, path: str, *, limit: int = MAX_BLOB_BYTES) -> bytes:
        normalized = _path(path, field="checkout_path")
        entry = self.entries().get(normalized)
        if entry is None:
            raise CompositionError("git:path_missing", normalized)
        self._require_regular(entry)
        return self._git_bytes(["cat-file", "blob", entry.oid], limit)

    def read_json(self, path: str, *, label: str) -> dict[str, Any]:
        value = strict_json(self.read_bytes(path, limit=MAX_JSON_BYTES), label=label)
        if not isinstance(value, dict):
            raise CompositionError(f"invalid:{label}_document")
        return value

    def subtree_oid(self, path: str) -> str:
        normalized = _path(path, field="frontier_path", allow_root=True)
        if normalized == ".":
            return self.tree
        output = self._git_text(["rev-parse", f"{self.commit}:{normalized}"]).strip()
        if not FULL_GIT_OID.fullmatch(output) or len(output) != len(self.commit):
            raise CompositionError("git:invalid_subtree_oid")
        if self._git_text(["cat-file", "-t", output]).strip() != "tree":
            raise CompositionError("git:frontier_path_not_tree")
        return output

    def materialize_subtree(
        self,
        path: str,
        destination: str | Path,
        *,
        max_files: int = MAX_TREE_FILES,
        max_tree_bytes: int = MAX_TREE_BYTES,
        max_blob_bytes: int = MAX_BLOB_BYTES,
    ) -> dict[str, Any]:
        """Materialize one exact regular-file subtree without invoking checkout."""

        normalized = _path(path, field="frontier_path", allow_root=True)
        target = Path(destination)
        if target.exists():
            if target.is_symlink() or not target.is_dir() or any(target.iterdir()):
                raise CompositionError("checkout:destination_not_empty")
        else:
            target.mkdir(mode=0o700, parents=True)
        if target.is_symlink():
            raise CompositionError("checkout:destination_symlink")
        prefix = "" if normalized == "." else f"{normalized}/"
        selected = [
            entry
            for entry in self.entries().values()
            if normalized == "." or entry.path.startswith(prefix)
        ]
        if not selected:
            raise CompositionError("checkout:frontier_empty")
        total = 0
        written = 0
        for entry in sorted(selected, key=lambda item: item.path):
            self._require_regular(entry)
            relative = entry.path if normalized == "." else entry.path[len(prefix) :]
            relative = _path(relative, field="materialized_path")
            raw = self.read_bytes(entry.path, limit=max_blob_bytes)
            total += len(raw)
            written += 1
            if total > max_tree_bytes:
                raise CompositionError("checkout:tree_oversized")
            if written > max_files:
                raise CompositionError("checkout:too_many_files")
            output = target.joinpath(*PurePosixPath(relative).parts)
            output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            cursor = target
            for component in PurePosixPath(relative).parts[:-1]:
                cursor = cursor / component
                if cursor.is_symlink() or not cursor.is_dir():
                    raise CompositionError("checkout:path_escape")
            flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            try:
                descriptor = os.open(output, flags, 0o700 if entry.mode == "100755" else 0o600)
                with os.fdopen(descriptor, "wb") as stream:
                    stream.write(raw)
            except OSError as error:
                raise CompositionError(
                    "checkout:materialization_failed", type(error).__name__
                ) from error
        required = (
            ".vela/config.toml",
            "frontier.json",
            "vela.lock",
            "proof/latest.json",
        )
        if any(not (target / item).is_file() for item in required):
            raise CompositionError("checkout:canonical_frontier_incomplete")
        return {
            "git_commit": self.commit,
            "git_tree": self.tree,
            "frontier_git_tree": self.subtree_oid(normalized),
            "files": written,
            "bytes": total,
        }


def _offline_environment() -> dict[str, str]:
    return {
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
        "VELA_ADVICE": "0",
        "VELA_OFFLINE": "1",
        "NO_PROXY": "*",
        "no_proxy": "*",
    }


def _regular_executable_identity(executable: str | Path) -> tuple[Path, dict[str, Any]]:
    path = Path(executable)
    if path.is_symlink() or not path.is_file():
        raise CompositionError("runner:executable_not_regular")
    resolved = path.resolve()
    size = resolved.stat().st_size
    if size <= 0 or size > 256 * 1024 * 1024:
        raise CompositionError("runner:executable_oversized")
    digest = hashlib.sha256()
    with resolved.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    version = _run_bounded(
        [str(resolved), "--version"],
        environment=_offline_environment(),
        stdout_limit=64 * 1024,
        stderr_limit=64 * 1024,
        timeout=10,
        label="vela_version",
    )
    if version.returncode != 0:
        raise CompositionError("runner:version_failed")
    try:
        version_text = version.stdout.decode("utf-8").strip()
    except UnicodeDecodeError as error:
        raise CompositionError("runner:version_invalid") from error
    if version_text != "vela 0.800.13":
        raise CompositionError("runner:release_mismatch", version_text)
    return resolved, {
        "path": str(resolved),
        "bytes": size,
        "sha256": f"sha256:{digest.hexdigest()}",
        "version": version_text,
    }


def _run_vela_json(
    executable: Path,
    args: list[str],
    *,
    cwd: Path,
    label: str,
) -> tuple[subprocess.CompletedProcess[bytes], dict[str, Any] | None]:
    result = _run_bounded(
        [str(executable), *args],
        environment=_offline_environment(),
        cwd=cwd,
        stdout_limit=32 * 1024 * 1024,
        stderr_limit=256 * 1024,
        timeout=30,
        label=label,
    )
    try:
        value = strict_json(result.stdout, label=f"{label}_output")
    except CompositionError:
        value = None
    return result, value if isinstance(value, dict) else None


def _root(value: Any) -> str | None:
    return value if isinstance(value, str) and SHA256.fullmatch(value) else None


def _prefixed_root(value: Any) -> str | None:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        return None
    return f"sha256:{value}"


def _nested(value: Any, *path: str) -> Any:
    current = value
    for field in path:
        if not isinstance(current, dict):
            return None
        current = current.get(field)
    return current


def _lock_root(raw: bytes, field: str) -> str | None:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return None
    match = re.search(
        rf"(?m)^{re.escape(field)}:[ \t]*(sha256:[0-9a-f]{{64}})[ \t]*$",
        text,
    )
    return match.group(1) if match else None


def verify_canonical_materialization(
    frontier: str | Path,
    vela_executable: str | Path,
) -> dict[str, Any]:
    """Run the two frozen read-only commands and require every root lane to agree."""

    path = Path(frontier)
    try:
        if path.is_symlink() or not path.is_dir():
            raise CompositionError("checkout:frontier_missing")
        executable, runner = _regular_executable_identity(vela_executable)
        check_process, check = _run_vela_json(
            executable,
            ["check", ".", "--strict", "--json"],
            cwd=path,
            label="vela_check_strict",
        )
        proof_process, proof = _run_vela_json(
            executable,
            ["proof", "verify", ".", "--json"],
            cwd=path,
            label="vela_proof_verify",
        )
        commands = {
            "check_strict": {
                "returncode": check_process.returncode,
                "json": check is not None,
            },
            "proof_verify": {
                "returncode": proof_process.returncode,
                "json": proof is not None,
            },
        }
        if check_process.returncode != 0 or check is None or check.get("ok") is not True:
            return {
                "schema": CANONICAL_CUSTODY_SCHEMA,
                "ok": False,
                "status": "rejected",
                "code": "rejected:vela_check_strict_failed",
                "runner": runner,
                "commands": commands,
            }
        if proof_process.returncode != 0 or proof is None or proof.get("ok") is not True:
            return {
                "schema": CANONICAL_CUSTODY_SCHEMA,
                "ok": False,
                "status": "rejected",
                "code": "rejected:vela_proof_verify_failed",
                "runner": runner,
                "commands": commands,
            }

        visible = strict_json(
            (path / "frontier.json").read_bytes(), label="visible_frontier"
        )
        proof_latest = strict_json(
            (path / "proof/latest.json").read_bytes(), label="proof_latest"
        )
        proof_hashes = strict_json(
            (path / "proof/hashes.json").read_bytes(), label="proof_hashes"
        )
        if not all(isinstance(item, dict) for item in (visible, proof_latest, proof_hashes)):
            raise CompositionError("invalid:canonical_views")
        lock = (path / "vela.lock").read_bytes()

        replay_event = _prefixed_root(_nested(check, "replay", "event_log_hash"))
        replay_current = _prefixed_root(_nested(check, "replay", "current_hash"))
        replay_replayed = _prefixed_root(_nested(check, "replay", "replayed_hash"))
        replay_source = _prefixed_root(_nested(check, "replay", "source_hash"))
        proof_event = _root(proof.get("event_log_hash"))
        nested_proof_event = _root(_nested(proof, "proof", "event_log_hash"))
        proof_snapshot = _root(proof.get("snapshot_hash"))
        nested_proof_snapshot = _root(_nested(proof, "proof", "frontier_hash"))
        visible_event = _root(_nested(visible, "_meta", "event_log_hash"))
        visible_snapshot = _root(_nested(visible, "_meta", "snapshot_hash"))
        latest_event = _root(proof_latest.get("event_log_hash"))
        latest_snapshot = _root(proof_latest.get("frontier_hash"))
        hashes_event = _root(proof_hashes.get("event_log_hash"))
        hashes_snapshot = _root(proof_hashes.get("snapshot_hash"))
        lock_event = _lock_root(lock, "event_log_hash")
        lock_snapshot = _lock_root(lock, "snapshot_hash")
        event_roots = [
            replay_event,
            proof_event,
            nested_proof_event,
            visible_event,
            latest_event,
            hashes_event,
            lock_event,
        ]
        snapshot_roots = [
            replay_current,
            replay_replayed,
            replay_source,
            proof_snapshot,
            nested_proof_snapshot,
            visible_snapshot,
            latest_snapshot,
            hashes_snapshot,
            lock_snapshot,
        ]
        roots = {
            "event_log": event_roots,
            "snapshot": snapshot_roots,
        }
        parity = (
            _nested(check, "replay", "ok") is True
            and proof.get("issues") == []
            and all(root is not None for root in event_roots + snapshot_roots)
            and len(set(event_roots)) == 1
            and len(set(snapshot_roots)) == 1
        )
        if not parity:
            return {
                "schema": CANONICAL_CUSTODY_SCHEMA,
                "ok": False,
                "status": "rejected",
                "code": "rejected:canonical_root_parity",
                "runner": runner,
                "commands": commands,
                "roots": roots,
            }
        return {
            "schema": CANONICAL_CUSTODY_SCHEMA,
            "ok": True,
            "status": "verified",
            "code": "canonical_custody_verified",
            "runner": runner,
            "commands": commands,
            "roots": {
                "event_log": event_roots[0],
                "snapshot": snapshot_roots[0],
            },
        }
    except (CompositionError, OSError) as error:
        code = error.code if isinstance(error, CompositionError) else "checkout:io_failed"
        return {
            "schema": CANONICAL_CUSTODY_SCHEMA,
            "ok": False,
            "status": "rejected",
            "code": code,
            "detail": error.detail if isinstance(error, CompositionError) else type(error).__name__,
        }


def inspect_canonical_checkout(
    repo: str | Path,
    commit: str,
    frontier_path: str,
    destination: str | Path,
    vela_executable: str | Path,
    *,
    expected_frontier_tree: str,
) -> dict[str, Any]:
    try:
        checkout = ExactGitCheckout(repo, commit)
        identity = checkout.materialize_subtree(frontier_path, destination)
        if identity["frontier_git_tree"] != expected_frontier_tree:
            raise CompositionError("mismatch:frontier_git_tree")
        result = verify_canonical_materialization(destination, vela_executable)
        result["git"] = identity
        return result
    except CompositionError as error:
        return {
            "schema": CANONICAL_CUSTODY_SCHEMA,
            "ok": False,
            "status": "rejected",
            "code": error.code,
            "detail": error.detail,
        }


DECISION_PLAN_DOMAIN = b"vela.decision-plan.internal.v1\0"
REVIEWER_AUTHORITY_DOMAIN = b"vela.reviewer-authority.internal.v1\0"
DECISION_ROOT_PREFIX = "urn:vela:decision-root:"
DECISION_PREIMAGE_VERSION = "vela.decision-plan.internal.v1"
DECISION_PREIMAGE_FIELDS = {
    "decision_preimage_version",
    "frontier_id",
    "expected_event_log_root",
    "ordered_answers",
    "consumed_fact_roots",
    "policy_input_root",
    "semantic_event_cores",
}
DECISION_ANSWER_FIELDS = {"proposal_id", "proposal_root", "action", "reason"}
CONSUMED_ROOT_FIELDS = {
    "proposal_id",
    "proposal_root",
    "receipt_observation_root",
    "receipt_root",
    "evidence_or_reference_root",
    "evidence_availability",
    "verifier_snapshot_root",
    "policy_input_root",
    "policy_result_root",
    "engine_gate_root",
    "reviewer_authority_root",
    "semantic_effect_root",
    "downstream_impact_root",
}
EVENT_CORE_FIELDS = {"answer_ordinal", "event_ordinal", "event"}
DECISION_SIGNING_FIELDS = (
    "schema",
    "id",
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


def _decision_result(code: str, **context: Any) -> dict[str, Any]:
    status = code.split(":", 1)[0]
    return {
        "schema": "vela.named-decision-inspection.v0.1",
        "ok": status == "verified",
        "status": status,
        "code": code,
        **context,
    }


def _parse_time(value: Any) -> dt.datetime | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed if parsed.tzinfo is not None else None


def _proposal_id(proposal: dict[str, Any]) -> str:
    fields = (
        "schema",
        "kind",
        "target",
        "actor",
        "reason",
        "payload",
        "source_refs",
        "caveats",
    )
    if any(field not in proposal for field in fields):
        return ""
    return f"vpr_{sha256_json({field: proposal[field] for field in fields})[7:23]}"


def _normalize_event_core(event: dict[str, Any]) -> dict[str, Any]:
    normalized = copy.deepcopy(event)
    normalized["id"] = ""
    normalized["signature"] = None
    payload = normalized.get("payload")
    provenance = payload.get("provenance") if isinstance(payload, dict) else None
    if isinstance(provenance, dict):
        refs = provenance.get("input_refs")
        if isinstance(refs, list):
            retained = [
                reference
                for reference in refs
                if not (
                    isinstance(reference, str)
                    and reference.startswith(DECISION_ROOT_PREFIX)
                )
            ]
            if retained:
                provenance["input_refs"] = retained
            else:
                provenance.pop("input_refs", None)
        if not provenance:
            payload.pop("provenance", None)
    return normalized


def _verify_python_event_signature(
    event: dict[str, Any], public_key_hex: str
) -> bool:
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import (
            Ed25519PublicKey,
        )

        signature = event.get("signature")
        if not isinstance(signature, str):
            return False
        version_one = signature.startswith("v1:")
        signature_hex = signature[3:] if version_one else signature
        signature_bytes = bytes.fromhex(signature_hex)
        public_key = Ed25519PublicKey.from_public_bytes(bytes.fromhex(public_key_hex))
        body = canonical_bytes(
            {field: event[field] for field in DECISION_SIGNING_FIELDS}
        )
        if version_one:
            payload_type = b"application/vnd.vela.event+json"
            body = (
                b"DSSEv1 "
                + str(len(payload_type)).encode()
                + b" "
                + payload_type
                + b" "
                + str(len(body)).encode()
                + b" "
                + body
            )
        public_key.verify(signature_bytes, body)
        return True
    except (InvalidSignature, KeyError, TypeError, ValueError):
        return False


def _historical_actor(
    actor: dict[str, Any], decision_time: dt.datetime
) -> dict[str, Any]:
    historical = copy.deepcopy(actor)
    revoked = _parse_time(actor.get("revoked_at"))
    if revoked is not None and revoked > decision_time:
        historical.pop("revoked_at", None)
        historical.pop("revoked_reason", None)
    return historical


def _python_authority_root(
    frontier_id: str, actor: dict[str, Any], decided_at: str
) -> str:
    commitment = {
        "schema": "vela.reviewer-authority.internal.v1",
        "frontier_id": frontier_id,
        "reviewer": actor,
        "decided_at": decided_at,
        "authorization": "authorized",
    }
    return sha256_bytes(REVIEWER_AUTHORITY_DOMAIN + canonical_bytes(commitment))


def inspect_named_decision_python(
    project: dict[str, Any],
    decision_event_id: str,
    decision_event_content_root: str,
    decision_preimage: bytes | None,
) -> dict[str, Any]:
    """Independent Python classification parity for the pure Rust inspector."""

    events = project.get("events")
    if (
        not isinstance(events, list)
        or not isinstance(decision_event_id, str)
        or not EVENT_ID.fullmatch(decision_event_id)
    ):
        return _decision_result("rejected:decision_event_not_unique")
    matches = [
        (index, event)
        for index, event in enumerate(events)
        if isinstance(event, dict) and event.get("id") == decision_event_id
    ]
    if len(matches) != 1:
        return _decision_result("rejected:decision_event_not_unique")
    decision_index, event = matches[0]
    if derived_event_id(event) != event.get("id"):
        return _decision_result("rejected:decision_event_id_mismatch")
    content_root = event_content_root(event)
    if content_root != decision_event_content_root:
        return _decision_result("rejected:decision_content_root_mismatch")
    actor_ref = event.get("actor")
    if (
        event.get("kind") != "review.accepted"
        or not isinstance(actor_ref, dict)
        or actor_ref.get("type") != "human"
        or not isinstance(actor_ref.get("id"), str)
        or actor_ref["id"].startswith(("agent:", "ci:"))
    ):
        return _decision_result("rejected:decision_actor_invalid")
    actor_id = actor_ref["id"]
    actors = project.get("actors")
    actor_matches = (
        [actor for actor in actors if isinstance(actor, dict) and actor.get("id") == actor_id]
        if isinstance(actors, list)
        else []
    )
    decision_time = _parse_time(event.get("timestamp"))
    if len(actor_matches) != 1 or decision_time is None:
        return _decision_result("rejected:reviewer_unauthorized")
    actor = actor_matches[0]
    created = _parse_time(actor.get("created_at"))
    revoked = _parse_time(actor.get("revoked_at")) if "revoked_at" in actor else None
    authority_namespace = actor_id.startswith("reviewer:") or actor_id.startswith("steward:")
    key = actor.get("public_key")
    if (
        not authority_namespace
        or actor.get("algorithm", "ed25519") != "ed25519"
        or not isinstance(key, str)
        or not re.fullmatch(r"[0-9a-fA-F]{64}", key)
        or created is None
        or created > decision_time
        or ("revoked_at" in actor and revoked is None)
        or (revoked is not None and decision_time >= revoked)
    ):
        return _decision_result("rejected:reviewer_unauthorized")
    if not _verify_python_event_signature(event, key):
        return _decision_result("rejected:decision_signature_invalid")

    payload = event.get("payload")
    proposal_id = payload.get("proposal_id") if isinstance(payload, dict) else None
    proposal_kind = payload.get("proposal_kind") if isinstance(payload, dict) else None
    applied_event_id = payload.get("applied_event_id") if isinstance(payload, dict) else None
    if (
        not isinstance(payload, dict)
        or payload.get("verdict") != "accepted"
        or not all(
            isinstance(value, str) and value
            for value in (proposal_id, proposal_kind, applied_event_id)
        )
        or event.get("target") != {"type": "proposal", "id": proposal_id}
    ):
        return _decision_result("rejected:proposal_link_mismatch")
    proposals = project.get("proposals")
    proposal_matches = (
        [
            proposal
            for proposal in proposals
            if isinstance(proposal, dict) and proposal.get("id") == proposal_id
        ]
        if isinstance(proposals, list)
        else []
    )
    if len(proposal_matches) != 1:
        return _decision_result("rejected:proposal_link_mismatch")
    proposal = proposal_matches[0]
    if (
        _proposal_id(proposal) != proposal_id
        or proposal.get("kind") != proposal_kind
        or proposal.get("status") != "applied"
        or proposal.get("reviewed_by") != actor_id
        or proposal.get("reviewed_at") != event.get("timestamp")
        or proposal.get("decision_reason") != event.get("reason")
    ):
        return _decision_result("rejected:proposal_link_mismatch")
    if proposal.get("applied_event_id") != applied_event_id:
        return _decision_result("rejected:applied_event_link_mismatch")
    applied_matches = [
        (index, candidate)
        for index, candidate in enumerate(events)
        if isinstance(candidate, dict) and candidate.get("id") == applied_event_id
    ]
    if len(applied_matches) != 1:
        return _decision_result("rejected:applied_event_link_mismatch")
    applied_index, applied = applied_matches[0]
    if (
        derived_event_id(applied) != applied_event_id
        or not isinstance(applied.get("payload"), dict)
        or applied["payload"].get("proposal_id") != proposal_id
        or applied.get("actor") != actor_ref
        or applied.get("timestamp") != event.get("timestamp")
        or (applied_event_id != decision_event_id and applied.get("target") != proposal.get("target"))
    ):
        return _decision_result("rejected:applied_event_link_mismatch")
    provenance = payload.get("provenance")
    refs = provenance.get("input_refs") if isinstance(provenance, dict) else []
    if not isinstance(refs, list) or any(not isinstance(ref, str) for ref in refs):
        return _decision_result("rejected:decision_root_not_unique")
    decision_roots = [ref[len(DECISION_ROOT_PREFIX) :] for ref in refs if ref.startswith(DECISION_ROOT_PREFIX)]
    if len(decision_roots) != 1 or not SHA256.fullmatch(decision_roots[0]):
        return _decision_result("rejected:decision_root_not_unique")
    decision_root = decision_roots[0]
    context = {
        "decision_event_id": decision_event_id,
        "decision_event_content_root": content_root,
        "decision_root": decision_root,
        "proposal_id": proposal_id,
        "applied_event_id": applied_event_id,
        "authority_id": actor_id,
    }
    if decision_preimage is None:
        return _decision_result(
            "unresolvable:decision_preimage_unavailable", **context
        )
    if len(decision_preimage) > 1024 * 1024:
        return _decision_result("rejected:decision_preimage_oversized", **context)
    try:
        preimage = strict_json(decision_preimage, label="decision_preimage")
    except CompositionError:
        return _decision_result("rejected:decision_preimage_invalid", **context)
    if not isinstance(preimage, dict) or set(preimage) != DECISION_PREIMAGE_FIELDS:
        return _decision_result("rejected:decision_preimage_invalid", **context)
    if preimage.get("decision_preimage_version") != DECISION_PREIMAGE_VERSION:
        return _decision_result("rejected:decision_preimage_version", **context)
    canonical = canonical_bytes(preimage)
    if canonical != decision_preimage:
        return _decision_result("rejected:decision_preimage_noncanonical", **context)
    answers = preimage.get("ordered_answers")
    roots = preimage.get("consumed_fact_roots")
    cores = preimage.get("semantic_event_cores")
    if (
        not isinstance(answers, list)
        or len(answers) != 1
        or not isinstance(roots, list)
        or len(roots) != 1
        or not isinstance(cores, list)
        or not cores
        or len(cores) > 512
        or not isinstance(answers[0], dict)
        or set(answers[0]) != DECISION_ANSWER_FIELDS
        or not isinstance(roots[0], dict)
        or set(roots[0]) != CONSUMED_ROOT_FIELDS
        or any(not isinstance(core, dict) or set(core) != EVENT_CORE_FIELDS for core in cores)
    ):
        return _decision_result("rejected:decision_preimage_scope", **context)
    answer = answers[0]
    consumed = roots[0]
    root_fields = [
        preimage.get("expected_event_log_root"),
        preimage.get("policy_input_root"),
        answer.get("proposal_root"),
        *[
            value
            for key, value in consumed.items()
            if key.endswith("_root") and value is not None
        ],
    ]
    if any(not isinstance(value, str) or not SHA256.fullmatch(value) for value in root_fields):
        return _decision_result("rejected:decision_preimage_invalid", **context)
    derived_root = sha256_bytes(DECISION_PLAN_DOMAIN + canonical)
    if derived_root != decision_root:
        return _decision_result("rejected:decision_preimage_root_mismatch", **context)
    if preimage.get("frontier_id") != project.get("frontier_id"):
        return _decision_result("rejected:decision_frontier_mismatch", **context)
    if (
        answer.get("action") != "accept"
        or answer.get("proposal_id") != proposal_id
        or consumed.get("proposal_id") != proposal_id
        or answer.get("proposal_root") != consumed.get("proposal_root")
        or answer.get("reason") != proposal.get("decision_reason")
    ):
        return _decision_result("rejected:decision_answer_mismatch", **context)
    pending = copy.deepcopy(proposal)
    pending["status"] = "pending_review"
    for field in ("reviewed_by", "reviewed_at", "decision_reason", "applied_event_id"):
        pending.pop(field, None)
    proposal_root = sha256_json(pending)
    if answer.get("proposal_root") != proposal_root:
        return _decision_result("rejected:decision_proposal_root_mismatch", **context)
    historical_actor = _historical_actor(actor, decision_time)
    authority_root = _python_authority_root(
        project["frontier_id"], historical_actor, event["timestamp"]
    )
    if consumed.get("reviewer_authority_root") != authority_root:
        return _decision_result("rejected:decision_authority_root_mismatch", **context)

    matched: set[int] = set()
    for ordinal, core in enumerate(cores):
        if core.get("answer_ordinal") != 0 or core.get("event_ordinal") != ordinal:
            return _decision_result("rejected:decision_event_core_mismatch", **context)
        candidates = [
            index
            for index, candidate in enumerate(events)
            if index not in matched and _normalize_event_core(candidate) == core.get("event")
        ]
        if len(candidates) != 1:
            return _decision_result("rejected:decision_event_core_mismatch", **context)
        matched.add(candidates[0])
    if decision_index not in matched or applied_index not in matched:
        return _decision_result("rejected:decision_event_core_mismatch", **context)
    if any(
        events[index].get("actor") != actor_ref
        or events[index].get("timestamp") != event.get("timestamp")
        for index in matched
    ):
        return _decision_result("rejected:decision_event_core_mismatch", **context)
    if any(not _verify_python_event_signature(events[index], key) for index in matched):
        return _decision_result(
            "rejected:decision_event_signature_invalid", **context
        )
    historical = []
    for index, candidate in enumerate(events):
        if index in matched:
            continue
        timestamp = _parse_time(candidate.get("timestamp"))
        if timestamp is None:
            return _decision_result("rejected:decision_timestamp_invalid", **context)
        if timestamp < decision_time:
            historical.append(candidate)
        elif timestamp == decision_time:
            return _decision_result(
                "rejected:decision_historical_head_ambiguous", **context
            )
    historical_root = event_log_root(historical)
    if preimage.get("expected_event_log_root") != historical_root:
        return _decision_result("rejected:decision_event_log_root_mismatch", **context)
    return _decision_result(
        "verified:decision_evidence_bound",
        expected_event_log_root=historical_root,
        **context,
    )


def parse_selection(raw: bytes) -> dict[str, Any]:
    value = strict_json(raw, label="selection", limit=1024 * 1024)
    if not isinstance(value, dict):
        raise CompositionError("invalid:selection_document")
    missing = sorted(SELECTION_FIELDS - set(value))
    if missing:
        raise CompositionError(f"missing:{missing[0]}")
    extra = sorted(set(value) - SELECTION_FIELDS)
    if extra:
        raise CompositionError(f"unexpected:{extra[0]}")
    if value["schema"] != SELECTION_SCHEMA:
        raise CompositionError("invalid:schema")
    value["frontier_path"] = _path(
        value["frontier_path"], field="frontier_path", allow_root=True
    )
    value["premise_path"] = _path(value["premise_path"], field="premise_path")
    if not isinstance(value["finding_id"], str) or not FINDING_ID.fullmatch(
        value["finding_id"]
    ):
        raise CompositionError("invalid:finding_id")
    if not isinstance(value["decision_event_id"], str) or not EVENT_ID.fullmatch(
        value["decision_event_id"]
    ):
        raise CompositionError("invalid:decision_event_id")
    attachments = value["verifier_attachment_ids"]
    if not isinstance(attachments, list) or not attachments:
        raise CompositionError("invalid:verifier_attachment_ids")
    if len(attachments) > MAX_LIST_ITEMS:
        raise CompositionError("oversized:verifier_attachment_ids")
    if any(
        not isinstance(item, str) or not ATTACHMENT_ID.fullmatch(item)
        for item in attachments
    ):
        raise CompositionError("invalid:verifier_attachment_ids")
    if len(set(attachments)) != len(attachments):
        raise CompositionError("duplicate:verifier_attachment_ids")
    if value["role"] not in ROLES:
        raise CompositionError("invalid:role")
    return value


def event_content_root(event: dict[str, Any]) -> str:
    missing = [field for field in EVENT_CONTENT_FIELDS if field not in event]
    if missing:
        raise CompositionError(f"invalid:event_missing_{missing[0]}")
    return sha256_json({field: event[field] for field in EVENT_CONTENT_FIELDS})


def derived_event_id(event: dict[str, Any]) -> str:
    return f"vev_{event_content_root(event)[7:23]}"


def event_log_root(events: list[dict[str, Any]]) -> str:
    identifiers: set[str] = set()
    stripped: list[dict[str, Any]] = []
    for event in events:
        identifier = event.get("id")
        if not isinstance(identifier, str) or identifier in identifiers:
            raise CompositionError("invalid:event_ids")
        identifiers.add(identifier)
        item = copy.deepcopy(event)
        item.pop("signature", None)
        stripped.append(item)
    stripped.sort(key=lambda event: event["id"])
    return sha256_json(stripped)


def finding_revision_root(finding: dict[str, Any]) -> str:
    value = copy.deepcopy(finding)
    value["links"] = []
    return sha256_json(value)


def attachment_content_root(attachment: dict[str, Any]) -> str:
    return sha256_json(attachment)


def derived_attachment_id(attachment: dict[str, Any]) -> str:
    value = copy.deepcopy(attachment)
    value["id"] = ""
    return f"vva_{sha256_json(value)[7:23]}"


def _validate_attachment_claim_binding(
    attachment: dict[str, Any], finding: dict[str, Any]
) -> None:
    assertion = finding.get("assertion")
    claim = assertion.get("text") if isinstance(assertion, dict) else None
    if not isinstance(claim, str) or not claim.strip():
        raise CompositionError("invalid:finding_claim")
    claim_digest = hashlib.sha256(claim.strip().encode("utf-8")).hexdigest()[:16]
    match_to_claim = attachment.get("match_to_claim")
    if (
        attachment.get("claim_digest") != claim_digest
        or not isinstance(match_to_claim, dict)
        or match_to_claim.get("matches") is not True
        or attachment.get("outcome") != "passed"
        or attachment.get("method_integrity") != "sound"
        or attachment.get("undischarged_hypotheses", []) != []
    ):
        raise CompositionError("unresolvable:attachment_not_exact_claim_match")
    for field in ("verifier_method", "solver_id", "verifier_actor"):
        if not isinstance(attachment.get(field), str) or not attachment[field].strip():
            raise CompositionError("unresolvable:attachment_method_incomplete")


def snapshot_candidate_root(frontier: dict[str, Any]) -> str:
    """Reference parity only; not a normative public Vela snapshot verdict."""

    value = copy.deepcopy(frontier)
    for field in ("_meta", "_warning", "events", "signatures", "proof_state"):
        value.pop(field, None)
    return sha256_json(value)


def _object_list(frontier: dict[str, Any], field: str) -> list[dict[str, Any]]:
    value = frontier.get(field)
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        raise CompositionError(f"invalid:frontier_{field}")
    return value


def _one(items: list[dict[str, Any]], identifier: str, *, kind: str) -> dict[str, Any]:
    matches = [item for item in items if item.get("id") == identifier]
    if len(matches) != 1:
        raise CompositionError(f"invalid:{kind}_missing_or_duplicated")
    return matches[0]


def _target(value: Any, *, kind: str, identifier: str) -> bool:
    return (
        isinstance(value, dict)
        and value.get("type") == kind
        and value.get("id") == identifier
    )


def _derive_observation(
    checkout: ExactGitCheckout,
    selection: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    frontier_path = selection["frontier_path"]
    frontier = checkout.read_json(
        _join(frontier_path, "frontier.json"), label="frontier"
    )
    frontier_id = frontier.get("frontier_id")
    if not isinstance(frontier_id, str) or not FRONTIER_ID.fullmatch(frontier_id):
        raise CompositionError("invalid:parent_frontier_id")
    meta = frontier.get("_meta")
    if not isinstance(meta, dict):
        raise CompositionError("invalid:frontier_meta")
    declared_event_root = meta.get("event_log_hash")
    declared_snapshot_root = meta.get("snapshot_hash")
    if not isinstance(declared_event_root, str) or not SHA256.fullmatch(
        declared_event_root
    ):
        raise CompositionError("invalid:parent_event_log_root")
    if not isinstance(declared_snapshot_root, str) or not SHA256.fullmatch(
        declared_snapshot_root
    ):
        raise CompositionError("invalid:parent_snapshot_root")

    events = _object_list(frontier, "events")
    actual_event_root = event_log_root(events)
    if actual_event_root != declared_event_root:
        raise CompositionError("mismatch:frontier_declared_event_log_root")

    finding_id = selection["finding_id"]
    finding = _one(_object_list(frontier, "findings"), finding_id, kind="finding")
    finding_root = finding_revision_root(finding)

    decision_id = selection["decision_event_id"]
    decision = _one(events, decision_id, kind="decision_event")
    if decision.get("kind") != "review.accepted":
        raise CompositionError("invalid:decision_not_review_accepted")
    if derived_event_id(decision) != decision_id:
        raise CompositionError("mismatch:decision_event_id")
    signature = decision.get("signature")
    if not isinstance(signature, str) or not SIGNATURE.fullmatch(signature):
        raise CompositionError("invalid:decision_signature")
    actor = decision.get("actor")
    if not isinstance(actor, dict) or actor.get("type") != "human":
        raise CompositionError("invalid:decision_actor")
    authority_id = actor.get("id")
    if (
        not isinstance(authority_id, str)
        or len(authority_id.encode("utf-8")) > MAX_AUTHORITY_BYTES
        or not AUTHORITY.fullmatch(authority_id)
    ):
        raise CompositionError("invalid:authority_id")
    payload = decision.get("payload")
    if not isinstance(payload, dict) or payload.get("verdict") != "accepted":
        raise CompositionError("invalid:decision_payload")
    proposal_id = payload.get("proposal_id")
    proposal_kind = payload.get("proposal_kind")
    applied_event_id = payload.get("applied_event_id")
    if not all(
        isinstance(item, str) and item
        for item in (proposal_id, proposal_kind, applied_event_id)
    ):
        raise CompositionError("invalid:decision_links")
    if not _target(decision.get("target"), kind="proposal", identifier=proposal_id):
        raise CompositionError("mismatch:decision_target")

    proposal = _one(_object_list(frontier, "proposals"), proposal_id, kind="proposal")
    if (
        proposal_kind != "finding.add"
        or proposal.get("kind") != proposal_kind
        or proposal.get("status") != "applied"
        or proposal.get("applied_event_id") != applied_event_id
        or not _target(proposal.get("target"), kind="finding", identifier=finding_id)
        or proposal.get("reviewed_by") != authority_id
    ):
        raise CompositionError("mismatch:accepted_proposal_links")

    applied = _one(events, applied_event_id, kind="applied_event")
    if derived_event_id(applied) != applied_event_id:
        raise CompositionError("mismatch:applied_event_id")
    if applied.get("kind") != "finding.asserted":
        raise CompositionError("unresolvable:decision_not_finding_assertion")
    if not _target(applied.get("target"), kind="finding", identifier=finding_id):
        raise CompositionError("mismatch:applied_event_target")
    if applied.get("after_hash") != finding_root:
        raise CompositionError("mismatch:finding_revision_root")
    applied_payload = applied.get("payload")
    if (
        not isinstance(applied_payload, dict)
        or applied_payload.get("proposal_id") != proposal_id
    ):
        raise CompositionError("mismatch:applied_event_proposal")

    proposal_payload = proposal.get("payload")
    submission = (
        proposal_payload.get("vela_submission")
        if isinstance(proposal_payload, dict)
        else None
    )
    if not isinstance(submission, dict):
        raise CompositionError("invalid:vela_submission")
    receipt_root = submission.get("receipt_root")
    receipt_path = submission.get("receipt_path")
    if not isinstance(receipt_root, str) or not SHA256.fullmatch(receipt_root):
        raise CompositionError("invalid:receipt_root")
    expected_receipt_path = f"records/receipts/sha256/{receipt_root[7:]}.json"
    if receipt_path != expected_receipt_path:
        raise CompositionError("mismatch:receipt_path")
    receipt_bytes = checkout.read_bytes(
        _join(frontier_path, expected_receipt_path), limit=8 * 1024 * 1024
    )
    try:
        receipt = strict_receipt_json_load_bytes(receipt_bytes)
    except (UnicodeDecodeError, ValueError, RecursionError) as error:
        raise CompositionError("invalid:receipt_json") from error
    if not isinstance(receipt, dict):
        raise CompositionError("invalid:receipt_document")
    try:
        receipt_errors = validate_receipt(receipt)
        actual_receipt_root = sha256_bytes(canonical_receipt_json_bytes(receipt))
    except (TypeError, ValueError, RecursionError) as error:
        raise CompositionError("invalid:receipt_v1") from error
    if receipt_errors:
        raise CompositionError("invalid:receipt_v1", str(receipt_errors[0])[:512])
    if actual_receipt_root != receipt_root:
        raise CompositionError("mismatch:receipt_root")

    premise = checkout.read_bytes(
        _join(frontier_path, selection["premise_path"]), limit=MAX_BLOB_BYTES
    )
    premise_digest = sha256_bytes(premise)
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list):
        raise CompositionError("invalid:receipt_artifacts")
    premise_artifacts = [
        artifact
        for artifact in artifacts
        if isinstance(artifact, dict)
        and artifact.get("path") == selection["premise_path"]
        and artifact.get("sha256") == premise_digest[7:]
    ]
    if len(premise_artifacts) != 1:
        raise CompositionError("unrepresentable:premise_not_exact_receipt_artifact")

    attachment_objects = _object_list(frontier, "verifier_attachments")
    attachment_roots: list[dict[str, str]] = []
    for attachment_id in sorted(selection["verifier_attachment_ids"]):
        attachment = _one(attachment_objects, attachment_id, kind="verifier_attachment")
        if attachment.get("target") != finding_id:
            raise CompositionError("mismatch:verifier_attachment_target")
        if derived_attachment_id(attachment) != attachment_id:
            raise CompositionError("mismatch:verifier_attachment_id")
        _validate_attachment_claim_binding(attachment, finding)
        attachment_roots.append(
            {
                "attachment_id": attachment_id,
                "attachment_content_root": attachment_content_root(attachment),
            }
        )

    observation: dict[str, Any] = {
        "schema": "vela.experimental-dependency-observation.v0",
        "parent_frontier_id": frontier_id,
        "parent_git_commit": checkout.commit,
        "parent_git_tree": checkout.tree,
        "parent_event_log_root": actual_event_root,
        "parent_snapshot_root": declared_snapshot_root,
        "finding_id": finding_id,
        "finding_revision_root": finding_root,
        "decision_event_id": decision_id,
        "decision_event_content_root": event_content_root(decision),
        "decision_signature": signature,
        "authority_id": authority_id,
        "receipt_roots": [receipt_root],
        "verifier_attachments": attachment_roots,
        "premise_digest": premise_digest,
        "role": selection["role"],
    }
    validate_observation(observation)
    context = {
        "snapshot_candidate_root": snapshot_candidate_root(frontier),
    }
    return observation, context


def encode_observation(
    repo: str | Path,
    commit: str,
    selection_raw: bytes,
) -> dict[str, Any]:
    selection = parse_selection(selection_raw)
    checkout = ExactGitCheckout(repo, commit)
    observation, _ = _derive_observation(checkout, selection)
    return observation


def _check(name: str, status: str, detail: str) -> dict[str, str]:
    return {"check": name, "status": status, "detail": detail}


def _result(
    status: str,
    code: str,
    checks: list[dict[str, str]],
    detail: str = "",
) -> dict[str, Any]:
    return {
        "schema": RESOLUTION_SCHEMA,
        "ok": False,
        "status": status,
        "code": code,
        "detail": detail,
        "checks": checks,
    }


def _normalized_attachments(value: list[dict[str, str]]) -> list[dict[str, str]]:
    return sorted(value, key=lambda item: item["attachment_id"])


def resolve_observation(
    repo: str | Path,
    observation: dict[str, Any],
    *,
    frontier_path: str,
    premise_path: str,
) -> dict[str, Any]:
    """Recompute representable roots, then stop at missing normative checks."""

    checks: list[dict[str, str]] = []
    try:
        validate_observation(observation)
        normalized_frontier_path = _path(
            frontier_path, field="frontier_path", allow_root=True
        )
        normalized_premise_path = _path(premise_path, field="premise_path")
        checkout = ExactGitCheckout(repo, observation["parent_git_commit"])
        checks.append(_check("git_commit", "exact_match", checkout.commit))
        if checkout.tree != observation["parent_git_tree"]:
            raise CompositionError("mismatch:parent_git_tree")
        checks.append(_check("git_tree", "exact_match", checkout.tree))
        selection = {
            "schema": SELECTION_SCHEMA,
            "frontier_path": normalized_frontier_path,
            "finding_id": observation["finding_id"],
            "decision_event_id": observation["decision_event_id"],
            "verifier_attachment_ids": [
                item["attachment_id"] for item in observation["verifier_attachments"]
            ],
            "premise_path": normalized_premise_path,
            "role": observation["role"],
        }
        derived, context = _derive_observation(checkout, selection)
        comparisons: list[tuple[str, Any, Any]] = [
            (
                "parent_frontier_id",
                observation["parent_frontier_id"],
                derived["parent_frontier_id"],
            ),
            (
                "parent_event_log_root",
                observation["parent_event_log_root"],
                derived["parent_event_log_root"],
            ),
            (
                "parent_snapshot_root",
                observation["parent_snapshot_root"],
                derived["parent_snapshot_root"],
            ),
            (
                "finding_revision_root",
                observation["finding_revision_root"],
                derived["finding_revision_root"],
            ),
            (
                "decision_event_content_root",
                observation["decision_event_content_root"],
                derived["decision_event_content_root"],
            ),
            (
                "decision_signature_bytes",
                observation["decision_signature"],
                derived["decision_signature"],
            ),
            (
                "authority_actor_id",
                observation["authority_id"],
                derived["authority_id"],
            ),
            (
                "receipt_roots",
                sorted(observation["receipt_roots"]),
                sorted(derived["receipt_roots"]),
            ),
            (
                "verifier_attachment_roots",
                _normalized_attachments(observation["verifier_attachments"]),
                _normalized_attachments(derived["verifier_attachments"]),
            ),
            (
                "premise_digest",
                observation["premise_digest"],
                derived["premise_digest"],
            ),
        ]
        for name, asserted, actual in comparisons:
            if asserted != actual:
                raise CompositionError(f"mismatch:{name}")
            qualifier = (
                "byte_match_not_cryptographically_verified"
                if name == "decision_signature_bytes"
                else "structural_match"
            )
            checks.append(_check(name, qualifier, str(actual)))
        checks.append(
            _check(
                "premise_receipt_artifact_binding",
                "structural_match",
                "premise bytes match the unique same-path artifact digest in the retained Receipt v1",
            )
        )
        checks.append(
            _check(
                "attachment_claim_binding",
                "structural_match",
                "selected objects are id-valid, claim-matched, passed, sound, and hypothesis-free",
            )
        )
        checks.append(
            _check(
                "selected_attachment_decision_binding",
                "unresolvable",
                "public decision event omits the Decision Plan consumed attachment set",
            )
        )
        checks.append(
            _check(
                "canonical_state_replay",
                "unresolvable",
                "derived_view_not_canonical_state: frontier.json is not replayed from canonical .vela state",
            )
        )
        candidate_root = context["snapshot_candidate_root"]
        candidate_status = (
            "reference_parity"
            if candidate_root == observation["parent_snapshot_root"]
            else "reference_mismatch"
        )
        checks.append(
            _check(
                "snapshot_candidate",
                candidate_status,
                str(candidate_root),
            )
        )
        if candidate_status == "reference_mismatch":
            raise CompositionError("mismatch:snapshot_candidate_root")
        checks.append(
            _check(
                "historical_decision_authority",
                "unresolvable",
                "public Vela has no read-only single-event signature plus historical scope verdict",
            )
        )
        checks.append(
            _check(
                "snapshot_normative_root",
                "unresolvable",
                "strict JSON does not expose the recomputed Project snapshot root",
            )
        )
        return _result(
            "unresolvable",
            "unresolvable:authority_snapshot_porcelain_missing",
            checks,
            (
                "All representable structural roots match. Public read-only Vela "
                "cannot prove derived-view parity with canonical state, historical "
                "decision authority, or the normative snapshot root, so no "
                "dependency verdict was produced."
            ),
        )
    except CompositionError as error:
        status = (
            "unresolvable"
            if error.code.startswith(("unresolvable:", "unrepresentable:"))
            else "rejected"
        )
        return _result(status, error.code, checks, error.detail)
    except ValueError as error:
        # ObservationError intentionally exposes only its stable code string.
        return _result("rejected", str(error), checks)
