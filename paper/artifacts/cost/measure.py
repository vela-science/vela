#!/usr/bin/env python3
"""Measure bounded read-only Vela protocol costs under a frozen plan."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import stat
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


EXPECTED_FRONTIERS = ("erdos", "formal", "sidon", "quantum")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
DURATION = re.compile(r"\(\d+(?:\.\d+)?s\)")


def canonical_bytes(value: object) -> bytes:
    return f"{json.dumps(value, sort_keys=True, separators=(',', ':'))}\n".encode()


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def command(
    argv: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
) -> bytes:
    completed = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise ValueError(
            f"command failed ({completed.returncode}): {' '.join(argv)}; "
            f"stdout={sha256(completed.stdout)}; stderr={sha256(completed.stderr)}"
        )
    return completed.stdout


def git(frontier: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(frontier), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def replace_path(value: Any, frontier: Path) -> Any:
    if isinstance(value, str):
        return value.replace(str(frontier), "<frontier>")
    if isinstance(value, list):
        return [replace_path(item, frontier) for item in value]
    if isinstance(value, dict):
        return {key: replace_path(item, frontier) for key, item in value.items()}
    return value


def normalize_output(raw: bytes, frontier: Path) -> bytes:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError:
        text = ANSI.sub("", raw.decode("utf-8"))
        text = text.replace(str(frontier), "<frontier>")
        return DURATION.sub("(<duration>)", text).encode()
    return canonical_bytes(replace_path(value, frontier))


def summarize(samples_ns: list[int]) -> dict[str, object]:
    require(bool(samples_ns), "timing sample set is empty")
    samples_ms = [round(sample / 1_000_000, 3) for sample in samples_ns]
    return {
        "samples_ms": samples_ms,
        "minimum_ms": min(samples_ms),
        "median_ms": round(statistics.median(samples_ms), 3),
        "maximum_ms": max(samples_ms),
    }


def benchmark(
    argv: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    sandbox: list[str],
    warmups: int,
    repetitions: int,
) -> dict[str, object]:
    full = [*sandbox, *argv]
    for _ in range(warmups):
        command(full, cwd=cwd, env=env)
    samples: list[int] = []
    output_roots: list[str] = []
    for _ in range(repetitions):
        started = time.perf_counter_ns()
        raw = command(full, cwd=cwd, env=env)
        samples.append(time.perf_counter_ns() - started)
        output_roots.append(sha256(normalize_output(raw, cwd)))
    require(len(set(output_roots)) == 1, "normalized command output drifted")
    return {
        **summarize(samples),
        "normalized_output_root": output_roots[0],
    }


def tracked_inventory(frontier: Path) -> dict[str, object]:
    paths = [
        Path(item.decode())
        for item in git(frontier, "ls-files", "-z").split(b"\0")
        if item
    ]
    top_level: dict[str, int] = {}
    total = 0
    for relative in paths:
        candidate = frontier / relative
        metadata = candidate.lstat()
        require(
            stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode),
            f"tracked path is neither a file nor symlink: {relative}",
        )
        size = metadata.st_size
        total += size
        head = relative.parts[0]
        top_level[head] = top_level.get(head, 0) + size
    return {
        "tracked_file_count": len(paths),
        "tracked_file_bytes": total,
        "top_level_tracked_bytes": dict(sorted(top_level.items())),
    }


def parse_frontiers(values: list[str]) -> dict[str, Path]:
    parsed: dict[str, Path] = {}
    for value in values:
        name, separator, raw_path = value.partition("=")
        require(bool(separator) and bool(raw_path), f"invalid --frontier value: {value}")
        require(name not in parsed, f"duplicate Frontier name: {name}")
        parsed[name] = Path(raw_path).expanduser().resolve()
    require(tuple(parsed) == EXPECTED_FRONTIERS, "Frontiers must use the frozen order")
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vela", type=Path, required=True)
    parser.add_argument("--frontier", action="append", default=[], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    here = Path(__file__).resolve().parent
    plan = json.loads((here / "plan.json").read_bytes())
    plan_root = sha256(canonical_bytes(plan))
    vela = args.vela.expanduser().resolve()
    frontiers = parse_frontiers(args.frontier)
    output = args.output.expanduser().resolve()
    require(not output.exists(), "output already exists")
    require(vela.is_file(), "Vela binary is missing")
    repetitions = int(plan["sampling"]["repetitions"])
    warmups = int(plan["sampling"]["warmups"])

    with tempfile.TemporaryDirectory(prefix="vela-cost-home-") as home:
        env = {
            "HOME": home,
            "LANG": "C",
            "LC_ALL": "C",
            "NO_COLOR": "1",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "VELA_ADVICE": "0",
        }
        sandbox: list[str] = []
        isolation = "credential environment removed; network not sandboxed"
        sandbox_binary = Path("/usr/bin/sandbox-exec")
        if platform.system() == "Darwin" and sandbox_binary.is_file():
            sandbox = [
                str(sandbox_binary),
                "-p",
                "(version 1) (allow default) (deny network*)",
            ]
            isolation = "credential environment removed; macOS sandbox denies network"

        version = command([str(vela), "--version"], cwd=here, env=env).decode().strip()
        for name, frontier in frontiers.items():
            require(frontier.is_dir(), f"Frontier is missing: {name}")
            require(
                git(frontier, "status", "--porcelain=v1", "--untracked-files=all") == b"",
                f"Frontier is dirty: {name}",
            )
        erdos = frontiers["erdos"]
        proposal = plan["entry_gate"]["erdos_proposal_id"]
        entry_review = json.loads(
            command(
                [*sandbox, str(vela), "review", "show", ".", proposal, "--json"],
                cwd=erdos,
                env=env,
            )
        )
        require(
            entry_review["standing"] in plan["entry_gate"]["required_standing"],
            "Erdős Decision is not terminal",
        )

        observations: list[dict[str, object]] = []
        for name, frontier in frontiers.items():
            commit = git(frontier, "rev-parse", "HEAD^{commit}").decode().strip()
            tree = git(frontier, "rev-parse", "HEAD^{tree}").decode().strip()
            status_argv = [str(vela), "status", ".", "--json"]
            check_argv = [str(vela), "check", ".", "--strict", "--json"]
            operations: dict[str, object] = {
                "status": benchmark(
                    status_argv,
                    cwd=frontier,
                    env=env,
                    sandbox=sandbox,
                    warmups=warmups,
                    repetitions=repetitions,
                ),
                "strict_check": benchmark(
                    check_argv,
                    cwd=frontier,
                    env=env,
                    sandbox=sandbox,
                    warmups=warmups,
                    repetitions=repetitions,
                ),
            }
            status = json.loads(command([*sandbox, *status_argv], cwd=frontier, env=env))
            check = json.loads(command([*sandbox, *check_argv], cwd=frontier, env=env))
            require(status["roots"]["repository"] == check["repository_root"], "root drift")
            if name == "erdos":
                review_argv = [str(vela), "review", "show", ".", proposal, "--json"]
                operations["review_show"] = benchmark(
                    review_argv,
                    cwd=frontier,
                    env=env,
                    sandbox=sandbox,
                    warmups=warmups,
                    repetitions=repetitions,
                )
                operations["reproduce"] = benchmark(
                    [str(vela), "reproduce", "."],
                    cwd=frontier,
                    env=env,
                    sandbox=sandbox,
                    warmups=warmups,
                    repetitions=repetitions,
                )
            require(
                git(frontier, "status", "--porcelain=v1", "--untracked-files=all") == b"",
                f"read-only measurement changed Frontier: {name}",
            )
            observations.append(
                {
                    "name": name,
                    "git_commit": commit,
                    "git_tree": tree,
                    "repository_root": check["repository_root"],
                    "counts": check["counts"],
                    "storage": tracked_inventory(frontier),
                    "operations": operations,
                }
            )

    result = {
        "schema": "vela.cost-evaluation-result.v1",
        "plan_root": plan_root,
        "observed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "environment": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "isolation": isolation,
        },
        "vela": {
            "version": version,
            "binary_root": sha256(vela.read_bytes()),
            "binary_bytes": vela.stat().st_size,
        },
        "frontiers": observations,
        "limits": plan["does_not_establish"],
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(canonical_bytes(result))
    print(canonical_bytes(result).decode(), end="")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"cost measurement failed: {error}", file=sys.stderr)
        raise SystemExit(1)
