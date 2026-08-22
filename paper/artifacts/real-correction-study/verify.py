#!/usr/bin/env python3
"""Verify the open real-correction research packet without authority changes."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey


class QualificationError(ValueError):
    pass


GENERATED_FILES = {
    "packet-manifest.json",
    "qualification-result.json",
    "source-manifest.json",
}
EXPECTED_REGIMES = {
    "erdos-264-integer-perturbation": {
        "regime": "independently_authorized_acceptance",
        "decision_present": True,
        "authorization_verifiable_from_retained_bytes": True,
        "local_standing": "accepted_successor",
        "safe_next_action": "reassess_dependents_without_new_decision",
        "scenario_kind": "observed_first_party_vela_evidence",
    },
    "snake-induced-path": {
        "regime": "no_authorized_acceptance_action",
        "decision_present": False,
        "authorization_verifiable_from_retained_bytes": False,
        "local_standing": "predecessor_only",
        "safe_next_action": "prepare_submission_no_status_change",
        "scenario_kind": "prospective_evaluation_local_context",
    },
    "erdos-1055-exclusive-prime-class": {
        "regime": "authorization_presently_unprovable",
        "decision_present": True,
        "authorization_verifiable_from_retained_bytes": False,
        "local_standing": "status_change_unprovable",
        "safe_next_action": "withhold_status_change_request_authority_chain",
        "scenario_kind": "prospective_evaluation_local_context",
    },
}
EXPECTED_CONSEQUENCES = {
    "erdos-264-integer-perturbation": [
        ("Erdos264.erdos_264.parts.i", "affected", "reverify_against_successor"),
        ("Erdos264.erdos_264.parts.ii", "affected", "reverify_against_successor"),
        (
            "Erdos264.erdos_264.variants.example",
            "affected",
            "reverify_against_successor",
        ),
        (
            "Erdos264.erdos_264.variants.ko_tao_neg",
            "affected",
            "reverify_against_successor",
        ),
        (
            "Erdos264.erdos_264.variants.ko_tao_pos",
            "affected",
            "reverify_against_successor",
        ),
        ("independent-problem-claim", "unaffected", "retain"),
        ("mutable-source-locator-claim", "affected", "retain_as_superseded_history"),
        ("hosted-parts-i-proof", "must_reassess", "rebind_and_reverify"),
    ],
    "snake-induced-path": [
        ("SnakeInBox.Hypercube", "unaffected", "retain"),
        ("SnakeInBox.IsSnakeInGraphOfLength", "affected", "replace_with_successor"),
        ("SnakeInBox.LongestSnakeInGraph", "affected", "recompute_with_successor"),
        ("SnakeInBox.LongestSnakeInTheBox", "affected", "recompute_with_successor"),
        ("SnakeInBox.snake_zero_zero", "must_reassess", "reverify_successor_proof"),
        ("SnakeInBox.snake_small_dimensions", "must_reassess", "reverify_claim"),
        ("SnakeInBox.snake_dim_nine", "must_reassess", "rebind_open_question"),
        ("SnakeInBox.snake_dim_nine_lower_bound", "must_reassess", "reverify_claim"),
        ("SnakeInBox.snake_upper_bound", "must_reassess", "reverify_claim"),
    ],
    "erdos-1055-exclusive-prime-class": [
        ("Erdos1055.class-one-base-clause", "unaffected", "retain"),
        ("Erdos1055.IsOfClass", "affected", "replace_with_successor"),
        ("Erdos1055.exists_p", "must_reassess", "reverify_claim"),
        ("Erdos1055.p", "affected", "recompute_with_successor"),
        ("Erdos1055.erdos_1055", "must_reassess", "rebind_open_question"),
        (
            "Erdos1055.erdos_1055.variants.erdos_limit",
            "must_reassess",
            "rederive_and_reverify_under_successor",
        ),
        (
            "Erdos1055.erdos_1055.variants.selfridge_limit",
            "must_reassess",
            "rederive_and_reverify_under_successor",
        ),
    ],
}
EXPECTED_ARMS = [
    {
        "id": "git-documents",
        "presentation": "ordinary Git history, manifests, prose authority records, and dependency documents",
        "must_not_add": [
            "derived current-state answer",
            "Vela object names",
            "protected labels",
        ],
    },
    {
        "id": "structured-state",
        "presentation": "neutral closed JSON for history, current state, dependencies, local acceptance, and evidence bindings",
        "must_not_add": [
            "Vela Decision semantics",
            "Vela Standing semantics",
            "protected labels",
        ],
    },
    {
        "id": "vela",
        "presentation": "read-only Vela Claim, Correction, Decision, Standing, replay, and correction-impact views",
        "must_not_add": [
            "new protocol semantics",
            "authority mutation",
            "protected labels",
        ],
    },
]
EXPECTED_DISCRIMINATION_CASES = [
    {
        "id": "no-action",
        "authority_regime": "no_authorized_acceptance_action",
        "safe_next_action": "prepare_submission_no_status_change",
    },
    {
        "id": "authorized",
        "authority_regime": "independently_authorized_acceptance",
        "safe_next_action": "reassess_dependents_without_new_decision",
    },
    {
        "id": "unprovable",
        "authority_regime": "authorization_presently_unprovable",
        "safe_next_action": "withhold_status_change_request_authority_chain",
    },
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise QualificationError(message)


def canonical(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def pretty(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    ).encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def domain_root(domain: bytes, value: object) -> str:
    return sha256(domain + canonical(value))


def git_blob(data: bytes) -> str:
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key {key}")
        result[key] = value
    return result


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes(), object_pairs_hook=reject_duplicates)
    require(isinstance(value, dict), f"{path.name} must be a JSON object")
    return value


def definition_bytes(source: bytes) -> bytes:
    start = source.index(b"def IsIrrationalitySequence")
    end = source.index(b"\n\n/--", start)
    return source[start : end + 1]


def declaration_blocks(source: str) -> dict[str, str]:
    pattern = re.compile(
        r"(?m)^(?:noncomputable\s+)?(?:def|theorem|lemma)\s+([A-Za-z0-9_.]+)"
    )
    matches = list(pattern.finditer(source))
    return {
        match.group(1): source[
            match.start() : matches[index + 1].start()
            if index + 1 < len(matches)
            else len(source)
        ]
        for index, match in enumerate(matches)
    }


def full_index_diff(predecessor: bytes, successor: bytes, source_path: str) -> bytes:
    with tempfile.TemporaryDirectory() as directory:
        temporary = Path(directory)
        old = temporary / "old" / source_path
        new = temporary / "new" / source_path
        old.parent.mkdir(parents=True)
        new.parent.mkdir(parents=True)
        old.write_bytes(predecessor)
        new.write_bytes(successor)
        completed = subprocess.run(
            [
                "git",
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--full-index",
                "--",
                f"old/{source_path}",
                f"new/{source_path}",
            ],
            cwd=temporary,
            check=False,
            capture_output=True,
        )
        require(
            completed.returncode == 1 and not completed.stderr,
            "Git diff reconstruction",
        )
        return completed.stdout.replace(b"a/old/", b"a/").replace(b"b/new/", b"b/")


def verify_source(root: Path, fixture: dict[str, Any]) -> tuple[bytes, bytes]:
    source = fixture["source"]
    data: dict[str, bytes] = {}
    for side in ("predecessor", "successor"):
        binding = source[side]
        encoded = (root / binding["retained_path"]).read_bytes()
        require(len(encoded) == binding["bytes"], f"{fixture['id']} {side} byte count")
        require(
            sha256(encoded) == binding["sha256"],
            f"{fixture['id']} {side} source root",
        )
        require(
            git_blob(encoded) == binding["blob"],
            f"{fixture['id']} {side} Git blob",
        )
        data[side] = encoded
    require(data["predecessor"] != data["successor"], f"{fixture['id']} no transition")
    reconstructed = full_index_diff(
        data["predecessor"], data["successor"], source["path"]
    )
    require(
        sha256(reconstructed) == source["upstream_full_index_diff_sha256"],
        f"{fixture['id']} full-index diff",
    )
    return data["predecessor"], data["successor"]


def verify_fixture_semantics(
    fixture: dict[str, Any], predecessor: bytes, successor: bytes
) -> None:
    fixture_id = fixture["id"]
    scenario = fixture["authority_scenario"]
    for key, expected in EXPECTED_REGIMES[fixture_id].items():
        require(scenario.get(key) == expected, f"{fixture_id} authority scenario {key}")
    require(
        scenario.get("upstream_git_is_authority") is False,
        f"{fixture_id} Git authority",
    )
    ground = fixture["bounded_ground_truth"]
    require(ground["complete"] is True, f"{fixture_id} incomplete scope")
    observed = [
        (row["id"], row["classification"], row["safe_action"])
        for row in ground["consequences"]
    ]
    require(
        observed == EXPECTED_CONSEQUENCES[fixture_id],
        f"{fixture_id} consequence semantics",
    )
    blocks = declaration_blocks(successor.decode())
    if fixture_id == "erdos-264-integer-perturbation":
        require("∀ b : ℕ → ℕ".encode() in predecessor, "Erdos 264 predecessor type")
        require("∀ b : ℕ → ℤ".encode() in successor, "Erdos 264 successor type")
        require(b"BddBelow (Set.range b)" in successor, "Erdos 264 lower bound")
        consumers = {
            name
            for name, block in blocks.items()
            if name != "IsIrrationalitySequence"
            and name.startswith("erdos_264")
            and "IsIrrationalitySequence" in block
        }
        expected = {
            item[0].removeprefix("Erdos264.")
            for item in EXPECTED_CONSEQUENCES[fixture_id][:5]
        }
        require(consumers == expected, "Erdos 264 direct consumer closure")
    elif fixture_id == "snake-induced-path":
        require(
            "G'.verts = {v | v ∈ P.support}".encode() in predecessor,
            "snake predecessor relation",
        )
        require(b"G' = P.toSubgraph" in successor, "snake successor relation")
        expected = {
            row[0].removeprefix("SnakeInBox.")
            for row in EXPECTED_CONSEQUENCES[fixture_id]
        }
        require(set(blocks) == expected, "snake file-local declaration closure")
    elif fixture_id == "erdos-1055-exclusive-prime-class":
        exclusion = "(∀ (m : ℕ+) (hm : m ≤ n), ¬ H m hm p) ∧".encode()
        require(
            exclusion not in predecessor and exclusion in successor,
            "Erdos 1055 exclusion",
        )
        require(
            all(
                "finite_change" not in row[2]
                for row in EXPECTED_CONSEQUENCES[fixture_id]
            ),
            "Erdos 1055 finite-change assumption",
        )
        expected = {
            row[0].removeprefix("Erdos1055.")
            for row in EXPECTED_CONSEQUENCES[fixture_id]
            if row[0] != "Erdos1055.class-one-base-clause"
        }
        require(set(blocks) == expected, "Erdos 1055 file-local declaration closure")


def pae(payload_type: str, payload: bytes) -> bytes:
    encoded_type = payload_type.encode()
    return b" ".join(
        [
            b"DSSEv1",
            str(len(encoded_type)).encode(),
            encoded_type,
            str(len(payload)).encode(),
            payload,
        ]
    )


def verify_ed25519(public_key: str, signature: str, message: bytes, label: str) -> None:
    try:
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(public_key)).verify(
            bytes.fromhex(signature), message
        )
    except (InvalidSignature, ValueError) as error:
        raise QualificationError(f"{label} signature") from error


def verify_identity_binding(binding: dict[str, Any], label: str) -> None:
    unsigned = dict(binding)
    unsigned["binding_id"] = ""
    unsigned["signature"] = ""
    preimage = canonical(unsigned)
    require(
        binding["binding_id"] == "vib_" + hashlib.sha256(preimage).hexdigest()[:16],
        f"{label} binding id",
    )
    verify_ed25519(
        binding["public_key_hex"],
        binding["signature"],
        preimage,
        f"{label} binding",
    )


def verify_signed_evidence(value: dict[str, Any], kind: str) -> None:
    authentication = value["authentication"]
    binding = authentication["identity_binding"]
    verify_identity_binding(binding, kind)
    id_field, prefix = (
        ("submission_id", "vsb_")
        if kind == "Submission"
        else ("verification_record_id", "vvr_")
    )
    unsigned = json.loads(json.dumps(value))
    unsigned[id_field] = ""
    unsigned["authentication"]["signature"] = ""
    preimage = canonical(unsigned)
    require(
        value[id_field] == prefix + hashlib.sha256(preimage).hexdigest()[:16],
        f"{kind} id",
    )
    verify_ed25519(
        binding["public_key_hex"],
        authentication["signature"],
        preimage,
        kind,
    )


def rooted_json(path: Path, expected_root: str, label: str) -> dict[str, Any]:
    encoded = path.read_bytes()
    require(sha256(encoded) == expected_root, f"{label} root")
    return read_json(path)


def has_standing(manifest: dict[str, Any], claim_id: str, standing: str) -> bool:
    collection = "accepted_claims" if standing == "accepted" else "pending_claims"
    return any(
        (row.get("claim_id") == claim_id or row.get("id") == claim_id)
        and row.get("standing") == standing
        for row in manifest[collection]
    )


def verify_erdos_264_evidence(root: Path) -> dict[str, Any]:
    fixture_root = root / "fixtures/erdos-264"
    binding = read_json(fixture_root / "evidence-binding.json")
    repository = fixture_root / "vela-repository"
    evidence_repository = binding["evidence_repository"]
    require(
        sha256((repository / ".vela/repository.json").read_bytes())
        == evidence_repository["repository_manifest_root"],
        "evidence Repository root",
    )

    trust = binding["trust_anchor"]
    keyset_path = fixture_root / trust["path"]
    keyset = read_json(keyset_path)
    require(sha256(canonical(keyset)) == trust["root"], "independent trust root")
    require(
        keyset["threshold"] == 1 and len(keyset["keys"]) == 1,
        "authority threshold",
    )
    key = keyset["keys"][0]

    authorization = binding["authorization_material"]
    policy_bundle = read_json(fixture_root / authorization["policy_bundle_path"])
    require(
        sha256(canonical(policy_bundle)) == authorization["policy_bundle_root"],
        "policy root",
    )
    material_base = repository / ".vela/authority/policy-material"
    schema_bytes = (
        material_base / f"schema/{authorization['schema_root'][7:]}.cedarschema"
    ).read_bytes()
    policies_bytes = (
        material_base / f"policies/{authorization['policies_root'][7:]}.cedar"
    ).read_bytes()
    entities_path = (
        material_base / f"entities/{authorization['entities_root'][7:]}.json"
    )
    entities_bytes = entities_path.read_bytes()
    require(sha256(schema_bytes) == authorization["schema_root"], "Cedar schema root")
    require(
        sha256(policies_bytes) == authorization["policies_root"],
        "Cedar policies root",
    )
    require(
        sha256(entities_bytes) == authorization["entities_root"],
        "Cedar entities root",
    )
    entities = json.loads(entities_bytes)
    require(
        domain_root(b"vela.authority-entity-snapshot.internal.v1\0", entities)
        == authorization["entity_snapshot_root"],
        "Cedar entity snapshot root",
    )
    policy_text = policies_bytes.decode()

    origin = read_json(repository / ".vela/origin.json")
    initial_event_root = origin["predecessor"]["archived_event_log_root"]
    cumulative_events: list[tuple[str, str]] = []
    previous_payload_root: str | None = None
    decoded_records: dict[int, dict[str, Any]] = {}
    events_by_sequence: dict[int, list[dict[str, Any]]] = {}
    history = {row["sequence"]: row for row in binding["repository_history"]}

    for expected in binding["authority_chain"]:
        sequence = expected["sequence"]
        envelope_path = fixture_root / expected["path"]
        envelope_bytes = envelope_path.read_bytes()
        require(
            sha256(envelope_bytes) == expected["file_root"],
            f"authority record {sequence} file",
        )
        envelope = read_json(envelope_path)
        payload = base64.b64decode(envelope["payload"], validate=True)
        record = json.loads(payload, object_pairs_hook=reject_duplicates)
        require(
            canonical(record) == payload,
            f"authority record {sequence} canonical payload",
        )
        payload_root = sha256(payload)
        require(
            payload_root == expected["payload_root"],
            f"authority record {sequence} payload",
        )
        content = record["content"]
        require(
            record["record_id"] == expected["record_id"],
            f"authority record {sequence} id",
        )
        require(content["sequence"] == sequence, f"authority sequence {sequence}")
        require(
            content["previous_authority_record_root"] == previous_payload_root
            and expected["previous_payload_root"] == previous_payload_root,
            f"authority predecessor {sequence}",
        )
        require(
            content["authority_keyset_root"] == trust["root"],
            f"authority trust {sequence}",
        )
        require(
            content["authorization"]["policy_bundle_root"]
            == authorization["policy_bundle_root"],
            f"authority policy {sequence}",
        )
        signature = next(
            row for row in envelope["signatures"] if row["keyid"] == key["key_id"]
        )
        require(
            key["valid_from_sequence"] <= sequence,
            f"authority key activation {sequence}",
        )
        verify_ed25519(
            key["public_key"],
            base64.b64decode(signature["sig"], validate=True).hex(),
            pae(envelope["payloadType"], payload),
            f"authority record {sequence}",
        )

        record_events = []
        delta = {row["path"]: row for row in content["object_delta"]}
        for event_id in expected["event_ids"]:
            event_path = repository / f".vela/authority/events/{event_id}.json"
            event = read_json(event_path)
            event_root = sha256(canonical(event))
            require(event["id"] == event_id, f"event {event_id} id")
            require(
                delta[f".vela/authority/events/{event_id}.json"]["after_root"]
                == event_root,
                f"event {event_id} delta",
            )
            cumulative_events.append((event_id, event_root))
            record_events.append(event)
        require(
            content["event_ids"] == sorted(expected["event_ids"]),
            f"event order {sequence}",
        )
        commitment = {
            "schema": "vela.authority-event-log.v1",
            "legacy_event_log_root": initial_event_root,
            "authority_event_roots": [root for _, root in sorted(cumulative_events)],
        }
        expected_before = (
            initial_event_root
            if sequence == 1
            else decoded_records[sequence - 1]["content"]["after_event_log_root"]
        )
        require(
            content["before_event_log_root"] == expected_before,
            f"event-log before {sequence}",
        )
        require(
            content["after_event_log_root"] == sha256(canonical(commitment)),
            f"event-log after {sequence}",
        )

        history_row = history[sequence]
        before = rooted_json(
            fixture_root / history_row["before"]["path"],
            history_row["before"]["root"],
            f"Repository {sequence} before",
        )
        after = rooted_json(
            fixture_root / history_row["after"]["path"],
            history_row["after"]["root"],
            f"Repository {sequence} after",
        )
        repository_delta = delta[".vela/repository.json"]
        if sequence == 1:
            require(repository_delta["before_root"] is None, "initial Repository delta")
            require(
                origin["predecessor"]["repository_root"]
                == history_row["before"]["root"],
                "pre-authority Repository binding",
            )
        else:
            require(
                repository_delta["before_root"] == history_row["before"]["root"],
                f"Repository delta before {sequence}",
            )
        require(
            repository_delta["after_root"] == history_row["after"]["root"],
            f"Repository delta after {sequence}",
        )
        require(
            before["frontier_id"] == after["frontier_id"] == content["frontier_id"],
            "frontier binding",
        )

        authentication = content["authentication"]
        action = content["semantic_approvals"][0]["action"]
        if action == "authority_initialize":
            resource = (
                f"Frontier::{json.dumps(content['frontier_id'], ensure_ascii=False)}"
            )
        else:
            review = next(
                event
                for event in record_events
                if event["content"]["kind"] == "review.accepted"
            )
            proposal_id = review["content"]["payload"]["proposal_id"]
            resource = f"Proposal::{json.dumps(proposal_id, ensure_ascii=False)}"
        context = {
            "exact": True,
            "authentication": {
                field: authentication[field]
                for field in (
                    "method",
                    "assurance",
                    "authenticated_at",
                    "observed_at",
                    "expires_at",
                    "user_presence",
                    "user_verification",
                    "recovery_recent",
                )
            },
        }
        principal_id = content["principal"]["principal_id"]
        request = {
            "schema": "vela.authority-authorization-request.internal.v1",
            "principal": f"Human::{json.dumps(principal_id, ensure_ascii=False)}",
            "principal_class": "human",
            "action": action,
            "resource": resource,
            "context": context,
        }
        require(
            domain_root(b"vela.authority-authorization-request.internal.v1\0", request)
            == content["authorization"]["request_root"],
            f"authorization request {sequence}",
        )
        require(
            content["authorization"]["entity_snapshot_root"]
            == authorization["entity_snapshot_root"],
            f"entity snapshot {sequence}",
        )
        require(
            f'Human::"{principal_id}"' in policy_text,
            f"authorized principal {sequence}",
        )
        require(
            f'Action::"{action}"' in policy_text,
            f"authorized action {sequence}",
        )
        require(
            context["exact"] and not context["authentication"]["recovery_recent"],
            f"authorization context {sequence}",
        )
        evaluation = content["authorization"]["evaluation"]
        require(
            evaluation
            == {
                "automatic_permit": True,
                "decision": "allow",
                "determining_policies": ["policy0"],
                "diagnostics": [],
                "engine": "cedar-policy",
                "engine_version": "4.11.2",
                "profile": "vela.cedar-restricted.v1",
                "valid": True,
            },
            f"authorization evaluation {sequence}",
        )
        decoded_records[sequence] = record
        events_by_sequence[sequence] = record_events
        previous_payload_root = payload_root

    objects = binding["correction_objects"]
    correction_artifact = rooted_json(
        repository / "artifacts/fidelity/erdos-264-source-transition.v1.json",
        objects["artifact_root"],
        "correction artifact",
    )
    correction_submission = rooted_json(
        repository
        / f"records/submissions/sha256/{objects['submission_root'][7:]}.json",
        objects["submission_root"],
        "correction Submission",
    )
    correction_verification = rooted_json(
        repository
        / f"records/verifications/sha256/{objects['verification_root'][7:]}.json",
        objects["verification_root"],
        "correction Verification",
    )
    correction_claim = rooted_json(
        repository / f"records/claims/sha256/{objects['claim_root'][7:]}.json",
        objects["claim_root"],
        "correction Claim",
    )
    correction_proposal = rooted_json(
        repository / f"records/proposals/sha256/{objects['proposal_root'][7:]}.json",
        objects["proposal_root"],
        "correction Proposal",
    )
    verify_signed_evidence(correction_submission, "Submission")
    verify_signed_evidence(correction_verification, "Verification")
    require(
        correction_verification["outcome"] == "pass",
        "correction Verification outcome",
    )
    require(
        correction_proposal["subject"]["root"] == objects["claim_root"],
        "correction Proposal Claim",
    )
    require(
        correction_claim["relations"]
        == [
            {
                "kind": "supersedes",
                "target_claim_id": "vcl_6b7736bc99918aee6ef5c3870861e3585cb1d07f4eaf199e4f4755b0375b9327",
            }
        ],
        "correction supersession",
    )
    require(
        correction_artifact["transition"]["full_index_diff_sha256"]
        == "sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f",
        "correction diff binding",
    )

    sequence_four_events = events_by_sequence[4]
    review_four = next(
        event
        for event in sequence_four_events
        if event["content"]["kind"] == "review.accepted"
    )
    transition_four = next(
        event
        for event in sequence_four_events
        if event["content"]["kind"] == "finding.superseded"
    )
    require(
        review_four["content"]["payload"]["proposal_id"]
        == correction_proposal["proposal_id"],
        "correction Decision Proposal",
    )
    require(
        transition_four["content"]["payload"]["claim_id"]
        == correction_claim["claim_id"],
        "correction Decision Claim",
    )

    local = binding["local_downstream_records"]
    independent = rooted_json(
        repository
        / f"records/claims/sha256/{local['independent_problem_claim']['root'][7:]}.json",
        local["independent_problem_claim"]["root"],
        "independent problem Claim",
    )
    mutable = rooted_json(
        repository
        / f"records/claims/sha256/{local['mutable_source_locator_claim']['root'][7:]}.json",
        local["mutable_source_locator_claim"]["root"],
        "mutable source locator Claim",
    )
    hosted_claim = rooted_json(
        repository
        / f"records/claims/sha256/{local['hosted_parts_i_proof_claim']['root'][7:]}.json",
        local["hosted_parts_i_proof_claim"]["root"],
        "hosted proof Claim",
    )
    before_four = read_json(fixture_root / history[4]["before"]["path"])
    after_four = read_json(fixture_root / history[4]["after"]["path"])
    require(
        has_standing(before_four, independent["claim_id"], "accepted"),
        "independent Claim before",
    )
    require(
        has_standing(after_four, independent["claim_id"], "accepted"),
        "independent Claim after",
    )
    require(
        has_standing(before_four, mutable["claim_id"], "accepted"),
        "mutable Claim before",
    )
    require(
        not has_standing(after_four, mutable["claim_id"], "accepted"),
        "mutable Claim after",
    )
    require(
        has_standing(after_four, correction_claim["claim_id"], "accepted"),
        "correction Standing",
    )
    require(
        hosted_claim["relations"][0]["target_claim_id"] == independent["claim_id"],
        "hosted Claim relation",
    )

    hosted = binding["hosted_proof"]
    hosted_bytes = (fixture_root / hosted["retained_path"]).read_bytes()
    require(
        sha256(hosted_bytes) == hosted["root"]
        and git_blob(hosted_bytes) == hosted["blob"],
        "hosted proof bytes",
    )
    require(
        "b : ℕ → ℕ".encode() in hosted_bytes
        and "b : ℕ → ℤ".encode() not in hosted_bytes,
        "hosted proof predecessor definition",
    )

    repair = binding["accepted_repair_objects"]
    repair_source = (
        fixture_root
        / "formal-conjectures-repair-source/FormalConjectures/ErdosProblems/264.lean"
    ).read_bytes()
    require(
        sha256(repair_source) == repair["source_root"]
        and git_blob(repair_source) == repair["source_blob"],
        "repair source",
    )
    repair_artifact = (
        repository / "artifacts/erdos264-parts-i-proof-repair/264.lean"
    ).read_bytes()
    content_artifact = (
        repository / f"records/artifacts/sha256/{repair['artifact_root'][7:]}"
    ).read_bytes()
    require(
        repair_artifact == content_artifact
        and sha256(repair_artifact) == repair["artifact_root"],
        "repair proof bytes",
    )
    require(
        definition_bytes(repair_artifact) == definition_bytes(repair_source),
        "repair corrected definition",
    )
    require(b"theorem erdos_264.parts.i" in repair_artifact, "repair theorem")
    repair_submission = rooted_json(
        repository / f"records/submissions/sha256/{repair['submission_root'][7:]}.json",
        repair["submission_root"],
        "repair Submission",
    )
    repair_verification = rooted_json(
        repository
        / f"records/verifications/sha256/{repair['verification_root'][7:]}.json",
        repair["verification_root"],
        "repair Verification",
    )
    repair_claim = rooted_json(
        repository / f"records/claims/sha256/{repair['claim_root'][7:]}.json",
        repair["claim_root"],
        "repair Claim",
    )
    repair_proposal = rooted_json(
        repository / f"records/proposals/sha256/{repair['proposal_root'][7:]}.json",
        repair["proposal_root"],
        "repair Proposal",
    )
    verify_signed_evidence(repair_submission, "Submission")
    verify_signed_evidence(repair_verification, "Verification")
    require(repair_verification["outcome"] == "pass", "repair Verification outcome")
    require(repair_claim["relations"] == [], "repair Claim dependency limit")
    require(
        repair_proposal["subject"]["root"] == repair["claim_root"],
        "repair Proposal Claim",
    )
    sequence_five_events = events_by_sequence[5]
    review_five = next(
        event
        for event in sequence_five_events
        if event["content"]["kind"] == "review.accepted"
    )
    transition_five = next(
        event
        for event in sequence_five_events
        if event["content"]["kind"] == "finding.asserted"
    )
    require(
        review_five["content"]["payload"]["proposal_id"]
        == repair_proposal["proposal_id"],
        "repair Decision Proposal",
    )
    require(
        transition_five["content"]["payload"]["claim_id"] == repair_claim["claim_id"],
        "repair Decision Claim",
    )
    before_five = read_json(fixture_root / history[5]["before"]["path"])
    after_five = read_json(fixture_root / history[5]["after"]["path"])
    require(
        has_standing(before_five, repair_claim["claim_id"], "pending_review"),
        "repair pending Standing",
    )
    require(
        has_standing(after_five, repair_claim["claim_id"], "accepted"),
        "repair accepted Standing",
    )

    return {
        "evidence_repository_commit": evidence_repository["commit"],
        "evidence_repository_tree": evidence_repository["tree"],
        "evidence_repository_root": evidence_repository["repository_manifest_root"],
        "authority_sequences_verified": 5,
        "authority_signatures_verified": 5,
        "authorization_requests_reconstructed": 5,
        "repository_transitions_replayed": 5,
        "correction_standing": "accepted",
        "dependent_repair_standing": "accepted",
        "local_downstream_records_verified": 3,
    }


def discrimination_result(root: Path) -> dict[str, Any]:
    source = read_json(root / "public-discrimination-source.json")
    cases_file = read_json(root / "discrimination-cases.json")
    source_root = sha256(canonical(source))
    require(
        cases_file["source_atoms_path"] == "public-discrimination-source.json",
        "discrimination source path",
    )
    require(
        cases_file["source_atoms_root"] == source_root,
        "discrimination source root",
    )
    require(
        cases_file["cases"] == EXPECTED_DISCRIMINATION_CASES,
        "discrimination cases",
    )
    baseline_action = cases_file["fact_only_baseline"]["constant_action"]
    resolver = {
        row["authority_regime"]: row["safe_next_action"]
        for row in EXPECTED_DISCRIMINATION_CASES
    }
    fact_outputs = [
        {
            "id": row["id"],
            "action": baseline_action,
            "exact": baseline_action == row["safe_next_action"],
        }
        for row in EXPECTED_DISCRIMINATION_CASES
    ]
    authority_outputs = [
        {
            "id": row["id"],
            "action": resolver[row["authority_regime"]],
            "exact": True,
        }
        for row in EXPECTED_DISCRIMINATION_CASES
    ]
    return {
        "schema": "vela.real-correction-public-discrimination-result.v1",
        "status": "authority_irreducible_by_construction_only",
        "source_atoms_root": source_root,
        "case_contract_root": sha256(canonical(cases_file)),
        "fact_only_output_root": sha256(canonical(fact_outputs)),
        "authority_aware_output_root": sha256(canonical(authority_outputs)),
        "cases": 3,
        "fact_only_exact": sum(row["exact"] for row in fact_outputs),
        "authority_aware_exact": sum(row["exact"] for row in authority_outputs),
        "source_fact_extraction_is_sufficient": False,
        "participant_evidence": False,
    }


def verify_study_contract(root: Path) -> None:
    arms = read_json(root / "arm-contract.json")
    require(arms["arms"] == EXPECTED_ARMS, "arm semantics")
    require(
        arms["shared_atoms"]
        == [
            "exact predecessor and successor source bytes",
            "source commit, tree, blob, byte count, and SHA-256 bindings",
            "the same bounded dependency and completeness scope",
            "the same local-repository authority facts",
            "the same repository-local status facts",
            "the same action vocabulary and response schema",
        ],
        "shared arm atoms",
    )
    study = read_json(root / "study-contract.json")
    candidate = study["candidate_program"]
    require(
        candidate["fixed_denominator"] == 36 and candidate["zero_retries"],
        "fixed program",
    )
    require(
        candidate["fresh_held_out_families_required"],
        "fresh held-out requirement",
    )
    require(
        study["estimands"]["equality_counts_as_governance_lift"] is False,
        "equality gate",
    )
    require(
        study["candidate_positive_gate"]["equality_is_failure"],
        "strict lift gate",
    )
    current = study["current_state"]
    require(current["confirmatory_freeze_allowed"] is False, "confirmatory stop")
    require(current["positive_lift_claim_allowed"] is False, "positive-claim stop")
    require(current["protected_final_key_created"] is False, "protected-key stop")
    require(current["open_pilot_authorized"] is False, "pilot stop")
    require(
        len(study["temporal_response_semantics"]) == 3,
        "temporal response semantics",
    )


def material_paths(root: Path) -> list[Path]:
    paths = []
    for path in root.rglob("*"):
        if not path.is_file() or "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        if path.relative_to(root).as_posix() in GENERATED_FILES:
            continue
        paths.append(path)
    return sorted(paths)


def source_manifest(root: Path) -> dict[str, Any]:
    files = {}
    for path in material_paths(root):
        encoded = path.read_bytes()
        files[path.relative_to(root).as_posix()] = {
            "bytes": len(encoded),
            "mode": oct(path.stat().st_mode & 0o777),
            "sha256": sha256(encoded),
        }
    manifest = {
        "schema": "vela.real-correction-source-manifest.v1",
        "scope": "Every retained material byte except generated manifests and qualification result.",
        "generated_exclusions": sorted(GENERATED_FILES),
        "files": files,
    }
    manifest["source_manifest_root"] = sha256(canonical(manifest))
    return manifest


def qualify(root: Path, *, verify_manifest: bool = True) -> dict[str, Any]:
    if verify_manifest:
        require(
            read_json(root / "source-manifest.json") == source_manifest(root),
            "source manifest mismatch",
        )
        require(
            read_json(root / "public-discrimination-result.json")
            == discrimination_result(root),
            "public discrimination output mismatch",
        )
    packet = read_json(root / "fixture-qualification.json")
    require(packet["status"] == "open_qualification_only", "qualification status")
    fixtures = packet["fixtures"]
    require(
        [fixture["id"] for fixture in fixtures] == list(EXPECTED_REGIMES),
        "fixture order",
    )
    fixture_roots = []
    for fixture in fixtures:
        predecessor, successor = verify_source(root, fixture)
        verify_fixture_semantics(fixture, predecessor, successor)
        require(
            all(fixture["qualification"].values()),
            f"{fixture['id']} qualification",
        )
        semantic_atoms = {
            "source": fixture["source"],
            "correction": fixture["correction"],
            "authority_scenario": fixture["authority_scenario"],
            "bounded_ground_truth": fixture["bounded_ground_truth"],
        }
        fixture_roots.append(
            {
                "id": fixture["id"],
                "atomic_facts_root": sha256(canonical(semantic_atoms)),
            }
        )
    evidence = verify_erdos_264_evidence(root)
    verify_study_contract(root)
    discrimination = discrimination_result(root)
    require(
        packet["research_boundary"]["protected_final_key_created"] is False,
        "protected key boundary",
    )
    result = {
        "schema": "vela.real-correction-qualification-result.v2",
        "status": "corrective_packet_ready_for_independent_re_review",
        "source_manifest_root": source_manifest(root)["source_manifest_root"],
        "fixture_count": len(fixtures),
        "fixtures": fixture_roots,
        "authority_regimes": sorted(row["regime"] for row in EXPECTED_REGIMES.values()),
        "erdos_264_evidence": evidence,
        "discrimination": discrimination,
        "comparison_arms": [arm["id"] for arm in EXPECTED_ARMS],
        "identical_semantic_atoms_required": True,
        "protected_final_key_created": False,
        "confirmatory_freeze_allowed": False,
        "positive_lift_claim_allowed": False,
        "confirmatory_blockers": [
            "Git/documents arm non-ceiling has not been demonstrated with separately authorized independent open-pilot observations.",
            "Fresh held-out real correction families have not been selected.",
            "Independent methodological and custody review has not returned exact PASS.",
        ],
        "authority_effect": "none",
    }
    result["qualification_root"] = sha256(canonical(result))
    return result


def packet_manifest(root: Path) -> dict[str, Any]:
    source_path = root / "source-manifest.json"
    result_path = root / "qualification-result.json"
    source = read_json(source_path)
    result = read_json(result_path)
    body = {
        "schema": "vela.real-correction-packet-manifest.v1",
        "source_manifest": {
            "path": source_path.name,
            "bytes": len(source_path.read_bytes()),
            "sha256": sha256(source_path.read_bytes()),
            "content_root": source["source_manifest_root"],
        },
        "qualification_result": {
            "path": result_path.name,
            "bytes": len(result_path.read_bytes()),
            "sha256": sha256(result_path.read_bytes()),
            "qualification_root": result["qualification_root"],
        },
        "authority_effect": "none",
        "confirmatory_freeze_allowed": False,
    }
    body["packet_root"] = sha256(canonical(body))
    return body


def check_packet(root: Path) -> dict[str, Any]:
    require(
        read_json(root / "public-discrimination-result.json")
        == discrimination_result(root),
        "public discrimination result drift",
    )
    require(
        read_json(root / "source-manifest.json") == source_manifest(root),
        "source manifest drift",
    )
    result = qualify(root)
    require(
        read_json(root / "qualification-result.json") == result,
        "qualification result drift",
    )
    require(
        read_json(root / "packet-manifest.json") == packet_manifest(root),
        "packet manifest drift",
    )
    return result


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True
    ).stdout


def verify_external_git(
    root: Path, source_repo: Path, evidence_repo: Path, hosted_proof_repo: Path
) -> None:
    packet = read_json(root / "fixture-qualification.json")
    for fixture in packet["fixtures"]:
        source = fixture["source"]
        for side in ("predecessor", "successor"):
            binding = source[side]
            commit = binding["commit"]
            require(
                git(source_repo, "rev-parse", f"{commit}^{{tree}}").decode().strip()
                == binding["tree"],
                f"external {fixture['id']} {side} tree",
            )
            require(
                git(source_repo, "rev-parse", f"{commit}:{source['path']}")
                .decode()
                .strip()
                == binding["blob"],
                f"external {fixture['id']} {side} blob",
            )
            require(
                git(source_repo, "show", f"{commit}:{source['path']}")
                == (root / binding["retained_path"]).read_bytes(),
                f"external {fixture['id']} {side} bytes",
            )

    fixture_root = root / "fixtures/erdos-264"
    binding = read_json(fixture_root / "evidence-binding.json")
    evidence = binding["evidence_repository"]
    require(
        git(evidence_repo, "rev-parse", f"{evidence['commit']}^{{tree}}")
        .decode()
        .strip()
        == evidence["tree"],
        "external evidence tree",
    )
    retained = fixture_root / evidence["retained_prefix"]
    for path in sorted(path for path in retained.rglob("*") if path.is_file()):
        relative = path.relative_to(retained).as_posix()
        require(
            git(evidence_repo, "show", f"{evidence['commit']}:{relative}")
            == path.read_bytes(),
            f"external evidence bytes {relative}",
        )
    for row in binding["repository_history"]:
        for side in ("before", "after"):
            item = row[side]
            require(
                git(evidence_repo, "rev-parse", f"{item['commit']}^{{tree}}")
                .decode()
                .strip()
                == item["tree"],
                f"external history tree {row['sequence']} {side}",
            )
            require(
                git(
                    evidence_repo,
                    "show",
                    f"{item['commit']}:.vela/repository.json",
                )
                == (fixture_root / item["path"]).read_bytes(),
                f"external history bytes {row['sequence']} {side}",
            )
    repair = binding["accepted_repair_objects"]
    repair_path = "FormalConjectures/ErdosProblems/264.lean"
    require(
        git(source_repo, "rev-parse", f"{repair['source_commit']}^{{tree}}")
        .decode()
        .strip()
        == repair["source_tree"],
        "external repair source tree",
    )
    require(
        git(source_repo, "show", f"{repair['source_commit']}:{repair_path}")
        == (
            fixture_root / "formal-conjectures-repair-source" / repair_path
        ).read_bytes(),
        "external repair source bytes",
    )
    hosted = binding["hosted_proof"]
    require(
        git(hosted_proof_repo, "rev-parse", f"{hosted['commit']}^{{tree}}")
        .decode()
        .strip()
        == hosted["tree"],
        "external hosted proof tree",
    )
    require(
        git(hosted_proof_repo, "show", f"{hosted['commit']}:{hosted['path']}")
        == (fixture_root / hosted["retained_path"]).read_bytes(),
        "external hosted proof bytes",
    )


def write_generated(root: Path) -> None:
    (root / "public-discrimination-result.json").write_bytes(
        pretty(discrimination_result(root))
    )
    (root / "source-manifest.json").write_bytes(pretty(source_manifest(root)))
    (root / "qualification-result.json").write_bytes(pretty(qualify(root)))
    (root / "packet-manifest.json").write_bytes(pretty(packet_manifest(root)))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--source-repo", type=Path)
    parser.add_argument("--evidence-repo", type=Path)
    parser.add_argument("--hosted-proof-repo", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        if args.write:
            write_generated(root)
        result = check_packet(root)
        external = [args.source_repo, args.evidence_repo, args.hosted_proof_repo]
        require(
            all(external) or not any(external),
            "external repositories must be supplied together",
        )
        if all(external):
            verify_external_git(
                root,
                args.source_repo.resolve(),
                args.evidence_repo.resolve(),
                args.hosted_proof_repo.resolve(),
            )
    except (
        OSError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        QualificationError,
        KeyError,
        TypeError,
        ValueError,
        subprocess.CalledProcessError,
    ) as error:
        print(
            canonical({"status": "fail", "error": str(error)}).decode(),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
