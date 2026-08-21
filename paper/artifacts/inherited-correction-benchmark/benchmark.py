#!/usr/bin/env python3
"""Build, verify, capture, and score the bounded inherited-correction study."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import re
import shutil
import statistics
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
FACTS_PATH = ROOT / "fixture/public-facts.json"
ADJUDICATION_PATH = ROOT / "scoring/adjudication.json"
PREREG_SOURCE_PATH = ROOT / "preregistration-source.json"
PREREG_PATH = ROOT / "preregistration.json"
TASK_PATH = ROOT / "protocol/participant-task.md"
TEMPLATE_PATH = ROOT / "protocol/response-template.json"
EQUIVALENCE_PATH = ROOT / "input-equivalence.json"
RESULT_PATH = ROOT / "result.json"
MANIFEST_PATH = ROOT / "manifest.sha256"
GENERATED_DIRS = (ROOT / "conditions/git-documents", ROOT / "conditions/vela")
CONDITIONS = ("git-documents", "vela")
LABELS = {"affected", "unaffected", "must_reassess", "presently_unprovable"}
ACTION_CODES = {
    "retrieve_exact_site_q_source",
    "no_correction_reassessment",
    "rerun_stability_method",
    "recalculate_with_successor_factor",
}
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
RESPONSE_KEYS = {
    "schema",
    "fixture_id",
    "predecessor_claim_id",
    "successor_claim_id",
    "consequences",
    "standing_effect",
    "source_or_evidence_binding",
}
CONSEQUENCE_KEYS = {"claim_id", "classification", "action_code"}
REGISTERED_ADJUDICATION_ROOT = (
    "sha256:6b2e94c7bfce7c41353eb48cd4962243e3f177fdaccb8c7da48567d99dfca557"
)
REGISTERED_ADJUDICATION_BYTES = (
    "a7af895d0e85dafe8cd35b624dd7a54a8da583ac52421fd993431296928f3971"
)
CUSTODY_BRIDGE_PATH = (
    ROOT.parent / "inherited-correction-benchmark-execution" / "confirmatory-custody.py"
)
CUSTODY_TEST_PATH = CUSTODY_BRIDGE_PATH.with_name("test_confirmatory_custody.py")
_CUSTODY_BRIDGE: Any | None = None


class BenchmarkError(ValueError):
    """Stable benchmark contract error."""


def custody_bridge() -> Any:
    global _CUSTODY_BRIDGE
    if _CUSTODY_BRIDGE is None:
        spec = importlib.util.spec_from_file_location(
            "inherited_correction_confirmatory_custody", CUSTODY_BRIDGE_PATH
        )
        if spec is None or spec.loader is None:
            raise BenchmarkError("runtime_custody_bridge_unavailable")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _CUSTODY_BRIDGE = module
    return _CUSTODY_BRIDGE


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def canonical_root(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def byte_digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def source_files(facts: dict[str, Any]) -> dict[str, bytes]:
    paths = {
        facts["correction"]["predecessor"]["source_path"],
        facts["correction"]["successor"]["source_path"],
    }
    paths.update(claim["evidence_path"] for claim in facts["claims"])
    output: dict[str, bytes] = {}
    for relative in sorted(paths):
        path = ROOT / "fixture" / relative
        if not path.is_file() or path.is_symlink():
            raise BenchmarkError(f"source_file_invalid:{relative}")
        output[relative] = path.read_bytes()
    return output


def evidence_bindings(files: dict[str, bytes]) -> list[dict[str, Any]]:
    return [
        {"path": path, "bytes": len(data), "sha256": byte_digest(data)}
        for path, data in sorted(files.items())
    ]


def flatten_atoms(value: Any, prefix: str = "facts") -> dict[str, Any]:
    atoms: dict[str, Any] = {}
    if isinstance(value, dict):
        for key in sorted(value):
            atoms.update(flatten_atoms(value[key], f"{prefix}.{key}"))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            atoms.update(flatten_atoms(item, f"{prefix}[{index}]"))
    else:
        atoms[prefix] = value
    return atoms


def packet_root(files: dict[str, bytes]) -> str:
    entries = [
        {"path": path, "sha256": byte_digest(data), "bytes": len(data)}
        for path, data in sorted(files.items())
    ]
    return canonical_root(entries)


def packet_root_from_directory(directory: Path) -> str:
    if not directory.is_dir() or directory.is_symlink():
        raise BenchmarkError("packet_directory_invalid")
    files: dict[str, bytes] = {}
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise BenchmarkError("packet_symlink_forbidden")
        if path.is_dir():
            continue
        if not path.is_file():
            raise BenchmarkError("packet_entry_invalid")
        files[path.relative_to(directory).as_posix()] = path.read_bytes()
    return packet_root(files)


def replay_projection(facts: dict[str, Any]) -> dict[str, Any]:
    predecessor = facts["correction"]["predecessor"]["claim_id"]
    successor = facts["correction"]["successor"]["claim_id"]
    events = []
    first = {
        "sequence": 1,
        "kind": "fixture_claim_recorded",
        "claim_id": predecessor,
        "previous_event_root": None,
    }
    first["event_root"] = canonical_root(first)
    events.append(first)
    second = {
        "sequence": 2,
        "kind": "fixture_correction_observed",
        "predecessor_claim_id": predecessor,
        "successor_claim_id": successor,
        "previous_event_root": first["event_root"],
    }
    second["event_root"] = canonical_root(second)
    events.append(second)
    replay = {
        "schema": "vela.inherited-correction-replay.v1",
        "fixture_id": facts["fixture_id"],
        "events": events,
        "event_chain_root": second["event_root"],
        "derived_current_upstream_claim_id": successor,
        "replay": "matched",
        "standing_effect": facts["authority"]["standing_effect"],
        "decision_present": facts["authority"]["decision_present"],
        "does_not_establish": ["scientific truth", "acceptance", "Standing"],
    }
    validate_replay(replay, facts)
    return replay


def validate_replay(replay: dict[str, Any], facts: dict[str, Any]) -> None:
    events = replay.get("events")
    if not isinstance(events, list) or len(events) != 2:
        raise BenchmarkError("replay_event_count_invalid")
    previous = None
    for sequence, event in enumerate(events, start=1):
        if (
            event.get("sequence") != sequence
            or event.get("previous_event_root") != previous
        ):
            raise BenchmarkError("replay_event_chain_invalid")
        supplied = event.get("event_root")
        unsigned = dict(event)
        unsigned.pop("event_root", None)
        if supplied != canonical_root(unsigned):
            raise BenchmarkError("replay_event_root_mismatch")
        previous = supplied
    predecessor = facts["correction"]["predecessor"]["claim_id"]
    successor = facts["correction"]["successor"]["claim_id"]
    if events[0].get("claim_id") != predecessor:
        raise BenchmarkError("replay_predecessor_mismatch")
    if (
        events[1].get("predecessor_claim_id") != predecessor
        or events[1].get("successor_claim_id") != successor
    ):
        raise BenchmarkError("replay_correction_mismatch")
    if replay.get("event_chain_root") != previous:
        raise BenchmarkError("replay_chain_root_mismatch")
    if replay.get("derived_current_upstream_claim_id") != successor:
        raise BenchmarkError("replay_current_claim_mismatch")
    if (
        replay.get("standing_effect") != "none"
        or replay.get("decision_present") is not False
    ):
        raise BenchmarkError("replay_authority_escalation")


def derive_classifications(facts: dict[str, Any]) -> dict[str, str]:
    predecessor = facts["correction"]["predecessor"]["claim_id"]
    listed = set(facts["bounds"]["listed_consequence_claim_ids"])
    missing = set(facts["bounds"]["known_missing_claim_ids"])
    incoming: dict[str, list[dict[str, Any]]] = {claim_id: [] for claim_id in listed}
    for relation in facts["relations"]:
        if relation["source_claim_id"] in incoming:
            incoming[relation["source_claim_id"]].append(relation)
    derived: dict[str, str] = {}
    while len(derived) < len(listed):
        changed = False
        for claim_id in sorted(listed - derived.keys()):
            relations = incoming[claim_id]
            if any(
                relation["kind"] == "requires_unavailable"
                and relation["target_claim_id"] in missing
                for relation in relations
            ):
                derived[claim_id] = "presently_unprovable"
            elif any(
                relation["kind"] == "requires_exact"
                and relation["target_claim_id"] == predecessor
                for relation in relations
            ):
                derived[claim_id] = "affected"
            elif relations and all(
                relation["kind"] == "discovery_only" for relation in relations
            ):
                derived[claim_id] = "unaffected"
            elif any(
                relation["kind"] == "requires_result"
                and derived.get(relation["target_claim_id"])
                in {"affected", "must_reassess"}
                for relation in relations
            ):
                derived[claim_id] = "must_reassess"
            else:
                continue
            changed = True
        if not changed:
            raise BenchmarkError("classification_derivation_incomplete")
    return derived


def baseline_files(
    facts: dict[str, Any], files: dict[str, bytes], bindings: list[dict[str, Any]]
) -> dict[str, bytes]:
    correction = facts["correction"]
    claims = {claim["claim_id"]: claim for claim in facts["claims"]}
    history = f"""# Git history note

