from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import harness
import materialize


def root(character: str) -> str:
    return f"sha256:{character * 64}"


# fmt: off
def answer() -> dict:
    claim, receipt = f"vcl_{'d' * 64}", root("6")
    return {
        "schema": "vela.product-compression-answer.v2",
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
        "review": {"proposal_id": "vpr_0123456789abcdef", "proposal_root": root("a"), "source_submission_id": "vsb_0123456789abcdef", "target_claim_id": claim, "verification_id": "vvr_0123456789abcdef", "inbox_projection_root": root("b"), "inbox_entry_root": root("c"), "protocol_gate": "satisfied", "human_decision_required": True, "verification_is_acceptance": False, "standing_transition": "add accepted Claim", "accepted_before": [], "accepted_if_accept": [{"claim_id": claim, "claim_root": root("e")}], "accepted_if_reject": [], "staleness": "current", "next_if_accept_code": "replay_and_recompute_targets", "next_if_reject_code": "replay_without_standing_change"},
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }


def tool_contract(*prefixes: list[str]) -> dict:
    return harness.seal({"tool_contract_root": "", "argv_prefixes": list(prefixes)}, "tool_contract_root")


def plan() -> dict:
    return {
        "schema": "vela.product-compression-plan.v2", "plan_root": "", "fixture_root": root("f"), "answer_key_root": "", "supervisor_runner_root": root("0"),
        "model": {"id": "test-model", "config_root": root("1")},
        "arms": {"git-files": tool_contract(["git", "status"]), "vela-guided": tool_contract(["vela", "status"])},
        "budgets": {"elapsed_ms": 60_000, "commands": 20, "effective_tokens": 10_000, "per_command_output_bytes": 10_000, "total_tool_output_bytes": 100_000, "answer_bytes": 100_000},
        "assignments": [{"pair": "01", "order": ["git-files-01", "vela-guided-01"]}, {"pair": "02", "order": ["vela-guided-02", "git-files-02"]}],
        "publication_policy": {"publish_all_sessions": True, "publish_failures": True, "independence_claim": "first_party_only", "plan_changes_after_output": "forbidden"},
    }


def answer_key() -> dict:
    return harness.seal({"schema": "vela.product-compression-answer-key.v2", "answer_key_root": "", "fixture_root": root("f"), "expected": answer()}, "answer_key_root")


def frozen_material() -> tuple[dict, dict]:
    key, study = answer_key(), plan()
    study["answer_key_root"] = key["answer_key_root"]
    return harness.seal(study, "plan_root"), key


def session(session_id: str = "git-files-01") -> dict:
    study, key = frozen_material()
    arm, retained = session_id.rsplit("-", 1)[0], answer()
    value = {
        "schema": "vela.product-compression-session.v2", "session_root": "", "plan_root": study["plan_root"], "fixture_root": study["fixture_root"], "answer_key_root": key["answer_key_root"], "session_id": session_id, "arm": arm,
        "model": "test-model", "model_config_root": root("1"), "tool_contract_root": study["arms"][arm]["tool_contract_root"], "supervisor_runner_root": study["supervisor_runner_root"],
        "started_at": "2026-07-31T10:00:00Z", "completed_at": "2026-07-31T10:00:01Z", "elapsed_ms": 1000, "termination": "completed", "exit_code": 0,
        "usage": {"input_tokens": 100, "cached_input_tokens": 40, "output_tokens": 50},
        "commands": [{"index": 1, "argv": ["git", "status"] if arm == "git-files" else ["vela", "status", "--json"], "elapsed_ms": 100, "exit_code": 0, "stdout_bytes": 3, "stdout_root": root("4"), "stderr_bytes": 0, "stderr_root": root("5")}],
        "semantic_interventions": [], "state": {name: {"before": root(char), "after": root(char)} for name, char in (("git_status", "6"), ("repository", "7"), ("standing", "8"))},
        "violations": [], "answer_root": harness.sha256_root(harness.canonical_bytes(retained)), "answer": retained,
    }
    return harness.seal(value, "session_root")


def score_for(session_id: str, elapsed_ms: int, commands: int, tokens: int) -> dict:
    study, _ = frozen_material()
    value = {"schema": "vela.product-compression-score.v2", "score_root": "", "plan_root": study["plan_root"], "session_root": root(session_id[-1]), "session_id": session_id, "arm": session_id.rsplit("-", 1)[0], "passed": True, "failure_codes": [], "metrics": {"elapsed_ms": elapsed_ms, "command_count": commands, "effective_tokens": tokens, "semantic_intervention_count": 0, "tool_output_bytes": 3}}
    return harness.seal(value, "score_root")
# fmt: on


