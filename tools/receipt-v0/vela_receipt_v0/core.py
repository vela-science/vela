from __future__ import annotations

import datetime as _dt
import base64
import hashlib
import importlib.resources
import json
import unicodedata
from pathlib import Path
from typing import Any

from .receipt_json import canonical_json_bytes, strict_json_load_bytes, strict_json_loads

RECEIPT_SCHEMA = "vela.receipt.v1"
INTOTO_STATEMENT_TYPE = "https://in-toto.io/Statement/v1"
VELA_PREDICATE_TYPE = "https://vela.science/receipt/v1"
INTOTO_PAYLOAD_TYPE = "application/vnd.in-toto+json"
RECEIPT_BODY_BINDING_FIELD = "vela:receipt_body"
NO_ACTIVE_POLICY_REF = "urn:vela:policy:none"
MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES = 512
MAX_RESTRICTED_ARTIFACT_KIND_BYTES = 128
MAX_RESTRICTED_ARTIFACT_MEDIA_TYPE_BYTES = 255
RESTRICTED_ARTIFACT_FIELDS = {
    "path",
    "kind",
    "disclosure",
    "media_type",
    "locator_integrity",
    "availability",
}
SENSITIVE_DISCLOSURE_ALIASES = {"restricted", "classified", "private", "sealed"}


def receipt_schema() -> dict[str, Any]:
    with importlib.resources.files(__package__).joinpath("vela.receipt.v1.schema.json").open(
        "r", encoding="utf-8"
    ) as f:
        schema = strict_json_loads(f.read())
    if not isinstance(schema, dict):
        raise ValueError("bundled receipt schema must be a JSON object")
    return schema


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
    return hashlib.sha256(canonical_json_bytes(payload)).hexdigest()


def receipt_body_sha256(receipt: dict[str, Any]) -> str:
    body = {key: value for key, value in receipt.items() if key != "attestation"}
    return hashlib.sha256(canonical_json_bytes(body)).hexdigest()


def _subject_for_receipt(receipt: dict[str, Any]) -> list[dict[str, Any]]:
    subjects: list[dict[str, Any]] = []
    for artifact in receipt.get("artifacts", []):
        subject: dict[str, Any] = {"name": artifact.get("path", "artifact")}
        if artifact.get("disclosure") != "restricted":
            digest = artifact.get("sha256")
            if digest:
                subject["digest"] = {"sha256": digest}
            if artifact.get("uri"):
                subject["uri"] = artifact["uri"]
        subjects.append(subject)
    return subjects


def _machine_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    verifier_runs = receipt.get("verifier_runs") or []
    status = "producer_reported" if verifier_runs else "not_assessed"
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
                "kind": "producer_reported",
                "authority": "producer",
            },
        },
    }


def _acceptance_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    return {
        "profile": "producer.emission.v1",
        "mechanism": ACCEPTANCE_MECHANISM,
        "acceptor": None,
        "policyRef": NO_ACTIVE_POLICY_REF,
        "evidenceRefs": [],
        "artifact_verification": {
            "status": "not_assessed",
            "authority": "producer",
        },
        "claim_acceptance": {
            "status": "not_assessed",
            "accepted_by": None,
            "authority": "producer",
        },
        "distillation_acceptance": {
            "status": "not_assessed",
            "accepted_by": None,
        },
        "acceptance_scope": "hypothesis_only",
    }


def _distillation_layer(receipt: dict[str, Any]) -> dict[str, Any]:
    existing = receipt.get("distillation")
    if isinstance(existing, dict):
        return existing
    return {
        "status": "missing",
        "uri": None,
        "digest": None,
        "audience": "unspecified",
        "level": "not_assessed",
        "accepted_by": None,
        "rubric": "not_assessed",
        "comprehension_budget": "not_assessed",
        "inheritance_note": "Producer evidence has not received distillation review.",
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
    submitter = (receipt.get("provenance") or {}).get("submitter") or {}
    producer = submitter.get("actor") or receipt["provenance"]["generated_by"]
    return {
        "producer": {
            "role": "producer",
            "signatureRef": None,
            "mechanism": "sigstore_keyless_oidc",
            "subject": producer,
            "identity_assurance": "not_assessed",
            "note": "Schema-required producer identity declaration only; no signature or acceptance authority is inferred.",
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
        entity = {
            "prov:type": "vela:artifact",
            "vela:kind": artifact.get("kind"),
        }
        if artifact.get("disclosure") != "restricted" and artifact.get("sha256"):
            entity["vela:sha256"] = artifact["sha256"]
        entities[artifact_id] = entity
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
            "provenance": receipt["provenance"],
            RECEIPT_BODY_BINDING_FIELD: {"sha256": receipt_body_sha256(receipt)},
            "ro_crate": source.get("ro_crate") or (receipt.get("environment") or {}).get("ro_crate"),
        },
    }


def dsse_envelope_for_statement(statement: dict[str, Any]) -> dict[str, Any]:
    """Unsigned DSSE envelope skeleton for systems that sign outside this tool."""
    payload = canonical_json_bytes(statement)
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
        "prov": _prov_for_receipt(receipt),
        "ro_crate": statement["predicate"].get("ro_crate"),
    }
    return receipt


