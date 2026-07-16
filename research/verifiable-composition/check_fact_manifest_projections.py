#!/usr/bin/env python3
"""Hostile stdlib-only checks for the ADR 0004 removable projections."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REFERENCE = ROOT / "reference"
sys.path.insert(0, str(REFERENCE))

from fact_manifest import (  # noqa: E402
    FACT_ENVELOPE_SCHEMA,
    FACT_MANIFEST_SCHEMA,
    accepted_context_pack_projection,
    build_envelope,
    canonical_bytes,
    correction_ci_projection,
    fact_manifest_root,
    finding_revision_root,
    resolve_bytes,
    resolve_envelope,
)
from offline_bundle_inspection import (  # noqa: E402
    INSPECTION_RESULT_SCHEMA,
    build_inspection_envelope,
    event_content_root,
    event_log_root,
)
from reader_c import read_bytes as reader_c_read_bytes  # noqa: E402


VECTORS = ROOT / "vectors/fact-manifest-projection-cases.json"
RESOLVER = REFERENCE / "resolve_fact_manifest.py"
CORRECTION_CI = REFERENCE / "correction_ci.py"
CONTEXT_PACK = REFERENCE / "accepted_context_pack.py"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def root(label: str) -> str:
    return f"sha256:{hashlib.sha256(label.encode('utf-8')).hexdigest()}"


def event(label: str, *, effect: str | None = None) -> dict[str, Any]:
    payload: dict[str, Any] = {"label": label}
    if effect is not None:
        payload["dependency_effect"] = effect
    value = {
        "schema": "vela.event.v0.1",
        "id": "",
        "kind": "finding.revised" if effect else "review.accepted",
        "target": {"type": "finding", "id": "vf_1111111111111111"},
        "actor": {"type": "human", "id": "reviewer:fixture"},
        "timestamp": f"2026-07-16T00:00:{len(label):02d}Z",
        "reason": label,
        "before_hash": root(f"{label}:before"),
        "after_hash": root(f"{label}:after"),
        "payload": payload,
        "caveats": ["internal fixture"],
        "signature": "v1:" + hashlib.sha512(label.encode()).hexdigest(),
    }
    preimage = {
        field: value[field]
        for field in (
            "schema",
            "kind",
            "target",
            "actor",
            "timestamp",
            "reason",
            "before_hash",
            "after_hash",
            "payload",
            "caveats",
        )
    }
    value["id"] = f"vev_{root_from_value(preimage)[7:23]}"
    return value


def state(
    commit: str, tree: str, snapshot: dict[str, Any], events: list[dict[str, Any]]
) -> dict[str, Any]:
    return {
        "git_commit": commit,
        "git_tree": tree,
        "event_log_root": event_log_root(events),
        "snapshot_root": root_from_value(snapshot),
    }


def root_from_value(value: Any) -> str:
    return f"sha256:{hashlib.sha256(canonical_bytes(value)).hexdigest()}"


def install_inspection(
    manifest: dict[str, Any],
    *,
    last_commit: str,
    last_tree: str,
    delivered_commit: str,
    delivered_tree: str,
    merge_base: str,
    git_relation: str,
    event_relation: str,
    last_snapshot: dict[str, Any],
    delivered_snapshot: dict[str, Any],
    last_events: list[dict[str, Any]],
    delivered_events: list[dict[str, Any]],
) -> None:
    result = {
        "schema": INSPECTION_RESULT_SCHEMA,
        "verification": "verified",
        "bundle_root": root("offline-bundle"),
        "state_path": "inspection/state.json",
        "last_seen_git_commit": last_commit,
        "last_seen_git_tree": last_tree,
        "delivered_git_commit": delivered_commit,
        "delivered_git_tree": delivered_tree,
        "merge_base": merge_base,
        "git_relation": git_relation,
        "event_relation": event_relation,
        "last_seen_snapshot": copy.deepcopy(last_snapshot),
        "delivered_snapshot": copy.deepcopy(delivered_snapshot),
        "last_seen_events": copy.deepcopy(last_events),
        "delivered_events": copy.deepcopy(delivered_events),
        "last_seen_state_document_root": root_from_value(
            {
                "schema": "vela.verifiable-composition.bundle-state.v0",
                "snapshot": last_snapshot,
                "events": last_events,
            }
        ),
        "delivered_state_document_root": root_from_value(
            {
                "schema": "vela.verifiable-composition.bundle-state.v0",
                "snapshot": delivered_snapshot,
                "events": delivered_events,
            }
        ),
    }
    manifest["delivery_inspection"] = build_inspection_envelope(result)
    manifest["last_seen"] = state(last_commit, last_tree, last_snapshot, last_events)
    manifest["delivered"] = state(
        delivered_commit,
        delivered_tree,
        delivered_snapshot,
        delivered_events,
    )
    dependency = manifest["dependency"]
    dependency["parent_git_commit"] = manifest["last_seen"]["git_commit"]
    dependency["parent_git_tree"] = manifest["last_seen"]["git_tree"]
    dependency["parent_event_log_root"] = manifest["last_seen"]["event_log_root"]
    dependency["parent_snapshot_root"] = manifest["last_seen"]["snapshot_root"]


def base_manifest() -> dict[str, Any]:
    finding = {
        "id": "vf_1111111111111111",
        "version": 1,
        "previous_version": None,
        "assertion": {
            "text": "The exact registered graph is triangle-free.",
            "type": "theoretical",
        },
        "evidence": {
            "type": "computational",
            "method": "independent exact checker",
        },
        "conditions": {"text": "For the content-addressed graph bytes only."},
        "confidence": {"score": 1, "kind": "frontier_epistemic"},
        "provenance": {"source_type": "internal_fixture"},
        "flags": {"retracted": False, "contested": False},
        "links": [
            {
                "type": "depends",
                "target": "vf_2222222222222222",
                "note": "mutable review surface; excluded from the finding root",
            }
        ],
        "annotations": [],
        "created": "2026-07-16T00:00:00Z",
        "updated": None,
    }
    accepted_event = event("accepted")
    dependency = {
        "schema": "vela.experimental-dependency-observation.v0",
        "parent_frontier_id": "vfr_aaaaaaaaaaaaaaaa",
        "parent_git_commit": "1" * 40,
        "parent_git_tree": "2" * 40,
        "parent_event_log_root": root("placeholder-event-log"),
        "parent_snapshot_root": root("placeholder-snapshot"),
        "finding_id": finding["id"],
        "finding_revision_root": finding_revision_root(finding),
        "decision_event_content_root": event_content_root(accepted_event),
        "decision_signature": accepted_event["signature"],
        "authority_id": "reviewer:fixture",
        "receipt_roots": sorted([root("receipt-b"), root("receipt-a")]),
        "verifier_attachments": [
            {
                "attachment_id": "vva_5555555555555555",
                "attachment_content_root": root("attachment-a"),
            },
            {
                "attachment_id": "vva_6666666666666666",
                "attachment_content_root": root("attachment-b"),
            },
        ],
        "premise_digest": root("premise"),
        "role": "hard",
    }
    dependency["decision_event_id"] = (
        f"vev_{dependency['decision_event_content_root'][7:23]}"
    )
    manifest = {
        "schema": FACT_MANIFEST_SCHEMA,
        "dependency": dependency,
        "accepted_finding": finding,
        "last_seen": {},
        "delivered": {},
        "delivery_inspection": {},
        "standing": {
            "selected_finding_revision_root": dependency["finding_revision_root"],
            "decision_event_content_root": dependency["decision_event_content_root"],
            "authority_id": dependency["authority_id"],
            "receipt_roots": copy.deepcopy(dependency["receipt_roots"]),
            "verifier_attachments": copy.deepcopy(dependency["verifier_attachments"]),
            "premise_digest": dependency["premise_digest"],
            "finding_status": "accepted",
            "decision_status": "valid",
            "verifier_status": "valid",
            "evidence_status": "available",
            "change_event": None,
        },
    }
    snapshot = {"accepted": [dependency["finding_revision_root"]], "revision": 1}
    install_inspection(
        manifest,
        last_commit="1" * 40,
        last_tree="2" * 40,
        delivered_commit="1" * 40,
        delivered_tree="2" * 40,
        merge_base="1" * 40,
        git_relation="same",
        event_relation="same",
        last_snapshot=snapshot,
        delivered_snapshot=snapshot,
        last_events=[accepted_event],
        delivered_events=[accepted_event],
    )
    return manifest


def descendant(manifest: dict[str, Any]) -> None:
    result = manifest["delivery_inspection"]["result"]
    install_inspection(
        manifest,
        last_commit=result["last_seen_git_commit"],
        last_tree=result["last_seen_git_tree"],
        delivered_commit="6" * 40,
        delivered_tree="7" * 40,
        merge_base=result["last_seen_git_commit"],
        git_relation="descendant",
        event_relation="same",
        last_snapshot=result["last_seen_snapshot"],
        delivered_snapshot={"accepted": ["unchanged"], "revision": 2},
        last_events=result["last_seen_events"],
        delivered_events=result["last_seen_events"],
    )


def changed(manifest: dict[str, Any], field: str, value: str) -> None:
    effect = (
        value
        if field == "finding_status"
        else "decision_revoked"
        if field == "decision_status"
        else "verifier_revoked"
    )
    change = event(f"change-{effect}", effect=effect)
    result = manifest["delivery_inspection"]["result"]
    install_inspection(
        manifest,
        last_commit=result["last_seen_git_commit"],
        last_tree=result["last_seen_git_tree"],
        delivered_commit="6" * 40,
        delivered_tree="7" * 40,
        merge_base=result["last_seen_git_commit"],
        git_relation="descendant",
        event_relation="descendant",
        last_snapshot=result["last_seen_snapshot"],
        delivered_snapshot={"accepted": ["changed"], "revision": 2},
        last_events=result["last_seen_events"],
        delivered_events=[*result["last_seen_events"], change],
    )
    manifest["standing"][field] = value
    manifest["standing"]["change_event"] = {
        "event_id": change["id"],
        "event_content_root": event_content_root(change),
        "event_signature": change["signature"],
        "authority_id": "reviewer:fixture",
        "effect": effect,
        "inspection_result_root": manifest["delivery_inspection"]["inspection_root"],
    }


def mutate(manifest: dict[str, Any], operation: str) -> bytes:
    if operation == "none":
        pass
    elif operation == "descendant_unchanged":
        descendant(manifest)
    elif operation == "finding_corrected":
        changed(manifest, "finding_status", "corrected")
    elif operation == "finding_superseded":
        changed(manifest, "finding_status", "superseded")
    elif operation == "soft_finding_corrected":
        manifest["dependency"]["role"] = "soft"
        changed(manifest, "finding_status", "corrected")
    elif operation == "contextual_finding_superseded":
        manifest["dependency"]["role"] = "contextual"
        changed(manifest, "finding_status", "superseded")
    elif operation == "hard_withdrawn":
        changed(manifest, "finding_status", "withdrawn")
    elif operation == "soft_withdrawn":
        manifest["dependency"]["role"] = "soft"
        changed(manifest, "finding_status", "withdrawn")
    elif operation == "data_decision_revoked":
        manifest["dependency"]["role"] = "data"
        changed(manifest, "decision_status", "revoked")
    elif operation == "finding_corrected_and_decision_revoked":
        changed(manifest, "finding_status", "corrected")
        manifest["standing"]["decision_status"] = "revoked"
    elif operation == "contextual_verifier_revoked":
        manifest["dependency"]["role"] = "contextual"
        changed(manifest, "verifier_status", "revoked")
    elif operation == "invalid_change_event":
        changed(manifest, "finding_status", "corrected")
        manifest["standing"]["change_event"]["inspection_result_root"] = root(
            "wrong-inspection"
        )
    elif operation == "stale_ancestor":
        accepted = manifest["delivery_inspection"]["result"]["last_seen_events"][0]
        note = event("later-note")
        install_inspection(
            manifest,
            last_commit="8" * 40,
            last_tree="9" * 40,
            delivered_commit="1" * 40,
            delivered_tree="2" * 40,
            merge_base="1" * 40,
            git_relation="ancestor",
            event_relation="ancestor",
            last_snapshot={"accepted": ["newer"], "revision": 2},
            delivered_snapshot={"accepted": ["base"], "revision": 1},
            last_events=[accepted, note],
            delivered_events=[accepted],
        )
    elif operation == "valid_fork":
        accepted = manifest["delivery_inspection"]["result"]["last_seen_events"][0]
        install_inspection(
            manifest,
            last_commit="8" * 40,
            last_tree="9" * 40,
            delivered_commit="a" * 40,
            delivered_tree="b" * 40,
            merge_base="0" * 40,
            git_relation="forked",
            event_relation="forked",
            last_snapshot={"accepted": ["left"], "revision": 2},
            delivered_snapshot={"accepted": ["right"], "revision": 2},
            last_events=[accepted, event("left-note")],
            delivered_events=[accepted, event("right-note")],
        )
    elif operation == "continuity_missing":
        manifest.pop("delivery_inspection")
    elif operation == "continuity_invalid":
        manifest["delivery_inspection"]["result"]["verification"] = "invalid"
        manifest["delivery_inspection"]["inspection_root"] = root_from_value(
            manifest["delivery_inspection"]["result"]
        )
    elif operation == "evidence_missing":
        descendant(manifest)
        manifest["standing"]["evidence_status"] = "missing"
    elif operation == "evidence_invalid":
        descendant(manifest)
        manifest["standing"]["evidence_status"] = "invalid"
    elif operation == "decision_missing":
        descendant(manifest)
        manifest["standing"]["decision_status"] = "missing"
    elif operation == "verifier_invalid":
        descendant(manifest)
        manifest["standing"]["verifier_status"] = "invalid"
    elif operation == "mutated_finding_same_handle":
        manifest["accepted_finding"]["assertion"]["text"] += " Mutated."
    elif operation == "finding_handle_collision":
        manifest["accepted_finding"]["id"] = "vf_ffffffffffffffff"
    elif operation == "decision_handle_root_mismatch":
        manifest["dependency"]["decision_event_id"] = "vev_ffffffffffffffff"
    elif operation == "standing_finding_root_mismatch":
        manifest["standing"]["selected_finding_revision_root"] = root("wrong-finding")
    elif operation == "standing_decision_root_mismatch":
        manifest["standing"]["decision_event_content_root"] = root("wrong-decision")
    elif operation == "standing_receipt_root_mismatch":
        manifest["standing"]["receipt_roots"] = [root("wrong-receipt")]
    elif operation == "standing_attachment_root_mismatch":
        manifest["standing"]["verifier_attachments"][0]["attachment_content_root"] = (
            root("wrong-attachment")
        )
    elif operation == "standing_premise_mismatch":
        manifest["standing"]["premise_digest"] = root("wrong-premise")
    elif operation == "standing_authority_mismatch":
        manifest["standing"]["authority_id"] = "reviewer:other"
    elif operation == "dependency_parent_commit_mismatch":
        manifest["dependency"]["parent_git_commit"] = "f" * 40
    elif operation == "dependency_parent_tree_mismatch":
        manifest["dependency"]["parent_git_tree"] = "e" * 40
    elif operation == "dependency_parent_event_mismatch":
        manifest["dependency"]["parent_event_log_root"] = root("wrong-parent-event")
    elif operation == "dependency_parent_snapshot_mismatch":
        manifest["dependency"]["parent_snapshot_root"] = root("wrong-parent-snapshot")
    elif operation == "short_delivered_commit":
        manifest["delivered"]["git_commit"] = "1234abcd"
    elif operation == "delivered_tree_mismatch":
        manifest["delivered"]["git_tree"] = "d" * 40
    elif operation == "same_relation_different_state":
        manifest["delivered"]["snapshot_root"] = root("other-snapshot")
    elif operation == "continuity_endpoint_mismatch":
        manifest["delivery_inspection"]["result"]["delivered_git_commit"] = "f" * 40
        manifest["delivery_inspection"]["inspection_root"] = root_from_value(
            manifest["delivery_inspection"]["result"]
        )
    elif operation == "inspection_root_mismatch":
        manifest["delivery_inspection"]["inspection_root"] = root(
            "wrong-inspection-root"
        )
    elif operation == "change_event_missing_from_history":
        changed(manifest, "finding_status", "corrected")
        result = manifest["delivery_inspection"]["result"]
        install_inspection(
            manifest,
            last_commit=result["last_seen_git_commit"],
            last_tree=result["last_seen_git_tree"],
            delivered_commit=result["delivered_git_commit"],
            delivered_tree=result["delivered_git_tree"],
            merge_base=result["merge_base"],
            git_relation="descendant",
            event_relation="same",
            last_snapshot=result["last_seen_snapshot"],
            delivered_snapshot=result["delivered_snapshot"],
            last_events=result["last_seen_events"],
            delivered_events=result["last_seen_events"],
        )
        manifest["standing"]["change_event"]["inspection_result_root"] = manifest[
            "delivery_inspection"
        ]["inspection_root"]
    elif operation == "change_event_effect_mismatch":
        changed(manifest, "finding_status", "corrected")
        manifest["standing"]["change_event"]["effect"] = "superseded"
    elif operation == "dependency_receipt_permutation":
        manifest["dependency"]["receipt_roots"].reverse()
    elif operation == "standing_receipt_permutation":
        manifest["standing"]["receipt_roots"].reverse()
    elif operation == "dependency_attachment_permutation":
        manifest["dependency"]["verifier_attachments"].reverse()
    elif operation == "standing_attachment_permutation":
        manifest["standing"]["verifier_attachments"].reverse()
    elif operation == "float_number":
        manifest["accepted_finding"]["confidence"]["score"] = 1.0
        return unsafe_numeric_envelope(manifest)
    elif operation == "negative_zero":
        manifest["accepted_finding"]["confidence"]["score"] = -0.0
        return unsafe_numeric_envelope(manifest)
    elif operation == "exponent_number":
        manifest["accepted_finding"]["confidence"]["score"] = 1e-7
        return unsafe_numeric_envelope(manifest)
    elif operation == "unsafe_integer":
        manifest["accepted_finding"]["confidence"]["score"] = 2**53
        return unsafe_numeric_envelope(manifest)
    elif operation == "unknown_manifest_field":
        manifest["child_truth"] = "true"
    elif operation == "automatic_child_truth_claim":
        manifest["standing"]["child_truth"] = "false"
    elif operation == "oversized_claim":
        manifest["accepted_finding"]["assertion"]["text"] = "x" * (64 * 1024 + 1)
    elif operation == "fact_manifest_root_mismatch":
        envelope = unchecked_envelope(manifest)
        envelope["fact_manifest_root"] = root("wrong-manifest")
        return canonical_bytes(envelope)
    elif operation == "duplicate_json_name":
        envelope = unchecked_envelope(manifest)
        raw = canonical_bytes(envelope).decode("utf-8")
        return raw.replace(
            '"schema":"vela.verifiable-composition.fact-envelope.v0"',
            '"schema":"vela.verifiable-composition.fact-envelope.v0",'
            '"schema":"vela.verifiable-composition.fact-envelope.v0"',
            1,
        ).encode("utf-8")
    elif operation == "nonfinite_number":
        envelope = unchecked_envelope(manifest)
        raw = canonical_bytes(envelope).decode("utf-8")
        return raw.replace(
            '"fact_manifest":{',
            '"fact_manifest":{"nonfinite_probe":NaN,',
            1,
        ).encode("utf-8")
    else:
        raise RuntimeError(f"unknown mutation {operation}")
    return canonical_bytes(unchecked_envelope(manifest))


def unchecked_envelope(manifest: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": FACT_ENVELOPE_SCHEMA,
        "fact_manifest_root": fact_manifest_root(manifest),
        "fact_manifest": manifest,
    }


def unsafe_numeric_envelope(manifest: dict[str, Any]) -> bytes:
    return json.dumps(
        {
            "schema": FACT_ENVELOPE_SCHEMA,
            "fact_manifest_root": root("unsafe-numeric-placeholder"),
            "fact_manifest": manifest,
        },
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def run_cli(script: Path, manifest: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(script), "--manifest", str(manifest)],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
        env={"PATH": "/usr/bin:/bin", "PYTHONDONTWRITEBYTECODE": "1"},
    )


def assert_non_authoritative(value: dict[str, Any], label: str) -> None:
    require(value["authoritative"] is False, f"{label}: authority flag changed")
    require(value["rebuildable"] is True, f"{label}: not rebuildable")
    require(value["projection"] == "derived_read_only", f"{label}: not derived")
    require(value["child_truth"] == "not_assessed", f"{label}: child truth inferred")
    require(value["child_mutation"] == "none", f"{label}: child mutation claimed")
    require(value["authority_effect"] == "none", f"{label}: authority effect claimed")
    require(value["writes"] == [], f"{label}: write surface appeared")


def main() -> int:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    require(
        vectors.get("schema") == "vela.verifiable-composition.fact-manifest-vectors.v0",
        "vector schema drift",
    )
    cases = vectors.get("cases")
    require(isinstance(cases, list) and cases, "vectors missing")
    ids = [case["id"] for case in cases]
    require(len(ids) == len(set(ids)), "duplicate vector ids")
    require(
        {case["expected_status"] for case in cases}
        == {
            "satisfied",
            "warning",
            "review_required",
            "blocked",
            "stale",
            "forked",
            "unresolvable",
        },
        "vectors do not cover exactly the seven projection statuses",
    )

    baseline = base_manifest()
    baseline_snapshot = copy.deepcopy(baseline)
    built = build_envelope(baseline)
    require(baseline == baseline_snapshot, "build_envelope mutated caller input")
    require(
        built["fact_manifest_root"] == fact_manifest_root(baseline),
        "built envelope root drift",
    )
    direct = resolve_envelope(built)
    require(baseline == baseline_snapshot, "resolve_envelope mutated caller input")
    require(direct["dependency_status"] == "satisfied", "baseline did not satisfy")
    require(
        direct["selected_parent"] == baseline["dependency"],
        "resolver did not preserve the complete exact dependency tuple",
    )

    checked = 0
    cli_checked = 0
    with tempfile.TemporaryDirectory(prefix="vela-adr4-projections-") as raw_temp:
        temporary = Path(raw_temp)
        for case in cases:
            raw = mutate(copy.deepcopy(base_manifest()), case["mutation"])
            original_raw = bytes(raw)
            envelope, first = resolve_bytes(raw)
            _, second = resolve_bytes(raw)
            reader_c = reader_c_read_bytes(raw)
            require(raw == original_raw, f"{case['id']}: resolver mutated bytes")
            require(
                canonical_bytes(first) == canonical_bytes(second),
                f"{case['id']}: nondeterministic resolution",
            )
            require(
                first["dependency_status"] == case["expected_status"],
                f"{case['id']}: expected {case['expected_status']}, got "
                f"{first['dependency_status']}",
            )
            require(
                first["code"] == case["expected_code"],
                f"{case['id']}: expected {case['expected_code']}, got {first['code']}",
            )
            require(
                reader_c["dependency_status"] == first["dependency_status"],
                f"{case['id']}: Reader C status drift "
                f"{reader_c['dependency_status']} != {first['dependency_status']}",
            )
            require(
                reader_c["fact_manifest_root"] == first["fact_manifest_root"]
                and reader_c["dependency_observation_root"]
                == first["dependency_observation_root"],
                f"{case['id']}: Reader C root drift",
            )
            require(
                reader_c["child_truth"] == "not_assessed"
                and reader_c["authority_effect"] == "none",
                f"{case['id']}: Reader C crossed the trust boundary",
            )
            assert_non_authoritative(first, f"{case['id']}:resolver")

            envelope_snapshot = copy.deepcopy(envelope)
            first_snapshot = copy.deepcopy(first)
            ci = correction_ci_projection(first)
            context = accepted_context_pack_projection(envelope, first)
            require(
                envelope == envelope_snapshot and first == first_snapshot,
                f"{case['id']}: projection mutated its input",
            )
            require(
                ci["dependency_status"] == first["dependency_status"],
                f"{case['id']}: CI status drift",
            )
            require(
                context["dependency_status"] == first["dependency_status"],
                f"{case['id']}: context status drift",
            )
            require(
                ci["fact_manifest_root"]
                == first["fact_manifest_root"]
                == context["fact_manifest_root"],
                f"{case['id']}: fact root drift across consumers",
            )
            require(
                ci["dependency_observation_root"]
                == first["dependency_observation_root"]
                == context["dependency_observation_root"],
                f"{case['id']}: exact tuple root drift across consumers",
            )
            assert_non_authoritative(ci, f"{case['id']}:ci")
            assert_non_authoritative(context, f"{case['id']}:context")
            require(
                bool(ci["warning_targets"])
                == (first["dependency_status"] == "warning"),
                f"{case['id']}: CI warning visibility drift",
            )
            if first["dependency_status"] in {"satisfied", "warning"}:
                require(
                    context["active_context_count"] == 1,
                    f"{case['id']}: usable context not activated",
                )
                require(
                    context["active_context"][0]["finding"]["links"] == [],
                    f"{case['id']}: mutable finding links leaked into accepted context",
                )
                require(
                    not context["quarantined_context"],
                    f"{case['id']}: usable context quarantined",
                )
                require(
                    bool(context["context_warnings"])
                    == (first["dependency_status"] == "warning"),
                    f"{case['id']}: warning visibility drift",
                )
            else:
                require(
                    context["active_context_count"] == 0
                    and context["active_context"] == [],
                    f"{case['id']}: unsafe context activation",
                )
            checked += 1

            if case.get("cli"):
                path = temporary / f"{case['id']}.json"
                path.write_bytes(raw)
                before = sorted(item.name for item in temporary.iterdir())
                resolver_runs = [run_cli(RESOLVER, path), run_cli(RESOLVER, path)]
                require(
                    resolver_runs[0].stdout == resolver_runs[1].stdout,
                    f"{case['id']}: resolver CLI nondeterministic",
                )
                resolver_value = json.loads(resolver_runs[0].stdout)
                require(
                    resolver_value["dependency_status"] == case["expected_status"],
                    f"{case['id']}: resolver CLI status drift",
                )
                expected_resolver_exit = (
                    1 if case["expected_status"] == "unresolvable" else 0
                )
                require(
                    resolver_runs[0].returncode == expected_resolver_exit,
                    f"{case['id']}: resolver CLI exit drift",
                )

                ci_run = run_cli(CORRECTION_CI, path)
                ci_value = json.loads(ci_run.stdout)
                require(
                    ci_value["dependency_status"] == case["expected_status"],
                    f"{case['id']}: CI CLI status drift",
                )
                require(
                    ci_run.returncode == ci_value["suggested_exit_code"],
                    f"{case['id']}: CI CLI exit drift",
                )

                context_run = run_cli(CONTEXT_PACK, path)
                context_value = json.loads(context_run.stdout)
                require(
                    context_value["dependency_status"] == case["expected_status"],
                    f"{case['id']}: context CLI status drift",
                )
                require(
                    context_run.returncode
                    == (
                        0 if case["expected_status"] in {"satisfied", "warning"} else 1
                    ),
                    f"{case['id']}: context CLI exit drift",
                )
                after = sorted(item.name for item in temporary.iterdir())
                require(before == after, f"{case['id']}: CLI wrote an output file")
                cli_checked += 1

        target = temporary / "regular.json"
        target.write_bytes(canonical_bytes(build_envelope(base_manifest())))
        symlink = temporary / "manifest-link.json"
        symlink.symlink_to(target.name)
        for script in (RESOLVER, CORRECTION_CI, CONTEXT_PACK):
            result = run_cli(script, symlink)
            require(result.returncode != 0, f"{script.name}: symlink input accepted")
            value = json.loads(result.stdout)
            require(
                value["dependency_status"] == "unresolvable",
                f"{script.name}: symlink did not fail closed",
            )

    core_source = (REFERENCE / "fact_manifest.py").read_text(encoding="utf-8")
    for forbidden in (
        "import os",
        "import pathlib",
        "import socket",
        "import subprocess",
        "import time",
        "import random",
        "requests",
        "urllib",
    ):
        require(forbidden not in core_source, f"pure core gained {forbidden}")
    reader_source = (REFERENCE / "reader_c.py").read_text(encoding="utf-8")
    for forbidden in (
        "import fact_manifest",
        "from fact_manifest",
        "import vela",
        "from vela",
        "sys.path.insert",
        "subprocess",
    ):
        require(forbidden not in reader_source, f"Reader C gained {forbidden}")

    print(
        "fact-manifest projections: "
        f"{checked}/{checked} hostile vectors; "
        f"{cli_checked} three-consumer CLI cases; "
        "independent Reader C status/root parity; "
        "all seven statuses covered; zero authority or child-truth effects"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
