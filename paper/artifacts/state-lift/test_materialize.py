from __future__ import annotations

import copy
import importlib.util
import unittest
from pathlib import Path


SOURCE = Path(__file__).with_name("materialize.py")
SPEC = importlib.util.spec_from_file_location("vela_state_lift_materialize", SOURCE)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"cannot load {SOURCE}")
materialize = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(materialize)


def review(standing: str) -> dict:
    decision = {
        "standing": standing,
        "event_id": "vev_decision",
        "event_root": "sha256:" + "d" * 64,
        "reason": "Exact terminal reason.",
        "applied_event_id": "vev_applied" if standing == "accepted" else None,
    }
    return {
        "schema": "vela.review.v1",
        "proposal_id": materialize.PROPOSAL_ID,
        "proposal_root": materialize.PROPOSAL_ROOT,
        "repository_root": "sha256:" + "f" * 64,
        "standing": standing,
        "proposal": {
            "producer_package": {"root": materialize.SUBMISSION_ROOT},
            "subject": {"root": materialize.REPLACEMENT_CLAIM_ROOT},
        },
        "submission": {"submission_id": materialize.SUBMISSION_ID},
        "claim": {
            "claim_id": materialize.REPLACEMENT_CLAIM_ID,
            "relations": [
                {
                    "kind": "supersedes",
                    "target_claim_id": materialize.PREDECESSOR_CLAIM_ID,
                }
            ],
            "evidence": [{"artifact_root": materialize.SOURCE_ARTIFACT_ROOT}],
        },
        "decision": decision,
        "verification_records": [
            {
                "verification_record_root": materialize.VERIFICATION_ROOT,
                "record": {
                    "verification_record_id": materialize.VERIFICATION_ID,
                    "outcome": "pass",
                    "subject": {
                        "proposal_id": materialize.PROPOSAL_ID,
                        "claim_id": materialize.REPLACEMENT_CLAIM_ID,
                        "submission_id": materialize.SUBMISSION_ID,
                        "submission_root": materialize.SUBMISSION_ROOT,
                    },
                    "method": {"profile": "exact-source-transition-v1"},
                },
            }
        ],
    }


def inputs(standing: str = "accepted") -> dict:
    return {
        "frozen_at": "2026-07-29T12:00:00Z",
        "protocol_root": "sha256:" + "1" * 64,
        "scorer_root": "sha256:" + "2" * 64,
        "verifier_source_root": "sha256:" + "3" * 64,
        "frontier_check": {
            "ok": True,
            "frontier_id": "vfr_test",
            "git_commit": "a" * 40,
            "git_tree": "b" * 40,
            "repository_root": "sha256:" + "f" * 64,
            "epoch_id": "vre_test",
            "epoch_root": "sha256:" + "e" * 64,
        },
        "predecessor_why": {
            "claim_id": materialize.PREDECESSOR_CLAIM_ID,
            "claim_root": materialize.PREDECESSOR_CLAIM_ROOT,
            "standing": "superseded" if standing == "accepted" else "accepted",
        },
        "replacement_why": {
            "claim_id": materialize.REPLACEMENT_CLAIM_ID,
            "claim_root": materialize.REPLACEMENT_CLAIM_ROOT,
            "standing": standing,
        },
        "review": review(standing),
        "registration": {
            "schema": "vela.registration-record.v1",
            "registration_record_id": materialize.REGISTRATION_ID,
            "submission_id": materialize.SUBMISSION_ID,
            "proposal_id": materialize.PROPOSAL_ID,
            "claim_id": materialize.REPLACEMENT_CLAIM_ID,
            "accepted_state_changed": False,
            "roots": {
                "event_log_before": "sha256:" + "c" * 64,
                "event_log_after": "sha256:" + "c" * 64,
            },
        },
        "source_transition": {
            "repository": "https://example.invalid/source",
            "path": "Theorem.lean",
            "predecessor_commit": "4" * 40,
            "predecessor_file_root": "sha256:" + "5" * 64,
            "predecessor_predicate": "generatedSet.HasPosDensity",
            "successor_commit": "6" * 40,
            "successor_file_root": "sha256:" + "7" * 64,
            "successor_predicate": "generatedSet.HasPosLowerDensity",
            "diff_root": "sha256:" + "8" * 64,
        },
        "vela_version": "vela 0.940.9",
        "vela_binary_root": "sha256:" + "9" * 64,
        "runtime_name": "codex",
        "runtime_version": "codex-cli test",
        "runtime_binary_root": "sha256:" + "a" * 64,
        "model_id": "test-model",
    }


class MaterializeTests(unittest.TestCase):
    def test_accepted_outcome_is_frozen(self) -> None:
        task, answer, amendment = materialize.build_documents(**inputs())
        self.assertEqual(
            answer["expected"]["next_action_code"],
            "inspect_dependents_and_repair_or_revalidate",
        )
        self.assertEqual(
            answer["expected"]["predecessor"]["standing"],
            "superseded",
        )
        self.assertEqual(
            answer["expected"]["evidence"]["event_ids"],
            ["vev_decision", "vev_applied"],
        )
        self.assertEqual(
            answer["task_instance_root"],
            materialize.sha256_bytes(materialize.canonical_bytes(task)),
        )
        self.assertEqual(
            amendment["bindings"]["answer_key_root"],
            materialize.sha256_bytes(materialize.canonical_bytes(answer)),
        )

    def test_rejected_outcome_is_frozen(self) -> None:
        _, answer, _ = materialize.build_documents(**inputs("rejected"))
        self.assertEqual(
            answer["expected"]["next_action_code"],
            "preserve_predecessor_and_prepare_new_bounded_revision",
        )
        self.assertEqual(
            answer["expected"]["predecessor"]["standing"],
            "accepted",
        )
        self.assertEqual(
            answer["expected"]["evidence"]["event_ids"],
            ["vev_decision"],
        )

    def test_pending_decision_refuses_materialization(self) -> None:
        values = inputs()
        values["review"]["standing"] = "pending_review"
        values["review"]["decision"] = None
        with self.assertRaisesRegex(materialize.MaterializeError, "not terminal"):
            materialize.build_documents(**values)

    def test_wrong_verification_refuses_materialization(self) -> None:
        values = inputs()
        values["review"]["verification_records"][0][
            "verification_record_root"
        ] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(
            materialize.MaterializeError,
            "Verification Record is not imported",
        ):
            materialize.build_documents(**values)

    def test_review_root_drift_refuses_materialization(self) -> None:
        values = inputs()
        values["review"]["proposal_root"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(materialize.MaterializeError, "proposal root"):
            materialize.build_documents(**values)

    def test_task_materialization_is_deterministic(self) -> None:
        first = materialize.build_documents(**inputs())
        second = materialize.build_documents(**copy.deepcopy(inputs()))
        self.assertEqual(
            [materialize.canonical_bytes(item) for item in first],
            [materialize.canonical_bytes(item) for item in second],
        )


if __name__ == "__main__":
    unittest.main()
