#!/usr/bin/env python3
"""Replay and bind the exact Formal Erdős 505 verifier capsule."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


EXPECTED_RUN_ID = "run_585c951f-ed51-49b9-805d-02e7e5a8a0e9"
EXPECTED_RUN_ROOT = "sha256:57a535047c30fbd003900452e5f73af17de47d13f4a2865bb32597a087731db0"
EXPECTED_MISSION_ROOT = "sha256:a22cd3b32b08ea7f54fd684535765d83a934d7321a0943dd330b32eeab5b95ed"
EXPECTED_CAPSULE_ROOT = "sha256:c1ef5a0914e9d537d2acba2b74a27f139d37414629c7676937515e99438b58dd"
EXPECTED_STDOUT_ROOT = "sha256:7e91245afe87fd56f70ed396b2f1754b40f1f4c610486f0b27bcb12918c44662"
EXPECTED_STDERR_ROOT = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
EXPECTED_CANOPUS_CLI_ROOT = (
    "sha256:19e4642e5ca165786a6aa7bf8e352b4461935eb42273c19114818b057c71559d"
)
EXPECTED_CANOPUS_VERSION = "canopus 0.8.0"


class ReplayError(ValueError):
    """Raised when exact replay evidence drifts."""


def file_root(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReplayError(message)


def validate_replay(value: dict[str, Any]) -> dict[str, Any]:
    require(value.get("schema") == "canopus.replay.v1", "replay schema drift")
    require(value.get("ok") is True, "replay did not succeed")
    require(value.get("run_id") == EXPECTED_RUN_ID, "Run identity drift")
    require(value.get("mission_root") == EXPECTED_MISSION_ROOT, "Mission root drift")
    require(value.get("verifier_status") == "passed", "verifier did not pass")
    require(value.get("stdout_digest") == EXPECTED_STDOUT_ROOT, "stdout root drift")
    require(value.get("stderr_digest") == EXPECTED_STDERR_ROOT, "stderr root drift")
    require(value.get("matched") is True, "fresh replay did not match")
    return value


def run(argv: list[str]) -> str:
    result = subprocess.run(
        argv,
        check=False,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=120,
        env={
            "PATH": "/opt/homebrew/bin:/usr/bin:/bin",
            "LANG": "C",
            "LC_ALL": "C",
            "NO_COLOR": "1",
        },
    )
    if result.returncode != 0:
        raise ReplayError(
            "command failed with stderr root "
            + "sha256:"
            + hashlib.sha256(result.stderr.encode()).hexdigest()
        )
    return result.stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--canopus-cli", type=Path, required=True)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    require(args.canopus_cli.is_file(), "Canopus CLI is missing")
    require(args.run.is_file(), "retained Run is missing")
    require(file_root(args.canopus_cli) == EXPECTED_CANOPUS_CLI_ROOT, "Canopus CLI drift")
    require(file_root(args.run) == EXPECTED_RUN_ROOT, "retained Run root drift")
    require(
        run(["node", str(args.canopus_cli), "--version"]) == EXPECTED_CANOPUS_VERSION,
        "Canopus version drift",
    )
    replay = validate_replay(
        json.loads(run(["node", str(args.canopus_cli), "replay", str(args.run)]))
    )
    report = {
        "schema": "vela.formal-505-replay-report.v1",
        "run_id": EXPECTED_RUN_ID,
        "run_root": EXPECTED_RUN_ROOT,
        "mission_root": EXPECTED_MISSION_ROOT,
        "verifier_capsule_root": EXPECTED_CAPSULE_ROOT,
        "canopus_version": EXPECTED_CANOPUS_VERSION,
        "canopus_cli_root": EXPECTED_CANOPUS_CLI_ROOT,
        "verifier_status": replay["verifier_status"],
        "stdout_root": replay["stdout_digest"],
        "stderr_root": replay["stderr_digest"],
        "matched": replay["matched"],
        "authority_effect": "none",
    }
    args.output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"ok": True, "report_root": file_root(args.output)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ReplayError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"formal-505 verifier: {error}") from error
