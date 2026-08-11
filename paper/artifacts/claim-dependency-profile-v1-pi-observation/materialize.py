#!/usr/bin/env python3
"""Materialize the frozen Pi-SDK observation study without running a model."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Sequence

import rfc8785


PACKET = Path(__file__).resolve().parent
REPOSITORY = PACKET.parents[2]
V0_COMMIT = "530cb806ad9d219341cf3e5ec168e9683136a427"
V0_PATH = "paper/artifacts/claim-dependency-profile-v0-observation"
V0_TREE = "e8eb017239535aa86d56e322ea98a5d4fd00fea4"
EXPERIMENT_COMMIT = "4e3f942dfff55ca5fd00b16f5e2ff41c156c3be6"
EXPERIMENT_PATH = "conformance/experiments/claim-dependency-profile-v0"
EXPERIMENT_TREE = "02bac2c905f7bf773313dea0096818a80fee2166"
ARMS = ("disciplined-git-ro-crate", "rooted-source-plus-profile")
RUNS = (
    (
        "block-1-profile",
        "rooted-source-plus-profile",
        "1f823974-9f91-44c8-8fb5-19db463b2993",
    ),
    (
        "block-1-baseline",
        "disciplined-git-ro-crate",
        "bb98b175-f6df-440f-9352-01be011b9518",
    ),
    (
        "block-2-baseline",
        "disciplined-git-ro-crate",
        "9194d3e6-f16a-4a87-a1e2-6a6a3af16612",
    ),
    (
        "block-2-profile",
        "rooted-source-plus-profile",
        "560064c2-1177-4539-954b-7209c3fc501d",
    ),
)
COPIED_V0 = (
    "answer.schema.json",
    "answer-key.json",
    "scorer.py",
    "shared-scope.json",
    "input-manifests/disciplined-git-ro-crate.json",
    "input-manifests/rooted-source-plus-profile.json",
    "prompts/task.txt",
)
RUNTIME_FILES = (
    "Dockerfile",
    "package.json",
    "package-lock.json",
    "participant.mjs",
    "auth-preflight.mjs",
    "request-capture.mjs",
    "egress-broker.mjs",
    "run-participant.sh",
    "LICENSE.pi-v0.84.1.base64",
)
PACKET_MANIFEST = "manifest.json"
ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")


class ContractError(ValueError):
    """Stable packet/materialization refusal."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def strict_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def reject_constant(constant: str) -> None:
    raise ContractError(f"unsupported JSON constant: {constant}")


def parse_json(data: bytes, label: str) -> Any:
    try:
        value = json.loads(
            data,
            object_pairs_hook=strict_pairs,
            parse_constant=reject_constant,
        )
        rfc8785.dumps(value)
        return value
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
        ContractError,
        rfc8785.CanonicalizationError,
    ) as exc:
        raise ContractError(f"invalid JSON in {label}: {exc}") from exc


def json_bytes(value: Any) -> bytes:
    return rfc8785.dumps(value) + b"\n"


