#!/usr/bin/env python3
"""Frozen verifier for a commit-pinned external Lean declaration.

The verifier reconstructs source files from a hash manifest into a Vela-owned
Lake project. It ignores the producer's build scripts, workflows, lakefile, and
commands. The only external input elaborated is Lean source, under the pinned
Lean toolchain and Mathlib commit recorded in the replay packet.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import posixpath
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path
from typing import Any

from receipt_v1 import (
    artifact as receipt_artifact,
    distillation_block,
    make_receipt,
    validate_receipt,
    write_json,
)

INSTALL_ROOT = Path(__file__).resolve().parents[1]
_runtime_root = os.environ.get("VELA_EXTERNAL_LEAN_RUNTIME_ROOT")
if _runtime_root is not None and not Path(_runtime_root).is_absolute():
    raise RuntimeError("VELA_EXTERNAL_LEAN_RUNTIME_ROOT must be absolute")
ROOT = Path(_runtime_root).resolve() if _runtime_root is not None else INSTALL_ROOT
sys.dont_write_bytecode = True
SANDBOX_DRIVER = (
    INSTALL_ROOT
    / "vendor"
    / "vela"
    / "crates"
    / "vela-cli"
    / "resources"
    / "external_lean_sandbox.py"
)
if not SANDBOX_DRIVER.is_file():
    raise RuntimeError(
        f"installed external-Lean sandbox core is missing: {SANDBOX_DRIVER}"
    )
_SANDBOX_SPEC = importlib.util.spec_from_file_location(
    "vela_external_lean_sandbox", SANDBOX_DRIVER
)
if _SANDBOX_SPEC is None or _SANDBOX_SPEC.loader is None:
    raise RuntimeError("could not load installed external-Lean sandbox core")
_SANDBOX = importlib.util.module_from_spec(_SANDBOX_SPEC)
_SANDBOX_SPEC.loader.exec_module(_SANDBOX)
ALLOWED_AXIOMS = {"propext", "Quot.sound", "Classical.choice"}
DENIED_AXIOMS = {"sorryAx", "Lean.ofReduceBool", "Lean.trustCompiler"}
VERDICTS = {
    "reproduced",
    "reproduction_failed",
    "dirty_axioms",
    "contradicted",
    "skipped_with_reason",
}
DECLARATION_START = re.compile(r"(?m)^(?:theorem|lemma)\s+(?P<name>[A-Za-z0-9_'.]+)\b")
FULL_COMMIT = re.compile(r"[a-f0-9]{40}")
TOOLCHAIN = re.compile(r"leanprover/lean4:v[0-9]+\.[0-9]+\.[0-9]+(?:-rc[0-9]+)?")
COMMON_SOURCE_ROOTS = {"formal", "src", "lean"}
MAX_LEAN_FILES = 5000
MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_PACKET_BYTES = 1024 * 1024
MAX_MANIFEST_BYTES = 4 * 1024 * 1024
MAX_SOURCE_FILE_BYTES = 8 * 1024 * 1024
LEAN_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_']*")
PACKAGE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}")
GITHUB_REPO_URL = re.compile(
    r"https://github\.com/[A-Za-z0-9][A-Za-z0-9_.-]{0,99}/"
    r"[A-Za-z0-9][A-Za-z0-9_.-]{0,99}(?:\.git)?"
)
CONTENT_HASH = re.compile(r"sha256:[a-f0-9]{64}")
PACKET_FILENAME = re.compile(r"sha256-(?P<digest>[a-f0-9]{64})\.json")
VERIFIER_REGISTRY_PATH = INSTALL_ROOT / "verifiers" / "registry.v1.json"


def toolchain_root(toolchain: str, *, allow_install: bool = True) -> Path:
    if not TOOLCHAIN.fullmatch(toolchain):
        raise ValueError(f"unfrozen or unsupported Lean toolchain: {toolchain}")
    elan_home = Path(os.environ.get("ELAN_HOME", str(Path.home() / ".elan"))).resolve()
    directory = toolchain.replace("/", "--").replace(":", "---")
    root = (elan_home / "toolchains" / directory).resolve()
    lean = root / "bin" / "lean"
    lake = root / "bin" / "lake"
    if (not lean.is_file() or not lake.is_file()) and allow_install:
        elan = shutil.which("elan")
        if not elan:
            raise ValueError(
                f"pinned Lean toolchain is not installed and elan is unavailable: {toolchain}"
            )
        with tempfile.TemporaryDirectory(
            prefix="vela-external-lean-provision-home-"
        ) as home:
            environment = trusted_environment(Path(home), {"ELAN_HOME": str(elan_home)})
            provision = subprocess.run(
                [elan, "toolchain", "install", toolchain],
                cwd=elan_home.parent,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=900,
            )
        if provision.returncode != 0:
            raise ValueError(
                f"could not install pinned Lean toolchain {toolchain}: "
                f"{provision.stderr or provision.stdout}"
            )
    if not lean.is_file() or not lake.is_file():
        raise ValueError(
            f"pinned Lean toolchain is unavailable after installation: {toolchain}"
        )
    return root


def trusted_environment(
    home: Path, extra: dict[str, str] | None = None
) -> dict[str, str]:
    environment = {
        "HOME": str(home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": "/usr/bin:/bin",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_COUNT": "1",
        "GIT_CONFIG_KEY_0": "credential.helper",
        "GIT_CONFIG_VALUE_0": "",
        "GIT_TERMINAL_PROMPT": "0",
        "SSH_AUTH_SOCK": "",
    }
    if extra:
        environment.update(extra)
    return environment


def run_trusted_command(
    command: list[str],
    cwd: Path,
    timeout: int,
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    if environment is not None:
        return subprocess.run(
            command,
            cwd=cwd,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    with tempfile.TemporaryDirectory(
        prefix="vela-external-lean-provision-home-"
    ) as home:
        return subprocess.run(
            command,
            cwd=cwd,
            env=trusted_environment(Path(home)),
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )


def run_sandboxed_lean_command(
    command: list[str],
    cwd: Path,
    timeout: int,
    *,
    source_root: Path,
    toolchain: str,
    dependency_root: Path,
    dependency_identity: str,
    lean_path: str | None = None,
    execution_copy_root: Path | None = None,
    dependency_roots: list[Path] | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict[str, Any]]:
    root = toolchain_root(toolchain, allow_install=False)
    executables = [
        path.resolve()
        for path in [
            root / "bin" / "lake",
            root / "bin" / "lean",
            root / "bin" / "leanc",
        ]
        if path.is_file()
    ]
    git_paths: list[Path] = []
    git_shim = Path("/usr/bin/git")
    xcrun = Path("/usr/bin/xcrun")
    if xcrun.is_file():
        discovered = subprocess.run(
            [str(xcrun), "--find", "git"],
            env=trusted_environment(cwd / ".vela-git-discovery-home"),
            text=True,
            capture_output=True,
            check=False,
            timeout=10,
        )
        if discovered.returncode == 0:
            discovered_git = Path(discovered.stdout.strip())
            if discovered_git.is_absolute() and discovered_git.is_file():
                git_paths.append(discovered_git.resolve())
    if git_shim.is_file():
        git_paths.append(git_shim.resolve())
    executables.extend(path for path in git_paths if path not in executables)
    executable = Path(command[0])
    if not executable.is_absolute():
        matches = [path for path in executables if path.name == executable.name]
        if len(matches) != 1:
            raise ValueError(
                f"Lean sandbox command is not pinned to one toolchain executable: {command[0]}"
            )
        executable = matches[0]
    executable = executable.resolve()
    allowed_executables = [executable] if executable.name == "lean" else executables
    bounded_timeout = min(timeout, _SANDBOX.DEFAULT_LIMITS["wall_seconds"])
    measured_dependencies = [
        path.resolve()
        for path in (
            dependency_roots if dependency_roots is not None else [dependency_root]
        )
    ]
    if not measured_dependencies or any(
        not path.is_dir() for path in measured_dependencies
    ):
        raise ValueError("sandbox dependency roots must be non-empty directories")
    request = {
        "schema": _SANDBOX.SCHEMA_REQUEST,
        "command": [str(executable), *command[1:]],
        "cwd": str(cwd.resolve()),
        "read_roots": [
            str(source_root.resolve()),
            str(root),
            *(str(path) for path in measured_dependencies),
        ],
        "write_root": str(cwd.resolve()),
        "allowed_executables": [str(path) for path in allowed_executables],
        "environment": {
            "PATH": ":".join(
                dict.fromkeys(str(path.parent) for path in allowed_executables)
            ),
            "LEAN_SYSROOT": str(root),
        },
        "root_roles": {
            str(source_root.resolve()): "source",
            str(root): "toolchain",
            **{str(path): "dependencies" for path in measured_dependencies},
        },
        "declared_identities": {
            str(root): {"kind": "lean_toolchain", "value": toolchain},
            **{
                str(path): {
                    "kind": "pinned_dependency_runtime",
                    "value": dependency_identity,
                }
                for path in measured_dependencies
            },
        },
        "limits": {"wall_seconds": bounded_timeout},
    }
    if execution_copy_root is not None:
        request["execution_copy_roots"] = [str(execution_copy_root.resolve())]
    if lean_path:
        request["environment"]["LEAN_PATH"] = lean_path
    sandbox_result = _SANDBOX.execute(request)
    stdout = sandbox_result.get("stdout", {}).get("rendered", "")
    stderr = sandbox_result.get("stderr", {}).get("rendered", "")
    returncode = int(sandbox_result.get("exit_code", 1))
    accepted = sandbox_execution_accepted(sandbox_result)
    if sandbox_result.get("limit_hit"):
        stderr = (
            f"{stderr}\nVela sandbox limit hit: {sandbox_result['limit_hit']}".strip()
        )
    if not sandbox_result.get("protected_inputs_unchanged", True):
        stderr = f"{stderr}\nVela sandbox input-integrity failure".strip()
    if not sandbox_result.get("canonical_inputs_unchanged", True):
        stderr = f"{stderr}\nVela sandbox canonical-input mutation detected".strip()
    if not accepted:
        returncode = returncode or 125
        code = sandbox_result.get("error", {}).get("code", "execution_rejected")
        stderr = f"{stderr}\nVela sandbox rejected execution: {code}".strip()
    return subprocess.CompletedProcess(
        command, returncode, stdout, stderr
    ), sandbox_result


def sandbox_execution_accepted(result: dict[str, Any]) -> bool:
    return (
        result.get("ok") is True
        and result.get("canonical_inputs_unchanged") is True
        and result.get("protected_inputs_unchanged") is True
        and result.get("limit_hit") is None
        and result.get("sandbox", {}).get("fail_closed") is True
    )


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_regular_file(path: Path, *, max_bytes: int | None = None) -> bytes:
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        fact = os.fstat(descriptor)
        if not stat.S_ISREG(fact.st_mode):
            raise ValueError(f"artifact is not a regular file: {path}")
        if max_bytes is not None and fact.st_size > max_bytes:
            raise ValueError(f"artifact exceeds {max_bytes} bytes: {path}")
        chunks: list[bytes] = []
        total = 0
        while block := os.read(descriptor, 1024 * 1024):
            total += len(block)
            if max_bytes is not None and total > max_bytes:
                raise ValueError(f"artifact exceeds {max_bytes} bytes: {path}")
            chunks.append(block)
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def sha256_file(path: Path, *, max_bytes: int | None = None) -> str:
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        fact = os.fstat(descriptor)
        if not stat.S_ISREG(fact.st_mode):
            raise ValueError(f"artifact is not a regular file: {path}")
        if max_bytes is not None and fact.st_size > max_bytes:
            raise ValueError(f"artifact exceeds {max_bytes} bytes: {path}")
        digest = hashlib.sha256()
        total = 0
        while block := os.read(descriptor, 1024 * 1024):
            total += len(block)
            if max_bytes is not None and total > max_bytes:
                raise ValueError(f"artifact exceeds {max_bytes} bytes: {path}")
            digest.update(block)
        return digest.hexdigest()
    finally:
        os.close(descriptor)


def load_json(path: Path, *, max_bytes: int | None = None) -> dict[str, Any]:
    data = json.loads(read_regular_file(path, max_bytes=max_bytes))
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def write_bytes_exclusive(path: Path, content: bytes, *, mode: int) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, mode)
    try:
        view = memoryview(content)
        while view:
            written = os.write(descriptor, view)
            if written < 1:
                raise OSError(f"short write to controlled artifact: {path}")
            view = view[written:]
        os.fsync(descriptor)
        os.fchmod(descriptor, mode)
    finally:
        os.close(descriptor)


def normalized_statement(block: str) -> str:
    signature = block.split(":=", 1)[0]
    return " ".join(signature.split())


def declaration_block(source: str, declaration: str) -> str:
    short_name = declaration.rsplit(".", 1)[-1]
    matches = list(DECLARATION_START.finditer(source))
    for index, match in enumerate(matches):
        if match.group("name") != short_name:
            continue
        end = matches[index + 1].start() if index + 1 < len(matches) else len(source)
        return source[match.start() : end].strip()
    raise ValueError(f"declaration {declaration} not found in source module")


def declaration_statement_sha256(source: str, declaration: str) -> str:
    block = declaration_block(source, declaration)
    return sha256_bytes(normalized_statement(block).encode("utf-8"))


def safe_relative_path(relative: Any, label: str) -> tuple[str, ...]:
    if not isinstance(relative, str) or not relative or len(relative) > 1024:
        raise ValueError(f"{label} must be a bounded non-empty relative path")
    if "\0" in relative or "\\" in relative or relative.startswith("/"):
        raise ValueError(f"{label} is not a portable relative path: {relative!r}")
    parts = tuple(relative.split("/"))
    if any(part in {"", ".", ".."} for part in parts):
        raise ValueError(f"{label} contains an unsafe path component: {relative!r}")
    return parts


def safe_git_input_revision(revision: Any, label: str) -> str:
    if (
        not isinstance(revision, str)
        or not revision
        or len(revision) > 128
        or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,127}", revision)
        or any(part in {"", ".", ".."} for part in revision.split("/"))
    ):
        raise ValueError(f"{label} is not a bounded portable Git revision")
    return revision


def canonical_github_repo_url(url: Any, label: str) -> str:
    if not isinstance(url, str) or not GITHUB_REPO_URL.fullmatch(url):
        raise ValueError(f"{label} must be a canonical public GitHub HTTPS repository")
    return url.removesuffix(".git")


def lean_source_path(relative: Any, label: str = "Lean source path") -> str:
    parts = safe_relative_path(relative, label)
    if not parts[-1].endswith(".lean"):
        raise ValueError(f"{label} must end in .lean: {relative!r}")
    module_parts = [*parts[:-1], parts[-1].removesuffix(".lean")]
    if any(not LEAN_NAME.fullmatch(part) for part in module_parts):
        raise ValueError(
            f"{label} contains a nonportable Lean module component: {relative!r}"
        )
    return "/".join(parts)


def source_snapshot_path(relative: Any) -> str:
    """Validate retained bundle paths without widening the execution set.

    Historical Diderot bundles retain three declarative Lake metadata files
    beside their Lean modules. Only `.lean` paths are elaborated; these other
    exact filenames remain hash-bound provenance.
    """
    parts = safe_relative_path(relative, "source snapshot path")
    normalized = "/".join(parts)
    if normalized.endswith(".lean"):
        return lean_source_path(normalized)
    if len(parts) == 1 and normalized in {
        "formalization.yaml",
        "lake-manifest.json",
        "lean-toolchain",
    }:
        return normalized
    raise ValueError(f"unsupported source snapshot path: {relative!r}")


def validate_declaration(declaration: Any) -> str:
    if not isinstance(declaration, str) or len(declaration) > 512:
        raise ValueError(
            "declaration must be a bounded fully-qualified Lean identifier"
        )
    parts = declaration.split(".")
    if len(parts) < 2 or any(not LEAN_NAME.fullmatch(part) for part in parts):
        raise ValueError("declaration must be a fully-qualified Lean identifier")
    return declaration


def validate_claim_id(claim_id: Any) -> str:
    if (
        not isinstance(claim_id, str)
        or not claim_id
        or len(claim_id) > 512
        or any(ord(character) < 32 or ord(character) == 127 for character in claim_id)
    ):
        raise ValueError("claim_id must be a bounded non-empty printable string")
    return claim_id


def safe_artifact(root: Path, relative: Any) -> Path:
    parts = safe_relative_path(relative, "artifact path")
    unresolved = root.joinpath(*parts)
    current = root
    for part in parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"artifact path contains a symlink: {relative}")
    candidate = unresolved.resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as exc:
        raise ValueError(f"artifact escapes replay root: {relative}") from exc
    return candidate


def load_trusted_packet(
    packet_path: Path,
    trusted_bundle_root: Path,
) -> tuple[Path, bytes, dict[str, Any], str]:
    trusted_root = trusted_bundle_root.resolve(strict=True)
    unresolved = packet_path if packet_path.is_absolute() else Path.cwd() / packet_path
    try:
        relative = unresolved.relative_to(trusted_root).as_posix()
    except ValueError as exc:
        raise ValueError("replay packet is outside the trusted bundle root") from exc
    safe_relative_path(relative, "replay packet path")
    safe_packet = safe_artifact(trusted_root, relative)
    packet_bytes = read_regular_file(safe_packet, max_bytes=MAX_PACKET_BYTES)
    packet_digest = sha256_bytes(packet_bytes)
    filename = PACKET_FILENAME.fullmatch(safe_packet.name)
    if filename is None or filename.group("digest") != packet_digest:
        raise ValueError("replay packet filename does not match its content digest")
    packet = json.loads(packet_bytes)
    if not isinstance(packet, dict):
        raise ValueError("replay packet must contain a JSON object")
    return safe_packet, packet_bytes, packet, packet_digest


def validate_packet_execution_overlay(
    index: dict[str, Any],
    index_relative: str,
    registry_entry: dict[str, Any],
    packet: dict[str, Any],
    packet_digest: str,
) -> dict[str, Any]:
    if (
        index.get("schema") != "vela.lean_packet_execution_index.v1"
        or set(index) != {"schema", "hash_graph", "packets"}
        or index.get("hash_graph")
        != {
            "current_content_hash_embedded": False,
            "executed_verifier_reference": "registry_id_only",
        }
        or index_relative not in registry_entry.get("content_hash_paths", [])
        or not isinstance(index.get("packets"), list)
    ):
        raise ValueError("packet execution index is invalid or cyclic")
    overlay = next(
        (
            item
            for item in index["packets"]
            if isinstance(item, dict) and item.get("packet_sha256") == packet_digest
        ),
        None,
    )
    if overlay is None:
        raise ValueError("historical replay packet is absent from the execution index")
    historical = {
        "id": packet.get("verifier_id"),
        "content_hash": packet.get("verifier_content_hash"),
    }
    required_overlay_keys = {
        "packet_sha256",
        "packet_path",
        "claim_id",
        "historical_verifier",
        "executed_verifier_id",
        "environment_contract",
        "legacy_artifact_root",
    }
    environment_contract = overlay.get("environment_contract")
    valid_environment_contract = (
        isinstance(environment_contract, dict)
        and set(environment_contract) == {"id", "path", "sha256"}
        and bool(
            re.fullmatch(
                r"lean-env-[a-f0-9]{24}", str(environment_contract.get("id", ""))
            )
        )
        and bool(
            re.fullmatch(r"[a-f0-9]{64}", str(environment_contract.get("sha256", "")))
        )
    )
    if valid_environment_contract:
        try:
            safe_relative_path(
                environment_contract["path"], "environment contract path"
            )
        except ValueError:
            valid_environment_contract = False
    if (
        set(overlay) != required_overlay_keys
        or overlay.get("claim_id") != packet.get("claim_id")
        or overlay.get("historical_verifier") != historical
        or overlay.get("executed_verifier_id") != registry_entry.get("id")
        or not valid_environment_contract
    ):
        raise ValueError(
            "historical replay packet execution lineage does not match the packet"
        )
    legacy_artifact_root = overlay.get("legacy_artifact_root")
    if legacy_artifact_root not in {None, packet.get("artifact_root")}:
        raise ValueError(
            "historical replay packet artifact-root exception does not match"
        )
    return overlay


def computed_registry_content_hash(entry: dict[str, Any]) -> str:
    paths = entry.get("content_hash_paths")
    if not isinstance(paths, list) or not paths or len(paths) > 64:
        raise ValueError("active verifier content-hash closure is invalid")
    digest = hashlib.sha256()
    seen: set[str] = set()
    for relative in sorted(paths):
        if not isinstance(relative, str) or relative in seen:
            raise ValueError(
                "active verifier content-hash closure has duplicate or invalid paths"
            )
        seen.add(relative)
        path = safe_artifact(ROOT, relative)
        content = read_regular_file(path, max_bytes=MAX_MANIFEST_BYTES)
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(content)
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def verifier_lineage(
    packet_path: Path,
    packet: dict[str, Any],
    packet_digest: str,
) -> dict[str, Any]:
    registry = load_json(VERIFIER_REGISTRY_PATH, max_bytes=MAX_MANIFEST_BYTES)
    verifier_id = packet.get("verifier_id")
    entry = next(
        (
            item
            for item in registry.get("entries", [])
            if isinstance(item, dict) and item.get("id") == verifier_id
        ),
        None,
    )
    if entry is None or not CONTENT_HASH.fullmatch(str(entry.get("content_hash", ""))):
        raise ValueError("replay packet verifier is not active in the registry")
    if computed_registry_content_hash(entry) != entry["content_hash"]:
        raise ValueError(
            "active verifier content hash does not match its frozen input closure"
        )
    packet_hash = packet.get("verifier_content_hash")
    if not isinstance(packet_hash, str) or not CONTENT_HASH.fullmatch(packet_hash):
        raise ValueError("replay packet has an invalid historical verifier hash")
    executed = {"id": verifier_id, "content_hash": entry["content_hash"]}
    historical = {"id": verifier_id, "content_hash": packet_hash}
    if packet_hash == entry["content_hash"]:
        return {
            "mode": "current_registry_hash",
            "packet_sha256": packet_digest,
            "historical_verifier": historical,
            "executed_verifier": executed,
            "registry_entry": entry,
            "legacy_artifact_root": None,
            "environment_contract": None,
        }

    index_ref = entry.get("packet_execution_index")
    if not isinstance(index_ref, dict):
        raise ValueError(
            "historical replay packet has no registry-bound execution index"
        )
    index_relative = index_ref.get("path")
    index_sha256 = index_ref.get("sha256")
    if not isinstance(index_sha256, str) or not re.fullmatch(
        r"[a-f0-9]{64}", index_sha256
    ):
        raise ValueError(
            "packet execution index registry binding has an invalid digest"
        )
    index_path = safe_artifact(ROOT, index_relative)
    index_bytes = read_regular_file(index_path, max_bytes=MAX_MANIFEST_BYTES)
    if sha256_bytes(index_bytes) != index_sha256:
        raise ValueError(
            "packet execution index digest does not match the registry binding"
        )
    index = json.loads(index_bytes)
    if not isinstance(index, dict):
        raise ValueError("packet execution index must contain a JSON object")
    overlay = validate_packet_execution_overlay(
        index,
        index_relative,
        entry,
        packet,
        packet_digest,
    )
    canonical_packet = safe_artifact(ROOT, overlay.get("packet_path"))
    if sha256_file(canonical_packet, max_bytes=MAX_PACKET_BYTES) != packet_digest:
        raise ValueError(
            "historical replay packet no longer matches its indexed immutable bytes"
        )
    legacy_artifact_root = overlay.get("legacy_artifact_root")
    return {
        "mode": "historical_packet_overlay",
        "packet_sha256": packet_digest,
        "historical_verifier": historical,
        "executed_verifier": executed,
        "registry_entry": entry,
        "legacy_artifact_root": legacy_artifact_root,
        "environment_contract": overlay["environment_contract"],
        "packet_execution_index": {
            "path": index_relative,
            "sha256": index_sha256,
        },
        "canonical_packet_path": str(canonical_packet.relative_to(ROOT)),
        "input_packet_path": str(packet_path),
    }


def safe_output_path(root: Path, relative: Any) -> Path:
    source_path = lean_source_path(relative)
    candidate = root.joinpath(*source_path.split("/"))
    resolved_parent = candidate.parent.resolve()
    try:
        resolved_parent.relative_to(root.resolve())
    except ValueError as exc:
        raise ValueError(
            f"source destination escapes controlled root: {relative}"
        ) from exc
    return candidate


def validate_source_manifest(
    packet_path: Path,
    packet: dict[str, Any],
    trusted_bundle_root: Path,
    *,
    legacy_artifact_root: str | None = None,
) -> tuple[Path, dict[str, Any], list[dict[str, Any]], dict[str, bytes]]:
    trusted_root = trusted_bundle_root.resolve(strict=True)
    if not trusted_root.is_dir():
        raise ValueError("trusted bundle root is not a directory")
    try:
        packet_path.resolve(strict=True).relative_to(trusted_root)
    except ValueError as exc:
        raise ValueError("replay packet is outside the trusted bundle root") from exc
    artifact_root_value = packet.get("artifact_root", ".")
    if legacy_artifact_root is not None:
        if artifact_root_value != legacy_artifact_root:
            raise ValueError(
                "historical packet artifact_root does not match its immutable overlay"
            )
        artifact_root = (packet_path.parent / legacy_artifact_root).resolve(strict=True)
    elif artifact_root_value == ".":
        artifact_root = trusted_root
    else:
        artifact_root_parts = safe_relative_path(artifact_root_value, "artifact_root")
        artifact_root = safe_artifact(trusted_root, "/".join(artifact_root_parts))
    try:
        artifact_root.relative_to(trusted_root)
    except ValueError as exc:
        raise ValueError("artifact_root escapes the trusted bundle root") from exc
    source_manifest = packet.get("source_manifest")
    if not isinstance(source_manifest, dict):
        raise ValueError("packet source_manifest must be an object")
    manifest_path = safe_artifact(artifact_root, source_manifest.get("path"))
    expected_manifest_hash = source_manifest.get("sha256")
    if not isinstance(expected_manifest_hash, str) or not re.fullmatch(
        r"[a-f0-9]{64}", expected_manifest_hash
    ):
        raise ValueError("source manifest digest must be a full SHA-256")
    manifest_bytes = read_regular_file(manifest_path, max_bytes=MAX_MANIFEST_BYTES)
    if sha256_bytes(manifest_bytes) != expected_manifest_hash:
        raise ValueError("source manifest digest mismatch")
    manifest = json.loads(manifest_bytes)
    if not isinstance(manifest, dict):
        raise ValueError("source manifest must contain a JSON object")
    files = manifest.get("files")
    if not isinstance(files, list) or not files:
        raise ValueError("source manifest has no files")
    if len(files) > MAX_LEAN_FILES:
        raise ValueError(f"source manifest exceeds {MAX_LEAN_FILES} files")
    total_bytes = 0
    source_contents: dict[str, bytes] = {}
    source_paths_casefolded: set[str] = set()
    for item in files:
        if not isinstance(item, dict):
            raise ValueError("source manifest file entry must be an object")
        source_path = source_snapshot_path(item.get("source_path"))
        artifact = safe_artifact(artifact_root, item.get("artifact_path"))
        try:
            fact = artifact.stat(follow_symlinks=False)
        except FileNotFoundError as exc:
            raise ValueError(
                f"source artifact missing: {item.get('artifact_path')}"
            ) from exc
        if not stat.S_ISREG(fact.st_mode) or fact.st_size > MAX_SOURCE_FILE_BYTES:
            raise ValueError(
                f"source artifact is not a bounded regular file: {source_path}"
            )
        expected = item.get("sha256")
        if not isinstance(expected, str) or not re.fullmatch(r"[a-f0-9]{64}", expected):
            raise ValueError(f"source artifact has an invalid digest: {source_path}")
        content = read_regular_file(artifact, max_bytes=MAX_SOURCE_FILE_BYTES)
        total_bytes += len(content)
        if total_bytes > MAX_SOURCE_BYTES:
            raise ValueError(f"source artifact bundle exceeds {MAX_SOURCE_BYTES} bytes")
        if sha256_bytes(content) != expected:
            raise ValueError(f"source artifact digest mismatch: {source_path}")
        folded = source_path.casefold()
        if folded in source_paths_casefolded:
            raise ValueError(
                f"source manifest contains a duplicate source path: {source_path}"
            )
        source_paths_casefolded.add(folded)
        source_contents[source_path] = content
        item["source_path"] = source_path
    return artifact_root, manifest, files, source_contents


def audit_declaration(
    source: str, declaration: str, all_sources: list[str]
) -> dict[str, Any]:
    block = declaration_block(source, declaration)
    statement = normalized_statement(block)
    body = block.split(":=", 1)[1].strip() if ":=" in block else ""
    source_joined = "\n".join(all_sources)
    checks = {
        "statement_present": bool(statement),
        "not_vacuous_true": not bool(re.search(r":\s*True(?:\s|$)", statement)),
        "no_sorry_in_declaration": not bool(re.search(r"\b(?:sorry|admit)\b", block)),
        "no_native_decide_in_snapshot": "native_decide" not in source_joined,
        "not_tautological_rfl": body
        not in {"rfl", "by rfl", "by\n  rfl", "by\n    rfl"},
    }
    return {
        "passed": all(checks.values()),
        "checks": checks,
        "statement": statement,
        "statement_sha256": sha256_bytes(statement.encode("utf-8")),
    }


def controlled_lakefile(mathlib_commit: str) -> str:
    return f"""name = "VelaDiderotReplay"
