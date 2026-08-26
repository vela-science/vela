#!/usr/bin/env python3
"""Write the exact locked normal-dependency graph for one release target."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

import tomllib

FORMAT = "vela.release-selected-packages.v1"
REQUIRED_WORKSPACE = {
    "vela-authority",
    "vela-cli",
    "vela-protocol",
    "vela-repository",
}
TREE_LINE = re.compile(r"^(?P<depth>[0-9]+)(?P<name>\S+) v(?P<version>\S+)(?P<rest>.*)$")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def lock_index(path: Path) -> dict[tuple[str, str], list[dict[str, object]]]:
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    packages = document.get("package")
    if not isinstance(packages, list) or not packages:
        raise SystemExit(f"selected release graph: {path} has no package list")
    result: dict[tuple[str, str], list[dict[str, object]]] = {}
    for package in packages:
        if not isinstance(package, dict):
            raise SystemExit(f"selected release graph: malformed package in {path}")
        name = package.get("name")
        version = package.get("version")
        if isinstance(name, str) and isinstance(version, str):
            result.setdefault((name, version), []).append(package)
    return result


def selected_packages(cargo_lock: Path, target: str) -> list[dict[str, object]]:
    command = [
        "cargo",
        "tree",
        "--color",
        "never",
        "--locked",
        "--offline",
        "-p",
        "vela-cli",
        "--target",
        target,
        "-e",
        "normal",
        "--prefix",
        "depth",
        "--no-dedupe",
        "--charset",
        "ascii",
        "--format",
        "{p}",
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        if completed.stdout:
            print(completed.stdout, end="")
        if completed.stderr:
            print(completed.stderr, end="", file=sys.stderr)
        raise SystemExit("selected release graph: cargo tree failed")

    locked = lock_index(cargo_lock)
    classifications: dict[tuple[str, str], set[str]] = {}
    proc_macro_stack: list[bool] = []
    for line in completed.stdout.splitlines():
        match = TREE_LINE.fullmatch(line)
        if match is None:
            raise SystemExit(f"selected release graph: cannot parse cargo tree line {line!r}")
        depth = int(match.group("depth"))
        if depth > len(proc_macro_stack):
            raise SystemExit("selected release graph: cargo tree depth jumped unexpectedly")
        del proc_macro_stack[depth:]
        is_proc_macro = "(proc-macro)" in match.group("rest")
        build_context = is_proc_macro or any(proc_macro_stack)
        proc_macro_stack.append(build_context)
        identity = (match.group("name"), match.group("version"))
        classifications.setdefault(identity, set()).add(
            "build-contributor" if build_context else "contained"
        )

    rows = []
    for (name, version), kinds in classifications.items():
        entries = locked.get((name, version), [])
        if len(entries) != 1:
            raise SystemExit(
                "selected release graph: Cargo.lock identity is not unique for "
                f"{name} {version}"
            )
        entry = entries[0]
        source = entry.get("source")
        workspace = source is None
        checksum = entry.get("checksum")
        if not workspace and (
            not isinstance(source, str)
            or not isinstance(checksum, str)
            or re.fullmatch(r"[0-9a-f]{64}", checksum) is None
        ):
            raise SystemExit(
                f"selected release graph: incomplete lock binding for {name} {version}"
            )
        rows.append(
            {
                "classification": "contained"
                if "contained" in kinds
                else "build-contributor",
                "name": name,
                "version": version,
                "workspace": workspace,
                **({"checksum": checksum, "source": source} if not workspace else {}),
            }
        )
    rows.sort(key=lambda row: (str(row["name"]), str(row["version"])))
    workspace_names = {str(row["name"]) for row in rows if row["workspace"]}
    if workspace_names != REQUIRED_WORKSPACE:
        raise SystemExit(
            "selected release graph: unexpected workspace packages: "
            + ", ".join(sorted(workspace_names))
        )
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(prog="selected-release-packages")
    parser.add_argument("--cargo-lock", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    rows = selected_packages(arguments.cargo_lock, arguments.target)
    document = {
        "format": FORMAT,
        "cargo_lock_sha256": sha256(arguments.cargo_lock),
        "target": arguments.target,
        "packages": rows,
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    third_party = sum(not bool(row["workspace"]) for row in rows)
    print(
        f"selected release graph: {arguments.target} "
        f"({third_party} third-party, {len(rows) - third_party} workspace)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
