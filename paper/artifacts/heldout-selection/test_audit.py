#!/usr/bin/env python3
"""Focused tests for the held-out correction selector."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("audit.py")
SPEC = importlib.util.spec_from_file_location("vela_heldout_audit", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class HeldoutSelectionTests(unittest.TestCase):
    def test_current_repository_epoch_is_supported(self) -> None:
        with patch.object(
            AUDIT,
            "git_json",
            return_value={"schema": "vela.repository.v4", "accepted_claims": []},
        ):
            value = AUDIT.repository_at(Path("frontier"), "head")

        self.assertEqual(value["schema"], "vela.repository.v4")

    def test_unknown_repository_epoch_fails_closed(self) -> None:
        with patch.object(
            AUDIT,
            "git_json",
            return_value={"schema": "vela.repository.v5", "accepted_claims": []},
        ):
            with self.assertRaisesRegex(ValueError, "not a supported Vela repository epoch"):
                AUDIT.repository_at(Path("frontier"), "head")

    def test_topology_requires_each_declared_route_class(self) -> None:
        predecessor = f"vcl_{'a' * 64}"
        independent = f"vcl_{'e' * 64}"
        claims = {
            predecessor: {"relations": []},
            independent: {"relations": []},
            f"vcl_{'b' * 64}": {
                "relations": [
                    {"kind": "depends_on", "target_claim_id": predecessor}
                ]
            },
            f"vcl_{'c' * 64}": {
                "relations": [
                    {"kind": "supports", "target_claim_id": predecessor},
                    {"kind": "supports", "target_claim_id": independent},
                ]
            },
            f"vcl_{'d' * 64}": {
                "relations": [{"kind": "discovery", "target_claim_id": predecessor}]
            },
        }

        result = AUDIT.topology(claims, predecessor)

        self.assertEqual(result["incoming_relation_count"], 3)
        self.assertEqual(result["hard_dependents"], [f"vcl_{'b' * 64}"])
        self.assertEqual(result["support_diamonds"], [f"vcl_{'c' * 64}"])
        self.assertEqual(
            result["nonconsequential_discovery_relations"],
            [f"vcl_{'d' * 64}"],
        )

    def test_support_route_to_an_absent_claim_is_not_a_diamond(self) -> None:
        predecessor = f"vcl_{'a' * 64}"
        source = f"vcl_{'c' * 64}"
        claims = {
            predecessor: {"relations": []},
            source: {
                "relations": [
                    {"kind": "supports", "target_claim_id": predecessor},
                    {"kind": "supports", "target_claim_id": f"vcl_{'f' * 64}"},
                ]
            },
        }

        self.assertEqual(AUDIT.topology(claims, predecessor)["support_diamonds"], [])

    def test_decision_must_match_the_applied_proposal(self) -> None:
        claim_id = f"vcl_{'1' * 64}"
        events = [
            {
                "id": "vev_applied",
                "content": {
                    "kind": "finding.superseded",
                    "timestamp": "2026-07-29T00:00:00Z",
                    "payload": {"claim_id": claim_id, "proposal_id": "vpr_right"},
                },
            },
            {
                "id": "vev_wrong",
                "content": {
                    "kind": "review.accepted",
                    "timestamp": "2026-07-29T00:00:01Z",
                    "payload": {"proposal_id": "vpr_wrong"},
                },
            },
            {
                "id": "vev_decision",
                "content": {
                    "kind": "review.accepted",
                    "timestamp": "2026-07-29T00:00:02Z",
                    "payload": {"proposal_id": "vpr_right"},
                },
            },
        ]

        self.assertEqual(
            AUDIT.decision_for_claim(events, claim_id),
            {
                "proposal_id": "vpr_right",
                "applied_event_id": "vev_applied",
                "decision_event_id": "vev_decision",
                "recorded_at": "2026-07-29T00:00:02Z",
            },
        )


if __name__ == "__main__":
    unittest.main()
