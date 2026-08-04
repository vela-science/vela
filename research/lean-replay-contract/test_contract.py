#!/usr/bin/env python3
"""Focused conformance checks for the source-local Lean replay contract."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

import build_root
from lean_replay_contract import ContractError, parse_axioms, verify_package_reference


ROOT = Path(__file__).resolve().parent


class LeanReplayContractTests(unittest.TestCase):
    def test_axiom_vectors(self) -> None:
        fixture = json.loads((ROOT / "fixtures/axiom-reports.v1.json").read_bytes())
        for vector in fixture["vectors"]:
            with self.subTest(vector=vector["name"]):
                if vector["outcome"] == "pass":
                    self.assertEqual(
                        parse_axioms(
                            vector["output"],
                            declaration=fixture["declaration"],
                            permitted=fixture["permitted"],
                            expected=fixture["permitted"],
                        ),
                        fixture["permitted"],
                    )
                else:
                    with self.assertRaises(ContractError):
                        parse_axioms(
                            vector["output"],
                            declaration=fixture["declaration"],
                            permitted=fixture["permitted"],
                        )

    def test_reference_verifies_the_exact_package(self) -> None:
        built = build_root.build(ROOT)
        reference = {
            "schema": "vela.package-consumer-reference.v1",
            "authority_effect": "none",
            "package_root": built["package_root"],
            "descriptor_jcs": built["descriptor_jcs"],
        }
        self.assertEqual(
            verify_package_reference(ROOT, reference), built["package_root"]
        )

    def test_mutated_package_file_fails_closed(self) -> None:
        built = build_root.build(ROOT)
        reference = {
            "schema": "vela.package-consumer-reference.v1",
            "authority_effect": "none",
            "package_root": built["package_root"],
            "descriptor_jcs": built["descriptor_jcs"],
        }
        with tempfile.TemporaryDirectory(prefix="vela-lean-contract-") as temporary:
            copy = Path(temporary) / "package"
            copy.mkdir()
            for row in json.loads(built["descriptor_jcs"])["files"]:
                source = ROOT / row["path"]
                target = copy / row["path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
            (copy / "README.md").write_bytes((copy / "README.md").read_bytes() + b"\n")
            with self.assertRaisesRegex(ContractError, "size differs|root differs"):
                verify_package_reference(copy, reference)

    def test_package_manifest_is_closed_against_its_schema(self) -> None:
        try:
            import jsonschema
        except ImportError as error:  # pragma: no cover - environment contract
            self.fail(f"jsonschema is required by the focused conformance environment: {error}")
        manifest = json.loads((ROOT / "package.json").read_bytes())
        schema = json.loads(
            (ROOT / "schemas/package-candidate.v1.schema.json").read_bytes()
        )
        jsonschema.Draft202012Validator(schema).validate(manifest)


if __name__ == "__main__":
    unittest.main()
