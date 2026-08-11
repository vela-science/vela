#!/usr/bin/env python3
"""Materialize the frozen Harbor observation tasks without running a model."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any, Sequence

import rfc8785


PACKET = Path(__file__).resolve().parent
REPOSITORY = PACKET.parents[2]
ARMS = ("disciplined-git-ro-crate", "rooted-source-plus-profile")
COMMON_INPUT_MAP = (
    (
        "conformance/experiments/claim-dependency-profile-v0/baseline/raw-source.json",
        "baseline/raw-source.json",
    ),
    (
        "conformance/experiments/claim-dependency-profile-v0/state.json",
        "state.json",
    ),
    (
        "conformance/experiments/claim-dependency-profile-v0/participant-task.json",
        "participant-task.json",
    ),
    (
        "conformance/experiments/claim-dependency-profile-v0/dependency-semantics.json",
        "dependency-semantics.json",
    ),
    (
        "conformance/experiments/claim-dependency-profile-v0/baseline/ro-crate-metadata.json",
        "baseline/ro-crate-metadata.json",
    ),
    (
        "conformance/experiments/claim-dependency-profile-v0/baseline/review-record.json",
        "baseline/review-record.json",
    ),
    (
        "paper/artifacts/claim-dependency-profile-v0-observation/shared-scope.json",
        "shared-scope.json",
    ),
)
PROFILE_INPUT = (
    "conformance/experiments/claim-dependency-profile-v0/profile.json",
    "profile.json",
)
RUNS = (
    ("block-1-profile", "rooted-source-plus-profile"),
    ("block-1-baseline", "disciplined-git-ro-crate"),
    ("block-2-baseline", "disciplined-git-ro-crate"),
    ("block-2-profile", "rooted-source-plus-profile"),
)
ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
IMAGE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
PACKET_MANIFEST = "manifest.json"
INSTRUCTION_SUFFIX = (
    b"The only task inputs are regular files under /input. Write the exact JSON object "
    b"to /logs/artifacts/answer.json. Your final assistant message must contain the "
    b"byte-identical JSON object and no other text.\n"
)


class ContractError(ValueError):
    """Stable materialization contract failure."""


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


def root(data: bytes) -> str:
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
        resolved = candidate.resolve(strict=False)
        resolved.relative_to(base)
    except (OSError, ValueError) as exc:
        raise ContractError(f"path escapes base: {relative}") from exc
    current = base
    for part in relative.parts[:-1]:
        current = current / part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            break
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ContractError(f"non-directory or symlink parent refused: {relative}")
    return candidate


def read_regular(path: Path, maximum: int, expected_mode: int | None = None) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise ContractError(f"cannot open regular file {path}: {exc}") from exc
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
            chunk = os.read(descriptor, min(65536, remaining))
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
        raise ContractError(f"file identity changed: {path}: {exc}") from exc
    if (
        len(data) != before.st_size
        or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        or (before.st_dev, before.st_ino) != (current.st_dev, current.st_ino)
    ):
        raise ContractError(f"file changed while reading: {path}")
    return data


def packet_manifest() -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    manifest_path = PACKET / PACKET_MANIFEST
    manifest_data = read_regular(manifest_path, 262_144, 0o644)
    manifest = parse_json(manifest_data, PACKET_MANIFEST)
    require(isinstance(manifest, dict), "source manifest must be an object")
    require(
        set(manifest)
        == {
            "schema",
            "source_parent_commit",
            "files",
            "files_canonical_root",
            "excludes",
            "authority_effect",
            "claim_credit",
        },
        "source manifest key set is invalid",
    )
    require(
        manifest["schema"] == "vela.claim-dependency-observation-source-manifest.v0",
        "source manifest schema is invalid",
    )
    require(
        isinstance(manifest["source_parent_commit"], str)
        and COMMIT.fullmatch(manifest["source_parent_commit"]) is not None,
        "source manifest parent commit is invalid",
    )
    require(isinstance(manifest["files"], list), "source manifest files are invalid")
    require(
        manifest["excludes"] == [PACKET_MANIFEST]
        and manifest["authority_effect"] == "none"
        and manifest["claim_credit"] is False,
        "source manifest boundary fields are invalid",
    )
    rows: dict[str, dict[str, Any]] = {}
    for row in manifest["files"]:
        if not isinstance(row, dict) or set(row) != {
            "path",
            "mode",
            "bytes",
            "raw_root",
        }:
            raise ContractError("source manifest row is invalid")
        path = str(relative_path(row["path"], "source manifest path"))
        if path in rows or path == PACKET_MANIFEST:
            raise ContractError(f"duplicate or self-listed source path: {path}")
        require(
            isinstance(row["bytes"], int)
            and not isinstance(row["bytes"], bool)
            and row["bytes"] >= 0
            and isinstance(row["raw_root"], str)
            and ROOT.fullmatch(row["raw_root"]) is not None,
            f"invalid registered size or root: {path}",
        )
        rows[path] = row
    actual: set[str] = set()
    for path in PACKET.rglob("*"):
        metadata = path.lstat()
        relative = path.relative_to(PACKET).as_posix()
        if stat.S_ISLNK(metadata.st_mode) or not (
            stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)
        ):
            raise ContractError(f"nonregular packet path refused: {relative}")
        if stat.S_ISREG(metadata.st_mode) and relative != PACKET_MANIFEST:
            actual.add(relative)
    if set(rows) != actual:
        raise ContractError(
            f"source manifest path set mismatch: missing={sorted(actual - set(rows))}, "
            f"extra={sorted(set(rows) - actual)}"
        )
    for name, row in rows.items():
        mode_text = row["mode"]
        if not isinstance(mode_text, str) or not re.fullmatch(
            r"100(?:644|755)", mode_text
        ):
            raise ContractError(f"invalid registered mode: {name}")
        data = read_regular(
            contained_path(PACKET, relative_path(name, "packet path")),
            4_194_304,
            int(mode_text[-3:], 8),
        )
        if len(data) != row["bytes"] or root(data) != row["raw_root"]:
            raise ContractError(f"source file drift: {name}")
    if root(rfc8785.dumps(manifest["files"])) != manifest.get("files_canonical_root"):
        raise ContractError("source manifest files root is invalid")
    return rows, manifest


def packet_file(rows: dict[str, dict[str, Any]], name: str) -> bytes:
    if name not in rows:
        raise ContractError(f"unregistered packet input: {name}")
    data = read_regular(
        contained_path(PACKET, relative_path(name, "packet input")),
        4_194_304,
        int(rows[name]["mode"][-3:], 8),
    )
    if len(data) != rows[name]["bytes"] or root(data) != rows[name]["raw_root"]:
        raise ContractError(f"packet input changed after manifest validation: {name}")
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


def git_result(
    arguments: Sequence[str], *, text: bool = True
) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=REPOSITORY,
            check=False,
            capture_output=True,
            text=text,
            env=git_environment(),
        )
    except OSError as exc:
        raise ContractError(f"cannot execute Git: {exc}") from exc


def git_text(arguments: Sequence[str], label: str) -> str:
    result = git_result(arguments)
    if result.returncode != 0:
        raise ContractError(f"cannot resolve {label}: {result.stderr.strip()}")
    return result.stdout.strip()


def ensure_output_outside_git(output: Path) -> None:
    existing = output.parent
    while not existing.exists() and existing != existing.parent:
        existing = existing.parent
    require(existing.is_dir(), "output has no existing directory ancestor")
    for predicate in ("--is-inside-work-tree", "--is-inside-git-dir"):
        try:
            result = subprocess.run(
                ["git", "-C", str(existing), "rev-parse", predicate],
                check=False,
                capture_output=True,
                text=True,
                env=git_environment(),
            )
        except OSError as exc:
            raise ContractError(f"cannot inspect output Git boundary: {exc}") from exc
        if result.returncode == 0 and result.stdout.strip() == "true":
            raise ContractError("output must be outside every Git worktree and Git dir")


def packet_identity(
    source_manifest: dict[str, Any], *, development_worktree: bool
) -> dict[str, Any]:
    repository_root = Path(
        git_text(["rev-parse", "--show-toplevel"], "repository root")
    ).resolve()
    require(repository_root == REPOSITORY, "packet repository root is unexpected")
    head = git_text(["rev-parse", "--verify", "HEAD^{commit}"], "repository HEAD")
    require(COMMIT.fullmatch(head) is not None, "repository HEAD is invalid")
    source_parent = source_manifest["source_parent_commit"]
    parent_type = git_text(["cat-file", "-t", source_parent], "source parent commit")
    require(parent_type == "commit", "source parent is not a commit")
    ancestry = git_result(["merge-base", "--is-ancestor", source_parent, head])
    require(
        ancestry.returncode == 0,
        "source manifest parent commit is not an ancestor of repository HEAD",
    )
    manifest_data = read_regular(PACKET / PACKET_MANIFEST, 262_144, 0o644)
    require(
        parse_json(manifest_data, PACKET_MANIFEST) == source_manifest,
        "source manifest changed during identity derivation",
    )
    relative_packet = PACKET.relative_to(REPOSITORY).as_posix()
    identity: dict[str, Any] = {
        "status": "uncommitted_test_only"
        if development_worktree
        else "committed_clean",
        "repository_head": head,
        "packet_path": relative_packet,
        "packet_tree": None,
        "source_parent_commit": source_parent,
        "source_manifest_raw_root": root(manifest_data),
        "source_manifest_files_canonical_root": source_manifest["files_canonical_root"],
    }
    if development_worktree:
        return identity

    status = git_text(
        [
            "--literal-pathspecs",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            relative_packet,
        ],
        "source packet worktree status",
    )
    require(not status, "source packet must be committed and clean")
    packet_tree = git_text(
        ["rev-parse", f"HEAD:{relative_packet}"], "source packet tree"
    )
    require(COMMIT.fullmatch(packet_tree) is not None, "source packet tree is invalid")
    tree_type = git_text(["cat-file", "-t", packet_tree], "source packet tree type")
    require(tree_type == "tree", "source packet identity does not name a tree")
    committed_manifest = git_result(
        ["show", f"HEAD:{relative_packet}/{PACKET_MANIFEST}"], text=False
    )
    if committed_manifest.returncode != 0:
        stderr = committed_manifest.stderr.decode("utf-8", errors="replace").strip()
        raise ContractError(f"cannot read committed source manifest: {stderr}")
    require(
        committed_manifest.stdout == manifest_data,
        "committed source manifest differs from materialized bytes",
    )
    identity["packet_tree"] = packet_tree
    return identity


def source_file(row: dict[str, Any]) -> bytes:
    if set(row) != {"source_path", "mounted_path", "mode", "bytes", "raw_root"}:
        raise ContractError("input manifest row is invalid")
    if row["mode"] != "100644" or not isinstance(row["bytes"], int) or row["bytes"] < 0:
        raise ContractError("input manifest mode or size is invalid")
    source = contained_path(
        REPOSITORY, relative_path(row["source_path"], "source path")
    )
    data = read_regular(source, 1_048_576, 0o644)
    if len(data) != row["bytes"] or root(data) != row["raw_root"]:
        raise ContractError(f"input source drift: {row['source_path']}")
    relative_path(row["mounted_path"], "mounted path")
    return data


def write_file(base: Path, name: str, data: bytes, mode: int = 0o644) -> None:
    path = contained_path(base, relative_path(name, "generated path"))
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        raise ContractError(f"generated path collision: {name}")
    path.write_bytes(data)
    path.chmod(mode)


def render_instruction(rows: dict[str, dict[str, Any]]) -> bytes:
    system = packet_file(rows, "prompts/system.txt")
    task = packet_file(rows, "prompts/task.txt")
    expected = system + b"\n" + task + b"\n" + INSTRUCTION_SUFFIX
    committed = packet_file(rows, "task/instruction.md")
    if committed != expected:
        raise ContractError(
            "committed instruction is not the exact prompt concatenation"
        )
    return committed


def load_input_manifest(
    rows: dict[str, dict[str, Any]], arm: str
) -> tuple[dict[str, Any], bytes]:
    name = f"input-manifests/{arm}.json"
    data = packet_file(rows, name)
    value = parse_json(data, name)
    if (
        not isinstance(value, dict)
        or value.get("schema") != "vela.claim-dependency-observation-input-manifest.v0"
        or value.get("experiment_id") != "synthetic-counterfactual-erdos-321-v0"
        or value.get("arm") != arm
        or not isinstance(value.get("files"), list)
    ):
        raise ContractError(f"input manifest shape is invalid: {arm}")
    mounted: set[str] = set()
    for row in value["files"]:
        path = str(relative_path(row.get("mounted_path", ""), "mounted path"))
        if path in mounted:
            raise ContractError(f"duplicate mounted path: {path}")
        mounted.add(path)
    observed_map = tuple(
        (row["source_path"], row["mounted_path"]) for row in value["files"]
    )
    expected_map = (
        COMMON_INPUT_MAP + (PROFILE_INPUT,)
        if arm == "rooted-source-plus-profile"
        else COMMON_INPUT_MAP
    )
    if observed_map != expected_map:
        raise ContractError(f"input allowlist drift: {arm}")
    return value, data


def job_config(run_id: str, arm: str) -> dict[str, Any]:
    return {
        "job_name": f"claim-dependency-{run_id}",
        "jobs_dir": "runs",
        "n_attempts": 1,
        "n_concurrent_trials": 1,
        "retry": {"max_retries": 0},
        "environment": {"type": "docker", "delete": True},
        "verifier": {"disable": False},
        "agents": [
            {
                "name": "codex",
                "model_name": "gpt-5.6-sol",
                "n_concurrent": 1,
                "skills": [],
                "resume_trajectory": False,
                "load_trajectory": None,
                "kwargs": {
                    "version": "0.145.0",
                    "reasoning_effort": "high",
                    "reasoning_summary": "auto",
                    "web_search": "disabled",
                },
                "mcp_servers": [],
            }
        ],
        "tasks": [{"path": f"tasks/{arm}"}],
        "datasets": [],
        "artifacts": [],
        "extra_instruction_paths": [],
    }


def raw_task_config(arm: str) -> dict[str, Any]:
    return {
        "schema_version": "1.3",
        "task": {
            "name": f"vela/claim-dependency-observation-{arm}",
            "description": "Matched read-only claim-dependency interpretation pilot.",
            "authors": [{"name": "Vela"}],
            "keywords": [
                "vela",
                "read-only",
                "claim-dependency",
                "instrumentation",
            ],
        },
        "agent": {
            "timeout_sec": 900.0,
            "user": "participant",
            "network_mode": "allowlist",
            "allowed_hosts": [
                "api.openai.com",
                "chatgpt.com",
                "*.chatgpt.com",
                "auth.openai.com",
                "*.auth.openai.com",
            ],
        },
        "verifier": {
            "timeout_sec": 60.0,
            "environment_mode": "separate",
            "network_mode": "no-network",
            "environment": {
                "network_mode": "no-network",
                "os": "linux",
                "cpus": 1,
                "memory_mb": 512,
                "storage_mb": 1024,
            },
        },
        "environment": {
            "network_mode": "no-network",
            "os": "linux",
            "cpus": 2,
            "memory_mb": 4096,
            "storage_mb": 8192,
            "build_timeout_sec": 900.0,
            "workdir": "/workspace",
        },
        "metadata": {
            "experiment_id": "synthetic-counterfactual-erdos-321-v0",
            "arm": arm,
            "authority_effect": "none",
            "claim_credit": False,
        },
    }


def validate_raw_harbor_configs(output: Path) -> None:
    task_entries = list((output / "tasks").iterdir())
    require(
        all(path.is_dir() and not path.is_symlink() for path in task_entries)
        and {path.name for path in task_entries} == set(ARMS)
        and len(task_entries) == len(ARMS),
        "generated task directory set drifted",
    )
    for arm in ARMS:
        path = output / "tasks" / arm / "task.toml"
        data = read_regular(path, 262_144, 0o644)
        try:
            raw = tomllib.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            raise ContractError(f"invalid generated task TOML: {arm}: {exc}") from exc
        require(
            raw == raw_task_config(arm),
            f"raw generated TaskConfig key or value drift: {arm}",
        )

    job_paths = sorted((output / "jobs").glob("*.json"))
    require(
        {path.stem for path in job_paths} == {run_id for run_id, _ in RUNS}
        and len(job_paths) == len(RUNS),
        "generated job file set drifted",
    )
    require(
        set((output / "jobs").iterdir()) == set(job_paths),
        "unexpected generated job path",
    )
    for run_id, arm in RUNS:
        path = output / "jobs" / f"{run_id}.json"
        data = read_regular(path, 262_144, 0o644)
        raw = parse_json(data, f"raw Harbor job {run_id}")
        require(
            raw == job_config(run_id, arm),
            f"raw generated JobConfig key or value drift: {run_id}",
        )


def validate_harbor(output: Path) -> None:
    validate_raw_harbor_configs(output)
    harbor = shutil.which("harbor")
    if harbor is None:
        raise ContractError("Harbor executable is unavailable")
    version = subprocess.run(
        [harbor, "--version"], check=False, capture_output=True, text=True
    )
    if version.returncode != 0 or version.stdout.strip() != "0.20.0":
        raise ContractError("Harbor 0.20.0 is required")
    executable = Path(harbor).resolve()
    try:
        shebang = executable.read_text(encoding="utf-8").splitlines()[0]
    except (OSError, UnicodeError, IndexError) as exc:
        raise ContractError(f"cannot inspect Harbor executable: {exc}") from exc
    if not shebang.startswith("#!"):
        raise ContractError("Harbor executable has no Python shebang")
    interpreter = shebang[2:]
    validation_script = """
