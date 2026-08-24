#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


benchmark = load_module("held_out_benchmark_test", ROOT / "benchmark.py")
custody = load_module("held_out_custody_test", ROOT / "custody.py")


def structural_response(family_id: str, condition: str = "git-documents") -> dict:
    family = benchmark.family_map()[family_id]
    manifest = benchmark.load_json(
        ROOT / f"conditions/{family_id}/{condition}/packet/PACKET-MANIFEST.json"
    )
    by_path = {entry["path"]: entry for entry in manifest["source_and_evidence"]}
    return {
        "schema": "inherited-correction-held-out-response.v1",
        "family_id": family_id,
        "predecessor_claim_id": "closed-code-test-predecessor",
        "successor_claim_id": "closed-code-test-successor",
        "consequences": [
            {
                "claim_id": claim_id,
                "classification": "unaffected",
                "action_code": "no_correction_reassessment",
            }
            for claim_id in sorted(item["claim_id"] for item in family["consequences"])
        ],
        "authority_effect_code": "no_authoritative_status_change",
        "authority_action_code": "record_no_status_change",
        "evidence_bindings": [
            {
                "path": item["evidence_path"],
                "sha256": by_path[item["evidence_path"]]["sha256"],
            }
            for item in sorted(
                family["consequences"], key=lambda item: item["claim_id"]
            )
        ],
    }


def score_snapshot_fixture(runs_dir: Path) -> dict:
    entries = []
    for row in benchmark.load_json(ROOT / "assignment-schedule.json")["assignments"]:
        run_dir = runs_dir / row["run_id"]
        run_dir.mkdir()
        run = {
            "schema": "vela.inherited-correction-held-out-run.v1",
            "run_id": row["run_id"],
            "participant_instance_id": row["participant_instance_id"],
            "family_id": row["family_id"],
            "condition": row["condition"],
            "status": "completed",
            "duration_seconds": 17,
            "tool_calls": 0,
            "attempt": 1,
            "timeout_seconds": 600,
        }
        response = structural_response(row["family_id"], row["condition"])
        run_raw = benchmark.json_bytes(run)
        response_raw = benchmark.json_bytes(response)
        (run_dir / "run.json").write_bytes(run_raw)
        (run_dir / "response.json").write_bytes(response_raw)
        entries.append(
            {
                "run_id": row["run_id"],
                "family_id": row["family_id"],
                "condition": row["condition"],
                "run_bytes": benchmark.byte_digest(run_raw),
                "response_bytes": benchmark.byte_digest(response_raw),
                "runtime_custody_root": "sha256:" + "1" * 64,
            }
        )
    capture = {
        "schema": "vela.inherited-correction-held-out-capture.v1",
        "registration_root": "sha256:" + "2" * 64,
        "complete_custody_root": "sha256:" + "3" * 64,
        "runs": entries,
        "adjudication_accessed": False,
    }
    capture["capture_root"] = benchmark.canonical_root(capture)
    return capture


