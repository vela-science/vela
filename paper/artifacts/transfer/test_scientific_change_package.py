from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PACKAGE = HERE / "erdos-424"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


reader = load_module(
    "read_scientific_change_package",
    HERE / "read_scientific_change_package.py",
)
builder = load_module(
    "build_scientific_change_package",
    HERE / "build_scientific_change_package.py",
)


class ScientificChangePackageTest(unittest.TestCase):
    def test_checked_in_package_has_exact_native_and_ro_crate_parity(self) -> None:
        result = reader.assess(
            PACKAGE,
            HERE / "scientific-change-package-plan.v1.json",
            HERE / "scientific-change-package-plan-amendment-001.v1.json",
        )
        self.assertTrue(result["ok"])
        self.assertEqual(result["object_count"], 11)
        self.assertEqual(
            result["native_manifest_root"],
            "sha256:b7b330ae6ea4915d5bac218233f0a272"
            "ee961060682be6d22f6a8ea1b78c4ed6",
        )
        self.assertEqual(result["source_standing"], "accepted")
        self.assertEqual(result["local_standing_effect"], "none")

    def test_generated_core_is_reproducible_from_native_files(self) -> None:
        plan = builder.load_frozen(
            HERE / "scientific-change-package-plan.v1.json",
            builder.PLAN_ROOT,
            builder.PLAN_BYTES_SHA256,
        )
        native = builder.load_source(PACKAGE, plan)
        with (
            tempfile.TemporaryDirectory() as raw_a,
            tempfile.TemporaryDirectory() as raw_b,
        ):
            first = Path(raw_a) / "package"
            second = Path(raw_b) / "package"
            builder.copy_native(PACKAGE, first, native)
            builder.copy_native(PACKAGE, second, native)
            first_outputs = builder.write_core(first, plan, native)
            second_outputs = builder.write_core(second, plan, native)
            self.assertEqual(first_outputs, second_outputs)
            for name, encoded in first_outputs.items():
                self.assertEqual(encoded, (PACKAGE / name).read_bytes())

    def test_all_registered_mutations_fail_closed(self) -> None:
        observed = builder.mutation_results(PACKAGE)
        self.assertEqual(
            [(item["id"], item["diagnostic"]) for item in observed],
            [
                ("dropped_decision", "native_object_unavailable"),
                ("root_drift", "native_object_root_drift"),
                ("authority_escalation", "authority_escalation"),
                ("missing_loss_report", "loss_report_missing"),
            ],
        )
        self.assertTrue(all(item["standing_effect"] == "none" for item in observed))

    def test_sha256_manifest_is_complete_and_exact(self) -> None:
        native = builder.load_source(
            PACKAGE,
            builder.load_frozen(
                HERE / "scientific-change-package-plan.v1.json",
                builder.PLAN_ROOT,
                builder.PLAN_BYTES_SHA256,
            ),
        )
        expected = builder.sha256_manifest(PACKAGE, native)
        observed = (PACKAGE / "SHA256SUMS").read_bytes()
        self.assertEqual(observed, expected)
        self.assertEqual(
            f"sha256:{hashlib.sha256(observed).hexdigest()}",
            "sha256:e8eabd53d1bf9433956659720e92189"
            "d724750d6a4a3435fb2c4069d42989628",
        )

    def test_result_records_external_profile_gap_without_substitution(self) -> None:
        result = json.loads((PACKAGE / "result.v1.json").read_bytes())
        readers = json.loads((PACKAGE / "reader-result.v1.json").read_bytes())
        self.assertEqual(
            result["outcome"], "baseline_complete_with_external_validator_gap"
        )
        self.assertEqual(result["readers"]["native"], "pass")
        self.assertEqual(result["readers"]["ro_crate_clean_room"], "pass")
        self.assertEqual(result["readers"]["external_ro_crate"], "unsupported_profile")
        self.assertFalse(
            readers["external_ro_crate"]["profile_substitution_performed"]
        )
        self.assertEqual(result["authority"]["local_standing_effect"], "none")

    def test_publisher_rejects_unrelated_destination_and_stale_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            unrelated = Path(raw) / "unrelated"
            unrelated.mkdir()
            with self.assertRaisesRegex(
                builder.BuildError,
                "publish_destination_must_be_exact_source_package",
            ):
                builder.build(
                    PACKAGE,
                    unrelated,
                    Path("/not/reached"),
                )

            stale = unrelated / "result.v1.json"
            stale.write_bytes(b"stale")
            with self.assertRaisesRegex(
                builder.BuildError,
                "stale_generated_output:result.v1.json",
            ):
                builder.publish_outputs(
                    unrelated,
                    {"result.v1.json": b"expected"},
                )


if __name__ == "__main__":
    unittest.main()
