from __future__ import annotations

import io
import importlib.util
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PACKAGE = Path(__file__).resolve().parent / "erdos-424"
PACK = Path(__file__).resolve().parent / "pack_reference.py"
VERIFY = Path(__file__).resolve().parent / "verify_archive.py"
MATERIALIZE = Path(__file__).resolve().parent / "materialize_real_reference.py"
REFERENCE_ROOT = (
    "sha256:b7b330ae6ea4915d5bac218233f0a272"
    "ee961060682be6d22f6a8ea1b78c4ed6"
)


def load_materializer():
    spec = importlib.util.spec_from_file_location("materialize_real_reference", MATERIALIZE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


materializer = load_materializer()


class VerifyArchiveTest(unittest.TestCase):
    def test_source_snapshot_stays_pinned_after_later_commits(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(
                ["git", "config", "user.email", "test@vela.space"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Vela test"],
                cwd=repository,
                check=True,
            )
            tracked = repository / "state"
            tracked.write_text("retained\n")
            subprocess.run(["git", "add", "state"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "retained"], cwd=repository, check=True)
            retained = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            retained_tree = subprocess.run(
                ["git", "rev-parse", "HEAD^{tree}"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            tracked.write_text("later compaction\n")
            subprocess.run(["git", "commit", "-qam", "later"], cwd=repository, check=True)

            self.assertEqual(
                materializer.resolve_source_snapshot(repository, retained),
                (retained, retained_tree),
            )

    def test_real_archive_passes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            archive = Path(raw) / "reference.tar.gz"
            subprocess.run(
                [
                    sys.executable,
                    str(PACK),
                    "--package-root",
                    str(PACKAGE),
                    "--output",
                    str(archive),
                ],
                cwd=ROOT,
                check=True,
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(VERIFY),
                    "--archive",
                    str(archive),
                    "--expected-root",
                    REFERENCE_ROOT,
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("authority_signature=verified", result.stdout)

    def test_path_escape_fails(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            archive = Path(raw) / "escape.tar.gz"
            with tarfile.open(archive, mode="w:gz") as handle:
                data = b"{}"
                member = tarfile.TarInfo("../reference.v1.json")
                member.size = len(data)
                handle.addfile(member, io.BytesIO(data))
            result = subprocess.run(
                [
                    sys.executable,
                    str(VERIFY),
                    "--archive",
                    str(archive),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unsafe archive path", result.stderr)


if __name__ == "__main__":
    unittest.main()
