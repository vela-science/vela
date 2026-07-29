#!/usr/bin/env python3
"""Verify the exact Formal Conjectures source transition used by the Erdős 424 fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path


OLD_COMMIT = "e751934294a381afd2d5fc1124c5953c8e25f9fa"
OLD_TREE = "571f59471e87f66e33dd734eb6794125b0c1f8c8"
OLD_FILE_ROOT = "sha256:05876ee2a0d7e75f72af414dbb6c415212cfa04ef3941dfbd5d9f7c87e6a30d9"
OLD_STATEMENT = "theorem erdos_424 : answer(sorry) ↔ generatedSet.HasPosDensity := by"

NEW_COMMIT = "8046fbff7b6c801d8debd4a85bf67a0541b78dda"
NEW_TREE = "774933a12da7926492e0e08cbd46ccbf99178cf8"
NEW_FILE_ROOT = "sha256:6b425608614ac52f32a09bfa2a8ad989edef9fb21bbd599ef7bdcc2814b373a2"
NEW_STATEMENT = "theorem erdos_424 : answer(sorry) ↔ generatedSet.HasPosLowerDensity := by"

SOURCE_PATH = "FormalConjectures/ErdosProblems/424.lean"
ARTIFACT_ROOT = "sha256:d18024c4333f77144955adf0036ce831e71b331ea7d9cc9cb69958f960f56d6c"


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load_artifact(path: Path) -> tuple[bytes, dict[str, object]]:
    encoded = path.read_bytes()
    require(sha256(encoded) == ARTIFACT_ROOT, "source-diff artifact root mismatch")
    value = json.loads(encoded)
    require(isinstance(value, dict), "source-diff artifact must be a JSON object")
    return encoded, value


def verify(repo: Path, artifact_path: Path) -> dict[str, object]:
    artifact_bytes, artifact = load_artifact(artifact_path)
    old_tree = git(repo, "rev-parse", f"{OLD_COMMIT}^{{tree}}").decode().strip()
    new_tree = git(repo, "rev-parse", f"{NEW_COMMIT}^{{tree}}").decode().strip()
    require(old_tree == OLD_TREE, "predecessor tree mismatch")
    require(new_tree == NEW_TREE, "successor tree mismatch")

    old_bytes = git(repo, "show", f"{OLD_COMMIT}:{SOURCE_PATH}")
    new_bytes = git(repo, "show", f"{NEW_COMMIT}:{SOURCE_PATH}")
    require(sha256(old_bytes) == OLD_FILE_ROOT, "predecessor file root mismatch")
    require(sha256(new_bytes) == NEW_FILE_ROOT, "successor file root mismatch")

    old_text = old_bytes.decode("utf-8")
    new_text = new_bytes.decode("utf-8")
    require(OLD_STATEMENT in old_text, "predecessor theorem statement missing")
    require(NEW_STATEMENT in new_text, "successor theorem statement missing")
    require(NEW_STATEMENT not in old_text, "successor statement already occurs in predecessor")
    require(OLD_STATEMENT not in new_text, "predecessor statement survives in successor")

    subject = artifact.get("subject")
    predecessor = artifact.get("predecessor")
    successor = artifact.get("successor")
    require(isinstance(subject, dict), "source-diff subject missing")
    require(isinstance(predecessor, dict), "source-diff predecessor missing")
    require(isinstance(successor, dict), "source-diff successor missing")
    require(subject.get("path") == SOURCE_PATH, "source-diff path mismatch")
    require(predecessor.get("commit") == OLD_COMMIT, "source-diff predecessor commit mismatch")
    require(predecessor.get("tree") == OLD_TREE, "source-diff predecessor tree mismatch")
    require(predecessor.get("file_sha256") == OLD_FILE_ROOT, "source-diff predecessor file root mismatch")
    require(predecessor.get("statement") == OLD_STATEMENT, "source-diff predecessor statement mismatch")
    require(successor.get("commit") == NEW_COMMIT, "source-diff successor commit mismatch")
    require(successor.get("tree") == NEW_TREE, "source-diff successor tree mismatch")
    require(successor.get("file_sha256") == NEW_FILE_ROOT, "source-diff successor file root mismatch")
    require(successor.get("statement") == NEW_STATEMENT, "source-diff successor statement mismatch")

    diff = git(repo, "diff", OLD_COMMIT, NEW_COMMIT, "--", SOURCE_PATH)
    require(b"HasPosDensity" in diff, "diff does not remove the predecessor predicate")
    require(b"HasPosLowerDensity" in diff, "diff does not add the successor predicate")

    return {
        "schema": "vela.erdos-424-source-verification.v1",
        "outcome": "pass",
        "source": {
            "repository": "https://github.com/google-deepmind/formal-conjectures",
            "path": SOURCE_PATH,
            "predecessor": {
                "commit": OLD_COMMIT,
                "tree": old_tree,
                "file_root": sha256(old_bytes),
                "statement": OLD_STATEMENT,
            },
            "successor": {
                "commit": NEW_COMMIT,
                "tree": new_tree,
                "file_root": sha256(new_bytes),
                "statement": NEW_STATEMENT,
            },
            "diff_root": sha256(diff),
        },
        "artifact_root": sha256(artifact_bytes),
        "checks": {
            "artifact_matches_source_objects": True,
            "predicate_changed": True,
            "predecessor_predicate_absent_from_successor": True,
            "successor_predicate_absent_from_predecessor": True,
        },
        "limits": [
            "This verifies exact retained source bytes and their stated transition.",
            "It does not prove Erdős problem 424 or establish the preferred informal interpretation.",
            "It does not constitute organizationally independent verification or scientific acceptance.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-repo", type=Path, required=True)
    parser.add_argument("--source-diff", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = verify(args.source_repo.resolve(), args.source_diff.resolve())
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    encoded = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
