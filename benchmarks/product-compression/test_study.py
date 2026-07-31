from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import study as harness
import materialize


def root(character: str) -> str:
    return f"sha256:{character * 64}"


# fmt: off
def answer() -> dict:
    claim, receipt = f"vcl_{'d' * 64}", root("6")
    return {
        "schema": "vela.product-compression-answer.v3",
        "work": {"frontier_id": "vfr_0123456789abcdef", "repository_root": root("1"), "target_id": "erdos:1056", "target_index_root": root("2"), "packet_sha256": root("3")},
        "campaign": {
            "attempt_id": f"vat_{'4' * 64}", "authorization_root": root("5"), "state": "completed_target_advanced", "completed_target_packet_sha256": root("0"), "consequence_ceiling": "pending_review",
            "budget": {"max_runs": 16, "max_submissions": 4, "max_verifications": 4, "max_artifacts": 16, "max_artifact_bytes": 16_777_216},
            "usage": {"runs": 2, "submissions": 1, "verifications": 1, "artifacts": 2, "artifact_bytes": 4096},
            "runs": [
                {"run_number": 1, "run_id": "run_00000000-0000-0000-0000-000000000001", "receipt_root": receipt, "previous_receipt_root": None, "evidence_root": root("7"), "submission_state": "retained_corroboration", "submission_id": None, "proposal_id": None, "claim_id": None, "verification_id": None},
                {"run_number": 2, "run_id": "run_00000000-0000-0000-0000-000000000002", "receipt_root": root("8"), "previous_receipt_root": receipt, "evidence_root": root("9"), "submission_state": "registered", "submission_id": "vsb_0123456789abcdef", "proposal_id": "vpr_0123456789abcdef", "claim_id": claim, "verification_id": "vvr_0123456789abcdef"},
            ],
            "next_action_code": "start_successor_attempt",
        },
        "review": {"proposal_id": "vpr_0123456789abcdef", "proposal_root": root("a"), "source_submission_id": "vsb_0123456789abcdef", "proposed_claim_id": claim, "verification_id": "vvr_0123456789abcdef", "inbox_projection_root": root("b"), "inbox_entry_root": root("c"), "protocol_gate": "satisfied", "human_decision_required": True, "verification_is_acceptance": False, "standing_delta": {"transition": "add accepted Claim", "scope": {"kind": "proposal_affected_claims", "target_claim_id": claim, "affected_claim_ids": [claim]}, "before": {"repository_root": root("1"), "accepted": []}, "if_accept": {"repository_root": root("4"), "accepted": [{"claim_id": claim, "claim_root": root("e")}]}, "if_reject": {"repository_root": root("5"), "accepted": []}, "counts": {"unchanged_accepted_claims": 0, "global_accepted_claims": {"before": 0, "if_accept": 1, "if_reject": 0}}}, "staleness": "current", "next_if_accept_code": "replay_and_recompute_targets", "next_if_reject_code": "replay_without_standing_change"},
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }


def tool_contract(interface: str, vela_available: bool) -> dict:
    return harness.seal(
        {"tool_contract_root": "", "interface": interface, "vela_available": vela_available},
        "tool_contract_root",
    )


def plan() -> dict:
    return {
        "schema": "vela.product-compression-plan.v3", "plan_root": "", "fixture_root": root("f"), "answer_key_root": "",
        "executor": harness.seal({"executor_root": "", **harness.HARBOR}, "executor_root"),
        "model": harness.seal({
            "id": "test-model", "agent": "codex", "agent_version": "0.145.0",
            "config_root": "",
        }, "config_root"),
        "task_environment": harness.seal({
            "environment_root": "", "base_image": harness.TASK_ENVIRONMENT_IMAGE,
            "vela_version": "vela 0.test",
            "vela_linux_sha256": harness.sha256_root(b"\x7fELFtest-static-vela"),
        }, "environment_root"),
        "arms": {
            "git-files": tool_contract("native-read-only-workspace", False),
            "vela-guided": tool_contract("native-read-only-workspace-plus-vela", True),
        },
        "limits": {"elapsed_ms": 60_000, "per_tool_reported_output_bytes": 10_000, "total_tool_reported_output_bytes": 100_000, "trajectory_bytes": 200_000, "verifier_output_bytes": 20_000, "answer_bytes": 100_000},
        "comparison_rule": harness.COMPARISON_RULE,
        "assignments": [{"pair": "01", "order": ["git-files-01", "vela-guided-01"]}, {"pair": "02", "order": ["vela-guided-02", "git-files-02"]}],
        "publication_policy": {"publish_all_sessions": True, "publish_failures": True, "independence_claim": "first_party_only", "plan_changes_after_output": "forbidden"},
    }


