#!/usr/bin/env python3
"""Prove removable projections cannot mutate accepted Vela or Git state."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[1]
FRONTIER = REPOSITORY / "examples/erdos-formalization"
REFERENCE = ROOT / "reference"
PROJECTIONS = (
    "fact_manifest.py",
    "resolve_fact_manifest.py",
    "correction_ci.py",
    "accepted_context_pack.py",
    "projection_cli.py",
    "reader_c.py",
    "offline_bundle_inspection.py",
)
ROOT_PATTERN = re.compile(
    r"^(snapshot_hash|event_log_hash|proposal_state_hash|sources_hash|"
    r"artifacts_hash|review_hash): (sha256:[0-9a-f]{64})$",
    re.MULTILINE,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def run(command: list[str], *, cwd: Path) -> str:
    environment = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": "/nonexistent",
        "LANG": "C",
        "LC_ALL": "C",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0",
    }
    result = subprocess.run(
        command,
        cwd=cwd,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed {command!r}: {result.stderr.strip()}")
    return result.stdout.strip()


def accepted_roots(frontier: Path) -> dict[str, str]:
    text = (frontier / "vela.lock").read_text(encoding="utf-8")
    roots = dict(ROOT_PATTERN.findall(text))
    require(len(roots) == 6, "lockfile accepted-root set incomplete")
    return roots


def canonical_bytes_root(frontier: Path) -> str:
    digest = hashlib.sha256()
    paths = [
        path
        for path in frontier.rglob("*")
        if path.is_file()
        and (
            path.relative_to(frontier).as_posix().startswith(".vela/")
            or path.name in {"frontier.json", "vela.lock"}
            or path.relative_to(frontier).as_posix().startswith("proof/")
        )
    ]
    for path in sorted(paths):
        relative = path.relative_to(frontier).as_posix().encode()
        raw = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(raw).to_bytes(8, "big"))
        digest.update(raw)
    return f"sha256:{digest.hexdigest()}"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="vela-adr4-deletion-") as raw:
        repository = Path(raw) / "repository"
        frontier = repository / "frontier"
        shutil.copytree(FRONTIER, frontier, symlinks=True)
        projection_dir = repository / "derived-projections"
        projection_dir.mkdir(parents=True)
        for name in PROJECTIONS:
            shutil.copy2(REFERENCE / name, projection_dir / name)

        run(["git", "init", "-q", "-b", "main"], cwd=repository)
        run(["git", "add", "."], cwd=repository)
        run(
            [
                "git",
                "-c",
                "user.name=ADR4 Deletion Fixture",
                "-c",
                "user.email=adr4@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "accepted state plus removable projections",
            ],
            cwd=repository,
        )

        before = {
            "head": run(["git", "rev-parse", "HEAD"], cwd=repository),
            "tree": run(["git", "rev-parse", "HEAD^{tree}"], cwd=repository),
            "index": run(["git", "write-tree"], cwd=repository),
            "accepted_roots": accepted_roots(frontier),
            "canonical_bytes_root": canonical_bytes_root(frontier),
        }
        for path in projection_dir.iterdir():
            path.unlink()
        projection_dir.rmdir()
        after = {
            "head": run(["git", "rev-parse", "HEAD"], cwd=repository),
            "tree": run(["git", "rev-parse", "HEAD^{tree}"], cwd=repository),
            "index": run(["git", "write-tree"], cwd=repository),
            "accepted_roots": accepted_roots(frontier),
            "canonical_bytes_root": canonical_bytes_root(frontier),
        }
        require(before == after, "projection deletion changed accepted or Git state")
        status = run(["git", "status", "--short"], cwd=repository).splitlines()
        require(status, "projection deletion did not register as removable porcelain")
        require(
            all(line.split()[-1].startswith("derived-projections/") for line in status),
            f"projection deletion touched canonical frontier paths: {status}",
        )

    print(
        "projection deletion: accepted roots, canonical frontier bytes, "
        "HEAD tree, and index tree unchanged"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