def completed_capture_fixture(capture: Path, row: dict) -> None:
    state = custody.static_state()
    run_id = row["run_id"]
    condition = row["condition"]
    family_id = row["family_id"]
    evidence = capture / "evidence"
    permit_dir = capture / "permit"
    evidence.mkdir(parents=True)
    permit_dir.mkdir()
    held = custody.expected_permit(run_id)
    consumed = {
        **held,
        "status": "authorized",
        "expires_at": "2030-01-01T00:10:00Z",
    }
    consumed_raw = benchmark.json_bytes(consumed)
    consumed_path = permit_dir / f"{run_id}.permit.consumed.json"
    consumed_path.write_bytes(consumed_raw)
    launch = {
        "schema": "vela.inherited-correction-launch.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "condition": condition,
        "permit_bytes": benchmark.byte_digest(consumed_raw),
        "consumed_at": "2030-01-01T00:00:00Z",
    }
    response = structural_response(family_id, condition)
    response_raw = benchmark.json_bytes(response)
    events = [
        {"type": "thread.started"},
        {"type": "turn.started"},
        {
            "type": "item.completed",
            "item": {"type": "agent_message", "text": response_raw.decode()},
        },
        {
            "type": "turn.completed",
            "usage": {
                "input_tokens": 1000,
                "cached_input_tokens": 0,
                "output_tokens": 200,
            },
        },
    ]
    events_raw = b"".join(benchmark.compact_bytes(item) + b"\n" for item in events)
    stderr_raw = b""
    (evidence / "launch.json").write_bytes(benchmark.json_bytes(launch))
    (evidence / "provider-events.jsonl").write_bytes(events_raw)
    (evidence / "provider-stderr.txt").write_bytes(stderr_raw)
    (evidence / "participant-response.raw.json").write_bytes(response_raw)
    event_receipt = custody.validate_events(events_raw, True)
    event_receipt.pop("messages")
    receipt = {
        "schema": "vela.inherited-correction-terminal-receipt.v1",
        "run_id": run_id,
        "condition": condition,
        "participant_instance_id": row["participant_instance_id"],
        "attempt": 1,
        "status": "completed",
        "validation_error": None,
        "provider_started_at": "2030-01-01T00:00:01Z",
        "provider_completed_at": "2030-01-01T00:00:31Z",
        "duration_seconds": 30,
        "timeout_seconds": 600,
        "process_exit_code": 0,
        "process_timed_out": False,
        "registration_root": state["preregistration"]["registration_root"],
        "image_digest": state["freeze"]["image_digest"],
        "participant_configuration_root": state["freeze"][
            "runtime_configuration_roots"
        ][family_id][condition],
        "assignment_root": state["freeze"]["assignment_root"],
        "trust_bundle_bytes": state["freeze"]["trust_bundle_bytes"],
        "prompt_root": state["preregistration"]["bindings"]["prompt_roots"][family_id][
            condition
        ],
        "packet_root": state["preregistration"]["bindings"]["packet_roots"][family_id][
            condition
        ],
        "provider_events_bytes": benchmark.byte_digest(events_raw),
        "provider_stderr_bytes": benchmark.byte_digest(stderr_raw),
        "response_bytes": benchmark.byte_digest(response_raw),
        "event_receipt": event_receipt,
        "cumulative_provider_usage_is_telemetry_only": True,
        "credential_retained": False,
    }
    (evidence / "terminal-receipt.json").write_bytes(benchmark.json_bytes(receipt))