import json, sys
from pathlib import Path
from harbor.models.job.config import JobConfig
from harbor.models.task.config import TaskConfig

ARMS = ('disciplined-git-ro-crate', 'rooted-source-plus-profile')
RUNS = {
    'block-1-profile': 'rooted-source-plus-profile',
    'block-1-baseline': 'disciplined-git-ro-crate',
    'block-2-baseline': 'disciplined-git-ro-crate',
    'block-2-profile': 'rooted-source-plus-profile',
}
RETRY_EXCLUSIONS = {
    'AgentTimeoutError',
    'VerifierTimeoutError',
    'RewardFileNotFoundError',
    'RewardFileEmptyError',
    'VerifierOutputParseError',
    'ApiUsageLimitError',
    'AgentSafetyRefusalError',
    'AgentAuthenticationError',
    'ModelNotFoundError',
}

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def expected_environment(*, cpus, memory_mb, storage_mb, build_timeout_sec, workdir):
    return {
        'network_mode': 'no-network',
        'allowed_hosts': None,
        'build_timeout_sec': build_timeout_sec,
        'docker_image': None,
        'os': 'linux',
        'cpus': cpus,
        'memory_mb': memory_mb,
        'storage_mb': storage_mb,
        'gpus': None,
        'gpu_types': None,
        'tpu': None,
        'mcp_servers': [],
        'env': {},
        'skills_dir': None,
        'healthcheck': None,
        'workdir': workdir,
    }