`{correction["successor"]["claim_id"]}` supersedes
`{correction["predecessor"]["claim_id"]}` because
{correction["reason"]} The predecessor asserted
“{correction["predecessor"]["assertion"]}” The successor asserts
“{correction["successor"]["assertion"]}”

This is a candidate-visible research fixture, not a Decision. Standing effect:
`{facts["authority"]["standing_effect"]}`. Decision present:
`{str(facts["authority"]["decision_present"]).lower()}`. Scientific validation
present: `{str(facts["authority"]["scientific_validation_present"]).lower()}`.
"""
    claim_lines = ["# Downstream claims", ""]
    for claim_id in sorted(claims):
        claim = claims[claim_id]
        claim_lines.extend(
            [
                f"## {claim_id}",
                "",
                claim["assertion"],
                "",
                f"Evidence: `{claim['evidence_path']}`",
                "",
                f"Recorded next action: {claim['next_action']}",
                "",
            ]
        )
    relation_lines = ["# Dependency notes", ""]
    for relation in sorted(facts["relations"], key=lambda item: item["relation_id"]):
        relation_lines.extend(
            [
                f"## {relation['relation_id']}",
                "",
                (
                    f"Kind: `{relation['kind']}`. Source: `{relation['source_claim_id']}`. "
                    f"Target: `{relation['target_claim_id']}`."
                ),
                "",
                relation["meaning"],
                "",
            ]
        )
    bounds = facts["bounds"]
    flat_facts = ["# Flat substantive fact index", ""]
    for fact_id, value in sorted(flatten_atoms(facts).items()):
        flat_facts.append(
            f"- `{fact_id}` = `{json.dumps(value, ensure_ascii=False, sort_keys=True)}`"
        )
    boundary = f"""# Bounds and missing information

