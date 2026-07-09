from __future__ import annotations

import datetime as _dt
import base64
import hashlib
import importlib.resources
import json
from pathlib import Path
from typing import Any

RECEIPT_SCHEMA = "vela.receipt.v1"
INTOTO_STATEMENT_TYPE = "https://in-toto.io/Statement/v1"
VELA_PREDICATE_TYPE = "https://vela.science/receipt/v1"
INTOTO_PAYLOAD_TYPE = "application/vnd.in-toto+json"


def receipt_schema() -> dict[str, Any]:
    with importlib.resources.files(__package__).joinpath("vela.receipt.v1.schema.json").open(
        "r", encoding="utf-8"
    ) as f:
        return json.load(f)


_SCHEMA = receipt_schema()
CLAIM_TYPES = set(_SCHEMA["properties"]["type"]["enum"])
REPLAYABILITY = set(_SCHEMA["properties"]["replayability"]["enum"])
STATUS_KINDS = set(_SCHEMA["properties"]["status"]["properties"]["kind"]["enum"])
STATUS_AUTHORITIES = set(_SCHEMA["properties"]["status"]["properties"]["authority"]["enum"])
VERIFIER_OUTCOMES = set(_SCHEMA["$defs"]["verifier_run"]["properties"]["outcome"]["enum"])
REQUIRED_FIELDS = set(_SCHEMA["required"])
PROVENANCE_REQUIRED = set(_SCHEMA["properties"]["provenance"]["required"])
STATUS_REQUIRED = set(_SCHEMA["properties"]["status"]["required"])
ARTIFACT_REQUIRED = set(_SCHEMA["$defs"]["artifact"]["required"])
VERIFIER_REQUIRED = set(_SCHEMA["$defs"]["verifier_run"]["required"])
ACCEPTANCE_SCOPES = set(_SCHEMA["$defs"]["acceptance_scope"]["enum"])
ACCEPTANCE_MECHANISM = "accountable_scientific_steward_signoff"


def _utc_now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _artifact(spec: str, base_dir: Path | None) -> dict[str, Any]:
    path, kind = (spec.split(":", 1) + ["witness"])[:2] if ":" in spec else (spec, "witness")
    item: dict[str, Any] = {"path": path, "kind": kind or "witness"}
    if base_dir is not None:
        candidate = (base_dir / path).resolve()
        if candidate.exists() and candidate.is_file():
            item["sha256"] = _sha256(candidate)
    return item


def _json_digest(payload: Any) -> str:
    body = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def _subject_for_receipt(receipt: dict[str, Any]) -> list[dict[str, Any]]:
    subjects: list[dict[str, Any]] = []
    for artifact in receipt.get("artifacts", []):
        digest = artifact.get("sha256")
        if digest:
            subjects.append({
                "name": artifact.get("path", "artifact"),
                "digest": {"sha256": digest},
            })
    if not subjects:
        subjects.append({
            "name": "claim",
            "digest": {"sha256": hashlib.sha256(receipt["claim"].encode("utf-8")).hexdigest()},
        })
    return subjects


def _machine_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    verifier_runs = receipt.get("verifier_runs") or []
    status = verifier_runs[0].get("outcome", "unknown") if verifier_runs else "unknown"
    return {
        "subject": _subject_for_receipt(receipt),
        "claim": {
            "id": receipt.get("claim_id"),
            "text": receipt["claim"],
            "type": receipt["type"],
        },
        "verification": {
            "status": status,
            "verifier_runs": verifier_runs,
            "trust_base": {
                "kind": "producer-reported unless independently re-derived",
                "allowed_axioms": [],
                "toolchain": receipt.get("environment", {}).get("toolchain"),
            },
            "dependency_lock": receipt.get("environment", {}).get("dependency_lock", {}),
        },
    }