version = "1.0.0"
defaultTargets = ["KrafftSieve"]

[leanOptions]
relaxedAutoImplicit = false

[[require]]
name = "mathlib"
git = "https://github.com/leanprover-community/mathlib4.git"
rev = "{mathlib_commit}"

[[lean_lib]]
name = "KrafftSieve"
srcDir = "../source"
"""


def run_bytes(
    command: list[str],
    cwd: Path,
    timeout: int,
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    timeout_override = os.environ.get("VELA_EXTERNAL_LEAN_TIMEOUT_SECONDS")
    if timeout_override:
        try:
            timeout = min(timeout, max(1, int(timeout_override)))
        except ValueError as exc:
            raise ValueError(
                "VELA_EXTERNAL_LEAN_TIMEOUT_SECONDS must be an integer"
            ) from exc
    if environment is not None:
        return subprocess.run(
            command,
            cwd=cwd,
            env=environment,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    with tempfile.TemporaryDirectory(prefix="vela-external-lean-git-home-") as home:
        return subprocess.run(
            command,
            cwd=cwd,
            env=trusted_environment(Path(home)),
            capture_output=True,
            check=False,
            timeout=timeout,
        )


def parse_axioms(output: str, declaration: str) -> list[str]:
    quoted_name = rf"(?:['`])?{re.escape(declaration)}(?:['`])?"
    listed = list(
        re.finditer(
            rf"{quoted_name}\s+depends on axioms:\s*\[(?P<axioms>[^\]]*)\]",
            output,
            re.MULTILINE,
        )
    )
    empty = list(
        re.finditer(
            rf"{quoted_name}\s+does not depend on any axioms",
            output,
            re.MULTILINE,
        )
    )
    trust_messages = re.findall(
        r"(?:depends on axioms:\s*\[|does not depend on any axioms)",
        output,
    )
    if len(trust_messages) != 1 or len(listed) + len(empty) != 1:
        raise ValueError(
            "Lean output did not contain exactly one unambiguous #print axioms result"
        )
    if empty:
        return []
    axioms = [
        item.strip() for item in listed[0].group("axioms").split(",") if item.strip()
    ]
    if not axioms or any(
        not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_']*(?:\.[A-Za-z_][A-Za-z0-9_']*)*", item)
        for item in axioms
    ):
        raise ValueError("Lean output contained an invalid or empty axiom list")
    return axioms


def local_module_build_order(
    source_by_path: dict[str, str], module_path: str
) -> list[tuple[str, str]]:
    modules = {
        path.removesuffix(".lean").replace("/", "."): path
        for path in source_by_path
        if path.endswith(".lean")
    }
    target = module_path.removesuffix(".lean").replace("/", ".")
    if modules.get(target) != module_path:
        raise ValueError(f"could not derive local module identity for {module_path}")
    state: dict[str, str] = {}
    order: list[tuple[str, str]] = []

    def visit(module: str) -> None:
        status = state.get(module)
        if status == "done":
            return
        if status == "visiting":
            raise ValueError(f"local Lean import cycle at {module}")
        state[module] = "visiting"
        path = modules[module]
        for line in source_by_path[path].splitlines():
            command = line.split("--", 1)[0].strip()
            if not command.startswith("import "):
                continue
            for imported in command.removeprefix("import ").split():
                if imported in modules:
                    visit(imported)
        state[module] = "done"
        order.append((module, path))

    visit(target)
    return order


def _canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def platform_contract_id() -> str:
    return f"{platform.system().lower()}-{platform.machine().lower()}"


def lean_environment_path_key(relative: str) -> str:
    return unicodedata.normalize("NFC", relative).casefold()


def lean_environment_contract_id(contract_facts: dict[str, Any]) -> str:
    if "contract_id" in contract_facts or "environment_root" in contract_facts:
        raise ValueError(
            "environment contract identity facts must not contain derived fields"
        )
    return "lean-env-" + sha256_bytes(_canonical_json_bytes(contract_facts))[:24]


def validate_environment_contract(contract: Any) -> dict[str, Any]:
    if not isinstance(contract, dict):
        raise ValueError("Lean environment contract must contain a JSON object")
    if (
        contract.get("schema") != "vela.lean_environment_contract.v1"
        or contract.get("tree_algorithm") != "vela.lean_environment_tree.v1"
        or not CONTENT_HASH.fullmatch(str(contract.get("environment_root", "")))
    ):
        raise ValueError("Lean environment contract has invalid identity fields")
    facts_with_id = {
        key: value for key, value in contract.items() if key != "environment_root"
    }
    contract_facts = {
        key: value for key, value in facts_with_id.items() if key != "contract_id"
    }
    if contract.get("contract_id") != lean_environment_contract_id(contract_facts):
        raise ValueError("Lean environment contract ID is not content-derived")
    expected_root = "sha256:" + sha256_bytes(_canonical_json_bytes(facts_with_id))
    if contract["environment_root"] != expected_root:
        raise ValueError("Lean environment contract root is not content-derived")
    return contract


def lean_environment_tree_root(root: Path) -> str:
    """Measure stable F/L records; directories are traversed but intentionally omitted."""

    def stable_identity(fact: os.stat_result) -> tuple[int, int, int, int, int, int]:
        return (
            fact.st_dev,
            fact.st_ino,
            fact.st_mode,
            fact.st_size,
            fact.st_mtime_ns,
            fact.st_ctime_ns,
        )

    def utf8(value: str, label: str) -> bytes:
        try:
            return value.encode("utf-8")
        except UnicodeEncodeError as exc:
            raise ValueError(f"Lean environment tree has a non-UTF-8 {label}") from exc

    directory_flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        directory_flags |= os.O_DIRECTORY
    if hasattr(os, "O_CLOEXEC"):
        directory_flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    file_flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        file_flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        file_flags |= os.O_NOFOLLOW

    root = Path(root)
    try:
        root_descriptor = os.open(root, directory_flags)
    except OSError as exc:
        raise ValueError(
            f"Lean environment tree is not a real directory: {root}"
        ) from exc
    records: list[tuple[bytes, bytes]] = []
    casefolded: set[str] = set()
    resolved_root = root.resolve(strict=True)

    def walk(directory_descriptor: int, prefix: str) -> None:
        try:
            names = os.listdir(directory_descriptor)
        except OSError as exc:
            raise ValueError(
                f"could not enumerate Lean environment tree: {prefix or '.'}"
            ) from exc
        encoded_names = [(utf8(name, "path"), name) for name in names]
        for _encoded_name, name in sorted(encoded_names):
            relative = f"{prefix}/{name}" if prefix else name
            relative_bytes = utf8(relative, "path")
            folded = lean_environment_path_key(relative)
            if folded in casefolded:
                raise ValueError(
                    f"Lean environment tree has a case-colliding path: {relative}"
                )
            casefolded.add(folded)
            before = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
            if stat.S_ISLNK(before.st_mode):
                target = os.readlink(name, dir_fd=directory_descriptor)
                target_bytes = utf8(target, "symlink target")
                if target.startswith("/"):
                    raise ValueError(
                        f"Lean environment tree has an absolute symlink: {relative}"
                    )
                lexical_target = posixpath.normpath(
                    posixpath.join(posixpath.dirname(relative), target)
                )
                if lexical_target == ".." or lexical_target.startswith("../"):
                    raise ValueError(
                        f"Lean environment tree symlink escapes: {relative}"
                    )
                try:
                    (resolved_root / relative).resolve(strict=True).relative_to(
                        resolved_root
                    )
                except (FileNotFoundError, ValueError) as exc:
                    raise ValueError(
                        f"Lean environment tree symlink escapes or dangles: {relative}"
                    ) from exc
                after = os.stat(
                    name, dir_fd=directory_descriptor, follow_symlinks=False
                )
                if (
                    stable_identity(before) != stable_identity(after)
                    or os.readlink(name, dir_fd=directory_descriptor) != target
                ):
                    raise ValueError(
                        f"Lean environment tree changed while measuring: {relative}"
                    )
                records.append(
                    (
                        relative_bytes,
                        b"L\0" + relative_bytes + b"\0" + target_bytes + b"\0",
                    )
                )
            elif stat.S_ISREG(before.st_mode):
                try:
                    descriptor = os.open(name, file_flags, dir_fd=directory_descriptor)
                except OSError as exc:
                    raise ValueError(
                        f"could not open Lean environment file safely: {relative}"
                    ) from exc
                try:
                    opened = os.fstat(descriptor)
                    if not stat.S_ISREG(opened.st_mode) or stable_identity(
                        before
                    ) != stable_identity(opened):
                        raise ValueError(
                            f"Lean environment entry changed type: {relative}"
                        )
                    file_digest = hashlib.sha256()
                    while block := os.read(descriptor, 1024 * 1024):
                        file_digest.update(block)
                    finished = os.fstat(descriptor)
                    after = os.stat(
                        name, dir_fd=directory_descriptor, follow_symlinks=False
                    )
                    if stable_identity(opened) != stable_identity(
                        finished
                    ) or stable_identity(opened) != stable_identity(after):
                        raise ValueError(
                            f"Lean environment file changed while hashing: {relative}"
                        )
                    executable = b"1" if opened.st_mode & 0o111 else b"0"
                    records.append(
                        (
                            relative_bytes,
                            b"F\0"
                            + relative_bytes
                            + b"\0"
                            + executable
                            + b"\0"
                            + file_digest.hexdigest().encode("ascii")
                            + b"\0",
                        )
                    )
                finally:
                    os.close(descriptor)
            elif stat.S_ISDIR(before.st_mode):
                try:
                    child_descriptor = os.open(
                        name, directory_flags, dir_fd=directory_descriptor
                    )
                except OSError as exc:
                    raise ValueError(
                        f"could not open Lean environment directory safely: {relative}"
                    ) from exc
                try:
                    opened = os.fstat(child_descriptor)
                    if not stat.S_ISDIR(opened.st_mode) or stable_identity(
                        before
                    ) != stable_identity(opened):
                        raise ValueError(
                            f"Lean environment entry changed type: {relative}"
                        )
                    walk(child_descriptor, relative)
                    finished = os.fstat(child_descriptor)
                    after = os.stat(
                        name, dir_fd=directory_descriptor, follow_symlinks=False
                    )
                    if stable_identity(opened) != stable_identity(
                        finished
                    ) or stable_identity(opened) != stable_identity(after):
                        raise ValueError(
                            f"Lean environment directory changed while measuring: {relative}"
                        )
                finally:
                    os.close(child_descriptor)
            else:
                raise ValueError(
                    f"Lean environment tree contains a special file: {relative}"
                )
        if sorted(names, key=lambda item: utf8(item, "path")) != sorted(
            os.listdir(directory_descriptor), key=lambda item: utf8(item, "path")
        ):
            raise ValueError(
                f"Lean environment directory changed while measuring: {prefix or '.'}"
            )

    try:
        root_fact = os.fstat(root_descriptor)
        if not stat.S_ISDIR(root_fact.st_mode):
            raise ValueError(f"Lean environment tree is not a directory: {root}")
        walk(root_descriptor, "")
        if stable_identity(root_fact) != stable_identity(os.fstat(root_descriptor)):
            raise ValueError("Lean environment root changed while measuring")
    finally:
        os.close(root_descriptor)
    digest = hashlib.sha256(b"vela.lean_environment_tree.v1\0")
    for _path, record in sorted(records, key=lambda item: item[0]):
        digest.update(record)
    return "sha256:" + digest.hexdigest()


def measure_diderot_environment(
    packet: dict[str, Any],
    environment_root: Path,
) -> tuple[dict[str, Any], list[Path]]:
    toolchain = packet["toolchain"]
    mathlib_commit = packet["mathlib_commit"]
    manifest_path = environment_root / "lake-manifest.json"
    manifest_bytes = read_regular_file(manifest_path, max_bytes=MAX_MANIFEST_BYTES)
    manifest = json.loads(manifest_bytes)
    if not isinstance(manifest, dict) or not isinstance(manifest.get("packages"), list):
        raise ValueError("preprovisioned environment manifest is invalid")
    recorded_toolchain = (
        read_regular_file(
            environment_root / "lean-toolchain",
            max_bytes=256,
        )
        .decode("utf-8")
        .strip()
    )
    if recorded_toolchain != toolchain:
        raise ValueError("preprovisioned environment toolchain drift")
    dependency_root = environment_root / ".lake" / "packages"
    if not dependency_root.is_dir() or dependency_root.is_symlink():
        raise ValueError("preprovisioned dependency root is unavailable or aliased")
    packages: list[dict[str, Any]] = []
    runtime_paths: list[Path] = []
    seen: set[str] = set()
    for package in manifest["packages"]:
        if not isinstance(package, dict):
            raise ValueError("preprovisioned manifest package is not an object")
        name = package.get("name")
        url = package.get("url")
        revision = package.get("rev")
        if (
            not isinstance(name, str)
            or not PACKAGE_NAME.fullmatch(name)
            or name.startswith(".")
            or name.casefold() in seen
        ):
            raise ValueError("preprovisioned manifest contains an unsafe package name")
        seen.add(name.casefold())
        url = canonical_github_repo_url(url, f"preprovisioned package {name} URL")
        if not isinstance(revision, str) or not FULL_COMMIT.fullmatch(revision):
            raise ValueError(f"preprovisioned package is not pinned: {name}")
        package_dir = _package_directory(dependency_root, name)
        head = git_checked(
            ["-C", str(package_dir), "rev-parse", "HEAD"], dependency_root
        ).stdout.strip()
        tree = git_checked(
            ["-C", str(package_dir), "rev-parse", "HEAD^{tree}"],
            dependency_root,
        ).stdout.strip()
        remote = git_checked(
            ["-C", str(package_dir), "remote", "get-url", "origin"],
            dependency_root,
        ).stdout.strip()
        dirty = git_checked(
            [
                "-C",
                str(package_dir),
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
            ],
            dependency_root,
        ).stdout.strip()
        if (
            head != revision
            or canonical_github_repo_url(
                remote,
                f"preprovisioned package {name} remote",
            )
            != url
            or dirty
        ):
            raise ValueError(f"preprovisioned package checkout drift: {name}")
        runtime = package_dir / ".lake" / "build" / "lib" / "lean"
        runtime_relative: str | None = None
        runtime_root: str | None = None
        if runtime.is_dir() and not runtime.is_symlink():
            runtime_relative = f"{name}/.lake/build/lib/lean"
            runtime_root = lean_environment_tree_root(runtime)
            runtime_paths.append(runtime.resolve())
        packages.append(
            {
                "name": name,
                "url": url,
                "revision": revision,
                "git_tree": tree,
                "runtime_path": runtime_relative,
                "runtime_root": runtime_root,
                "runtime_granted": runtime_root is not None,
            }
        )
    mathlib = next((item for item in packages if item["name"] == "mathlib"), None)
    if mathlib is None or mathlib["revision"] != mathlib_commit:
        raise ValueError("preprovisioned Mathlib revision drift")
    if not runtime_paths:
        raise ValueError(
            "preprovisioned environment has no sealed Lean runtime artifacts"
        )
    toolchain_path = toolchain_root(toolchain, allow_install=False)
    source_repo_url = canonical_github_repo_url(
        packet.get("source_repo_url"),
        "packet source_repo_url",
    )
    lean_path_order = [
        "controlled_workspace",
        *[
            f"packages/{package['runtime_path']}"
            for package in packages
            if package["runtime_granted"]
        ],
    ]
    contract_facts = {
        "schema": "vela.lean_environment_contract.v1",
        "tree_algorithm": "vela.lean_environment_tree.v1",
        "platform": platform_contract_id(),
        "source_repo": packet["source_repo"],
        "source_repo_url": source_repo_url,
        "source_commit": packet["source_commit"],
        "source_provenance": "historical_assertion_unverified_git_linkage",
        "toolchain": {
            "id": toolchain,
            "measured_root": lean_environment_tree_root(toolchain_path),
            "read_surface": ".",
            "allowed_executable": "bin/lean",
        },
        "lake_manifest_sha256": sha256_bytes(manifest_bytes),
        "mathlib_commit": mathlib_commit,
        "packages": packages,
        "runtime_boundary": {
            "profile": "vela.external_lean_sandbox.macos.v1",
            "backend": "sandbox-exec",
            "sandbox_driver_sha256": sha256_file(SANDBOX_DRIVER),
            "allowed_executables": ["toolchain/bin/lean"],
            "read_surfaces": [
                "controlled_source",
                "toolchain",
                *lean_path_order[1:],
            ],
            "write_surfaces": ["controlled_workspace"],
            "lean_path_order": lean_path_order,
            "network": "denied",
            "environment": "allowlisted_and_scrubbed",
            "limit_ceilings": dict(_SANDBOX.DEFAULT_LIMITS),
        },
    }
    candidate = {
        **contract_facts,
        "contract_id": lean_environment_contract_id(contract_facts),
    }
    candidate["environment_root"] = "sha256:" + sha256_bytes(
        _canonical_json_bytes(candidate)
    )
    return candidate, runtime_paths


def approved_diderot_environment(
    packet: dict[str, Any],
    environment_root: Path,
    lineage: dict[str, Any],
) -> tuple[dict[str, Any], list[Path]]:
    entry = lineage["registry_entry"]
    index_path = ROOT / "verifiers" / "environments" / "lean" / "index.v1.json"
    index_ref = entry.get("environment_contract_index")
    if not isinstance(index_ref, dict) or index_ref.get("path") != str(
        index_path.relative_to(ROOT)
    ):
        raise ValueError("registry does not bind the Lean environment contract index")
    index_sha256 = index_ref.get("sha256")
    if not isinstance(index_sha256, str) or not re.fullmatch(
        r"[a-f0-9]{64}", index_sha256
    ):
        raise ValueError(
            "Lean environment contract index binding has an invalid digest"
        )
    index_bytes = read_regular_file(index_path, max_bytes=MAX_MANIFEST_BYTES)
    if sha256_bytes(index_bytes) != index_sha256:
        raise ValueError("Lean environment contract index digest mismatch")
    index = json.loads(index_bytes)
    bindings = index.get("bindings")
    if index.get("schema") != "vela.lean_environment_index.v1" or not isinstance(
        bindings, list
    ):
        raise ValueError("Lean environment contract index is invalid")
    binding = next(
        (
            item
            for item in bindings
            if isinstance(item, dict)
            and packet.get("claim_id") in item.get("claim_ids", [])
            and item.get("source_repo") == packet.get("source_repo")
            and item.get("source_repo_url")
            == canonical_github_repo_url(
                packet.get("source_repo_url"),
                "packet source_repo_url",
            )
            and item.get("source_provenance")
            == "historical_assertion_unverified_git_linkage"
            and item.get("source_commit") == packet.get("source_commit")
            and item.get("toolchain") == packet.get("toolchain")
            and item.get("mathlib_commit") == packet.get("mathlib_commit")
            and item.get("platform") == platform_contract_id()
        ),
        None,
    )
    if binding is None:
        raise ValueError(
            "no approved Lean environment contract matches this replay packet"
        )
    contract_path = safe_artifact(ROOT, binding.get("contract_path"))
    contract_sha256 = binding.get("contract_sha256")
    if not isinstance(contract_sha256, str) or not re.fullmatch(
        r"[a-f0-9]{64}", contract_sha256
    ):
        raise ValueError("Lean environment contract binding has an invalid digest")
    contract_bytes = read_regular_file(contract_path, max_bytes=MAX_MANIFEST_BYTES)
    if sha256_bytes(contract_bytes) != contract_sha256:
        raise ValueError("approved Lean environment contract digest mismatch")
    contract = validate_environment_contract(json.loads(contract_bytes))
    actual, runtime_paths = measure_diderot_environment(packet, environment_root)
    if contract != actual:
        raise ValueError(
            "measured Lean environment does not match its approved contract"
        )
    approved = entry.get("environment_contracts") or []
    if not any(
        item.get("id") == contract.get("contract_id")
        and binding.get("contract_id") == contract.get("contract_id")
        and item.get("path") == binding.get("contract_path")
        and item.get("sha256") == contract_sha256
        for item in approved
        if isinstance(item, dict)
    ):
        raise ValueError(
            "registry does not bind the approved Lean environment contract"
        )
    lineage_contract = lineage.get("environment_contract")
    expected_contract_ref = {
        "id": contract.get("contract_id"),
        "path": binding.get("contract_path"),
        "sha256": contract_sha256,
    }
    if lineage_contract is not None and lineage_contract != expected_contract_ref:
        raise ValueError(
            "historical packet overlay selects a different environment contract"
        )
    return contract, runtime_paths


def environment_contract_candidate(
    packet_path: Path, bundle_root: Path | None = None
) -> dict[str, Any]:
    trusted_root = (bundle_root or ROOT).resolve(strict=True)
    _safe_packet, _packet_bytes, packet, _packet_digest = load_trusted_packet(
        packet_path,
        trusted_root,
    )
    required_strings = [
        "claim_id",
        "source_repo",
        "source_commit",
        "toolchain",
        "mathlib_commit",
    ]
    if packet.get("schema") != "vela.diderot_lean_replay.v1" or any(
        not isinstance(packet.get(key), str) for key in required_strings
    ):
        raise ValueError("packet cannot identify a Lean environment candidate")
    toolchain = packet["toolchain"]
    mathlib_commit = packet["mathlib_commit"]
    if not TOOLCHAIN.fullmatch(toolchain) or not FULL_COMMIT.fullmatch(mathlib_commit):
        raise ValueError("packet environment pins are invalid")
    workspace_key = sha256_bytes(
        f"{packet['source_repo']}\0{packet['source_commit']}\0{toolchain}\0{mathlib_commit}".encode(
            "utf-8"
        )
    )[:20]
    candidate, _runtime_paths = measure_diderot_environment(
        packet,
        ROOT / "envs" / "diderot-lean" / workspace_key,
    )
    return candidate


def verify(packet_path: Path, *, bundle_root: Path | None = None) -> dict[str, Any]:
    trusted_bundle_root = (bundle_root or ROOT).resolve(strict=True)
    packet_path, _packet_bytes, packet, packet_digest = load_trusted_packet(
        packet_path,
        trusted_bundle_root,
    )
    if packet.get("schema") != "vela.diderot_lean_replay.v1":
        raise ValueError("unsupported replay packet schema")
    declaration = validate_declaration(packet.get("declaration"))
    module_path = lean_source_path(packet.get("module_path"), "packet module_path")
    claim_id = validate_claim_id(packet.get("claim_id"))
    lineage = verifier_lineage(packet_path, packet, packet_digest)
    _artifact_root, manifest, files, source_contents = validate_source_manifest(
        packet_path,
        packet,
        trusted_bundle_root,
        legacy_artifact_root=lineage.get("legacy_artifact_root"),
    )
    if manifest.get("source_commit") != packet.get("source_commit"):
        raise ValueError("source commit drift between packet and manifest")
    for field in ("source_repo", "source_repo_url", "toolchain", "mathlib_commit"):
        if manifest.get(field) != packet.get(field):
            raise ValueError(f"{field} drift between packet and source manifest")
    source_artifact_records = [
        {
            "source_path": item["source_path"],
            "sha256": sha256_bytes(source_contents[item["source_path"]]),
            "bytes": len(source_contents[item["source_path"]]),
        }
        for item in sorted(files, key=lambda value: value["source_path"])
    ]
    source_artifact_root = "sha256:" + sha256_bytes(
        _canonical_json_bytes(source_artifact_records)
    )

    source_by_path: dict[str, str] = {}
    for item in files:
        if not item["source_path"].endswith(".lean"):
            continue
        try:
            source_by_path[item["source_path"]] = source_contents[
                item["source_path"]
            ].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ValueError(
                f"source artifact is not UTF-8: {item['source_path']}"
            ) from exc
    if module_path not in source_by_path:
        raise ValueError(f"module missing from source snapshot: {module_path}")
    audit = audit_declaration(
        source_by_path[module_path],
        declaration,
        list(source_by_path.values()),
    )
    if not audit["passed"]:
        raise ValueError(f"adversarial source audit failed: {audit['checks']}")
    if audit["statement_sha256"] != packet["statement_sha256"]:
        raise ValueError("statement digest drift")

    toolchain = packet["toolchain"]
    if not re.fullmatch(
        r"leanprover/lean4:v[0-9]+\.[0-9]+\.[0-9]+(?:-rc[0-9]+)?", toolchain
    ):
        raise ValueError(f"unfrozen Lean toolchain: {toolchain}")
    mathlib_commit = packet["mathlib_commit"]
    if not re.fullmatch(r"[a-f0-9]{40}", mathlib_commit):
        raise ValueError("mathlib commit must be a full Git SHA")

    workspace_key = sha256_bytes(
        f"{packet['source_repo']}\0{packet['source_commit']}\0{toolchain}\0{mathlib_commit}".encode(
            "utf-8"
        )
    )[:20]
    environment_root = ROOT / "envs" / "diderot-lean" / workspace_key
    manifest_source = environment_root / "lake-manifest.json"
    dependency_root = environment_root / ".lake" / "packages"
    if not manifest_source.is_file() or not dependency_root.is_dir():
        raise ValueError(
            "packet dependencies are not pre-provisioned; Vela refuses to run "
            "lake update or a dependency cache executable outside the OS sandbox"
        )
    environment_contract, dependency_lean_paths = approved_diderot_environment(
        packet,
        environment_root,
        lineage,
    )
    # The dependency environment is shared and immutable, but elaboration is
    # not. A relay, registry audit, and installed verifier may legitimately run
    # the same packet at once. Give every process private source and write
    # roots so one verifier can never remove or replace another verifier's
    # sandbox roots mid-execution.
    workspace_guard = tempfile.TemporaryDirectory(prefix="run-", dir=environment_root)
    source_guard = tempfile.TemporaryDirectory(prefix="source-", dir=environment_root)
    workspace = Path(workspace_guard.name)
    source_root = Path(source_guard.name)
    write_text(workspace / "lean-toolchain", toolchain + "\n")

    for item in files:
        if not item["source_path"].endswith(".lean"):
            continue
        destination = safe_output_path(source_root, item["source_path"])
        destination.parent.mkdir(parents=True, exist_ok=True)
        write_bytes_exclusive(
            destination, source_contents[item["source_path"]], mode=0o644
        )

    import_name = module_path.removesuffix(".lean").replace("/", ".")
    lean_path = os.pathsep.join(
        [str(workspace), *(str(path) for path in dependency_lean_paths)]
    )
    build_sandboxes: list[dict[str, Any]] = []
    build_logs: list[dict[str, Any]] = []
    for local_module, local_path in local_module_build_order(
        source_by_path, module_path
    ):
        olean_path = workspace / Path(*local_module.split(".")).with_suffix(".olean")
        olean_path.parent.mkdir(parents=True, exist_ok=True)
        build, build_sandbox = run_sandboxed_lean_command(
            [
                "lean",
                "-R",
                str(source_root),
                "-o",
                str(olean_path),
                str(source_root / local_path),
            ],
            workspace,
            timeout=1800,
            source_root=source_root,
            toolchain=toolchain,
            dependency_root=dependency_root,
            dependency_identity=environment_contract["environment_root"],
            lean_path=lean_path,
            dependency_roots=dependency_lean_paths,
        )
        build_sandboxes.append(build_sandbox)
        build_logs.append(
            {
                "module": local_module,
                "source_path": local_path,
                "stdout": build.stdout,
                "stderr": build.stderr,
            }
        )
        if build.returncode != 0:
            raise ValueError(
                f"Lean build failed for local module {local_module}: {build.stderr or build.stdout}"
            )

    audit_source = (
        f"import {import_name}\n\n"
        f"#check {declaration}\n"
        f"#print axioms {declaration}\n"
    )
    write_text(workspace / "_VelaDiderotAudit.lean", audit_source)
    lean, audit_sandbox = run_sandboxed_lean_command(
        ["lean", "-R", str(workspace), "_VelaDiderotAudit.lean"],
        workspace,
        timeout=600,
        source_root=source_root,
        toolchain=toolchain,
        dependency_root=dependency_root,
        dependency_identity=environment_contract["environment_root"],
        lean_path=lean_path,
        dependency_roots=dependency_lean_paths,
    )
    combined = (lean.stdout or "") + "\n" + (lean.stderr or "")
    if lean.returncode != 0:
        raise ValueError(f"Lean declaration audit failed: {combined}")
    if audit_sandbox["stdout"]["truncated"] or audit_sandbox["stderr"]["truncated"]:
        raise ValueError(
            "Lean declaration audit output was truncated before its trust result: "
            f"stdout={lean.stdout!r}; stderr={lean.stderr!r}"
        )
    try:
        axioms = parse_axioms(combined, declaration)
    except ValueError as exc:
        raise ValueError(
            f"{exc}; bounded audit stdout={lean.stdout!r}; stderr={lean.stderr!r}"
        ) from exc
    forbidden_axioms = sorted(set(axioms) - ALLOWED_AXIOMS)
    if forbidden_axioms:
        raise ValueError(f"declaration depends on forbidden axioms: {forbidden_axioms}")
    if "sorryAx" in combined:
        raise ValueError("Lean output contains sorryAx")

    log = {
        "local_module_builds": build_logs,
        "audit_stdout": lean.stdout,
        "audit_stderr": lean.stderr,
        "sandbox_executions": [*build_sandboxes, audit_sandbox],
    }
    log_path = environment_root / (
        f"verification-{sha256_bytes(claim_id.encode('utf-8'))[:24]}-"
        f"{os.getpid()}.json"
    )
    log_path.write_text(
        json.dumps(log, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    result = {
        "ok": True,
        "strength": "rederived",
        "verifier_id": lineage["executed_verifier"]["id"],
        "packet_sha256": packet_digest,
        "packet_verifier": lineage["historical_verifier"],
        "executed_verifier": lineage["executed_verifier"],
        "verifier_lineage": {
            key: value
            for key, value in lineage.items()
            if key not in {"registry_entry", "input_packet_path"}
        },
        "claim_id": claim_id,
        "declaration": declaration,
        "source_repo": packet["source_repo"],
        "source_commit": packet["source_commit"],
        "source_provenance": {
            "repository_url_assertion": packet["source_repo_url"],
            "commit_assertion": packet["source_commit"],
            "git_commit_tree_blob_linkage": "historical_assertion_unverified",
            "checked_artifact_manifest_sha256": packet["source_manifest"]["sha256"],
            "checked_artifact_root": source_artifact_root,
            "checked_file_count": len(source_artifact_records),
            "scope": "Lean kernel re-derived the exact content-addressed artifact bytes",
        },
        "toolchain": toolchain,
        "mathlib_commit": mathlib_commit,
        "environment_contract": {
            "id": environment_contract["contract_id"],
            "root": environment_contract["environment_root"],
        },
        "statement": audit["statement"],
        "statement_sha256": audit["statement_sha256"],
        "zero_drift": True,
        "axioms": axioms,
        "audit": audit,
        "workspace_key": workspace_key,
        "verifier_log": str(log_path),
        "sandbox": {
            "driver_root": "sha256:" + sha256_file(SANDBOX_DRIVER),
            "executions": [*build_sandboxes, audit_sandbox],
            "all_fail_closed": all(
                sandbox_execution_accepted(record)
                for record in [*build_sandboxes, audit_sandbox]
            ),
        },
        "message": (
            f"Lean kernel re-derived {declaration} from exact artifact bytes with axioms "
            f"{axioms}; repository commit linkage remains a historical assertion"
        ),
    }
    workspace_guard.cleanup()
    source_guard.cleanup()
    return result


def git_checked(
    command: list[str], cwd: Path, timeout: int = 300
) -> subprocess.CompletedProcess[str]:
    proc = run_trusted_command(
        ["/usr/bin/git", "-c", "core.hooksPath=/dev/null", *command],
        cwd,
        timeout,
    )
    if proc.returncode != 0:
        raise ValueError((proc.stderr or proc.stdout or "git command failed").strip())
    return proc


def read_git_blob(
    git_dir: Path,
    commit: str,
    source_path: str,
    *,
    max_bytes: int = MAX_SOURCE_FILE_BYTES,
) -> bytes:
    size = run_bytes(
        [
            "/usr/bin/git",
            "-c",
            "core.hooksPath=/dev/null",
            f"--git-dir={git_dir}",
            "cat-file",
            "-s",
            f"{commit}:{source_path}",
        ],
        git_dir.parent,
        120,
    )
    if size.returncode != 0:
        raise ValueError(size.stderr.decode("utf-8", errors="replace").strip())
    try:
        declared_size = int(size.stdout.strip())
    except ValueError as exc:
        raise ValueError(
            f"Git returned an invalid blob size for {source_path}"
        ) from exc
    if declared_size < 0 or declared_size > max_bytes:
        raise ValueError(f"Git blob exceeds {max_bytes} bytes: {source_path}")
    proc = run_bytes(
        [
            "/usr/bin/git",
            "-c",
            "core.hooksPath=/dev/null",
            f"--git-dir={git_dir}",
            "show",
            f"{commit}:{source_path}",
        ],
        git_dir.parent,
        120,
    )
    if proc.returncode != 0:
        raise ValueError(proc.stderr.decode("utf-8", errors="replace").strip())
    if len(proc.stdout) != declared_size:
        raise ValueError(f"Git blob size changed while reading: {source_path}")
    return proc.stdout


def fetch_commit_read_only(repo_url: str, commit: str, git_dir: Path) -> dict[str, Any]:
    local_fixture = (
        repo_url.startswith("file://")
        and os.environ.get("VELA_ALLOW_LOCAL_EXTERNAL_LEAN_FIXTURE") == "1"
    )
    if not local_fixture and not GITHUB_REPO_URL.fullmatch(repo_url):
        raise ValueError("repo URL must be a canonical public GitHub HTTPS repository")
    if not FULL_COMMIT.fullmatch(commit):
        raise ValueError("commit must be a full lowercase Git SHA")
    git_dir.parent.mkdir(parents=True, exist_ok=True)
    if not git_dir.is_dir():
        init = run_trusted_command(
            ["/usr/bin/git", "init", "--bare", str(git_dir)],
            git_dir.parent,
            120,
        )
        if init.returncode != 0:
            raise ValueError((init.stderr or init.stdout).strip())
    remotes = git_checked(
        [f"--git-dir={git_dir}", "remote"], git_dir.parent
    ).stdout.split()
    if "origin" in remotes:
        git_checked(
            [f"--git-dir={git_dir}", "remote", "set-url", "origin", repo_url],
            git_dir.parent,
        )
    else:
        git_checked(
            [f"--git-dir={git_dir}", "remote", "add", "origin", repo_url],
            git_dir.parent,
        )
    git_checked(
        [
            "-c",
            f"protocol.file.allow={'always' if local_fixture else 'never'}",
            "-c",
            "http.followRedirects=false",
            f"--git-dir={git_dir}",
            "fetch",
            "--no-tags",
            "--depth=1",
            "origin",
            commit,
        ],
        git_dir.parent,
        timeout=900,
    )
    resolved = git_checked(
        [f"--git-dir={git_dir}", "rev-parse", "FETCH_HEAD^{commit}"],
        git_dir.parent,
    ).stdout.strip()
    if resolved != commit:
        raise ValueError(f"fetched commit drift: expected {commit}, got {resolved}")
    committed_at = git_checked(
        [f"--git-dir={git_dir}", "show", "-s", "--format=%cI", commit],
        git_dir.parent,
    ).stdout.strip()
    tree = run_bytes(
        [
            "/usr/bin/git",
            "-c",
            "core.hooksPath=/dev/null",
            f"--git-dir={git_dir}",
            "ls-tree",
            "-r",
            "-z",
            "--name-only",
            commit,
        ],
        git_dir.parent,
        120,
    )
    if tree.returncode != 0:
        raise ValueError(tree.stderr.decode("utf-8", errors="replace").strip())
    paths = [item.decode("utf-8") for item in tree.stdout.split(b"\0") if item]
    return {"resolved_commit": resolved, "committed_at": committed_at, "paths": paths}


def resolve_frozen_pins(git_dir: Path, commit: str, paths: list[str]) -> dict[str, Any]:
    toolchains = sorted(
        (path for path in paths if path.endswith("lean-toolchain")),
        key=lambda p: (p.count("/"), p),
    )
    manifests = set(path for path in paths if path.endswith("lake-manifest.json"))
    if not toolchains or not manifests:
        raise ValueError(
            "repository does not expose lean-toolchain and lake-manifest.json at the pinned commit"
        )
    selected_toolchain = None
    selected_manifest = None
    for toolchain_path in toolchains:
        parent = toolchain_path.rsplit("/", 1)[0] if "/" in toolchain_path else ""
        candidate = f"{parent}/lake-manifest.json" if parent else "lake-manifest.json"
        if candidate in manifests:
            selected_toolchain = toolchain_path
            selected_manifest = candidate
            break
    if selected_toolchain is None or selected_manifest is None:
        selected_toolchain = toolchains[0]
        selected_manifest = sorted(manifests, key=lambda p: (p.count("/"), p))[0]
    safe_relative_path(selected_toolchain, "lean-toolchain Git path")
    safe_relative_path(selected_manifest, "lake-manifest.json Git path")
    toolchain_bytes = read_git_blob(git_dir, commit, selected_toolchain, max_bytes=256)
    toolchain = toolchain_bytes.decode("utf-8").strip()
    if not TOOLCHAIN.fullmatch(toolchain):
        raise ValueError(f"unfrozen or unsupported Lean toolchain: {toolchain}")
    manifest_bytes = read_git_blob(
        git_dir,
        commit,
        selected_manifest,
        max_bytes=MAX_MANIFEST_BYTES,
    )
    try:
        manifest = json.loads(manifest_bytes)
    except json.JSONDecodeError as exc:
        raise ValueError("lake-manifest.json is not valid JSON") from exc
    if not isinstance(manifest, dict) or not isinstance(manifest.get("packages"), list):
        raise ValueError("lake-manifest.json must contain a package array")
    mathlib = next(
        (
            item
            for item in manifest["packages"]
            if isinstance(item, dict) and item.get("name") == "mathlib"
        ),
        None,
    )
    mathlib_commit = mathlib.get("rev") if isinstance(mathlib, dict) else None
    if mathlib_commit is not None and (
        not isinstance(mathlib_commit, str) or not FULL_COMMIT.fullmatch(mathlib_commit)
    ):
        raise ValueError("lake-manifest.json does not pin Mathlib to a full Git SHA")
    mathlib_url = mathlib.get("url") if isinstance(mathlib, dict) else None
    mathlib_input_revision = (
        mathlib.get("inputRev") if isinstance(mathlib, dict) else None
    )
    if mathlib is not None and (
        not isinstance(mathlib_url, str) or not GITHUB_REPO_URL.fullmatch(mathlib_url)
    ):
        raise ValueError("lake-manifest.json has invalid Mathlib source identity")
    packages: list[dict[str, str]] = []
    normalized_packages: list[dict[str, Any]] = []
    names: set[str] = set()
    for package in manifest["packages"]:
        if not isinstance(package, dict) or package.get("type") != "git":
            raise ValueError("lake-manifest.json contains a non-Git dependency")
        name = package.get("name")
        url = package.get("url")
        revision = package.get("rev")
        input_revision = package.get("inputRev")
        subdirectory = package.get("subDir")
        if (
            not isinstance(name, str)
            or not PACKAGE_NAME.fullmatch(name)
            or name in {".", "..", ".git"}
            or name.startswith(".")
        ):
            raise ValueError("lake-manifest.json contains an unsafe dependency name")
        folded = name.casefold()
        if folded in names:
            raise ValueError(
                f"lake-manifest.json contains a duplicate dependency name: {name}"
            )
        names.add(folded)
        if not isinstance(url, str) or not GITHUB_REPO_URL.fullmatch(url):
            raise ValueError(
                f"dependency {name} does not use a public GitHub HTTPS URL"
            )
        if not isinstance(revision, str) or not FULL_COMMIT.fullmatch(revision):
            raise ValueError(f"dependency {name} is not pinned to a full Git SHA")
        safe_git_input_revision(input_revision, f"dependency {name} inputRev")
        if subdirectory not in {None, ""}:
            raise ValueError(f"dependency {name} uses an unsupported subdirectory")
        config_file = package.get("configFile")
        manifest_file = package.get("manifestFile")
        inherited = package.get("inherited")
        scope = package.get("scope")
        if config_file not in {"lakefile.lean", "lakefile.toml"}:
            raise ValueError(f"dependency {name} has an unsafe config file")
        if manifest_file != "lake-manifest.json":
            raise ValueError(f"dependency {name} has an unsafe manifest file")
        if (
            not isinstance(inherited, bool)
            or not isinstance(scope, str)
            or len(scope) > 128
            or (scope and not PACKAGE_NAME.fullmatch(scope))
        ):
            raise ValueError(f"dependency {name} has unsupported manifest metadata")
        packages.append({"name": name, "url": url, "rev": revision})
        normalized_packages.append(
            {
                "url": url,
                "type": "git",
                "subDir": None,
                "scope": scope,
                "rev": revision,
                "name": name,
                "manifestFile": "lake-manifest.json",
                "inputRev": revision,
                "inherited": inherited,
                "configFile": config_file,
            }
        )
    normalized_manifest = {
        "version": "1.1.0",
        "packagesDir": ".lake/packages",
        "packages": normalized_packages,
        "name": "VelaExternalLeanReplay",
        "lakeDir": ".lake",
    }
    return {
        "toolchain": toolchain,
        "toolchain_path": selected_toolchain,
        "toolchain_sha256": sha256_bytes(toolchain_bytes),
        "mathlib_commit": mathlib_commit,
        "mathlib_url": mathlib_url,
        "mathlib_input_revision": mathlib_input_revision,
        "manifest_path": selected_manifest,
        "manifest_sha256": sha256_bytes(manifest_bytes),
        "manifest": normalized_manifest,
        "packages": packages,
    }


def snapshot_lean_sources(
    git_dir: Path,
    repo_url: str,
    commit: str,
    paths: list[str],
    source_dir: Path,
) -> tuple[dict[str, str], dict[str, Any]]:
    lean_paths = sorted(
        lean_source_path(path)
        for path in paths
        if path.endswith(".lean") and "/.lake/" not in f"/{path}"
    )
    if len({path.casefold() for path in lean_paths}) != len(lean_paths):
        raise ValueError("repository contains case-colliding Lean source paths")
    if not lean_paths:
        raise ValueError("repository commit contains no Lean source files")
    if len(lean_paths) > MAX_LEAN_FILES:
        raise ValueError(
            f"repository contains {len(lean_paths)} Lean files; limit is {MAX_LEAN_FILES}"
        )
    source_by_path: dict[str, str] = {}
    files: list[dict[str, Any]] = []
    total_bytes = 0
    for source_path in lean_paths:
        blob = read_git_blob(git_dir, commit, source_path)
        total_bytes += len(blob)
        if total_bytes > MAX_SOURCE_BYTES:
            raise ValueError(f"Lean source snapshot exceeds {MAX_SOURCE_BYTES} bytes")
        try:
            source = blob.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ValueError(f"Lean source is not UTF-8: {source_path}") from exc
        destination = safe_output_path(source_dir, source_path)
        destination.parent.mkdir(parents=True, exist_ok=True)
        write_bytes_exclusive(destination, blob, mode=0o644)
        digest = sha256_bytes(blob)
        source_by_path[source_path] = source
        files.append(
            {
                "source_path": source_path,
                "sha256": digest,
                "bytes": len(blob),
                "source_uri": f"{repo_url.removesuffix('.git')}/blob/{commit}/{source_path}",
            }
        )
    manifest = {
        "schema": "vela.external_lean_source_manifest.v1",
        "source_repo_url": repo_url,
        "source_commit": commit,
        "external_code_execution": False,
        "files": files,
        "file_count": len(files),
        "total_bytes": total_bytes,
    }
    return source_by_path, manifest


def find_declaration_source(
    source_by_path: dict[str, str],
    declaration: str,
    expected_path: str | None = None,
) -> tuple[str, str]:
    if expected_path is not None:
        source = source_by_path.get(expected_path)
        if source is None:
            raise ValueError(
                f"recorded declaration source path is missing: {expected_path}"
            )
        declaration_block(source, declaration)
        return expected_path, source
    matches: list[tuple[str, str]] = []
    for source_path, source in source_by_path.items():
        try:
            declaration_block(source, declaration)
        except ValueError:
            continue
        matches.append((source_path, source))
    if not matches:
        raise ValueError(
            f"declaration {declaration} was not found in commit-pinned Lean sources"
        )
    if len(matches) > 1:
        namespace_prefix = declaration.rsplit(".", 1)[0] if "." in declaration else ""
        qualified = []
        for path, source in matches:
            namespaces = re.findall(r"(?m)^namespace\s+([A-Za-z0-9_'.]+)\s*$", source)
            flattened = []
            for namespace in namespaces:
                flattened.extend(namespace.split("."))
            target = namespace_prefix.split(".") if namespace_prefix else []
            exact_namespace = namespace_prefix in namespaces
            ordered_namespace = bool(target) and any(
                flattened[index : index + len(target)] == target
                for index in range(max(0, len(flattened) - len(target) + 1))
            )
            if exact_namespace or ordered_namespace:
                qualified.append((path, source))
        if len(qualified) == 1:
            return qualified[0]
        clean = [
            (path, source)
            for path, source in matches
            if audit_declaration(source, declaration, [source])["passed"]
        ]
        if len(clean) == 1:
            return clean[0]
        paths = ", ".join(path for path, _ in (clean or matches)[:5])
        raise ValueError(
            f"declaration short name is ambiguous across source files: {paths}"
        )
    return matches[0]


def source_roots(source_paths: list[str], source_root: str) -> list[str]:
    root_parts = Path(source_root).parts[1:]
    roots: set[str] = set()
    for source_path in source_paths:
        parts = Path(source_path).with_suffix("").parts
        if tuple(parts[: len(root_parts)]) != root_parts or len(parts) <= len(
            root_parts
        ):
            continue
        root = parts[len(root_parts)]
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_']*", root):
            roots.add(root)
    return sorted(roots)


def module_candidates(
    source_path: str, source_paths: list[str]
) -> list[dict[str, Any]]:
    parts = Path(source_path).with_suffix("").parts
    preferred = (
        [1, 0] if parts and parts[0] in COMMON_SOURCE_ROOTS and len(parts) > 1 else [0]
    )
    preferred.extend(index for index in range(1, len(parts)) if index not in preferred)
    candidates = []
    for split in preferred:
        module_parts = parts[split:]
        if not module_parts or any(
            not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_']*", part) for part in module_parts
        ):
            continue
        source_root = Path("source", *parts[:split]).as_posix()
        module = ".".join(module_parts)
        source_root = Path("source", *parts[:split]).as_posix()
        candidates.append(
            {
                "source_root": source_root,
                "module": module,
                "roots": source_roots(source_paths, source_root),
            }
        )
    return candidates


def controlled_pull_lakefile(
    mathlib_url: str | None,
    mathlib_commit: str | None,
    candidate: dict[str, Any],
) -> str:
    source_root = candidate.get("source_root")
    roots_value = candidate.get("roots")
    try:
        source_parts = safe_relative_path(source_root, "controlled Lean source root")
    except ValueError as exc:
        raise ValueError("controlled Lean source layout is not portable") from exc
    if (
        source_parts[0] != "source"
        or any(not LEAN_NAME.fullmatch(part) for part in source_parts[1:])
        or not isinstance(roots_value, list)
        or any(
            not isinstance(root, str) or not LEAN_NAME.fullmatch(root)
            for root in roots_value
        )
    ):
        raise ValueError("controlled Lean source layout is not portable")
    serialized_source_root = "../" + "/".join(source_parts)
    if (mathlib_url is None) != (mathlib_commit is None) or (
        mathlib_url is not None
        and (
            not GITHUB_REPO_URL.fullmatch(mathlib_url)
            or not FULL_COMMIT.fullmatch(mathlib_commit or "")
        )
    ):
        raise ValueError("controlled Mathlib dependency identity is invalid")
    roots = ", ".join(json.dumps(root, ensure_ascii=True) for root in roots_value)
    dependency = (
        f"""\n[[require]]
