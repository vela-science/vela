from __future__ import annotations

import importlib.util
import hashlib
import json
import statistics
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("measure.py")
SPEC = importlib.util.spec_from_file_location("cost_measure", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
HERE = Path(__file__).resolve().parent


def deterministic_projection(result: dict[str, object]) -> dict[str, object]:
    projected_frontiers = []
    for frontier in result["frontiers"]:
        operations = {
            name: {"normalized_output_root": observation["normalized_output_root"]}
            for name, observation in frontier["operations"].items()
        }
        projected_frontiers.append(
            {
                "name": frontier["name"],
                "git_commit": frontier["git_commit"],
                "git_tree": frontier["git_tree"],
                "repository_root": frontier["repository_root"],
                "counts": frontier["counts"],
                "storage": frontier["storage"],
                "operations": operations,
            }
        )
    return {
        "schema": result["schema"],
        "plan_root": result["plan_root"],
        "vela": result["vela"],
        "frontiers": projected_frontiers,
        "limits": result["limits"],
    }


class CostMeasurementTests(unittest.TestCase):
    def test_normalization_removes_path_duration_and_json_formatting(self) -> None:
        frontier = Path("/tmp/private/frontier")
        raw_json = json.dumps(
            {"frontier": str(frontier), "ok": True},
            indent=2,
        ).encode()
        self.assertEqual(
            MODULE.normalize_output(raw_json, frontier),
            b'{"frontier":"<frontier>","ok":true}\n',
        )
        raw_text = (
            f"VELA {frontier}\n"
            "reproduce: ok (38/38) - every witness verified (1.63s)\n"
        ).encode()
        self.assertEqual(
            MODULE.normalize_output(raw_text, frontier),
            (
                "VELA <frontier>\n"
                "reproduce: ok (38/38) - every witness verified (<duration>)\n"
            ).encode(),
        )

    def test_summary_retains_samples_and_exact_median(self) -> None:
        self.assertEqual(
            MODULE.summarize([1_000_000, 3_000_000, 2_000_000]),
            {
                "samples_ms": [1.0, 3.0, 2.0],
                "minimum_ms": 1.0,
                "median_ms": 2.0,
                "maximum_ms": 3.0,
            },
        )

    def test_frontier_order_is_frozen(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            values = [
                f"erdos={directory}",
                f"formal={directory}",
                f"sidon={directory}",
                f"quantum={directory}",
            ]
            parsed = MODULE.parse_frontiers(values)
            self.assertEqual(tuple(parsed), MODULE.EXPECTED_FRONTIERS)
            with self.assertRaisesRegex(ValueError, "frozen order"):
                MODULE.parse_frontiers(list(reversed(values)))

    def test_registered_result_retains_the_frozen_plan_and_all_samples(self) -> None:
        plan = json.loads((HERE / "plan.json").read_bytes())
        result_path = HERE / "result.json"
        result = json.loads(result_path.read_bytes())

        self.assertEqual(
            hashlib.sha256(result_path.read_bytes()).hexdigest(),
            "1ba33ce4387c624c7c0381091140db34bb7ff4bf933ce56d0abe5479cf495acd",
        )
        self.assertEqual(result["schema"], "vela.cost-evaluation-result.v1")
        self.assertEqual(result["plan_root"], MODULE.sha256(MODULE.canonical_bytes(plan)))
        self.assertEqual(result["limits"], plan["does_not_establish"])
        self.assertEqual(
            tuple(frontier["name"] for frontier in result["frontiers"]),
            MODULE.EXPECTED_FRONTIERS,
        )
        self.assertRegex(result["vela"]["binary_root"], r"^sha256:[0-9a-f]{64}$")

        expected_operations = {
            "erdos": {"status", "strict_check", "review_show", "reproduce"},
            "formal": {"status", "strict_check"},
            "sidon": {"status", "strict_check"},
            "quantum": {"status", "strict_check"},
        }
        repetitions = plan["sampling"]["repetitions"]
        for frontier in result["frontiers"]:
            self.assertRegex(frontier["git_commit"], r"^[0-9a-f]{40}$")
            self.assertRegex(frontier["git_tree"], r"^[0-9a-f]{40}$")
            self.assertRegex(frontier["repository_root"], r"^sha256:[0-9a-f]{64}$")
            self.assertEqual(set(frontier["operations"]), expected_operations[frontier["name"]])
            self.assertGreater(frontier["storage"]["tracked_file_count"], 0)
            self.assertGreater(frontier["storage"]["tracked_file_bytes"], 0)
            for observation in frontier["operations"].values():
                samples = observation["samples_ms"]
                self.assertEqual(len(samples), repetitions)
                self.assertEqual(observation["minimum_ms"], min(samples))
                self.assertEqual(observation["maximum_ms"], max(samples))
                self.assertEqual(
                    observation["median_ms"],
                    round(statistics.median(samples), 3),
                )
                self.assertRegex(
                    observation["normalized_output_root"],
                    r"^sha256:[0-9a-f]{64}$",
                )

    def test_isolated_reproduction_matches_every_deterministic_field(self) -> None:
        registered = json.loads((HERE / "result.json").read_bytes())
        reproduction_path = HERE / "reproduction.json"
        reproduction = json.loads(reproduction_path.read_bytes())

        self.assertEqual(
            hashlib.sha256(reproduction_path.read_bytes()).hexdigest(),
            "8ee2588e3745324555862a14a7559d2374984661aa5ce783d6ed7c400b02599b",
        )
        self.assertEqual(
            deterministic_projection(reproduction),
            deterministic_projection(registered),
        )
        projection = MODULE.canonical_bytes(deterministic_projection(registered))
        self.assertEqual(
            hashlib.sha256(projection).hexdigest(),
            "f30d4c3464618e0159603ae8adaf58eb7addd63a4ce00f7a1d3fec18d2f85bd3",
        )


if __name__ == "__main__":
    unittest.main()
