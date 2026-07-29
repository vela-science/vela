#!/usr/bin/env python3
"""Create a deterministic gzip-compressed tar of a reference package."""

from __future__ import annotations

import argparse
import gzip
import io
import tarfile
from pathlib import Path


def add_file(archive: tarfile.TarFile, root: Path, path: Path) -> None:
    relative = path.relative_to(root).as_posix()
    data = path.read_bytes()
    info = tarfile.TarInfo(relative)
    info.size = len(data)
    info.mode = 0o644
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    archive.addfile(info, io.BytesIO(data))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    package = args.package_root.expanduser().resolve()
    output = args.output.expanduser().resolve()
    included = [package / "reference.v1.json"]
    included.extend(
        sorted(path for path in (package / "objects").rglob("*") if path.is_file())
    )
    if any(not path.is_file() for path in included):
        raise SystemExit("reference package is incomplete")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for path in included:
                    add_file(archive, package, path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