def attestation_binding(receipt: dict[str, Any]) -> str:
    """Validate the DSSE payload and return bound or legacy_unbound."""
    attestation = receipt.get("attestation")
    if not isinstance(attestation, dict):
        raise ValueError("attestation must be an object")
    statement = attestation.get("statement")
    envelope = attestation.get("dsse_envelope")
    if not isinstance(statement, dict) or not isinstance(envelope, dict):
        raise ValueError("attestation statement and DSSE envelope must be objects")
    if envelope.get("payloadType") != INTOTO_PAYLOAD_TYPE:
        raise ValueError(f"attestation.dsse_envelope.payloadType must be {INTOTO_PAYLOAD_TYPE}")
    payload = envelope.get("payload")
    if not isinstance(payload, str) or not payload:
        raise ValueError("attestation.dsse_envelope.payload must be base64 text")
    try:
        decoded = base64.b64decode(payload, validate=True)
        payload_statement = strict_json_load_bytes(decoded)
    except Exception as exc:
        raise ValueError(f"invalid DSSE statement payload: {exc}") from exc
    if payload_statement != statement:
        raise ValueError("DSSE payload does not match attestation.statement")

    predicate = statement.get("predicate")
    if not isinstance(predicate, dict):
        raise ValueError("attestation.statement.predicate must be an object")
    if RECEIPT_BODY_BINDING_FIELD not in predicate:
        return "legacy_unbound"

    expected = in_toto_statement_from_receipt(receipt)
    for field in ("_type", "subject", "predicateType"):
        if statement.get(field) != expected[field]:
            raise ValueError(f"attestation.statement.{field} does not match receipt body")
    for field in (
        "schema",
        "machine",
        "acceptance",
        "distillation",
        "lineage",
        "contributors",
        "signature_identities",
        "provenance",
        RECEIPT_BODY_BINDING_FIELD,
    ):
        expected_value = expected["predicate"][field]
        if predicate.get(field) != expected_value:
            raise ValueError(
                f"attestation.statement.predicate.{field} does not match receipt body"
            )
    binding = predicate[RECEIPT_BODY_BINDING_FIELD]
    if not isinstance(binding, dict) or set(binding) != {"sha256"}:
        raise ValueError(
            "attestation.statement.predicate.vela:receipt_body must contain only sha256"
        )
    return "bound"


def _safe_public_metadata_text(value: Any, path: str, byte_limit: int) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{path} must be a string")
    if not value.strip():
        raise ValueError(f"{path} must be a non-empty string")
    if (
        value != value.strip()
        or len(value.encode("utf-8")) > byte_limit
        or any(unicodedata.category(character) == "Cc" for character in value)
    ):
        raise ValueError(
            f"{path} safe-public metadata must be trimmed, control-free, "
            f"and at most {byte_limit} bytes"
        )
    return value