def expected_task(arm):
    verifier_environment = expected_environment(
        cpus=1,
        memory_mb=512,
        storage_mb=1024,
        build_timeout_sec=600.0,
        workdir=None,
    )
    return {
        'schema_version': '1.3',
        'task': {
            'name': f'vela/claim-dependency-observation-{arm}',
            'description': 'Matched read-only claim-dependency interpretation pilot.',
            'authors': [{'name': 'Vela', 'email': None}],
            'keywords': ['vela', 'read-only', 'claim-dependency', 'instrumentation'],
        },
        'metadata': {
            'experiment_id': 'synthetic-counterfactual-erdos-321-v0',
            'arm': arm,
            'authority_effect': 'none',
            'claim_credit': False,
        },
        'verifier': {
            'network_mode': 'no-network',
            'allowed_hosts': None,
            'timeout_sec': 60.0,
            'env': {},
            'user': None,
            'environment_mode': 'separate',
            'environment': verifier_environment,
            'collect': [],
        },
        'agent': {
            'network_mode': 'allowlist',
            'allowed_hosts': [
                'api.openai.com',
                'chatgpt.com',
                '*.chatgpt.com',
                'auth.openai.com',
                '*.auth.openai.com',
            ],
            'timeout_sec': 900.0,
            'user': 'participant',
        },
        'environment': expected_environment(
            cpus=2,
            memory_mb=4096,
            storage_mb=8192,
            build_timeout_sec=900.0,
            workdir='/workspace',
        ),
        'solution': {'env': {}},
        'source': None,
        'multi_step_reward_strategy': None,
        'steps': None,
        'artifacts': [],
    }