def raw_root(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def relative_path(value: str, label: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
        or "\x00" in value
    ):
        raise ContractError(f"invalid {label}: {value!r}")
    return path


def contained_path(base: Path, relative: PurePosixPath) -> Path:
    base = base.resolve()
    candidate = base.joinpath(*relative.parts)
    try:
        candidate.resolve(strict=False).relative_to(base)
    except (OSError, ValueError) as exc:
        raise ContractError(f"path escapes base: {relative}") from exc
    current = base
    for part in relative.parts[:-1]:
        current /= part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            break
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ContractError(f"non-directory parent refused: {relative}")
    return candidate


def read_regular(path: Path, maximum: int, expected_mode: int | None = None) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        before_path = path.lstat()
    except OSError as exc:
        raise ContractError(f"cannot inspect regular file: {path}") from exc
    if stat.S_ISLNK(before_path.st_mode) or not stat.S_ISREG(before_path.st_mode):
        raise ContractError(f"nonregular file refused: {path}")
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise ContractError(f"cannot open regular file: {path}") from exc
    try:
        before = os.fstat(descriptor)
        mode = stat.S_IMODE(before.st_mode)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_size > maximum
            or (expected_mode is not None and mode != expected_mode)
        ):
            raise ContractError(f"file type, mode, or size refused: {path}")
        chunks: list[bytes] = []
        remaining = before.st_size + 1
        while remaining:
            chunk = os.read(descriptor, min(65_536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        current = path.lstat()
    except OSError as exc:
        raise ContractError(f"file identity changed: {path}") from exc
    if (
        len(data) != before.st_size
        or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        or (before.st_dev, before.st_ino) != (current.st_dev, current.st_ino)
    ):
        raise ContractError(f"file changed while reading: {path}")
    return data


def git_environment() -> dict[str, str]:
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.upper().startswith("GIT_")
    }
    environment.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    return environment


def git_result(arguments: Sequence[str], *, text: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", *arguments],
        cwd=REPOSITORY,
        check=False,
        capture_output=True,
        text=text,
        env=git_environment(),
    )


def git_text(arguments: Sequence[str], label: str) -> str:
    result = git_result(arguments)
    if result.returncode != 0:
        raise ContractError(f"cannot resolve {label}: {result.stderr.strip()}")
    return result.stdout.strip()


def git_bytes(arguments: Sequence[str], label: str) -> bytes:
    result = git_result(arguments, text=False)
    if result.returncode != 0:
        raise ContractError(f"cannot resolve {label}")
    return result.stdout


def validate_frozen_sources() -> None:
    require(
        git_text(["rev-parse", f"{V0_COMMIT}:{V0_PATH}"], "v0 packet tree")
        == V0_TREE,
        "frozen v0 packet tree drifted",
    )
    require(
        git_text(
            ["rev-parse", f"{EXPERIMENT_COMMIT}:{EXPERIMENT_PATH}"],
            "experiment tree",
        )
        == EXPERIMENT_TREE,
        "frozen experiment tree drifted",
    )
    for name in COPIED_V0:
        current = read_regular(PACKET / name, 1_048_576)
        frozen = git_bytes(["show", f"{V0_COMMIT}:{V0_PATH}/{name}"], name)
        require(current == frozen, f"copied v0 contract drifted: {name}")
    manifest = git_bytes(
        ["show", f"{V0_COMMIT}:{V0_PATH}/manifest.json"], "v0 manifest"
    )
    plan = parse_json(read_regular(PACKET / "plan.json", 262_144), "plan.json")
    require(plan["frozen_v0_packet"]["manifest_raw_root"] == raw_root(manifest), "v0 manifest root binding drifted")
    frozen_manifest = parse_json(manifest, "frozen v0 manifest")
    require(
        plan["frozen_v0_packet"]["files_canonical_root"]
        == frozen_manifest["files_canonical_root"],
        "v0 files root binding drifted",
    )


def validate_package_lock(data: bytes) -> None:
    value = parse_json(data, "package-lock.json")
    require(value.get("lockfileVersion") == 3, "package lock version drifted")
    packages = value.get("packages")
    require(isinstance(packages, dict), "package lock packages are invalid")
    for name, package in packages.items():
        if not name:
            continue
        require(isinstance(package, dict), f"package lock row is invalid: {name}")
        if "resolved" in package:
            require(
                isinstance(package.get("integrity"), str)
                and package["integrity"].startswith("sha512-"),
                f"registry package lacks integrity: {name}",
            )
    pi = packages.get("node_modules/@earendil-works/pi-coding-agent")
    require(
        isinstance(pi, dict)
        and pi.get("version") == "0.84.1"
        and pi.get("integrity")
        == "sha512-ncAqFrG+iybuPGOhMiZoEHkEzTpJgz3guYD32pD+M7ucc0WeHmauP6wa7qwP8V/KWvsZDVNa5XGsdZ7fkC7w7A==",
        "direct Pi package lock binding drifted",
    )


def packet_manifest() -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    data = read_regular(PACKET / PACKET_MANIFEST, 262_144, 0o644)
    manifest = parse_json(data, PACKET_MANIFEST)
    require(
        isinstance(manifest, dict)
        and set(manifest)
        == {
            "schema",
            "source_parent_commit",
            "files",
            "files_canonical_root",
            "excludes",
            "authority_effect",
            "claim_credit",
        },
        "source manifest shape is invalid",
    )
    require(
        manifest["schema"] == "vela.claim-dependency-pi-observation-source-manifest.v1"
        and manifest["excludes"] == [PACKET_MANIFEST]
        and manifest["authority_effect"] == "none"
        and manifest["claim_credit"] is False,
        "source manifest boundary drifted",
    )
    rows: dict[str, dict[str, Any]] = {}
    for row in manifest["files"]:
        require(
            isinstance(row, dict)
            and set(row) == {"path", "mode", "bytes", "raw_root"},
            "source manifest row is invalid",
        )
        name = str(relative_path(row["path"], "source manifest path"))
        require(name not in rows and name != PACKET_MANIFEST, "source manifest path duplicated")
        rows[name] = row
    actual: set[str] = set()
    for path in PACKET.rglob("*"):
        metadata = path.lstat()
        name = path.relative_to(PACKET).as_posix()
        if stat.S_ISLNK(metadata.st_mode) or not (
            stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)
        ):
            raise ContractError(f"nonregular source packet path: {name}")
        if stat.S_ISREG(metadata.st_mode) and name != PACKET_MANIFEST:
            actual.add(name)
    require(set(rows) == actual, "source manifest path set drifted")
    for name, row in rows.items():
        require(row["mode"] in {"100644", "100755"}, f"invalid packet mode: {name}")
        data = read_regular(PACKET / name, 4_194_304, int(row["mode"][-3:], 8))
        require(len(data) == row["bytes"] and raw_root(data) == row["raw_root"], f"packet file drifted: {name}")
    require(
        manifest["files_canonical_root"] == raw_root(rfc8785.dumps(manifest["files"])),
        "source manifest files root drifted",
    )
    return rows, manifest