def _validate_restricted_opaque_locator(value: Any, path: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{path} must be a string")
    if not value.strip():
        raise ValueError(f"{path} must be a non-empty string")
    opaque_id = None
    for prefix in ("custodian:", "opaque:"):
        if value.startswith(prefix):
            opaque_id = value.removeprefix(prefix)
            break
    if opaque_id is None:
        raise ValueError(
            f"{path} restricted artifacts require a non-resolving "
            "custodian: or opaque: identifier"
        )
    if (
        not opaque_id
        or len(opaque_id.encode("utf-8")) > MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES
        or any(
            not (
                character.isascii()
                and (character.isalnum() or character in "-_.:")
            )
            for character in opaque_id
        )
    ):
        raise ValueError(
            f"{path} opaque identifier must be "
            f"1..={MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES} ASCII identifier bytes "
            "and contain no path, URL, query, fragment, whitespace, or control syntax"
        )
    return value


def validate_safe_public_artifact_descriptors(receipt: dict[str, Any]) -> None:
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list):
        raise ValueError("artifacts must be an array")
    for index, artifact in enumerate(artifacts):
        path = f"artifacts[{index}]"
        if not isinstance(artifact, dict):
            raise ValueError(f"{path} must be an object")
        disclosure = artifact.get("disclosure")
        if "disclosure" in artifact:
            if not isinstance(disclosure, str):
                raise ValueError(f"{path}.disclosure must be a string")
            if disclosure not in {"public", "restricted"}:
                raise ValueError(f"{path}.disclosure must be public or restricted")

        for alias in ("visibility", "access_tier", "accessTier"):
            alias_value = artifact.get(alias)
            if (
                isinstance(alias_value, str)
                and alias_value in SENSITIVE_DISCLOSURE_ALIASES
                and disclosure != "restricted"
            ):
                raise ValueError(
                    f"{path}.{alias} sensitive artifacts must use "
                    "disclosure: restricted"
                )

        if disclosure != "restricted":
            continue

        unexpected = sorted(set(artifact) - RESTRICTED_ARTIFACT_FIELDS)
        if unexpected:
            raise ValueError(
                f"{path}.{unexpected[0]} is not permitted in a restricted "
                "artifact's safe-public descriptor"
            )
        _validate_restricted_opaque_locator(artifact.get("path"), f"{path}.path")
        _safe_public_metadata_text(
            artifact.get("kind"),
            f"{path}.kind",
            MAX_RESTRICTED_ARTIFACT_KIND_BYTES,
        )
        if "media_type" in artifact:
            _safe_public_metadata_text(
                artifact["media_type"],
                f"{path}.media_type",
                MAX_RESTRICTED_ARTIFACT_MEDIA_TYPE_BYTES,
            )
        if "locator_integrity" in artifact:
            integrity = artifact["locator_integrity"]
            if not isinstance(integrity, str) or integrity not in {
                "immutable",
                "mutable",
                "unknown",
            }:
                raise ValueError(
                    f"{path}.locator_integrity must be one of immutable, mutable, unknown"
                )
        if "availability" in artifact:
            availability = artifact["availability"]
            if not isinstance(availability, str) or availability not in {
                "available",
                "unavailable",
                "unknown",
            }:
                raise ValueError(
                    f"{path}.availability must be one of available, unavailable, unknown"
                )

    validate_restricted_artifact_mirrors(receipt)


def validate_restricted_artifact_mirrors(receipt: dict[str, Any]) -> None:
    restricted = [
        artifact
        for artifact in receipt.get("artifacts", [])
        if isinstance(artifact, dict) and artifact.get("disclosure") == "restricted"
    ]
    if not restricted:
        return
    expected_subjects = _subject_for_receipt(receipt)
    machine = receipt.get("machine") or {}
    if machine.get("subject") != expected_subjects:
        raise ValueError(
            "machine.subject must be the artifact-derived public projection when "
            "restricted artifacts are present"
        )
    attestation = receipt.get("attestation") or {}
    statement = attestation.get("statement") or {}
    if statement.get("subject") != expected_subjects:
        raise ValueError("attestation.statement.subject leaks a restricted artifact mirror")
    predicate_machine = (statement.get("predicate") or {}).get("machine") or {}
    if predicate_machine.get("subject") != expected_subjects:
        raise ValueError(
            "attestation.statement.predicate.machine.subject leaks a restricted artifact mirror"
        )
    if "prov" not in attestation:
        entities = {}
    else:
        prov = attestation["prov"]
        if not isinstance(prov, dict):
            raise ValueError("attestation.prov must be an object")
        entities = prov["entity"] if "entity" in prov else {}
    if not isinstance(entities, dict):
        raise ValueError("attestation.prov.entity must be an object")
    for artifact in restricted:
        entity_id = f"artifact:{artifact.get('path')}"
        entity = entities.get(entity_id)
        if entity is None:
            continue
        if not isinstance(entity, dict):
            raise ValueError(f"attestation.prov.entity.{entity_id} must be an object")
        extras = sorted(set(entity) - {"prov:type", "vela:kind"})
        if extras:
            raise ValueError(
                f"attestation.prov.entity.{entity_id}.{extras[0]} is not permitted "
                "for a restricted artifact"
            )


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
            "evidence_status": "runs" if verifier_runs else "proposed",
            "note": "Producer emission only. Vela landing and human acceptance are separate.",
            "scope": {"acceptance_scope": "hypothesis_only"},
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
        if status.get("authority") == "producer" and status.get("kind") not in {"draft", "emitted"}:
            errors.append("producer authority may emit only draft or emitted status")
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
    if not errors:
        try:
            canonical_json_bytes(receipt)
            validate_safe_public_artifact_descriptors(receipt)
            attestation_binding(receipt)
        except ValueError as exc:
            errors.append(str(exc))
    if errors:
        raise ValueError("\n".join(errors))
    return []


def load_json(path: str | Path) -> dict[str, Any]:
    data = strict_json_load_bytes(Path(path).read_bytes())
    if not isinstance(data, dict):
        raise ValueError("receipt JSON must be an object")
    return data


def dump_json(data: dict[str, Any], path: str | Path | None = None) -> None:
    text = json.dumps(data, indent=2, sort_keys=True) + "\n"
    if path:
        Path(path).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