name = "mathlib"
git = {json.dumps(mathlib_url, ensure_ascii=True)}
rev = {json.dumps(mathlib_commit, ensure_ascii=True)}
"""
        if mathlib_url and mathlib_commit
        else ""
    )
    return f"""name = "VelaExternalLeanReplay"
version = "1.0.0"
{dependency}

[[lean_lib]]
name = "VelaExternalSources"
srcDir = {json.dumps(serialized_source_root, ensure_ascii=True)}
roots = [{roots}]
"""


def controlled_cache_lakefile(
    mathlib_url: str | None,
    mathlib_commit: str | None,
) -> str:
    if (mathlib_url is None) != (mathlib_commit is None) or (
        mathlib_url is not None
        and (
            not GITHUB_REPO_URL.fullmatch(mathlib_url)
            or not FULL_COMMIT.fullmatch(mathlib_commit or "")
        )
    ):
        raise ValueError("controlled Mathlib dependency identity is invalid")
    dependency = (
        f"""\n[[require]]
name = "mathlib"
git = {json.dumps(mathlib_url, ensure_ascii=True)}
rev = {json.dumps(mathlib_commit, ensure_ascii=True)}
"""
        if mathlib_url and mathlib_commit
        else ""
    )
    return f"""name = "VelaExternalLeanCache"