def packet_file(rows: dict[str, dict[str, Any]], name: str) -> bytes:
    require(name in rows, f"unregistered packet file: {name}")
    row = rows[name]
    data = read_regular(PACKET / name, 4_194_304, int(row["mode"][-3:], 8))
    require(len(data) == row["bytes"] and raw_root(data) == row["raw_root"], f"packet input drifted: {name}")
    return data


def source_file(row: dict[str, Any]) -> bytes:
    require(
        isinstance(row, dict)
        and set(row) == {"source_path", "mounted_path", "mode", "bytes", "raw_root"}
        and row["mode"] == "100644",
        "input manifest row is invalid",
    )
    source = contained_path(
        REPOSITORY, relative_path(row["source_path"], "source path")
    )
    data = read_regular(source, 1_048_576, 0o644)
    require(
        len(data) == row["bytes"] and raw_root(data) == row["raw_root"],
        f"scientific input drifted: {row['source_path']}",
    )
    relative_path(row["mounted_path"], "virtual input path")
    return data


def load_input_manifest(
    rows: dict[str, dict[str, Any]], arm: str
) -> tuple[dict[str, Any], bytes]:
    name = f"input-manifests/{arm}.json"
    data = packet_file(rows, name)
    value = parse_json(data, name)
    require(
        isinstance(value, dict)
        and value.get("schema") == "vela.claim-dependency-observation-input-manifest.v0"
        and value.get("experiment_id") == "synthetic-counterfactual-erdos-321-v0"
        and value.get("arm") == arm
        and isinstance(value.get("files"), list)
        and len(value["files"]) == (7 if arm == ARMS[0] else 8),
        f"input manifest shape drifted: {arm}",
    )
    paths = [row["mounted_path"] for row in value["files"]]
    require(len(paths) == len(set(paths)), f"duplicate virtual input path: {arm}")
    if arm == ARMS[1]:
        require(paths[-1] == "profile.json", "profile must be the sole final treatment input")
    return value, data


def render_block(kind: str, path: str, data: bytes) -> bytes:
    require(data.endswith(b"\n"), f"embedded {kind} must end in newline: {path}")
    header = (
        f"\n--- BEGIN EXACT {kind} ---\n"
        f"virtual_path: {path}\n"
        f"bytes: {len(data)}\n"
        f"raw_root: {raw_root(data)}\n"
        "content:\n"
    ).encode()
    return header + data + f"--- END EXACT {kind} ---\n".encode()


