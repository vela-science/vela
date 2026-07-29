#!/usr/bin/env python3
"""Run the frozen removability check and retain exact command output."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def command(
    *args: str,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
) -> bytes:
    return subprocess.run(
        list(args),
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        cwd=cwd,
    ).stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def normalize_frontier_path(value: object, frontier: str) -> object:
    if isinstance(value, str):
        return value.replace(frontier, "<frontier>")
    if isinstance(value, list):
        return [normalize_frontier_path(item, frontier) for item in value]
    if isinstance(value, dict):
        return {
            key: normalize_frontier_path(item, frontier)
            for key, item in value.items()
        }
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--clone", type=Path, required=True)
    parser.add_argument("--vela", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    here = Path(__file__).resolve().parent
    plan_path = here / "plan.v1.json"
    plan_bytes = plan_path.read_bytes()
    plan = json.loads(plan_bytes)
    clone = args.clone.resolve()
    vela = args.vela.resolve()
    output = args.output.resolve()
    require(not output.exists(), "output directory already exists")
    output.mkdir(parents=True, mode=0o755)
    empty_home = output / "empty-home"
    empty_home.mkdir(mode=0o700)

    try:
        expected_frontier = plan["inputs"]["frontier"]
        expected_vela = plan["inputs"]["vela"]
        commit = command("git", "-C", str(clone), "rev-parse", "HEAD").decode().strip()
        tree = command("git", "-C", str(clone), "rev-parse", "HEAD^{tree}").decode().strip()
        require(commit == expected_frontier["commit"], "Frontier commit mismatch")
        require(tree == expected_frontier["tree"], "Frontier tree mismatch")
        require(command(str(vela), "--version").decode().strip() == expected_vela["version"], "Vela version mismatch")
        require(sha256(vela.read_bytes()) == expected_vela["binary_root"], "Vela binary root mismatch")

        isolated_env = {
            "HOME": str(empty_home),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        }
        sandbox = [
            "/usr/bin/sandbox-exec",
            "-p",
            "(version 1) (allow default) (deny network*)",
            "/usr/bin/env",
            "-i",
            *[f"{key}={value}" for key, value in isolated_env.items()],
        ]
        invocations = {
            "check": [str(vela), "check", ".", "--strict", "--json"],
            "status": [str(vela), "status", ".", "--json"],
            "review": [
                str(vela),
                "review",
                "show",
                ".",
                plan["inputs"]["proposal_id"],
                "--json",
            ],
        }
        retained: dict[str, bytes] = {}
        parsed: dict[str, dict[str, object]] = {}
        for name, invocation in invocations.items():
            encoded = command(*sandbox, *invocation, env={}, cwd=clone)
            parsed[name] = json.loads(encoded)
            normalized = normalize_frontier_path(parsed[name], str(clone))
            retained[name] = f"{json.dumps(normalized, sort_keys=True, separators=(',', ':'))}\n".encode()
            (output / f"{name}.json").write_bytes(retained[name])

        expected = plan["expected"]
        check = parsed["check"]
        status = parsed["status"]
        review = parsed["review"]
        require(check["ok"] is expected["strict_check_ok"], "strict check outcome mismatch")
        require(check["repository_root"] == expected["repository_root"], "check repository root mismatch")
        require(status["roots"]["repository"] == expected["repository_root"], "status repository root mismatch")
        for field in ("accepted_claims", "pending_claims", "pending_review"):
            require(status["counts"][field] == expected[field], f"{field} mismatch")
        require(review["standing"] == expected["proposal_standing"], "proposal standing mismatch")
        require(
            len(review["verification_records"]) == expected["verification_record_count_for_proposal"],
            "proposal Verification count mismatch",
        )
        require(review["decision"] == expected["decision"], "proposal Decision mismatch")

        result = {
            "schema": "vela.removability-evaluation-result.v1",
            "outcome": "pass",
            "plan_root": sha256(plan_bytes),
            "frontier": {
                "commit": commit,
                "tree": tree,
                "repository_root": check["repository_root"],
            },
            "vela": {
                "version": expected_vela["version"],
                "binary_root": expected_vela["binary_root"],
            },
            "isolation": {
                "fresh_clone": True,
                "empty_home": True,
                "network_denied_by": "macOS sandbox-exec deny network*",
                "repository_authority_environment_present": False,
            },
            "normalized_output_roots": {
                name: sha256(encoded) for name, encoded in retained.items()
            },
            "observed": {
                "strict_check_ok": check["ok"],
                "repository_root": check["repository_root"],
                "counts": status["counts"],
                "proposal_standing": review["standing"],
                "proposal_verification_count": len(review["verification_records"]),
                "proposal_decision": review["decision"],
            },
            "limits": plan["classification"]["does_not_establish"],
        }
        result_bytes = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n".encode()
        (output / "result.json").write_bytes(result_bytes)
        print(result_bytes.decode(), end="")
        return 0
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"removability check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