class HeldOutBenchmarkTests(unittest.TestCase):
    def test_committed_outputs_verify_and_remain_held(self) -> None:
        benchmark.verify()
        receipt = custody.verify_prelaunch()
        self.assertEqual(receipt["status"], "verified_hold")
        self.assertEqual(receipt["permits"], 36)
        self.assertEqual(receipt["consumed"], 0)

    def test_fixed_schedule_is_four_per_family_arm(self) -> None:
        schedule = benchmark.load_json(ROOT / "assignment-schedule.json")
        self.assertEqual(len(schedule["assignments"]), 36)
        for family_id in benchmark.family_map():
            for condition in benchmark.CONDITIONS:
                self.assertEqual(
                    sum(
                        row["family_id"] == family_id and row["condition"] == condition
                        for row in schedule["assignments"]
                    ),
                    4,
                )

    def test_authority_effect_is_a_closed_code_not_free_text(self) -> None:
        family_id = sorted(benchmark.family_map())[0]
        family = benchmark.family_map()[family_id]
        manifest = benchmark.load_json(
            ROOT / f"conditions/{family_id}/git-documents/packet/PACKET-MANIFEST.json"
        )
        response = structural_response(family_id)
        self.assertEqual(
            benchmark.validate_response(response, family, manifest), response
        )
        response["authority_effect_code"] = "none; status did not change"
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_authority_effect_invalid"
        ):
            benchmark.validate_response(response, family, manifest)

    def test_structured_binding_requires_exact_path_and_digest(self) -> None:
        family_id = sorted(benchmark.family_map())[0]
        family = benchmark.family_map()[family_id]
        manifest = benchmark.load_json(
            ROOT / f"conditions/{family_id}/vela/packet/PACKET-MANIFEST.json"
        )
        response = structural_response(family_id, "vela")
        response["evidence_bindings"][0].pop("sha256")
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_binding_not_in_packet"
        ):
            benchmark.validate_response(response, family, manifest)
        response = structural_response(family_id, "vela")
        response["evidence_bindings"][0]["sha256"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_binding_not_in_packet"
        ):
            benchmark.validate_response(response, family, manifest)
        response = structural_response(family_id, "vela")
        response["evidence_bindings"].pop()
        with self.assertRaisesRegex(
            benchmark.BenchmarkError, "response_bindings_invalid"
        ):
            benchmark.validate_response(response, family, manifest)

    def test_atomic_information_is_equal_within_each_family(self) -> None:
        equivalence = benchmark.load_json(ROOT / "input-equivalence.json")
        for entry in equivalence["families"]:
            family = benchmark.family_map()[entry["family_id"]]
            self.assertEqual(
                entry["atomic_facts_root"],
                benchmark.canonical_root(benchmark.family_atoms(family)),
            )
            for condition in benchmark.CONDITIONS:
                files = benchmark.packet_files(family, condition)
                self.assertEqual(
                    entry["condition_packet_roots"][condition],
                    benchmark.packet_root(files),
                )
                self.assertEqual(
                    entry["source_and_evidence"],
                    [
                        benchmark.file_entry(path, raw)
                        for path, raw in sorted(benchmark.source_files(family).items())
                    ],
                )
            self.assertLessEqual(entry["max_to_min_prompt_basis_points"], 12_000)
            self.assertTrue(entry["prompt_length_bound_pass"])

    def test_authority_regimes_are_prospectively_varied(self) -> None:
        regimes = {
            family["authority"]["regime"] for family in benchmark.family_map().values()
        }
        self.assertEqual(
            regimes,
            {
                "no_acceptance_action",
                "independently_authorized_acceptance_action",
                "authorization_presently_unprovable",
            },
        )

    def test_state_wrapper_uses_neutral_vocabulary(self) -> None:
        forbidden = benchmark.FORBIDDEN_WRAPPER_VOCABULARY
        for family_id in benchmark.family_map():
            base = ROOT / "conditions" / family_id / "state-wrapper"
            for path in base.rglob("*"):
                if path.is_file() and (
                    "/packet/" in str(path) or path.name == "prompt.txt"
                ):
                    words = set(
                        benchmark.re.findall(r"[A-Za-z]+", path.read_text().lower())
                    )
                    self.assertTrue(forbidden.isdisjoint(words), path)

    def test_packets_do_not_contain_answer_key_fields(self) -> None:
        forbidden = (
            b"required_action_code",
            b"expected_classification",
            b"adjudication_root",
            b"protected_answer",
        )
        for path in (ROOT / "conditions").rglob("*"):
            if path.is_file():
                raw = path.read_bytes()
                for token in forbidden:
                    self.assertNotIn(token, raw, path)

    def test_family_packets_do_not_leak_other_family_identifiers(self) -> None:
        family_ids = set(benchmark.family_map())
        for family_id in family_ids:
            other_ids = family_ids - {family_id}
            for path in (ROOT / "conditions" / family_id).rglob("*"):
                if path.is_file() and (
                    "/packet/" in str(path) or path.name == "prompt.txt"
                ):
                    raw = path.read_bytes()
                    for other in other_ids:
                        self.assertNotIn(other.encode(), raw, path)

    def test_missing_terminal_receipt_fails_before_ingest(self) -> None:
        run_id = "heldout-run-01"
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory)
            (capture / "evidence").mkdir()
            (capture / "permit").mkdir()
            with self.assertRaisesRegex(
                custody.CustodyError, "capture_terminal_missing"
            ):
                custody.validate_capture(capture, run_id)

    def test_held_permit_identity_is_root_bound(self) -> None:
        permit = custody.expected_permit("heldout-run-01")
        issued = {
            **permit,
            "status": "authorized",
            "expires_at": "2030-01-01T00:00:00Z",
        }
        self.assertEqual(
            custody.permit_identity(issued), custody.permit_identity(permit)
        )
        issued["packet_root"] = "sha256:" + "0" * 64
        self.assertNotEqual(
            custody.permit_identity(issued), custody.permit_identity(permit)
        )

    def test_runtime_configuration_mapping_fails_closed(self) -> None:
        row = benchmark.load_json(ROOT / "assignment-schedule.json")["assignments"][0]
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory)
            completed_capture_fixture(capture, row)
            receipt_path = capture / "evidence/terminal-receipt.json"
            receipt = json.loads(receipt_path.read_bytes())
            receipt["participant_configuration_root"] = "sha256:" + "0" * 64
            receipt_path.write_bytes(benchmark.json_bytes(receipt))
            with self.assertRaisesRegex(
                custody.CustodyError,
                "receipt_drift:participant_configuration_root",
            ):
                custody.validate_capture(capture, row["run_id"])

    def test_all_36_offline_runtime_captures_ingest_and_freeze(self) -> None:
        schedule = benchmark.load_json(ROOT / "assignment-schedule.json")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runs = root / "runs"
            runs.mkdir()
            for row in schedule["assignments"]:
                capture = root / row["run_id"]
                completed_capture_fixture(capture, row)
                custody.ingest(capture, runs, row["run_id"])
            complete = custody.complete_custody(runs)
            self.assertEqual(len(complete["runs"]), 36)
            self.assertEqual(
                complete["assignment_root"],
                benchmark.load_json(ROOT / "prelaunch-freeze.json")["assignment_root"],
            )

    def test_scoring_cannot_open_unfrozen_adjudication(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(
                benchmark.BenchmarkError, "capture_manifest_missing"
            ):
                benchmark.score_runs(Path(directory), Path("protected-key-absent.json"))

    def test_score_snapshot_rejects_structurally_valid_gate_boundary_mutation(
        self,
    ) -> None:
        for filename in ("run.json", "response.json"):
            with (
                self.subTest(filename=filename),
                tempfile.TemporaryDirectory() as directory,
            ):
                runs_dir = Path(directory)
                capture = score_snapshot_fixture(runs_dir)
                target = runs_dir / capture["runs"][0]["run_id"] / filename
                value = benchmark.load_json(target)
                if filename == "run.json":
                    value["duration_seconds"] = 18
                else:
                    value["authority_action_code"] = "accept_authorized_status_change"
                target.write_bytes(benchmark.json_bytes(value))
                with self.assertRaisesRegex(
                    benchmark.BenchmarkError,
                    "score_(run|response)_snapshot_drift",
                ):
                    benchmark.capture_bound_score_snapshot(runs_dir, capture)

    def test_full_result_summary_fixture_is_byte_stable(self) -> None:
        records = []
        family_ids = ["family-a", "family-b", "family-c"]
        exact_counts = {
            "git-documents": [1, 2, 1],
            "state-wrapper": [3, 3, 3],
            "vela": [4, 4, 4],
        }
        impact_counts = {
            "git-documents": [2, 2, 1],
            "state-wrapper": [3, 3, 3],
            "vela": [4, 4, 4],
        }
        for family_index, family_id in enumerate(family_ids):
            for condition in benchmark.CONDITIONS:
                for cell in range(4):
                    exact = cell < exact_counts[condition][family_index]
                    impact = cell < impact_counts[condition][family_index]
                    record = {
                        "family_id": family_id,
                        "condition": condition,
                        "status": "completed",
                        "duration_seconds": Decimal(
                            str(23 + family_index * 7 + cell * 3)
                        ),
                        "tool_calls": 0,
                    }
                    score = {
                        "exact_success": exact,
                        "correction_impact_complete": impact,
                        "authority_error": condition == "git-documents" and cell == 3,
                    }
                    records.append((record, score))
        summary = benchmark.summarize_records(records, family_ids)
        self.assertTrue(summary["positive"])
        for metric in (
            "exact_success_lift",
            "exact_success_rate_lift",
            "correction_impact_complete_lift",
            "correction_impact_complete_rate_lift",
            "authority_safety_lift",
            "authority_error_rate_reduction",
            "restricted_seconds_saved",
        ):
            structure = Decimal(
                str(summary["aggregate_estimands"]["structure"][metric])
            )
            governance = Decimal(
                str(summary["aggregate_estimands"]["governance_inheritance"][metric])
            )
            total = Decimal(str(summary["aggregate_estimands"]["total"][metric]))
            self.assertEqual(structure + governance, total)
        raw = benchmark.json_bytes(summary)
        self.assertEqual(
            hashlib.sha256(raw).hexdigest(),
            "b34f94e54e04babd92605ad73ba749a7352e4d9877fcb0415d0fe94ad3659ea3",
        )

    def test_governance_equality_cannot_pass(self) -> None:
        records = []
        for family_id in ("family-a", "family-b", "family-c"):
            for condition in benchmark.CONDITIONS:
                for cell in range(4):
                    exact = condition != "git-documents" and cell < 3
                    records.append(
                        (
                            {
                                "family_id": family_id,
                                "condition": condition,
                                "status": "completed",
                                "duration_seconds": Decimal("30"),
                                "tool_calls": 0,
                            },
                            {
                                "exact_success": exact,
                                "correction_impact_complete": exact,
                                "authority_error": False,
                            },
                        )
                    )
        summary = benchmark.summarize_records(
            records, ["family-a", "family-b", "family-c"]
        )
        self.assertFalse(summary["governance_strict_increment"])
        self.assertFalse(summary["gates"]["governance_inheritance"])

    def test_thresholds_are_frozen_before_external_answers(self) -> None:
        preregistration = benchmark.load_json(ROOT / "preregistration.json")
        self.assertIn(
            "Vela >=3/4 exact",
            preregistration["scoring"]["family_gates"]["total"],
        )
        self.assertIn(
            ">=3/4 correction-impact-complete",
            preregistration["scoring"]["family_gates"]["total"],
        )
        self.assertIn(
            "Equality alone cannot pass",
            preregistration["scoring"]["aggregate_gates"]["governance_inheritance"],
        )
        self.assertIn(
            "Vela >=9/12 exact",
            preregistration["scoring"]["aggregate_gates"]["total"],
        )
        commitment = preregistration["bindings"]["adjudication_commitment"]
        self.assertEqual(
            commitment["adjudication_root"],
            "sha256:26f5a7fb4ae0afcd4f0143e7efb9087b9dd05ff264590450d4361473deb2c39d",
        )
        self.assertEqual(commitment["status"], "frozen")
        self.assertFalse(commitment["plaintext_disclosed"])
        self.assertFalse(commitment["answer_bytes_present_in_producer_artifact"])
        amendment = preregistration["bindings"]["launch_authorization_amendment"]
        self.assertEqual(amendment["status"], "authorized_held_pending_binding_review")
        self.assertEqual(amendment["execution_state"]["sessions_completed"], 0)
        self.assertEqual(amendment["execution_state"]["permits_consumed"], 0)

    def test_public_adjudication_commitment_is_exact_and_answer_free(self) -> None:
        commitment = benchmark.load_json(ROOT / "adjudication-commitment.json")
        self.assertEqual(
            commitment,
            {
                "schema": "vela.inherited-correction-held-out-adjudication-commitment.v1",
                "status": "frozen",
                "adjudication_root": "sha256:26f5a7fb4ae0afcd4f0143e7efb9087b9dd05ff264590450d4361473deb2c39d",
                "adjudication_bytes_sha256": "sha256:26f5a7fb4ae0afcd4f0143e7efb9087b9dd05ff264590450d4361473deb2c39d",
                "adjudication_bytes_length": 5883,
                "private_validation_receipt_root": "sha256:581b944cdfdb82a2f9730ffd3d60fba13c3e4916bbf344ab1d495565dafccf11",
                "public_commitment_root": "sha256:cf22cc93f1b882e85327943e074ef6d0cd60f90c3989f0801c46d60f5fad721a",
                "frozen_at": "2026-08-21T21:51:33Z",
                "family_count": 3,
                "consequence_count": 12,
                "plaintext_disclosed": False,
                "answer_bytes_present_in_producer_artifact": False,
                "required_before_permit_release": True,
            },
        )
        amendment = benchmark.load_json(ROOT / "launch-authorization-amendment.json")
        for key, value in amendment["evaluator_commitment"].items():
            self.assertEqual(commitment[key], value)
        self.assertFalse((ROOT / "adjudication.json").exists())
        self.assertEqual(
            amendment["authorization"]["bound_prior_producer_commit"],
            "8cc1a89d7b1ae47cb6cabb36bfd79b46c3f4db81",
        )
        self.assertEqual(
            amendment["authorization"]["bound_prior_review_commit"],
            "f9b5d67a55c1ad41fcb67cc1d7ebe86d03d37782",
        )


if __name__ == "__main__":
    unittest.main()
