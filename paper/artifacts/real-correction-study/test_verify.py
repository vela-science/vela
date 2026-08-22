from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("real_correction_verify", ROOT / "verify.py")
assert SPEC is not None and SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class QualificationTests(unittest.TestCase):
    def test_frozen_packet_qualifies_without_confirmatory_authorization(self) -> None:
        result = VERIFY.qualify(ROOT)
        self.assertEqual(result["fixture_count"], 3)
        self.assertFalse(result["confirmatory_freeze_allowed"])
        self.assertFalse(result["protected_final_key_created"])
        self.assertEqual(result["discrimination"]["fact_only_exact"], 1)
        self.assertEqual(result["discrimination"]["authority_aware_exact"], 3)

    def test_source_byte_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            clone = Path(directory) / "packet"
            shutil.copytree(ROOT, clone)
            source = clone / "fixtures/erdos-264/successor/FormalConjectures/ErdosProblems/264.lean"
            source.write_bytes(source.read_bytes() + b"\n")
            with self.assertRaisesRegex(VERIFY.QualificationError, "byte count"):
                VERIFY.qualify(clone)

    def test_authority_signature_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            clone = Path(directory) / "packet"
            shutil.copytree(ROOT, clone)
            record = clone / (
                "fixtures/erdos-264/vela-repository/.vela/authority/records/"
                "var_862a88c79456a6a5.dsse.json"
            )
            envelope = json.loads(record.read_text())
            envelope["signatures"][0]["sig"] = "A" + envelope["signatures"][0]["sig"][1:]
            record.write_text(json.dumps(envelope, sort_keys=True, separators=(",", ":")))
            with self.assertRaisesRegex(VERIFY.QualificationError, "authority signature"):
                VERIFY.qualify(clone)

    def test_duplicate_authority_regime_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            clone = Path(directory) / "packet"
            shutil.copytree(ROOT, clone)
            packet_path = clone / "fixture-qualification.json"
            packet = json.loads(packet_path.read_text())
            packet["fixtures"][1]["authority_scenario"]["regime"] = (
                "authorization_presently_unprovable"
            )
            packet_path.write_text(json.dumps(packet, indent=2) + "\n")
            with self.assertRaisesRegex(VERIFY.QualificationError, "regime coverage"):
                VERIFY.qualify(clone)


if __name__ == "__main__":
    unittest.main()