class HarnessTests(unittest.TestCase):
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

    def test_valid_material_scores_and_derives_tokens(self) -> None:
        study, key = frozen_material()
        result = harness.score_session(study, key, session())
        self.assertTrue(result["passed"])
        self.assertEqual(result["metrics"]["effective_tokens"], 110)

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
        value["review"]["accepted_if_reject"] = value["review"]["accepted_if_accept"]
        with self.assertRaisesRegex(harness.ContractError, "rejection must preserve"):
            harness.validate_answer(value)

    def test_plan_rejects_unbalanced_assignment_and_fake_tool_binding(self) -> None:
        study, _ = frozen_material()
        study["assignments"][1]["order"].reverse()
        harness.seal(study, "plan_root")
        with self.assertRaisesRegex(harness.ContractError, "AB/BA"):
            harness.validate_plan(study)
        study, _ = frozen_material()
        study["arms"]["vela-guided"]["argv_prefixes"].append(["bash"])
        harness.seal(study, "plan_root")
        with self.assertRaisesRegex(harness.ContractError, "non-read-only prefix"):
            harness.validate_plan(study)

    def test_semantic_intervention_and_tool_escape_fail_score(self) -> None:
        study, key = frozen_material()
        value = session()
        value["semantic_interventions"] = ["told participant which Run to choose"]
        value["commands"][0]["argv"] = ["bash", "-lc", "git status"]
        harness.seal(value, "session_root")
        result = harness.score_session(study, key, value)
        self.assertIn("semantic_intervention", result["failure_codes"])
        self.assertIn("command_outside_tool_contract", result["failure_codes"])

    def test_session_and_answer_key_tampering_break_roots(self) -> None:
        value = session()
        value["usage"]["output_tokens"] += 1
        with self.assertRaisesRegex(harness.ContractError, "session_root"):
            harness.validate_session(value)
        _, key = frozen_material()
        key["expected"]["work"]["target_id"] = "erdos:9999"
        with self.assertRaisesRegex(harness.ContractError, "answer_key_root"):
            harness.validate_answer_key(key)

    def test_plan_and_score_tampering_break_roots(self) -> None:
        study, _ = frozen_material()
        study["budgets"]["commands"] += 1
        with self.assertRaisesRegex(harness.ContractError, "plan_root"):
            harness.validate_plan(study)
        value = score_for("git-files-01", 100_000, 10, 1000)
        value["metrics"]["command_count"] += 1
        with self.assertRaisesRegex(harness.ContractError, "score_root"):
            harness.validate_score(value)

    def test_cached_tokens_cannot_exceed_input(self) -> None:
        value = session()
        value["usage"]["cached_input_tokens"] = 101
        harness.seal(value, "session_root")
        with self.assertRaisesRegex(harness.ContractError, "exceeds input"):
            harness.validate_session(value)

    def test_score_with_failure_code_cannot_claim_pass(self) -> None:
        value = score_for("git-files-01", 100_000, 10, 1000)
        value["failure_codes"] = ["forged_failure"]
        harness.seal(value, "score_root")
        with self.assertRaisesRegex(harness.ContractError, "absence of failure"):
            harness.validate_score(value)

    def test_report_is_counterbalanced_rooted_and_tamper_evident(self) -> None:
        study, _ = frozen_material()
        scores = [score_for("git-files-01", 50_000, 10, 1000), score_for("vela-guided-01", 35_000, 8, 800), score_for("git-files-02", 55_000, 12, 1200), score_for("vela-guided-02", 40_000, 9, 900)]
        sessions = [session("git-files-01"), session("vela-guided-01"), session("git-files-02"), session("vela-guided-02")]
        for retained, score in zip(sessions, scores, strict=True):
            retained["completed_at"] = f"2026-07-31T10:00:{score['metrics']['elapsed_ms'] // 1000:02d}Z"
            retained["elapsed_ms"] = score["metrics"]["elapsed_ms"]
            retained["usage"] = {"input_tokens": score["metrics"]["effective_tokens"], "cached_input_tokens": 0, "output_tokens": 0}
            retained["commands"] = [dict(retained["commands"][0], index=index) for index in range(1, score["metrics"]["command_count"] + 1)]
            harness.seal(retained, "session_root")
        result = harness.build_report(study, frozen_material()[1], sessions)
        self.assertTrue(result["passed"])
        result["elapsed_improvement_basis_points"] = 0
        with self.assertRaisesRegex(harness.ContractError, "report_root"):
            harness.validate_report(result)

    def test_report_recomputes_scores_from_sessions(self) -> None:
        study, key = frozen_material()
        sessions = [session("git-files-01"), session("vela-guided-01"), session("git-files-02"), session("vela-guided-02")]
        sessions[0]["answer"]["work"]["target_id"] = "erdos:9999"
        sessions[0]["answer_root"] = harness.sha256_root(harness.canonical_bytes(sessions[0]["answer"]))
        harness.seal(sessions[0], "session_root")
        result = harness.build_report(study, key, sessions)
        self.assertFalse(result["passed"])
        self.assertIn("session_failure", result["failure_codes"])

    def test_zero_elapsed_baseline_fails_instead_of_dividing_by_zero(self) -> None:
        study, key = frozen_material()
        sessions = [session("git-files-01"), session("vela-guided-01"), session("git-files-02"), session("vela-guided-02")]
        for retained in sessions:
            retained["completed_at"] = retained["started_at"]
            retained["elapsed_ms"] = 0
            harness.seal(retained, "session_root")
        result = harness.build_report(study, key, sessions)
        self.assertFalse(result["passed"])
        self.assertIn("zero_baseline_elapsed", result["failure_codes"])

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
                "standing_diff": {"transition": "add accepted Claim", "accepted_before": [], "accepted_if_accept": [{"claim_id": claim_id, "claim_root": root("e")}], "accepted_if_reject": []},
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