def render_user_message(
    rows: dict[str, dict[str, Any]], arm: str, manifest: dict[str, Any]
) -> tuple[bytes, list[tuple[str, bytes]]]:
    task = packet_file(rows, "prompts/task.txt")
    schema = packet_file(rows, "answer.schema.json")
    require(task.endswith(b"\n") and schema.endswith(b"\n"), "prompt/schema newline drifted")
    message = task + (
        b"\nThe common answer schema and exact scientific arm inputs below are the complete model-visible context. "
        b"Scientific paths are virtual /input-relative evidence paths; no filesystem is available. "
        b"Treat the bytes between each exact marker as the named file and do not infer any unlisted file.\n"
    )
    message += render_block("COMMON ANSWER SCHEMA", "answer.schema.json", schema)
    scientific: list[tuple[str, bytes]] = []
    for row in manifest["files"]:
        data = source_file(row)
        path = row["mounted_path"]
        scientific.append((path, data))
        message += render_block("SCIENTIFIC ARM INPUT", path, data)
    message += b"\nReturn the one JSON object now.\n"
    return message, scientific


def request_value(
    run_id: str,
    arm: str,
    session_id: str,
    system: bytes,
    message: bytes,
    input_manifest: bytes,
    answer_schema: bytes,
    scientific_count: int,
) -> dict[str, Any]:
    return {
        "schema": "vela.claim-dependency-pi-participant-request.v1",
        "experiment_id": "synthetic-counterfactual-erdos-321-v0",
        "run_id": run_id,
        "arm": arm,
        "session_id": session_id,
        "provider": "openai-codex",
        "model": "gpt-5.6-sol",
        "thinking_level": "high",
        "system_prompt": system.decode(),
        "system_prompt_raw_root": raw_root(system),
        "user_message": message.decode(),
        "user_message_raw_root": raw_root(message),
        "input_manifest_raw_root": raw_root(input_manifest),
        "answer_schema_raw_root": raw_root(answer_schema),
        "embedded_scientific_input_count": scientific_count,
        "embedded_answer_schema": True,
        "output_contract": "last_assistant_text_only",
        "authority_effect": "none",
        "claim_credit": False,
    }


def write_file(base: Path, name: str, data: bytes, mode: int = 0o644) -> None:
    path = contained_path(base, relative_path(name, "generated path"))
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        raise ContractError(f"generated path collision: {name}")
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fsync(descriptor)
        os.fchmod(descriptor, mode)
    finally:
        os.close(descriptor)


def file_ledger(root_path: Path, excluded: set[str]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in sorted(root_path.rglob("*")):
        metadata = path.lstat()
        name = path.relative_to(root_path).as_posix()
        if stat.S_ISLNK(metadata.st_mode) or not (
            stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)
        ):
            raise ContractError(f"nonregular generated path: {name}")
        if not stat.S_ISREG(metadata.st_mode) or name in excluded:
            continue
        data = read_regular(path, 16_777_216)
        row: dict[str, Any] = {
            "path": name,
            "mode": f"100{stat.S_IMODE(metadata.st_mode):03o}",
            "bytes": len(data),
            "raw_root": raw_root(data),
        }
        if name.endswith(".json"):
            row["canonical_root"] = raw_root(rfc8785.dumps(parse_json(data, name)))
        rows.append(row)
    return rows


def synthetic_auth() -> bytes:
    header = base64.urlsafe_b64encode(b'{"alg":"none","typ":"JWT"}').rstrip(b"=")
    payload_value = {
        "exp": 4_102_444_800,
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-capture-account"
        },
    }
    payload = base64.urlsafe_b64encode(rfc8785.dumps(payload_value)).rstrip(b"=")
    access = b".".join((header, payload, b"synthetic"))
    value = {
        "openai-codex": {
            "type": "oauth",
            "access": access.decode(),
            "refresh": "vela-nonrefreshable-sentinel-v1",
            "expires": 4_102_444_800_000,
            "accountId": "synthetic-capture-account",
        }
    }
    return json_bytes(value)


def ensure_output_outside_git(output: Path) -> None:
    existing = output.parent
    while not existing.exists() and existing != existing.parent:
        existing = existing.parent
    require(existing.is_dir(), "output has no existing directory ancestor")
    for predicate in ("--is-inside-work-tree", "--is-inside-git-dir"):
        result = subprocess.run(
            ["git", "-C", str(existing), "rev-parse", predicate],
            check=False,
            capture_output=True,
            text=True,
            env=git_environment(),
        )
        if result.returncode == 0 and result.stdout.strip() == "true":
            raise ContractError("output must be outside every Git worktree and Git directory")


