#!/usr/bin/env python3
"""Fail unless a staged or unpacked release carries exact required notices."""

from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path

PROJECT_LICENSES = ("LICENSE", "LICENSE-APACHE", "LICENSE-MIT")
THIRD_PARTY = "THIRD-PARTY-LICENSES.txt"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_line(payload: str, label: str, value: str) -> None:
    expected = f"{label}: {value}"
    if expected not in payload.splitlines():
        raise SystemExit(f"notice bundle: missing exact line {expected!r}")


def main() -> int:
    parser = argparse.ArgumentParser(prog="check-notice-bundle")
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--cargo-about-version", required=True)
    arguments = parser.parse_args()
    bundle = arguments.bundle.resolve()
    source = arguments.source_root.resolve()
    if not bundle.is_dir():
        raise SystemExit(f"notice bundle: missing directory {bundle}")

    for name in (*PROJECT_LICENSES, THIRD_PARTY):
        packaged = bundle / name
        if not packaged.is_file() or packaged.is_symlink():
            raise SystemExit(f"notice bundle: missing required regular file {name}")
        if not packaged.read_bytes():
            raise SystemExit(f"notice bundle: required file is empty: {name}")
    for name in PROJECT_LICENSES:
        if (bundle / name).read_bytes() != (source / name).read_bytes():
            raise SystemExit(f"notice bundle: packaged {name} differs from source")

    payload = (bundle / THIRD_PARTY).read_text(encoding="utf-8")
    require_line(payload, "Format", "vela.third-party-notices.v1")
    require_line(
        payload,
        "Generator",
        f"cargo-about {arguments.cargo_about_version} (--frozen --fail)",
    )
    require_line(payload, "Cargo.lock sha256", sha256(source / "Cargo.lock"))
    require_line(
        payload,
        "about.toml sha256",
        sha256(source / ".github/release/about.toml"),
    )
    require_line(payload, "deny.toml sha256", sha256(source / "deny.toml"))
    for label in ("Package count", "License text count"):
        match = re.search(
            rf"^{re.escape(label)}: ([1-9][0-9]*)$", payload, re.MULTILINE
        )
        if match is None:
            raise SystemExit(f"notice bundle: missing positive {label.lower()}")
    if "LICENSE TEXTS (" not in payload or "ADDITIONAL PACKAGE NOTICES (" not in payload:
        raise SystemExit("notice bundle: generated license or notice sections are missing")
    print(f"release notice bundle: ok ({bundle})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
