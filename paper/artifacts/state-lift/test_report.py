#!/usr/bin/env python3
"""Tests for the terminal state-lift report."""

from __future__ import annotations

import unittest

from report import percent_reduction


class ReportTests(unittest.TestCase):
    def test_percent_reduction(self) -> None:
        self.assertEqual(percent_reduction(100, 25), 75.0)

    def test_percent_reduction_rejects_zero_baseline(self) -> None:
        with self.assertRaisesRegex(ValueError, "positive"):
            percent_reduction(0, 1)


if __name__ == "__main__":
    unittest.main()
