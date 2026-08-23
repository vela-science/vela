from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import evidence_tree


class EvidenceTreeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="diagnostic-evidence-")
        self.root = Path(self.temporary.name)
        self.sources = self.root / "sources"
        shutil.copytree(ROOT / "evidence-sources", self.sources)
        self.packet = sorted((ROOT / "execution-packets").glob("*.json"))[1]

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def materialize(self, name: str = "workspace") -> dict[str, object]:
        return evidence_tree.materialize(
            cell_id="test-cell",
            packet_path=self.packet,
            source_root=self.sources,
            destination=self.root / name,
        )

    def packet_object(self) -> Path:
        packet = json.loads(self.packet.read_bytes())
        root = packet["base_semantic_atoms"][0]["sha256"].removeprefix("sha256:")
        return self.sources / "objects" / root

    def test_two_location_materialization_is_byte_identical(self) -> None:
        first = self.materialize("first")
        second = self.materialize("second")
        self.assertEqual(
            first["workspace_content_root"], second["workspace_content_root"]
        )
        self.assertEqual(
            first["evidence_manifest_root"], second["evidence_manifest_root"]
        )
        self.assertEqual(
            evidence_tree.inventory(self.root / "first"),
            evidence_tree.inventory(self.root / "second"),
        )

    def test_missing_or_substituted_source_object_fails(self) -> None:
        for replacement in (None, b"substituted\n"):
            with self.subTest(replacement=replacement):
                shutil.rmtree(self.sources)
                shutil.copytree(ROOT / "evidence-sources", self.sources)
                target = self.packet_object()
                if replacement is None:
                    target.unlink()
                else:
                    target.write_bytes(replacement)
                with self.assertRaises((FileNotFoundError, ValueError)):
                    self.materialize()

    def test_source_object_symlink_or_external_hardlink_fails(self) -> None:
        target = self.packet_object()
        original = target.read_bytes()
        target.unlink()
        outside = self.root / "outside-object"
        outside.write_bytes(original)
        target.symlink_to(outside)
        with self.assertRaises(ValueError):
            self.materialize("symlink")
        target.unlink()
        target.write_bytes(original)
        alias = self.root / "external-hardlink"
        os.link(target, alias)
        with self.assertRaises(ValueError):
            self.materialize("hardlink")

    def test_cross_case_catalog_binding_fails(self) -> None:
        catalog_path = self.sources / "catalog.json"
        catalog = json.loads(catalog_path.read_bytes())
        assignment = json.loads(self.packet.read_bytes())["assignment_id"]
        current = catalog["assignment_cases"][assignment]
        catalog["assignment_cases"][assignment] = next(
            case for case in catalog["cases"] if case != current
        )
        evidence_tree.write_json(catalog_path, catalog)
        with self.assertRaises(ValueError):
            self.materialize()

    def test_regular_reader_rejects_replacement_between_named_stat_and_open(
        self,
    ) -> None:
        victim = self.root / "victim"
        retained = self.root / "retained"
        victim.write_bytes(b"ORIGINAL")
        original_open = os.open
        replaced = False

        def replace_then_open(
            path: object, flags: int, *args: object, **kwargs: object
        ) -> int:
            nonlocal replaced
            if path == "victim" and not replaced:
                replaced = True
                victim.rename(retained)
                victim.write_bytes(b"FORGED")
            return original_open(path, flags, *args, **kwargs)

        with (
            patch("secure_reader.os.open", side_effect=replace_then_open),
            self.assertRaises(ValueError),
        ):
            evidence_tree.regular_bytes(victim, "replacement victim")

    def test_regular_reader_rejects_external_hardlink(self) -> None:
        victim = self.root / "hardlinked-victim"
        alias = self.root / "hardlinked-alias"
        victim.write_bytes(b"BOUND")
        os.link(victim, alias)
        with self.assertRaises(ValueError):
            evidence_tree.regular_bytes(victim, "hardlinked victim")


if __name__ == "__main__":
    unittest.main()
