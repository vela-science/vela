from __future__ import annotations

import copy
import unittest
from pathlib import Path

from score import ScoreInputError, load_object, score, sha256_file


HERE = Path(__file__).resolve().parent
PLAN = HERE / "plan.v1.json"
KEY = HERE / "answer-key.v1.json"


def answer(arm: str = "git_files", session_id: str = "git-files-01"):
    process = {
        "attempt_started": False,
        "attempt_start_method": "manual_inspection_only",
        "attempt_id": None,
        "authorization_root": None,
        "elapsed_ms": 1000,
        "observed_tokens": 100,
        "command_count": 10,
        "intervention_count": 0,
        "command_log_root": "sha256:" + "0" * 64,
    }
    if arm == "vela_guided":
        process.update(
            {
                "attempt_started": True,
                "attempt_start_method": "vela_start",
                "attempt_id": "vat_" + "1" * 64,
                "authorization_root": "sha256:" + "2" * 64,
            }
        )
    return {
        "schema": "vela.product-compression-answer.v1",
        "plan_root": sha256_file(PLAN),
        "session_id": session_id,
        "arm": arm,
        "process": process,
        "answers": copy.deepcopy(load_object(KEY)["expected"]),
    }


class ScoreTests(unittest.TestCase):
    def test_exact_answer_passes(self):
        result = score(PLAN, load_object(KEY), answer())
        self.assertEqual(result["score_basis_points"], 10000)
        self.assertTrue(result["passed"])
        self.assertEqual(result["hard_failures"], [])

    def test_guided_process_requires_retained_attempt(self):
        candidate = answer("vela_guided", "vela-guided-01")
        candidate["process"]["attempt_id"] = None
        with self.assertRaisesRegex(
            ScoreInputError, "full vat_ identifier"
        ):
            score(PLAN, load_object(KEY), candidate)

    def test_readiness_is_not_recommendation_is_hard_gate(self):
        candidate = answer()
        candidate["answers"]["inbox"][
            "ready_is_acceptance_recommendation"
        ] = True
        result = score(PLAN, load_object(KEY), candidate)
        self.assertFalse(result["passed"])
        self.assertIn(
            "inbox.ready_is_acceptance_recommendation",
            result["hard_failures"],
        )

    def test_wrong_next_obligation_is_hard_gate(self):
        candidate = answer()
        candidate["answers"]["terminal_correction"][
            "next_obligation_code"
        ] = "accept_every_dependent"
        result = score(PLAN, load_object(KEY), candidate)
        self.assertFalse(result["passed"])
        self.assertIn(
            "terminal_correction.next_obligation_code",
            result["hard_failures"],
        )

    def test_set_fields_are_order_independent(self):
        candidate = answer()
        candidate["answers"]["attempt"]["allowed_operations"].reverse()
        candidate["answers"]["terminal_correction"][
            "scope_limit_codes"
        ].reverse()
        result = score(PLAN, load_object(KEY), candidate)
        self.assertEqual(result["score_basis_points"], 10000)
        self.assertTrue(result["passed"])

    def test_extra_answer_field_fails_closed(self):
        candidate = answer()
        candidate["answers"]["run"]["invented"] = True
        with self.assertRaisesRegex(ScoreInputError, "leaf paths differ"):
            score(PLAN, load_object(KEY), candidate)


if __name__ == "__main__":
    unittest.main()