def _acceptance_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    status = receipt.get("status") or {}
    scope = status.get("scope", {}).get("acceptance_scope") or "machine_verified"
    if scope not in ACCEPTANCE_SCOPES:
        scope = "machine_verified"
    verifier_runs = receipt.get("verifier_runs") or []
    verification_status = verifier_runs[0].get("outcome", "unknown") if verifier_runs else "unknown"
    scope_data = status.get("scope", {})
    accepted_by = scope_data.get("accepted_by")
    return {
        "profile": scope_data.get("profile", "producer.default.v1"),
        "mechanism": ACCEPTANCE_MECHANISM,
        "acceptor": accepted_by,
        "policyRef": scope_data.get("policyRef", "producer emission policy"),
        "evidenceRefs": scope_data.get("evidenceRefs", []),
        "evidenceLevel": scope_data.get("evidenceLevel"),
        "artifact_verification": {
            "status": verification_status,
            "authority": "producer" if status.get("authority") == "producer" else status.get("authority"),
        },
        "claim_acceptance": {
            "status": status.get("kind", "emitted"),
            "accepted_by": accepted_by,
            "authority_scope": scope,
            "policy": scope_data.get("policyRef", "producer emission policy"),
            "rationale": status.get("note", ""),
            "accepted_at": scope_data.get("accepted_at"),
            "signatures": scope_data.get("signatures", []),
        },
        "distillation_acceptance": {
            "status": receipt.get("distillation", {}).get("status", "not_required"),
            "accepted_by": receipt.get("distillation", {}).get("accepted_by"),
            "rubric": receipt.get("distillation", {}).get("rubric", "not required for machine_verified receipts"),
        },
        "acceptance_scope": scope,
    }


def _distillation_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    existing = receipt.get("distillation")
    if isinstance(existing, dict):
        return existing
    return {
        "status": "not_required",
        "uri": None,
        "digest": None,
        "audience": "frontier reviewer",
        "level": "none",
        "accepted_by": None,
        "rubric": "Distillation is required only for frontier_accepted and canon_accepted scopes.",
        "comprehension_budget": "not applicable",
        "inheritance_note": "Producer receipt carries machine evidence only until a frontier accepts it.",
        "known_gaps": [],
        "signature_refs": [],
    }


def _lineage_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    source = (receipt.get("environment") or {}).get("source") or {}
    state_diff = receipt.get("state_diff") or {}
    return {
        "frontier": state_diff.get("frontier"),
        "parents": state_diff.get("parents", []),
        "derived_from": state_diff.get("derived_from", []),
        "supersedes": state_diff.get("supersedes", []),
        "source_refs": source.get("source_refs", []),
        "producer_run_id": source.get("run_id"),
    }


def _contributors_layer(receipt: dict[str, Any]) -> list[dict[str, Any]]:
    submitter = (receipt.get("provenance") or {}).get("submitter") or {}
    producer = submitter.get("actor") or receipt["provenance"]["generated_by"]
    return [{
        "id": producer,
        "roles": ["machine_producer", "software"],
        "credit_taxonomy": "CRediT+Vela",
        "author": False,
        "note": "Machine or relay is recorded as producer and originator, never author.",
    }]


def _signature_identities_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    source = (receipt.get("environment") or {}).get("source") or {}
    return {
        "producer": {
            "role": "producer",
            "signatureRef": None,
            "mechanism": "sigstore_keyless_oidc",
            "oidcIssuer": "https://token.actions.githubusercontent.com",
            "subject": source.get("source_uri") or receipt["provenance"]["generated_by"],
            "orcid": source.get("orcid"),
            "note": "Producer identity records origin only. It is not human acceptance.",
        }
    }


