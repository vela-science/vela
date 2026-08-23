"""Descriptor-relative, race-detecting reads for immutable evidence files."""

from __future__ import annotations

import os
import stat
from collections.abc import Callable
from pathlib import Path
from typing import TypeVar

T = TypeVar("T")


def _identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        stat.S_IFMT(value.st_mode),
        value.st_nlink,
        value.st_size,
    )


def read_regular(
    root: Path,
    relative: Path | str,
    label: str,
    *,
    error_type: type[Exception] = ValueError,
    validator: Callable[[bytes], T] | None = None,
) -> bytes | tuple[bytes, T]:
    """Read one single-link file while binding its complete named path.

    Every path component is opened descriptor-relatively without following
    symlinks. The named identity observed immediately before the final open is
    required to equal the opened descriptor, the descriptor after reading, and
    the name after reading. The descriptor remains open while ``validator``
    inspects the bytes.
    """

    relative = Path(relative)
    if (
        relative.is_absolute()
        or not relative.parts
        or any(part in {"", ".", ".."} for part in relative.parts)
    ):
        raise error_type(f"{label}_path_unsafe")
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | nofollow
    try:
        directory = os.open(root, directory_flags)
    except OSError as error:
        raise error_type(f"{label}_root_missing_or_unsafe") from error
    descriptor = -1
    try:
        root_metadata = os.fstat(directory)
        if not stat.S_ISDIR(root_metadata.st_mode):
            raise error_type(f"{label}_root_not_directory")
        for part in relative.parts[:-1]:
            before = os.stat(part, dir_fd=directory, follow_symlinks=False)
            if not stat.S_ISDIR(before.st_mode) or before.st_nlink < 1:
                raise error_type(f"{label}_directory_not_real")
            child = os.open(part, directory_flags, dir_fd=directory)
            opened = os.fstat(child)
            after = os.stat(part, dir_fd=directory, follow_symlinks=False)
            if _identity(before) != _identity(opened) or _identity(opened) != _identity(
                after
            ):
                os.close(child)
                raise error_type(f"{label}_directory_custody_drift")
            os.close(directory)
            directory = child

        name = relative.parts[-1]
        named_before = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if not stat.S_ISREG(named_before.st_mode) or named_before.st_nlink != 1:
            raise error_type(f"{label}_not_regular_single_link")
        descriptor = os.open(name, os.O_RDONLY | nofollow, dir_fd=directory)
        opened = os.fstat(descriptor)
        if _identity(named_before) != _identity(opened):
            raise error_type(f"{label}_changed_before_open")
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        raw = b"".join(chunks)
        validated = validator(raw) if validator is not None else None
        after_read = os.fstat(descriptor)
        named_after = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if (
            _identity(named_before) != _identity(opened)
            or _identity(opened) != _identity(after_read)
            or _identity(after_read) != _identity(named_after)
            or after_read.st_nlink != 1
            or len(raw) != after_read.st_size
        ):
            raise error_type(f"{label}_custody_drift")
        return (raw, validated) if validator is not None else raw
    except OSError as error:
        raise error_type(f"{label}_missing_or_unsafe") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        os.close(directory)


def read_absolute_regular(
    path: Path,
    label: str,
    *,
    error_type: type[Exception] = ValueError,
    validator: Callable[[bytes], T] | None = None,
) -> bytes | tuple[bytes, T]:
    """Read an absolute path while binding every component from filesystem root."""

    absolute = path.absolute()
    try:
        final_metadata = os.lstat(absolute)
    except OSError as error:
        raise error_type(f"{label}_missing_or_unsafe") from error
    if stat.S_ISLNK(final_metadata.st_mode):
        raise error_type(f"{label}_not_regular_single_link")
    # macOS exposes stable system aliases such as /var -> /private/var. Bind
    # the canonical parent descriptor, then apply the full four-way identity
    # check to the final name without ever following that final name.
    canonical_parent = absolute.parent.resolve(strict=True)
    return read_regular(
        canonical_parent,
        Path(absolute.name),
        label,
        error_type=error_type,
        validator=validator,
    )
