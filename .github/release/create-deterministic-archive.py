#!/usr/bin/env python3
"""Create a byte-reproducible Vela archive from one staged directory."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import os
import stat
import tarfile
import time
import zipfile
from pathlib import Path


def files(source: Path) -> list[Path]:
    selected = []
    for path in sorted(source.rglob("*")):
        relative = path.relative_to(source)
        if path.is_symlink() or (not path.is_dir() and not path.is_file()):
            raise SystemExit(f"unsupported staged path: {relative}")
        if path.is_file():
            selected.append(path)
    if not selected:
        raise SystemExit("staged directory has no files")
    return selected


def archive_tar(source: Path, output: Path, epoch: int) -> None:
    with (
        output.open("wb") as raw,
        gzip.GzipFile(
            filename="", mode="wb", fileobj=raw, mtime=epoch, compresslevel=9
        ) as zipped,
        tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as archive,
    ):
        for path in files(source):
            relative = path.relative_to(source).as_posix()
            info = tarfile.TarInfo(relative)
            info.size = path.stat().st_size
            info.mode = stat.S_IMODE(path.stat().st_mode)
            info.mtime = epoch
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            with path.open("rb") as payload:
                archive.addfile(info, payload)


def archive_zip(source: Path, output: Path, epoch: int) -> None:
    timestamp = time.gmtime(max(epoch, 315532800))[:6]
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files(source):
            relative = path.relative_to(source).as_posix()
            info = zipfile.ZipInfo(relative, date_time=timestamp)
            info.create_system = 3
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (stat.S_IFREG | stat.S_IMODE(path.stat().st_mode)) << 16
            with path.open("rb") as payload:
                archive.writestr(info, payload.read(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--epoch", required=True, type=int)
    arguments = parser.parse_args()
    source = arguments.source.resolve()
    output = arguments.output.resolve()
    if not source.is_dir():
        raise SystemExit(f"no staged directory: {source}")
    if arguments.epoch < 0:
        raise SystemExit("epoch must be nonnegative")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp-{os.getpid()}")
    temporary.unlink(missing_ok=True)
    try:
        if output.name.endswith(".tar.gz"):
            archive_tar(source, temporary, arguments.epoch)
        elif output.name.endswith(".zip"):
            archive_zip(source, temporary, arguments.epoch)
        else:
            raise SystemExit("output must end in .tar.gz or .zip")
        temporary.replace(output)
    finally:
        temporary.unlink(missing_ok=True)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    print(f"{digest}  {output.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
