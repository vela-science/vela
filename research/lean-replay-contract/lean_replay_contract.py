"""Generic, non-authoritative primitives for exact Lean replay consumers."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from typing import Any, Iterable, Mapping, Sequence


SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


class ContractError(RuntimeError):
    """The replay request, package reference, or native environment failed closed."""


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def canonical_result_bytes(value: Any) -> bytes:
    """Encode result JSON for transport; this is not package-root canonicalization."""
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode()


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"read JSON {path}: {error}") from error


def verify_package_reference(package: Path, reference: Mapping[str, Any]) -> str:
    """Verify embedded JCS descriptor bytes and every referenced package file."""
    package = package.resolve(strict=True)
    if reference.get("schema") != "vela.package-consumer-reference.v1":
        raise ContractError("package consumer-reference schema differs")
    if reference.get("authority_effect") != "none":
        raise ContractError("package authority_effect must be none")
    expected_root = reference.get("package_root")
    if not isinstance(expected_root, str) or not SHA256_RE.fullmatch(expected_root):
        raise ContractError("package root is not a sha256 root")
    descriptor_jcs = reference.get("descriptor_jcs")
    if not isinstance(descriptor_jcs, str):
        raise ContractError("package reference omits descriptor JCS bytes")
    if sha256_bytes(descriptor_jcs.encode("utf-8")) != expected_root:
        raise ContractError("package descriptor JCS root differs")
    try:
        descriptor = json.loads(descriptor_jcs)
    except json.JSONDecodeError as error:
        raise ContractError("package descriptor JCS is not JSON") from error
    if descriptor.get("schema") != "vela.logical-package-root.v1":
        raise ContractError("logical package-root schema differs")
    rows = descriptor.get("files")
    if not isinstance(rows, list) or not rows:
        raise ContractError("logical package root has no files")
    observed_paths: list[str] = []
    for row in rows:
        if not isinstance(row, dict):
            raise ContractError("package file descriptor is not an object")
        relative = row.get("path")
        if not isinstance(relative, str) or not relative or relative.startswith("/"):
            raise ContractError("package file path is not normalized relative text")
        candidate = package / relative
        if candidate.is_symlink():
            raise ContractError(f"package file may not be a symlink: {relative}")
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(package)
        except (FileNotFoundError, ValueError) as error:
            raise ContractError(f"package file escapes or is missing: {relative}") from error
        if not resolved.is_file():
            raise ContractError(f"package path is not a file: {relative}")
        value = resolved.read_bytes()
        if len(value) != row.get("size"):
            raise ContractError(f"package file size differs: {relative}")
        if sha256_bytes(value) != row.get("sha256"):
            raise ContractError(f"package file root differs: {relative}")
        observed_paths.append(relative)
    if observed_paths != sorted(observed_paths) or len(observed_paths) != len(set(observed_paths)):
        raise ContractError("package file descriptors are not unique and sorted")
    return expected_root


def git_output(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ContractError(
            f"git {' '.join(arguments)} failed in {repository}: "
            f"{result.stderr.strip()}"
        )
    return result.stdout.strip()


def validate_git_identity(
    repository: Path, *, commit: str, tree: str, require_head: bool, require_clean: bool
) -> None:
    repository = repository.resolve(strict=True)
    try:
        git_output(repository, "cat-file", "-e", f"{commit}^{{commit}}")
    except ContractError as error:
        raise ContractError(f"source checkout lacks frozen commit {commit}") from error
    if git_output(repository, "rev-parse", f"{commit}^{{tree}}") != tree:
        raise ContractError("source tree differs")
    if require_head and git_output(repository, "rev-parse", "HEAD") != commit:
        raise ContractError("source HEAD differs")
    if require_clean and git_output(
        repository, "status", "--porcelain=v1", "--untracked-files=all"
    ):
        raise ContractError("source checkout is dirty")


def validate_toolchain(
    workspace: Path, *, expected_text: str | None = None, expected_root: str | None = None
) -> str:
    path = workspace / "lean-toolchain"
    value = path.read_bytes()
    text = value.decode("utf-8").strip()
    if expected_text is not None and text != expected_text:
        raise ContractError("Lean toolchain differs")
    if expected_root is not None and sha256_bytes(value) != expected_root:
        raise ContractError("Lean toolchain root differs")
    return text


def validate_mathlib_revision(workspace: Path, expected: str) -> None:
    manifest = load_json(workspace / "lake-manifest.json")
    packages = manifest.get("packages", []) if isinstance(manifest, dict) else []
    row = next(
        (item for item in packages if isinstance(item, dict) and item.get("name") == "mathlib"),
        None,
    )
    if row is None or row.get("rev") != expected:
        raise ContractError("Mathlib revision differs")


def exact_environment(environment: Mapping[str, str] | None = None) -> dict[str, str]:
    source = os.environ if environment is None else environment
    return {
        key: value
        for key, value in source.items()
        if key.lower() not in {"http_proxy", "https_proxy", "all_proxy"}
    }


def network_denied_command(command: Sequence[str]) -> list[str]:
    sandbox = Path("/usr/bin/sandbox-exec")
    if not sandbox.is_file():
        raise ContractError(
            "network-denied replay requires a qualified external sandbox on this platform"
        )
    return [
        str(sandbox),
        "-p",
        "(version 1) (allow default) (deny network*)",
        *command,
    ]


def run_checked(
    command: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str] | None = None,
    timeout_seconds: int | None = None,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        list(command),
        cwd=cwd,
        env=None if environment is None else dict(environment),
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
    )
    if result.returncode != 0:
        raise ContractError(
            f"{' '.join(command)} failed with {result.returncode}: "
            f"{result.stderr.strip() or result.stdout.strip()}"
        )
    return result


def parse_axioms(
    output: str,
    *,
    declaration: str,
    permitted: Iterable[str],
    expected: Sequence[str] | None = None,
) -> list[str]:
    pattern = re.compile(
        rf"(?:'{re.escape(declaration)}'|{re.escape(declaration)})\s+depends on axioms:\s*\[([^]]*)\]",
        re.DOTALL,
    )
    match = pattern.search(output)
    if not match:
        raise ContractError("Lean did not report the exact target declaration's axioms")
    values = [item.strip() for item in match.group(1).split(",") if item.strip()]
    unexpected = sorted(set(values) - set(permitted))
    if unexpected:
        raise ContractError(f"target uses forbidden axioms: {', '.join(unexpected)}")
    if expected is not None and values != list(expected):
        raise ContractError(
            f"target axiom set differs: expected {list(expected)!r}, observed {values!r}"
        )
    return values

