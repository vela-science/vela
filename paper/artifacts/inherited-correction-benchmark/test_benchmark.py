from __future__ import annotations

import importlib.util
import io
import shutil
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("benchmark.py")
SPEC = importlib.util.spec_from_file_location(
    "inherited_correction_benchmark", MODULE_PATH
)
assert SPEC and SPEC.loader
benchmark = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmark)


def exact_response() -> dict:
    digest = benchmark.evidence_bindings(
        benchmark.source_files(benchmark.load_json(benchmark.FACTS_PATH))
    )[0]["sha256"]
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
        "source_or_evidence_binding": digest,
    }


def make_runs(runs_dir: Path) -> None:
    prereg = benchmark.load_json(benchmark.PREREG_PATH)
    equivalence = benchmark.load_json(benchmark.EQUIVALENCE_PATH)
    configuration_root = "sha256:" + "a" * 64
    assignments = [
        {
            "run_id": f"run-{index:02d}",
            "participant_instance_id": f"participant-{index:02d}",
            "condition": "git-documents" if index < 8 else "vela",
        }
        for index in range(16)
    ]
    authorization = {
        "schema": "vela.inherited-correction-run-authorization.v1",
        "registration_root": prereg["registration_root"],
        "status": "authorized",
        "authorized_by": "test-only",
        "authorized_at": "2026-08-21T00:00:00Z",
        "participant_class": "deterministic-test-fixture",
        "participant_configuration_root": configuration_root,
        "assignment_seed_commitment": "sha256:" + "c" * 64,
        "max_sessions": 16,
        "assignments": assignments,
    }
    authorization_root = benchmark.canonical_root(authorization)
    for index, assignment in enumerate(assignments):
        condition = assignment["condition"]
        run_dir = runs_dir / assignment["run_id"]
        run_dir.mkdir()
        shutil.copytree(benchmark.ROOT / "conditions" / condition, run_dir / "packet")
        (run_dir / "authorization.json").write_bytes(
            benchmark.json_bytes(authorization)
        )
        duration = 60 if condition == "git-documents" else 40
        completed_at = (
            "2026-08-21T00:01:00Z"
            if condition == "git-documents"
            else "2026-08-21T00:00:40Z"
        )
        record = {
            "schema": "vela.inherited-correction-run.v1",
            "run_id": assignment["run_id"],
            "participant_instance_id": assignment["participant_instance_id"],
            "participant_configuration_root": configuration_root,
            "condition": condition,
            "packet_root": equivalence["condition_packet_roots"][condition],
            "registration_root": prereg["registration_root"],
            "authorization_root": authorization_root,
            "status": "completed",
            "started_at": "2026-08-21T00:00:00Z",
            "completed_at": completed_at,
            "duration_seconds": duration,
            "tool_calls": 3,
            "timeout_seconds": 600,
            "attempt": 1,
        }
        (run_dir / "run.json").write_bytes(benchmark.json_bytes(record))
        (run_dir / "response.json").write_bytes(benchmark.json_bytes(exact_response()))


