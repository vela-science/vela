from __future__ import annotations

import base64
import json
import tempfile
import unittest
from pathlib import Path

import materialize_post_decision as subject


POST = {
    "frontier": {"frontier_id": "vfr_test"},
    "producer": {
        "proposal_id": "vpr_test",
        "proposal_root": "sha256:proposal",
        "claim_id": "vcl_test",
        "claim_root": "sha256:claim",
    },
    "verification": {
        "record_id": "vvr_test",
        "record_root": "sha256:verification",
        "outcome": "pass",
    },
}


def review(standing: str = "accepted") -> dict:
    return {
        "schema": "vela.review.v1",
        "ok": True,
        "frontier_id": "vfr_test",
        "proposal_id": "vpr_test",
        "proposal_root": "sha256:proposal",
        "claim": {"claim_id": "vcl_test"},
        "proposal": {"subject": {"root": "sha256:claim"}},
        "verification_records": [
            {
                "verification_record_root": "sha256:verification",
                "record": {
                    "verification_record_id": "vvr_test",
                    "outcome": "pass",
                },
            }
        ],
        "standing": standing,
        "decision": {
            "standing": standing,
            "event_id": "vev_decision",
            "event_root": "sha256:event",
            "decided_at": "2026-07-30T00:00:00Z",
            "actor": "reviewer:test",
            "reason": "Exact bounded reason.",
            "applied_event_id": "vev_applied" if standing == "accepted" else None,
        },
    }


class ReviewValidationTests(unittest.TestCase):
    def test_accept_requires_exact_verification_and_applied_event(self) -> None:
        result = subject.validate_review(review(), POST)
        self.assertEqual(result["action"], "accept")
        self.assertEqual(result["applied_event_id"], "vev_applied")

    def test_reject_has_no_applied_event(self) -> None:
        result = subject.validate_review(review("rejected"), POST)
        self.assertEqual(result["action"], "reject")
        self.assertIsNone(result["applied_event_id"])

    def test_pending_refuses_materialization(self) -> None:
        value = review()
        value["standing"] = "pending_review"
        value["decision"] = None
        with self.assertRaisesRegex(subject.MaterializeError, "human Decision"):
            subject.validate_review(value, POST)

    def test_wrong_verification_fails_closed(self) -> None:
        value = review()
        value["verification_records"][0]["verification_record_root"] = "sha256:wrong"
        with self.assertRaisesRegex(subject.MaterializeError, "Verification"):
            subject.validate_review(value, POST)


class RootDeltaTests(unittest.TestCase):
    def test_changed_and_unchanged_roots_are_explicit(self) -> None:
        checkpoint = {
            "candidate": {"release_root": "sha256:release-before"},
            "frontier": {
                "repository_root": "sha256:repo-before",
                "origin_root": "sha256:origin",
                "authority_keyset_root": "sha256:keyset",
                "authority_policy_root": "sha256:policy",
                "graph_source_root": "sha256:graph-before",
                "graph_layout_root": "sha256:layout",
            },
        }
        status = {
            "repository_root": "sha256:repo-after",
            "origin_root": "sha256:origin",
            "authority_keyset_root": "sha256:keyset",
            "authority_policy_root": "sha256:policy",
        }
        projection = {
            "graph_source_root": "sha256:graph-after",
            "graph_layout_root": "sha256:layout",
            "release_root": "sha256:release-after",
        }
        result = subject.root_delta(checkpoint, status, projection)
        self.assertEqual(
            {item["name"] for item in result["changed"]},
            {"repository", "graph_source", "projection_release"},
        )
        self.assertEqual(
            {item["name"] for item in result["unchanged"]},
            {"origin", "authority_keyset", "authority_policy", "graph_layout"},
        )


class SemanticDeltaTests(unittest.TestCase):
    BEFORE = {
        "accepted_claim_count": 10,
        "pending_claim_count": 1,
        "pending_review_count": 1,
        "accepted_review_count": 0,
        "rejected_review_count": 0,
    }

    def test_accept_has_one_exact_scientific_transition(self) -> None:
        result = subject.semantic_count_delta(
            "accepted",
            self.BEFORE,
            {
                "accepted_claims": 11,
                "pending_claims": 0,
                "pending_review": 0,
                "accepted_review": 1,
                "rejected_review": 0,
            },
        )
        self.assertEqual(result["accepted_claim_count_delta"], 1)

    def test_reject_changes_review_but_not_accepted_claims(self) -> None:
        result = subject.semantic_count_delta(
            "rejected",
            self.BEFORE,
            {
                "accepted_claims": 10,
                "pending_claims": 0,
                "pending_review": 0,
                "accepted_review": 0,
                "rejected_review": 1,
            },
        )
        self.assertEqual(result["accepted_claim_count_delta"], 0)

    def test_unrelated_count_change_fails_closed(self) -> None:
        with self.assertRaisesRegex(subject.MaterializeError, "unexpected"):
            subject.semantic_count_delta(
                "accepted",
                self.BEFORE,
                {
                    "accepted_claims": 12,
                    "pending_claims": 0,
                    "pending_review": 0,
                    "accepted_review": 1,
                    "rejected_review": 0,
                },
            )


class AuthorityEvidenceTests(unittest.TestCase):
    def test_authority_record_must_cover_exact_decision_events(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            record_dir = root / ".vela/authority/records"
            event_dir = root / ".vela/authority/events"
            record_dir.mkdir(parents=True)
            event_dir.mkdir(parents=True)
            events = []
            for event_id, kind in [
                ("vev_applied", "finding.asserted"),
                ("vev_decision", "review.accepted"),
            ]:
                value = {
                    "schema": "vela.authority-event.v1",
                    "id": event_id,
                    "content": {"kind": kind},
                }
                path = event_dir / f"{event_id}.json"
                path.write_bytes(subject.pretty_bytes(value))
                events.append(
                    {
                        "change": "A",
                        "path": str(path.relative_to(root)),
                    }
                )
            decision_event = json.loads((event_dir / "vev_decision.json").read_text())
            decision_root = subject.sha256_bytes(
                subject.canonical_bytes(decision_event)
            )
            payload = {
                "schema": "vela.authority-record.v1",
                "record_id": "var_test",
                "content": {
                    "authentication": "local_os_session",
                    "operation_id": "vop_test",
                    "transaction_id": "vtx_test",
                    "event_ids": ["vev_applied", "vev_decision"],
                },
            }
            envelope = {
                "payloadType": "application/vnd.vela.authority-record.v1+json",
                "payload": base64.b64encode(subject.canonical_bytes(payload)).decode(
                    "ascii"
                ),
                "signatures": [],
            }
            record_path = record_dir / "var_test.dsse.json"
            record_path.write_bytes(subject.pretty_bytes(envelope))
            events.append(
                {
                    "change": "A",
                    "path": str(record_path.relative_to(root)),
                }
            )
            result = subject.read_authority_evidence(
                root,
                events,
                {
                    "event_id": "vev_decision",
                    "event_root": decision_root,
                    "applied_event_id": "vev_applied",
                },
            )
            self.assertEqual(result["authority_record_id"], "var_test")
            self.assertEqual(len(result["events"]), 2)


if __name__ == "__main__":
    unittest.main()
