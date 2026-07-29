from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify_source_transition.py")
SPEC = importlib.util.spec_from_file_location("verify_source_transition", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


def run(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


class CanonicalDiffTests(unittest.TestCase):
    def test_blob_abbreviation_configuration_cannot_change_diff_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="vela-diff-root-") as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            run(source, "init", "-q")
            run(source, "config", "user.name", "Vela Test")
            run(source, "config", "user.email", "vela-test@example.invalid")
            tracked = source / "statement.lean"
            tracked.write_text("theorem example : True := by\n  trivial\n", encoding="utf-8")
            run(source, "add", "statement.lean")
            run(source, "commit", "-q", "-m", "predecessor")
            predecessor = run(source, "rev-parse", "HEAD").decode().strip()
            tracked.write_text(
                "theorem example : True := by\n  exact True.intro\n",
                encoding="utf-8",
            )
            run(source, "commit", "-qam", "successor")
            successor = run(source, "rev-parse", "HEAD").decode().strip()

            short = root / "short"
            long = root / "long"
            run(root, "clone", "-q", str(source), str(short))
            run(root, "clone", "-q", str(source), str(long))
            run(short, "config", "core.abbrev", "7")
            run(long, "config", "core.abbrev", "16")

            short_default = run(
                short,
                "diff",
                predecessor,
                successor,
                "--",
                "statement.lean",
            )
            long_default = run(
                long,
                "diff",
                predecessor,
                successor,
                "--",
                "statement.lean",
            )
            self.assertNotEqual(short_default, long_default)

            short_canonical = VERIFIER.canonical_diff(
                short,
                predecessor,
                successor,
                "statement.lean",
            )
            long_canonical = VERIFIER.canonical_diff(
                long,
                predecessor,
                successor,
                "statement.lean",
            )
            self.assertEqual(short_canonical, long_canonical)
            index_line = next(
                line for line in short_canonical.splitlines() if line.startswith(b"index ")
            )
            old_blob, new_blob = index_line.split()[1].split(b"..")
            self.assertEqual(len(old_blob), 40)
            self.assertEqual(len(new_blob), 40)


if __name__ == "__main__":
    unittest.main()
