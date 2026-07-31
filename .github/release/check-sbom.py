#!/usr/bin/env python3
"""Fail a release whose SPDX inventory lost the embedded Rust dependency graph."""

from __future__ import annotations

import json
import pathlib
import sys


REQUIRED = {
    "vela-authority",
    "vela-cli",
    "vela-edge",
    "vela-protocol",
    "vela-verify",
}


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: check-sbom.py <spdx.json>")
    path = pathlib.Path(sys.argv[1])
    document = json.loads(path.read_text(encoding="utf-8"))
    packages = document.get("packages")
    if not isinstance(packages, list):
        raise SystemExit(f"{path}: SPDX document has no package inventory")
    names = {
        package.get("name")
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }
    missing = sorted(REQUIRED - names)
    if missing:
        raise SystemExit(f"{path}: missing Vela packages: {', '.join(missing)}")
    if len(names) < 20:
        raise SystemExit(
            f"{path}: only {len(names)} package names; auditable dependency recovery failed"
        )
    print(f"SBOM dependency graph: ok ({len(names)} package names)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
