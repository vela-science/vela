from __future__ import annotations

import importlib.util
import hashlib
import json
import pathlib
import tomllib

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
MATERIALIZER_PATH = ROOT / "benchmarks/erdos-264-proof-repair/materialize.py"
SPEC = importlib.util.spec_from_file_location(
    "erdos_264_proof_repair_materializer", MATERIALIZER_PATH
)
assert SPEC is not None and SPEC.loader is not None
MATERIALIZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATERIALIZER)
DECISION_PACKET = ROOT / "paper/artifacts/erdos-264/decision-packet.v1.json"
STUDY_RESULT = (
    ROOT
    / "paper/artifacts/erdos-264-proof-repair-2026-08-03/result.v1.json"
)


def exact_inputs() -> tuple[dict, dict, dict]:
    packet = {
        "schema": "erdos-frontier.correction-inheritance-work.v1",
        "authority": "non_authoritative",
        "target": {"id": MATERIALIZER.TARGET_ID},
        "prerequisite": {
            "accepted_claim": {
                "claim_id": MATERIALIZER.CORRECTION_CLAIM_ID,
                "claim_root": MATERIALIZER.CORRECTION_CLAIM_ROOT,
            }
        },
        "source": {
            "repository": "https://github.com/google-deepmind/formal-conjectures.git",
            "commit": MATERIALIZER.FORMAL_COMMIT,
            "tree": MATERIALIZER.FORMAL_TREE,
            "path": MATERIALIZER.FORMAL_PATH,
            "sha256": MATERIALIZER.FORMAL_SHA256,
            "lean_toolchain": "leanprover/lean4:v4.27.0",
            "mathlib_commit": "a3a10db0e9d66acbebf76c5e6a135066525ac900",
            "declaration": "Erdos264.erdos_264.parts.i",
        },
        "known_prior_evidence": {
            "revision": MATERIALIZER.REFERENCE_COMMIT,
            "path": MATERIALIZER.REFERENCE_PATH,
            "file_sha256": MATERIALIZER.REFERENCE_SHA256,
        },
    }
    offer = {
        "availability": {"configured": 3, "stale": 0, "fresh": 3, "returned": 1},
        "targets": [
            {
                "target_id": MATERIALIZER.TARGET_ID,
                "rank": 1,
                "packet": {"path": MATERIALIZER.TARGET_PACKET},
            }
        ],
    }
    repository = {
        "accepted_claims": [
            {
                "claim_id": MATERIALIZER.CORRECTION_CLAIM_ID,
                "claim_root": MATERIALIZER.CORRECTION_CLAIM_ROOT,
                "standing": "accepted",
            }
        ]
    }
    return offer, packet, repository


def test_significant_repair_requires_exact_accepted_correction() -> None:
    offer, packet, repository = exact_inputs()
    target = MATERIALIZER.validate_target_projection(offer, packet, repository)
    assert target["target_id"] == MATERIALIZER.TARGET_ID
    repository["accepted_claims"][0]["standing"] = "pending_review"
    with pytest.raises(MATERIALIZER.MaterializationError, match="not accepted"):
        MATERIALIZER.validate_target_projection(offer, packet, repository)


def test_significant_repair_must_be_first_current_target() -> None:
    offer, packet, repository = exact_inputs()
    offer["targets"][0]["target_id"] = "erdos:1056"
    with pytest.raises(MATERIALIZER.MaterializationError, match="first Target"):
        MATERIALIZER.validate_target_projection(offer, packet, repository)


def test_unrelated_fresh_targets_do_not_invalidate_the_first_target() -> None:
    offer, packet, repository = exact_inputs()
    offer["availability"] = {
        "configured": 19,
        "stale": 0,
        "fresh": 19,
        "returned": 1,
    }
    target = MATERIALIZER.validate_target_projection(offer, packet, repository)
    assert target["target_id"] == MATERIALIZER.TARGET_ID


