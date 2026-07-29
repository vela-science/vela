#!/usr/bin/env python3
"""Tests for the exact Formal Erdős 505 replay verifier."""

from __future__ import annotations

import unittest

from verify_replay import (
    EXPECTED_MISSION_ROOT,
    EXPECTED_RUN_ID,
    EXPECTED_STDERR_ROOT,
    EXPECTED_STDOUT_ROOT,
    ReplayError,
    validate_replay,
)


class ReplayTests(unittest.TestCase):
    def fixture(self) -> dict[str, object]:
        return {
            "schema": "canopus.replay.v1",
            "ok": True,
            "run_id": EXPECTED_RUN_ID,
            "mission_root": EXPECTED_MISSION_ROOT,
            "verifier_status": "passed",
            "stdout_digest": EXPECTED_STDOUT_ROOT,
            "stderr_digest": EXPECTED_STDERR_ROOT,
            "matched": True,
        }

    def test_exact_replay_passes(self) -> None:
        self.assertEqual(validate_replay(self.fixture())["matched"], True)

    def test_root_drift_fails(self) -> None:
        value = self.fixture()
        value["stdout_digest"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(ReplayError, "stdout root drift"):
            validate_replay(value)


if __name__ == "__main__":
    unittest.main()
