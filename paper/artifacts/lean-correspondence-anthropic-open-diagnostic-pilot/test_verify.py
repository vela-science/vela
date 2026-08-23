from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

import generate  # noqa: E402
import scorer  # noqa: E402
import verify  # noqa: E402

ROOT = SCRIPT_ROOT


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def refresh_manifest(root: Path) -> None:
    entries = []
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.name != "artifact-manifest.json":
            raw = path.read_bytes()
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(root).as_posix(),
                    "sha256": generate.raw_root(raw),
                }
            )
    write_json(
        root / "artifact-manifest.json",
        {
            "artifact_root": generate.canonical_root(entries),
            "authority_effect": "none",
            "entries": entries,
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-manifest.v1",
        },
    )


class PackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="anthropic-diag-test-")
        self.root = Path(self.temporary.name) / "artifact"
        shutil.copytree(ROOT, self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def mutate_json(self, relative: str, mutate: Any) -> None:
        path = self.root / relative
        value = json.loads(path.read_text(encoding="utf-8"))
        mutate(value)
        write_json(path, value)
        refresh_manifest(self.root)

    def assert_rejected(self, relative: str, mutate: Any) -> None:
        self.mutate_json(relative, mutate)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_valid_package(self) -> None:
        self.assertEqual(
            verify.verify_package(self.root),
            json.loads((self.root / "artifact-manifest.json").read_text())[
                "artifact_root"
            ],
        )

    def test_deterministic_regeneration(self) -> None:
        regenerated = Path(self.temporary.name) / "regenerated"
        generate.generate(regenerated)
        expected = sorted(
            path.relative_to(self.root)
            for path in self.root.rglob("*")
            if path.is_file()
        )
        actual = sorted(
            path.relative_to(regenerated)
            for path in regenerated.rglob("*")
            if path.is_file()
        )
        self.assertEqual(actual, expected)
        for relative in expected:
            self.assertEqual(
                (self.root / relative).read_bytes(),
                (regenerated / relative).read_bytes(),
            )

    def test_denominator_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "prelaunch-state.json",
            lambda value: value.__setitem__("fixed_denominator", 5),
        )

    def test_arm_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "assignment-schedule.json",
            lambda value: value["rows"][0].__setitem__(
                "arm", "correspondence-assisted"
            ),
        )

    def test_case_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "assignment-schedule.json",
            lambda value: value["rows"][0].__setitem__("case_id", "invented-case"),
        )

    def test_configuration_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "participant-configuration.json",
            lambda value: value["configuration"].__setitem__("model", "floating-alias"),
        )

    def test_permit_cross_binding_resealed_manifest(self) -> None:
        permits = sorted((self.root / "permits").glob("*.json"))
        first = json.loads(permits[0].read_text())
        second = json.loads(permits[1].read_text())
        first["participant_id"] = second["participant_id"]
        write_json(permits[0], first)
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_runtime_root_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "runtime-binding.json",
            lambda value: value["artifacts"].__setitem__(
                "image_digest", "sha256:" + "0" * 64
            ),
        )

    def test_claim_ceiling_inflation_resealed_manifest(self) -> None:
        self.assert_rejected(
            "preregistration.json",
            lambda value: value["claim_ceiling"].__setitem__("scientific_lift", True),
        )

    def test_stale_positive_prior_result_rejected(self) -> None:
        self.assert_rejected(
            "roadmap-boundary.json",
            lambda value: value["latest_sealed_36_cell_result"].__setitem__(
                "positive_gate", "supported"
            ),
        )

    def test_g3_or_stage_b_overclaim_rejected(self) -> None:
        self.assert_rejected(
            "preregistration.json",
            lambda value: value["claim_ceiling"].__setitem__(
                "living_frontier_g3_inheritance_advantage", True
            ),
        )

    def test_boolean_as_zero_rejected(self) -> None:
        self.assert_rejected(
            "prelaunch-state.json",
            lambda value: value.__setitem__("provider_calls", False),
        )

    def test_unknown_field_rejected(self) -> None:
        self.assert_rejected(
            "runtime-binding.json",
            lambda value: value["anthropic_configuration"].__setitem__(
                "cross_provider", True
            ),
        )

    def test_prompt_byte_drift_resealed_manifest(self) -> None:
        schedule = json.loads((self.root / "assignment-schedule.json").read_text())
        path = self.root / schedule["rows"][0]["prompt_path"]
        path.write_bytes(path.read_bytes() + b"\n")
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_undeclared_file_rejected(self) -> None:
        (self.root / "harmless.txt").write_text("harmless\n")
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)

    def test_symlink_extra_rejected(self) -> None:
        (self.root / "extra-link").symlink_to("README.md")
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)

    def test_manifest_extra_entry_rejected(self) -> None:
        manifest = json.loads((self.root / "artifact-manifest.json").read_text())
        manifest["entries"].append(dict(manifest["entries"][0]))
        write_json(self.root / "artifact-manifest.json", manifest)
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)

    def test_execution_packet_semantic_drift_resealed_manifest(self) -> None:
        schedule = json.loads((self.root / "assignment-schedule.json").read_text())
        path = self.root / schedule["rows"][0]["execution_packet_path"]
        value = json.loads(path.read_text())
        value["authority_effect"] = "invented"
        path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_stale_packet_derivation_receipt_rejected(self) -> None:
        schedule = json.loads((self.root / "assignment-schedule.json").read_text())
        path = self.root / schedule["rows"][0]["packet_derivation_receipt_path"]
        value = json.loads(path.read_text())
        value["parsed_semantic_equality"] = False
        write_json(path, value)
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_offline_receipt_or_qualifier_root_drift_rejected(self) -> None:
        registry = json.loads(
            (self.root / "execution-bundle-registry.json").read_text()
        )
        path = (
            self.root
            / registry["bundles"][0]["bundle_path"]
            / "execution/offline-evidence/offline-pre-request-validation.json"
        )
        value = json.loads(path.read_text())
        value["provider_calls"] = 1
        write_json(path, value)
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)


