#!/usr/bin/env python3
"""Freeze the terminal stopped first confirmatory registration."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
STUDY = ROOT / "confirmatory-study"
EXECUTION = ROOT / "confirmatory-execution"
BENCHMARK = ROOT.parent / "inherited-correction-benchmark"
OUTPUT = EXECUTION / "stopped-registration.json"
STOPPED_AT = "2026-08-21T19:19:49Z"
RUN_COMMIT = "3931477893bd92015590a89a71129b581cc06ea3"
PASSED_PRELAUNCH_COMMIT = "7596c12291c22a5b4b81a1ab1eb49189318f57de"
PASSED_REVIEW_COMMIT = "59c104efefd163d2e4c86e1bd535ac5f7c03f17d"


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def load(path: Path) -> Any:
    return json.loads(path.read_text())


def tree_manifest(directory: Path) -> list[dict[str, Any]]:
    files = []
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise SystemExit(f"symlink forbidden: {path}")
        if not path.is_file():
            continue
        content = path.read_bytes()
        files.append(
            {
                "path": path.relative_to(directory).as_posix(),
                "bytes": len(content),
                "sha256": digest(content),
            }
        )
    return files


def validate_response(response: Any) -> None:
    spec = importlib.util.spec_from_file_location(
        "stopped_benchmark", BENCHMARK / "benchmark.py"
    )
    if spec is None or spec.loader is None:
        raise SystemExit("benchmark unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.validate_response(response)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing {output}")
    freeze = load(STUDY / "prelaunch-freeze.json")
    registration = load(STUDY / "registration.json")
    run_dir = EXECUTION / "runs/confirm-run-01"
    capture_dir = EXECUTION / "captures/confirm-run-01"
    run = load(run_dir / "run.json")
    receipt = load(capture_dir / "evidence/terminal-receipt.json")
    response_path = capture_dir / "evidence/participant-response.raw.json"
    response = load(response_path)
    validate_response(response)
    if canonical_root(registration) != freeze["registration_root"]:
        raise SystemExit("stopped registration root drifted")
    if set(path.name for path in (EXECUTION / "runs").iterdir()) != {"confirm-run-01"}:
        raise SystemExit("unexpected stopped run directory")
    if set(path.name for path in (EXECUTION / "captures").iterdir()) != {
        "confirm-run-01"
    }:
        raise SystemExit("unexpected stopped capture directory")
    expected_error = (
        'no schema with key or ref "https://json-schema.org/draft/2020-12/schema"'
    )
    if (
        receipt.get("status") != "non_result"
        or receipt.get("validation_error") != expected_error
        or receipt.get("process_exit_code") != 0
        or receipt.get("response_bytes") != digest(response_path.read_bytes())
        or run.get("status") != "failed"
        or run.get("runtime_custody_root")
        != "sha256:717415534787f48d68206c2e8b8de1bfc21742c333944277519021797b10f76c"
    ):
        raise SystemExit("stopped run evidence drifted")
    unissued = [f"confirm-run-{index:02d}" for index in range(2, 17)]
    run_manifest = tree_manifest(run_dir)
    capture_manifest = tree_manifest(capture_dir)
    value = {
        "schema": "vela.inherited-correction-stopped-confirmatory.v1",
        "status": "stopped_after_one_terminal_harness_non_result",
        "stopped_at": STOPPED_AT,
        "passed_prelaunch_producer_commit": PASSED_PRELAUNCH_COMMIT,
        "passed_prelaunch_review_commit": PASSED_REVIEW_COMMIT,
        "terminal_execution_commit": RUN_COMMIT,
        "benchmark_registration_root": freeze["benchmark_registration_root"],
        "runtime_registration_root": freeze["registration_root"],
        "assignment_root": freeze["assignment_root"],
        "authorization_root": freeze["authorization_root"],
        "participant_configuration_root": freeze["participant_configuration_root"],
        "image_digest": freeze["image_digest"],
        "trust_bundle_bytes": freeze["trust_bundle_bytes"],
        "stop_reason": "the pinned default Ajv mode could not compile the preregistered Draft 2020-12 response schema",
        "terminal_runs": [
            {
                "run_id": "confirm-run-01",
                "condition": run["condition"],
                "participant_instance_id": run["participant_instance_id"],
                "harness_status": receipt["status"],
                "benchmark_status": run["status"],
                "validation_error": receipt["validation_error"],
                "provider_response_retained": True,
                "provider_response_structurally_valid": True,
                "response_bytes": receipt["response_bytes"],
                "runtime_custody_root": run["runtime_custody_root"],
                "attempt": 1,
                "retries": 0,
                "substitutions": 0,
                "stopped_registration_denominator_credit": True,
                "replacement_denominator_credit": False,
            }
        ],
        "unissued_runs": unissued,
        "unissued_runs_must_never_launch": True,
        "protected_adjudication_access_count": 0,
        "score_status": "not_run_and_forbidden",
        "provider_response_disposition": "retained exact; never retried, substituted, scored, or reinterpreted",
        "run_manifest_root": canonical_root(run_manifest),
        "capture_manifest_root": canonical_root(capture_manifest),
        "run_files": run_manifest,
        "capture_files": capture_manifest,
        "freezer_bytes": digest(Path(__file__).read_bytes()),
        "authority_effect": "none",
    }
    value["stop_root"] = canonical_root(value)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded(value))
    print(json.dumps({"stop_root": value["stop_root"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
