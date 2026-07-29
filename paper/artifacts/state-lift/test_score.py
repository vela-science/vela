#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("score.py")
SPEC = importlib.util.spec_from_file_location("state_lift_score", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
SCORER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCORER)


def expected() -> dict:
    return {
        "predecessor": {
            "claim_id": "vcl_predecessor",
            "claim_root": "sha256:" + "1" * 64,
            "standing": "superseded",
        },
        "replacement": {
            "claim_id": "vcl_replacement",
            "claim_root": "sha256:" + "2" * 64,
            "standing": "accepted",
        },
        "source_transition": {
            "path": "FormalConjectures/ErdosProblems/424.lean",
            "predecessor_commit": "a" * 40,
            "predecessor_file_root": "sha256:" + "3" * 64,
            "predecessor_predicate": "generatedSet.HasPosDensity",
            "successor_commit": "b" * 40,
            "successor_file_root": "sha256:" + "4" * 64,
            "successor_predicate": "generatedSet.HasPosLowerDensity",
        },
        "evidence": {
            "submission_id": "vsb_example",
            "submission_root": "sha256:" + "5" * 64,
            "verification_ids": ["vvr_one", "vvr_two"],
            "decision_id": "vdc_example",
            "decision_root": "sha256:" + "6" * 64,
            "event_ids": ["vev_example"],
        },
        "accepted_state_delta": {
            "registration": 0,
            "verification": 0,
        },
        "authority": {
            "verification_changed_standing": False,
            "model_or_tool_has_decision_authority": False,
        },
        "next_action_code": "inspect_downstream_impact",
        "scope_limit_codes": [
            "does_not_prove_erdos_424",
            "does_not_establish_unique_informal_interpretation",
        ],
    }


def answer_key() -> dict:
    return {
        "schema": SCORER.ANSWER_KEY_SCHEMA,
        "task_instance_root": "sha256:" + "7" * 64,
        "expected": expected(),
    }


def answer() -> dict:
    return {
        "schema": SCORER.ANSWER_SCHEMA,
        "task_instance_root": "sha256:" + "7" * 64,
        "session_id": "session-1",
        "arm": "git",
        "answers": expected(),
    }


class ScoreTests(unittest.TestCase):
    def test_exact_answer_passes(self) -> None:
        result = SCORER.score(answer_key(), answer())
        self.assertTrue(result["all_fields_correct"])
        self.assertEqual(result["hard_failures"], [])
        self.assertEqual(result["correct_fields"], result["total_fields"])
        self.assertTrue(result["result_root"].startswith("sha256:"))

    def test_set_fields_ignore_order(self) -> None:
        observed = answer()
        observed["answers"]["evidence"]["verification_ids"].reverse()
        observed["answers"]["scope_limit_codes"].reverse()
        result = SCORER.score(answer_key(), observed)
        self.assertTrue(result["all_fields_correct"])

    def test_false_authority_is_a_hard_failure(self) -> None:
        observed = answer()
        observed["answers"]["authority"]["verification_changed_standing"] = True
        result = SCORER.score(answer_key(), observed)
        self.assertFalse(result["all_fields_correct"])
        self.assertIn(
            "verification_described_as_standing_change",
            result["hard_failures"],
        )

    def test_wrong_source_root_is_a_hard_failure(self) -> None:
        observed = answer()
        observed["answers"]["source_transition"]["successor_file_root"] = (
            "sha256:" + "8" * 64
        )
        result = SCORER.score(answer_key(), observed)
        self.assertIn(
            "wrong_source_transition_successor_file_root",
            result["hard_failures"],
        )

    def test_task_instance_mismatch_fails_closed(self) -> None:
        observed = answer()
        observed["task_instance_root"] = "sha256:" + "9" * 64
        with self.assertRaises(SCORER.ScoreInputError):
            SCORER.score(answer_key(), observed)

    def test_extra_field_fails_closed(self) -> None:
        observed = answer()
        observed["answers"]["authority"]["confidence"] = 1
        with self.assertRaises(SCORER.ScoreInputError):
            SCORER.score(answer_key(), observed)

    def test_result_root_is_deterministic(self) -> None:
        first = SCORER.score(answer_key(), answer())
        second = SCORER.score(copy.deepcopy(answer_key()), copy.deepcopy(answer()))
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