def packet_identity(
    source_manifest: dict[str, Any], *, development_worktree: bool
) -> dict[str, Any]:
    head = git_text(["rev-parse", "--verify", "HEAD^{commit}"], "HEAD")
    require(COMMIT.fullmatch(head) is not None, "HEAD is invalid")
    parent = source_manifest["source_parent_commit"]
    require(
        git_result(["merge-base", "--is-ancestor", parent, head]).returncode == 0,
        "source manifest parent is not an ancestor of HEAD",
    )
    manifest_data = read_regular(PACKET / PACKET_MANIFEST, 262_144, 0o644)
    identity = {
        "status": "uncommitted_test_only" if development_worktree else "committed_clean",
        "repository_head": head,
        "packet_path": PACKET.relative_to(REPOSITORY).as_posix(),
        "packet_tree": None,
        "source_parent_commit": parent,
        "source_manifest_raw_root": raw_root(manifest_data),
        "source_manifest_files_canonical_root": source_manifest[
            "files_canonical_root"
        ],
    }
    if development_worktree:
        return identity
    relative = PACKET.relative_to(REPOSITORY).as_posix()
    status = git_text(
        [
            "--literal-pathspecs",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            relative,
        ],
        "packet status",
    )
    require(not status, "source packet must be committed and clean")
    identity["packet_tree"] = git_text(
        ["rev-parse", f"HEAD:{relative}"], "packet tree"
    )
    return identity


def evidence_file(
    attestation_path: Path, row: Any, label: str, maximum: int = 2_097_152
) -> tuple[Any, bytes]:
    require(
        isinstance(row, dict)
        and set(row) == {"path", "mode", "bytes", "raw_root"}
        and row["mode"] == "0444",
        f"{label} evidence row is invalid",
    )
    path = contained_path(
        attestation_path.parent.resolve(),
        relative_path(row["path"], f"{label} evidence path"),
    )
    data = read_regular(path, maximum, 0o444)
    require(
        len(data) == row["bytes"] and raw_root(data) == row["raw_root"],
        f"{label} evidence root drifted",
    )
    return parse_json(data, label), data


