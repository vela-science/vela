#!/usr/bin/env python3
"""Canonicalize Syft output and fail if its Rust dependency graph is incomplete."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

REQUIRED = {
    "vela-authority",
    "vela-cli",
    "vela-protocol",
    "vela-repository",
}


def check(path: pathlib.Path) -> int:
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


def canonicalize(arguments: argparse.Namespace) -> int:
    source = pathlib.Path(arguments.input)
    document = json.loads(source.read_text(encoding="utf-8"))
    if document.get("spdxVersion") != "SPDX-2.3":
        raise SystemExit(f"{source}: expected SPDX-2.3")
    packages = document.get("packages")
    if not isinstance(packages, list):
        raise SystemExit(f"{source}: SPDX document has no package inventory")
    roots = [
        package
        for package in packages
        if isinstance(package, dict)
        and isinstance(package.get("SPDXID"), str)
        and package["SPDXID"].startswith("SPDXRef-DocumentRoot-")
    ]
    if len(roots) != 1:
        raise SystemExit(f"{source}: expected exactly one Syft document-root package")

    root = roots[0]
    old_root_id = root["SPDXID"]
    relationships = document.get("relationships")
    if not isinstance(relationships, list):
        raise SystemExit(f"{source}: SPDX document has no relationships")
    for relationship in relationships:
        if not isinstance(relationship, dict):
            raise SystemExit(f"{source}: SPDX relationship is not an object")
        for field in ("spdxElementId", "relatedSpdxElement"):
            if relationship.get(field) == old_root_id:
                relationship[field] = arguments.root_id

    creation = document.get("creationInfo")
    if not isinstance(creation, dict):
        raise SystemExit(f"{source}: SPDX document has no creation information")
    document["name"] = arguments.name
    document["documentNamespace"] = arguments.namespace
    creation["created"] = arguments.created
    root["name"] = arguments.root_name
    root["SPDXID"] = arguments.root_id

    rendered = json.dumps(
        document, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    )
    if old_root_id != arguments.root_id and old_root_id in rendered:
        raise SystemExit(
            f"{source}: stale Syft document-root identity remains after normalization"
        )
    output = pathlib.Path(arguments.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered + "\n", encoding="utf-8")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] != "--canonicalize":
        return check(pathlib.Path(sys.argv[1]))
    parser = argparse.ArgumentParser(prog="check-sbom")
    parser.add_argument("--canonicalize", action="store_true", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--namespace", required=True)
    parser.add_argument("--created", required=True)
    parser.add_argument("--root-name", required=True)
    parser.add_argument("--root-id", required=True)
    return canonicalize(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