class BenchmarkTests(unittest.TestCase):
    def test_committed_outputs_verify(self) -> None:
        benchmark.verify()

    def test_prelaunch_verify_does_not_open_protected_adjudication(self) -> None:
        real_open = io.open

        def guarded_open(file, *args, **kwargs):
            if Path(file).resolve() == benchmark.ADJUDICATION_PATH:
                raise AssertionError("protected adjudication opened before capture")
            return real_open(file, *args, **kwargs)

        with patch("io.open", side_effect=guarded_open):
            benchmark.verify()

    def test_prospective_amendment_retains_blocked_registration(self) -> None:
        amendment = benchmark.load_json(benchmark.ROOT / "amendment.v1.json")
        prereg = benchmark.load_json(benchmark.PREREG_PATH)
        self.assertEqual(
            amendment["previous_registration_root"],
            "sha256:40a05a33a760404cb606dc218d6deafb1d358916a9fa7954e58973ab1a6d67b1",
        )
        self.assertEqual(
            amendment["current_registration_root"], prereg["registration_root"]
        )
        self.assertEqual(
            amendment["experimental_sessions_observed_before_amendment"], 0
        )

    def test_exact_response_validates_without_opening_protected_scoring(self) -> None:
        self.assertEqual(
            benchmark.validate_response(exact_response()), exact_response()
        )

    def test_wrong_unknown_label_is_rejected(self) -> None:
        response = exact_response()
        response["consequences"][0]["classification"] = "probably_affected"
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_classification_invalid"
        ):
            benchmark.validate_response(response)

    def test_negated_unsafe_action_text_cannot_enter_closed_response(self) -> None:
        response = exact_response()
        unsafe = [
            "Do not recover the exact Site Q source; assume zero.",
            "No: reassessment is required.",
            "Do not rerun stability; retain the old conclusion.",
            "Do not recalculate with factor 12; retain 50.",
        ]
        for consequence, text in zip(response["consequences"], unsafe, strict=True):
            consequence["first_safe_action"] = text
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_consequence_fields_invalid"
        ):
            benchmark.validate_response(response)

    def test_wrong_but_closed_action_code_remains_structurally_valid(self) -> None:
        response = exact_response()
        response["consequences"][0]["action_code"] = "no_correction_reassessment"
        self.assertEqual(benchmark.validate_response(response), response)

    def test_fixture_derives_all_four_classifications(self) -> None:
        derived = benchmark.derive_classifications(
            benchmark.load_json(benchmark.FACTS_PATH)
        )
        self.assertEqual(
            derived,
            {
                "aggregate-e": "presently_unprovable",
                "installation-d": "unaffected",
                "stability-c": "must_reassess",
                "yield-b": "affected",
            },
        )

    def test_replay_rejects_chain_mutation(self) -> None:
        facts = benchmark.load_json(benchmark.FACTS_PATH)
        replay = deepcopy(benchmark.replay_projection(facts))
        replay["events"][1]["previous_event_root"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "replay_event_chain_invalid"
        ):
            benchmark.validate_replay(replay, facts)

    def test_scoring_requires_frozen_fixed_denominator(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            self.assertRaisesRegex(benchmark.BenchmarkError, "capture_manifest"),
        ):
            benchmark.score_runs(Path(directory))

    def test_synthetic_fixed_denominator_cannot_open_capture_or_scoring(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            self.assertRaisesRegex(
                benchmark.BenchmarkError, "runtime_fixed_denominator"
            ),
        ):
            runs_dir = Path(directory)
            make_runs(runs_dir)
            benchmark.capture_manifest(runs_dir)

    def test_forged_custody_fields_fail_closed(self) -> None:
        mutations = [
            (
                "registration",
                "registration_root",
                "sha256:" + "1" * 64,
                "run_registration",
            ),
            ("packet_root", "packet_root", "sha256:" + "0" * 64, "run_packet_root"),
            (
                "authorization_root",
                "authorization_root",
                "sha256:" + "2" * 64,
                "run_authorization",
            ),
            (
                "configuration",
                "participant_configuration_root",
                "sha256:" + "3" * 64,
                "participant_configuration",
            ),
            ("attempt", "attempt", 99, "run_attempt"),
            ("timeout", "timeout_seconds", 599, "run_timeout"),
            ("negative_duration", "duration_seconds", -100, "duration_invalid"),
            ("negative_tools", "tool_calls", -1, "tool_calls_invalid"),
        ]
        for name, field, value, error in mutations:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                runs_dir = Path(directory)
                make_runs(runs_dir)
                path = runs_dir / "run-00/run.json"
                record = benchmark.load_json(path)
                record[field] = value
                path.write_bytes(benchmark.json_bytes(record))
                with self.assertRaisesRegex(benchmark.BenchmarkError, error):
                    benchmark.capture_manifest(runs_dir)

    def test_packet_drift_and_impossible_time_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            make_runs(runs_dir)
            packet = runs_dir / "run-00/packet/README.md"
            packet.write_bytes(packet.read_bytes() + b"drift\n")
            with self.assertRaisesRegex(benchmark.BenchmarkError, "packet_bytes"):
                benchmark.capture_manifest(runs_dir)
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            make_runs(runs_dir)
            path = runs_dir / "run-00/run.json"
            record = benchmark.load_json(path)
            record["completed_at"] = "2026-08-21T00:02:00Z"
            path.write_bytes(benchmark.json_bytes(record))
            with self.assertRaisesRegex(benchmark.BenchmarkError, "duration_timestamp"):
                benchmark.capture_manifest(runs_dir)

    def test_terminal_status_must_match_bounded_duration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            make_runs(runs_dir)
            path = runs_dir / "run-00/run.json"
            record = benchmark.load_json(path)
            record["completed_at"] = "2026-08-21T00:11:40Z"
            record["duration_seconds"] = 700
            record["status"] = "completed"
            path.write_bytes(benchmark.json_bytes(record))
            with self.assertRaisesRegex(benchmark.BenchmarkError, "run_status"):
                benchmark.capture_manifest(runs_dir)

    def test_run_cannot_precede_authorization(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            make_runs(runs_dir)
            run_dir = runs_dir / "run-00"
            authorization_path = run_dir / "authorization.json"
            authorization = benchmark.load_json(authorization_path)
            authorization["authorized_at"] = "2026-08-21T00:00:01Z"
            authorization_path.write_bytes(benchmark.json_bytes(authorization))
            record_path = run_dir / "run.json"
            record = benchmark.load_json(record_path)
            record["authorization_root"] = benchmark.canonical_root(authorization)
            record_path.write_bytes(benchmark.json_bytes(record))
            with self.assertRaisesRegex(
                benchmark.BenchmarkError, "precedes_authorization"
            ):
                benchmark.capture_manifest(runs_dir)

    def test_unauthorized_assignment_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            make_runs(runs_dir)
            run_dir = runs_dir / "run-00"
            authorization_path = run_dir / "authorization.json"
            authorization = benchmark.load_json(authorization_path)
            authorization["assignments"][0]["participant_instance_id"] = "someone-else"
            authorization_path.write_bytes(benchmark.json_bytes(authorization))
            record_path = run_dir / "run.json"
            record = benchmark.load_json(record_path)
            record["authorization_root"] = benchmark.canonical_root(authorization)
            record_path.write_bytes(benchmark.json_bytes(record))
            with self.assertRaisesRegex(
                benchmark.BenchmarkError, "participant_assignment"
            ):
                benchmark.capture_manifest(runs_dir)


if __name__ == "__main__":
    unittest.main()