def expected_job(run_id, arm):
    return {
        'job_name': f'claim-dependency-{run_id}',
        'jobs_dir': 'runs',
        'n_attempts': 1,
        'install_only': False,
        'timeout_multiplier': 1.0,
        'agent_timeout_multiplier': None,
        'verifier_timeout_multiplier': None,
        'agent_setup_timeout_multiplier': None,
        'environment_build_timeout_multiplier': None,
        'debug': False,
        'n_concurrent_trials': 1,
        'quiet': False,
        'retry': {
            'max_retries': 0,
            'include_exceptions': None,
            'wait_multiplier': 1.0,
            'min_wait_sec': 1.0,
            'max_wait_sec': 60.0,
        },
        'environment': {
            'type': 'docker',
            'import_path': None,
            'force_build': False,
            'delete': True,
            'cpu_enforcement_policy': 'auto',
            'memory_enforcement_policy': 'auto',
            'override_cpus': None,
            'override_memory_mb': None,
            'override_storage_mb': None,
            'override_gpus': None,
            'override_tpu': None,
            'mounts': None,
            'extra_docker_compose': [],
            'kwargs': {},
            'extra_allowed_hosts': [],
        },
        'verifier': {
            'override_timeout_sec': None,
            'max_timeout_sec': None,
            'disable': False,
        },
        'metrics': [],
        'agents': [{
            'name': 'codex',
            'import_path': None,
            'model_name': 'gpt-5.6-sol',
            'n_concurrent': 1,
            'concurrency_group': None,
            'skills': [],
            'override_timeout_sec': None,
            'override_setup_timeout_sec': None,
            'max_timeout_sec': None,
            'resume_trajectory': False,
            'load_trajectory': None,
            'extra_allowed_hosts': [],
            'kwargs': {
                'version': '0.145.0',
                'reasoning_effort': 'high',
                'reasoning_summary': 'auto',
                'web_search': 'disabled',
            },
            'mcp_servers': [],
        }],
        'datasets': [],
        'tasks': [{
            'path': f'tasks/{arm}',
            'git_url': None,
            'git_commit_id': None,
            'name': None,
            'ref': None,
            'overwrite': False,
            'download_dir': None,
            'source': None,
        }],
        'artifacts': [],
        'extra_instruction_paths': [],
    }

