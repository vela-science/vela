from __future__ import annotations

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from conformance.verify import _check_manifest


class FixtureManifestClosureTests(unittest.TestCase):
    fixture_bytes = b'{"fixture_version": 1}\n'

    def _entry(self, name: str) -> dict[str, object]:
        return {
            "bytes": len(self.fixture_bytes),
            "path": name,
            "sha256": "sha256:" + hashlib.sha256(self.fixture_bytes).hexdigest(),
        }

    def _write_manifest(
        self, fixtures_dir: Path, entries: list[dict[str, object]]
    ) -> None:
        manifest = {
            "schema": "vela.conformance-fixtures-manifest.v1",
            "fixtures": entries,
        }
        (fixtures_dir / "fixtures.manifest.json").write_text(json.dumps(manifest))

    def test_unmanifested_cascade_fixture_is_invocation_error(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixtures_dir = Path(temp)
            listed_name = "cascade-fixture-00.json"
            extra_name = "cascade-fixture-01.json"
            (fixtures_dir / listed_name).write_bytes(self.fixture_bytes)
            (fixtures_dir / extra_name).write_bytes(self.fixture_bytes)
            self._write_manifest(fixtures_dir, [self._entry(listed_name)])

            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                result = _check_manifest(fixtures_dir)

            self.assertEqual(result, 2)
            self.assertIn(
                f"{extra_name}: present on disk but absent from manifest",
                stderr.getvalue(),
            )

    def test_duplicate_manifest_path_is_invocation_error(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixtures_dir = Path(temp)
            name = "cascade-fixture-00.json"
            (fixtures_dir / name).write_bytes(self.fixture_bytes)
            entry = self._entry(name)
            self._write_manifest(fixtures_dir, [entry, entry.copy()])

            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                result = _check_manifest(fixtures_dir)

            self.assertEqual(result, 2)
            self.assertIn(
                f"duplicate fixture paths: {name}",
                stderr.getvalue(),
            )


if __name__ == "__main__":
    unittest.main()
