#!/usr/bin/env python3
"""Offline qualification for the provider-only schema adapter."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
RUNTIME = (
    REPO
    / "paper/artifacts/inherited-correction-benchmark-execution/container-runtime-provider-schema-v2"
)
PRIOR = REPO / "paper/artifacts/inherited-correction-held-out"


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: object) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


class ProviderSchemaRuntimeTests(unittest.TestCase):
    def test_registered_schema_and_participant_prompts_are_byte_exact(self) -> None:
        self.assertEqual(
            (ROOT / "response-schema.json").read_bytes(),
            (PRIOR / "response-schema.json").read_bytes(),
        )
        for current in (ROOT / "conditions").glob("*/*/input/prompt.txt"):
            relative = current.relative_to(ROOT)
            self.assertEqual(current.read_bytes(), (PRIOR / relative).read_bytes())

    def test_provider_schema_has_exactly_one_deleted_keyword(self) -> None:
        registered = json.loads((ROOT / "response-schema.json").read_text())
        provider = json.loads(
            (ROOT / "calibration/input/provider-response-schema.json").read_text()
        )
        self.assertTrue(
            registered["properties"]["evidence_bindings"].pop("uniqueItems")
        )
        self.assertEqual(provider, registered)

    def test_runtime_source_root_and_dual_validation_are_bound(self) -> None:
        entries = []
        for path in sorted(
            item
            for item in RUNTIME.rglob("*")
            if item.is_file() and "node_modules" not in item.parts
        ):
            raw = path.read_bytes()
            entries.append(
                {
                    "path": path.relative_to(RUNTIME).as_posix(),
                    "bytes": len(raw),
                    "sha256": digest(raw),
                }
            )
        runtime = json.loads((ROOT / "runtime-binding.json").read_text())
        self.assertEqual(runtime["runtime_source_root"], canonical_root(entries))
        source = (RUNTIME / "run-once.mjs").read_text()
        self.assertIn('bytes("/input/response-schema.json")', source)
        self.assertIn('bytes("/input/provider-response-schema.json")', source)
        self.assertIn(
            '"--output-schema", "/input/provider-response-schema.json"', source
        )
        self.assertIn("const validate = compileResponseSchema(schema);", source)

    def test_reproducible_oci_build_contract_is_exact_and_held(self) -> None:
        dockerfile = (RUNTIME / "Dockerfile").read_text()
        self.assertIn("ARG SOURCE_DATE_EPOCH=1757289600", dockerfile)
        self.assertEqual(
            dockerfile.count("rm -rf /root/.npm /tmp/node-compile-cache"), 2
        )
        build = (RUNTIME / "build-reproducible-oci.sh").read_text()
        for control in (
            "--platform linux/arm64",
            "--no-cache",
            "--provenance=false",
            "--pull=false",
            "SOURCE_DATE_EPOCH=1757289600",
            "rewrite-timestamp=true",
        ):
            self.assertIn(control, build)
        runtime = json.loads((ROOT / "runtime-binding.json").read_text())
        amendment = json.loads(
            (ROOT / "runtime-reproducibility-amendment.json").read_text()
        )
        evidence = amendment["clean_build_evidence"]
        self.assertEqual(
            amendment["repair"]["dockerfile_bytes"],
            digest((RUNTIME / "Dockerfile").read_bytes()),
        )
        self.assertEqual(
            amendment["repair"]["build_contract_bytes"],
            digest((RUNTIME / "build-reproducible-oci.sh").read_bytes()),
        )
        self.assertEqual(
            amendment["repair"]["verification_contract_bytes"],
            digest((RUNTIME / "verify-reproducible-oci.sh").read_bytes()),
        )
        self.assertEqual(evidence["independent_builder_count"], 2)
        self.assertTrue(evidence["byte_identical_oci_layouts"])
        self.assertEqual(
            [receipt["builder"] for receipt in evidence["builder_receipts"]],
            ["independent-a", "independent-b"],
        )
        self.assertEqual(evidence["image_digest"], runtime["container_image_digest"])
        self.assertEqual(
            evidence["image_config_digest"], runtime["container_image_config_digest"]
        )
        self.assertEqual(
            evidence["oci_tar_bytes"], runtime["container_image_oci_tar_bytes"]
        )
        self.assertEqual(
            {
                (
                    receipt["image_digest"],
                    receipt["image_config_digest"],
                    receipt["oci_tar_bytes"],
                )
                for receipt in evidence["builder_receipts"]
            },
            {
                (
                    runtime["container_image_digest"],
                    runtime["container_image_config_digest"],
                    runtime["container_image_oci_tar_bytes"],
                )
            },
        )
        self.assertEqual(
            amendment["execution_state"],
            {
                "sessions_completed": 0,
                "fixed_denominator": 36,
                "participant_permits_held": 36,
                "participant_permits_consumed": 0,
                "calibration_permits_held": 1,
                "calibration_permits_consumed": 0,
                "provider_calls": 0,
                "protected_key_accesses": 0,
                "scoring_runs": 0,
            },
        )

    def test_frozen_offline_provider_surface_receipt_is_closed(self) -> None:
        base = ROOT / "offline-preflight/provider-schema"
        self.assertEqual((base / "provider-events.jsonl").read_bytes(), b"")
        self.assertEqual((base / "stderr.txt").read_bytes(), b"")
        receipt = json.loads((base / "receipt.json").read_text())
        self.assertFalse(receipt["provider_contact_possible"])
        self.assertEqual(receipt["container_network"], "none")
        self.assertEqual(
            receipt["exact_deleted_json_pointers"],
            ["/properties/evidence_bindings/uniqueItems"],
        )
        self.assertTrue(all(receipt["checks"].values()))


if __name__ == "__main__":
    unittest.main()
