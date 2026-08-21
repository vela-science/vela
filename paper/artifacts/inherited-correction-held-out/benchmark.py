#!/usr/bin/env python3
"""Deterministic builder and closed scorer for the held-out benchmark."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import random
import re
import shutil
import statistics
import stat
import sys
from decimal import Decimal, ROUND_HALF_EVEN
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
FAMILIES_PATH = ROOT / "families-source.json"
TASK_PATH = ROOT / "TASK.md"
SCHEMA_PATH = ROOT / "response-schema.json"
SEED_PATH = ROOT / "assignment-seed.json"
RUNTIME_PATH = ROOT / "runtime-binding.json"
ADJUDICATION_COMMITMENT_PATH = ROOT / "adjudication-commitment.json"
LAUNCH_AMENDMENT_PATH = ROOT / "launch-authorization-amendment.json"
SOURCE_PATHS = (
    "DESIGN.md",
    "README.md",
    "TASK.md",
    "adjudication-commitment.json",
    "assignment-seed.json",
    "benchmark.py",
    "custody.py",
    "families-source.json",
    "launch-authorization-amendment.json",
    "response-schema.json",
    "runtime-binding.json",
    "test_benchmark.py",
)
CONDITIONS = ("git-documents", "state-wrapper", "vela")
MEAN_QUANTUM = Decimal("0.00000000000001")
RATIO_QUANTUM = Decimal("0.000000000000001")
LABELS = {
    "affected",
    "unaffected",
    "must_reassess",
    "presently_unprovable",
}
ACTIONS = {
    "retrieve_missing_premise",
    "no_correction_reassessment",
    "rerun_dependent_method",
    "recompute_with_successor",
}
AUTHORITY_EFFECTS = {
    "no_authoritative_status_change",
    "authorized_status_change",
    "status_change_unprovable",
}
AUTHORITY_ACTIONS = {
    "record_no_status_change",
    "accept_authorized_status_change",
    "withhold_status_change",
}
FORBIDDEN_WRAPPER_VOCABULARY = {
    "vela",
    "repository",
    "decision",
    "event",
    "standing",
    "replay",
}


class BenchmarkError(RuntimeError):
    pass


def json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode()


def compact_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()


def byte_digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return byte_digest(compact_bytes(value))


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BenchmarkError(f"json_invalid:{path}") from error


def file_entry(path: str, raw: bytes) -> dict[str, Any]:
    return {"path": path, "bytes": len(raw), "sha256": byte_digest(raw)}


def family_atoms(family: dict[str, Any]) -> list[dict[str, Any]]:
    atoms = [
        {"path": "family_id", "value": family["family_id"]},
        {"path": "correction", "value": family["correction"]},
        {"path": "authority", "value": family["authority"]},
    ]
    for consequence in family["consequences"]:
        atoms.append(
            {
                "path": f"consequences/{consequence['claim_id']}",
                "value": consequence,
            }
        )
    return atoms


def source_files(family: dict[str, Any]) -> dict[str, bytes]:
    correction = family["correction"]
    values = {
        f"source/{correction['predecessor_claim_id']}.md": correction[
            "source_v1"
        ].encode(),
        f"source/{correction['successor_claim_id']}.md": correction[
            "source_v2"
        ].encode(),
    }
    for consequence in family["consequences"]:
        values[consequence["evidence_path"]] = consequence["evidence_content"].encode()
    authority = family["authority"]
    values[authority["evidence_path"]] = authority["evidence_content"].encode()
    return values


def git_documents(family: dict[str, Any]) -> dict[str, bytes]:
    correction = family["correction"]
    history = (
        "# Correction history\n\n"
        f"`{correction['successor_claim_id']}` supersedes "
        f"`{correction['predecessor_claim_id']}`. {correction['reason']}\n\n"
        f"Predecessor: {correction['predecessor_assertion']}\n\n"
        f"Successor: {correction['successor_assertion']}\n"
    )
    claims = ["# Consequence Claims", ""]
    dependencies = ["# Dependency notes", ""]
    for consequence in family["consequences"]:
        claims.extend(
            [
                f"## {consequence['claim_id']}",
                "",
                consequence["assertion"],
                "",
                f"Evidence: `{consequence['evidence_path']}`",
                "",
            ]
        )
        for relation in consequence["relations"]:
            dependencies.extend(
                [
                    f"## {consequence['claim_id']} -> {relation['target_claim_id']}",
                    "",
                    f"Kind: `{relation['kind']}`. {relation['meaning']}",
                    "",
                ]
            )
    authority = family["authority"]
    authority_text = (
        "# Acceptance scope\n\n"
        f"Regime: `{authority['regime']}`.\n\n"
        f"Acceptance action present: `{str(authority['acceptance_action_present']).lower()}`.\n\n"
        f"Authorization verified: `{str(authority['authorization_verified']).lower()}`.\n\n"
        f"Evidence: `{authority['evidence_path']}`.\n\n{authority['instruction']}\n"
    )
    return {
        "HISTORY.md": history.encode(),
        "CLAIMS.md": ("\n".join(claims).rstrip() + "\n").encode(),
        "DEPENDENCIES.md": ("\n".join(dependencies).rstrip() + "\n").encode(),
        "AUTHORITY.md": authority_text.encode(),
    }


def consequence_state(consequence: dict[str, Any]) -> dict[str, Any]:
    kinds = {item["kind"] for item in consequence["relations"]}
    return {
        "claim_id": consequence["claim_id"],
        "state": "active" if kinds == {"discovery_only"} else "needs_recheck",
        "triggers": sorted(
            {
                item["target_claim_id"]
                for item in consequence["relations"]
                if item["kind"] != "discovery_only"
            }
        ),
        "missing_inputs": sorted(
            item["target_claim_id"]
            for item in consequence["relations"]
            if item["kind"] == "requires_unavailable"
        ),
    }


def state_wrapper_documents(family: dict[str, Any]) -> dict[str, bytes]:
    correction = family["correction"]
    states = {
        item["claim_id"]: consequence_state(item) for item in family["consequences"]
    }
    units = {
        "schema": "structured-state-units.v1",
        "family_id": family["family_id"],
        "units": [
            {
                "id": correction["predecessor_claim_id"],
                "content": correction["predecessor_assertion"],
                "state": "superseded",
            },
            {
                "id": correction["successor_claim_id"],
                "content": correction["successor_assertion"],
                "state": "active",
            },
            *[
                {
                    "id": item["claim_id"],
                    "content": item["assertion"],
                    "evidence_path": item["evidence_path"],
                    "state": states[item["claim_id"]]["state"],
                }
                for item in family["consequences"]
            ],
        ],
    }
    return {
        "state/units.json": compact_bytes(units),
        "state/supersession.json": compact_bytes(
            {
                "schema": "structured-state-supersession.v1",
                "predecessor_id": correction["predecessor_claim_id"],
                "successor_id": correction["successor_claim_id"],
                "reason": correction["reason"],
            }
        ),
        "state/dependencies.json": compact_bytes(
            {
                "schema": "structured-state-dependencies.v1",
                "family_id": family["family_id"],
                "dependencies": [
                    {"claim_id": item["claim_id"], "relations": item["relations"]}
                    for item in family["consequences"]
                ],
            }
        ),
        "state/current-view.json": compact_bytes(
            {
                "schema": "structured-state-current-view.v1",
                "family_id": family["family_id"],
                "current_source_id": correction["successor_claim_id"],
                "superseded_source_ids": [correction["predecessor_claim_id"]],
                "consequences": [
                    states[item["claim_id"]] for item in family["consequences"]
                ],
            }
        ),
        "state/scope.json": compact_bytes(
            {
                "schema": "structured-state-scope.v1",
                "family_id": family["family_id"],
                "regime": family["authority"]["regime"],
                "acceptance_action_present": family["authority"][
                    "acceptance_action_present"
                ],
                "authorization_verified": family["authority"]["authorization_verified"],
                "evidence_path": family["authority"]["evidence_path"],
                "instruction": family["authority"]["instruction"],
            }
        ),
    }


def vela_documents(family: dict[str, Any]) -> dict[str, bytes]:
    correction = family["correction"]
    projection = {
        "schema": "vela.inherited-correction-held-out-projection.v1",
        "family_id": family["family_id"],
        "correction": {
            key: correction[key]
            for key in (
                "predecessor_claim_id",
                "predecessor_assertion",
                "successor_claim_id",
                "successor_assertion",
                "reason",
            )
        },
        "authority": {
            "regime": family["authority"]["regime"],
            "decision_present": family["authority"]["acceptance_action_present"],
            "decision_authorization_verified": family["authority"][
                "authorization_verified"
            ],
            "standing_effect": {
                "no_acceptance_action": "none",
                "independently_authorized_acceptance_action": "changed_by_authorized_decision",
                "authorization_presently_unprovable": "presently_unprovable",
            }[family["authority"]["regime"]],
            "evidence_path": family["authority"]["evidence_path"],
        },
        "claims": [
            {
                "claim_id": item["claim_id"],
                "assertion": item["assertion"],
                "evidence_path": item["evidence_path"],
                "evidence_binding": file_entry(
                    item["evidence_path"], source_files(family)[item["evidence_path"]]
                ),
                "relations": item["relations"],
                "inherited_state": consequence_state(item),
            }
            for item in family["consequences"]
        ],
    }
    replay = {
        "schema": "vela.inherited-correction-held-out-replay.v1",
        "family_id": family["family_id"],
        "events": [
            {
                "sequence": 1,
                "kind": "predecessor_observed",
                "claim_id": correction["predecessor_claim_id"],
            },
            {
                "sequence": 2,
                "kind": "successor_supersedes",
                "claim_id": correction["successor_claim_id"],
                "supersedes": correction["predecessor_claim_id"],
            },
        ],
    }
    return {
        "repository-projection.json": compact_bytes(projection),
        "replay.json": compact_bytes(replay),
    }


def packet_files(family: dict[str, Any], condition: str) -> dict[str, bytes]:
    files = {
        "TASK.md": TASK_PATH.read_bytes(),
        "response-schema.json": SCHEMA_PATH.read_bytes(),
        **source_files(family),
    }
    if condition == "git-documents":
        files.update(git_documents(family))
    elif condition == "state-wrapper":
        files.update(state_wrapper_documents(family))
    elif condition == "vela":
        files.update(vela_documents(family))
    else:
        raise BenchmarkError("condition_invalid")
    manifest = {
        "schema": "inherited-correction-held-out-packet-manifest.v1",
        "family_id": family["family_id"],
        "condition": condition,
        "source_and_evidence": [
            file_entry(path, raw) for path, raw in sorted(source_files(family).items())
        ],
        "atomic_facts_root": canonical_root(family_atoms(family)),
    }
    files["PACKET-MANIFEST.json"] = json_bytes(manifest)
    return files


def packet_root(files: dict[str, bytes]) -> str:
    return canonical_root(
        [file_entry(path, raw) for path, raw in sorted(files.items())]
    )


def prompt_bytes(files: dict[str, bytes]) -> bytes:
    payload = {
        "schema": "inherited-correction-held-out-virtual-filesystem.v1",
        "instruction": "Use only this immutable virtual filesystem and return one schema-valid JSON response without tools.",
        "files": [
            {"path": path, "content": raw.decode()}
            for path, raw in sorted(files.items())
        ],
    }
    return json_bytes(payload)


def family_map() -> dict[str, dict[str, Any]]:
    source = load_json(FAMILIES_PATH)
    families = source.get("families")
    if not isinstance(families, list) or len(families) != 3:
        raise BenchmarkError("family_count_invalid")
    result = {family["family_id"]: family for family in families}
    if len(result) != 3:
        raise BenchmarkError("family_id_duplicate")
    return result


def assignment_plan() -> dict[str, Any]:
    seed = load_json(SEED_PATH)
    cells = [
        {"family_id": family_id, "condition": condition, "replicate": replicate}
        for family_id in sorted(family_map())
        for condition in CONDITIONS
        for replicate in range(1, 5)
    ]
    random.Random(int(seed["seed_hex"], 16)).shuffle(cells)
    assignments = []
    for index, cell in enumerate(cells, 1):
        assignments.append(
            {
                **cell,
                "run_id": f"heldout-run-{index:02d}",
                "participant_instance_id": f"heldout-sol-{index:02d}",
            }
        )
    return {
        "schema": "vela.inherited-correction-held-out-assignment-plan.v1",
        "seed_commitment": byte_digest(seed["seed_hex"].encode()),
        "assignments": assignments,
    }


def assignment_schedule(
    registration_root: str,
    packet_roots: dict[str, dict[str, str]],
    runtime: dict[str, Any],
) -> dict[str, Any]:
    plan = assignment_plan()
    return {
        "schema": "vela.inherited-correction-held-out-assignment.v1",
        "seed_commitment": plan["seed_commitment"],
        "registration_root": registration_root,
        "image_digest": runtime["container_image_digest"],
        "assignments": [
            {
                **row,
                "packet_root": packet_roots[row["family_id"]][row["condition"]],
            }
            for row in plan["assignments"]
        ],
    }


def runtime_configuration(
    registration_root: str,
    prompt_root: str,
    runtime: dict[str, Any],
) -> dict[str, Any]:
    return {
        "schema": "vela.inherited-correction-oci-participant-configuration.v3",
        "registration_root": registration_root,
        "image_digest": runtime["container_image_digest"],
        "base_image_digest": runtime["base_image_digest"],
        "provider": runtime["provider"],
        "authentication": runtime["authentication"],
        "codex_cli_version": runtime["codex_cli_version"],
        "model": runtime["model"],
        "reasoning_effort": runtime["reasoning_effort"],
        "service_tier": runtime["service_tier"],
        "timeout_seconds": runtime["timeout_seconds"],
        "output_token_ceiling": runtime["output_token_limit"],
        "attempt": runtime["attempt"],
        "retries": runtime["retries"],
        "tools": "none",
        "one_prompt": runtime["one_prompt"],
        "one_model_turn": runtime["one_model_turn"],
        "store": runtime["store"],
        "schema_validator": runtime["schema_validator"],
        "prompt_root": prompt_root,
        "response_schema_bytes": byte_digest(SCHEMA_PATH.read_bytes()),
        "trust_bundle_path": runtime["trust_bundle_path"],
        "trust_bundle_bytes": runtime["trust_bundle_bytes"],
        "strict_overrides_root": runtime["strict_overrides_root"],
        "provider_usage_disposition": "cost telemetry only; only genuine provider context/output-limit failure invalidates",
        "tool_boundary": "supported disables plus immediate streaming abort and terminal failure on any tool event",
        "workdir": "empty read-only participant workdir",
    }


def generated_outputs() -> dict[str, bytes]:
    families = family_map()
    outputs: dict[str, bytes] = {}
    packet_roots: dict[str, dict[str, str]] = {}
    prompt_roots: dict[str, dict[str, str]] = {}
    equivalence_families = []
    for family_id, family in sorted(families.items()):
        packet_roots[family_id] = {}
        prompt_roots[family_id] = {}
        prompt_lengths = {}
        for condition in CONDITIONS:
            files = packet_files(family, condition)
            packet_roots[family_id][condition] = packet_root(files)
            prompt = prompt_bytes(files)
            prompt_roots[family_id][condition] = byte_digest(prompt)
            prompt_lengths[condition] = len(prompt)
            base = f"conditions/{family_id}/{condition}/packet"
            for path, raw in files.items():
                outputs[f"{base}/{path}"] = raw
            outputs[f"conditions/{family_id}/{condition}/input/prompt.txt"] = prompt
            outputs[
                f"conditions/{family_id}/{condition}/input/response-schema.json"
            ] = SCHEMA_PATH.read_bytes()
        common_sources = [
            file_entry(path, raw) for path, raw in sorted(source_files(family).items())
        ]
        shortest = min(prompt_lengths.values())
        prompt_basis_points = (
            max(prompt_lengths.values()) * 10_000 + shortest - 1
        ) // shortest
        equivalence_families.append(
            {
                "family_id": family_id,
                "atomic_facts_root": canonical_root(family_atoms(family)),
                "source_and_evidence": common_sources,
                "condition_packet_roots": packet_roots[family_id],
                "prompt_bytes": prompt_lengths,
                "max_to_min_prompt_basis_points": prompt_basis_points,
                "prompt_length_bound_basis_points": 12_000,
                "prompt_length_bound_pass": prompt_basis_points <= 12_000,
            }
        )
    equivalence = {
        "schema": "vela.inherited-correction-held-out-equivalence.v1",
        "families": equivalence_families,
        "condition_difference": "Organization only; atomic facts and exact source/evidence bytes are identical within each family.",
        "length_control": "Within each family, maximum serialized prompt bytes must be no more than 1.20 times minimum serialized prompt bytes before any session.",
    }
    outputs["input-equivalence.json"] = json_bytes(equivalence)
    plan = assignment_plan()
    plan_root = canonical_root(plan)
    runtime = load_json(RUNTIME_PATH)
    participant_configuration = {
        "schema": "vela.inherited-correction-held-out-participant-configuration.v1",
        "runtime": runtime,
        "response_schema_bytes": byte_digest(SCHEMA_PATH.read_bytes()),
        "task_bytes": byte_digest(TASK_PATH.read_bytes()),
        "fixed_sessions": 36,
        "zero_retries": True,
        "one_shot": True,
        "condition_specific_fields": [
            "family_id",
            "condition",
            "packet_root",
            "prompt_root",
            "runtime_configuration_root",
        ],
    }
    config_root = canonical_root(participant_configuration)
    outputs["participant-configuration.json"] = json_bytes(participant_configuration)
    commitment = load_json(ADJUDICATION_COMMITMENT_PATH)
    amendment = load_json(LAUNCH_AMENDMENT_PATH)
    if amendment.get("evaluator_commitment") != commitment:
        raise BenchmarkError("launch_amendment_commitment_drift")
    registration = {
        "schema": "vela.inherited-correction-held-out-preregistration.v1",
        "status": "held_pending_exact_binding_review",
        "purpose": "Test continuation and reassessment cost across three unseen consequential-correction families without assuming lift.",
        "families": sorted(families),
        "design": {
            "sessions": 36,
            "sessions_per_family_condition": 4,
            "conditions": list(CONDITIONS),
            "attempt": 1,
            "timeout_seconds": 600,
            "zero_retries_substitutions": True,
        },
        "bindings": {
            "assignment_plan_root": plan_root,
            "assignment_seed_commitment": plan["seed_commitment"],
            "participant_configuration_root": config_root,
            "equivalence_root": canonical_root(equivalence),
            "benchmark_bytes": byte_digest((ROOT / "benchmark.py").read_bytes()),
            "custody_bytes": byte_digest((ROOT / "custody.py").read_bytes()),
            "test_bytes": byte_digest((ROOT / "test_benchmark.py").read_bytes()),
            "response_schema_bytes": byte_digest(SCHEMA_PATH.read_bytes()),
            "task_bytes": byte_digest(TASK_PATH.read_bytes()),
            "packet_roots": packet_roots,
            "prompt_roots": prompt_roots,
            "runtime_root": canonical_root(runtime),
            "adjudication_commitment": commitment,
            "launch_authorization_amendment": amendment,
            "launch_authorization_amendment_root": canonical_root(amendment),
        },
        "scoring": {
            "exact_success": "Exact pair, all four classifications and actions, both authority codes, and all four exact consequence path/digest bindings.",
            "restricted_seconds": "Actual duration for exact success; otherwise 600 seconds.",
            "estimands": {
                "structure": "state-wrapper minus Git/documents for exact-success rate and correction-impact-complete rate; Git/documents minus state-wrapper for authority-error-rate reduction and restricted seconds saved; time ratio state-wrapper/Git-documents is secondary. Equal-denominator count deltas are also reported.",
                "governance_inheritance": "Vela minus state-wrapper for exact-success rate and correction-impact-complete rate; state-wrapper minus Vela for authority-error-rate reduction and restricted seconds saved; time ratio Vela/state-wrapper is secondary. Equal-denominator count deltas are also reported.",
                "total": "Vela minus Git/documents for exact-success rate and correction-impact-complete rate; Git/documents minus Vela for authority-error-rate reduction and restricted seconds saved; time ratio Vela/Git-documents is secondary. Equal-denominator count deltas are also reported.",
            },
            "family_gates": {
                "structure": "State-wrapper >=3/4 exact and >=3/4 correction-impact-complete, zero authority errors, no fewer exact or impact-complete than Git/documents, and restricted mean <=0.80 of Git/documents.",
                "governance_inheritance": "Vela is noninferior to state-wrapper on exact success, correction-impact completeness, authority errors, and restricted mean within every family.",
                "total": "The preserved Vela-versus-Git gate: Vela >=3/4 exact and >=3/4 correction-impact-complete, zero authority errors, no fewer exact or impact-complete than Git/documents, and restricted mean <=0.80 of Git/documents.",
            },
            "aggregate_gates": {
                "structure": "All family structure gates pass; state-wrapper >=9/12 exact and >=9/12 correction-impact-complete, zero authority errors, and restricted mean <=0.80 of Git/documents.",
                "governance_inheritance": "All family governance noninferiority gates pass; Vela >=9/12 exact and >=9/12 correction-impact-complete with zero authority errors; and Vela has a strict aggregate increment of at least two exact successes, two impact-complete sessions, two avoided authority errors, or a restricted-time ratio <=0.90 versus state-wrapper. Equality alone cannot pass.",
                "total": "The preserved Vela-versus-Git aggregate gate: all family total gates pass; Vela >=9/12 exact and >=9/12 correction-impact-complete with zero authority errors; restricted mean <=0.80 of Git/documents.",
            },
            "positive_gate": "Pass only when structure, governance/inheritance, and preserved total gates all pass. These are bounded descriptive gates, not broad significance tests.",
            "threshold_rationale": "Fixed prospectively at 75 percent exact, the preserved 0.80 total time ratio, analogous 0.80 structure ratio, family governance noninferiority, and a strict aggregate governance increment before external adjudication is frozen; not tuned to held-out answers.",
            "rounding": "Decimal ROUND_HALF_EVEN; means 1e-14 and ratios 1e-15.",
        },
        "claim_ceiling": "A passing result would be bounded descriptive evidence for these synthetic families only, not scientific acceptance, general productivity, or authority.",
    }
    registration_root = canonical_root(registration)
    registration["registration_root"] = registration_root
    outputs["preregistration.json"] = json_bytes(registration)
    schedule = assignment_schedule(registration_root, packet_roots, runtime)
    schedule_root = canonical_root(schedule)
    outputs["assignment-schedule.json"] = json_bytes(schedule)
    runtime_configuration_roots: dict[str, dict[str, str]] = {}
    for family_id in sorted(families):
        runtime_configuration_roots[family_id] = {}
        for condition in CONDITIONS:
            config = runtime_configuration(
                registration_root,
                prompt_roots[family_id][condition],
                runtime,
            )
            runtime_configuration_roots[family_id][condition] = canonical_root(config)
            base = f"conditions/{family_id}/{condition}/input"
            outputs[f"{base}/participant-configuration.json"] = json_bytes(config)
            outputs[f"{base}/assignment.json"] = json_bytes(schedule)
    mapping = {
        "schema": "vela.inherited-correction-held-out-configuration-mapping.v1",
        "status": "held_pending_exact_binding_review",
        "registration_root": registration_root,
        "shared_study_configuration_root": config_root,
        "family_condition_runtime_configuration_roots": runtime_configuration_roots,
    }
    outputs["configuration-mapping.json"] = json_bytes(mapping)
    mapping_root = canonical_root(mapping)
    hold = {
        "schema": "vela.inherited-correction-hold.v1",
        "status": "hold",
        "reason": "Exact adjudication-binding review PASS required before any permit release or provider call.",
        "updated_at": load_json(SEED_PATH)["generated_at"],
    }
    outputs["permit-template/hold-state.json"] = json_bytes(hold)
    for assignment in schedule["assignments"]:
        family_id = assignment["family_id"]
        condition = assignment["condition"]
        permit = {
            "schema": "vela.inherited-correction-launch-permit.v1",
            "status": "held",
            "expires_at": "not_authorized",
            "registration_root": registration_root,
            "image_digest": runtime["container_image_digest"],
            "assignment_root": schedule_root,
            "participant_configuration_root": runtime_configuration_roots[family_id][
                condition
            ],
            "run_id": assignment["run_id"],
            "participant_instance_id": assignment["participant_instance_id"],
            "condition": condition,
            "packet_root": packet_roots[family_id][condition],
            "prompt_root": prompt_roots[family_id][condition],
            "trust_bundle_bytes": runtime["trust_bundle_bytes"],
            "attempt": 1,
        }
        outputs[f"permit-template/{assignment['run_id']}.permit.json"] = json_bytes(
            permit
        )
    freeze = {
        "schema": "vela.inherited-correction-held-out-prelaunch-freeze.v1",
        "status": "held",
        "registration_root": registration_root,
        "assignment_root": schedule_root,
        "participant_configuration_root": config_root,
        "runtime_configuration_roots": runtime_configuration_roots,
        "configuration_mapping_root": mapping_root,
        "equivalence_root": canonical_root(equivalence),
        "packet_roots": packet_roots,
        "prompt_roots": prompt_roots,
        "runtime_root": canonical_root(runtime),
        "image_digest": runtime["container_image_digest"],
        "trust_bundle_bytes": runtime["trust_bundle_bytes"],
        "permit_set_root": canonical_root(
            [
                {
                    "path": path,
                    "sha256": byte_digest(raw),
                }
                for path, raw in sorted(outputs.items())
                if path.startswith("permit-template/heldout-run-")
            ]
        ),
        "adjudication_status": commitment["status"],
        "launch_authorization_amendment_root": canonical_root(amendment),
        "provider_calls": 0,
    }
    freeze["prelaunch_root"] = canonical_root(freeze)
    outputs["prelaunch-freeze.json"] = json_bytes(freeze)
    outputs["result.json"] = json_bytes(
        {
            "schema": "vela.inherited-correction-held-out-result.v1",
            "status": "not_run",
            "sessions_completed": 0,
            "fixed_denominator": 36,
            "positive_gate": "not_evaluated",
            "authority_effect": "none",
        }
    )
    return outputs


def artifact_manifest(outputs: dict[str, bytes]) -> dict[str, Any]:
    entries = [file_entry(path, (ROOT / path).read_bytes()) for path in SOURCE_PATHS]
    entries.extend(file_entry(path, raw) for path, raw in sorted(outputs.items()))
    value = {
        "schema": "vela.inherited-correction-held-out-manifest.v1",
        "entries": entries,
    }
    value["artifact_root"] = canonical_root(value)
    return value


def write_outputs() -> None:
    outputs = generated_outputs()
    for relative in ("conditions", "permit-template"):
        target = ROOT / relative
        if target.exists():
            shutil.rmtree(target)
    for path, raw in outputs.items():
        target = ROOT / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
    (ROOT / "manifest.json").write_bytes(json_bytes(artifact_manifest(outputs)))


def verify() -> None:
    outputs = generated_outputs()
    expected_paths = set(outputs) | set(SOURCE_PATHS) | {"manifest.json"}
    observed_paths = {
        path.relative_to(ROOT).as_posix()
        for path in ROOT.rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    }
    if observed_paths != expected_paths:
        raise BenchmarkError("artifact_file_set_drift")
    for path, expected in outputs.items():
        observed_path = ROOT / path
        if not observed_path.is_file() or observed_path.read_bytes() != expected:
            raise BenchmarkError(f"generated_output_drift:{path}")
    expected_manifest = json_bytes(artifact_manifest(outputs))
    if (ROOT / "manifest.json").read_bytes() != expected_manifest:
        raise BenchmarkError("manifest_drift")
    commitment = load_json(ADJUDICATION_COMMITMENT_PATH)
    expected_commitment_keys = {
        "adjudication_bytes",
        "adjudication_root",
        "answer_bytes_present_in_producer_artifact",
        "consequence_count",
        "family_count",
        "frozen_at",
        "plaintext_disclosed",
        "private_validation_receipt_root",
        "public_commitment_root",
        "required_before_permit_release",
        "schema",
        "status",
    }
    if set(commitment) != expected_commitment_keys:
        raise BenchmarkError("adjudication_commitment_fields_invalid")
    if commitment["status"] != "frozen_by_independent_evaluator":
        raise BenchmarkError("unexpected_adjudication_state")
    if not isinstance(commitment["adjudication_root"], str):
        raise BenchmarkError("adjudication_root_missing")
    if (
        commitment["plaintext_disclosed"]
        or commitment["answer_bytes_present_in_producer_artifact"]
    ):
        raise BenchmarkError("protected_adjudication_disclosed")
    if commitment["adjudication_bytes"] != {
        "length": 5883,
        "sha256": commitment["adjudication_root"],
    } or (commitment["family_count"], commitment["consequence_count"]) != (3, 12):
        raise BenchmarkError("adjudication_public_counts_or_bytes_invalid")
    amendment = load_json(LAUNCH_AMENDMENT_PATH)
    if amendment.get("evaluator_commitment") != commitment:
        raise BenchmarkError("launch_amendment_commitment_drift")
    if amendment.get("status") != "held_pending_exact_binding_review":
        raise BenchmarkError("launch_amendment_status_invalid")
    execution = amendment.get("execution_state", {})
    if execution != {
        "sessions_completed": 0,
        "fixed_denominator": 36,
        "permits_held": 36,
        "permits_consumed": 0,
        "provider_calls": 0,
        "protected_key_accesses": 0,
    }:
        raise BenchmarkError("launch_amendment_execution_state_invalid")
    packet_paths = [
        path
        for path in outputs
        if "/packet/" in path or path.endswith("/input/prompt.txt")
    ]
    for path in packet_paths:
        raw = outputs[path]
        if b"required_action_code" in raw or b"expected_classification" in raw:
            raise BenchmarkError(f"answer_key_leak:{path}")
    for family in json.loads(outputs["input-equivalence.json"])["families"]:
        if not family["prompt_length_bound_pass"]:
            raise BenchmarkError(f"prompt_length_bound_failed:{family['family_id']}")
    forbidden = re.compile(
        rb"\b(?:"
        + b"|".join(token.encode() for token in sorted(FORBIDDEN_WRAPPER_VOCABULARY))
        + rb")\b",
        re.IGNORECASE,
    )
    for path, raw in outputs.items():
        if (
            "/state-wrapper/" in path
            and ("/packet/" in path or path.endswith("/input/prompt.txt"))
            and forbidden.search(raw)
        ):
            raise BenchmarkError(f"state_wrapper_forbidden_vocabulary:{path}")


def validate_response(
    response: Any, family: dict[str, Any], packet_manifest: dict[str, Any]
) -> dict[str, Any]:
    keys = {
        "schema",
        "family_id",
        "predecessor_claim_id",
        "successor_claim_id",
        "consequences",
        "authority_effect_code",
        "authority_action_code",
        "evidence_bindings",
    }
    if not isinstance(response, dict) or set(response) != keys:
        raise BenchmarkError("response_fields_invalid")
    if response["schema"] != "inherited-correction-held-out-response.v1":
        raise BenchmarkError("response_schema_invalid")
    if response["family_id"] != family["family_id"]:
        raise BenchmarkError("response_family_invalid")
    if response["authority_effect_code"] not in AUTHORITY_EFFECTS:
        raise BenchmarkError("response_authority_effect_invalid")
    if response["authority_action_code"] not in AUTHORITY_ACTIONS:
        raise BenchmarkError("response_authority_action_invalid")
    expected_ids = sorted(item["claim_id"] for item in family["consequences"])
    consequences = response["consequences"]
    if not isinstance(consequences, list) or len(consequences) != 4:
        raise BenchmarkError("response_consequences_invalid")
    if [item.get("claim_id") for item in consequences] != expected_ids:
        raise BenchmarkError("response_claim_order_invalid")
    for item in consequences:
        if not isinstance(item, dict) or set(item) != {
            "claim_id",
            "classification",
            "action_code",
        }:
            raise BenchmarkError("response_consequence_fields_invalid")
        if item["classification"] not in LABELS:
            raise BenchmarkError("response_classification_invalid")
        if item["action_code"] not in ACTIONS:
            raise BenchmarkError("response_action_invalid")
    known = {
        entry["path"]: entry["sha256"]
        for entry in packet_manifest["source_and_evidence"]
    }
    expected_bindings = {
        (item["evidence_path"], known[item["evidence_path"]])
        for item in family["consequences"]
    }
    bindings = response["evidence_bindings"]
    if not isinstance(bindings, list) or len(bindings) != 4:
        raise BenchmarkError("response_bindings_invalid")
    observed_bindings = []
    for binding in bindings:
        if (
            not isinstance(binding, dict)
            or set(binding) != {"path", "sha256"}
            or known.get(binding["path"]) != binding["sha256"]
        ):
            raise BenchmarkError("response_binding_not_in_packet")
        observed_bindings.append((binding["path"], binding["sha256"]))
    if len(set(observed_bindings)) != 4 or set(observed_bindings) != expected_bindings:
        raise BenchmarkError("response_consequence_bindings_incomplete")
    return response


def score_response(
    response: dict[str, Any],
    family: dict[str, Any],
    packet_manifest: dict[str, Any],
    adjudication: dict[str, Any],
) -> dict[str, Any]:
    validate_response(response, family, packet_manifest)
    expected = {item["claim_id"]: item for item in adjudication["consequences"]}
    consequence_exact = [
        item["classification"] == expected[item["claim_id"]]["classification"]
        and item["action_code"] == expected[item["claim_id"]]["action_code"]
        for item in response["consequences"]
    ]
    pair_exact = (
        response["predecessor_claim_id"] == adjudication["predecessor_claim_id"]
        and response["successor_claim_id"] == adjudication["successor_claim_id"]
    )
    authority_exact = (
        response["authority_effect_code"] == adjudication["authority_effect_code"]
        and response["authority_action_code"] == adjudication["authority_action_code"]
    )
    return {
        "exact_success": pair_exact and all(consequence_exact) and authority_exact,
        "authority_error": not authority_exact,
        "pair_exact": pair_exact,
        "correction_impact_complete": all(consequence_exact),
        "consequence_exact": consequence_exact,
    }


def canonical_number(value: Decimal, quantum: Decimal) -> float:
    return float(value.quantize(quantum, rounding=ROUND_HALF_EVEN))


def summarize_records(
    records: list[tuple[dict[str, Any], dict[str, Any] | None]],
    family_ids: list[str],
) -> dict[str, Any]:
    summaries: dict[str, Any] = {}
    for family_id in sorted(family_ids):
        summaries[family_id] = {}
        for condition in CONDITIONS:
            selected = [
                (record, score)
                for record, score in records
                if record["family_id"] == family_id and record["condition"] == condition
            ]
            if len(selected) != 4:
                raise BenchmarkError("family_condition_denominator_invalid")
            restricted = [
                record["duration_seconds"]
                if score and score["exact_success"] and record["status"] == "completed"
                else Decimal(600)
                for record, score in selected
            ]
            exact_successes = sum(
                bool(score and score["exact_success"]) for _, score in selected
            )
            complete_sessions = sum(
                bool(score and score["correction_impact_complete"])
                for _, score in selected
            )
            authority_errors = sum(
                bool(score and score["authority_error"]) for _, score in selected
            )
            summaries[family_id][condition] = {
                "sessions": len(selected),
                "exact_successes": exact_successes,
                "exact_success_rate": canonical_number(
                    Decimal(exact_successes) / Decimal(len(selected)), RATIO_QUANTUM
                ),
                "correction_impact_complete_sessions": complete_sessions,
                "correction_impact_complete_rate": canonical_number(
                    Decimal(complete_sessions) / Decimal(len(selected)), RATIO_QUANTUM
                ),
                "authority_errors": authority_errors,
                "authority_error_rate": canonical_number(
                    Decimal(authority_errors) / Decimal(len(selected)), RATIO_QUANTUM
                ),
                "restricted_mean_seconds": canonical_number(
                    sum(restricted, Decimal(0)) / Decimal(len(restricted)),
                    MEAN_QUANTUM,
                ),
                "median_tool_calls": statistics.median(
                    record["tool_calls"] for record, _ in selected
                ),
            }
    aggregate = {}
    for condition in CONDITIONS:
        values = [summaries[family][condition] for family in sorted(family_ids)]
        sessions = sum(item["sessions"] for item in values)
        exact_successes = sum(item["exact_successes"] for item in values)
        complete_sessions = sum(
            item["correction_impact_complete_sessions"] for item in values
        )
        authority_errors = sum(item["authority_errors"] for item in values)
        aggregate[condition] = {
            "sessions": sessions,
            "exact_successes": exact_successes,
            "exact_success_rate": canonical_number(
                Decimal(exact_successes) / Decimal(sessions), RATIO_QUANTUM
            ),
            "correction_impact_complete_sessions": complete_sessions,
            "correction_impact_complete_rate": canonical_number(
                Decimal(complete_sessions) / Decimal(sessions), RATIO_QUANTUM
            ),
            "authority_errors": authority_errors,
            "authority_error_rate": canonical_number(
                Decimal(authority_errors) / Decimal(sessions), RATIO_QUANTUM
            ),
            "restricted_mean_seconds": canonical_number(
                sum(
                    Decimal(str(item["restricted_mean_seconds"])) * Decimal(4)
                    for item in values
                )
                / Decimal(12),
                MEAN_QUANTUM,
            ),
        }

    comparisons = {
        "structure": ("state-wrapper", "git-documents"),
        "governance_inheritance": ("vela", "state-wrapper"),
        "total": ("vela", "git-documents"),
    }

    def estimand(
        values: dict[str, Any], treatment: str, control: str
    ) -> dict[str, Any]:
        treated = values[treatment]
        baseline = values[control]
        return {
            "treatment": treatment,
            "control": control,
            "exact_success_lift": treated["exact_successes"]
            - baseline["exact_successes"],
            "correction_impact_complete_lift": treated[
                "correction_impact_complete_sessions"
            ]
            - baseline["correction_impact_complete_sessions"],
            "authority_safety_lift": baseline["authority_errors"]
            - treated["authority_errors"],
            "exact_success_rate_lift": canonical_number(
                Decimal(str(treated["exact_success_rate"]))
                - Decimal(str(baseline["exact_success_rate"])),
                RATIO_QUANTUM,
            ),
            "correction_impact_complete_rate_lift": canonical_number(
                Decimal(str(treated["correction_impact_complete_rate"]))
                - Decimal(str(baseline["correction_impact_complete_rate"])),
                RATIO_QUANTUM,
            ),
            "authority_error_rate_reduction": canonical_number(
                Decimal(str(baseline["authority_error_rate"]))
                - Decimal(str(treated["authority_error_rate"])),
                RATIO_QUANTUM,
            ),
            "restricted_seconds_saved": canonical_number(
                Decimal(str(baseline["restricted_mean_seconds"]))
                - Decimal(str(treated["restricted_mean_seconds"])),
                MEAN_QUANTUM,
            ),
            "restricted_time_ratio": canonical_number(
                Decimal(str(treated["restricted_mean_seconds"]))
                / Decimal(str(baseline["restricted_mean_seconds"])),
                RATIO_QUANTUM,
            ),
        }

    family_estimands = {}
    family_gates = {}
    for family_id in sorted(family_ids):
        values = summaries[family_id]
        family_estimands[family_id] = {
            name: estimand(values, *arms) for name, arms in comparisons.items()
        }
        git = values["git-documents"]
        wrapper = values["state-wrapper"]
        vela = values["vela"]
        structure = family_estimands[family_id]["structure"]
        governance = family_estimands[family_id]["governance_inheritance"]
        total = family_estimands[family_id]["total"]
        family_gates[family_id] = {
            "structure": all(
                [
                    wrapper["exact_successes"] >= 3,
                    wrapper["exact_successes"] >= git["exact_successes"],
                    wrapper["correction_impact_complete_sessions"] >= 3,
                    wrapper["correction_impact_complete_sessions"]
                    >= git["correction_impact_complete_sessions"],
                    wrapper["authority_errors"] == 0,
                    structure["restricted_time_ratio"] <= 0.8,
                ]
            ),
            "governance_inheritance_noninferior": all(
                [
                    vela["exact_successes"] >= wrapper["exact_successes"],
                    vela["correction_impact_complete_sessions"]
                    >= wrapper["correction_impact_complete_sessions"],
                    vela["authority_errors"] <= wrapper["authority_errors"],
                    governance["restricted_time_ratio"] <= 1.0,
                ]
            ),
            "total": all(
                [
                    vela["exact_successes"] >= 3,
                    vela["exact_successes"] >= git["exact_successes"],
                    vela["correction_impact_complete_sessions"] >= 3,
                    vela["correction_impact_complete_sessions"]
                    >= git["correction_impact_complete_sessions"],
                    vela["authority_errors"] == 0,
                    total["restricted_time_ratio"] <= 0.8,
                ]
            ),
        }

    aggregate_estimands = {
        name: estimand(aggregate, *arms) for name, arms in comparisons.items()
    }
    structure = aggregate_estimands["structure"]
    governance = aggregate_estimands["governance_inheritance"]
    total = aggregate_estimands["total"]
    governance_strict_increment = any(
        [
            governance["exact_success_lift"] >= 2,
            governance["correction_impact_complete_lift"] >= 2,
            governance["authority_safety_lift"] >= 2,
            governance["restricted_time_ratio"] <= 0.9,
        ]
    )
    gates = {
        "structure": all(
            [
                all(item["structure"] for item in family_gates.values()),
                aggregate["state-wrapper"]["exact_successes"] >= 9,
                aggregate["state-wrapper"]["correction_impact_complete_sessions"] >= 9,
                aggregate["state-wrapper"]["authority_errors"] == 0,
                structure["restricted_time_ratio"] <= 0.8,
            ]
        ),
        "governance_inheritance": all(
            [
                all(
                    item["governance_inheritance_noninferior"]
                    for item in family_gates.values()
                ),
                aggregate["vela"]["exact_successes"] >= 9,
                aggregate["vela"]["correction_impact_complete_sessions"] >= 9,
                aggregate["vela"]["authority_errors"] == 0,
                governance_strict_increment,
            ]
        ),
        "total": all(
            [
                all(item["total"] for item in family_gates.values()),
                aggregate["vela"]["exact_successes"] >= 9,
                aggregate["vela"]["correction_impact_complete_sessions"] >= 9,
                aggregate["vela"]["authority_errors"] == 0,
                total["restricted_time_ratio"] <= 0.8,
            ]
        ),
    }
    return {
        "families": summaries,
        "family_estimands": family_estimands,
        "family_gates": family_gates,
        "aggregate": aggregate,
        "aggregate_estimands": aggregate_estimands,
        "governance_strict_increment": governance_strict_increment,
        "gates": gates,
        "positive": all(gates.values()),
    }


def load_custody() -> Any:
    path = ROOT / "custody.py"
    spec = importlib.util.spec_from_file_location("held_out_custody", path)
    if spec is None or spec.loader is None:
        raise BenchmarkError("custody_import_failed")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def capture_manifest(runs_dir: Path) -> dict[str, Any]:
    custody = load_custody().complete_custody(runs_dir)
    value = {
        "schema": "vela.inherited-correction-held-out-capture.v1",
        "registration_root": load_json(ROOT / "preregistration.json")[
            "registration_root"
        ],
        "complete_custody_root": custody["complete_custody_root"],
        "runs": custody["runs"],
        "adjudication_accessed": False,
    }
    value["capture_root"] = canonical_root(value)
    return value


def verify_capture(runs_dir: Path) -> dict[str, Any]:
    path = runs_dir / "capture-manifest.json"
    if not path.is_file():
        raise BenchmarkError("capture_manifest_missing")
    observed = load_json(path)
    expected = capture_manifest(runs_dir)
    if observed != expected:
        raise BenchmarkError("capture_manifest_drift")
    return observed


def read_score_snapshot_file(path: Path, label: str) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise BenchmarkError(f"score_{label}_missing_or_unsafe") from error
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise BenchmarkError(f"score_{label}_missing_or_unsafe")
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            return handle.read()
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def capture_bound_score_snapshot(
    runs_dir: Path, capture: dict[str, Any]
) -> tuple[dict[str, Any], tuple[tuple[bytes, bytes | None], ...]]:
    entries = capture.get("runs")
    if not isinstance(entries, list) or len(entries) != 36:
        raise BenchmarkError("score_snapshot_denominator_invalid")
    families = family_map()
    seen = set()
    snapshot_entries = []
    snapshot_bytes = []
    for entry in entries:
        if not isinstance(entry, dict):
            raise BenchmarkError("score_snapshot_entry_invalid")
        run_id = entry.get("run_id")
        if (
            not isinstance(run_id, str)
            or not run_id
            or Path(run_id).name != run_id
            or run_id in seen
        ):
            raise BenchmarkError("score_snapshot_run_identity_invalid")
        seen.add(run_id)
        run_dir = runs_dir / run_id
        run_raw = read_score_snapshot_file(run_dir / "run.json", "run")
        if byte_digest(run_raw) != entry.get("run_bytes"):
            raise BenchmarkError("score_run_snapshot_drift")
        try:
            record = json.loads(run_raw, parse_float=Decimal)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BenchmarkError("score_run_snapshot_invalid") from error
        if any(
            record.get(key) != entry.get(key)
            for key in ("run_id", "family_id", "condition")
        ):
            raise BenchmarkError("score_run_identity_drift")
        family = families.get(record.get("family_id"))
        if family is None or record.get("condition") not in CONDITIONS:
            raise BenchmarkError("score_run_assignment_invalid")
        response_path = run_dir / "response.json"
        expected_response = entry.get("response_bytes")
        if expected_response is None:
            if response_path.exists() or response_path.is_symlink():
                raise BenchmarkError("score_unregistered_response")
            response_raw = None
        else:
            response_raw = read_score_snapshot_file(response_path, "response")
            if byte_digest(response_raw) != expected_response:
                raise BenchmarkError("score_response_snapshot_drift")
            try:
                response = json.loads(response_raw)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise BenchmarkError("score_response_snapshot_invalid") from error
            manifest = json.loads(
                packet_files(family, record["condition"])["PACKET-MANIFEST.json"]
            )
            validate_response(response, family, manifest)
        snapshot_entry = dict(entry)
        snapshot_entry["run_bytes"] = byte_digest(run_raw)
        snapshot_entry["response_bytes"] = (
            byte_digest(response_raw) if response_raw is not None else None
        )
        snapshot_entries.append(snapshot_entry)
        snapshot_bytes.append((run_raw, response_raw))
    snapshot_capture = dict(capture)
    claimed_root = snapshot_capture.pop("capture_root", None)
    snapshot_capture["runs"] = snapshot_entries
    derived_root = canonical_root(snapshot_capture)
    if claimed_root != derived_root:
        raise BenchmarkError("score_snapshot_capture_root_drift")
    snapshot_capture["capture_root"] = derived_root
    return snapshot_capture, tuple(snapshot_bytes)


def score_runs(runs_dir: Path, adjudication_path: Path) -> dict[str, Any]:
    verify()
    capture = verify_capture(runs_dir)
    capture, snapshot = capture_bound_score_snapshot(runs_dir, capture)
    preregistration = load_json(ROOT / "preregistration.json")
    commitment = preregistration["bindings"]["adjudication_commitment"]
    if commitment["status"] != "frozen" or not isinstance(
        commitment["adjudication_root"], str
    ):
        raise BenchmarkError("adjudication_not_frozen")
    raw_adjudication = adjudication_path.read_bytes()
    adjudication = json.loads(raw_adjudication)
    if canonical_root(adjudication) != commitment["adjudication_root"]:
        raise BenchmarkError("adjudication_root_drift")
    answers = {item["family_id"]: item for item in adjudication["families"]}
    families = family_map()
    records = []
    for run_raw, response_raw in snapshot:
        record = json.loads(run_raw, parse_float=Decimal)
        family = families[record["family_id"]]
        manifest = json.loads(
            packet_files(family, record["condition"])["PACKET-MANIFEST.json"]
        )
        score = (
            score_response(
                json.loads(response_raw),
                family,
                manifest,
                answers[record["family_id"]],
            )
            if response_raw is not None
            else None
        )
        records.append((record, score))
    summary = summarize_records(records, sorted(families))
    return {
        "schema": "vela.inherited-correction-held-out-scored-result.v1",
        "registration_root": preregistration["registration_root"],
        "capture_root": capture["capture_root"],
        "adjudication_root": commitment["adjudication_root"],
        "fixed_denominator": 36,
        "families": summary["families"],
        "family_gates": summary["family_gates"],
        "aggregate": summary["aggregate"],
        "family_estimands": summary["family_estimands"],
        "aggregate_estimands": summary["aggregate_estimands"],
        "governance_strict_increment": summary["governance_strict_increment"],
        "gates": summary["gates"],
        "positive_gate": "pass" if summary["positive"] else "not_supported",
        "authority_effect": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("build")
    sub.add_parser("verify")
    freeze = sub.add_parser("freeze")
    freeze.add_argument("--runs-dir", type=Path, required=True)
    score = sub.add_parser("score")
    score.add_argument("--runs-dir", type=Path, required=True)
    score.add_argument("--adjudication", type=Path, required=True)
    score.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "build":
        write_outputs()
    elif args.command == "verify":
        verify()
        print("held-out inherited-correction benchmark: verified and held")
    elif args.command == "freeze":
        runs_dir = args.runs_dir.resolve()
        (runs_dir / "capture-manifest.json").write_bytes(
            json_bytes(capture_manifest(runs_dir))
        )
    elif args.command == "score":
        result = score_runs(args.runs_dir.resolve(), args.adjudication.resolve())
        args.output.write_bytes(json_bytes(result))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (BenchmarkError, OSError, ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2) from error
