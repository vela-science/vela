#!/usr/bin/env python3
"""Frozen, dependency-free execution core for untrusted Lean elaboration.

The caller fetches and pins source and toolchains before invoking this module.
This module owns the trust boundary: it refuses to execute unless a supported
OS sandbox is present, supplies a credential-free environment, applies process
resource limits, bounds retained output, and returns one JSON result.

This file is embedded in the ``vela`` binary by ``external_lean.rs``.  The
campaign adapter imports the same file, so installed and in-repository runs do
not grow separate sandbox implementations.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import posixpath
import re
import resource
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unicodedata
from pathlib import Path
from typing import Any

SCHEMA_REQUEST = "vela.external_lean_sandbox_request.v1"
SCHEMA_RESULT = "vela.external_lean_sandbox_result.v1"

DEFAULT_LIMITS = {
    "cpu_seconds": 600,
    "memory_bytes": 4 * 1024 * 1024 * 1024,
    "disk_bytes": 1024 * 1024 * 1024,
    "processes": 128,
    "output_bytes": 128 * 1024,
    "wall_seconds": 900,
    "open_files": 256,
    "single_file_bytes": 512 * 1024 * 1024,
}

BLOCKED_CAPABILITIES = [
    "network",
    "inherited_environment",
    "credentials_and_tokens",
    "ssh_agent",
    "user_home_and_config",
    "writes_outside_output_root",
    "executables_outside_allowlist",
    "unbounded_cpu",
    "unbounded_memory",
    "unbounded_disk",
    "unbounded_processes",
    "unbounded_output",
    "unbounded_wall_time",
]

SAFE_ENV_KEYS = {
    "LANG",
    "LC_ALL",
    "LEAN_PATH",
    "LEAN_SYSROOT",
    "PATH",
}

FORBIDDEN_ENV_NAME = re.compile(
    r"(?i)(?:token|secret|credential|password|passwd|cookie|authorization|"
    r"ssh|gpg|aws|azure|google|github|gitlab|npm|pypi|docker|kube|netrc)"
)

_FILE_DIGEST_CACHE: dict[tuple[str, int, int, int, int, int], bytes] = {}
MAX_MEASURED_ENTRIES = 250_000
MAX_MEASURED_BYTES = 32 * 1024 * 1024 * 1024
MAX_MEASURED_PATH_BYTES = 4096


class SandboxUnavailable(RuntimeError):
    """No supported fail-closed sandbox is available."""


class InvalidRequest(ValueError):
    """The execution request is unsafe or malformed."""


def _canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _resolved_directory(value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise InvalidRequest(f"{label} must be a non-empty absolute path")
    path = Path(value)
    if not path.is_absolute():
        raise InvalidRequest(f"{label} must be absolute")
    if path.is_symlink():
        raise InvalidRequest(f"{label} must not be a symlink")
    resolved = path.resolve()
    if not resolved.is_dir():
        raise InvalidRequest(f"{label} is not a directory: {resolved}")
    return resolved


def _resolved_executable(value: Any) -> Path:
    if not isinstance(value, str) or not value:
        raise InvalidRequest("command executable must be a non-empty absolute path")
    path = Path(value)
    if not path.is_absolute():
        raise InvalidRequest("command executable must be absolute")
    resolved = path.resolve()
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise InvalidRequest(f"command executable is unavailable: {resolved}")
    return resolved


def _within(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def _scan_tree(
    root: Path,
    *,
    hash_files: bool,
    validate_symlinks: bool,
) -> list[dict[str, Any]]:
    """Descriptor-relative bounded tree scan; never follows a path-open race."""
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

    def identity(fact: os.stat_result) -> tuple[int, int, int, int, int, int]:
        return (
            fact.st_dev,
            fact.st_ino,
            fact.st_mode,
            fact.st_size,
            fact.st_mtime_ns,
            fact.st_ctime_ns,
        )

    try:
        root_descriptor = os.open(root, directory_flags)
    except OSError as exc:
        raise InvalidRequest(f"could not safely open measured root: {root}") from exc
    resolved_root = root.resolve(strict=True)
    entries: list[dict[str, Any]] = []
    casefolded: set[str] = set()
    measured_bytes = 0

    def walk(directory_descriptor: int, prefix: str) -> None:
        nonlocal measured_bytes
        names = os.listdir(directory_descriptor)
        encoded_names: list[tuple[bytes, str]] = []
        for name in names:
            try:
                encoded = name.encode("utf-8")
            except UnicodeEncodeError as exc:
                raise InvalidRequest("measured tree path is not UTF-8") from exc
            encoded_names.append((encoded, name))
        for _encoded, name in sorted(encoded_names):
            relative = f"{prefix}/{name}" if prefix else name
            relative_bytes = relative.encode("utf-8")
            if len(relative_bytes) > MAX_MEASURED_PATH_BYTES:
                raise InvalidRequest("measured tree path exceeds its bound")
            folded = unicodedata.normalize("NFC", relative).casefold()
            if folded in casefolded:
                raise InvalidRequest(f"measured tree contains a case-colliding path: {relative}")
            casefolded.add(folded)
            if len(casefolded) > MAX_MEASURED_ENTRIES:
                raise InvalidRequest("measured tree exceeds its entry bound")
            try:
                before = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
            except FileNotFoundError:
                if validate_symlinks:
                    raise InvalidRequest(f"measured tree changed during scan: {relative}")
                continue
            if stat.S_ISLNK(before.st_mode):
                target = os.readlink(name, dir_fd=directory_descriptor)
                if validate_symlinks:
                    if target.startswith("/"):
                        raise InvalidRequest(f"measured tree has an absolute symlink: {relative}")
                    lexical = posixpath.normpath(
                        posixpath.join(posixpath.dirname(relative), target)
                    )
                    if lexical == ".." or lexical.startswith("../"):
                        raise InvalidRequest(f"measured tree symlink escapes: {relative}")
                    try:
                        (resolved_root / relative).resolve(strict=True).relative_to(resolved_root)
                    except (FileNotFoundError, ValueError) as exc:
                        raise InvalidRequest(
                            f"measured tree symlink escapes or dangles: {relative}"
                        ) from exc
                after = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
                if identity(before) != identity(after) or os.readlink(
                    name, dir_fd=directory_descriptor
                ) != target:
                    raise InvalidRequest(f"measured tree changed during scan: {relative}")
                entries.append({"kind": "link", "path": relative, "target": target})
            elif stat.S_ISREG(before.st_mode):
                descriptor = os.open(name, file_flags, dir_fd=directory_descriptor)
                try:
                    opened = os.fstat(descriptor)
                    if not stat.S_ISREG(opened.st_mode) or identity(before) != identity(opened):
                        raise InvalidRequest(f"measured tree entry changed type: {relative}")
                    measured_bytes += opened.st_size
                    if measured_bytes > MAX_MEASURED_BYTES:
                        raise InvalidRequest("measured tree exceeds its byte bound")
                    file_digest: bytes | None = None
                    if hash_files:
                        cache_key = (
                            str(resolved_root / relative),
                            opened.st_dev,
                            opened.st_ino,
                            opened.st_size,
                            opened.st_mtime_ns,
                            opened.st_ctime_ns,
                        )
                        file_digest = _FILE_DIGEST_CACHE.get(cache_key)
                        if file_digest is None:
                            file_hasher = hashlib.sha256()
                            while block := os.read(descriptor, 1024 * 1024):
                                file_hasher.update(block)
                            file_digest = file_hasher.digest()
                            _FILE_DIGEST_CACHE[cache_key] = file_digest
                    finished = os.fstat(descriptor)
                    try:
                        after = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
                    except FileNotFoundError:
                        after = finished
                    if identity(opened) != identity(finished) or identity(opened) != identity(after):
                        if validate_symlinks:
                            raise InvalidRequest(
                                f"measured tree file changed during scan: {relative}"
                            )
                        measured_bytes += max(finished.st_size, after.st_size) - opened.st_size
                        if measured_bytes > MAX_MEASURED_BYTES:
                            raise InvalidRequest("measured tree exceeds its byte bound")
                        opened = finished
                    entries.append({
                        "kind": "file",
                        "path": relative,
                        "executable": bool(opened.st_mode & 0o111),
                        "sha256": file_digest,
                        "device": opened.st_dev,
                        "inode": opened.st_ino,
                        "bytes": opened.st_size,
                    })
                finally:
                    os.close(descriptor)
            elif stat.S_ISDIR(before.st_mode):
                child = os.open(name, directory_flags, dir_fd=directory_descriptor)
                try:
                    opened = os.fstat(child)
                    if not stat.S_ISDIR(opened.st_mode) or identity(before) != identity(opened):
                        raise InvalidRequest(f"measured tree entry changed type: {relative}")
                    walk(child, relative)
                    try:
                        after = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
                    except FileNotFoundError:
                        after = os.fstat(child)
                    if identity(opened) != identity(os.fstat(child)) or identity(opened) != identity(after):
                        if validate_symlinks:
                            raise InvalidRequest(
                                f"measured tree directory changed during scan: {relative}"
                            )
                finally:
                    os.close(child)
            else:
                raise InvalidRequest(f"measured tree contains a special file: {relative}")
        if validate_symlinks and sorted(names) != sorted(os.listdir(directory_descriptor)):
            raise InvalidRequest(f"measured tree directory changed during scan: {prefix or '.'}")

    try:
        walk(root_descriptor, "")
    finally:
        os.close(root_descriptor)
    return entries


def _directory_bytes(root: Path, excluded: list[Path]) -> int:
    if excluded and any(_within(root, item) or _within(item, root) for item in excluded):
        raise InvalidRequest("disk measurement root overlaps an excluded canonical root")
    return sum(
        int(entry["bytes"])
        for entry in _scan_tree(root, hash_files=False, validate_symlinks=False)
        if entry["kind"] == "file"
    )


def _tree_root(root: Path) -> str:
    digest = hashlib.sha256(b"vela.measured_tree.v3\0")
    for entry in sorted(
        _scan_tree(root, hash_files=True, validate_symlinks=True),
        key=lambda item: item["path"].encode("utf-8"),
    ):
        path = entry["path"].encode("utf-8")
        if entry["kind"] == "link":
            digest.update(b"L\0" + path + b"\0" + entry["target"].encode("utf-8") + b"\0")
        else:
            executable = b"1" if entry["executable"] else b"0"
            digest.update(
                b"F\0" + path + b"\0" + executable + b"\0" + entry["sha256"].hex().encode() + b"\0"
            )
    return "sha256:" + digest.hexdigest()


def _regular_file_inodes(root: Path) -> set[tuple[int, int]]:
    return {
        (int(entry["device"]), int(entry["inode"]))
        for entry in _scan_tree(root, hash_files=False, validate_symlinks=True)
        if entry["kind"] == "file"
    }


def _validate_execution_copy(root: Path, canonical_roots: list[Path]) -> None:
    resolved_root = root.resolve()
    for path in root.rglob("*"):
        if not path.is_symlink():
            continue
        target = Path(os.readlink(path))
        if target.is_absolute():
            raise InvalidRequest(f"execution copy contains an absolute symlink: {path}")
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(resolved_root)
        except (FileNotFoundError, ValueError) as exc:
            raise InvalidRequest(f"execution copy symlink escapes its root: {path}") from exc
        if any(_within(resolved, canonical) for canonical in canonical_roots):
            raise InvalidRequest(f"execution copy symlink resolves into a canonical root: {path}")
    execution_inodes = _regular_file_inodes(root)
    if not execution_inodes:
        return
    canonical_inodes: set[tuple[int, int]] = set()
    for canonical in canonical_roots:
        canonical_inodes.update(_regular_file_inodes(canonical))
    if execution_inodes & canonical_inodes:
        raise InvalidRequest("execution copy shares a regular-file inode with a canonical root")


def _protected_snapshot(
    root: Path,
    read_roots: list[Path],
    execution_copy_roots: list[Path],
) -> dict[str, dict[str, Any]]:
    snapshot: dict[str, dict[str, Any]] = {}
    for path in sorted(root.rglob("*"), key=lambda item: item.as_posix()):
        if any(path == execution or _within(path, execution) for execution in execution_copy_roots):
            continue
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            target = path.resolve()
            allowed_targets = [*read_roots, *execution_copy_roots]
            if not any(_within(target, allowed) for allowed in allowed_targets):
                raise InvalidRequest(f"write_root symlink escapes declared roots: {relative}")
            snapshot[relative] = {"kind": "symlink", "target": str(target)}
        elif path.is_file():
            snapshot[relative] = {
                "kind": "file",
                "sha256": _sha256_bytes(path.read_bytes()),
                "bytes": path.stat().st_size,
            }
        elif path.is_dir():
            snapshot[relative] = {"kind": "directory"}
        else:
            raise InvalidRequest(f"write_root contains unsupported filesystem object: {relative}")
    return snapshot


def _protected_snapshot_unchanged(root: Path, expected: dict[str, dict[str, Any]]) -> bool:
    for relative, fact in expected.items():
        path = root / relative
        if fact["kind"] == "directory":
            if not path.is_dir() or path.is_symlink():
                return False
        elif fact["kind"] == "symlink":
            if not path.is_symlink() or str(path.resolve()) != fact["target"]:
                return False
        elif not path.is_file() or path.is_symlink() or _sha256_bytes(path.read_bytes()) != fact["sha256"]:
            return False
    return True


def _safe_text(raw: bytes, *, max_bytes: int, max_scalars: int = 4096) -> dict[str, Any]:
    decoded = raw.decode("utf-8", errors="replace")
    rendered: list[str] = []
    used_bytes = 0
    truncated = False
    for index, character in enumerate(decoded):
        codepoint = ord(character)
        if character == "\n":
            replacement = "\n"
        elif character == "\t":
            replacement = "\\u{0009}"
        elif (
            codepoint < 0x20
            or 0x7F <= codepoint <= 0x9F
            or codepoint in {0x2028, 0x2029}
            or 0x202A <= codepoint <= 0x202E
            or 0x2066 <= codepoint <= 0x2069
        ):
            replacement = f"\\u{{{codepoint:04X}}}"
        else:
            replacement = character
        encoded = replacement.encode("utf-8")
        if index >= max_scalars or used_bytes + len(encoded) > max_bytes:
            truncated = True
            break
        rendered.append(replacement)
        used_bytes += len(encoded)
    return {
        "rendered": "".join(rendered),
        "raw_sha256": _sha256_bytes(raw),
        "raw_bytes": len(raw),
        "truncated": truncated,
    }


def _limits(requested: Any) -> dict[str, int]:
    if requested is None:
        return dict(DEFAULT_LIMITS)
    if not isinstance(requested, dict):
        raise InvalidRequest("limits must be an object")
    result = dict(DEFAULT_LIMITS)
    for name, ceiling in DEFAULT_LIMITS.items():
        if name not in requested:
            continue
        value = requested[name]
        if not isinstance(value, int) or value < 1 or value > ceiling:
            raise InvalidRequest(f"limit {name} must be an integer in 1..{ceiling}")
        result[name] = value
    unknown = sorted(set(requested) - set(DEFAULT_LIMITS))
    if unknown:
        raise InvalidRequest(f"unknown limits: {', '.join(unknown)}")
    return result


def _backend() -> tuple[str, Path]:
    forced = os.environ.get("VELA_EXTERNAL_LEAN_SANDBOX_BACKEND")
    if forced in {"none", "disabled"}:
        raise SandboxUnavailable("sandbox disabled by VELA_EXTERNAL_LEAN_SANDBOX_BACKEND")
    if forced not in {None, "sandbox-exec"}:
        raise SandboxUnavailable(f"unsupported forced sandbox backend: {forced}")
    sandbox_exec = Path("/usr/bin/sandbox-exec")
    if forced in {None, "sandbox-exec"} and platform.system() == "Darwin" and sandbox_exec.is_file():
        return "sandbox-exec", sandbox_exec
    # Bubblewrap is intentionally not accepted here.  A namespace which
    # read-only-binds a source/toolchain tree does not constrain executable
    # files found inside that tree.  Until Vela also installs a seccomp or LSM
    # exec allowlist, Linux fails closed instead of overstating that boundary.
    raise SandboxUnavailable("no supported OS sandbox found (sandbox-exec on macOS)")


def _sb_quote(path: Path) -> str:
    return json.dumps(str(path))


def _macos_profile(
    *,
    read_roots: list[Path],
    write_root: Path,
    home: Path,
    temporary: Path,
    executables: list[Path],
    protected: dict[str, dict[str, Any]],
    control_outputs: list[Path],
) -> str:
    # Darwin's loader and locale stack traverse paths which are not stable
    # across OS releases.  Start with read access, then remove every mutable
    # user/config/secret namespace and add back only the pinned roots.  Exec is
    # still deny-by-default and independently allowlisted below.
    sensitive_reads = [
        Path("/Users"),
        Path("/home"),
        Path("/root"),
        Path("/tmp"),
        Path("/private/tmp"),
        Path("/var/tmp"),
        Path("/private/var/tmp"),
        Path("/private/var/folders"),
        Path("/private/var/root"),
        Path("/private/etc/ssh"),
        Path("/etc/ssh"),
        Path("/private/etc/sudoers"),
        Path("/private/etc/master.passwd"),
        Path("/Library/Keychains"),
        Path("/private/var/db/dslocal"),
    ]
    lines = [
        "(version 1)",
        "(deny default)",
        "(deny file-link file-clone)",
        "(allow process-fork)",
        "(allow process-info*)",
        "(allow signal (target self))",
        "(allow sysctl-read)",
        "(allow mach-lookup)",
        "(allow file-read*)",
    ]
    for root in sensitive_reads:
        lines.append(f"(deny file-read* (subpath {_sb_quote(root)}))")
    for executable in executables:
        lines.append(f"(allow process-exec (literal {_sb_quote(executable)}))")
    for root in [*read_roots, write_root, home, temporary, *executables]:
        operation = "literal" if root.is_file() else "subpath"
        lines.append(f"(allow file-read* ({operation} {_sb_quote(root)}))")
        for parent in root.parents:
            if str(parent) == "/":
                continue
            lines.append(f"(allow file-read-metadata (literal {_sb_quote(parent)}))")
    for root in [write_root, home, temporary]:
        lines.append(f"(allow file-write* (subpath {_sb_quote(root)}))")
    for output in control_outputs:
        lines.append(f"(allow file-write-data (literal {_sb_quote(output)}))")
    # Toolchain utilities use the null device as a non-persistent sink.  It
    # cannot mutate host state and remains the only write exception outside
    # the bounded execution root.
    lines.append('(allow file-write* (literal "/dev/null"))')
    for relative, fact in protected.items():
        path = write_root / relative
        if fact["kind"] in {"file", "symlink"}:
            lines.append(f"(deny file-write* (literal {_sb_quote(path)}))")
        else:
            lines.append(f"(deny file-write-unlink (literal {_sb_quote(path)}))")
    # A read root nested beneath the writable build root remains immutable.
    for root in read_roots:
        if _within(root, write_root):
            lines.append(f"(deny file-write* (subpath {_sb_quote(root)}))")
    lines.append("(deny network*)")
    return "\n".join(lines) + "\n"


def _sanitized_environment(request: Any, *, home: Path, temporary: Path) -> dict[str, str]:
    if request is None:
        request = {}
    if not isinstance(request, dict):
        raise InvalidRequest("environment must be an object")
    unknown = sorted(set(request) - SAFE_ENV_KEYS)
    if unknown:
        raise InvalidRequest(f"environment key is not allowlisted: {unknown[0]}")
    for name, value in request.items():
        if FORBIDDEN_ENV_NAME.search(name) or not isinstance(value, str) or "\0" in value:
            raise InvalidRequest(f"unsafe environment entry: {name}")
    git_config = home / ".gitconfig"
    git_config.write_text("", encoding="utf-8")
    git_config.chmod(0o600)
    result = {
        "HOME": str(home),
        "TMPDIR": str(temporary),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": "",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": str(git_config),
        "GIT_CONFIG_COUNT": "1",
        "GIT_CONFIG_KEY_0": "credential.helper",
        "GIT_CONFIG_VALUE_0": "",
        "GIT_TERMINAL_PROMPT": "0",
        "SSH_AUTH_SOCK": "",
    }
    result.update(request)
    return result


def _validate_environment_paths(
    environment: dict[str, str],
    *,
    read_roots: list[Path],
    write_root: Path,
    executables: list[Path],
) -> None:
    sysroot = environment.get("LEAN_SYSROOT")
    if sysroot:
        root = _resolved_directory(sysroot, "LEAN_SYSROOT")
        if not any(root == allowed or _within(root, allowed) for allowed in read_roots):
            raise InvalidRequest("LEAN_SYSROOT must be inside a declared read root")
    lean_path = environment.get("LEAN_PATH")
    if lean_path:
        for value in lean_path.split(os.pathsep):
            root = _resolved_directory(value, "LEAN_PATH component")
            if not (
                root == write_root
                or _within(root, write_root)
                or any(root == allowed or _within(root, allowed) for allowed in read_roots)
            ):
                raise InvalidRequest("LEAN_PATH component is outside declared roots")
    path_value = environment.get("PATH", "")
    if path_value:
        allowed_parents = {executable.parent.resolve() for executable in executables}
        for value in path_value.split(os.pathsep):
            component = _resolved_directory(value, "PATH component")
            if component not in allowed_parents:
                raise InvalidRequest("PATH component is not an allowed executable directory")


def _preexec(limits: dict[str, int]) -> None:
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_CPU, (limits["cpu_seconds"], limits["cpu_seconds"]))
    if platform.system() != "Darwin":
        resource.setrlimit(resource.RLIMIT_AS, (limits["memory_bytes"], limits["memory_bytes"]))
    resource.setrlimit(resource.RLIMIT_FSIZE, (limits["single_file_bytes"], limits["single_file_bytes"]))
    resource.setrlimit(resource.RLIMIT_NOFILE, (limits["open_files"], limits["open_files"]))
    if platform.system() != "Darwin" and hasattr(resource, "RLIMIT_NPROC"):
        resource.setrlimit(resource.RLIMIT_NPROC, (limits["processes"], limits["processes"]))


def _secure_control_file(path: Path) -> int:
    flags = os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_APPEND
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return os.open(path, flags, 0o600)


def _write_control_file(descriptor: int, content: bytes) -> None:
    view = memoryview(content)
    while view:
        written = os.write(descriptor, view)
        if written < 1:
            raise OSError("short write to sandbox control file")
        view = view[written:]
    os.fsync(descriptor)


def _read_control_file(descriptor: int, limit: int) -> bytes:
    if limit <= 0:
        return b""
    return os.pread(descriptor, limit, 0)


def _terminate_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def _quiesce_group(process_group: int, timeout: float = 2.0) -> bool:
    """Kill and reap-visible poll a child process group before inspecting results."""
    deadline = time.monotonic() + timeout
    while True:
        try:
            os.killpg(process_group, signal.SIGKILL)
        except ProcessLookupError:
            return True
        processes, _memory_bytes = _process_metrics(process_group)
        if processes == 0:
            return True
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.01)


def _process_metrics(process_group: int) -> tuple[int | None, int | None]:
    try:
        result = subprocess.run(
            ["/bin/ps", "-axo", "pgid=,rss="],
            check=False,
            capture_output=True,
            timeout=2,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None, None
    if result.returncode != 0:
        return None, None
    count = 0
    rss_kib = 0
    for line in result.stdout.splitlines():
        fields = line.split()
        if len(fields) != 2 or fields[0] != str(process_group).encode("ascii"):
            continue
        count += 1
        try:
            rss_kib += int(fields[1])
        except ValueError:
            pass
    return count, rss_kib * 1024


def execute(request: dict[str, Any]) -> dict[str, Any]:
    if request.get("schema") != SCHEMA_REQUEST:
        raise InvalidRequest(f"request schema must be {SCHEMA_REQUEST}")
    command_value = request.get("command")
    if not isinstance(command_value, list) or not command_value or not all(
        isinstance(value, str) and "\0" not in value for value in command_value
    ):
        raise InvalidRequest("command must be a non-empty string array")
    executable = _resolved_executable(command_value[0])
    command = [str(executable), *command_value[1:]]
    read_value = request.get("read_roots")
    if not isinstance(read_value, list) or not read_value:
        raise InvalidRequest("read_roots must be a non-empty array")
    read_roots = [_resolved_directory(value, "read_root") for value in read_value]
    cwd = _resolved_directory(request.get("cwd"), "cwd")
    write_root = _resolved_directory(request.get("write_root"), "write_root")
    if any(_within(write_root, root) or _within(root, write_root) for root in read_roots):
        raise InvalidRequest("write_root must not overlap a read_root")
    if not _within(cwd, write_root) and not any(_within(cwd, root) for root in read_roots):
        raise InvalidRequest("cwd must be beneath a declared read or write root")
    executable_values = request.get("allowed_executables", [])
    if not isinstance(executable_values, list):
        raise InvalidRequest("allowed_executables must be an array")
    executables = list(dict.fromkeys(_resolved_executable(value) for value in executable_values))
    if executable not in executables:
        executables.insert(0, executable)
    execution_copy_values = request.get("execution_copy_roots", [])
    if not isinstance(execution_copy_values, list):
        raise InvalidRequest("execution_copy_roots must be an array")
    execution_copy_roots: list[Path] = []
    for value in execution_copy_values:
        if not isinstance(value, str) or not Path(value).is_absolute() or Path(value).is_symlink():
            raise InvalidRequest("execution copy root must be a non-symlink absolute directory")
        root = _resolved_directory(value, "execution_copy_root")
        if root == write_root or not _within(root, write_root):
            raise InvalidRequest("execution copy root must be strictly beneath write_root")
        _validate_execution_copy(root, read_roots)
        execution_copy_roots.append(root)
    limits = _limits(request.get("limits"))
    backend_name, backend_path = _backend()
    protected = _protected_snapshot(write_root, read_roots, execution_copy_roots)
    roles = request.get("root_roles", {})
    declared_identities = request.get("declared_identities", {})
    if not isinstance(roles, dict) or not isinstance(declared_identities, dict):
        raise InvalidRequest("root_roles and declared_identities must be objects")
    allowed_identity_keys = {str(root) for root in read_roots}
    if not set(roles).issubset(allowed_identity_keys) or not set(declared_identities).issubset(
        allowed_identity_keys
    ):
        raise InvalidRequest("root metadata contains an undeclared read root")
    if any(value not in {"source", "toolchain", "dependencies"} for value in roles.values()):
        raise InvalidRequest("unsupported read-root role")
    if any(
        not isinstance(value, dict)
        or not isinstance(value.get("kind"), str)
        or not isinstance(value.get("value"), str)
        or len(value["kind"]) > 64
        or len(value["value"]) > 256
        for value in declared_identities.values()
    ):
        raise InvalidRequest("declared root identities must contain bounded kind and value strings")
    runtime_boundary: Path | None = None
    control_boundary: Path | None = None
    stdout_descriptor: int | None = None
    stderr_descriptor: int | None = None
    profile_descriptor: int | None = None
    process: subprocess.Popen[bytes] | None = None
    try:
        runtime_boundary = Path(
            tempfile.mkdtemp(prefix="vela-external-lean-runtime-", dir=write_root)
        )
        control_boundary = Path(
            tempfile.mkdtemp(prefix="vela-external-lean-control-", dir="/private/var/tmp")
        )
        home = runtime_boundary / "home"
        temporary = runtime_boundary / "tmp"
        home.mkdir(mode=0o700)
        temporary.mkdir(mode=0o700)
        environment = _sanitized_environment(
            request.get("environment"), home=home, temporary=temporary
        )
        _validate_environment_paths(
            environment,
            read_roots=read_roots,
            write_root=write_root,
            executables=executables,
        )
        stdout_path = control_boundary / "stdout.bin"
        stderr_path = control_boundary / "stderr.bin"
        profile_path = control_boundary / "sandbox.sb"
        stdout_descriptor = _secure_control_file(stdout_path)
        stderr_descriptor = _secure_control_file(stderr_path)
        profile_descriptor = _secure_control_file(profile_path)
        baseline_bytes = _directory_bytes(write_root, read_roots)
        root_facts = [
            {
                "path": str(root),
                "role": roles.get(str(root), "dependencies"),
                "mode": "read_only",
                "tree_root": _tree_root(root),
                "root_source": "measured_tree",
                "declared_identity": declared_identities.get(str(root)),
            }
            for root in read_roots
        ]
        root_facts.append({"path": str(write_root), "mode": "bounded_write"})
        root_facts.extend(
            {
                "path": str(root),
                "role": "dependency_execution_copy",
                "mode": "bounded_write",
                "initial_tree_root": _tree_root(root),
                "root_source": "measured_tree",
            }
            for root in execution_copy_roots
        )
        profile = _macos_profile(
            read_roots=read_roots,
            write_root=write_root,
            home=home,
            temporary=temporary,
            executables=executables,
            protected=protected,
            control_outputs=[stdout_path, stderr_path],
        )
        _write_control_file(profile_descriptor, profile.encode("utf-8"))
        os.close(profile_descriptor)
        profile_descriptor = None
        sandbox_command = [str(backend_path), "-f", str(profile_path), *command]

        started = time.monotonic()
        limit_hit: str | None = None
        process = subprocess.Popen(
            sandbox_command,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout_descriptor,
            stderr=stderr_descriptor,
            start_new_session=True,
            preexec_fn=lambda: _preexec(limits),
        )
        while process.poll() is None:
            elapsed = time.monotonic() - started
            output_bytes = os.fstat(stdout_descriptor).st_size + os.fstat(stderr_descriptor).st_size
            try:
                disk_bytes = max(0, _directory_bytes(write_root, read_roots) - baseline_bytes)
            except InvalidRequest:
                disk_bytes = 0
                limit_hit = "monitor_unavailable"
            processes, memory_bytes = _process_metrics(process.pid)
            if elapsed > limits["wall_seconds"]:
                limit_hit = "wall_seconds"
            elif output_bytes > limits["output_bytes"]:
                limit_hit = "output_bytes"
            elif disk_bytes > limits["disk_bytes"]:
                limit_hit = "disk_bytes"
            elif limit_hit == "monitor_unavailable":
                pass
            elif processes is None or memory_bytes is None:
                limit_hit = "monitor_unavailable"
            elif processes > limits["processes"]:
                limit_hit = "processes"
            elif memory_bytes > limits["memory_bytes"]:
                limit_hit = "memory_bytes"
            if limit_hit:
                _terminate_group(process)
                break
            time.sleep(0.025)
        returncode = process.wait()
        # The leader may exit while descendants keep the process group alive.
        # End the whole group before reading outputs or measuring final roots.
        _terminate_group(process)
        process_group_quiescent = _quiesce_group(process.pid)
        if not process_group_quiescent and limit_hit is None:
            limit_hit = "process_group_not_quiescent"
        elapsed_ms = int((time.monotonic() - started) * 1000)
        raw_stdout = _read_control_file(stdout_descriptor, limits["output_bytes"])
        remaining = max(0, limits["output_bytes"] - len(raw_stdout))
        raw_stderr = _read_control_file(stderr_descriptor, remaining)
        final_disk_bytes = max(0, _directory_bytes(write_root, read_roots) - baseline_bytes)
        if limit_hit in {None, "monitor_unavailable"}:
            if len(raw_stdout) + len(raw_stderr) > limits["output_bytes"]:
                limit_hit = "output_bytes"
            elif final_disk_bytes > limits["disk_bytes"]:
                limit_hit = "disk_bytes"
        protected_unchanged = _protected_snapshot_unchanged(write_root, protected)
        canonical_inputs_unchanged = True
        for fact, root in zip(root_facts[: len(read_roots)], read_roots, strict=True):
            fact["post_tree_root"] = _tree_root(root)
            fact["unchanged"] = fact["post_tree_root"] == fact["tree_root"]
            canonical_inputs_unchanged = canonical_inputs_unchanged and fact["unchanged"]
        if execution_copy_roots:
            for fact, root in zip(
                root_facts[-len(execution_copy_roots):], execution_copy_roots, strict=True
            ):
                fact["post_tree_root"] = _tree_root(root)
        result = {
            "schema": SCHEMA_RESULT,
            "ok": (
                returncode == 0
                and limit_hit is None
                and protected_unchanged
                and canonical_inputs_unchanged
            ),
            "sandbox": {
                "backend": backend_name,
                "fail_closed": True,
                "network": "denied",
                "environment": "allowlisted_and_scrubbed",
                "home": "empty_temporary",
                "canonical_roots": "read_only",
                "execution_root": "bounded_write",
                "parent_control": "outside_child_visible_roots_fd_only",
                "executables": [str(path) for path in executables],
                "blocked_capabilities": BLOCKED_CAPABILITIES,
            },
            "limits": limits,
            "roots": root_facts,
            "command": command,
            "environment_keys": sorted(environment),
            "exit_code": returncode,
            "process_group": process.pid,
            "limit_hit": limit_hit,
            "wall_time_ms": elapsed_ms,
            "disk_delta_bytes": final_disk_bytes,
            "protected_input_root": "sha256:" + _sha256_bytes(_canonical_bytes(protected)),
            "protected_inputs_unchanged": protected_unchanged,
            "canonical_inputs_unchanged": canonical_inputs_unchanged,
            "process_group_quiescent": process_group_quiescent,
            "stdout": _safe_text(raw_stdout, max_bytes=limits["output_bytes"]),
            "stderr": _safe_text(raw_stderr, max_bytes=limits["output_bytes"]),
        }
        if limit_hit == "monitor_unavailable":
            result["error"] = {
                "code": "monitor_unavailable",
                "text": "sandbox process or memory measurement was unavailable",
            }
        elif not canonical_inputs_unchanged:
            result["error"] = {
                "code": "canonical_input_mutated",
                "text": "a measured canonical read root changed during sandbox execution",
            }
        elif not protected_unchanged:
            result["error"] = {
                "code": "protected_input_mutated",
                "text": "the sandbox did not preserve a pre-existing write-root input",
            }
        elif not process_group_quiescent:
            result["error"] = {
                "code": "process_group_not_quiescent",
                "text": "sandbox descendants remained alive after bounded process-group cleanup",
            }
        result["result_root"] = "sha256:" + _sha256_bytes(_canonical_bytes(result))
        return result
    finally:
        if process is not None:
            _terminate_group(process)
            if process.poll() is None:
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            _quiesce_group(process.pid)
        for descriptor in (profile_descriptor, stdout_descriptor, stderr_descriptor):
            if descriptor is not None:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        if runtime_boundary is not None:
            shutil.rmtree(runtime_boundary, ignore_errors=True)
        if control_boundary is not None:
            shutil.rmtree(control_boundary, ignore_errors=True)


def fail_closed_result(exc: Exception) -> dict[str, Any]:
    if isinstance(exc, SandboxUnavailable):
        code = "sandbox_unavailable"
    elif isinstance(exc, (InvalidRequest, OSError, subprocess.SubprocessError)):
        code = "invalid_request"
    else:
        code = "internal_error"
    result = {
        "schema": SCHEMA_RESULT,
        "ok": False,
        "sandbox": {
            "backend": None,
            "fail_closed": True,
            "blocked_capabilities": BLOCKED_CAPABILITIES,
        },
        "error": {
            "code": code,
            "text": str(exc),
        },
    }
    result["result_root"] = "sha256:" + _sha256_bytes(_canonical_bytes(result))
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", required=True)
    args = parser.parse_args(argv)
    try:
        request = json.loads(Path(args.request).read_text(encoding="utf-8"))
        if not isinstance(request, dict):
            raise InvalidRequest("request must be a JSON object")
        result = execute(request)
    # The executable boundary always emits one bounded JSON object.  Direct
    # callers of execute() still receive programmer errors normally, so tests
    # do not accidentally turn implementation defects into expected results.
    except Exception as exc:
        result = fail_closed_result(exc)
    sys.stdout.buffer.write(_canonical_bytes(result))
    return 0 if result.get("ok") else 1


if __name__ == "__main__":
    raise SystemExit(main())