def _prov_for_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    source = (receipt.get("environment") or {}).get("source") or {}
    submitter = (receipt.get("provenance") or {}).get("submitter") or {}
    agent_id = submitter.get("actor") or source.get("system") or receipt["provenance"]["generated_by"]
    activity_id = source.get("run_id") or _json_digest({
        "claim": receipt["claim"],
        "emitted_at": receipt["provenance"]["emitted_at"],
    })[:16]
    entities = {
        "claim": {
            "prov:type": "vela:claim",
            "vela:claim": receipt["claim"],
        }
    }
    for artifact in receipt.get("artifacts", []):
        artifact_id = f"artifact:{artifact.get('path', 'unknown')}"
        entities[artifact_id] = {
            "prov:type": "vela:artifact",
            "vela:kind": artifact.get("kind"),
            "vela:sha256": artifact.get("sha256"),
        }
    return {
        "prefix": {
            "prov": "http://www.w3.org/ns/prov#",
            "vela": "https://vela.science/ns#",
        },
        "entity": entities,
        "activity": {
            f"activity:{activity_id}": {
                "prov:type": "vela:receipt-emission",
                "prov:startedAtTime": receipt["provenance"]["emitted_at"],
            }
        },
        "agent": {
            f"agent:{agent_id}": {
                "prov:type": "prov:SoftwareAgent",
                "vela:source_system": source.get("system"),
                "vela:source_uri": source.get("source_uri"),
            }
        },
        "wasAssociatedWith": {
            "_:assoc0": {
                "prov:activity": f"activity:{activity_id}",
                "prov:agent": f"agent:{agent_id}",
            }
        },
        "wasGeneratedBy": {
            "_:gen0": {
                "prov:entity": "claim",
                "prov:activity": f"activity:{activity_id}",
            }
        },
    }


def in_toto_statement_from_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    """Build the standard in-toto Statement payload for a Vela receipt.

    The returned object is the DSSE payload Vela signs at the boundary. It is
    intentionally plain JSON so a standard in-toto parser can wrap, sign, and
    verify it without importing Vela code.
    """
    source = (receipt.get("environment") or {}).get("source") or {}
    machine = receipt.get("machine") or _machine_layer(receipt)
    acceptance = receipt.get("acceptance") or _acceptance_layer(receipt)
    distillation = receipt.get("distillation") or _distillation_layer(receipt)
    lineage = receipt.get("lineage") or _lineage_layer(receipt)
    return {
        "_type": INTOTO_STATEMENT_TYPE,
        "subject": machine["subject"],
        "predicateType": VELA_PREDICATE_TYPE,
        "predicate": {
            "schema": "vela.receipt.predicate.v1",
            "machine": machine,
            "acceptance": acceptance,
            "distillation": distillation,
            "lineage": lineage,
            "contributors": receipt.get("contributors") or _contributors_layer(receipt),
            "signature_identities": receipt.get("signature_identities") or _signature_identities_layer(receipt),
            "provenance": _prov_for_receipt(receipt),
            "ro_crate": source.get("ro_crate") or (receipt.get("environment") or {}).get("ro_crate"),
        },
    }


