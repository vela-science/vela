"""Descriptor-relative, race-detecting reads for immutable evidence files."""

from __future__ import annotations

import hashlib
import os
import stat
from collections.abc import Callable, Iterable
from pathlib import Path
from typing import TypeVar

T = TypeVar("T")
FileIdentity = tuple[int, int]


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        stat.S_IFMT(value.st_mode),
        value.st_nlink,
        value.st_size,
    )


def _absolute_parts(
    path: Path, label: str, error_type: type[Exception]
) -> tuple[str, ...]:
    raw = os.fspath(path)
    if not path.is_absolute() or os.path.normpath(raw) != raw:
        raise error_type(f"{label}_trusted_root_not_canonical_absolute")
    return tuple(part for part in path.parts if part != path.anchor)


def read_regular(
    root: Path,
    relative: Path | str,
    label: str,
    *,
    error_type: type[Exception] = ValueError,
    validator: Callable[[bytes], T] | None = None,
    expected_bytes: int | None = None,
    expected_sha256: str | None = None,
    identity_registry: set[FileIdentity] | None = None,
) -> bytes | tuple[bytes, T]:
    """Read one registered file from an explicit trusted filesystem root.

    The trusted root and every descendant component are traversed from an open
    filesystem-root descriptor. No pathname is resolved or opened absolutely.
    Every directory descriptor remains open until the final bytes have been
    validated, and every parent/name binding is checked again afterwards. The
    final file is bound across named-before, opened-FD, post-read-FD, and
    named-after identities.
    """

    root = Path(root)
    root_parts = _absolute_parts(root, label, error_type)
    relative = Path(relative)
    if (
        relative.is_absolute()
        or not relative.parts
        or any(part in {"", ".", ".."} for part in relative.parts)
    ):
        raise error_type(f"{label}_path_unsafe")

    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | nofollow
    descriptors: list[int] = []
    directory_bindings: list[tuple[int, str, int, tuple[int, int, int, int, int]]] = []
    file_descriptor = -1
    try:
        filesystem_root = os.open(os.sep, directory_flags)
        descriptors.append(filesystem_root)
        filesystem_root_before = os.fstat(filesystem_root)
        if not stat.S_ISDIR(filesystem_root_before.st_mode):
            raise error_type(f"{label}_filesystem_root_not_directory")

        directory = filesystem_root
        directory_names = (*root_parts, *relative.parts[:-1])
        for part in directory_names:
            before = os.stat(part, dir_fd=directory, follow_symlinks=False)
            if not stat.S_ISDIR(before.st_mode) or before.st_nlink < 1:
                raise error_type(f"{label}_directory_not_real")
            child = os.open(part, directory_flags, dir_fd=directory)
            descriptors.append(child)
            opened = os.fstat(child)
            after = os.stat(part, dir_fd=directory, follow_symlinks=False)
            bound = _identity(opened)
            if _identity(before) != bound or bound != _identity(after):
                raise error_type(f"{label}_directory_custody_drift")
            directory_bindings.append((directory, part, child, bound))
            directory = child

        name = relative.parts[-1]
        named_before = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if not stat.S_ISREG(named_before.st_mode) or named_before.st_nlink != 1:
            raise error_type(f"{label}_not_regular_single_link")
        file_descriptor = os.open(name, os.O_RDONLY | nofollow, dir_fd=directory)
        opened = os.fstat(file_descriptor)
        if _identity(named_before) != _identity(opened):
            raise error_type(f"{label}_changed_before_open")

        file_identity = (opened.st_dev, opened.st_ino)
        if identity_registry is not None and file_identity in identity_registry:
            raise error_type(f"{label}_duplicate_path_identity")

        chunks: list[bytes] = []
        while chunk := os.read(file_descriptor, 1024 * 1024):
            chunks.append(chunk)
        raw = b"".join(chunks)
        if expected_bytes is not None and (
            type(expected_bytes) is not int
            or expected_bytes < 0
            or len(raw) != expected_bytes
        ):
            raise error_type(f"{label}_byte_length_mismatch")
        if expected_sha256 is not None:
            observed_sha256 = hashlib.sha256(raw).hexdigest()
            normalized_sha256 = (
                expected_sha256.removeprefix("sha256:")
                if isinstance(expected_sha256, str)
                else None
            )
            if normalized_sha256 != observed_sha256:
                raise error_type(f"{label}_sha256_mismatch")
        validated = validator(raw) if validator is not None else None

        after_read = os.fstat(file_descriptor)
        named_after = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if (
            _identity(named_before) != _identity(opened)
            or _identity(opened) != _identity(after_read)
            or _identity(after_read) != _identity(named_after)
            or after_read.st_nlink != 1
            or len(raw) != after_read.st_size
        ):
            raise error_type(f"{label}_custody_drift")

        for parent, part, child, bound in reversed(directory_bindings):
            child_after = os.fstat(child)
            named_child_after = os.stat(part, dir_fd=parent, follow_symlinks=False)
            if (
                not stat.S_ISDIR(child_after.st_mode)
                or child_after.st_nlink < 1
                or _identity(child_after) != bound
                or _identity(named_child_after) != bound
            ):
                raise error_type(f"{label}_parent_custody_drift")
        if _identity(os.fstat(filesystem_root)) != _identity(filesystem_root_before):
            raise error_type(f"{label}_filesystem_root_custody_drift")

        if identity_registry is not None:
            identity_registry.add(file_identity)
        return (raw, validated) if validator is not None else raw
    except OSError as error:
        raise error_type(f"{label}_missing_or_unsafe") from error
    finally:
        if file_descriptor >= 0:
            os.close(file_descriptor)
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def read_absolute_regular(
    path: Path,
    label: str,
    *,
    trusted_roots: Iterable[Path],
    error_type: type[Exception] = ValueError,
    validator: Callable[[bytes], T] | None = None,
    expected_bytes: int | None = None,
    expected_sha256: str | None = None,
    identity_registry: set[FileIdentity] | None = None,
) -> bytes | tuple[bytes, T]:
    """Read an absolute registered path beneath exactly one trusted root."""

    path = Path(path)
    raw_path = os.fspath(path)
    if not path.is_absolute() or os.path.normpath(raw_path) != raw_path:
        raise error_type(f"{label}_path_not_canonical_absolute")
    candidates: list[tuple[Path, Path]] = []
    for root in trusted_roots:
        root = Path(root)
        _absolute_parts(root, label, error_type)
        try:
            relative = path.relative_to(root)
        except ValueError:
            continue
        if relative.parts:
            candidates.append((root, relative))
    if not candidates:
        raise error_type(f"{label}_outside_trusted_roots")
    candidates.sort(key=lambda candidate: len(candidate[0].parts), reverse=True)
    if len(candidates) > 1 and len(candidates[0][0].parts) == len(
        candidates[1][0].parts
    ):
        raise error_type(f"{label}_ambiguous_trusted_root")
    root, relative = candidates[0]
    return read_regular(
        root,
        relative,
        label,
        error_type=error_type,
        validator=validator,
        expected_bytes=expected_bytes,
        expected_sha256=expected_sha256,
        identity_registry=identity_registry,
    )