class PacketDerivationTests(unittest.TestCase):
    def test_recursive_canonical_derivation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="packet-derivation-") as temporary:
            root = Path(temporary)
            source = root / "source.json"
            output = root / "output.json"
            receipt = root / "receipt.json"
            source.write_bytes(b'{"z":[null,true,{"b":2,"a":"x"}],"a":1}\n')
            generate.packet_derivation.derive(source, output, receipt)
            self.assertEqual(
                output.read_bytes(),
                b'{"a":1,"z":[null,true,{"a":"x","b":2}]}\n',
            )

    def test_duplicate_and_noncanonical_numbers_rejected(self) -> None:
        for raw in (b'{"a":{"x":1,"x":2}}', b'{"a":1.0}', b'{"a":-0}'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                generate.packet_derivation.parse(raw)


def capture_package(root: Path) -> None:
    root.mkdir()
    for name in (
        "assignment-schedule.json",
        "case-selection.json",
        "open-adjudication.json",
        "registration.json",
    ):
        shutil.copyfile(ROOT / name, root / name)
    shutil.copytree(ROOT / "permits", root / "permits")


def scoring_document(
    root: Path,
    *,
    all_equal: bool = False,
    conservative: bool = False,
    unknown_safety: bool = False,
    unsafe: bool = False,
) -> dict[str, Any]:
    if (root / "captures").exists():
        shutil.rmtree(root / "captures")
    schedule = json.loads((root / "assignment-schedule.json").read_text())
    registration = json.loads((root / "registration.json").read_text())
    answers = {
        item["case_id"]: item
        for item in json.loads((root / "open-adjudication.json").read_text())["cases"]
    }
    manifests = []
    bindings = []
    first_raw = True
    for assignment in schedule["rows"]:
        cell_id = assignment["cell_id"]
        answer = answers[assignment["case_id"]]
        relation = answer["relation_validation"]
        if assignment["arm"] == "raw-source" and first_raw and not all_equal:
            relation = "cannot_determine"
            first_raw = False
        authority = dict(answer["authority_scientific_inference"])
        if conservative:
            authority = {
                "repository_authority_effect": "unprovable",
                "scientific_status": "unprovable",
            }
        if unknown_safety:
            authority["scientific_status"] = "unknown_code"
        if assignment["arm"] == "correspondence-assisted" and unsafe:
            authority["repository_authority_effect"] = (
                "repository_local_decision_evidenced"
            )
        response = {
            "assignment_id": assignment["source_assignment_id"],
            "authority_scientific_inference": authority,
            "change_classification": answer["change_classification"],
            "impact_closure": [
                {
                    "disposition": item["disposition"],
                    "evidence_ids": item["required_evidence_ids"],
                    "item_id": item["item_id"],
                }
                for item in answer["impact_closure"]
            ],
            "relation_validation": relation,
            "schema": "lean-correspondence.review-response.v1",
            "uncertainty": [],
        }
        directory = root / "captures" / cell_id
        directory.mkdir(parents=True)
        response_path = directory / "response.raw.json"
        write_json(response_path, response)
        seconds = "9.5" if assignment["arm"] == "correspondence-assisted" else "10.5"
        tool_count = 1 if assignment["arm"] == "correspondence-assisted" else 2
        permit = json.loads((root / "permits" / f"{cell_id}.permit.json").read_text())
        permit_root = scorer.maintained_root(permit)
        launch = {
            "attempt": 1,
            "cell_id": cell_id,
            "permit_root": permit_root,
            "provider_calls": 1,
            "run_id": permit["run_id"],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-launch.v3",
            "status": "started",
        }
        terminal = {
            "attempt": 1,
            "cell_id": cell_id,
            "provider_calls": 1,
            "restricted_seconds": seconds,
            "run_id": permit["run_id"],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-terminal.v2",
            "status": "response",
        }
        usage = {
            "cell_id": cell_id,
            "input_tokens": 100,
            "output_tokens": 50,
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-usage.v2",
            "tool_call_count": tool_count,
        }
        terminal_path = directory / "terminal.json"
        usage_path = directory / "usage.json"
        launch_path = directory / "launch.json"
        write_json(launch_path, launch)
        write_json(terminal_path, terminal)
        write_json(usage_path, usage)
        custody = {
            "attempt": 1,
            "cell_id": cell_id,
            "participant_id": assignment["participant_id"],
            "permit_root": permit_root,
            "provider_calls": 1,
            "raw_response_root": generate.raw_root(response_path.read_bytes()),
            "restricted_seconds": seconds,
            "run_id": permit["run_id"],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-custody-receipt.v2",
            "terminal_root": generate.raw_root(terminal_path.read_bytes()),
            "terminal_status": "response",
            "tool_call_count": tool_count,
            "usage_root": generate.raw_root(usage_path.read_bytes()),
        }
        custody_path = directory / "custody.json"
        write_json(custody_path, custody)
        teardown = {
            "cell_id": cell_id,
            "credential_retained": False,
            "process_reaped": True,
            "provider_calls": 1,
            "run_id": permit["run_id"],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-teardown.v3",
            "status": "completed",
            "terminal_status": "response",
        }
        teardown_path = directory / "teardown.json"
        write_json(teardown_path, teardown)
        entries = []
        for role, path in (
            ("custody", custody_path),
            ("launch", launch_path),
            ("raw_response", response_path),
            ("teardown", teardown_path),
            ("terminal", terminal_path),
            ("usage", usage_path),
        ):
            raw = path.read_bytes()
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(root).as_posix(),
                    "role": role,
                    "sha256": generate.raw_root(raw),
                }
            )
        body = {
            "attempt": 1,
            "cell_id": cell_id,
            "entries": entries,
            "participant_id": assignment["participant_id"],
            "permit_root": permit_root,
            "provider_calls": 1,
            "run_id": permit["run_id"],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-capture.v2",
            "terminal_status": "response",
        }
        manifest = dict(body, capture_root=scorer.canonical_root(body))
        manifest_path = directory / "capture.json"
        write_json(manifest_path, manifest)
        relative = manifest_path.relative_to(root).as_posix()
        manifests.append(relative)
        bindings.append({"capture_root": manifest["capture_root"], "path": relative})
    bindings.sort(key=lambda item: item["path"])
    return {
        "capture_manifests": manifests,
        "capture_set_root": scorer.canonical_root(bindings),
        "fixed_denominator": 6,
        "registration_root": registration["registration_root"],
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-score-input.v2",
        "score_attempt": 1,
    }


class ScorerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="anthropic-score-")
        self.root = Path(self.temporary.name) / "package"
        capture_package(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def document(self, **kwargs: Any) -> dict[str, Any]:
        return scoring_document(self.root, **kwargs)

    def test_capture_derived_gate_and_secondary_estimands(self) -> None:
        result = scorer.score_document(self.document(), self.root)
        self.assertTrue(result["diagnostic_gate_pass"])
        self.assertEqual(result["aggregate_restricted_seconds_difference"], "-3")
        self.assertEqual(result["aggregate_tool_call_count_difference"], -3)

    def test_equality_is_no_lift(self) -> None:
        result = scorer.score_document(self.document(all_equal=True), self.root)
        self.assertFalse(result["diagnostic_gate_pass"])
        self.assertFalse(result["informative_raw"])

    def test_assisted_safety_error_fails(self) -> None:
        result = scorer.score_document(self.document(unsafe=True), self.root)
        self.assertFalse(result["assisted_zero_safety_authority_errors"])

    def test_conservative_unprovable_is_safe(self) -> None:
        result = scorer.score_document(self.document(conservative=True), self.root)
        self.assertTrue(result["assisted_zero_safety_authority_errors"])

    def test_unknown_safety_code_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            scorer.score_document(self.document(unknown_safety=True), self.root)

    def test_root_resealed_zero_call_terminal_is_retained(self) -> None:
        document = self.document()
        manifest_path = self.root / document["capture_manifests"][0]
        manifest = json.loads(manifest_path.read_text())
        paths = {
            entry["role"]: self.root / entry["path"] for entry in manifest["entries"]
        }
        paths["raw_response"].write_bytes(b"")
        launch = json.loads(paths["launch"].read_text())
        launch["provider_calls"] = 0
        write_json(paths["launch"], launch)
        terminal = json.loads(paths["terminal"].read_text())
        terminal.update(
            {"provider_calls": 0, "restricted_seconds": "1200", "status": "failure"}
        )
        write_json(paths["terminal"], terminal)
        usage = json.loads(paths["usage"].read_text())
        usage.update({"input_tokens": 0, "output_tokens": 0, "tool_call_count": 0})
        write_json(paths["usage"], usage)
        teardown = json.loads(paths["teardown"].read_text())
        teardown.update({"provider_calls": 0, "terminal_status": "failure"})
        write_json(paths["teardown"], teardown)
        custody = json.loads(paths["custody"].read_text())
        custody.update(
            {
                "provider_calls": 0,
                "raw_response_root": generate.raw_root(b""),
                "restricted_seconds": "1200",
                "terminal_root": generate.raw_root(paths["terminal"].read_bytes()),
                "terminal_status": "failure",
                "tool_call_count": 0,
                "usage_root": generate.raw_root(paths["usage"].read_bytes()),
            }
        )
        write_json(paths["custody"], custody)
        for entry in manifest["entries"]:
            raw = (self.root / entry["path"]).read_bytes()
            entry.update({"bytes": len(raw), "sha256": generate.raw_root(raw)})
        manifest.update({"provider_calls": 0, "terminal_status": "failure"})
        body = {key: value for key, value in manifest.items() if key != "capture_root"}
        manifest["capture_root"] = scorer.canonical_root(body)
        write_json(manifest_path, manifest)
        bindings = []
        for relative in document["capture_manifests"]:
            value = json.loads((self.root / relative).read_text())
            bindings.append({"capture_root": value["capture_root"], "path": relative})
        bindings.sort(key=lambda item: item["path"])
        document["capture_set_root"] = scorer.canonical_root(bindings)
        result = scorer.score_document(document, self.root)
        self.assertEqual(result["capture_set_root"], document["capture_set_root"])

    def test_zero_response_synthetic_old_input_rejected(self) -> None:
        forged = {
            "fixed_denominator": 6,
            "registration_root": "sha256:" + "0" * 64,
            "rows": [],
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-score-input.v1",
            "score_attempt": 1,
        }
        with self.assertRaises(ValueError):
            scorer.score_document(forged, self.root)

    def test_forged_boolean_rejected(self) -> None:
        document = self.document()
        document["relation_validation_correct"] = True
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_missing_or_duplicate_capture_rejected(self) -> None:
        document = self.document()
        document["capture_manifests"][1] = document["capture_manifests"][0]
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_custody_or_raw_root_drift_rejected(self) -> None:
        document = self.document()
        first = self.root / document["capture_manifests"][0]
        manifest = json.loads(first.read_text())
        response = self.root / next(
            item["path"]
            for item in manifest["entries"]
            if item["role"] == "raw_response"
        )
        response.write_bytes(response.read_bytes() + b" ")
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_wrong_run_rejected(self) -> None:
        document = self.document()
        first = self.root / document["capture_manifests"][0]
        manifest = json.loads(first.read_text())
        manifest["run_id"] = "wrong-run"
        body = {key: value for key, value in manifest.items() if key != "capture_root"}
        manifest["capture_root"] = scorer.canonical_root(body)
        write_json(first, manifest)
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_wrong_assignment_response_rejected(self) -> None:
        document = self.document()
        first = self.root / document["capture_manifests"][0]
        manifest = json.loads(first.read_text())
        response = self.root / next(
            item["path"]
            for item in manifest["entries"]
            if item["role"] == "raw_response"
        )
        value = json.loads(response.read_text())
        value["assignment_id"] = "lc-wrong-case-arm"
        write_json(response, value)
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_time_or_tool_omission_rejected(self) -> None:
        document = self.document()
        first = self.root / document["capture_manifests"][0]
        manifest = json.loads(first.read_text())
        usage = self.root / next(
            item["path"] for item in manifest["entries"] if item["role"] == "usage"
        )
        value = json.loads(usage.read_text())
        del value["tool_call_count"]
        write_json(usage, value)
        with self.assertRaises(ValueError):
            scorer.score_document(document, self.root)

    def test_boolean_or_second_score_attempt_rejected(self) -> None:
        for value in (True, 2):
            document = self.document()
            document["score_attempt"] = value
            with self.assertRaises(ValueError):
                scorer.score_document(document, self.root)

    def test_descriptor_reader_rejects_external_hardlink(self) -> None:
        path = self.root / "hardlink-target.json"
        path.write_bytes(b"{}\n")
        alias = Path(self.temporary.name) / "external-hardlink.json"
        os.link(path, alias)
        with self.assertRaises(ValueError):
            scorer.read_bound(self.root, path.name, "hardlink")

    def test_descriptor_reader_rejects_path_replacement_during_validation(self) -> None:
        path = self.root / "replace-target.json"
        path.write_bytes(b'{"original":true}\n')
        moved = self.root / "replace-original.json"

        def replace(raw: bytes) -> None:
            self.assertEqual(raw, b'{"original":true}\n')
            path.rename(moved)
            path.write_bytes(b'{"substituted":true}\n')

        with self.assertRaises(ValueError):
            scorer.read_bound(self.root, path.name, "replacement", replace)


if __name__ == "__main__":
    unittest.main()