root = Path(sys.argv[1])
task_paths = sorted((root / 'tasks').iterdir())
require([path.name for path in task_paths] == sorted(ARMS), 'resolved task set drift')
for task_path in task_paths:
    arm = task_path.name
    config = TaskConfig.model_validate_toml((task_path / 'task.toml').read_text())
    resolved = config.model_dump(mode='json', exclude_none=False)
    require(resolved == expected_task(arm), f'full resolved TaskConfig drift: {arm}')

job_paths = sorted((root / 'jobs').glob('*.json'))
require({path.stem for path in job_paths} == set(RUNS), 'resolved job set drift')
for job_path in job_paths:
    run_id = job_path.stem
    raw = json.loads(job_path.read_text())
    config = JobConfig.model_validate(raw)
    require(config.environment.env == {}, f'job environment env drift: {run_id}')
    require(config.agents[0].env == {}, f'agent env drift: {run_id}')
    require(config.agents[0].include_logs == [], f'agent include logs drift: {run_id}')
    require(config.agents[0].exclude_logs == [], f'agent exclude logs drift: {run_id}')
    require(config.verifier.env == {}, f'verifier env drift: {run_id}')
    require(config.verifier.kwargs == {}, f'verifier kwargs drift: {run_id}')
    require(config.verifier.import_path is None, f'verifier import drift: {run_id}')
    require(config.verifier.include_logs == [], f'verifier include logs drift: {run_id}')
    require(config.verifier.exclude_logs == [], f'verifier exclude logs drift: {run_id}')
    resolved = config.model_dump(
        mode='json', exclude_none=False, context={'redact_sensitive_env': False}
    )
    exclusions = set(resolved['retry'].pop('exclude_exceptions'))
    require(exclusions == RETRY_EXCLUSIONS, f'retry exclusion drift: {run_id}')
    require(
        resolved == expected_job(run_id, RUNS[run_id]),
        f'full resolved JobConfig drift: {run_id}',
    )
