from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("measure.py")
SPEC = importlib.util.spec_from_file_location("cost_measure", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


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


if __name__ == "__main__":
    unittest.main()
