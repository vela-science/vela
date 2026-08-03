#!/usr/bin/env python3
"""Focused tests for the exact Erdős 264 source-transition verifier."""

from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify_source_transition.py")
SPEC = importlib.util.spec_from_file_location("erdos_264_verifier", MODULE_PATH)
assert SPEC and SPEC.loader
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


class SourceTransitionTests(unittest.TestCase):
    def test_changed_artifact_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            changed = Path(directory) / "changed.json"
            changed.write_bytes(b"{}\n")
            with self.assertRaisesRegex(
                VERIFIER.VerificationError, "artifact root mismatch"
            ):
                VERIFIER.load_artifact(changed)

    def test_full_index_diff_is_stable_across_abbreviation_settings(self) -> None:
        def run(repo: Path, *args: str) -> bytes:
            return subprocess.run(
                ["git", "-C", str(repo), *args],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            ).stdout

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            run(repo, "init", "-q")
            run(repo, "config", "user.name", "Vela Test")
            run(repo, "config", "user.email", "vela-test@example.invalid")
            source = repo / "statement.lean"
            source.write_text("def S : Nat := 1\n", encoding="utf-8")
            run(repo, "add", "statement.lean")
            run(repo, "commit", "-q", "-m", "old")
            old = run(repo, "rev-parse", "HEAD").decode().strip()
            source.write_text("def S : Int := 1\n", encoding="utf-8")
            run(repo, "commit", "-qam", "new")
            new = run(repo, "rev-parse", "HEAD").decode().strip()

            run(repo, "config", "core.abbrev", "7")
            short = run(repo, "diff", old, new, "--", "statement.lean")
            short_full = run(
                repo, "diff", "--full-index", old, new, "--", "statement.lean"
            )
            run(repo, "config", "core.abbrev", "16")
            long = run(repo, "diff", old, new, "--", "statement.lean")
            long_full = run(
                repo, "diff", "--full-index", old, new, "--", "statement.lean"
            )
            self.assertNotEqual(short, long)
            self.assertEqual(short_full, long_full)


if __name__ == "__main__":
    unittest.main()