print('ok')
"""
    checked = subprocess.run(
        [interpreter, "-c", validation_script, str(output)],
        cwd=output,
        check=False,
        capture_output=True,
        text=True,
    )
    if checked.returncode != 0 or checked.stdout.strip() != "ok":
        raise ContractError(f"Harbor model validation failed: {checked.stderr.strip()}")
    for run_id, _ in RUNS:
        result = subprocess.run(
            [harbor, "run", "--config", f"jobs/{run_id}.json", "--print-config"],
            cwd=output,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise ContractError(
                f"Harbor config resolution failed: {run_id}: {result.stderr.strip()}"
            )
        resolved = parse_json(result.stdout.encode(), f"resolved Harbor job {run_id}")
        expected = {
            "job_name": f"claim-dependency-{run_id}",
            "jobs_dir": "runs",
            "n_concurrent_trials": 1,
            "agents": [
                {
                    "name": "codex",
                    "model_name": "gpt-5.6-sol",
                    "n_concurrent": 1,
                    "kwargs": {
                        "reasoning_effort": "high",
                        "reasoning_summary": "auto",
                        "version": "0.145.0",
                        "web_search": "disabled",
                    },
                }
            ],
            "tasks": [{"path": f"tasks/{dict(RUNS)[run_id]}"}],
        }
        if resolved != expected:
            raise ContractError(f"resolved Harbor config drift: {run_id}")
        write_file(output, f"resolved-jobs/{run_id}.json", json_bytes(resolved))


def file_ledger(root_path: Path, excluded: set[str]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in sorted(item for item in root_path.rglob("*") if item.is_file()):
        name = path.relative_to(root_path).as_posix()
        if name in excluded:
            continue
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode):
            raise ContractError(f"nonregular generated file: {name}")
        data = read_regular(path, 8_388_608)
        row: dict[str, Any] = {
            "path": name,
            "mode": f"100{stat.S_IMODE(metadata.st_mode):03o}",
            "bytes": len(data),
            "raw_root": root(data),
        }
        if name.endswith(".json"):
            value = parse_json(data, name)
            row["canonical_root"] = root(rfc8785.dumps(value))
        rows.append(row)
    return rows


def validate_attestation(value: Any, task_roots: dict[str, str]) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {
        "schema",
        "harbor_version",
        "docker_client_version",
        "docker_server_version",
        "agent_image_ids",
        "verifier_image_ids",
        "linux_codex_binary_raw_root",
        "task_roots",
        "shell_probe",
        "auth_mechanism",
        "codex_force_auth_json",
        "codex_auth_json_path_set",
        "openai_api_key_set",
        "openai_base_url",
    }:
        raise ContractError("execution attestation shape is invalid")
    if (
        value["schema"] != "vela.claim-dependency-observation-execution-attestation.v0"
        or value["harbor_version"] != "0.20.0"
        or value["task_roots"] != task_roots
        or value["shell_probe"] != "passed"
        or value["auth_mechanism"] != "codex_auth_json_transport"
        or value["codex_force_auth_json"] != "1"
        or value["codex_auth_json_path_set"] is not False
        or value["openai_api_key_set"] is not False
        or value["openai_base_url"] is not None
        or not ROOT.fullmatch(value["linux_codex_binary_raw_root"])
    ):
        raise ContractError("execution attestation values are invalid")
    for field in ("agent_image_ids", "verifier_image_ids"):
        if set(value[field]) != set(ARMS) or not all(
            IMAGE.fullmatch(item) for item in value[field].values()
        ):
            raise ContractError(f"execution attestation image map is invalid: {field}")
    return value


def build(
    output: Path,
    attestation_path: Path | None,
    *,
    development_worktree: bool,
) -> dict[str, Any]:
    require(
        not development_worktree or attestation_path is None,
        "development-worktree materialization forbids execution attestation",
    )
    rows, source_manifest = packet_manifest()
    source_packet = packet_identity(
        source_manifest, development_worktree=development_worktree
    )
    instruction = render_instruction(rows)
    plan = packet_file(rows, "plan.json")
    answer_schema = packet_file(rows, "answer.schema.json")
    answer_key = packet_file(rows, "answer-key.json")
    scorer = packet_file(rows, "scorer.py")
    task_toml = packet_file(rows, "task/task.toml").decode("utf-8")
    environment_dockerfile = packet_file(rows, "task/environment/Dockerfile")
    verifier_dockerfile = packet_file(rows, "task/tests/Dockerfile")
    verifier_script = packet_file(rows, "task/tests/test.sh")
    write_file(output, "plan.json", plan)
    write_file(output, "source-packet-manifest.json", json_bytes(source_manifest))

    input_manifests: dict[str, tuple[dict[str, Any], bytes]] = {}
    for arm in ARMS:
        manifest, manifest_data = load_input_manifest(rows, arm)
        input_manifests[arm] = manifest, manifest_data
        task_root = f"tasks/{arm}"
        rendered_toml = task_toml.replace("{{ARM}}", arm)
        if "{{" in rendered_toml or "}}" in rendered_toml:
            raise ContractError(f"unrendered task token: {arm}")
        write_file(output, f"{task_root}/instruction.md", instruction)
        write_file(output, f"{task_root}/task.toml", rendered_toml.encode())
        write_file(
            output, f"{task_root}/environment/Dockerfile", environment_dockerfile
        )
        write_file(output, f"{task_root}/environment/answer.schema.json", answer_schema)
        write_file(output, f"{task_root}/tests/Dockerfile", verifier_dockerfile)
        write_file(output, f"{task_root}/tests/test.sh", verifier_script, 0o755)
        write_file(output, f"{task_root}/tests/scorer.py", scorer, 0o755)
        write_file(output, f"{task_root}/tests/answer-key.json", answer_key)
        write_file(output, f"{task_root}/tests/input-manifest.json", manifest_data)
        for row in manifest["files"]:
            data = source_file(row)
            mounted = str(relative_path(row["mounted_path"], "mounted path"))
            write_file(output, f"{task_root}/environment/input/{mounted}", data, 0o444)
            write_file(output, f"{task_root}/tests/input/{mounted}", data, 0o444)

    for run_id, arm in RUNS:
        write_file(output, f"jobs/{run_id}.json", json_bytes(job_config(run_id, arm)))
    validate_harbor(output)

    task_roots: dict[str, str] = {}
    for arm in ARMS:
        ledger = file_ledger(output / "tasks" / arm, set())
        task_roots[arm] = root(rfc8785.dumps(ledger))

    attestation: dict[str, Any] | None = None
    if attestation_path is not None:
        data = read_regular(attestation_path, 262_144, 0o644)
        attestation = validate_attestation(
            parse_json(data, str(attestation_path)), task_roots
        )
        write_file(output, "execution-attestation.json", json_bytes(attestation))

    ledger = file_ledger(output, {"study-manifest.json"})
    manifest = {
        "schema": "vela.claim-dependency-observation-study-manifest.v0",
        "experiment_id": "synthetic-counterfactual-erdos-321-v0",
        "source_packet": source_packet,
        "task_roots": task_roots,
        "files": ledger,
        "files_canonical_root": root(rfc8785.dumps(ledger)),
        "execution_attestation": attestation,
        "ready_for_participant_runs": (
            attestation is not None and not development_worktree
        ),
        "run_order": [run_id for run_id, _ in RUNS],
        "authority_effect": "none",
        "claim_credit": False,
    }
    write_file(output, "study-manifest.json", json_bytes(manifest))
    return manifest


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--execution-attestation", type=Path)
    result.add_argument(
        "--development-worktree",
        action="store_true",
        help=(
            "Allow an uncommitted source packet for deterministic tests only; "
            "forbids execution attestation and participant readiness."
        ),
    )
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    output = args.output.resolve()
    if output.exists() or output == Path("/") or output == Path.home().resolve():
        print("error: output must be a new, narrow directory", file=os.sys.stderr)
        return 1
    try:
        ensure_output_outside_git(output)
        require(
            not args.development_worktree or args.execution_attestation is None,
            "development-worktree materialization forbids execution attestation",
        )
        output.parent.mkdir(parents=True, exist_ok=True)
        staging = Path(tempfile.mkdtemp(prefix=f".{output.name}-", dir=output.parent))
    except (ContractError, OSError) as exc:
        print(f"error: {exc}", file=os.sys.stderr)
        return 1
    try:
        manifest = build(
            staging,
            args.execution_attestation,
            development_worktree=args.development_worktree,
        )
        staging.rename(output)
        os.sys.stdout.buffer.write(
            json_bytes(
                {
                    "ok": True,
                    "output": str(output),
                    "files_canonical_root": manifest["files_canonical_root"],
                    "ready_for_participant_runs": manifest[
                        "ready_for_participant_runs"
                    ],
                    "source_packet_status": manifest["source_packet"]["status"],
                    "runs_started": 0,
                }
            )
        )
        return 0
    except (ContractError, OSError, UnicodeError, KeyError, TypeError) as exc:
        shutil.rmtree(staging, ignore_errors=True)
        print(f"error: {exc}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