def test_task_is_one_matched_pair_with_separate_native_verifier() -> None:
    template = ROOT / "benchmarks/erdos-264-proof-repair/task/task.toml"
    value = tomllib.loads(template.read_text().replace("{{ARM}}", "git-files"))
    assert value["verifier"]["environment_mode"] == "separate"
    assert value["verifier"]["network_mode"] == "no-network"
    assert value["artifacts"] == [
        {"source": "/logs/artifacts/264.lean", "destination": "264.lean"}
    ]
    source = MATERIALIZER_PATH.read_text()
    assert '"--n-attempts",\n            "1"' in source
    assert 'automatic_decision": False' in source


def test_case_binds_recognizable_scientific_episode_and_limits_credit() -> None:
    assert MATERIALIZER.PUBLICATION_ID == "arXiv:2406.17593"
    assert MATERIALIZER.SOURCE_CORRECTION_BEFORE == (
        "593e6b76702c5dbffaaa91b59f4faaed705d04ce"
    )
    assert MATERIALIZER.SOURCE_CORRECTION_COMMIT == (
        "0598b8f281060a18416d60753fd75621d659bb07"
    )
    source = MATERIALIZER_PATH.read_text()
    assert '"evidence_level": "real_correction_case"' in source
    assert '"scientific_episode_root": episode_root' in source
    assert '"new theorem discovery"' in source
    assert '"statistical agent lift"' in source


def test_decision_packet_binds_one_human_choice_and_native_successor() -> None:
    packet = json.loads(DECISION_PACKET.read_text())
    assert packet["authority_boundary"].startswith(
        "This is a read-only packet for one human choice."
    )
    assert packet["decision_inbox"] == {
        "projection_root": "sha256:303e15ba1bf6482c3b51ac239a2d612c638367156f7eebe3718c7a6c75c449e7",
        "entry_root": "sha256:2975c7cbbb52e68c93c29539f0ff5bcf2b9ccc545645887022795aff2268f8b5",
        "protocol_gate": "satisfied",
        "human_decision_required": True,
        "blockers": [],
        "options": ["accept", "reject", "leave_pending"],
    }
    assert packet["proposal"]["proposal_id"] == "vpr_2f52fe736670aff8"
    assert (
        "--if-entry-root " + packet["decision_inbox"]["entry_root"]
        in packet["actions"]["accept"]
    )
    assert "<human reason>" in packet["actions"]["accept"]
    assert packet["verification"]["outcome"] == "pass"
    assert packet["standing_delta"]["unchanged_accepted_claims"] == 2773
    assert packet["successor"]["target_id"] == MATERIALIZER.TARGET_ID
    assert packet["successor"]["before_decision"] == "withheld"
    assert packet["benchmark"]["evidence_level"] == "real_correction_case"
    assert packet["benchmark"]["result_before_decision"] is None


def test_task_has_no_vocabulary_for_a_vela_runner_or_authority_action() -> None:
    source = "\n".join(
        path.read_text()
        for path in (ROOT / "benchmarks/erdos-264-proof-repair").rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    )
    assert "Canopus" not in source
    assert "LangGraph" not in source
    assert "review accept" not in source
    assert "review reject" not in source
    assert 'n-attempts", "2' not in source


def test_retained_study_reports_the_null_result_without_harbor_rewards() -> None:
    result = json.loads(STUDY_RESULT.read_text())
    expected_root = result["result_root"]
    result["result_root"] = ""
    canonical = json.dumps(
        result, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode() + b"\n"
    assert expected_root == (
        "sha256:d305b445f5a218b9abbd36a6b140b3da1926f970e32c14f19fc7635fbb42e7e9"
    )
    assert expected_root == "sha256:" + hashlib.sha256(canonical).hexdigest()
    assert result["comparison"]["exact_pass_at_1"] == {
        "git_files": 0,
        "vela_guided": 0,
    }
    assert result["harbor"]["completed_trials"] == 2
    assert result["harbor"]["errored_trials"] == 2
    assert result["harbor"]["recorded_rewards"] == 0
    assert all(not arm["registered_exact_pass"] for arm in result["arms"])
    assert result["claim_credit"] is False
    assert result["post_study_repair"]["verification"]["accepted_event_delta"] == 0
    assert result["post_study_repair"]["decision_packet"]["protocol_gate"] == (
        "satisfied"
    )
    assert result["authority"] == "non_authoritative"
