#!/usr/bin/env python3
"""Focused tests for Vela's small layer over native Harbor tasks."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import contract
import freeze_campaign
import materialize
import summarize


def root(char: str) -> str:
    return f"sha256:{char * 64}"


def verification(record_id: str = "vvr_0123456789abcdef", role: str = "requirement_satisfying") -> dict:
    return {
        "verification_record_id": record_id,
        "verification_record_root": root("6" if role == "requirement_satisfying" else "7"),
        "outcome": "pass",
        "property": "Replay the exact retained artifact.",
        "verifier": "verifier:test-v1",
        "independent_of_producer": True,
        "protocol_evidence_role": role,
        "satisfies_requirements": ["Replay the exact retained artifact."] if role == "requirement_satisfying" else [],
        "does_not_establish": ["Scientific acceptance."],
    }


def next_obligation() -> dict:
    return {
        "now": "Human authority may inspect this exact rooted entry.",
        "if_accept": "Replay the successor and recompute Targets.",
        "if_reject": "Replay without accepted Standing change.",
    }


def answer(scenario: str = materialize.SCENARIOS[0]) -> dict:
    proposed = f"vcl_{'d' * 64}"
    target = f"vcl_{'e' * 64}"
    quantum = scenario == materialize.SCENARIOS[1]
    before = [{"claim_id": target, "claim_root": root("b")}] if quantum else []
    accepted = [{"claim_id": proposed, "claim_root": root("e")}]
    transition = "supersede accepted Claim with corrected Claim" if quantum else "add accepted Claim"
    scope_target = target if quantum else proposed
    affected = [proposed, target] if quantum else [proposed]
    unchanged = 4 if quantum else 14
    result = {
        "schema": "vela.product-compression-answer.v9",
        "scenario": scenario,
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef",
            "repository_root": root("1"),
            "configured_targets": 0,
        },
        "decision": {
            "proposal_id": "vpr_0123456789abcdef",
            "proposal_root": root("a"),
            "source_submission_id": "vsb_0123456789abcdef",
            "source_submission_root": root("3"),
            "proposed_claim_id": proposed,
            "proposed_claim_root": root("e"),
            "requested_change": (
                {"kind": "supersede_claim", "target_claim_id": target, "target_claim_root": root("b")}
                if quantum else {"kind": "add_claim"}
            ),
            "assertion": "This exact bounded computation establishes one scoped result.",
            "conditions": ["The retained exact inputs."],
            "limits": ["This does not establish a broader result."],
            "verifications": [verification()],
            "verification_set_root": root("f"),
            "inbox_entry_root": root("c"),
            "protocol_gate": "satisfied",
            "blockers": [],
            "human_decision_required": True,
            "verification_is_acceptance": False,
            "standing_delta": {
                "transition": transition,
                "scope": {
                    "kind": "proposal_affected_claims",
                    "target_claim_id": scope_target,
                    "affected_claim_ids": affected,
                },
                "before": {"repository_root": root("1"), "accepted": before},
                "if_accept": {"repository_root": root("4"), "accepted": accepted},
                "if_reject": {"repository_root": root("5"), "accepted": before},
                "counts": {
                    "unchanged_accepted_claims": unchanged,
                    "global_accepted_claims": {
                        "before": unchanged + len(before),
                        "if_accept": unchanged + len(accepted),
                        "if_reject": unchanged + len(before),
                    },
                },
            },
            "staleness": "current",
            "next_obligation": next_obligation(),
        },
    }
    if quantum:
        result["decision"]["verifications"].append(
            verification("vvr_fedcba9876543210", "complementary")
        )
    return result


def fixture(
    scenario: str = materialize.SCENARIOS[0], commit: str = "1" * 40, tree: str = "2" * 40,
) -> dict:
    return contract.seal({
        "schema": "vela.product-compression-fixture.v7",
        "fixture_root": "",
        "scenario": scenario,
        "vela": {"version": "vela test", "binary_sha256": root("7")},
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef",
            "git_commit": commit,
            "git_tree": tree,
            "repository_root": root("1"),
            "inbox_projection_root": root("2"),
            "configured_targets": 0,
        },
        "anchor": {"kind": "test"},
    }, "fixture_root")


def answer_key(source: dict | None = None) -> dict:
    source = source or fixture()
    return contract.seal({
        "schema": "vela.product-compression-answer-key.v9",
        "answer_key_root": "",
        "fixture_root": source["fixture_root"],
        "scenario": source["scenario"],
        "expected": answer(source["scenario"]),
    }, "answer_key_root")


def plan(scenario: str = materialize.SCENARIOS[0]) -> dict:
    source = fixture(scenario)
    key = answer_key(source)
    return contract.seal({
        "schema": "vela.product-compression-plan.v11",
        "plan_root": "",
        "scenario": scenario,
        "fixture_root": source["fixture_root"],
        "answer_key_root": key["answer_key_root"],
        "task_roots": [],
        "harbor_job_root": root("8"),
        "comparison_rule": materialize.COMPARISON,
        "claim_limit": "Bounded first-party benchmark evidence only.",
    }, "plan_root")


def write_job(directory: Path, scenario: str, exact_trials: set[tuple[str, int]]) -> None:
    total_cost = 0.0
    rows = [(arm, attempt) for attempt in range(1, 3) for arm in materialize.ARMS]
    for index, (arm, attempt) in enumerate(rows, start=1):
        trial = directory / f"native-{index}"
        cost = 0.25 if arm == "vela-guided" else 0.5
        total_cost += cost
        contract.write_json(trial / "result.json", {
            "id": f"trial-{index}",
            "task_name": f"vela/product-compression-{scenario}-{arm}",
            "finished_at": "2026-07-31T12:00:05Z",
            "exception_info": None,
            "agent_execution": {
                "started_at": "2026-07-31T12:00:00Z",
                "finished_at": "2026-07-31T12:00:02Z" if arm == "vela-guided" else "2026-07-31T12:00:04Z",
            },
            "agent_result": {"cost_usd": cost},
            "verifier_result": {"rewards": {"eligible": 1, "exact": int((arm, attempt) in exact_trials)}},
        })
    contract.write_json(directory / "result.json", {
        "id": "product-compression-test",
        "finished_at": "2026-07-31T12:00:06Z",
        "n_total_trials": 4,
        "stats": {
            "n_completed_trials": 4,
            "n_errored_trials": 0,
            "n_running_trials": 0,
            "n_pending_trials": 0,
            "n_cancelled_trials": 0,
            "n_retries": 0,
            "cost_usd": total_cost,
        },
    })


def continuation_answer() -> dict:
    value = {
        "schema": "vela.product-compression-answer.v9",
        "scenario": materialize.SCENARIOS[2],
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef", "repository_root": root("1"),
            "target_index_root": root("2"), "configured_targets": 1,
        },
        "continuation": {
            "accepted_claim_id": f"vcl_{'a' * 64}", "accepted_claim_root": root("a"),
            "standing_basis": "compacted_origin", "origin_root": root("b"),
            "archive_bytes_re_read": False,
            "proposal_id": "vpr_0123456789abcdef", "proposal_root": root("c"),
            "submission_id": "vsb_0123456789abcdef", "submission_root": root("d"),
            "verification_id": "vvr_0123456789abcdef", "verification_root": root("e"),
            "decision_event_id": "vev_0123456789abcdef", "decision_event_root": root("f"),
            "decision_actor": "human", "accepted_first": 1, "accepted_through": 200,
            "next_target_id": "target:test", "next_first": 201, "next_last": 400,
            "packet_root": root("6"), "verifier_profile": "test-v1",
            "verification_is_acceptance": False, "decision_changes_standing": True,
            "next_target_changes_standing": False,
        },
    }
    return value


def action_complete_baseline() -> dict:
    slugs = ("erdos", "formal-conjectures", "quantum-codes", "sidon-sets")
    frontiers = []
    for index, slug in enumerate(slugs, start=1):
        row = {
            "slug": slug,
            "git_commit": str(index) * 40,
            "git_tree": chr(96 + index) * 40,
            "remote": f"https://example.invalid/{slug}.git",
            "frontier_id": f"vfr_{index:016x}",
            "repository_root": root(str(index)),
            "status_root": root(chr(96 + index)),
            "offer_root": root(str(index + 4)),
            "integrity": {"replay": "verified", "strict": "pass", "blocker_count": 0},
            "availability": (
                {"configured": 1, "stale": 0, "fresh": 1, "returned": 1}
                if slug == "erdos"
                else {"configured": 0, "stale": 0, "fresh": 0, "returned": 0}
            ),
            "counts": {"accepted_claims": index},
        }
        if slug == "erdos":
            row["target"] = {
                "target_id": "erdos:1056",
                "packet_root": root("f"),
                "verifier_profile": "erdos-1056-k15-bounded-replay-v1",
                "next_range": {"first": 10430801, "last": 10431000, "inclusive": True},
            }
        else:
            row["next_action"] = "No Target Index is configured; inspect the Frontier before inventing work."
        frontiers.append(row)
    value = {
        "schema": "vela.action-complete-campaign-baseline.v1",
        "baseline_root": "",
        "observed_at": "2026-08-03T12:00:00Z",
        "source_state": {
            "vela": {
                "version": "vela 0.963.0",
                "binary_sha256": root("a"),
                "source_identity_root": root("b"),
                "git_commit": "a" * 40,
                "git_tree": "b" * 40,
                "remote": "https://example.invalid/vela.git",
            },
            "harbor": {"version": "0.20.0"},
            "frontiers": frontiers,
            "observatory": {
                "url": "https://app.vela.space/.well-known/vela-site.json",
                "manifest_sha256": root("c"),
                "schema": "vela.site-deployment.v4",
                "authority": "read_only_projection",
                "site_version": "0.430.0",
                "site_commit": "c" * 40,
                "projection_schema": "vela.observatory-release-manifest.v9",
                "read_model_schema": "observatory.v8",
                "projection_root": root("d"),
                "vela_version": "vela 0.963.0",
                "vela_binary_sha256": root("e"),
                "frontiers": [
                    {
                        "slug": row["slug"],
                        "git_commit": row["git_commit"],
                        "git_tree": row["git_tree"],
                        "repository_root": row["repository_root"],
                    }
                    for row in frontiers
                ],
            },
        },
        "benchmark": {
            "implementation": "native_harbor",
            "arms": ["git-files", "vela-guided"],
            "task_classes": list(contract.ACTION_COMPLETE_TASK_CLASSES),
            "contract_roots": {"contract": root("1"), "task_template": root("2")},
            "custody": {
                "answer_key_available_to_agent": False,
                "authority_credentials_available_to_agent": False,
                "canonical_checkout_mutable": False,
                "automatic_decision": False,
            },
            "instrumentation_pilot": {"repetitions_per_arm": 2, "claim_credit": False},
            "confirmatory_design": {
                "power": 0.8,
                "two_sided_alpha": 0.05,
                "minimum_useful_effect": 0.2,
                "sample_size_rule": "computed_from_blinded_pilot_variance",
            },
            "primary_metrics": ["ETY", "VPAC", "FIE", "CPI", "correction_resilience"],
        },
        "limitations": ["Bounded test baseline."],
    }
    return contract.seal(value, "baseline_root")


def load_verifier():
    path = Path(__file__).parent / "task" / "tests" / "verify.py"
    spec = importlib.util.spec_from_file_location("product_compression_verify", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ProductCompressionTests(unittest.TestCase):
    def test_action_complete_baseline_is_content_bound(self) -> None:
        value = action_complete_baseline()
        contract.validate_action_complete_baseline(value)
        value["source_state"]["frontiers"][0]["repository_root"] = root("9")
        with self.assertRaisesRegex(contract.ContractError, "root mismatch"):
            contract.validate_action_complete_baseline(value)

    def test_action_complete_baseline_rejects_projection_drift(self) -> None:
        value = action_complete_baseline()
        value["source_state"]["observatory"]["frontiers"][0]["git_commit"] = "f" * 40
        value = contract.seal(value, "baseline_root")
        with self.assertRaisesRegex(contract.ContractError, "source drift"):
            contract.validate_action_complete_baseline(value)

    def test_action_complete_baseline_rejects_wrong_target_or_invented_work(self) -> None:
        value = action_complete_baseline()
        value["source_state"]["frontiers"][0]["target"]["next_range"]["first"] += 1
        value = contract.seal(value, "baseline_root")
        with self.assertRaisesRegex(contract.ContractError, "unexpected range"):
            contract.validate_action_complete_baseline(value)

        value = action_complete_baseline()
        value["source_state"]["frontiers"][1]["availability"]["configured"] = 1
        value = contract.seal(value, "baseline_root")
        with self.assertRaisesRegex(contract.ContractError, "exact absence"):
            contract.validate_action_complete_baseline(value)

    def test_action_complete_baseline_rejects_answer_or_authority_exposure(self) -> None:
        for field in ("answer_key_available_to_agent", "authority_credentials_available_to_agent", "automatic_decision"):
            value = action_complete_baseline()
            value["benchmark"]["custody"][field] = True
            value = contract.seal(value, "baseline_root")
            with self.assertRaisesRegex(contract.ContractError, "isolation"):
                contract.validate_action_complete_baseline(value)

    def test_action_complete_baseline_rejects_pilot_claim_credit(self) -> None:
        value = action_complete_baseline()
        value["benchmark"]["instrumentation_pilot"]["claim_credit"] = True
        value = contract.seal(value, "baseline_root")
        with self.assertRaisesRegex(contract.ContractError, "pilot cannot earn"):
            contract.validate_action_complete_baseline(value)

    def test_campaign_projection_removes_local_checkout_paths(self) -> None:
        status = {
            "frontier": {"id": "vfr_0123456789abcdef"},
            "git": {"commit": "a" * 40, "tree": "b" * 40},
            "integrity": {"replay": "verified", "strict": "pass", "blocker_count": 0},
            "roots": {"repository": root("1")},
            "counts": {},
            "work": {"ready_target_count": 0},
            "decision_inbox": {"pending_count": 0, "projection_root": root("2")},
            "actions": {"work": {"command": "vela next /private/local/path --json"}},
        }
        offer = {
            "frontier_id": "vfr_0123456789abcdef",
            "repository_root": root("1"),
            "availability": {"configured": 0, "stale": 0, "fresh": 0, "returned": 0},
            "targets": [],
            "next_action": "Inspect before inventing work.",
        }
        self.assertNotIn("actions", freeze_campaign.stable_status(status))
        self.assertNotIn("/private/local/path", json.dumps(freeze_campaign.stable_offer(offer)))

    def test_harbor_verifier_creates_its_output_directory(self) -> None:
        test_script = (Path(__file__).parent / "task" / "tests" / "test.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("mkdir -p /logs/verifier", test_script)

    def test_harbor_verifier_uses_the_native_post_agent_phase(self) -> None:
        task = (Path(__file__).parent / "task" / "task.toml").read_text(encoding="utf-8")
        self.assertNotIn("environment_mode", task)
        self.assertIn('network_mode = "no-network"', task)

        dockerfile = (Path(__file__).parent / "task" / "environment" / "Dockerfile").read_text(
            encoding="utf-8"
        )
        self.assertIn("python3", dockerfile)

    def test_contract_preserves_authority_boundary(self) -> None:
        contract.validate_answer(answer())
        value = answer()
        value["decision"]["verification_is_acceptance"] = True
        with self.assertRaisesRegex(contract.ContractError, "authority boundary"):
            contract.validate_answer(value)

    def test_contract_rejects_false_standing_delta(self) -> None:
        value = answer()
        value["decision"]["standing_delta"]["if_reject"] = value["decision"]["standing_delta"]["if_accept"]
        with self.assertRaisesRegex(contract.ContractError, "rejection must preserve"):
            contract.validate_answer(value)

    def test_contract_rejects_invented_target(self) -> None:
        value = answer()
        value["frontier"]["configured_targets"] = 1
        with self.assertRaisesRegex(contract.ContractError, "no invented Target"):
            contract.validate_answer(value)

    def test_quantum_supersession_is_exact(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        contract.validate_answer(value)
        value["decision"]["standing_delta"]["if_accept"] = value["decision"]["standing_delta"]["before"]
        with self.assertRaisesRegex(contract.ContractError, "replace exactly"):
            contract.validate_answer(value)

    def test_quantum_target_must_be_accepted_predecessor(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        value["decision"]["requested_change"]["target_claim_id"] = f"vcl_{'a' * 64}"
        with self.assertRaisesRegex(contract.ContractError, "invalid supersession scope"):
            contract.validate_answer(value)

    def test_quantum_count_drift_fails(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        value["decision"]["standing_delta"]["counts"]["global_accepted_claims"]["if_accept"] += 1
        with self.assertRaisesRegex(contract.ContractError, "counts"):
            contract.validate_answer(value)

    def test_post_decision_continuation_is_exact(self) -> None:
        contract.validate_answer(continuation_answer())
        value = continuation_answer()
        value["continuation"]["next_first"] += 1
        with self.assertRaisesRegex(contract.ContractError, "first post-Decision Target"):
            contract.validate_answer(value)

    def test_post_decision_preserves_authority_boundary(self) -> None:
        value = continuation_answer()
        value["continuation"]["verification_is_acceptance"] = True
        with self.assertRaisesRegex(contract.ContractError, "authority is misstated"):
            contract.validate_answer(value)

    def test_post_decision_materialization_binds_current_cli_views(self) -> None:
        accepted_claim = f"vcl_{'a' * 64}"
        packet = {
            "accepted_state": {"latest_bounded_negative": {
                "claim_id": accepted_claim, "claim_root": root("a"),
                "range": {"first": 1, "last": 200},
            }},
            "target": {"next_bounded_range": {"first": 201, "last": 400}},
            "completion_contract": {"duplicate_range_forbidden": True},
        }
        with tempfile.TemporaryDirectory() as directory:
            frontier = Path(directory).resolve()
            packet_path = frontier / "targets" / "packet.json"
            contract.write_json(packet_path, packet)
            packet_root = materialize.digest(packet_path)
            next_work = {
                "frontier_id": "vfr_0123456789abcdef", "repository_root": root("1"),
                "target_index_root": root("2"),
                "availability": {"configured": 1, "stale": 0, "fresh": 1, "returned": 1},
                "targets": [{
                    "target_id": "target:test", "verifier_profile": "test-v1",
                    "packet": {"path": "targets/packet.json", "sha256": packet_root},
                }],
            }
            proposal = {
                "proposal_root": root("7"),
                "proposal": {
                    "proposal_id": "vpr_0123456789abcdef",
                    "producer_package": {"id": "vsb_0123456789abcdef", "root": root("8")},
                },
            }
            verification_row = {
                "verification_record_root": root("9"),
                "verification_record": {"verification_record_id": "vvr_0123456789abcdef"},
            }
            why = {
                "frontier_id": next_work["frontier_id"],
                "repository_root": next_work["repository_root"],
                "claim_id": accepted_claim, "claim_root": root("a"), "standing": "accepted",
                "interpretation": {
                    "submission_is_acceptance": False,
                    "verification_is_acceptance": False,
                    "standing_is_derived": True,
                },
                "chain": {
                    "standing_basis": "compacted_origin",
                    "standing_basis_detail": {"origin_root": root("b"), "archive_bytes_re_read": False},
                    "proposals": [proposal], "verification_records": [verification_row],
                    "authority_events": [
                        {"authority_event_id": "vev_0123456789abcdef", "authority_event_root": root("c"), "event": {"content": {"kind": "review.accepted", "actor": {"type": "human"}}}},
                        {"authority_event_id": "vev_fedcba9876543210", "authority_event_root": root("d"), "event": {"content": {"kind": "finding.asserted", "actor": {"type": "human"}}}},
                    ],
                },
            }
            def fake_command(argv, *, cwd=None):
                self.assertEqual(cwd, frontier)
                if argv[1:3] == ("status", "--porcelain"):
                    return ""
                if argv[1:3] == ("rev-parse", "HEAD"):
                    return "1" * 40
                if argv[1:3] == ("rev-parse", "HEAD^{tree}"):
                    return "2" * 40
                self.fail(f"unexpected command: {argv}")

            def fake_json(argv, *, cwd):
                self.assertEqual(cwd, frontier)
                if argv[1:3] == ("next", "."):
                    return next_work
                if argv[1:3] == ("why", "."):
                    self.assertEqual(argv[3], accepted_claim)
                    return why
                self.fail(f"unexpected JSON command: {argv}")

            with mock.patch.object(materialize, "command", side_effect=fake_command), mock.patch.object(materialize, "json_command", side_effect=fake_json):
                inspection = materialize.inspect_terminal_continuation(frontier, frontier / "vela", accepted_claim)
            contract.validate_answer(materialize.terminal_continuation_answer(inspection))

    def test_answer_key_is_content_bound(self) -> None:
        key = answer_key()
        contract.validate_answer_key(key)
        key["expected"]["decision"]["proposal_root"] = root("4")
        with self.assertRaisesRegex(contract.ContractError, "root mismatch"):
            contract.validate_answer_key(key)

    def test_published_schema_is_recursively_closed(self) -> None:
        stack = [json.loads((Path(__file__).parent / "answer.schema.json").read_text())]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                if node.get("type") == "object":
                    self.assertIs(node.get("additionalProperties"), False)
                stack.extend(node.values())
            elif isinstance(node, list):
                stack.extend(node)

    def test_harbor_verifier_normalizes_scientific_sets_only(self) -> None:
        verifier = load_verifier()
        source = fixture(materialize.SCENARIOS[1])
        key = answer_key(source)
        exact = verifier.outcome(answer(source["scenario"]), key, source)
        self.assertEqual((exact["eligible"], exact["exact"]), (True, True))
        reordered = answer(source["scenario"])
        reordered["decision"]["verifications"].reverse()
        reordered["decision"]["verifications"][0]["does_not_establish"].reverse()
        order_independent = verifier.outcome(reordered, key, source)
        self.assertEqual((order_independent["eligible"], order_independent["exact"]), (True, True))
        omitted = answer(source["scenario"])
        omitted["decision"]["verifications"].pop()
        mismatch = verifier.outcome(omitted, key, source)
        self.assertEqual((mismatch["eligible"], mismatch["exact"]), (True, False))
        source["scenario"] = materialize.SCENARIOS[0]
        invalid = verifier.outcome(answer(materialize.SCENARIOS[1]), key, source)
        self.assertFalse(invalid["eligible"])
        self.assertIn("fixture_invalid", invalid["eligibility_failure_codes"])

    def test_materializer_builds_native_harbor_study(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            frontier = base / "frontier"
            frontier.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=frontier, check=True)
            (frontier / "README.md").write_text("fixture\n")
            subprocess.run(["git", "add", "README.md"], cwd=frontier, check=True)
            subprocess.run(["git", "commit", "-qm", "fixture"], cwd=frontier, check=True)
            commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()
            tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()

            source = fixture(materialize.SCENARIOS[1], commit, tree)
            key = answer_key(source)
            binary = base / "vela-linux"
            binary.write_bytes(b"\x7fELFtest")
            output = base / "study"
            prepared = materialize.build_study(
                source, key, "Inspect the quantum supersession.", "Bounded claim.",
                frontier, binary, "test-model", "0.1.0",
                "vela-product-compression-test", output,
            )
            self.assertEqual(prepared["schema"], "vela.product-compression-plan.v11")
            self.assertEqual(prepared["scenario"], materialize.SCENARIOS[1])
            self.assertEqual(len(prepared["task_roots"]), 2)
            baseline = output / "tasks/git-files"
            guided = output / "tasks/vela-guided"
            self.assertNotIn("{{", (baseline / "instruction.md").read_text())
            self.assertNotIn("COPY vela", (baseline / "environment/Dockerfile").read_text())
            self.assertIn("COPY vela", (guided / "environment/Dockerfile").read_text())
            self.assertIn(".frontier.git_commit", (guided / "environment/Dockerfile").read_text())
            self.assertIn("quantum-certificate-supersession", (guided / "task.toml").read_text())
            self.assertIn("@openai/codex@0.1.0", (baseline / "environment/Dockerfile").read_text())
            self.assertIn("python3", (baseline / "environment/Dockerfile").read_text())
            self.assertFalse(any(baseline.rglob("__pycache__")))
            resolved = subprocess.run(
                ["harbor", "run", "--config", str(output / "harbor-job.json"), "--print-config"],
                check=False, capture_output=True, text=True,
            )
            self.assertEqual(resolved.returncode, 0, resolved.stderr)
            self.assertEqual(json.loads(resolved.stdout)["n_attempts"], 2)

    def test_native_harbor_summary_records_bounded_lift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, {("vela-guided", 1), ("vela-guided", 2)})
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["schema"], "vela.product-compression-native-harbor-result.v6")
            self.assertEqual(result["scenario"], scenario)
            self.assertEqual(result["conclusion"]["outcome"], "pass_task_specific_exactness_advantage")
            self.assertEqual(result["result_root"], contract.record_root(result, "result_root"))

    def test_native_harbor_summary_preserves_negative_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, set())
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["conclusion"]["outcome"], "failed_no_product_lift_credit")
            self.assertEqual(result["result_root"], contract.record_root(result, "result_root"))

    def test_native_harbor_summary_rejects_mixed_scenarios(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, set())
            first = next(job.glob("*/result.json"))
            value = contract.read_json(first)
            value["task_name"] = f"vela/product-compression-{materialize.SCENARIOS[0]}-git-files"
            contract.write_json(first, value)
            with self.assertRaisesRegex(contract.ContractError, "unknown task identity"):
                summarize.summarize(plan_path, job)


if __name__ == "__main__":
    unittest.main()
