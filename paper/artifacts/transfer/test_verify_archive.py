from __future__ import annotations

import io
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
REFERENCE_ROOT = (
    "sha256:b7b330ae6ea4915d5bac218233f0a272"
    "ee961060682be6d22f6a8ea1b78c4ed6"
)


class VerifyArchiveTest(unittest.TestCase):
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
