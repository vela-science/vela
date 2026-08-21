from __future__ import annotations

import importlib.util
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

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
                "first_safe_action": "Recover the exact Site Q source binding.",
            },
            {
                "claim_id": "installation-d",
                "classification": "unaffected",
                "first_safe_action": "No correction-driven reassessment is required.",
            },
            {
                "claim_id": "stability-c",
                "classification": "must_reassess",
                "first_safe_action": "Rerun the stability method.",
            },
            {
                "claim_id": "yield-b",
                "classification": "affected",
                "first_safe_action": "Recalculate with factor 12.",
            },
        ],
        "standing_effect": "none",
        "source_or_evidence_binding": digest,
    }


class BenchmarkTests(unittest.TestCase):
    def test_committed_outputs_verify(self) -> None:
        benchmark.verify()

    def test_exact_response_scores_full_credit(self) -> None:
        score = benchmark.score_response(exact_response())
        self.assertTrue(score["exact_success"])
        self.assertEqual(score["points"], 17)
        self.assertFalse(score["authority_error"])

    def test_wrong_unknown_label_is_rejected(self) -> None:
        response = exact_response()
        response["consequences"][0]["classification"] = "probably_affected"
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_classification_invalid"
        ):
            benchmark.validate_response(response)

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

    def test_fixed_capture_and_scoring(self) -> None:
        prereg = benchmark.load_json(benchmark.PREREG_PATH)
        with tempfile.TemporaryDirectory() as directory:
            runs_dir = Path(directory)
            for index in range(16):
                condition = "git-documents" if index < 8 else "vela"
                run_dir = runs_dir / f"run-{index:02d}"
                run_dir.mkdir()
                duration = 60 if condition == "git-documents" else 40
                record = {
                    "schema": "vela.inherited-correction-run.v1",
                    "run_id": run_dir.name,
                    "participant_instance_id": f"participant-{index:02d}",
                    "participant_configuration_root": "sha256:" + "a" * 64,
                    "condition": condition,
                    "packet_root": benchmark.load_json(benchmark.EQUIVALENCE_PATH)[
                        "condition_packet_roots"
                    ][condition],
                    "registration_root": prereg["registration_root"],
                    "authorization_root": "sha256:" + "b" * 64,
                    "status": "completed",
                    "started_at": "2026-08-21T00:00:00Z",
                    "completed_at": "2026-08-21T00:01:00Z",
                    "duration_seconds": duration,
                    "tool_calls": 3,
                    "timeout_seconds": 600,
                    "attempt": 1,
                }
                (run_dir / "run.json").write_bytes(benchmark.json_bytes(record))
                (run_dir / "response.json").write_bytes(
                    benchmark.json_bytes(exact_response())
                )
            capture = benchmark.capture_manifest(runs_dir)
            (runs_dir / "capture-manifest.json").write_bytes(
                benchmark.json_bytes(capture)
            )
            result = benchmark.score_runs(runs_dir)
            self.assertEqual(result["positive_gate"], "pass")
            self.assertEqual(result["fixed_denominator"], 16)


if __name__ == "__main__":
    unittest.main()