def answer_key() -> dict:
    return harness.seal({"schema": "vela.product-compression-answer-key.v3", "answer_key_root": "", "fixture_root": root("f"), "expected": answer()}, "answer_key_root")


def frozen_material() -> tuple[dict, dict]:
    key, study = answer_key(), plan()
    study["answer_key_root"] = key["answer_key_root"]
    return harness.seal(study, "plan_root"), key


# fmt: on


class HarnessTests(unittest.TestCase):
    def test_harbor_verifier_uses_text_status_and_fails_closed(self) -> None:
        study, key = frozen_material()
        binding = harness.seal({
            "binding_root": "", "frontier": {"git_commit": "a" * 40},
        }, "binding_root")
        self.assertEqual(
            harness.harbor_verifier_outcome(
                answer(), key, binding, "a" * 40, "",
            )["eligible"],
            True,
        )
        self.assertIn(
            "frontier_worktree_drift",
            harness.harbor_verifier_outcome(
                answer(), key, binding, "a" * 40, b"",  # type: ignore[arg-type]
            )["eligibility_failure_codes"],
        )
        missing = harness.harbor_verifier_outcome(
            None, key, binding, "a" * 40, "",
        )
        self.assertTrue(missing["eligible"])
        self.assertFalse(missing["exact"])

    def test_prepare_harbor_is_native_closed_and_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root_path = Path(temporary)
            frontier = root_path / "frontier"
            frontier.mkdir()
            subprocess.run(["git", "init", "-q", "-b", "main"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.name", "Vela Test"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.email", "test@vela.invalid"], cwd=frontier, check=True)
            (frontier / "README.md").write_text("rooted frontier\n")
            subprocess.run(["git", "add", "README.md"], cwd=frontier, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "fixture"], cwd=frontier, check=True)
            commit = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=frontier, check=True,
                capture_output=True, text=True,
            ).stdout.strip()
            tree = subprocess.run(
                ["git", "rev-parse", "HEAD^{tree}"], cwd=frontier, check=True,
                capture_output=True, text=True,
            ).stdout.strip()
            materials = root_path / "materials"
            participant = materials / "participant" / "campaign"
            participant.mkdir(parents=True)
            retained = participant / "attempt.json"
            retained.write_text('{"retained":true}\n')
            fixture = harness.seal({
                "schema": "vela.product-compression-fixture.v2",
                "fixture_root": "",
                "frontier": {
                    "frontier_id": "vfr_0123456789abcdef",
                    "git_commit": commit,
                    "git_tree": tree,
                    "remote": "https://example.invalid/frontier.git",
                    "repository_root": root("1"),
                    "target_index_root": root("2"),
                },
                "participant_files": [{
                    "path": "campaign/attempt.json",
                    "sha256": harness.sha256_root(retained.read_bytes()),
                    "size": len(retained.read_bytes()),
                }],
                "sources": {},
                "vela": {"binary_sha256": root("3"), "version": "vela 0.test"},
            }, "fixture_root")
            key = answer_key()
            key["fixture_root"] = fixture["fixture_root"]
            harness.seal(key, "answer_key_root")
            harness.write_json(materials / "fixture.json", fixture)
            harness.write_json(materials / "answer-key.json", key)
            vela = root_path / "vela"
            vela.write_bytes(b"\x7fELFtest-static-vela")
            os.chmod(vela, 0o555)
            study = plan()
            study["fixture_root"] = fixture["fixture_root"]
            study["answer_key_root"] = key["answer_key_root"]
            study["task_environment"]["vela_linux_sha256"] = harness.sha256_root(vela.read_bytes())
            harness.seal(study["task_environment"], "environment_root")
            harness.seal(study, "plan_root")
            plan_path = root_path / "plan.json"
            harness.write_json(plan_path, study)

            roots = []
            for name in ("one", "two"):
                output = root_path / name
                result = harness.prepare_harbor(
                    plan_path, materials, frontier, vela,
                    "vela-product-compression-v3-test", output,
                )
                roots.append(result["task_set_root"])
                job = harness.read_json(output / "harbor-job.json")
                self.assertEqual(len(job["tasks"]), 4)
                self.assertNotIn("env", job["agents"][0])
                self.assertFalse((output / "tasks/01-git-files-01/environment/vela").exists())
                self.assertTrue((output / "tasks/02-vela-guided-01/environment/vela").exists())
                generated_verifier = (output / "tasks/01-git-files-01/tests/verify.py").read_text()
                self.assertIn("capture_output=True, text=True", generated_verifier)
                self.assertIn('"eligible": 1 if outcome["eligible"] else 0', generated_verifier)
                task_config = (output / "tasks/01-git-files-01/task.toml").read_text()
                self.assertIn('environment_mode = "shared"', task_config)
            self.assertEqual(roots[0], roots[1])

            (frontier / "dirty.txt").write_text("dirty\n")
            with self.assertRaisesRegex(harness.ContractError, "must be clean"):
                harness.prepare_harbor(
                    plan_path, materials, frontier, vela,
                    "vela-product-compression-v3-dirty", root_path / "dirty-output",
                )

    def test_freeze_plan_binds_agent_environment_and_materials(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root_path = Path(temporary)
            materials = root_path / "materials"
            materials.mkdir()
            key = answer_key()
            fixture = harness.seal({
                "schema": "fixture.test", "fixture_root": "", "value": 1,
            }, "fixture_root")
            key["fixture_root"] = fixture["fixture_root"]
            harness.seal(key, "answer_key_root")
            harness.write_json(materials / "fixture.json", fixture)
            harness.write_json(materials / "answer-key.json", key)
            vela = root_path / "vela"
            vela.write_bytes(b"\x7fELFtest-static-vela")
            os.chmod(vela, 0o500)
            frozen = harness.freeze_plan(
                materials, "test-model", "0.145.0", vela, "vela 0.test",
            )
            self.assertEqual(frozen["model"]["agent_version"], "0.145.0")
            self.assertEqual(
                frozen["task_environment"]["vela_linux_sha256"],
                harness.sha256_root(vela.read_bytes()),
            )
            harness.validate_plan(frozen)

    def test_plan_pins_harbor_and_closed_tool_contracts(self) -> None:
        study, _ = frozen_material()
        harness.validate_plan(study)
        self.assertEqual({key: study["executor"][key] for key in harness.HARBOR}, harness.HARBOR)
        self.assertEqual(study["arms"]["git-files"]["interface"], "native-read-only-workspace")
        self.assertTrue(study["arms"]["vela-guided"]["vela_available"])

    def test_plan_rejects_executor_drift(self) -> None:
        study, _ = frozen_material()
        study["executor"]["version"] = "0.20.1"
        harness.seal(study["executor"], "executor_root")
        harness.seal(study, "plan_root")
        with self.assertRaisesRegex(harness.ContractError, "unsupported or unpinned"):
            harness.validate_plan(study)

    def test_secret_materials_fail_closed_under_system_temporary_roots(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            with self.assertRaisesRegex(harness.ContractError, "system temporary root"):
                materialize.reject_system_temporary_output(temporary / "materials")

    def test_answer_schema_is_recursively_closed(self) -> None:
        stack = [json.loads((Path(__file__).parent / "answer.schema.json").read_text())]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                if node.get("type") == "object":
                    self.assertIs(node.get("additionalProperties"), False)
                stack.extend(node.values())
            elif isinstance(node, list):
                stack.extend(node)

    def test_submission_proposal_linkage_fails_closed(self) -> None:
        value = answer()
        value["review"]["source_submission_id"] = "vsb_ffffffffffffffff"
        with self.assertRaisesRegex(harness.ContractError, "not linked"):
            harness.validate_answer(value)

    def test_registration_order_is_independent_of_run_order(self) -> None:
        value = answer()
        fields = ("submission_state", "submission_id", "proposal_id", "claim_id", "verification_id")
        for field in fields:
            value["campaign"]["runs"][0][field], value["campaign"]["runs"][1][field] = value["campaign"]["runs"][1][field], value["campaign"]["runs"][0][field]
        harness.validate_answer(value)

    def test_receipt_and_standing_cross_fields_fail_closed(self) -> None:
        value = answer()
        value["campaign"]["runs"][1]["previous_receipt_root"] = root("0")
        with self.assertRaisesRegex(harness.ContractError, "receipt chain"):
            harness.validate_answer(value)
        value = answer()
        value["review"]["standing_delta"]["if_reject"] = value["review"]["standing_delta"]["if_accept"]
        with self.assertRaisesRegex(harness.ContractError, "rejection must preserve"):
            harness.validate_answer(value)

    def test_plan_rejects_unbalanced_assignment_and_wrong_execution_interface(self) -> None:
        study, _ = frozen_material()
        study["assignments"][1]["order"].reverse()
        harness.seal(study, "plan_root")
        with self.assertRaisesRegex(harness.ContractError, "AB/BA"):
            harness.validate_plan(study)
        study, _ = frozen_material()
        study["arms"]["vela-guided"]["interface"] = "custom-shell-dialect"
        harness.seal(study, "plan_root")
        with self.assertRaisesRegex(harness.ContractError, "wrong execution interface"):
            harness.validate_plan(study)

    def test_answer_key_tampering_breaks_its_root(self) -> None:
        _, key = frozen_material()
        key["expected"]["work"]["target_id"] = "erdos:9999"
        with self.assertRaisesRegex(harness.ContractError, "answer_key_root"):
            harness.validate_answer_key(key)

    def test_plan_tampering_breaks_its_root(self) -> None:
        study, _ = frozen_material()
        study["limits"]["elapsed_ms"] += 1
        with self.assertRaisesRegex(harness.ContractError, "plan_root"):
            harness.validate_plan(study)

    def test_materializer_binds_completed_attempt_to_successor_and_review(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root_dir = Path(directory)
            frontier, private = root_dir / "frontier", root_dir / "private"
            frontier.mkdir()
            private.mkdir()
            vela = root_dir / "vela"
            vela.write_bytes(b"vela-binary")
            packet = frontier / "targets" / "erdos-1056.json"
            packet.parent.mkdir()
            packet.write_bytes(b"successor-packet\n")
            packet_root = materialize.digest(packet)

            attempt_id, proposal_id = f"vat_{'4' * 64}", "vpr_0123456789abcdef"
            submission_id, claim_id = "vsb_0123456789abcdef", f"vcl_{'d' * 64}"
            run_files, evidence_files, receipts = [], [], []
            previous = None
            for number in (1, 2):
                run_path, evidence_path = private / f"run-{number}.json", private / f"evidence-{number}.json"
                run_path.write_bytes(f"run-{number}\n".encode())
                evidence_path.write_bytes(f"evidence-{number}\n".encode())
                run_id = f"run_00000000-0000-0000-0000-00000000000{number}"
                receipt_root = root(str(number + 4))
                evidence_root = root(str(number + 6))
                receipts.append({
                    "run_number": number, "receipt_root": receipt_root,
                    "previous_receipt_root": previous,
                    "result": {
                        "run": {"id": run_id, "path": str(run_path), "size": run_path.stat().st_size, "sha256": materialize.digest(run_path)},
                        "evidence_manifest": {"path": str(evidence_path), "size": evidence_path.stat().st_size, "sha256": materialize.digest(evidence_path), "root": evidence_root},
                    },
                })
                run_files.append(run_path)
                evidence_files.append(evidence_path)
                previous = receipt_root

            submission = {"submission_id": submission_id, "provenance": {"source_attempt": attempt_id, "source_run": receipts[1]["result"]["run"]["id"]}}
            submission_bytes = harness.canonical_bytes(submission)
            submission_root = harness.sha256_root(submission_bytes)
            submission_path = frontier / materialize.relative_content_path(submission_root, "submissions")
            submission_path.parent.mkdir(parents=True)
            submission_path.write_bytes(submission_bytes)
            attempt = {
                "schema": "vela.attempt.v8", "attempt_id": attempt_id,
                "authorization_root": root("3"), "frontier_id": "vfr_0123456789abcdef",
                "target": "erdos:1056",
                "starting_target_task_binding": {"packet": {"sha256": root("0")}},
                "consequence_ceiling": "pending_review",
                "budget": {"max_runs": 16, "max_submissions": 4, "max_verifications": 4, "max_artifacts": 16, "max_artifact_bytes": 16_777_216},
                "usage": {"runs": 2, "submissions": 1, "verifications": 1, "artifacts": 2, "artifact_bytes": 16},
                "agent_run_receipts": receipts,
                "agent_run_submission_links": [{"run_id": receipts[1]["result"]["run"]["id"], "submission_id": submission_id}],
            }
            attempt_path = private / "attempt.json"
            attempt_path.write_bytes(harness.canonical_bytes(attempt))
            next_work = {
                "frontier_id": "vfr_0123456789abcdef", "repository_root": root("1"), "target_index_root": root("2"),
                "targets": [{"target_id": "erdos:1056", "packet": {"path": "targets/erdos-1056.json", "sha256": packet_root}}],
            }
            entry = {
                "proposal_id": proposal_id, "claim_id": claim_id,
                "inputs": {"repository_root": root("1"), "proposal_root": root("a"), "submission_root": submission_root},
                "readiness": {"protocol_gate": "satisfied", "human_decision_required": True},
                "staleness": {"state": "current"},
                "verification_records": [{"verification_record_id": "vvr_0123456789abcdef", "verification_record_root": root("f")}],
                "standing_delta": {"transition": "add accepted Claim", "scope": {"kind": "proposal_affected_claims", "target_claim_id": claim_id, "affected_claim_ids": [claim_id]}, "before": {"repository_root": root("1"), "accepted": []}, "if_accept": {"repository_root": root("4"), "accepted": [{"claim_id": claim_id, "claim_root": root("e")}]}, "if_reject": {"repository_root": root("5"), "accepted": []}, "counts": {"unchanged_accepted_claims": 0, "global_accepted_claims": {"before": 0, "if_accept": 1, "if_reject": 0}}},
                "entry_root": root("c"),
            }
            status = {"campaign": {"active_attempt_count": 0}}
            inbox = {"projection_root": root("b"), "entries": [entry]}

            def fake_command(argv: tuple[str, ...], *, cwd: Path) -> str:
                if argv[:3] == ("git", "status", "--porcelain"):
                    return ""
                if argv[:3] == ("git", "rev-parse", "HEAD"):
                    return "1" * 40
                if argv[:3] == ("git", "rev-parse", "HEAD^{tree}"):
                    return "2" * 40
                if argv[:3] == ("git", "remote", "get-url"):
                    return "https://example.invalid/frontier.git"
                if argv == (str(vela.resolve()), "--version"):
                    return "vela 0.test"
                raise AssertionError(argv)

            def fake_json(argv: tuple[str, ...], *, cwd: Path) -> dict:
                return status if argv[1] == "status" else next_work if argv[1] == "next" else inbox

            with mock.patch.object(materialize, "command", side_effect=fake_command), mock.patch.object(materialize, "json_command", side_effect=fake_json):
                fixture, key, participant_files = materialize.materialize(frontier, vela, attempt_path, proposal_id)
            harness.validate_answer_key(key)
            self.assertEqual(fixture["frontier"]["repository_root"], root("1"))
            self.assertEqual(key["expected"]["campaign"]["runs"][0]["submission_state"], "retained_corroboration")
            self.assertEqual(key["expected"]["campaign"]["runs"][1]["submission_state"], "registered")
            self.assertEqual(key["expected"]["work"]["packet_sha256"], packet_root)
            sanitized = json.loads(participant_files["campaign/attempt.json"])
            self.assertEqual(sanitized["agent_run_receipts"][0]["result"]["run"]["path"], "campaign/run-01/run.json")
            self.assertFalse(any(str(private) in data.decode() for data in participant_files.values()))


if __name__ == "__main__":
    unittest.main()
