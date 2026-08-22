#!/usr/bin/env python3
"""Verify one exact repair of Erdős 264 part i in native Lean.

The verifier is read-only with respect to the supplied Formal Conjectures
checkout. It compiles a temporary candidate file under the checkout's pinned
Lake environment and reports evidence only; it never changes Vela Standing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import tempfile
from typing import Any

SOURCE_REPOSITORY = "https://github.com/google-deepmind/formal-conjectures.git"
SOURCE_COMMIT = "e6d6b867dc85eec2f88bc47496b4314c623f9f92"
SOURCE_TREE = "1e24e996a9fee330dc885ec2b314f60bfd508985"
SOURCE_PATH = pathlib.PurePosixPath("FormalConjectures/ErdosProblems/264.lean")
SOURCE_SHA256 = "c59caaa2524e3edd52944e63f5d9bb0614f1bc36d7fb8a0fec7029c14c266b46"
LEAN_TOOLCHAIN = "leanprover/lean4:v4.27.0"
MATHLIB_COMMIT = "a3a10db0e9d66acbebf76c5e6a135066525ac900"
DECLARATION = "Erdos264.erdos_264.parts.i"
LEAN_HEARTBEAT_MODE = "unlimited"
LEAN_COMMAND = ("lake", "env", "lean", "-DmaxHeartbeats=0")
THEOREM_MARKER = (
    "@[category research solved, AMS 11]\n"
    "theorem erdos_264.parts.i : ¬IsIrrationalitySequence (2 ^ ·) := by"
)
NEXT_DECLARATION_MARKER = "\n\n/--\nIs $n!$ an example of an irrationality sequence?"
ALLOWED_AXIOMS = {"Classical.choice", "Quot.sound", "propext"}
FORBIDDEN_PROOF_TOKENS = re.compile(
    r"(?<![A-Za-z0-9_])(sorry|admit|axiom|opaque|unsafe)(?![A-Za-z0-9_])"
)


class VerificationError(ValueError):
    """The candidate or environment violates the exact verifier contract."""


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def git(workspace: pathlib.Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(workspace), *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def validate_workspace(workspace: pathlib.Path) -> bytes:
    if git(workspace, "rev-parse", "HEAD") != SOURCE_COMMIT:
        raise VerificationError("Formal Conjectures commit differs")
    if git(workspace, "rev-parse", "HEAD^{tree}") != SOURCE_TREE:
        raise VerificationError("Formal Conjectures tree differs")
    if git(workspace, "status", "--porcelain=v1", "--untracked-files=all"):
        raise VerificationError("Formal Conjectures checkout is dirty")
    source = workspace.joinpath(*SOURCE_PATH.parts).read_bytes()
    if sha256(source) != f"sha256:{SOURCE_SHA256}":
        raise VerificationError("Erdős 264 source bytes differ")
    toolchain = (workspace / "lean-toolchain").read_text().strip()
    if toolchain != LEAN_TOOLCHAIN:
        raise VerificationError("Lean toolchain differs")
    manifest = json.loads((workspace / "lake-manifest.json").read_text())
    mathlib = next(
        (row for row in manifest.get("packages", []) if row.get("name") == "mathlib"),
        None,
    )
    if not isinstance(mathlib, dict) or mathlib.get("rev") != MATHLIB_COMMIT:
        raise VerificationError("mathlib commit differs")
    return source


def validate_candidate(source: bytes, candidate: bytes) -> str:
    try:
        source_text = source.decode()
        candidate_text = candidate.decode()
    except UnicodeDecodeError as error:
        raise VerificationError("candidate is not UTF-8 Lean source") from error
    if (
        source_text.count(THEOREM_MARKER) != 1
        or source_text.count(NEXT_DECLARATION_MARKER) != 1
    ):
        raise VerificationError("pinned source theorem boundary is ambiguous")
    source_start = source_text.index(THEOREM_MARKER)
    source_end = source_text.index(NEXT_DECLARATION_MARKER, source_start)
    if (
        candidate_text.count(THEOREM_MARKER) != 1
        or candidate_text.count(NEXT_DECLARATION_MARKER) != 1
    ):
        raise VerificationError("candidate changes the theorem boundary or signature")
    candidate_start = candidate_text.index(THEOREM_MARKER)
    candidate_end = candidate_text.index(NEXT_DECLARATION_MARKER, candidate_start)
    if candidate_text[:candidate_start] != source_text[:source_start]:
        raise VerificationError("candidate changes source before Erdős 264 part i")
    if candidate_text[candidate_end:] != source_text[source_end:]:
        raise VerificationError("candidate changes source after Erdős 264 part i")
    proof = candidate_text[candidate_start + len(THEOREM_MARKER) : candidate_end]
    if not proof.strip():
        raise VerificationError("candidate proof is empty")
    match = FORBIDDEN_PROOF_TOKENS.search(proof)
    if match:
        raise VerificationError(
            f"candidate proof contains forbidden token {match.group(1)}"
        )
    return proof


def parse_axioms(output: str) -> list[str]:
    match = re.search(r"depends on axioms:\s*\[([^\]]*)\]", output, re.DOTALL)
    if not match:
        raise VerificationError("Lean did not report the target declaration's axioms")
    values = [item.strip() for item in match.group(1).split(",") if item.strip()]
    unexpected = sorted(set(values) - ALLOWED_AXIOMS)
    if unexpected:
        raise VerificationError(
            f"target uses forbidden axioms: {', '.join(unexpected)}"
        )
    return sorted(values)


def verify(workspace: pathlib.Path, candidate_path: pathlib.Path) -> dict[str, Any]:
    source = validate_workspace(workspace)
    candidate = candidate_path.read_bytes()
    validate_candidate(source, candidate)
    check_source = candidate + f"\n#print axioms {DECLARATION}\n".encode()
    temporary_path: pathlib.Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            suffix=".lean",
            prefix="vela-erdos264-",
            dir=workspace,
            delete=False,
        ) as temporary:
            temporary.write(check_source)
            temporary_path = pathlib.Path(temporary.name)
        process = subprocess.run(
            [*LEAN_COMMAND, str(temporary_path)],
            cwd=workspace,
            capture_output=True,
            text=True,
            timeout=600,
        )
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
    if process.returncode != 0:
        raise VerificationError(
            "native Lean verification failed; "
            f"stdout_root={sha256(process.stdout.encode())}; "
            f"stderr_root={sha256(process.stderr.encode())}"
        )
    axioms = parse_axioms(process.stdout + "\n" + process.stderr)
    return {
        "schema": "erdos-frontier.lean-proof-repair-verification.v1",
        "outcome": "pass",
        "authority": "non_authoritative",
        "target": "erdos:264:parts-i-proof-repair",
        "declaration": DECLARATION,
        "candidate_root": sha256(candidate),
        "source": {
            "repository": SOURCE_REPOSITORY,
            "commit": SOURCE_COMMIT,
            "tree": SOURCE_TREE,
            "path": SOURCE_PATH.as_posix(),
            "sha256": f"sha256:{SOURCE_SHA256}",
        },
        "environment": {
            "lean_toolchain": LEAN_TOOLCHAIN,
            "mathlib_commit": MATHLIB_COMMIT,
            "heartbeat_mode": LEAN_HEARTBEAT_MODE,
        },
        "checks": {
            "source_identity_exact": True,
            "only_target_theorem_changed": True,
            "native_lean_passed": True,
            "permitted_axioms_only": True,
        },
        "axioms": axioms,
        "accepted_state_change": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=pathlib.Path, required=True)
    parser.add_argument("--candidate", type=pathlib.Path, required=True)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        result = verify(args.workspace.resolve(), args.candidate.resolve())
    except (OSError, subprocess.SubprocessError, VerificationError) as error:
        result = {
            "schema": "erdos-frontier.lean-proof-repair-verification.v1",
            "outcome": "fail",
            "authority": "non_authoritative",
            "target": "erdos:264:parts-i-proof-repair",
            "error": str(error),
            "accepted_state_change": "none",
        }
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