The listed consequence Claims are: {", ".join(f"`{item}`" for item in bounds["listed_consequence_claim_ids"])}.
Relations for those listed Claims are complete: `{str(bounds["complete_relation_set_for_listed_claims"]).lower()}`.
The overall Claim set is complete: `{str(bounds["complete_claim_set"]).lower()}`.
Known missing Claims: {", ".join(f"`{item}`" for item in bounds["known_missing_claim_ids"])}.
Bounds are {bounds["max_claims"]} Claims and {bounds["max_relations"]} relations.

{facts["authority"]["instruction"]}
"""
    manifest = {
        "schema": "vela.inherited-correction-packet-manifest.v1",
        "condition": "git-documents",
        "fixture_id": facts["fixture_id"],
        "source_and_evidence": bindings,
    }
    output = {
        "README.md": (
            b"# Git and documents condition\n\nRead `TASK.md`, then inspect the ordinary "
            b"history, Claim, dependency, source, and evidence files. No network or "
            b"private predecessor context is available.\n"
        ),
        "TASK.md": TASK_PATH.read_bytes(),
        "response-template.json": TEMPLATE_PATH.read_bytes(),
        "HISTORY.md": history.encode(),
        "CLAIMS.md": ("\n".join(claim_lines)).encode(),
        "DEPENDENCIES.md": ("\n".join(relation_lines)).encode(),
        "BOUNDS-AND-AUTHORITY.md": boundary.encode(),
        "FACT-INDEX.md": ("\n".join(flat_facts) + "\n").encode(),
        "PACKET-MANIFEST.json": json_bytes(manifest),
    }
    output.update(files)
    return output


def vela_files(
    facts: dict[str, Any], files: dict[str, bytes], bindings: list[dict[str, Any]]
) -> dict[str, bytes]:
    claims = {claim["claim_id"]: claim for claim in facts["claims"]}
    binding_by_path = {binding["path"]: binding for binding in bindings}
    projection = {
        "schema": "vela.inherited-correction-read-projection.v1",
        "fixture_id": facts["fixture_id"],
        "scope": facts["scope"],
        "correction": facts["correction"],
        "claims": facts["claims"],
        "relations": facts["relations"],
        "bounds": facts["bounds"],
        "authority": facts["authority"],
        "source_and_evidence": bindings,
        "projection_effect": "none",
    }
    replay = replay_projection(facts)
    output = {
        "README.md": (
            b"# Vela-organized condition\n\nRead `TASK.md`, then begin with "
            b"`repository-projection.json`, `replay.json`, and the per-Claim `why/` "
            b"records. No network or private predecessor context is available.\n"
        ),
        "TASK.md": TASK_PATH.read_bytes(),
        "response-template.json": TEMPLATE_PATH.read_bytes(),
        "repository-projection.json": json_bytes(projection),
        "replay.json": json_bytes(replay),
    }
    for claim_id in sorted(claims):
        claim = claims[claim_id]
        incoming = [
            relation
            for relation in facts["relations"]
            if relation["source_claim_id"] == claim_id
        ]
        why = {
            "schema": "vela.inherited-correction-why.v1",
            "claim": claim,
            "incoming_relations": incoming,
            "evidence_binding": binding_by_path[claim["evidence_path"]],
            "standing_effect": facts["authority"]["standing_effect"],
        }
        output[f"why/{claim_id}.json"] = json_bytes(why)
    output.update(files)
    return output


def registration_root(value: dict[str, Any]) -> str:
    copy = dict(value)
    copy.pop("registration_root", None)
    return canonical_root(copy)


def generated_outputs() -> dict[str, bytes]:
    facts = load_json(FACTS_PATH)
    files = source_files(facts)
    bindings = evidence_bindings(files)
    conditions = {
        "git-documents": baseline_files(facts, files, bindings),
        "vela": vela_files(facts, files, bindings),
    }
    outputs: dict[str, bytes] = {}
    for condition, packet in conditions.items():
        for path, data in packet.items():
            outputs[f"conditions/{condition}/{path}"] = data
    atoms = flatten_atoms(facts)
    for binding in bindings:
        atoms[f"bytes.{binding['path']}.sha256"] = binding["sha256"]
        atoms[f"bytes.{binding['path']}.size"] = binding["bytes"]
    equivalence = {
        "schema": "vela.inherited-correction-input-equivalence.v1",
        "fixture_id": facts["fixture_id"],
        "atomic_fact_count": len(atoms),
        "atomic_fact_set_root": canonical_root(
            [{"id": key, "value": atoms[key]} for key in sorted(atoms)]
        ),
        "condition_packet_roots": {
            condition: packet_root(packet) for condition, packet in conditions.items()
        },
        "same_source_and_evidence_bindings": bindings,
        "proof": "Both condition renderers consume the same public-facts object and exact source/evidence byte map. The Git/documents packet carries a flat fact index; the Vela packet carries the same object as a structured projection. verify regenerates and byte-compares both packets.",
        "protected_adjudication_present_in_packets": False,
    }
    outputs["input-equivalence.json"] = json_bytes(equivalence)
    prereg = load_json(PREREG_SOURCE_PATH)
    prereg["bindings"] = {
        "public_facts_root": canonical_root(facts),
        "public_facts_bytes": byte_digest(FACTS_PATH.read_bytes()),
        "adjudication_root": REGISTERED_ADJUDICATION_ROOT,
        "participant_task_bytes": byte_digest(TASK_PATH.read_bytes()),
        "response_template_root": canonical_root(load_json(TEMPLATE_PATH)),
        "benchmark_implementation_bytes": byte_digest(Path(__file__).read_bytes()),
        "benchmark_tests_bytes": byte_digest((ROOT / "test_benchmark.py").read_bytes()),
        "runtime_custody_bridge_bytes": byte_digest(CUSTODY_BRIDGE_PATH.read_bytes()),
        "runtime_custody_tests_bytes": byte_digest(CUSTODY_TEST_PATH.read_bytes()),
        "authorization_template_root": canonical_root(
            load_json(ROOT / "protocol/run-authorization-template.json")
        ),
        "input_equivalence_root": canonical_root(equivalence),
        "condition_packet_roots": equivalence["condition_packet_roots"],
    }
    prereg["registration_root"] = registration_root(prereg)
    outputs["preregistration.json"] = json_bytes(prereg)
    amendment = {
        "schema": "vela.inherited-correction-preregistration-amendment.v1",
        **prereg["prospective_amendment"],
        "current_registration_root": prereg["registration_root"],
        "changes": [
            "replace free-text keyword action scoring with closed exact action codes",
            "retain authorization bytes and revalidate full run, packet, assignment, configuration, attempt, and time custody at freeze",
            "add fail-closed polarity, forged-root, packet-drift, assignment, configuration, attempt, timeout, duration, status, and tool-count tests",
            "require exact consumed-permit, terminal-receipt, event-stream, runtime-response, and shared-to-condition configuration custody before capture or scoring",
        ],
        "authority_effect": "none",
    }
    outputs["amendment.v1.json"] = json_bytes(amendment)
    result = {
        "schema": "vela.inherited-correction-result.v1",
        "fixture_id": facts["fixture_id"],
        "registration_root": prereg["registration_root"],
        "status": "not_run",
        "valid_sessions": 0,
        "required_sessions": prereg["assignment"]["total_sessions"],
        "authority_effect": "none",
        "claim": "No result: the authorized confirmatory study remains at zero of sixteen pending exact independent prelaunch custody review.",
        "next_gate": "Obtain exact independent PASS for the frozen fail-closed runtime custody bridge before consuming the first single-use permit.",
    }
    outputs["result.json"] = json_bytes(result)
    return outputs


def artifact_manifest() -> bytes:
    excluded = {MANIFEST_PATH.resolve()}
    entries: list[str] = []
    for path in sorted(ROOT.rglob("*")):
        if (
            not path.is_file()
            or path.resolve() in excluded
            or "__pycache__" in path.parts
        ):
            continue
        relative = path.relative_to(ROOT).as_posix()
        file_hash = (
            REGISTERED_ADJUDICATION_BYTES
            if path == ADJUDICATION_PATH
            else hashlib.sha256(path.read_bytes()).hexdigest()
        )
        entries.append(f"{file_hash}  {relative}")
    return ("\n".join(entries) + "\n").encode()


def write_outputs() -> None:
    for directory in GENERATED_DIRS:
        if directory.exists():
            shutil.rmtree(directory)
    for relative, data in generated_outputs().items():
        path = ROOT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    MANIFEST_PATH.write_bytes(artifact_manifest())


def verify() -> None:
    expected = generated_outputs()
    for relative, data in expected.items():
        path = ROOT / relative
        if not path.is_file() or path.read_bytes() != data:
            raise BenchmarkError(f"generated_output_drift:{relative}")
    prereg = load_json(PREREG_PATH)
    if prereg.get("registration_root") != registration_root(prereg):
        raise BenchmarkError("registration_root_mismatch")
    if not ADJUDICATION_PATH.is_file() or ADJUDICATION_PATH.is_symlink():
        raise BenchmarkError("protected_adjudication_missing_or_unsafe")
    validate_replay(
        load_json(ROOT / "conditions/vela/replay.json"), load_json(FACTS_PATH)
    )
    for directory in GENERATED_DIRS:
        for path in directory.rglob("*"):
            if (
                path.is_file()
                and hashlib.sha256(path.read_bytes()).hexdigest()
                == REGISTERED_ADJUDICATION_BYTES
            ):
                raise BenchmarkError("protected_adjudication_leaked")
    if MANIFEST_PATH.read_bytes() != artifact_manifest():
        raise BenchmarkError("manifest_drift")


def validate_response(response: Any) -> dict[str, Any]:
    if (
        not isinstance(response, dict)
        or response.get("schema") != "vela.inherited-correction-response.v1"
    ):
        raise BenchmarkError("response_schema_invalid")
    if set(response) != RESPONSE_KEYS:
        raise BenchmarkError("response_fields_invalid")
    if response.get("fixture_id") != "bounded-calibration-correction-v1":
        raise BenchmarkError("response_fixture_invalid")
    consequences = response.get("consequences")
    if not isinstance(consequences, list) or len(consequences) != 4:
        raise BenchmarkError("response_consequences_invalid")
    ids = [item.get("claim_id") for item in consequences if isinstance(item, dict)]
    expected_ids = ["aggregate-e", "installation-d", "stability-c", "yield-b"]
    if ids != expected_ids or len(set(ids)) != 4:
        raise BenchmarkError("response_claim_order_invalid")
    for item in consequences:
        if not isinstance(item, dict) or set(item) != CONSEQUENCE_KEYS:
            raise BenchmarkError("response_consequence_fields_invalid")
        if item.get("classification") not in LABELS:
            raise BenchmarkError("response_classification_invalid")
        if item.get("action_code") not in ACTION_CODES:
            raise BenchmarkError("response_action_invalid")
    for field in (
        "predecessor_claim_id",
        "successor_claim_id",
        "standing_effect",
        "source_or_evidence_binding",
    ):
        if not isinstance(response.get(field), str) or not response[field].strip():
            raise BenchmarkError(f"response_field_invalid:{field}")
    return response


def load_registered_adjudication() -> dict[str, Any]:
    raw = ADJUDICATION_PATH.read_bytes()
    if hashlib.sha256(raw).hexdigest() != REGISTERED_ADJUDICATION_BYTES:
        raise BenchmarkError("protected_adjudication_bytes_drift")
    try:
        adjudication = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BenchmarkError("protected_adjudication_invalid") from error
    if canonical_root(adjudication) != REGISTERED_ADJUDICATION_ROOT:
        raise BenchmarkError("protected_adjudication_root_drift")
    return adjudication


def score_response(
    response: dict[str, Any], adjudication: dict[str, Any]
) -> dict[str, Any]:
    response = validate_response(response)
    expected = {item["claim_id"]: item for item in adjudication["consequences"]}
    points = 0
    pair_exact = (
        response["predecessor_claim_id"] == "calibration-a-v1"
        and response["successor_claim_id"] == "calibration-a-v2"
    )
    points += 2 if pair_exact else 0
    consequence_scores = []
    all_classifications = True
    all_actions = True
    for item in response["consequences"]:
        target = expected[item["claim_id"]]
        classification_exact = item["classification"] == target["classification"]
        action_exact = item["action_code"] == target["required_action_code"]
        points += 2 if classification_exact else 0
        points += 1 if action_exact else 0
        all_classifications &= classification_exact
        all_actions &= action_exact
        consequence_scores.append(
            {
                "claim_id": item["claim_id"],
                "classification_exact": classification_exact,
                "action_exact": action_exact,
            }
        )
    standing_exact = response["standing_effect"].casefold() == "none"
    points += 2 if standing_exact else 0
    known_digests = {
        binding["sha256"]
        for binding in evidence_bindings(source_files(load_json(FACTS_PATH)))
    }
    binding_exact = any(
        digest in response["source_or_evidence_binding"] for digest in known_digests
    )
    points += 1 if binding_exact else 0
    exact_success = all(
        [pair_exact, all_classifications, all_actions, standing_exact, binding_exact]
    )
    return {
        "schema": "vela.inherited-correction-response-score.v1",
        "points": points,
        "maximum_points": 17,
        "exact_success": exact_success,
        "authority_error": not standing_exact,
        "correction_pair_exact": pair_exact,
        "source_binding_exact": binding_exact,
        "consequences": consequence_scores,
    }


def parse_time(value: str) -> datetime:
    if not isinstance(value, str):
        raise BenchmarkError("timestamp_invalid")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise BenchmarkError("timestamp_timezone_missing")
    return parsed


def now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def validate_authorization(auth: Any, prereg: dict[str, Any]) -> dict[str, Any]:
    expected_keys = {
        "schema",
        "registration_root",
        "status",
        "authorized_by",
        "authorized_at",
        "participant_class",
        "participant_configuration_root",
        "assignment_seed_commitment",
        "max_sessions",
        "assignments",
    }
    if not isinstance(auth, dict) or set(auth) != expected_keys:
        raise BenchmarkError("authorization_fields_invalid")
    if auth.get("schema") != "vela.inherited-correction-run-authorization.v1":
        raise BenchmarkError("authorization_schema_invalid")
    if auth.get("registration_root") != prereg["registration_root"]:
        raise BenchmarkError("authorization_registration_mismatch")
    if auth.get("status") != "authorized":
        raise BenchmarkError("sessions_not_authorized")
    for field in ("authorized_by", "participant_class"):
        if not isinstance(auth.get(field), str) or not auth[field].strip():
            raise BenchmarkError(f"authorization_field_invalid:{field}")
    parse_time(auth.get("authorized_at"))
    if not SHA256.fullmatch(auth.get("assignment_seed_commitment", "")):
        raise BenchmarkError("authorization_seed_commitment_invalid")
    if auth.get("max_sessions") != prereg["assignment"]["total_sessions"]:
        raise BenchmarkError("authorization_denominator_invalid")
    assignments = auth.get("assignments")
    if not isinstance(assignments, list) or len(assignments) != auth["max_sessions"]:
        raise BenchmarkError("authorization_assignments_invalid")
    assignment_keys = {"run_id", "participant_instance_id", "condition"}
    if any(
        not isinstance(item, dict) or set(item) != assignment_keys
        for item in assignments
    ):
        raise BenchmarkError("authorization_assignment_fields_invalid")
    run_ids = [item.get("run_id") for item in assignments]
    participant_ids = [item.get("participant_instance_id") for item in assignments]
    conditions = [item.get("condition") for item in assignments]
    if any(
        not isinstance(run_id, str) or not re.fullmatch(r"[A-Za-z0-9._-]+", run_id)
        for run_id in run_ids
    ):
        raise BenchmarkError("authorization_run_id_invalid")
    if any(
        not isinstance(participant_id, str) or not participant_id.strip()
        for participant_id in participant_ids
    ):
        raise BenchmarkError("authorization_participant_invalid")
    if len(set(run_ids)) != len(assignments) or len(set(participant_ids)) != len(
        assignments
    ):
        raise BenchmarkError("authorization_assignment_duplicate")
    if any(conditions.count(condition) != 8 for condition in CONDITIONS):
        raise BenchmarkError("authorization_condition_count_invalid")
    configuration_root = auth.get("participant_configuration_root", "")
    if not isinstance(configuration_root, str) or not SHA256.fullmatch(
        configuration_root
    ):
        raise BenchmarkError("authorization_configuration_root_invalid")
    return auth


def load_authorization(
    path: Path, prereg: dict[str, Any], runs_dir: Path
) -> dict[str, Any]:
    auth = validate_authorization(load_json(path), prereg)
    existing = len(list(runs_dir.glob("*/run.json"))) if runs_dir.exists() else 0
    if existing >= min(
        auth.get("max_sessions", 0), prereg["assignment"]["total_sessions"]
    ):
        raise BenchmarkError("authorized_session_limit_reached")
    return auth


def start_run(args: argparse.Namespace) -> None:
    del args
    raise BenchmarkError("runtime_custody_ingest_required")


def finish_run(args: argparse.Namespace) -> None:
    del args
    raise BenchmarkError("runtime_custody_ingest_required")


def validate_frozen_run(
    run_path: Path, prereg: dict[str, Any], equivalence: dict[str, Any]
) -> tuple[dict[str, Any], Path | None, Path]:
    run_dir = run_path.parent
    response_path = run_dir / "response.json"
    authorization_path = run_dir / "authorization.json"
    if run_dir.is_symlink() or run_path.is_symlink() or not run_path.is_file():
        raise BenchmarkError(f"run_not_frozen:{run_dir.name}")
    if not authorization_path.is_file() or authorization_path.is_symlink():
        raise BenchmarkError("authorization_record_missing")
    record = load_json(run_path)
    if record.get("schema") == "vela.inherited-correction-run.v2":
        try:
            ingested = custody_bridge().validate_ingested_run(run_dir)
        except ValueError as error:
            raise BenchmarkError(str(error)) from error
        if record != ingested["record"]:
            raise BenchmarkError("runtime_benchmark_record_not_bridge_generated")
        expected_keys = {
            "schema",
            "run_id",
            "participant_instance_id",
            "participant_configuration_root",
            "condition_runtime_configuration_root",
            "authorized_configuration_mapping_root",
            "condition",
            "packet_root",
            "registration_root",
            "runtime_registration_root",
            "authorization_root",
            "runtime_custody_root",
            "status",
            "started_at",
            "timeout_seconds",
            "attempt",
            "completed_at",
            "duration_seconds",
            "tool_calls",
        }
        if set(record) != expected_keys:
            raise BenchmarkError("run_fields_invalid")
        if record.get("run_id") != run_dir.name:
            raise BenchmarkError("run_directory_identity_mismatch")
        if record.get("registration_root") != prereg["registration_root"]:
            raise BenchmarkError("run_registration_mismatch")
        condition = record.get("condition")
        if condition not in CONDITIONS:
            raise BenchmarkError("condition_invalid")
        expected_packet_root = equivalence["condition_packet_roots"][condition]
        if record.get("packet_root") != expected_packet_root:
            raise BenchmarkError("run_packet_root_mismatch")
        if packet_root_from_directory(run_dir / "packet") != expected_packet_root:
            raise BenchmarkError("run_packet_bytes_mismatch")
        authorization = validate_authorization(load_json(authorization_path), prereg)
        if record.get("authorization_root") != canonical_root(authorization):
            raise BenchmarkError("run_authorization_mismatch")
        assignment = next(
            (
                item
                for item in authorization["assignments"]
                if item["run_id"] == record["run_id"]
            ),
            None,
        )
        if assignment is None:
            raise BenchmarkError("run_not_assigned")
        if assignment["participant_instance_id"] != record.get(
            "participant_instance_id"
        ):
            raise BenchmarkError("participant_assignment_mismatch")
        if assignment["condition"] != condition:
            raise BenchmarkError("condition_assignment_mismatch")
        if authorization["participant_configuration_root"] != record.get(
            "participant_configuration_root"
        ):
            raise BenchmarkError("participant_configuration_mismatch")
        if parse_time(record["started_at"]) < parse_time(
            authorization["authorized_at"]
        ):
            raise BenchmarkError("run_precedes_authorization")
        if record.get("attempt") != 1 or record.get("timeout_seconds") != 600:
            raise BenchmarkError("run_attempt_or_timeout_invalid")
        if record.get("status") not in {"completed", "timed_out", "failed"}:
            raise BenchmarkError("run_status_mismatch")
        tool_calls = record.get("tool_calls")
        if (
            isinstance(tool_calls, bool)
            or not isinstance(tool_calls, int)
            or tool_calls < 0
        ):
            raise BenchmarkError("tool_calls_invalid")
        response = ingested["response_path"]
        if response is not None:
            validate_response(load_json(response))
        return record, response, authorization_path
    if not response_path.is_file() or response_path.is_symlink():
        raise BenchmarkError(f"run_not_frozen:{run_dir.name}")
    expected_keys = {
        "schema",
        "run_id",
        "participant_instance_id",
        "participant_configuration_root",
        "condition",
        "packet_root",
        "registration_root",
        "authorization_root",
        "status",
        "started_at",
        "timeout_seconds",
        "attempt",
        "completed_at",
        "duration_seconds",
        "tool_calls",
    }
    if not isinstance(record, dict) or set(record) != expected_keys:
        raise BenchmarkError("run_fields_invalid")
    if record.get("schema") != "vela.inherited-correction-run.v1":
        raise BenchmarkError("run_schema_invalid")
    if record.get("run_id") != run_dir.name:
        raise BenchmarkError("run_directory_identity_mismatch")
    if record.get("registration_root") != prereg["registration_root"]:
        raise BenchmarkError("run_registration_mismatch")
    condition = record.get("condition")
    if condition not in CONDITIONS:
        raise BenchmarkError("condition_invalid")
    expected_packet_root = equivalence["condition_packet_roots"][condition]
    if record.get("packet_root") != expected_packet_root:
        raise BenchmarkError("run_packet_root_mismatch")
    if packet_root_from_directory(run_dir / "packet") != expected_packet_root:
        raise BenchmarkError("run_packet_bytes_mismatch")
    authorization = validate_authorization(load_json(authorization_path), prereg)
    if record.get("authorization_root") != canonical_root(authorization):
        raise BenchmarkError("run_authorization_mismatch")
    assignment = next(
        (
            item
            for item in authorization["assignments"]
            if item["run_id"] == record["run_id"]
        ),
        None,
    )
    if assignment is None:
        raise BenchmarkError("run_not_assigned")
    if assignment["participant_instance_id"] != record.get("participant_instance_id"):
        raise BenchmarkError("participant_assignment_mismatch")
    if assignment["condition"] != condition:
        raise BenchmarkError("condition_assignment_mismatch")
    if authorization["participant_configuration_root"] != record.get(
        "participant_configuration_root"
    ):
        raise BenchmarkError("participant_configuration_mismatch")
    if record.get("attempt") != 1:
        raise BenchmarkError("run_attempt_invalid")
    timeout = prereg["assignment"]["timeout_seconds"]
    if record.get("timeout_seconds") != timeout:
        raise BenchmarkError("run_timeout_mismatch")
    started = parse_time(record.get("started_at"))
    completed = parse_time(record.get("completed_at"))
    if started < parse_time(authorization["authorized_at"]):
        raise BenchmarkError("run_precedes_authorization")
    duration = record.get("duration_seconds")
    if isinstance(duration, bool) or not isinstance(duration, (int, float)):
        raise BenchmarkError("duration_invalid")
    if not math.isfinite(duration) or duration < 0:
        raise BenchmarkError("duration_invalid")
    measured = (completed - started).total_seconds()
    if abs(measured - duration) > 1e-6:
        raise BenchmarkError("duration_timestamp_mismatch")
    expected_status = "completed" if duration <= timeout else "timed_out"
    if record.get("status") != expected_status:
        raise BenchmarkError("run_status_mismatch")
    tool_calls = record.get("tool_calls")
    if (
        isinstance(tool_calls, bool)
        or not isinstance(tool_calls, int)
        or tool_calls < 0
    ):
        raise BenchmarkError("tool_calls_invalid")
    validate_response(load_json(response_path))
    return record, response_path, authorization_path


def capture_manifest(runs_dir: Path) -> dict[str, Any]:
    prereg = load_json(PREREG_PATH)
    equivalence = load_json(EQUIVALENCE_PATH)
    entries = []
    participant_ids = []
    authorization_roots = []
    configuration_roots = []
    condition_counts = {condition: 0 for condition in CONDITIONS}
    for run_path in sorted(runs_dir.glob("*/run.json")):
        record, response_path, authorization_path = validate_frozen_run(
            run_path, prereg, equivalence
        )
        condition = record.get("condition")
        if condition not in CONDITIONS:
            raise BenchmarkError("condition_invalid")
        condition_counts[condition] += 1
        participant_ids.append(record.get("participant_instance_id"))
        authorization_roots.append(record["authorization_root"])
        configuration_roots.append(record["participant_configuration_root"])
        entries.append(
            {
                "run_id": record["run_id"],
                "condition": condition,
                "run_bytes": byte_digest(run_path.read_bytes()),
                "response_bytes": (
                    byte_digest(response_path.read_bytes())
                    if response_path is not None
                    else None
                ),
                "authorization_bytes": byte_digest(authorization_path.read_bytes()),
                "registration_root": record["registration_root"],
                "packet_root": record["packet_root"],
                "authorization_root": record["authorization_root"],
                "participant_configuration_root": record[
                    "participant_configuration_root"
                ],
                "duration_seconds": record["duration_seconds"],
                "tool_calls": record["tool_calls"],
                "status": record["status"],
                "runtime_custody_root": record.get("runtime_custody_root"),
            }
        )
    required = prereg["assignment"]["total_sessions"]
    if len(entries) != required:
        raise BenchmarkError(f"fixed_denominator_incomplete:{len(entries)}/{required}")
    if len(set(participant_ids)) != required:
        raise BenchmarkError("participant_instance_reused")
    if len(set(authorization_roots)) != 1:
        raise BenchmarkError("authorization_root_not_fixed")
    if len(set(configuration_roots)) != 1:
        raise BenchmarkError("participant_configuration_not_fixed")
    if any(condition_counts[condition] != 8 for condition in CONDITIONS):
        raise BenchmarkError("condition_denominator_invalid")
    try:
        complete_custody = custody_bridge().complete_custody(runs_dir)
    except ValueError as error:
        raise BenchmarkError(str(error)) from error
    value = {
        "schema": "vela.inherited-correction-capture-manifest.v1",
        "registration_root": prereg["registration_root"],
        "condition_counts": condition_counts,
        "runs": entries,
        "complete_runtime_custody_root": complete_custody[
            "complete_runtime_custody_root"
        ],
        "adjudication_accessed": False,
    }
    value["capture_root"] = canonical_root(value)
    return value


def verify_capture_manifest(runs_dir: Path) -> None:
    path = runs_dir / "capture-manifest.json"
    if not path.is_file() or load_json(path) != capture_manifest(runs_dir):
        raise BenchmarkError("capture_manifest_missing_or_drifted")


def score_runs(runs_dir: Path) -> dict[str, Any]:
    verify_capture_manifest(runs_dir)
    prereg = load_json(PREREG_PATH)
    capture = load_json(runs_dir / "capture-manifest.json")
    adjudication = load_registered_adjudication()
    records = []
    for run_path in sorted(runs_dir.glob("*/run.json")):
        record = load_json(run_path)
        response_path = run_path.parent / "response.json"
        score = (
            score_response(load_json(response_path), adjudication)
            if response_path.is_file()
            else None
        )
        records.append((record, score))
    required = prereg["assignment"]["total_sessions"]
    if len(records) != required:
        raise BenchmarkError(f"fixed_denominator_incomplete:{len(records)}/{required}")
    if len({record["participant_instance_id"] for record, _ in records}) != required:
        raise BenchmarkError("participant_instance_reused")
    counts = {condition: 0 for condition in CONDITIONS}
    for record, _ in records:
        counts[record["condition"]] += 1
    if any(counts[condition] != 8 for condition in CONDITIONS):
        raise BenchmarkError("condition_denominator_invalid")
    summaries: dict[str, Any] = {}
    for condition in CONDITIONS:
        selected = [
            (record, score)
            for record, score in records
            if record["condition"] == condition
        ]
        exact = sum(
            bool(score and score["exact_success"] and record["status"] == "completed")
            for record, score in selected
        )
        authority_errors = sum(
            bool(score and score["authority_error"]) for _, score in selected
        )
        restricted = [
            record["duration_seconds"]
            if score and score["exact_success"] and record["status"] == "completed"
            else prereg["assignment"]["timeout_seconds"]
            for record, score in selected
        ]
        summaries[condition] = {
            "sessions": len(selected),
            "exact_successes": exact,
            "authority_errors": authority_errors,
            "restricted_mean_seconds": sum(restricted) / len(restricted),
            "median_tool_calls": statistics.median(
                record["tool_calls"] for record, _ in selected
            ),
            "points": sum(score["points"] if score else 0 for _, score in selected),
        }
    ratio = (
        summaries["vela"]["restricted_mean_seconds"]
        / summaries["git-documents"]["restricted_mean_seconds"]
    )
    positive = all(
        [
            summaries["vela"]["exact_successes"] >= 6,
            summaries["vela"]["exact_successes"]
            >= summaries["git-documents"]["exact_successes"],
            summaries["vela"]["authority_errors"] == 0,
            ratio <= 0.8,
        ]
    )
    return {
        "schema": "vela.inherited-correction-scored-result.v1",
        "registration_root": prereg["registration_root"],
        "capture_root": capture["capture_root"],
        "adjudication_root": prereg["bindings"]["adjudication_root"],
        "fixed_denominator": required,
        "conditions": summaries,
        "restricted_mean_ratio_vela_over_git_documents": ratio,
        "positive_gate": "pass" if positive else "not_supported",
        "authority_effect": "none",
        "limitations": [
            "One synthetic case cannot establish scientific truth or general productivity.",
            "The result is internal unless separately and independently reproduced.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("build")
    sub.add_parser("verify")
    score = sub.add_parser("score")
    score.add_argument("--runs-dir", type=Path, required=True)
    score.add_argument("--output", type=Path)
    freeze = sub.add_parser("freeze")
    freeze.add_argument("--runs-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "build":
        write_outputs()
    elif args.command == "verify":
        verify()
        print("inherited-correction benchmark: verified")
    elif args.command == "freeze":
        runs_dir = args.runs_dir.resolve()
        (runs_dir / "capture-manifest.json").write_bytes(
            json_bytes(capture_manifest(runs_dir))
        )
    elif args.command == "score":
        result = score_runs(args.runs_dir.resolve())
        data = json_bytes(result)
        if args.output:
            args.output.write_bytes(data)
        else:
            sys.stdout.buffer.write(data)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BenchmarkError as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2)
