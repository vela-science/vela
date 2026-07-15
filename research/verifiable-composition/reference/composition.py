#!/usr/bin/env python3
"""Experiment-only CLI for exact-checkout dependency candidates."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from dependency_observation import ObservationError, parse_observation
from exact_checkout import (
    CompositionError,
    encode_observation,
    resolve_observation,
)


def emit(value: dict[str, Any]) -> None:
    print(json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True))


def bounded_read(path: Path, limit: int = 32 * 1024 * 1024) -> bytes:
    try:
        size = path.stat().st_size
    except OSError as error:
        raise CompositionError("input:file_missing", str(path)) from error
    if size > limit:
        raise CompositionError("input:file_oversized", str(path))
    try:
        return path.read_bytes()
    except OSError as error:
        raise CompositionError("input:file_unreadable", str(path)) from error


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(
        description="ADR 0004 exact-checkout experiment; never signs or mutates a frontier"
    )
    commands = root.add_subparsers(dest="command", required=True)
    encode = commands.add_parser("encode", help="emit structural candidate provenance")
    encode.add_argument("--repo", required=True, type=Path)
    encode.add_argument("--commit", required=True)
    encode.add_argument("--selection", required=True, type=Path)
    resolve = commands.add_parser(
        "resolve", help="fail-closed read-only resolution attempt"
    )
    resolve.add_argument("--repo", required=True, type=Path)
    resolve.add_argument("--observation", required=True, type=Path)
    resolve.add_argument("--frontier-path", required=True)
    resolve.add_argument("--premise-path", required=True)
    return root


def main() -> int:
    arguments = parser().parse_args()
    try:
        if arguments.command == "encode":
            observation = encode_observation(
                arguments.repo,
                arguments.commit,
                bounded_read(arguments.selection, 1024 * 1024),
            )
            emit(
                {
                    "ok": True,
                    "status": "structural_candidate",
                    "authority_verified": False,
                    "observation": observation,
                }
            )
            return 0
        raw = bounded_read(arguments.observation, 1024 * 1024)
        observation = parse_observation(raw)
        result = resolve_observation(
            arguments.repo,
            observation,
            frontier_path=arguments.frontier_path,
            premise_path=arguments.premise_path,
        )
        emit(result)
        return 0 if result["ok"] is True else 1
    except (CompositionError, ObservationError) as error:
        emit(
            {
                "ok": False,
                "status": "rejected",
                "code": getattr(error, "code", str(error)),
                "detail": getattr(error, "detail", ""),
            }
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())