def dsse_envelope_for_statement(statement: dict[str, Any]) -> dict[str, Any]:
    """Unsigned DSSE envelope skeleton for systems that sign outside this tool."""
    payload = json.dumps(statement, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return {
        "payloadType": INTOTO_PAYLOAD_TYPE,
        "payload": base64.b64encode(payload).decode("ascii"),
        "signatures": [],
    }


def attach_intoto(receipt: dict[str, Any]) -> dict[str, Any]:
    statement = in_toto_statement_from_receipt(receipt)
    receipt["attestation"] = {
        "format": "in-toto-statement",
        "statement": statement,
        "dsse_envelope": dsse_envelope_for_statement(statement),
        "prov": statement["predicate"]["provenance"],
        "ro_crate": statement["predicate"].get("ro_crate"),
    }
    return receipt


def emit_receipt(
    *,
    claim: str,
    artifacts: list[str],
    caveats: list[str],
    claim_type: str = "computational",
    replayability: str = "unknown",
    verifier_runs: list[dict[str, str]] | None = None,
    generated_by: str = "vela-receipt-v0",
    submitter_actor: str | None = None,
    source_system: str | None = None,
    source_uri: str | None = None,
    source_name: str | None = None,
    source_type: str | None = None,
    source_authors: list[str] | None = None,
    source_refs: list[str] | None = None,
    run_id: str | None = None,
    base_dir: str | Path | None = None,
    conditions: list[str] | None = None,
    verification_requirements: list[str] | None = None,
    state_diff: dict[str, Any] | None = None,
    include_intoto: bool = True,
) -> dict[str, Any]:
    root = Path(base_dir).resolve() if base_dir else None
    environment: dict[str, Any] = {}
    source = {k: v for k, v in {
        "system": source_system,
        "source_uri": source_uri,
        "name": source_name,
        "source_type": source_type,
        "authors": source_authors,
        "source_refs": source_refs,
        "run_id": run_id,
        "exported_at": _utc_now() if (source_system or source_uri or run_id) else None,
    }.items() if v}
    if source:
        environment["source"] = source
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "claim": claim,
        "type": claim_type,
        "replayability": replayability,
        "artifacts": [_artifact(a, root) for a in artifacts],
        "caveats": caveats,
        "verifier_runs": verifier_runs or [],
        "conditions": conditions or [],
        "verification_requirements": verification_requirements or [],
        "state_diff": state_diff or {},
        "environment": environment,
        "provenance": {
            "generated_by": generated_by,
            "emitted_at": _utc_now(),
            "submitter": {"actor": submitter_actor} if submitter_actor else {},
        },
        "status": {
            "kind": "emitted",
            "authority": "producer",
            "evidence_status": "proposed",
            "note": "Producer emission only. Vela landing and human acceptance are separate.",
            "scope": {"acceptance_scope": "machine_verified"},
        },
    }
    receipt["machine"] = _machine_layer(receipt)
    receipt["distillation"] = _distillation_layer(receipt)
    receipt["acceptance"] = _acceptance_layer(receipt)
    receipt["lineage"] = _lineage_layer(receipt)
    receipt["contributors"] = _contributors_layer(receipt)
    receipt["signature_identities"] = _signature_identities_layer(receipt)
    if include_intoto:
        attach_intoto(receipt)
    validate_receipt(receipt)
    return receipt


