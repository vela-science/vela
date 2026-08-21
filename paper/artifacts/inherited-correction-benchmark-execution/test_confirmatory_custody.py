"""Fail-closed tests for confirmatory runtime-to-benchmark custody."""

from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent


def module_from(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


custody = module_from(ROOT / "confirmatory-custody.py", "confirmatory_custody_tests")
benchmark = module_from(
    ROOT.parent / "inherited-correction-benchmark/benchmark.py",
    "confirmatory_benchmark_tests",
)


def response() -> dict:
    return {
        "schema": "vela.inherited-correction-response.v1",
        "fixture_id": "bounded-calibration-correction-v1",
        "predecessor_claim_id": "calibration-a-v1",
        "successor_claim_id": "calibration-a-v2",
        "consequences": [
            {
                "claim_id": "aggregate-e",
                "classification": "presently_unprovable",
                "action_code": "retrieve_exact_site_q_source",
            },
            {
                "claim_id": "installation-d",
                "classification": "unaffected",
                "action_code": "no_correction_reassessment",
            },
            {
                "claim_id": "stability-c",
                "classification": "must_reassess",
                "action_code": "rerun_stability_method",
            },
            {
                "claim_id": "yield-b",
                "classification": "affected",
                "action_code": "recalculate_with_successor_factor",
            },
        ],
        "standing_effect": "none",
        "source_or_evidence_binding": "sha256:" + "1" * 64,
    }


def make_capture(directory: Path, run_id: str) -> Path:
    state = custody.static_state()
    row = state["rows"][run_id]
    permit_source = custody.STUDY / "permit-template" / f"{run_id}.permit.json"
    permit_dir = directory / "permit"
    evidence_dir = directory / "evidence"
    permit_dir.mkdir(parents=True)
    evidence_dir.mkdir()
    consumed = permit_dir / f"{run_id}.permit.consumed.json"
    shutil.copyfile(permit_source, consumed)
    value = response()
    response_bytes = custody.encoded(value)
    (evidence_dir / "participant-response.raw.json").write_bytes(response_bytes)
    usage = {
        "input_tokens": 20000,
        "cached_input_tokens": 1000,
        "cache_write_input_tokens": 0,
        "output_tokens": 300,
        "reasoning_output_tokens": 80,
    }
    event_response = json.dumps(value, sort_keys=True, separators=(",", ":"))
    events = [
        {"type": "thread.started", "thread_id": f"thread-{run_id}"},
        {"type": "turn.started"},
        {
            "type": "item.completed",
            "item": {"id": "item_0", "type": "agent_message", "text": event_response},
        },
        {"type": "turn.completed", "usage": usage},
    ]
    event_bytes = b"".join(
        json.dumps(event, separators=(",", ":")).encode() + b"\n" for event in events
    )
    (evidence_dir / "provider-events.jsonl").write_bytes(event_bytes)
    (evidence_dir / "provider-stderr.txt").write_bytes(b"")
    permit = custody.load(consumed)
    launch = {
        "schema": "vela.inherited-correction-launch.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "condition": row["condition"],
        "permit_bytes": custody.digest(consumed.read_bytes()),
        "consumed_at": "2026-08-22T00:00:00Z",
    }
    (evidence_dir / "launch.json").write_bytes(custody.encoded(launch))
    event_receipt = {
        "usage": usage,
        "event_count": 4,
        "response_count": 1,
        "tool_calls": 0,
        "turn_count": 1,
        "compactions": 0,
    }
    receipt = {
        "schema": "vela.inherited-correction-terminal-receipt.v1",
        "run_id": run_id,
        "condition": row["condition"],
        "participant_instance_id": row["participant_instance_id"],
        "attempt": 1,
        "status": "completed",
        "validation_error": None,
        "provider_started_at": "2026-08-22T00:00:01Z",
        "provider_completed_at": "2026-08-22T00:00:05Z",
        "duration_seconds": 4.0,
        "timeout_seconds": 600,
        "process_exit_code": 0,
        "process_timed_out": False,
        "registration_root": permit["registration_root"],
        "image_digest": permit["image_digest"],
        "participant_configuration_root": permit["participant_configuration_root"],
        "assignment_root": permit["assignment_root"],
        "trust_bundle_bytes": permit["trust_bundle_bytes"],
        "prompt_root": permit["prompt_root"],
        "packet_root": permit["packet_root"],
        "provider_events_bytes": custody.digest(event_bytes),
        "provider_stderr_bytes": custody.digest(b""),
        "response_bytes": custody.digest(response_bytes),
        "event_receipt": event_receipt,
        "cumulative_provider_usage_is_telemetry_only": True,
        "credential_retained": False,
    }
    (evidence_dir / "terminal-receipt.json").write_bytes(custody.encoded(receipt))
    return directory


def mutate_json(path: Path, field: str, value) -> None:
    document = custody.load(path)
    document[field] = value
    path.write_bytes(custody.encoded(document))


def make_complete_runs(base: Path) -> Path:
    runs = base / "runs"
    for run_id in sorted(custody.static_state()["rows"]):
        custody.ingest(make_capture(base / "captures" / run_id, run_id), runs, run_id)
    (runs / "capture-manifest.json").write_bytes(
        benchmark.json_bytes(benchmark.capture_manifest(runs))
    )
    return runs


class ConfirmatoryCustodyTests(unittest.TestCase):
    def test_static_mapping_is_explicit_authorized_and_exact(self) -> None:
        state = custody.static_state()
        mapping = custody.load(custody.STUDY / "configuration-mapping.json")
        self.assertEqual(mapping["status"], "authorized")
        self.assertEqual(
            mapping["authorization_root"], state["freeze"]["authorization_root"]
        )
        self.assertEqual(
            mapping["shared_study_configuration_root"],
            state["study_configuration_root"],
        )
        self.assertEqual(
            mapping["condition_runtime_configuration_roots"],
            state["freeze"]["condition_runtime_configuration_roots"],
        )

    def test_valid_capture_ingests_only_through_bridge(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = make_capture(base / "capture", "confirm-run-01")
            run_dir = custody.ingest(capture, base / "runs", "confirm-run-01")
            validated = custody.validate_ingested_run(run_dir)
            self.assertEqual(
                validated["record"]["schema"], "vela.inherited-correction-run.v2"
            )
            self.assertEqual(validated["record"]["status"], "completed")
            self.assertEqual(validated["manifest"]["usage"]["input_tokens"], 20000)
            self.assertEqual(
                validated["manifest"]["runtime_source_root"],
                custody.static_state()["freeze"]["runtime_source_root"],
            )

    def test_missing_receipt_and_unconsumed_or_wrong_permit_fail_closed(self) -> None:
        cases = ("missing_receipt", "unconsumed", "wrong_permit")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                capture = make_capture(Path(temporary) / "capture", "confirm-run-01")
                if case == "missing_receipt":
                    (capture / "evidence/terminal-receipt.json").unlink()
                    error = "runtime_evidence_missing"
                elif case == "unconsumed":
                    consumed = capture / "permit/confirm-run-01.permit.consumed.json"
                    consumed.rename(capture / "permit/confirm-run-01.permit.json")
                    error = "runtime_permit_not_atomically_consumed"
                else:
                    source = (
                        custody.STUDY / "permit-template/confirm-run-02.permit.json"
                    )
                    shutil.copyfile(
                        source, capture / "permit/confirm-run-01.permit.consumed.json"
                    )
                    error = "runtime_consumed_permit_drift"
                with self.assertRaisesRegex(custody.CustodyError, error):
                    custody.validate_capture(capture, "confirm-run-01")

    def test_wrong_roots_fail_closed(self) -> None:
        mutations = {
            "condition": "git-documents",
            "packet_root": "sha256:" + "1" * 64,
            "prompt_root": "sha256:" + "2" * 64,
            "registration_root": "sha256:" + "3" * 64,
            "image_digest": "sha256:" + "4" * 64,
            "trust_bundle_bytes": "sha256:" + "5" * 64,
            "participant_configuration_root": "sha256:" + "6" * 64,
            "assignment_root": "sha256:" + "7" * 64,
        }
        for field, value in mutations.items():
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                capture = make_capture(Path(temporary) / "capture", "confirm-run-01")
                mutate_json(capture / "evidence/terminal-receipt.json", field, value)
                with self.assertRaisesRegex(
                    custody.CustodyError, "runtime_receipt_binding"
                ):
                    custody.validate_capture(capture, "confirm-run-01")

    def test_mapping_missing_unknown_or_drifted_fails_closed(self) -> None:
        for case in ("missing", "unknown", "drifted"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                copied = Path(temporary) / "study"
                shutil.copytree(custody.STUDY, copied)
                mapping_path = copied / "configuration-mapping.json"
                mapping = custody.load(mapping_path)
                roots = mapping["condition_runtime_configuration_roots"]
                if case == "missing":
                    del roots["vela"]
                elif case == "unknown":
                    roots["other"] = "sha256:" + "8" * 64
                else:
                    roots["vela"] = "sha256:" + "9" * 64
                mapping_path.write_bytes(custody.encoded(mapping))
                freeze_path = copied / "prelaunch-freeze.json"
                freeze = custody.load(freeze_path)
                freeze["authorized_configuration_mapping_root"] = (
                    custody.canonical_root(mapping)
                )
                for item in freeze["files"]:
                    if item["path"] == "configuration-mapping.json":
                        item["bytes"] = len(mapping_path.read_bytes())
                        item["sha256"] = custody.digest(mapping_path.read_bytes())
                freeze_path.write_bytes(custody.encoded(freeze))
                with (
                    patch.object(custody, "STUDY", copied),
                    self.assertRaises(custody.CustodyError),
                ):
                    custody.static_state()

    def test_event_response_time_status_and_usage_drift_fail_closed(self) -> None:
        cases = ("events", "response", "time", "status", "usage", "attempt", "timeout")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                capture = make_capture(Path(temporary) / "capture", "confirm-run-01")
                receipt_path = capture / "evidence/terminal-receipt.json"
                if case == "events":
                    path = capture / "evidence/provider-events.jsonl"
                    path.write_bytes(path.read_bytes() + b"drift\n")
                    error = "provider_events_drift"
                elif case == "response":
                    path = capture / "evidence/participant-response.raw.json"
                    path.write_bytes(path.read_bytes() + b" ")
                    error = "runtime_response_drift"
                elif case == "time":
                    mutate_json(
                        receipt_path, "provider_completed_at", "2026-08-22T00:01:05Z"
                    )
                    error = "duration_timestamp_mismatch"
                elif case == "status":
                    mutate_json(receipt_path, "status", "non_result")
                    error = "non_result_reason_missing"
                elif case == "attempt":
                    mutate_json(receipt_path, "attempt", 2)
                    error = "receipt_binding"
                elif case == "timeout":
                    mutate_json(receipt_path, "timeout_seconds", 599)
                    error = "receipt_binding"
                else:
                    events_path = capture / "evidence/provider-events.jsonl"
                    events = [
                        json.loads(line)
                        for line in events_path.read_text().splitlines()
                    ]
                    events[-1]["usage"]["input_tokens"] = -1
                    changed = b"".join(
                        json.dumps(event, separators=(",", ":")).encode() + b"\n"
                        for event in events
                    )
                    events_path.write_bytes(changed)
                    receipt = custody.load(receipt_path)
                    receipt["provider_events_bytes"] = custody.digest(changed)
                    receipt_path.write_bytes(custody.encoded(receipt))
                    error = "provider_usage_invalid"
                with self.assertRaisesRegex(custody.CustodyError, error):
                    custody.validate_capture(capture, "confirm-run-01")

    def test_provider_failure_and_timeout_are_retained_without_retry(self) -> None:
        for case in ("failure", "timeout"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                capture = make_capture(base / "capture", "confirm-run-01")
                receipt_path = capture / "evidence/terminal-receipt.json"
                receipt = custody.load(receipt_path)
                receipt["status"] = "non_result"
                receipt["event_receipt"] = None
                if case == "failure":
                    receipt["validation_error"] = "provider_exit_1"
                    receipt["process_exit_code"] = 1
                    expected = "failed"
                else:
                    receipt["validation_error"] = "timeout"
                    receipt["process_exit_code"] = None
                    receipt["process_timed_out"] = True
                    receipt["provider_completed_at"] = "2026-08-22T00:10:01Z"
                    receipt["duration_seconds"] = 600.0
                    response_path = capture / "evidence/participant-response.raw.json"
                    response_path.unlink()
                    receipt["response_bytes"] = None
                    expected = "timed_out"
                receipt_path.write_bytes(custody.encoded(receipt))
                run_dir = custody.ingest(capture, base / "runs", "confirm-run-01")
                validated = custody.validate_ingested_run(run_dir)
                self.assertEqual(validated["record"]["status"], expected)
                self.assertFalse((run_dir / "response.json").exists())
                self.assertEqual(validated["record"]["attempt"], 1)

    def test_complete_fixed_denominator_captures_without_scoring(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            runs = base / "runs"
            for run_id in sorted(custody.static_state()["rows"]):
                custody.ingest(
                    make_capture(base / "captures" / run_id, run_id), runs, run_id
                )
            complete = custody.complete_custody(runs)
            self.assertEqual(len(complete["runs"]), 16)
            with patch.object(
                benchmark, "ADJUDICATION_PATH", base / "protected-key-not-opened.json"
            ):
                capture = benchmark.capture_manifest(runs)
            self.assertEqual(
                capture["complete_runtime_custody_root"],
                complete["complete_runtime_custody_root"],
            )
            self.assertFalse(capture["adjudication_accessed"])

    def test_gate_boundary_run_and_response_mutations_fail_before_key(self) -> None:
        cases = ("run", "response")
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                runs = make_complete_runs(base)
                original_verify = benchmark.verify_capture_manifest

                def verify_then_mutate(runs_dir: Path):
                    capture = original_verify(runs_dir)
                    if case == "run":
                        path = runs_dir / "confirm-run-01/run.json"
                        value = benchmark.load_json(path)
                        value["tool_calls"] = 1
                    else:
                        path = runs_dir / "confirm-run-01/response.json"
                        value = benchmark.load_json(path)
                        value["consequences"][0]["action_code"] = (
                            "no_correction_reassessment"
                        )
                        benchmark.validate_response(value)
                    path.write_bytes(benchmark.json_bytes(value))
                    return capture

                expected = (
                    "capture_run_bytes_drift"
                    if case == "run"
                    else "capture_response_bytes_drift"
                )
                with (
                    patch.object(
                        benchmark,
                        "verify_capture_manifest",
                        side_effect=verify_then_mutate,
                    ),
                    patch.object(
                        benchmark,
                        "load_registered_adjudication",
                        side_effect=AssertionError("protected key opened"),
                    ),
                    self.assertRaisesRegex(benchmark.BenchmarkError, expected),
                ):
                    benchmark.score_runs(runs)

    def test_missing_duplicate_and_synthetic_records_never_open_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            runs = base / "runs"
            custody.ingest(
                make_capture(base / "capture", "confirm-run-01"),
                runs,
                "confirm-run-01",
            )
            with self.assertRaisesRegex(custody.CustodyError, "fixed_denominator"):
                custody.complete_custody(runs)
            duplicate = base / "duplicate"
            shutil.copytree(runs / "confirm-run-01", duplicate / "confirm-run-02")
            with self.assertRaises(custody.CustodyError):
                custody.validate_ingested_run(duplicate / "confirm-run-02")
        with tempfile.TemporaryDirectory() as temporary:
            runs = Path(temporary)
            prereg = benchmark.load_json(benchmark.PREREG_PATH)
            equivalence = benchmark.load_json(benchmark.EQUIVALENCE_PATH)
            authorization = custody.static_state()["authorization"]
            for row in custody.static_state()["rows"].values():
                run_dir = runs / row["run_id"]
                run_dir.mkdir()
                shutil.copytree(
                    benchmark.ROOT / "conditions" / row["condition"], run_dir / "packet"
                )
                (run_dir / "authorization.json").write_bytes(
                    benchmark.json_bytes(authorization)
                )
                record = {
                    "schema": "vela.inherited-correction-run.v1",
                    "run_id": row["run_id"],
                    "participant_instance_id": row["participant_instance_id"],
                    "participant_configuration_root": authorization[
                        "participant_configuration_root"
                    ],
                    "condition": row["condition"],
                    "packet_root": equivalence["condition_packet_roots"][
                        row["condition"]
                    ],
                    "registration_root": prereg["registration_root"],
                    "authorization_root": benchmark.canonical_root(authorization),
                    "status": "completed",
                    "started_at": "2026-08-22T00:00:01Z",
                    "completed_at": "2026-08-22T00:00:05Z",
                    "duration_seconds": 4,
                    "tool_calls": 0,
                    "timeout_seconds": 600,
                    "attempt": 1,
                }
                (run_dir / "run.json").write_bytes(benchmark.json_bytes(record))
                (run_dir / "response.json").write_bytes(
                    benchmark.json_bytes(response())
                )
            with self.assertRaisesRegex(
                benchmark.BenchmarkError, "runtime_ingested_run_schema"
            ):
                benchmark.capture_manifest(runs)


if __name__ == "__main__":
    unittest.main()