version = "1.0.0"
{dependency}
"""


def directory_bytes(path: Path) -> int:
    total = 0
    if not path.exists():
        return total
    for item in path.rglob("*"):
        try:
            if item.is_file() and not item.is_symlink():
                total += item.stat().st_size
        except FileNotFoundError:
            continue
    return total


def _git_package_clean(package_dir: Path, revision: str) -> bool:
    if not (package_dir / ".git").is_dir():
        return False
    head = git_checked(
        ["-C", str(package_dir), "rev-parse", "HEAD"], package_dir.parent
    ).stdout.strip()
    if head != revision:
        return False
    status = git_checked(
        ["-C", str(package_dir), "status", "--porcelain=v1", "--untracked-files=all"],
        package_dir.parent,
    ).stdout
    return not status.strip()


def _package_directory(packages_root: Path, name: str) -> Path:
    if (
        not PACKAGE_NAME.fullmatch(name)
        or name in {".", "..", ".git"}
        or name.startswith(".")
    ):
        raise ValueError(f"unsafe dependency package name: {name!r}")
    if packages_root.is_symlink() or not packages_root.is_dir():
        raise ValueError("dependency package root must be a real directory")
    root = packages_root.resolve(strict=True)
    candidate = packages_root / name
    if candidate.is_symlink():
        raise ValueError(f"dependency package destination is a symlink: {name}")
    resolved = candidate.resolve()
    if resolved.parent != root or resolved == root:
        raise ValueError(
            f"dependency package destination is not a strict direct child: {name}"
        )
    if candidate.exists() and not candidate.is_dir():
        raise ValueError(f"dependency package destination is not a directory: {name}")
    for sibling in packages_root.iterdir():
        if sibling.name != name and sibling.name.casefold() == name.casefold():
            raise ValueError(f"dependency package destination case-collides: {name}")
    return candidate


def _validate_package_links(package_dir: Path) -> None:
    for path in package_dir.rglob("*"):
        if not path.is_symlink():
            continue
        target = Path(os.readlink(path))
        if target.is_absolute():
            raise ValueError(f"dependency symlink has an absolute target: {path}")
        try:
            path.resolve(strict=True).relative_to(package_dir.resolve())
        except (FileNotFoundError, ValueError) as exc:
            raise ValueError(
                f"dependency symlink escapes its pinned checkout: {path}"
            ) from exc


def _stream_copy_file(source: str, destination: str) -> str:
    source_path = Path(source)
    destination_path = Path(destination)
    source_flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        source_flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        source_flags |= os.O_NOFOLLOW
    source_descriptor = os.open(source_path, source_flags)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        source_stat = os.fstat(source_descriptor)
        if not stat.S_ISREG(source_stat.st_mode):
            raise ValueError(
                f"dependency execution copy source is not a regular file: {source_path}"
            )
        descriptor = os.open(destination_path, flags, 0o600)
        try:
            while block := os.read(source_descriptor, 1024 * 1024):
                view = memoryview(block)
                while view:
                    written = os.write(descriptor, view)
                    if written < 1:
                        raise OSError(
                            f"short dependency copy write: {destination_path}"
                        )
                    view = view[written:]
            finished_source = os.fstat(source_descriptor)
            if (
                source_stat.st_dev,
                source_stat.st_ino,
                source_stat.st_mode,
                source_stat.st_size,
                source_stat.st_mtime_ns,
                source_stat.st_ctime_ns,
            ) != (
                finished_source.st_dev,
                finished_source.st_ino,
                finished_source.st_mode,
                finished_source.st_size,
                finished_source.st_mtime_ns,
                finished_source.st_ctime_ns,
            ):
                raise ValueError(
                    f"dependency source changed while copying: {source_path}"
                )
            os.fsync(descriptor)
            os.fchmod(descriptor, stat.S_IMODE(source_stat.st_mode) & 0o777)
        finally:
            os.close(descriptor)
    finally:
        os.close(source_descriptor)
    return str(destination_path)


def _regular_file_inodes(root: Path) -> set[tuple[int, int]]:
    inodes: set[tuple[int, int]] = set()
    for path in root.rglob("*"):
        if path.is_symlink() or not path.is_file():
            continue
        fact = path.stat(follow_symlinks=False)
        inodes.add((fact.st_dev, fact.st_ino))
    return inodes


def _validate_execution_copy(canonical_root: Path, execution_root: Path) -> None:
    canonical = canonical_root.resolve()
    execution = execution_root.resolve()
    if execution_root.is_symlink() or canonical == execution:
        raise ValueError(
            "dependency execution copy must be a distinct non-symlink directory"
        )
    for path in execution_root.rglob("*"):
        if not path.is_symlink():
            continue
        target = Path(os.readlink(path))
        if target.is_absolute():
            raise ValueError(f"execution-copy symlink has an absolute target: {path}")
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(execution)
        except (FileNotFoundError, ValueError) as exc:
            raise ValueError(
                f"execution-copy symlink escapes its checkout: {path}"
            ) from exc
        try:
            resolved.relative_to(canonical)
        except ValueError:
            pass
        else:
            raise ValueError(
                f"execution-copy symlink resolves into canonical inputs: {path}"
            )
    shared_inodes = _regular_file_inodes(canonical_root) & _regular_file_inodes(
        execution_root
    )
    if shared_inodes:
        raise ValueError(
            "dependency execution copy shares regular-file inodes with canonical inputs"
        )


def _materialize_git_package(spec: dict[str, str], packages_root: Path) -> bool:
    package_dir = _package_directory(packages_root, spec["name"])
    if package_dir.exists() and _git_package_clean(package_dir, spec["rev"]):
        _validate_package_links(package_dir)
        return True
    if package_dir.exists():
        shutil.rmtree(package_dir)
    package_dir.parent.mkdir(parents=True, exist_ok=True)
    git_checked(["init", "--quiet", str(package_dir)], package_dir.parent)
    git_checked(
        ["-C", str(package_dir), "remote", "add", "origin", spec["url"]],
        package_dir.parent,
    )
    git_checked(
        [
            "-C",
            str(package_dir),
            "-c",
            "protocol.file.allow=never",
            "-c",
            "http.followRedirects=false",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
            "fetch",
            "--no-tags",
            "--depth=1",
            "origin",
            spec["rev"],
        ],
        package_dir.parent,
        timeout=900,
    )
    git_checked(
        [
            "-C",
            str(package_dir),
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
            "checkout",
            "--detach",
            "--force",
            spec["rev"],
        ],
        package_dir.parent,
        timeout=600,
    )
    if not _git_package_clean(package_dir, spec["rev"]):
        raise ValueError(
            f"dependency checkout did not reproduce its exact commit: {spec['name']}"
        )
    _validate_package_links(package_dir)
    return False


def shared_mathlib_cache(pins: dict[str, Any]) -> tuple[Path, dict[str, Any]]:
    cache_key = sha256_bytes(
        f"{pins['toolchain']}\0{pins['manifest_sha256']}".encode("utf-8")
    )[:24]
    cache_dir = ROOT / "target" / "external-lean" / "mathlib-cache" / cache_key
    cache_root = cache_dir.parent
    cache_root.mkdir(parents=True, exist_ok=True)
    evicted_bytes = 0
    for other in cache_root.iterdir():
        if not other.is_dir() or other == cache_dir:
            continue
        evicted_bytes += directory_bytes(other)
        shutil.rmtree(other)
    marker_path = cache_dir / "vela-cache-ready.json"
    expected = {
        "schema": "vela.external_lean_mathlib_cache.v1",
        "cache_key": cache_key,
        "lean_toolchain": pins["toolchain"],
        "mathlib_commit": pins["mathlib_commit"],
        "lake_manifest_sha256": pins["manifest_sha256"],
        "provisioner": "git-only-no-checkout-code.v1",
        "packages": pins["packages"],
    }
    if marker_path.is_file():
        try:
            marker = load_json(marker_path)
        except (ValueError, json.JSONDecodeError):
            marker = {}
        packages_root = cache_dir / ".lake" / "packages"
        if (
            packages_root.is_dir()
            and all(marker.get(key) == value for key, value in expected.items())
            and all(
                _git_package_clean(
                    _package_directory(packages_root, package["name"]),
                    package["rev"],
                )
                for package in pins["packages"]
            )
            and marker.get("canonical_packages_root")
            == _SANDBOX._tree_root(packages_root)
        ):
            return cache_dir, {
                **expected,
                "reused": True,
                "bytes": directory_bytes(cache_dir),
                "evicted_bytes": evicted_bytes,
                "canonical_packages_root": marker["canonical_packages_root"],
            }
    if cache_dir.exists():
        shutil.rmtree(cache_dir)
    cache_dir.mkdir(parents=True)
    write_text(cache_dir / "lean-toolchain", pins["toolchain"] + "\n")
    write_text(
        cache_dir / "lakefile.toml",
        controlled_cache_lakefile(
            pins["mathlib_url"],
            pins["mathlib_commit"],
        ),
    )
    write_json(cache_dir / "lake-manifest.json", pins["manifest"])
    packages_root = cache_dir / ".lake" / "packages"
    packages_root.mkdir(parents=True)
    reused_packages = 0
    for package in pins["packages"]:
        reused_packages += int(_materialize_git_package(package, packages_root))
    marker = {
        **expected,
        "reused": False,
        "canonical_packages_root": _SANDBOX._tree_root(packages_root),
    }
    write_json(marker_path, marker)
    return cache_dir, {
        **marker,
        "bytes": directory_bytes(cache_dir),
        "evicted_bytes": evicted_bytes,
        "reused_packages": reused_packages,
        "package_count": len(pins["packages"]),
    }


def process_output(proc: subprocess.CompletedProcess[str]) -> str:
    return "\n".join(part for part in [proc.stdout, proc.stderr] if part).strip()


def first_lean_error(output: str) -> dict[str, Any] | None:
    lines = output.splitlines()
    for index, line in enumerate(lines):
        match = re.match(r"^(?P<location>.+\.lean:\d+:\d+): error: (?P<text>.*)$", line)
        if not match:
            match = re.match(
                r"^error: (?P<location>.+\.lean:\d+:\d+): (?P<text>.*)$", line
            )
        if not match:
            continue
        detail = [match.group("text")]
        for following in lines[index + 1 : index + 6]:
            if re.match(r"^.+\.lean:\d+:\d+: (?:error|warning):", following):
                break
            if following.strip():
                detail.append(following)
        return {
            "location": match.group("location"),
            "text": "\n".join(detail),
        }
    return None


def is_module_resolution_failure(output: str) -> bool:
    lowered = output.lower()
    return any(
        marker in lowered
        for marker in [
            "unknown module prefix",
            "unknown package",
            "no such file or directory",
            "unknown target",
            "unknown module",
        ]
    )


def setup_controlled_environment(
    workspace: Path,
    pins: dict[str, Any],
    candidate: dict[str, Any],
) -> tuple[dict[str, Any], Path]:
    write_text(workspace / "lean-toolchain", pins["toolchain"] + "\n")
    write_text(
        workspace / "lakefile.toml",
        controlled_pull_lakefile(
            pins["mathlib_url"],
            pins["mathlib_commit"],
            candidate,
        ),
    )
    cache_dir, cache_stats = shared_mathlib_cache(pins)
    manifest_path = workspace / "lake-manifest.json"
    controlled_manifest = dict(pins["manifest"])
    controlled_manifest.update(
        {
            "name": "VelaExternalLeanReplay",
            "lakeDir": ".lake",
            "packagesDir": ".lake/packages",
        }
    )
    write_json(manifest_path, controlled_manifest)
    lake_dir = workspace / ".lake"
    lake_dir.mkdir(parents=True, exist_ok=True)
    execution_packages = lake_dir / "packages"
    if execution_packages.exists() or execution_packages.is_symlink():
        if execution_packages.is_dir() and not execution_packages.is_symlink():
            shutil.rmtree(execution_packages)
        else:
            execution_packages.unlink()
    canonical_packages = cache_dir / ".lake" / "packages"
    shutil.copytree(
        canonical_packages,
        execution_packages,
        symlinks=True,
        copy_function=_stream_copy_file,
    )
    for package in pins["packages"]:
        _validate_package_links(execution_packages / package["name"])
    _validate_execution_copy(canonical_packages, execution_packages)
    execution_copy_root = _SANDBOX._tree_root(execution_packages)
    if execution_copy_root != cache_stats["canonical_packages_root"]:
        raise ValueError(
            "dependency execution copy does not match its measured canonical input"
        )
    manifest = load_json(manifest_path)
    mathlib = next(
        (
            item
            for item in manifest.get("packages", [])
            if item.get("name") == "mathlib"
        ),
        None,
    )
    if pins["mathlib_commit"] is not None and (
        not mathlib or mathlib.get("rev") != pins["mathlib_commit"]
    ):
        raise ValueError("controlled environment resolved a different Mathlib revision")
    root = toolchain_root(pins["toolchain"])
    version = run_trusted_command(
        [str(root / "bin" / "lean"), "--version"], workspace, timeout=60
    )
    expected_version = pins["toolchain"].split(":v", 1)[-1]
    if version.returncode != 0 or expected_version not in process_output(version):
        raise ValueError("controlled environment resolved a different Lean toolchain")
    return {
        **cache_stats,
        "path": str(cache_dir),
        "canonical_role": "dependency_input",
        "execution_copy": {
            "path": str(execution_packages),
            "role": "dependency_execution_copy",
            "initial_tree_root": execution_copy_root,
            "copy_mode": "streamed_bytes_fsync_no_hardlinks_or_reflinks",
        },
    }, cache_dir


def build_target_module(
    workspace: Path,
    source_root: Path,
    pins: dict[str, Any],
    candidates: list[dict[str, Any]],
) -> tuple[
    dict[str, Any],
    subprocess.CompletedProcess[str],
    dict[str, Any],
    list[dict[str, Any]],
]:
    attempts: list[tuple[dict[str, Any], subprocess.CompletedProcess[str]]] = []
    cache_stats: dict[str, Any] = {}
    cache_dir: Path | None = None
    sandbox_records: list[dict[str, Any]] = []
    for index, candidate in enumerate(candidates):
        write_text(
            workspace / "lakefile.toml",
            controlled_pull_lakefile(
                pins["mathlib_url"],
                pins["mathlib_commit"],
                candidate,
            ),
        )
        if index == 0:
            cache_stats, cache_dir = setup_controlled_environment(
                workspace, pins, candidate
            )
        if cache_dir is None:
            raise ValueError("controlled dependency cache was not prepared")
        build_root = workspace / ".lake" / "build"
        if build_root.exists() and index > 0:
            shutil.rmtree(build_root)
        build, sandbox = run_sandboxed_lean_command(
            ["lake", "build", f"+{candidate['module']}:olean"],
            workspace,
            timeout=1800,
            source_root=source_root,
            toolchain=pins["toolchain"],
            dependency_root=cache_dir,
            dependency_identity=pins["mathlib_commit"]
            or f"sha256:{pins['manifest_sha256']}",
            execution_copy_root=workspace / ".lake" / "packages",
        )
        sandbox_records.append(sandbox)
        attempts.append((candidate, build))
        if build.returncode == 0:
            return candidate, build, cache_stats, sandbox_records
        if not is_module_resolution_failure(process_output(build)):
            return candidate, build, cache_stats, sandbox_records
    candidate, build = attempts[-1]
    return candidate, build, cache_stats, sandbox_records


def audit_built_declaration(
    workspace: Path,
    source_root: Path,
    pins: dict[str, Any],
    dependency_root: Path,
    candidate: dict[str, Any],
    declaration: str,
) -> tuple[subprocess.CompletedProcess[str], list[str], dict[str, Any]]:
    audit_source = (
        f"import {candidate['module']}\n\n"
        f"#check {declaration}\n"
        f"#print axioms {declaration}\n"
    )
    audit_path = workspace / "_VelaExternalAudit.lean"
    write_text(audit_path, audit_source)
    lean, sandbox = run_sandboxed_lean_command(
        ["lake", "env", "lean", audit_path.name],
        workspace,
        timeout=600,
        source_root=source_root,
        toolchain=pins["toolchain"],
        dependency_root=dependency_root,
        dependency_identity=pins["mathlib_commit"]
        or f"sha256:{pins['manifest_sha256']}",
        execution_copy_root=workspace / ".lake" / "packages",
    )
    output = process_output(lean)
    if lean.returncode != 0:
        return lean, [], sandbox
    if sandbox["stdout"]["truncated"] or sandbox["stderr"]["truncated"]:
        failed = subprocess.CompletedProcess(
            lean.args,
            125,
            lean.stdout,
            f"{lean.stderr}\nLean audit output truncated before trust result".strip(),
        )
        return failed, [], sandbox
    try:
        axioms = parse_axioms(output, declaration)
    except ValueError as exc:
        failed = subprocess.CompletedProcess(
            lean.args,
            125,
            lean.stdout,
            f"{lean.stderr}\n{exc}; bounded audit output: {output}".strip(),
        )
        return failed, [], sandbox
    return lean, axioms, sandbox


def closed_proposition(source_audit: dict[str, Any]) -> str | None:
    statement = str(source_audit.get("statement") or "")
    match = re.fullmatch(
        r"(?:theorem|lemma)\s+[A-Za-z0-9_'.]+\s*:\s*(?P<proposition>.+)", statement
    )
    return match.group("proposition") if match else None


def run_contradiction_probe(
    workspace: Path,
    source_root: Path,
    pins: dict[str, Any],
    dependency_root: Path,
    source_audit: dict[str, Any],
) -> dict[str, Any] | None:
    proposition = closed_proposition(source_audit)
    if not proposition:
        return None
    probe_path = workspace / "_VelaExternalContradiction.lean"
    write_text(
        probe_path,
        f"example : ¬ ({proposition}) := by\n" "  decide\n",
    )
    probe, sandbox = run_sandboxed_lean_command(
        ["lake", "env", "lean", probe_path.name],
        workspace,
        timeout=300,
        source_root=source_root,
        toolchain=pins["toolchain"],
        dependency_root=dependency_root,
        dependency_identity=pins["mathlib_commit"]
        or f"sha256:{pins['manifest_sha256']}",
        execution_copy_root=workspace / ".lake" / "packages",
    )
    return {
        "contradicted": probe.returncode == 0,
        "proposition": proposition,
        "method": "Lean decidability exact counter-check",
        "exit_code": probe.returncode,
        "output": process_output(probe),
        "sandbox": sandbox,
    }


def external_identity(
    repo_url: str,
    commit: str,
    declaration: str,
    source_path: str,
    source_sha256: str,
    statement_sha256: str,
    pins: dict[str, Any],
    module: str,
    source_manifest_sha256: str,
) -> dict[str, Any]:
    return {
        "source_repo_url": repo_url,
        "source_commit": commit,
        "declaration": declaration,
        "module": module,
        "source_path": source_path,
        "source_sha256": source_sha256,
        "statement_sha256": statement_sha256,
        "lean_toolchain": pins["toolchain"],
        "lean_toolchain_path": pins["toolchain_path"],
        "lean_toolchain_sha256": pins["toolchain_sha256"],
        "mathlib_commit": pins["mathlib_commit"],
        "lake_manifest_path": pins["manifest_path"],
        "lake_manifest_sha256": pins["manifest_sha256"],
        "source_manifest_sha256": source_manifest_sha256,
    }


def typed_result(
    verdict: str,
    identity: dict[str, Any],
    *,
    phase: str,
    axioms: list[str] | None = None,
    error: dict[str, Any] | None = None,
    source_audit: dict[str, Any] | None = None,
    log_sha256: str | None = None,
) -> dict[str, Any]:
    if verdict not in VERDICTS:
        raise ValueError(f"unsupported reproduction verdict: {verdict}")
    return {
        "schema": "vela.external_lean_reproduction_result.v1",
        "ok": True,
        "verdict": verdict,
        "verdict_class": "reproduction",
        "truth_verdict": None,
        "phase": phase,
        "identity": identity,
        "axioms": axioms or [],
        "error": error,
        "source_audit": source_audit,
        "log_sha256": log_sha256,
        "external_code_execution": False,
        "producer_configuration_executed": False,
        "trust_anchor": "Lean kernel in Vela-owned controlled Lake project",
    }


def result_claim(result: dict[str, Any]) -> tuple[str, str, str]:
    identity = result["identity"]
    declaration = identity["declaration"]
    commit = identity["source_commit"]
    verdict = result["verdict"]
    if verdict == "reproduced":
        claim = f"Vela reproduced Lean declaration {declaration} at commit {commit} in the frozen environment."
        caveat = "This is a reproduction verdict, not acceptance of the declaration as frontier truth."
        return claim, "theoretical", caveat
    if verdict == "reproduction_failed":
        claim = f"Vela could not reproduce Lean declaration {declaration} at commit {commit} in the frozen environment."
        caveat = "Build failure is a reproduction verdict and does not establish that the mathematical statement is false."
        return claim, "negative", caveat
    if verdict == "dirty_axioms":
        claim = f"Lean declaration {declaration} at commit {commit} builds but its axiom closure is outside Vela's clean set."
        caveat = "Dirty axioms block a reproduced verdict; they are not a truth verdict on the statement."
        return claim, "negative", caveat
    if verdict == "skipped_with_reason":
        claim = f"Vela skipped reproduction of Lean declaration {declaration} at commit {commit} with a typed reason."
        caveat = "A skipped reproduction is not a build result and is not a truth or quality verdict."
        return claim, "negative", caveat
    claim = f"A registered frozen check contradicted the scoped statement bound to Lean declaration {declaration} at commit {commit}."
    caveat = "Contradiction is scoped to the frozen counter-witness and does not authorize acceptance or finalization."
    return claim, "negative", caveat


def make_reproduction_receipt(
    result: dict[str, Any],
    output_dir: Path,
    manifest_path: Path,
    result_path: Path,
) -> dict[str, Any]:
    identity = result["identity"]
    claim, claim_type, caveat = result_claim(result)
    claim_id = (
        "vf_external_lean_"
        + sha256_bytes(
            f"{identity['source_repo_url']}\0{identity['source_commit']}\0{identity['declaration']}".encode(
                "utf-8"
            )
        )[:16]
    )
    artifacts = [
        receipt_artifact(manifest_path, "external_lean_source_manifest", output_dir),
        receipt_artifact(result_path, "external_lean_reproduction_result", output_dir),
    ]
    outcome = "pass" if result["verdict"] == "reproduced" else result["verdict"]
    receipt = make_receipt(
        claim_id=claim_id,
        claim=claim,
        claim_type=claim_type,
        replayability="exact",
        artifacts=artifacts,
        verifier_runs=[
            {
                "method": "vela reproduce-external",
                "outcome": outcome,
                "log": result.get("error") or f"axioms={result.get('axioms', [])}",
                "replay_command": (
                    f"vela reproduce-external {identity['source_repo_url']} "
                    f"{identity['source_commit']} {identity['declaration']} "
                    f"--source-path {identity['source_path']} --json"
                ),
                "verifier_id": "verifier.lean_external_declaration.v1",
            }
        ],
        caveats=[
            caveat,
            "The receipt is unsigned and remains a draft pending Will's key-custody ceremony.",
        ],
        generated_by="vela reproduce-external",
        submitter="agent:external-lean-onramp",
        acceptance_scope="machine_verified",
        acceptance_status="draft",
        acceptance_authority="frozen_verifier",
        acceptance_profile="vela.frontier.formal_math.v1",
        evidence_refs=[f"urn:sha256:{item['sha256']}" for item in artifacts],
        evidence_level=None,
        distillation=distillation_block(
            status="draft",
            audience="external Lean producer and frontier reviewer",
            level="reproduction outcome",
            rubric="source identity, statement, frozen pins, kernel result, and caveat are present",
        ),
        lineage={
            "frontier": "projects/formal-conjectures-lean",
            "parents": [],
            "derived_from": [f"urn:git:{identity['source_commit']}"],
            "supersedes": [],
            "source_refs": [
                identity["source_repo_url"],
                f"{identity['source_repo_url'].removesuffix('.git')}/commit/{identity['source_commit']}",
            ],
        },
        contributors=[
            {
                "id": "agent:external-lean-onramp",
                "roles": ["machine_producer", "software"],
                "credit_taxonomy": "CRediT+Vela",
                "author": False,
                "note": "The machine is the reproduction originator, never an author.",
            },
            {
                "id": identity["source_repo_url"],
                "roles": ["human_formalizer", "formal_analysis"],
                "credit_taxonomy": "CRediT+Vela",
                "author": True,
                "note": "Repository-level source credit; individual authorship is not inferred.",
            },
        ],
        environment={
            "external_lean_reproduction": result,
            "controlled_project": True,
            "producer_configuration_executed": False,
        },
        provenance_extra={
            "external_source": identity["source_repo_url"],
            "originator": "agent:external-lean-onramp",
        },
    )
    receipt["signature_identities"] = {
        "producer": {
            "role": "producer",
            "signatureRef": None,
            "mechanism": "sigstore_keyless_oidc_expected",
            "oidcIssuer": "https://token.actions.githubusercontent.com",
            "subject": identity["source_repo_url"],
            "status": "unsigned_external_source",
        },
        "acceptor": {
            "role": "acceptor",
            "signatureRef": None,
            "mechanism": "ed25519_key_custody_ceremony",
            "subject": "reviewer:will-blair",
            "status": "awaiting_human_signature",
        },
    }
    from receipt_v1 import attach_statement

    attach_statement(receipt)
    errors = [
        error
        for error in validate_receipt(receipt)
        if error != "standard in-toto package is required for this check"
    ]
    if errors:
        raise ValueError("generated Receipt-v1 is invalid: " + "; ".join(errors))
    return receipt


def pull_reproduce(
    repo_url: str,
    commit: str,
    declaration: str,
    *,
    source_path_hint: str | None = None,
    output_root: Path | None = None,
    draft_frontier: Path | None = None,
    emit_legacy_receipt: bool = True,
) -> dict[str, Any]:
    declaration = validate_declaration(declaration)
    if source_path_hint is not None:
        source_path_hint = lean_source_path(source_path_hint, "requested source path")
    workspace_key = sha256_bytes(f"{repo_url}\0{commit}".encode("utf-8"))[:24]
    result_key = sha256_bytes(f"{repo_url}\0{commit}\0{declaration}".encode("utf-8"))[
        :24
    ]
    session_root = ROOT / "target" / "external-lean" / "workspaces" / result_key
    workspace = session_root / "run"
    output_dir = (
        output_root or (ROOT / "target" / "external-lean" / "results")
    ) / result_key
    git_dir = (
        ROOT / "target" / "external-lean" / "source-cache" / f"{workspace_key}.git"
    )
    source_dir = session_root / "source"
    if session_root.exists():
        shutil.rmtree(session_root)
    workspace.mkdir(parents=True)
    source_dir.mkdir(parents=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    fetched = fetch_commit_read_only(repo_url, commit, git_dir)
    pins = resolve_frozen_pins(git_dir, commit, fetched["paths"])
    source_by_path, manifest = snapshot_lean_sources(
        git_dir, repo_url, commit, fetched["paths"], source_dir
    )
    source_path, source = find_declaration_source(
        source_by_path, declaration, source_path_hint
    )
    source_audit = audit_declaration(source, declaration, [source])
    statement_sha256 = declaration_statement_sha256(source, declaration)
    manifest["resolved_pins"] = pins
    manifest["declaration_source_path"] = source_path
    manifest["declaration_statement_sha256"] = statement_sha256
    manifest_path = output_dir / "artifacts" / "source-manifest.json"
    write_json(manifest_path, manifest)
    manifest_sha256 = sha256_file(manifest_path)
    candidates = module_candidates(source_path, list(source_by_path))
    if not candidates:
        raise ValueError(
            f"could not derive a controlled Lean module name from {source_path}"
        )
    candidate, build, cache_stats, sandbox_records = build_target_module(
        workspace,
        source_dir,
        pins,
        candidates,
    )
    dependency_root = Path(cache_stats["path"])
    source_sha256 = sha256_bytes(source.encode("utf-8"))
    identity = external_identity(
        repo_url,
        commit,
        declaration,
        source_path,
        source_sha256,
        statement_sha256,
        pins,
        candidate["module"],
        manifest_sha256,
    )
    build_output = process_output(build)
    log = {
        "schema": "vela.external_lean_reproduction_log.v1",
        "commands": [
            "git fetch --no-tags --depth=1 origin <commit>",
            "git ls-tree and git show commit-pinned blobs",
            "Git-only materialization of manifest-pinned dependency sources",
            "streamed private execution copy of canonical dependency sources",
            f"lake build +{candidate['module']}:olean",
        ],
        "build_exit_code": build.returncode,
        "build_output": build_output,
        "producer_configuration_executed": False,
        "sandbox_executions": sandbox_records,
    }
    if build.returncode != 0:
        error = first_lean_error(build_output) or {
            "location": None,
            "text": build_output[-4000:] or "Lean module build failed without output",
        }
        contradiction = run_contradiction_probe(
            workspace,
            source_dir,
            pins,
            dependency_root,
            source_audit,
        )
        if contradiction and contradiction["contradicted"]:
            sandbox_records.append(contradiction["sandbox"])
            log["commands"].append("Lean decidability exact counter-check")
            log["contradiction_probe"] = contradiction
            result = typed_result(
                "contradicted",
                identity,
                phase="exact_counter_check",
                error={
                    "location": source_path,
                    "text": f"exact counter-check proved not ({contradiction['proposition']})",
                },
                source_audit=source_audit,
            )
        else:
            if contradiction:
                sandbox_records.append(contradiction["sandbox"])
                log["contradiction_probe"] = contradiction
            result = typed_result(
                "reproduction_failed",
                identity,
                phase="module_build",
                error=error,
                source_audit=source_audit,
            )
    else:
        lean, axioms, audit_sandbox = audit_built_declaration(
            workspace,
            source_dir,
            pins,
            dependency_root,
            candidate,
            declaration,
        )
        sandbox_records.append(audit_sandbox)
        audit_output = process_output(lean)
        log.update(
            {
                "commands": [*log["commands"], f"#print axioms {declaration}"],
                "audit_exit_code": lean.returncode,
                "audit_output": audit_output,
            }
        )
        if lean.returncode != 0:
            error = first_lean_error(audit_output) or {
                "location": None,
                "text": audit_output[-4000:]
                or "Lean declaration audit failed without output",
            }
            result = typed_result(
                "reproduction_failed",
                identity,
                phase="declaration_audit",
                error=error,
                source_audit=source_audit,
            )
        else:
            forbidden = sorted(
                axiom
                for axiom in axioms
                if axiom not in ALLOWED_AXIOMS
                or axiom in DENIED_AXIOMS
                or "native_decide" in axiom
            )
            source_smells = sorted(
                name for name, passed in source_audit["checks"].items() if not passed
            )
            if forbidden or source_smells:
                result = typed_result(
                    "dirty_axioms",
                    identity,
                    phase="axiom_audit",
                    axioms=axioms,
                    error={
                        "location": source_path,
                        "text": f"forbidden_axioms={forbidden}; source_audit_failures={source_smells}",
                    },
                    source_audit=source_audit,
                )
            else:
                result = typed_result(
                    "reproduced",
                    identity,
                    phase="axiom_audit",
                    axioms=axioms,
                    source_audit=source_audit,
                )
    result["external_code_execution"] = True
    result["trust_anchor"] = "Lean kernel under Vela's fail-closed OS sandbox"
    result["sandbox"] = {
        "driver_root": "sha256:" + sha256_file(SANDBOX_DRIVER),
        "backend": sandbox_records[-1]["sandbox"]["backend"]
        if sandbox_records
        else None,
        "executions": sandbox_records,
        "all_fail_closed": bool(sandbox_records)
        and all(sandbox_execution_accepted(record) for record in sandbox_records),
        "blocked_capabilities": _SANDBOX.BLOCKED_CAPABILITIES,
    }
    log["sandbox_executions"] = sandbox_records
    log_path = output_dir / "artifacts" / "reproduction-log.json"
    write_json(log_path, log)
    result["log_sha256"] = sha256_file(log_path)
    result["replay_command"] = (
        f"vela reproduce-external {repo_url} {commit} {declaration} "
        f"--source-path {source_path} --json"
    )
    result_path = output_dir / "artifacts" / "reproduction-result.json"
    write_json(result_path, result)
    receipt = None
    receipt_path = None
    if emit_legacy_receipt:
        receipt = make_reproduction_receipt(
            result, output_dir, manifest_path, result_path
        )
        receipt_temp = output_dir / "receipts" / ".receipt.tmp.json"
        write_json(receipt_temp, receipt)
        receipt_digest = sha256_file(receipt_temp)
        receipt_path = output_dir / "receipts" / f"sha256-{receipt_digest}.json"
        receipt_temp.replace(receipt_path)
    staged_path = None
    if (
        emit_legacy_receipt
        and draft_frontier is not None
        and result["verdict"] == "reproduced"
    ):
        assert receipt is not None
        staged_path = (
            draft_frontier / ".vela" / "receipts" / f"{receipt['claim_id']}.json"
        )
        write_json(staged_path, receipt)
    return {
        "ok": True,
        "command": "reproduce-external",
        "verdict": result["verdict"],
        "receipt": (
            {
                "path": str(receipt_path),
                "sha256": sha256_file(receipt_path),
                "content_address": f"sha256:{sha256_file(receipt_path)}",
                "schema": receipt["schema"],
                "claim_id": receipt["claim_id"],
                "signed": False,
                "acceptance_status": "draft",
            }
            if receipt is not None and receipt_path is not None
            else None
        ),
        "result": result,
        "reproduction_result": {
            "path": str(result_path),
            "sha256": sha256_file(result_path),
        },
        "source_manifest": {"path": str(manifest_path), "sha256": manifest_sha256},
        "verifier_log": {"path": str(log_path), "sha256": sha256_file(log_path)},
        "draft_frontier_receipt": str(staged_path) if staged_path else None,
        "landing": {
            "supported": True,
            "receipt_path": str(receipt_path) if receipt_path is not None else None,
            "command_template": (
                "vela reproduce-external <repo-url> <commit> <declaration> "
                "--land-work <target> --as agent:<name> --json"
            ),
            "direct_state_mutation": False,
        },
        "workspace_key": workspace_key,
        "result_key": result_key,
        "disk": {
            "shared_mathlib_cache_key": cache_stats["cache_key"],
            "shared_mathlib_cache_bytes": cache_stats["bytes"],
            "shared_mathlib_cache_reused": cache_stats["reused"],
            "shared_mathlib_cache_evicted_bytes": cache_stats["evicted_bytes"],
        },
    }


def classify_skip(exc: Exception) -> tuple[str, str]:
    text = str(exc)
    lowered = text.lower()
    if isinstance(exc, subprocess.TimeoutExpired):
        return "timeout", f"command timed out after {exc.timeout} seconds"
    if "does not expose lean-toolchain and lake-manifest.json" in lowered:
        return "missing_pins", text
    if "does not pin mathlib" in lowered or "unfrozen" in lowered:
        return "floating_or_unsupported_pin", text
    if "was not found in commit-pinned lean sources" in lowered:
        return "declaration_drift", text
    if "declaration short name is ambiguous" in lowered:
        return "declaration_ambiguous", text
    if "source snapshot exceeds" in lowered or "repository contains" in lowered:
        return "source_limit", text
    if "fetch" in lowered or "repository" in lowered or "git" in lowered:
        return "source_unavailable", text
    return "runner_error", text


def skipped_pull_result(
    repo_url: str,
    commit: str,
    declaration: str,
    exc: Exception,
    source_path_hint: str | None = None,
) -> dict[str, Any]:
    reason_code, text = classify_skip(exc)
    identity = {
        "source_repo_url": repo_url,
        "source_commit": commit,
        "declaration": declaration,
        "module": None,
        "source_path": source_path_hint,
        "source_sha256": None,
        "statement_sha256": None,
        "lean_toolchain": None,
        "lean_toolchain_path": None,
        "lean_toolchain_sha256": None,
        "mathlib_commit": None,
        "lake_manifest_path": None,
        "lake_manifest_sha256": None,
        "source_manifest_sha256": None,
    }
    result = typed_result(
        "skipped_with_reason",
        identity,
        phase="preflight" if reason_code != "timeout" else "timeout",
        error={"code": reason_code, "location": None, "text": text},
    )
    source_arg = f" --source-path {source_path_hint}" if source_path_hint else ""
    result["replay_command"] = (
        f"vela reproduce-external {repo_url} {commit} {declaration}{source_arg} --json"
    )
    return {
        "ok": True,
        "command": "reproduce-external",
        "verdict": "skipped_with_reason",
        "receipt": None,
        "result": result,
        "source_manifest": None,
        "verifier_log": None,
        "draft_frontier_receipt": None,
        "workspace_key": sha256_bytes(f"{repo_url}\0{commit}".encode("utf-8"))[:24],
        "result_key": sha256_bytes(
            f"{repo_url}\0{commit}\0{declaration}".encode("utf-8")
        )[:24],
        "disk": {
            "shared_mathlib_cache_key": None,
            "shared_mathlib_cache_bytes": 0,
            "shared_mathlib_cache_reused": False,
            "shared_mathlib_cache_evicted_bytes": 0,
        },
    }


def pull_reproduce_typed(
    repo_url: str,
    commit: str,
    declaration: str,
    *,
    source_path_hint: str | None = None,
    output_root: Path | None = None,
    draft_frontier: Path | None = None,
    emit_legacy_receipt: bool = True,
) -> dict[str, Any]:
    result_key = sha256_bytes(f"{repo_url}\0{commit}\0{declaration}".encode("utf-8"))[
        :24
    ]
    workspace = ROOT / "target" / "external-lean" / "workspaces" / result_key
    try:
        result = pull_reproduce(
            repo_url,
            commit,
            declaration,
            source_path_hint=source_path_hint,
            output_root=output_root,
            draft_frontier=draft_frontier,
            emit_legacy_receipt=emit_legacy_receipt,
        )
    except Exception as exc:
        result = skipped_pull_result(
            repo_url, commit, declaration, exc, source_path_hint
        )
    reclaimed = directory_bytes(workspace)
    if (
        workspace.exists()
        and os.environ.get("VELA_EXTERNAL_LEAN_KEEP_WORKSPACE") != "1"
    ):
        shutil.rmtree(workspace)
    result["disk"]["workspace_reclaimed_bytes"] = reclaimed
    result["disk"]["workspace_cleaned"] = not workspace.exists()
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--packet")
    parser.add_argument("--bundle-root")
    parser.add_argument("--print-environment-contract-candidate", action="store_true")
    parser.add_argument("--repo-url")
    parser.add_argument("--commit")
    parser.add_argument("--declaration")
    parser.add_argument("--source-path")
    parser.add_argument("--output-root")
    parser.add_argument("--draft-frontier")
    parser.add_argument(
        "--installed-result-only", action="store_true", help=argparse.SUPPRESS
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.packet:
            packet_path = Path(args.packet)
            bundle_root = Path(args.bundle_root).resolve() if args.bundle_root else None
            if args.print_environment_contract_candidate:
                result = environment_contract_candidate(packet_path, bundle_root)
                message = result["environment_root"]
            else:
                result = verify(packet_path, bundle_root=bundle_root)
                message = result["message"]
        elif args.repo_url and args.commit and args.declaration:
            result = pull_reproduce_typed(
                args.repo_url,
                args.commit,
                args.declaration,
                source_path_hint=args.source_path,
                output_root=Path(args.output_root).resolve()
                if args.output_root
                else None,
                draft_frontier=Path(args.draft_frontier).resolve()
                if args.draft_frontier
                else None,
                emit_legacy_receipt=not args.installed_result_only,
            )
            if result.get("receipt"):
                message = f"{result['verdict']}: {result['receipt']['content_address']}"
            elif isinstance(result.get("result", {}).get("error"), dict):
                error = result["result"]["error"]
                error_kind = error.get("code") or error.get("class") or "typed_failure"
                message = f"{result['verdict']}: {error_kind}"
            else:
                message = f"{result['verdict']}: installed result only"
        else:
            parser.error("provide --packet or --repo-url, --commit, and --declaration")
            raise AssertionError("unreachable")
        print(json.dumps(result, sort_keys=True) if args.json else message)
        return 0
    except Exception as exc:
        result = {"ok": False, "error": str(exc)}
        print(json.dumps(result, sort_keys=True) if args.json else f"ERROR {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