def validate_attestation(
    value: Any,
    request_roots: dict[str, str],
    requests: dict[str, dict[str, Any]],
    attestation_path: Path,
) -> tuple[dict[str, Any], dict[str, bytes]]:
    require(
        isinstance(value, dict)
        and set(value)
        == {
            "schema",
            "image",
            "evidence",
            "authority_effect",
            "claim_credit",
        },
        "execution attestation shape is invalid",
    )
    image = value["image"]
    require(
        value["schema"] == "vela.claim-dependency-pi-execution-attestation.v1"
        and isinstance(image, dict)
        and set(image)
        == {
            "image_id",
            "platform",
            "node_version",
            "pi_version",
            "package_lock_raw_root",
        }
        and isinstance(image["image_id"], str)
        and ROOT.fullmatch(image["image_id"])
        and image["platform"] == "linux/amd64"
        and image["node_version"] == "v24.12.0"
        and image["pi_version"] == "0.84.1"
        and image["package_lock_raw_root"]
        == raw_root(read_regular(PACKET / "package-lock.json", 2_097_152))
        and value["authority_effect"] == "none"
        and value["claim_credit"] is False,
        "execution image attestation drifted",
    )
    evidence = value["evidence"]
    require(
        isinstance(evidence, dict)
        and set(evidence)
        == {
            "request_captures",
            "container_probe",
            "unix_socket_broker_probe",
            "auth_cleanup_probe",
        }
        and isinstance(evidence["request_captures"], list)
        and len(evidence["request_captures"]) == len(RUNS),
        "execution evidence shape drifted",
    )
    expected_runs = {run_id: arm for run_id, arm, _ in RUNS}
    captures: dict[str, Any] = {}
    retained_files: dict[str, bytes] = {}
    retained_capture_rows: list[dict[str, Any]] = []
    for index, row in enumerate(evidence["request_captures"]):
        report, report_data = evidence_file(
            attestation_path, row, f"request capture {index}"
        )
        require(
            isinstance(report, dict)
            and set(report)
            == {
                "schema",
                "run_id",
                "arm",
                "request_raw_root",
                "fetch_count",
                "url",
                "method",
                "content_encoding",
                "encoded_request_raw_root",
                "decoded_request_raw_root",
                "instructions_raw_root",
                "user_message_raw_root",
                "response_raw_root",
                "model",
                "reasoning",
                "text",
                "prompt_cache_key",
                "input_message_count",
                "tool_definition_count",
                "continuation_present",
                "session_counts",
                "event_types",
                "sanitized_environment_names",
                "external_network_calls",
                "authority_effect",
                "claim_credit",
            },
            "request capture report shape drifted",
        )
        run_id = report["run_id"]
        require(
            isinstance(run_id, str)
            and run_id in expected_runs
            and run_id not in captures,
            "request capture run set drifted",
        )
        request = requests[run_id]
        expected_instructions = (
            request["system_prompt"] + "\nCurrent working directory: /workspace"
        ).encode()
        require(
            report["schema"] == "vela.claim-dependency-pi-request-capture.v1"
            and report["arm"] == expected_runs[run_id]
            and report["request_raw_root"] == request_roots[run_id]
            and all(
                isinstance(report[name], str)
                and ROOT.fullmatch(report[name]) is not None
                for name in (
                    "encoded_request_raw_root",
                    "decoded_request_raw_root",
                    "instructions_raw_root",
                    "user_message_raw_root",
                    "response_raw_root",
                )
            )
            and report["instructions_raw_root"] == raw_root(expected_instructions)
            and report["user_message_raw_root"] == request["user_message_raw_root"]
            and report["response_raw_root"] == raw_root(b"{}")
            and report["fetch_count"] == 1
            and report["url"]
            == "https://chatgpt.com/backend-api/codex/responses"
            and report["method"] == "POST"
            and report["content_encoding"] == "zstd"
            and report["model"] == "gpt-5.6-sol"
            and report["reasoning"] == {"effort": "high", "summary": "auto"}
            and report["text"] == {"verbosity": "low"}
            and report["prompt_cache_key"] == request["session_id"]
            and report["input_message_count"] == 1
            and report["tool_definition_count"] == 0
            and report["continuation_present"] is False
            and report["external_network_calls"] == 0
            and isinstance(report["event_types"], list)
            and all(isinstance(event, str) for event in report["event_types"])
            and report["event_types"].count("agent_start") == 1
            and report["event_types"].count("agent_end") == 1
            and "agent_settled" in report["event_types"]
            and not any(
                "tool" in event
                or event.startswith(
                    (
                        "auto_retry_",
                        "compaction_",
                        "summarization_retry_",
                    )
                )
                for event in report["event_types"]
            )
            and report["sanitized_environment_names"]
            == ["HTTPS_PROXY", "OPENAI_BASE_URL"]
            and report["session_counts"]
            == {
                "userMessages": 1,
                "assistantMessages": 1,
                "toolCalls": 0,
                "toolResults": 0,
                "totalMessages": 2,
            }
            and report["authority_effect"] == "none"
            and report["claim_credit"] is False,
            "request capture report value drifted",
        )
        captures[run_id] = report
        retained_name = f"evidence/request-captures/{run_id}.json"
        retained_files[retained_name] = report_data
        retained_capture_rows.append(
            {
                "run_id": run_id,
                "path": retained_name,
                "mode": "0444",
                "bytes": len(report_data),
                "raw_root": raw_root(report_data),
            }
        )
    require(set(captures) == set(expected_runs), "request capture run set is incomplete")

    probe_specs = (
        (
            "container_probe",
            "vela.claim-dependency-pi-container-evidence.v1",
            {
                "image_id": image["image_id"],
                "platform": "linux/amd64",
                "node_version": "v24.12.0",
                "pi_version": "0.84.1",
                "package_lock_raw_root": image["package_lock_raw_root"],
                "license_raw_root": "sha256:0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48",
                "nonroot": True,
                "request_read": True,
                "auth_read": True,
                "request_write_refused": True,
                "auth_write_refused": True,
                "authority_effect": "none",
                "claim_credit": False,
            },
        ),
        (
            "unix_socket_broker_probe",
            "vela.claim-dependency-pi-unix-socket-probe.v1",
            {
                "image_id": image["image_id"],
                "participant_network": "none",
                "broker_network": "none",
                "shared_socket_connected": True,
                "invalid_request_refused": True,
                "external_requests": 0,
                "authority_effect": "none",
                "claim_credit": False,
            },
        ),
        (
            "auth_cleanup_probe",
            "vela.claim-dependency-pi-auth-cleanup-probe.v1",
            {
                "image_id": image["image_id"],
                "nonroot": True,
                "derived_created": True,
                "derived_mode": "0400",
                "real_refresh_absent": True,
                "read_succeeded": True,
                "write_refused": True,
                "derived_absent_after": True,
                "temporary_directory_absent_after": True,
                "authority_effect": "none",
                "claim_credit": False,
            },
        ),
    )
    for name, schema, expected in probe_specs:
        report, report_data = evidence_file(
            attestation_path, evidence[name], name
        )
        require(
            isinstance(report, dict)
            and report.get("schema") == schema
            and {key: report.get(key) for key in expected} == expected
            and set(report) == {"schema", *expected},
            f"{name} report drifted",
        )
        retained_files[f"evidence/{name}.json"] = report_data
    retained = {
        "schema": value["schema"],
        "image": image,
        "evidence": {
            "request_captures": sorted(
                retained_capture_rows,
                key=lambda row: [run_id for run_id, _, _ in RUNS].index(
                    row["run_id"]
                ),
            ),
            "container_probe": {
                "path": "evidence/container_probe.json",
                "mode": "0444",
                "bytes": len(retained_files["evidence/container_probe.json"]),
                "raw_root": raw_root(
                    retained_files["evidence/container_probe.json"]
                ),
            },
            "unix_socket_broker_probe": {
                "path": "evidence/unix_socket_broker_probe.json",
                "mode": "0444",
                "bytes": len(
                    retained_files["evidence/unix_socket_broker_probe.json"]
                ),
                "raw_root": raw_root(
                    retained_files["evidence/unix_socket_broker_probe.json"]
                ),
            },
            "auth_cleanup_probe": {
                "path": "evidence/auth_cleanup_probe.json",
                "mode": "0444",
                "bytes": len(
                    retained_files["evidence/auth_cleanup_probe.json"]
                ),
                "raw_root": raw_root(
                    retained_files["evidence/auth_cleanup_probe.json"]
                ),
            },
        },
        "authority_effect": "none",
        "claim_credit": False,
    }
    return retained, retained_files


