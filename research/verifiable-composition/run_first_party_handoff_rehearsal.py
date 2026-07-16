#!/usr/bin/env python3
"""Run the registered, first-party, authority-free ADR 0006 rehearsal."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
REFERENCE = ROOT / "reference"
sys.path.insert(0, str(REFERENCE))

from graph_handoff import canonical_bytes, child_artifacts, parent_artifacts  # noqa: E402
from graph_handoff_v2 import verify_child, verify_parent  # noqa: E402


class RehearsalError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RehearsalError(message)


def sha(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def file_record(path: Path, base: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    return {"path": str(path.relative_to(base)), "sha256": sha(raw), "bytes": len(raw)}


def run(argv: list[str], *, cwd: Path = ROOT, exits: set[int] = {0}) -> dict[str, Any]:
    started = time.monotonic_ns()
    result = subprocess.run(
        argv,
        cwd=cwd,
        check=False,
        capture_output=True,
        timeout=600,
        env={
            "PATH": "/opt/homebrew/bin:/usr/bin:/bin",
            "PYTHONDONTWRITEBYTECODE": "1",
            "HOME": os.environ.get("HOME", "/nonexistent"),
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "VELA_NO_KEY_ACCESS": "1",
        },
    )
    duration_ms = (time.monotonic_ns() - started) // 1_000_000
    require(result.returncode in exits, f"{argv[0]} exited {result.returncode}: {result.stderr[:500]!r}")
    return {
        "argv": argv,
        "exit_code": result.returncode,
        "stdout_sha256": sha(result.stdout),
        "stderr_sha256": sha(result.stderr),
        "duration_ms": duration_ms,
        "stdout": result.stdout.decode("utf-8", "replace")[:1000],
    }


def executable(path: Path) -> dict[str, Any]:
    require(path.is_file() and os.access(path, os.X_OK), f"missing executable {path}")
    return {"path": str(path), "sha256": sha(path.read_bytes())}


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--registration", type=Path, default=ROOT / "registration/first-party-handoff-rehearsal-v1.json")
    value.add_argument("--vela", type=Path, required=True)
    value.add_argument("--cadical", type=Path, required=True)
    value.add_argument("--drat-trim", type=Path, required=True)
    value.add_argument("--lrat-check", type=Path, required=True)
    value.add_argument("--output", type=Path, required=True)
    value.add_argument("--preflight", action="store_true")
    return value


def main() -> int:
    args = parser().parse_args()
    args.registration = args.registration.resolve()
    args.vela = args.vela.resolve()
    args.cadical = args.cadical.resolve()
    args.drat_trim = args.drat_trim.resolve()
    args.lrat_check = args.lrat_check.resolve()
    args.output = args.output.resolve()
    registration = json.loads(args.registration.read_text())
    require(registration["status"] == "registered_not_run", "registration is not runnable")
    require(registration["authority"]["signing_allowed"] is False, "signing enabled")
    forbidden = [name for name in os.environ if any(token in name.upper() for token in ("VELA_KEY", "SIGNING_KEY", "PRIVATE_KEY"))]
    require(not forbidden, f"key-related environment present: {forbidden}")
    tools = {
        "vela": executable(args.vela),
        "cadical": executable(args.cadical),
        "drat_trim": executable(args.drat_trim),
        "lrat_check": executable(args.lrat_check),
    }
    require(tools["vela"]["sha256"] == registration["release"]["macos_aarch64_sha256"], "Vela binary root drift")
    for label in ("cadical", "drat_trim", "lrat_check"):
        require(tools[label]["sha256"] == registration["tools"][label]["sha256"], f"{label} root drift")
    if args.preflight:
        print(registration["registration_root"])
        return 0

    require(not args.output.exists(), "output path already exists")
    started = time.monotonic_ns()
    args.output.mkdir(parents=True)
    artifacts = args.output / "artifacts"
    artifacts.mkdir()
    commands: list[dict[str, Any]] = []
    graph_case = json.loads((ROOT / "registration/graph-case.json").read_text())
    parent = parent_artifacts(graph_case)
    v2_parent = verify_parent(graph_case)
    require(parent["graph_root"] == v2_parent["graph_root"], "parent verifier disagreement")

    parent_graph = artifacts / "parent-graph.json"
    parent_graph.write_bytes(canonical_bytes(parent["graph"]) + b"\n")
    parent_coloring = artifacts / "parent-four-colouring.json"
    parent_coloring.write_bytes(canonical_bytes(parent["four_colouring"]) + b"\n")
    parent_cnf = artifacts / "parent-three-colourability.cnf"
    parent_cnf.write_text(parent["three_colour_dimacs"])
    drat = artifacts / ".parent.drat"
    parent_lrat = artifacts / "parent-three-colourability.lrat"
    commands.append(run([str(args.cadical), str(parent_cnf), str(drat)], exits={20}))
    drat_root = sha(drat.read_bytes())
    commands.append(run([str(args.drat_trim), str(parent_cnf), str(drat), "-L", str(parent_lrat)]))
    commands.append(run([str(args.lrat_check), str(parent_cnf), str(parent_lrat)]))
    drat.unlink()

    parent_files = [parent_graph, parent_coloring, parent_cnf, parent_lrat]
    pending = {
        "schema": "vela.first-party-pending-handoff.v1",
        "registration_root": registration["registration_root"],
        "parent_graph_root": parent["graph_root"],
        "artifact_roots": [file_record(path, args.output)["sha256"] for path in parent_files],
        "verifier_paths": ["python_adjacency_dsat", "python_bitset_static_order", "cadical_drat_to_lrat"],
        "authority_status": "pending_review",
        "hard_dependency_usable": False,
        "accepted_state_claim": False,
        "human_key_access": False,
    }
    pending_path = artifacts / "pending-handoff.json"
    pending_path.write_bytes(canonical_bytes(pending) + b"\n")

    child = child_artifacts(canonical_bytes(parent["graph"]))
    v2_child = verify_child(canonical_bytes(parent["graph"]), child["child"], child["five_colouring"])
    require(child["child_root"] == v2_child["child_root"], "child verifier disagreement")
    child_graph = artifacts / "child-graph.json"
    child_graph.write_bytes(canonical_bytes(child["child"]) + b"\n")
    child_coloring = artifacts / "child-five-colouring.json"
    child_coloring.write_bytes(canonical_bytes(child["five_colouring"]) + b"\n")
    child_cnf = artifacts / "child-four-colourability.cnf"
    child_cnf.write_text(child["four_colour_dimacs"])
    child_drat = artifacts / ".child.drat"
    child_lrat = artifacts / "child-four-colourability.lrat"
    commands.append(run([str(args.cadical), str(child_cnf), str(child_drat)], exits={20}))
    child_drat_root = sha(child_drat.read_bytes())
    commands.append(run([str(args.drat_trim), str(child_cnf), str(child_drat), "-L", str(child_lrat)]))
    commands.append(run([str(args.lrat_check), str(child_cnf), str(child_lrat)]))
    child_drat.unlink()

    checks = [
        "check_graph_handoff.py",
        "check_fact_manifest_projections.py",
        "check_offline_bundle_inspection.py",
        "check_standards_baseline.py",
    ]
    for script in checks:
        commands.append(run([sys.executable, str(ROOT / script)]))

    vectors = json.loads((ROOT / "vectors/fact-manifest-projection-cases.json").read_text())["cases"]
    status_distribution = dict(sorted(Counter(case["expected_status"] for case in vectors).items()))
    standards_vectors = json.loads((ROOT / "vectors/standards-baseline-cases.json").read_text())["cases"]
    retained = [*parent_files, pending_path, child_graph, child_coloring, child_cnf, child_lrat]
    result = {
        "schema": "vela.first-party-handoff-rehearsal-result.v1",
        "registration_root": registration["registration_root"],
        "run_class": "first_party_internal_fixture",
        "result": "pass",
        "tools": tools,
        "producer_a": {
            "graph_root": parent["graph_root"],
            "triangle_free": True,
            "chromatic_number": 4,
            "v1_v2_parity": True,
            "drat_intermediate_root": drat_root,
            "lrat_verified": True,
        },
        "pending_handoff": pending,
        "child": {
            "parent_root_consumed": child["parent_root"],
            "child_root": child["child_root"],
            "vertices": child["child"]["vertices"],
            "triangle_free": True,
            "chromatic_number": 5,
            "v1_v2_parity": True,
            "drat_intermediate_root": child_drat_root,
            "lrat_verified": True,
            "authority_context": "simulated_internal_fixture_only",
        },
        "correction_replay": {
            "vectors": len(vectors),
            "status_distribution": status_distribution,
            "reader_c_parity": len(vectors),
            "child_truth": "not_assessed",
        },
        "standards_baseline": {"vectors": len(standards_vectors), "passed": len(standards_vectors)},
        "artifacts": [file_record(path, args.output) for path in retained],
        "commands": commands,
        "measurements": {
            "wall_ms": (time.monotonic_ns() - started) // 1_000_000,
            "command_count": len(commands),
            "repair_count": 0,
            "maintainer_semantic_interventions": 0,
            "network_requests": 0,
        },
        "authority": {
            "human_key_access": False,
            "authority_attempts": 0,
            "accepted_state_claim": False,
            "historical_event_rewrites": 0,
        },
        "credit": registration["credit"],
        "gap_verdicts": {
            "adr_0007": "not_reproduced",
            "adr_0008": "not_reproduced",
            "adr_0009": "not_reproduced",
        },
    }
    result_path = args.output / "result.json"
    result_path.write_bytes(canonical_bytes(result) + b"\n")
    print(sha(canonical_bytes(result)))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RehearsalError as error:
        print(f"rehearsal refused: {error}", file=sys.stderr)
        sys.exit(1)