def validate_receipt(receipt: dict[str, Any]) -> list[str]:
    if isinstance(receipt, dict) and receipt.get("schema") == RECEIPT_SCHEMA:
        receipt.setdefault("machine", _machine_layer(receipt))
        receipt.setdefault("distillation", _distillation_layer(receipt))
        receipt.setdefault("acceptance", _acceptance_layer(receipt))
        receipt.setdefault("lineage", _lineage_layer(receipt))
        receipt.setdefault("contributors", _contributors_layer(receipt))
        receipt.setdefault("signature_identities", _signature_identities_layer(receipt))
        if "attestation" not in receipt:
            attach_intoto(receipt)
    errors: list[str] = []
    for field in sorted(REQUIRED_FIELDS):
        if field not in receipt:
            errors.append(f"{field} is required by the bundled schema")
    if receipt.get("schema") != RECEIPT_SCHEMA:
        errors.append(f"schema must be {RECEIPT_SCHEMA}")
    if not isinstance(receipt.get("claim"), str) or not receipt["claim"].strip():
        errors.append("claim must be a non-empty string")
    if receipt.get("type") not in CLAIM_TYPES:
        errors.append(f"type must be one of {', '.join(sorted(CLAIM_TYPES))}")
    if receipt.get("replayability") not in REPLAYABILITY:
        errors.append(f"replayability must be one of {', '.join(sorted(REPLAYABILITY))}")
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list):
        errors.append("artifacts must be an array")
    else:
        for i, artifact in enumerate(artifacts):
            if not isinstance(artifact, dict):
                errors.append(f"artifacts[{i}] must be an object")
                continue
            for field in sorted(ARTIFACT_REQUIRED):
                if not artifact.get(field):
                    errors.append(f"artifacts[{i}].{field} is required")
            sha = artifact.get("sha256")
            if sha is not None and (not isinstance(sha, str) or len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha)):
                errors.append(f"artifacts[{i}].sha256 must be lowercase hex sha256")
    caveats = receipt.get("caveats")
    if not isinstance(caveats, list) or not caveats or any(not isinstance(c, str) or not c.strip() for c in caveats):
        errors.append("caveats must contain at least one non-empty string")
    verifier_runs = receipt.get("verifier_runs")
    if not isinstance(verifier_runs, list):
        errors.append("verifier_runs must be an array")
    else:
        for i, run in enumerate(verifier_runs):
            if not isinstance(run, dict):
                errors.append(f"verifier_runs[{i}] must be an object")
                continue
            for field in sorted(VERIFIER_REQUIRED):
                if not run.get(field):
                    errors.append(f"verifier_runs[{i}].{field} is required")
            if run.get("outcome") not in VERIFIER_OUTCOMES:
                errors.append(f"verifier_runs[{i}].outcome must be one of {', '.join(sorted(VERIFIER_OUTCOMES))}")
    if not isinstance(receipt.get("environment"), dict):
        errors.append("environment must be an object")
    provenance = receipt.get("provenance")
    if not isinstance(provenance, dict):
        errors.append("provenance must be an object")
    else:
        for field in sorted(PROVENANCE_REQUIRED):
            if not provenance.get(field):
                errors.append(f"provenance.{field} is required")
    status = receipt.get("status")
    if not isinstance(status, dict):
        errors.append("status must be an object")
    else:
        for field in sorted(STATUS_REQUIRED):
            if not status.get(field):
                errors.append(f"status.{field} is required")
        if status.get("kind") not in STATUS_KINDS:
            errors.append(f"status.kind must be one of {', '.join(sorted(STATUS_KINDS))}")
        if status.get("authority") not in STATUS_AUTHORITIES:
            errors.append(f"status.authority must be one of {', '.join(sorted(STATUS_AUTHORITIES))}")
        if status.get("kind") == "accepted" and not isinstance(status.get("scope"), dict):
            errors.append("status.scope is required for accepted receipts")
    attestation = receipt.get("attestation")
    if attestation is not None:
        if not isinstance(attestation, dict):
            errors.append("attestation must be an object")
        else:
            statement = attestation.get("statement")
            if not isinstance(statement, dict):
                errors.append("attestation.statement must be an object")
            else:
                if statement.get("_type") != INTOTO_STATEMENT_TYPE:
                    errors.append(f"attestation.statement._type must be {INTOTO_STATEMENT_TYPE}")
                if statement.get("predicateType") != VELA_PREDICATE_TYPE:
                    errors.append(f"attestation.statement.predicateType must be {VELA_PREDICATE_TYPE}")
                if not isinstance(statement.get("subject"), list) or not statement["subject"]:
                    errors.append("attestation.statement.subject must be a non-empty array")
                if not isinstance(statement.get("predicate"), dict):
                    errors.append("attestation.statement.predicate must be an object")
            envelope = attestation.get("dsse_envelope")
            if not isinstance(envelope, dict):
                errors.append("attestation.dsse_envelope must be an object")
            else:
                if envelope.get("payloadType") != INTOTO_PAYLOAD_TYPE:
                    errors.append(f"attestation.dsse_envelope.payloadType must be {INTOTO_PAYLOAD_TYPE}")
                if not isinstance(envelope.get("payload"), str) or not envelope["payload"]:
                    errors.append("attestation.dsse_envelope.payload must be base64 text")
                if not isinstance(envelope.get("signatures"), list):
                    errors.append("attestation.dsse_envelope.signatures must be an array")
    if errors:
        raise ValueError("\n".join(errors))
    return []


def load_json(path: str | Path) -> dict[str, Any]:
    with Path(path).open("r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("receipt JSON must be an object")
    return data


def dump_json(data: dict[str, Any], path: str | Path | None = None) -> None:
    text = json.dumps(data, indent=2, sort_keys=True) + "\n"
    if path:
        Path(path).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
