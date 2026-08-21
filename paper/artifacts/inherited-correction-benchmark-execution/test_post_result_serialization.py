#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("canonicalize-post-result.py")
SPEC = importlib.util.spec_from_file_location("post_result_canonicalizer", MODULE_PATH)
assert SPEC and SPEC.loader
canonicalizer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(canonicalizer)


class PostResultSerializationTests(unittest.TestCase):
    def test_decimal_fixture_has_exact_cross_version_bytes(self) -> None:
        self.assertEqual(
            canonicalizer.check_fixture(),
            "sha256:0edc8b9adea2302c60ac988c9a27c0b5e7c3148152ecbae4dcb41fb613159473",
        )


if __name__ == "__main__":
    unittest.main()
