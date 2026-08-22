from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "real_correction_verify", ROOT / "verify.py"
)
assert SPEC is not None and SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


@contextmanager
def packet_clone():
    with tempfile.TemporaryDirectory() as directory:
        clone = Path(directory) / "packet"
        shutil.copytree(ROOT, clone)
        yield clone


def rewrite_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    )


class QualificationTests(unittest.TestCase):
    def test_exact_packet_checks_and_remains_unfrozen(self) -> None:
        result = VERIFY.check_packet(ROOT)
        self.assertEqual(result["fixture_count"], 3)
        self.assertEqual(
            result["erdos_264_evidence"]["authority_sequences_verified"], 5
        )
        self.assertEqual(
            result["erdos_264_evidence"]["repository_transitions_replayed"], 5
        )
        self.assertFalse(result["confirmatory_freeze_allowed"])
        self.assertFalse(result["positive_lift_claim_allowed"])
        self.assertFalse(result["protected_final_key_created"])

    def test_source_byte_drift_fails_closed(self) -> None:
        with packet_clone() as clone:
            source = clone / (
                "fixtures/erdos-264/successor/FormalConjectures/ErdosProblems/264.lean"
            )
            source.write_bytes(source.read_bytes() + b"\n")
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "source manifest drift"
            ):
                VERIFY.check_packet(clone)

    def test_authority_signature_drift_fails_cryptographically(self) -> None:
        with packet_clone() as clone:
            record = clone / (
                "fixtures/erdos-264/vela-repository/.vela/authority/records/"
                "var_862a88c79456a6a5.dsse.json"
            )
            envelope = json.loads(record.read_text())
            envelope["signatures"][0]["sig"] = (
                "A" + envelope["signatures"][0]["sig"][1:]
            )
            rewrite_json(record, envelope)
            binding_path = clone / "fixtures/erdos-264/evidence-binding.json"
            binding = json.loads(binding_path.read_text())
            binding["authority_chain"][3]["file_root"] = VERIFY.sha256(
                record.read_bytes()
            )
            rewrite_json(binding_path, binding)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "authority record 4 signature"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_authority_predecessor_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            binding_path = clone / "fixtures/erdos-264/evidence-binding.json"
            binding = json.loads(binding_path.read_text())
            binding["authority_chain"][4]["previous_payload_root"] = (
                "sha256:" + "0" * 64
            )
            rewrite_json(binding_path, binding)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "authority predecessor 5"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_independent_trust_root_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            binding_path = clone / "fixtures/erdos-264/evidence-binding.json"
            binding = json.loads(binding_path.read_text())
            binding["trust_anchor"]["root"] = "sha256:" + "0" * 64
            rewrite_json(binding_path, binding)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "independent trust root"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_arm_semantic_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "arm-contract.json"
            arms = json.loads(path.read_text())
            arms["arms"][1]["presentation"] = "changed neutral presentation"
            rewrite_json(path, arms)
            with self.assertRaisesRegex(VERIFY.QualificationError, "arm semantics"):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_authority_regime_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "fixture-qualification.json"
            packet = json.loads(path.read_text())
            packet["fixtures"][1]["authority_scenario"]["regime"] = (
                "authorization_presently_unprovable"
            )
            rewrite_json(path, packet)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "authority scenario regime"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_authority_action_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "fixture-qualification.json"
            packet = json.loads(path.read_text())
            packet["fixtures"][2]["authority_scenario"]["safe_next_action"] = (
                "prepare_submission_no_status_change"
            )
            rewrite_json(path, packet)
            with self.assertRaisesRegex(VERIFY.QualificationError, "safe_next_action"):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_consequence_and_safe_action_mutations_fail_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "fixture-qualification.json"
            packet = json.loads(path.read_text())
            packet["fixtures"][2]["bounded_ground_truth"]["consequences"][5][
                "safe_action"
            ] = "reverify_finite_change_invariance"
            rewrite_json(path, packet)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "consequence semantics"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_source_root_mutation_fails_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "fixture-qualification.json"
            packet = json.loads(path.read_text())
            packet["fixtures"][0]["source"]["successor"]["sha256"] = (
                "sha256:" + "0" * 64
            )
            rewrite_json(path, packet)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "successor source root"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_discrimination_source_and_action_mutations_fail_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "discrimination-cases.json"
            cases = json.loads(path.read_text())
            cases["source_atoms_root"] = "sha256:" + "0" * 64
            rewrite_json(path, cases)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "discrimination source root"
            ):
                VERIFY.qualify(clone, verify_manifest=False)
        with packet_clone() as clone:
            path = clone / "discrimination-cases.json"
            cases = json.loads(path.read_text())
            cases["cases"][0]["safe_next_action"] = cases["cases"][1][
                "safe_next_action"
            ]
            rewrite_json(path, cases)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "discrimination cases"
            ):
                VERIFY.qualify(clone, verify_manifest=False)
        with packet_clone() as clone:
            path = clone / "discrimination-cases.json"
            cases = json.loads(path.read_text())
            cases["cases"][0]["authority_regime"] = cases["cases"][1][
                "authority_regime"
            ]
            rewrite_json(path, cases)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "discrimination cases"
            ):
                VERIFY.qualify(clone, verify_manifest=False)
        with packet_clone() as clone:
            path = clone / "discrimination-cases.json"
            cases = json.loads(path.read_text())
            first = cases["cases"][0]["safe_next_action"]
            cases["cases"][0]["safe_next_action"] = cases["cases"][1][
                "safe_next_action"
            ]
            cases["cases"][1]["safe_next_action"] = first
            rewrite_json(path, cases)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "discrimination cases"
            ):
                VERIFY.qualify(clone, verify_manifest=False)

    def test_external_binding_and_code_mutations_change_source_root(self) -> None:
        baseline = VERIFY.source_manifest(ROOT)["source_manifest_root"]
        with packet_clone() as clone:
            path = clone / "fixtures/erdos-264/evidence-binding.json"
            binding = json.loads(path.read_text())
            binding["evidence_repository"]["commit"] = "0" * 40
            rewrite_json(path, binding)
            self.assertNotEqual(
                baseline, VERIFY.source_manifest(clone)["source_manifest_root"]
            )
        with packet_clone() as clone:
            path = clone / "verify.py"
            path.write_bytes(path.read_bytes() + b"\n")
            self.assertNotEqual(
                baseline, VERIFY.source_manifest(clone)["source_manifest_root"]
            )

    def test_discrimination_output_and_generated_result_drift_fail_closed(self) -> None:
        with packet_clone() as clone:
            path = clone / "public-discrimination-result.json"
            output = json.loads(path.read_text())
            output["authority_aware_exact"] = 2
            rewrite_json(path, output)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "public discrimination result drift"
            ):
                VERIFY.check_packet(clone)
        with packet_clone() as clone:
            path = clone / "qualification-result.json"
            result = json.loads(path.read_text())
            result["confirmatory_freeze_allowed"] = True
            rewrite_json(path, result)
            with self.assertRaisesRegex(
                VERIFY.QualificationError, "qualification result"
            ):
                VERIFY.check_packet(clone)


if __name__ == "__main__":
    unittest.main()