def build(
    output: Path,
    attestation_path: Path | None,
    *,
    development_worktree: bool,
) -> dict[str, Any]:
    rows, source_manifest = packet_manifest()
    validate_frozen_sources()
    package_lock = packet_file(rows, "package-lock.json")
    validate_package_lock(package_lock)
    source_packet = packet_identity(
        source_manifest, development_worktree=development_worktree
    )
    system = packet_file(rows, "prompts/system.txt")
    answer_schema = packet_file(rows, "answer.schema.json")
    plan = packet_file(rows, "plan.json")
    runtime = packet_file(rows, "runtime.json")
    write_file(output, "plan.json", plan)
    write_file(output, "runtime.json", runtime)
    write_file(output, "source-packet-manifest.json", json_bytes(source_manifest))
    license_encoded = packet_file(rows, "LICENSE.pi-v0.84.1.base64")
    try:
        normalized_license = b"".join(license_encoded.split())
        require(
            base64.b64encode(base64.b64decode(normalized_license, validate=True))
            == normalized_license,
            "tagged license base64 is not canonical",
        )
        license_bytes = base64.b64decode(normalized_license, validate=True)
    except ValueError as exc:
        raise ContractError("tagged license base64 is invalid") from exc
    require(
        len(license_bytes) == 1_069
        and raw_root(license_bytes)
        == "sha256:0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48",
        "tagged Pi license bytes drifted",
    )
    for name in RUNTIME_FILES:
        mode = 0o755 if name.endswith((".mjs", ".sh")) else 0o644
        write_file(output, f"runtime/{name}", packet_file(rows, name), mode)
    write_file(output, "runtime/LICENSE.pi-v0.84.1", license_bytes)
    write_file(output, "capture/synthetic-auth.json", synthetic_auth(), 0o400)
    arm_messages: dict[str, bytes] = {}
    arm_manifests: dict[str, bytes] = {}
    for arm in ARMS:
        manifest, manifest_data = load_input_manifest(rows, arm)
        message, scientific = render_user_message(rows, arm, manifest)
        arm_messages[arm] = message
        arm_manifests[arm] = manifest_data
        write_file(output, f"arms/{arm}/input-manifest.json", manifest_data, 0o444)
        write_file(output, f"arms/{arm}/user-message.txt", message, 0o444)
        for path, data in scientific:
            write_file(output, f"verifier/{arm}/input/{path}", data, 0o444)
    profile_block = render_block(
        "SCIENTIFIC ARM INPUT",
        "profile.json",
        source_file(load_input_manifest(rows, ARMS[1])[0]["files"][-1]),
    )
    require(
        arm_messages[ARMS[1]].replace(profile_block, b"", 1)
        == arm_messages[ARMS[0]],
        "treatment message differs beyond the exact profile block",
    )
    write_file(output, "verifier/answer.schema.json", answer_schema, 0o444)
    write_file(
        output,
        "verifier/answer-key.json",
        packet_file(rows, "answer-key.json"),
        0o444,
    )
    write_file(output, "verifier/scorer.py", packet_file(rows, "scorer.py"), 0o555)
    request_roots: dict[str, str] = {}
    requests: dict[str, dict[str, Any]] = {}
    for run_id, arm, session_id in RUNS:
        request_object = request_value(
            run_id,
            arm,
            session_id,
            system,
            arm_messages[arm],
            arm_manifests[arm],
            answer_schema,
            7 if arm == ARMS[0] else 8,
        )
        request = json_bytes(request_object)
        write_file(output, f"runs/{run_id}/request.json", request, 0o444)
        request_roots[run_id] = raw_root(request)
        requests[run_id] = request_object
    attestation = None
    if attestation_path is not None:
        require(not development_worktree, "development packet cannot accept execution attestation")
        attestation_data = read_regular(attestation_path, 262_144, 0o444)
        attestation, retained_evidence = validate_attestation(
            parse_json(attestation_data, "execution attestation"),
            request_roots,
            requests,
            attestation_path,
        )
        for name, data in retained_evidence.items():
            write_file(output, name, data, 0o444)
        write_file(output, "execution-attestation.json", json_bytes(attestation), 0o444)
    ledger = file_ledger(output, {"study-manifest.json"})
    manifest = {
        "schema": "vela.claim-dependency-pi-observation-study-manifest.v1",
        "experiment_id": "synthetic-counterfactual-erdos-321-v0",
        "source_packet": source_packet,
        "request_roots": request_roots,
        "files": ledger,
        "files_canonical_root": raw_root(rfc8785.dumps(ledger)),
        "execution_attestation": attestation,
        "ready_for_participant_runs": attestation is not None
        and not development_worktree,
        "run_order": [run_id for run_id, _, _ in RUNS],
        "prior_invalid_outputs_imported": False,
        "authority_effect": "none",
        "claim_credit": False,
    }
    write_file(output, "study-manifest.json", json_bytes(manifest))
    return manifest


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--execution-attestation", type=Path)
    result.add_argument("--development-worktree", action="store_true")
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    output = args.output.resolve()
    if (
        output.exists()
        or output.is_symlink()
        or output == Path("/")
        or output == Path.home().resolve()
    ):
        print("error: output must be a new, narrow directory", file=os.sys.stderr)
        return 1
    staging: Path | None = None
    try:
        ensure_output_outside_git(output)
        output.parent.mkdir(parents=True, exist_ok=True)
        staging = Path(tempfile.mkdtemp(prefix=f".{output.name}-", dir=output.parent))
        manifest = build(
            staging,
            args.execution_attestation,
            development_worktree=args.development_worktree,
        )
        staging.rename(output)
        staging = None
        os.sys.stdout.buffer.write(json_bytes(manifest))
        return 0
    except (ContractError, OSError) as exc:
        print(f"error: {exc}", file=os.sys.stderr)
        return 1
    finally:
        if staging is not None and staging.exists():
            shutil.rmtree(staging)


if __name__ == "__main__":
    raise SystemExit(main())
