from __future__ import annotations

import unittest

from report import ReportInputError, load, report
from score import load_object, score
from test_score import KEY, PLAN, answer


def exact_scores():
    plan = load(PLAN)
    scores = []
    for index, assignment in enumerate(plan["assignment"]):
        arm = assignment["arm"]
        candidate = answer(arm, assignment["session_id"])
        candidate["process"]["elapsed_ms"] = (
            700 + index if arm == "vela_guided" else 1000 + index
        )
        candidate["process"]["observed_tokens"] = (
            7000 + index if arm == "vela_guided" else 10000 + index
        )
        scores.append(score(PLAN, load_object(KEY), candidate))
    return plan, scores


class ReportTests(unittest.TestCase):
    def test_exact_assignment_with_twenty_percent_lift_passes(self):
        plan, scores = exact_scores()
        result = report(PLAN, plan, scores)
        self.assertTrue(
            result["comparison"]["product_compression_gate_passed"]
        )
        self.assertGreaterEqual(
            result["comparison"]["median_elapsed_time_reduction"], 0.20
        )
        self.assertEqual(
            result["classification"], "first_party_fresh_session_only"
        )

    def test_assignment_order_drift_fails(self):
        plan, scores = exact_scores()
        scores[0], scores[1] = scores[1], scores[0]
        with self.assertRaisesRegex(
            ReportInputError, "order/assignment differs"
        ):
            report(PLAN, plan, scores)

    def test_neutral_time_result_does_not_pass_product_gate(self):
        plan, scores = exact_scores()
        for value in scores:
            value["process"]["elapsed_ms"] = 1000
            without_root = {
                key: item
                for key, item in value.items()
                if key != "result_root"
            }
            from report import root

            value["result_root"] = root(without_root)
        result = report(PLAN, plan, scores)
        self.assertFalse(
            result["comparison"]["product_compression_gate_passed"]
        )


if __name__ == "__main__":
    unittest.main()
