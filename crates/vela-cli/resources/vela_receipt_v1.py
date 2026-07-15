#!/usr/bin/env python3
"""Dependency-free Receipt v1 emitter and validator.

The only optional dependency is the standard Python in-toto package, used when
validating signed DSSE envelopes. Synthetic signing, acceptance fixtures, and
campaign demos deliberately live outside this installed resource.
"""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import sys
import unicodedata
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from receipt_json import (
    canonical_receipt_json_bytes,
    strict_receipt_json_load_bytes,
    strict_receipt_json_loads,
)

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "vela.receipt.v1"
INTOTO_STATEMENT = "https://in-toto.io/Statement/v1"
PREDICATE_TYPE = "https://vela.science/receipt/v1"
PAYLOAD_TYPE = "application/vnd.in-toto+json"
RECEIPT_BODY_BINDING_FIELD = "vela:receipt_body"
FIXTURE_KIND = "vela.conformance_fixture.v1"
ACCEPTANCE_SCOPES = {
    "machine_verified",
    "human_seen",
    "locally_accepted",
    "frontier_accepted",
    "canon_accepted",
    "hypothesis_only",
    "retracted",
    "superseded",
}
CANON_SCOPES = {"frontier_accepted", "canon_accepted"}
ALLOWED_AXIOM_DENY = {"sorryAx", "unsafe", "native_decide_whole_proof", "black_box_decide"}
ACCEPTANCE_MECHANISM = "accountable_scientific_steward_signoff"
EVIDENCE_LEVELS = {
    "local_signoff",
    "consortium_reviewed",
    "journal_accepted",
    "replicated",
    "regulator_grade",
}
CONTRIBUTOR_ROLES = {
    "conceptualization",
    "data_curation",
    "formal_analysis",
    "funding_acquisition",
    "investigation",
    "methodology",
    "project_administration",
    "resources",
    "software",
    "supervision",
    "validation",
    "visualization",
    "writing_original_draft",
    "writing_review_editing",
    "machine_producer",
    "human_formalizer",
    "human_distiller",
    "reviewer",
    "acceptor",
    "profile_maintainer",
}
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
REQUIRED_FIELDS = {
    "schema",
    "claim",
    "type",
    "replayability",
    "artifacts",
    "caveats",
    "verifier_runs",
    "environment",
    "provenance",
    "status",
    "machine",
    "acceptance",
    "distillation",
    "lineage",
    "contributors",
    "signature_identities",
    "attestation",
}
CLAIM_TYPES = {"computational", "theoretical", "empirical", "negative", "contradiction"}
REPLAYABILITY = {"exact", "bounded", "approximate", "unavailable", "unknown"}
STATUS_KINDS = {
    "draft",
    "emitted",
    "proposed",
    "runs",
    "minimal_sanity_check",
    "reported_metric_rederived",
    "full_reproduction",
    "landed_pending",
    "accepted",
    "rejected",
    "superseded",
    "retracted",
    "contested",
    "failed_reproduction",
}
STATUS_AUTHORITIES = {"producer", "vela_landing", "human_key", "signed_policy"}
VERIFIER_OUTCOMES = {"pass", "fail", "error", "skipped", "unknown"}


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def sha256_json(data: Any) -> str:
    return hashlib.sha256(canonical_json(data)).hexdigest()


def canonical_json(data: Any) -> bytes:
    return canonical_receipt_json_bytes(data)


def strict_json_loads(text: str) -> Any:
    return strict_receipt_json_loads(text)


def strict_json_load_bytes(data: bytes) -> Any:
    return strict_receipt_json_load_bytes(data)


def receipt_body_sha256(receipt: dict[str, Any]) -> str:
    body = {key: value for key, value in receipt.items() if key != "attestation"}
    return hashlib.sha256(canonical_json(body)).hexdigest()


def load_json(path: str | Path) -> dict[str, Any]:
    path = Path(path)
    data = strict_json_load_bytes(path.read_bytes())
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


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


def _artifact_subjects(receipt: dict[str, Any]) -> list[dict[str, Any]]:
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


def validate_restricted_artifact_mirrors(receipt: dict[str, Any]) -> None:
    restricted = [
        artifact
        for artifact in receipt.get("artifacts", [])
        if isinstance(artifact, dict) and artifact.get("disclosure") == "restricted"
    ]
    if not restricted:
        return
    expected_subjects = _artifact_subjects(receipt)
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


def validate_safe_public_artifact_descriptors(receipt: dict[str, Any]) -> None:
    """Validate restricted artifact descriptors inside the installed bundle."""

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


def write_json(path: str | Path, data: dict[str, Any]) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def artifact(path: Path, kind: str, base: Path) -> dict[str, Any]:
    rel = path if not path.is_absolute() else path.relative_to(base)
    item = {"path": rel.as_posix(), "kind": kind}
    actual = base / rel
    if actual.exists() and actual.is_file():
        item["sha256"] = sha256_file(actual)
    return item


def distillation_block(
    *,
    uri: str | None = None,
    digest: str | None = None,
    status: str = "not_required",
    accepted_by: str | None = None,
    audience: str = "frontier reviewer",
    level: str = "review note",
    rubric: str = "statement faithful, caveated, replay command present",
) -> dict[str, Any]:
    return {
        "status": status,
        "uri": uri,
        "digest": {"sha256": digest} if digest else None,
        "audience": audience,
        "level": level,
        "accepted_by": accepted_by,
        "rubric": rubric,
        "comprehension_budget": "one page",
        "inheritance_note": "Consumers inherit the pointer and digest, not inline prose.",
        "known_gaps": [],
        "signature_refs": [],
    }


def default_contributors(
    *,
    producer: str,
    acceptor: str | None = None,
    distiller: str | None = None,
) -> list[dict[str, Any]]:
    contributors: list[dict[str, Any]] = [{
        "id": producer,
        "roles": ["machine_producer", "software"],
        "credit_taxonomy": "CRediT+Vela",
        "author": False,
        "note": "Machine or relay is recorded as producer and originator, never author.",
    }]
    if distiller:
        contributors.append({
            "id": distiller,
            "roles": ["human_distiller", "writing_original_draft"],
            "credit_taxonomy": "CRediT+Vela",
            "author": True,
        })
    if acceptor:
        contributors.append({
            "id": acceptor,
            "roles": ["reviewer", "acceptor"],
            "credit_taxonomy": "CRediT+Vela",
            "author": False,
        })
    return contributors


def default_signature_identities(
    *, producer: str, include_acceptor: bool, fixture: bool = False
) -> dict[str, Any]:
    identities: dict[str, Any] = {
        "producer": {
            "role": "producer",
            "signatureRef": None,
            "mechanism": (
                "sigstore_keyless_oidc_fixture"
                if fixture
                else "sigstore_keyless_oidc"
            ),
            "subject": producer,
            "note": (
                "Conformance-only producer identity; no scientific authority."
                if fixture
                else "Producer identity declaration only; no signature or acceptance authority is inferred."
            ),
        }
    }
    if include_acceptor:
        identities["acceptor"] = {
            "role": "acceptor",
            "signatureRef": None,
            "mechanism": (
                "ed25519_key_custody_ceremony_fixture"
                if fixture
                else "ed25519_key_custody_ceremony"
            ),
            "subject": "reviewer:conformance-fixture",
            "note": (
                "Conformance-only acceptor identity; no human key was used."
                if fixture
                else "Declared acceptor identity; verification requires the referenced human-key signature."
            ),
        }
    return identities


def make_receipt(
    *,
    claim_id: str,
    claim: str,
    claim_type: str,
    replayability: str,
    artifacts: list[dict[str, Any]],
    verifier_runs: list[dict[str, Any]],
    caveats: list[str],
    generated_by: str,
    submitter: str,
    acceptance_scope: str,
    acceptance_status: str,
    acceptance_authority: str,
    acceptance_profile: str = "vela.frontier.sidon.v1",
    policy_ref: str = "docs/RECEIPT_GOVERNANCE.md#signing-and-acceptance",
    evidence_refs: list[str] | None = None,
    evidence_level: str | None = "local_signoff",
    distillation: dict[str, Any],
    lineage: dict[str, Any],
    contributors: list[dict[str, Any]] | None = None,
    environment: dict[str, Any] | None = None,
    provenance_extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    evidence_refs = evidence_refs or []
    canon_scope = acceptance_scope in CANON_SCOPES
    producer_neutral = acceptance_authority == "producer"
    if producer_neutral and acceptance_scope != "hypothesis_only":
        raise ValueError("producer receipts must use hypothesis_only acceptance scope")
    acceptor = acceptance_status if canon_scope else None
    machine_status = (
        "producer_reported"
        if producer_neutral and verifier_runs
        else "not_assessed"
        if producer_neutral
        else verifier_runs[0].get("outcome", "unknown")
        if verifier_runs
        else "unknown"
    )
    trust_base = (
        {"kind": "producer_reported", "authority": "producer"}
        if producer_neutral
        else {
            "kind": "frozen exact lane",
            "allowed_axioms": [],
            "toolchain": "Vela verifier",
        }
    )
    artifact_verification = (
        {"status": "not_assessed", "authority": "producer"}
        if producer_neutral
        else {
            "status": verifier_runs[0].get("outcome", "unknown") if verifier_runs else "unknown",
            "authority": "frozen verifier",
        }
    )
    claim_acceptance = (
        {"status": "not_assessed", "accepted_by": None, "authority": "producer"}
        if producer_neutral
        else {
            "status": acceptance_status,
            "accepted_by": acceptor,
            "authority_scope": acceptance_scope,
            "policy": policy_ref,
            "rationale": "scope-bound receipt validation",
            "accepted_at": utc_now() if canon_scope else None,
            "signatures": [],
        }
    )
    distillation_acceptance = (
        {"status": "not_assessed", "accepted_by": None}
        if producer_neutral
        else {
            "status": distillation.get("status"),
            "accepted_by": distillation.get("accepted_by"),
            "rubric": distillation.get("rubric"),
        }
    )
    receipt = {
        "schema": SCHEMA,
        "claim_id": claim_id,
        "claim": claim,
        "type": claim_type,
        "replayability": replayability,
        "artifacts": artifacts,
        "caveats": caveats,
        "verifier_runs": verifier_runs,
        "conditions": [],
        "verification_requirements": [],
        "state_diff": {},
        "environment": environment or {},
        "provenance": {
            "generated_by": generated_by,
            "emitted_at": utc_now(),
            "submitter": {"actor": submitter},
            **(provenance_extra or {}),
        },
        "status": {
            "kind": "accepted" if canon_scope else "emitted",
            "authority": acceptance_authority,
            "evidence_status": (
                "accepted"
                if canon_scope
                else "runs"
                if producer_neutral and verifier_runs
                else "proposed"
            ),
            "note": "Receipt-v1 separates machine verification, human acceptance, and distillation acceptance.",
            "scope": {"acceptance_scope": acceptance_scope, "accepted_by": acceptor},
        },
        "machine": {
            "subject": _artifact_subjects({"artifacts": artifacts}),
            "claim": {"id": claim_id, "text": claim, "type": claim_type},
            "verification": {
                "status": machine_status,
                "verifier_runs": verifier_runs,
                "trust_base": trust_base,
                "dependency_lock": {},
            },
        },
        "acceptance": {
            "profile": acceptance_profile,
            "mechanism": ACCEPTANCE_MECHANISM,
            "acceptor": acceptor,
            "policyRef": policy_ref,
            "evidenceRefs": evidence_refs,
            "evidenceLevel": evidence_level,
            "artifact_verification": artifact_verification,
            "claim_acceptance": claim_acceptance,
            "distillation_acceptance": distillation_acceptance,
            "acceptance_scope": acceptance_scope,
        },
        "distillation": distillation,
        "lineage": lineage,
        "contributors": contributors or default_contributors(
            producer=submitter,
            acceptor=acceptor,
            distiller=distillation.get("accepted_by"),
        ),
        "signature_identities": default_signature_identities(
            producer=submitter,
            include_acceptor=acceptance_scope in CANON_SCOPES,
        ),
    }
    attach_statement(receipt)
    return receipt


def emit_receipt(
    *,
    claim: str,
    artifacts: list[str],
    caveats: list[str],
    claim_type: str = "computational",
    replayability: str = "unknown",
    verifier_runs: list[dict[str, Any]] | None = None,
    generated_by: str = "vela-receipt-v1",
    submitter_actor: str | None = None,
    source_system: str | None = None,
    source_uri: str | None = None,
    run_id: str | None = None,
    base_dir: str | Path | None = None,
    conditions: list[str] | None = None,
    verification_requirements: list[str] | None = None,
    state_diff: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build one neutral producer Receipt v1 through the canonical core."""

    root = Path(base_dir or ".").resolve()
    artifact_records: list[dict[str, Any]] = []
    for spec in artifacts:
        path_text, separator, kind = spec.rpartition(":")
        if not separator:
            path_text, kind = spec, "witness"
        artifact_records.append(artifact(Path(path_text), kind or "witness", root))

    source = {
        key: value
        for key, value in {
            "system": source_system,
            "source_uri": source_uri,
            "run_id": run_id,
            "exported_at": utc_now()
            if source_system or source_uri or run_id
            else None,
        }.items()
        if value is not None
    }
    lineage_state = state_diff or {}
    receipt = make_receipt(
        claim_id=f"producer:{sha256_text(claim)[:24]}",
        claim=claim,
        claim_type=claim_type,
        replayability=replayability,
        artifacts=artifact_records,
        verifier_runs=verifier_runs or [],
        caveats=caveats,
        generated_by=generated_by,
        submitter=submitter_actor or generated_by,
        acceptance_scope="hypothesis_only",
        acceptance_status="not_assessed",
        acceptance_authority="producer",
        acceptance_profile="producer.emission.v1",
        policy_ref="urn:vela:policy:none",
        evidence_level=None,
        distillation=distillation_block(
            status="missing",
            audience="unspecified",
            level="not_assessed",
            rubric="not_assessed",
        ),
        lineage={
            "frontier": lineage_state.get("frontier"),
            "parents": lineage_state.get("parents", []),
            "derived_from": lineage_state.get("derived_from", []),
            "supersedes": lineage_state.get("supersedes", []),
            "source_refs": source.get("source_refs", []),
            "producer_run_id": run_id,
        },
        environment={"source": source} if source else {},
    )
    receipt["conditions"] = conditions or []
    receipt["verification_requirements"] = verification_requirements or []
    receipt["state_diff"] = lineage_state
    attach_statement(receipt)
    errors = validate_receipt(receipt)
    if errors:
        raise ValueError("; ".join(errors))
    return receipt


def prov_for_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    agent_id = receipt["provenance"].get("submitter", {}).get("actor") or receipt["provenance"]["generated_by"]
    activity_id = receipt.get("claim_id") or sha256_text(receipt["claim"])[:16]
    entities: dict[str, Any] = {
        "claim": {"prov:type": "vela:claim", "vela:claim": receipt["claim"]},
    }
    for item in receipt.get("artifacts", []):
        entity = {
            "prov:type": "vela:artifact",
            "vela:kind": item.get("kind"),
        }
        if item.get("disclosure") != "restricted" and item.get("sha256"):
            entity["vela:sha256"] = item["sha256"]
        entities[f"artifact:{item['path']}"] = entity
    dist = receipt.get("distillation") or {}
    if dist.get("uri"):
        entities["distillation"] = {
            "prov:type": "vela:distillation",
            "prov:location": dist.get("uri"),
            "vela:sha256": (dist.get("digest") or {}).get("sha256"),
        }
    return {
        "prefix": {
            "prov": "http://www.w3.org/ns/prov#",
            "vela": "https://vela.science/ns#",
        },
        "entity": entities,
        "activity": {
            f"activity:{activity_id}": {
                "prov:type": "vela:receipt-v1-emission",
                "prov:startedAtTime": receipt["provenance"]["emitted_at"],
            }
        },
        "agent": {f"agent:{agent_id}": {"prov:type": "prov:SoftwareAgent"}},
        "wasAssociatedWith": {
            "_:assoc0": {"prov:activity": f"activity:{activity_id}", "prov:agent": f"agent:{agent_id}"}
        },
        "wasGeneratedBy": {"_:gen0": {"prov:entity": "claim", "prov:activity": f"activity:{activity_id}"}},
    }


def statement_from_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    predicate = {
        "schema": "vela.receipt.predicate.v1",
        "machine": receipt["machine"],
        "acceptance": receipt["acceptance"],
        "distillation": receipt["distillation"],
        "lineage": receipt["lineage"],
        "contributors": receipt.get("contributors", []),
        "signature_identities": receipt.get("signature_identities", {}),
        "provenance": receipt["provenance"],
        RECEIPT_BODY_BINDING_FIELD: {"sha256": receipt_body_sha256(receipt)},
        "ro_crate": (receipt.get("environment") or {}).get("ro_crate"),
    }
    if receipt.get("fixture") is not None:
        predicate["fixture"] = receipt["fixture"]
    return {
        "_type": INTOTO_STATEMENT,
        "subject": receipt["machine"]["subject"],
        "predicateType": PREDICATE_TYPE,
        "predicate": predicate,
    }


def unsigned_envelope(statement: dict[str, Any]) -> dict[str, Any]:
    return {
        "payloadType": PAYLOAD_TYPE,
        "payload": base64.b64encode(canonical_json(statement)).decode("ascii"),
        "signatures": [],
    }


def attach_statement(receipt: dict[str, Any]) -> None:
    statement = statement_from_receipt(receipt)
    receipt["attestation"] = {
        "format": "in-toto-statement",
        "statement": statement,
        "dsse_envelope": unsigned_envelope(statement),
        "prov": prov_for_receipt(receipt),
        "ro_crate": statement["predicate"].get("ro_crate"),
    }


def attestation_binding(receipt: dict[str, Any]) -> str:
    attestation = receipt.get("attestation")
    if not isinstance(attestation, dict):
        raise ValueError("attestation must be an object")
    statement = attestation.get("statement")
    envelope = attestation.get("dsse_envelope")
    if not isinstance(statement, dict) or not isinstance(envelope, dict):
        raise ValueError("attestation statement and DSSE envelope must be objects")
    payload = envelope.get("payload")
    if not isinstance(payload, str) or not payload:
        raise ValueError("attestation.dsse_envelope.payload must be base64 text")
    try:
        payload_statement = strict_json_load_bytes(base64.b64decode(payload, validate=True))
    except Exception as exc:
        raise ValueError(f"invalid DSSE statement payload: {exc}") from exc
    if payload_statement != statement:
        raise ValueError("DSSE payload does not match attestation.statement")
    predicate = statement.get("predicate")
    if not isinstance(predicate, dict):
        raise ValueError("attestation.statement.predicate must be an object")
    if RECEIPT_BODY_BINDING_FIELD not in predicate:
        raise ValueError(
            "attestation.statement.predicate.vela:receipt_body is required"
        )
    expected = statement_from_receipt(receipt)
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


def require_intoto() -> tuple[Any, Any]:
    try:
        from in_toto.models.metadata import Envelope
        from securesystemslib.signer import CryptoSigner
    except Exception as exc:
        raise RuntimeError("standard in-toto package is required for this check") from exc
    return Envelope, CryptoSigner


def standard_verify(receipt: dict[str, Any], require_signature: bool) -> dict[str, Any]:
    statement = receipt.get("attestation", {}).get("statement")
    if not isinstance(statement, dict):
        raise ValueError("attestation.statement missing")
    if statement.get("_type") != INTOTO_STATEMENT:
        raise ValueError("statement is not in-toto Statement v1")
    if statement.get("predicateType") != PREDICATE_TYPE:
        raise ValueError("unexpected predicateType")
    payload = canonical_json(statement)
    envelope_data = copy.deepcopy(receipt.get("attestation", {}).get("dsse_envelope"))
    if not isinstance(envelope_data, dict):
        raise ValueError("missing DSSE envelope")
    if envelope_data.get("signatures"):
        Envelope, _ = require_intoto()
        env = Envelope.from_dict(envelope_data)
        keys = receipt.get("attestation", {}).get("fixture_public_keys")
        if not keys and receipt.get("attestation", {}).get("fixture_public_key"):
            keys = {"producer": receipt["attestation"]["fixture_public_key"]}
        if keys and receipt.get("fixture") != {
            "kind": FIXTURE_KIND,
            "authority": "none",
            "importable": False,
        }:
            raise ValueError("embedded fixture keys require a non-importable fixture marker")
        if require_signature and not keys:
            raise ValueError("signed fixture needs fixture public keys for verification")
        if keys:
            for key in keys.values():
                env.verify_signature(key)
        round_trip = strict_json_load_bytes(env.payload)
    else:
        if require_signature:
            raise ValueError("DSSE envelope has no signature")
        round_trip = strict_json_load_bytes(
            base64.b64decode(envelope_data["payload"], validate=True)
        )
    if round_trip != statement:
        raise ValueError("DSSE payload does not match statement")
    if payload != canonical_json(round_trip):
        raise ValueError("noncanonical statement round trip")
    binding = attestation_binding(receipt)
    return {
        "standard_tool": "in-toto",
        "signature_verified": bool(envelope_data.get("signatures")),
        "predicate_type": PREDICATE_TYPE,
        "subject_count": len(statement.get("subject") or []),
        "signature_count": len(envelope_data.get("signatures") or []),
        "fixture_only": bool(receipt.get("fixture")),
        "attestation_binding": binding,
    }


def validate_receipt(receipt: dict[str, Any], *, require_signed: bool = False) -> list[str]:
    errors: list[str] = []
    for field in sorted(REQUIRED_FIELDS):
        if field not in receipt:
            errors.append(f"{field} is required")
    if receipt.get("schema") != SCHEMA:
        errors.append(f"schema must be {SCHEMA}")
    if not isinstance(receipt.get("claim"), str) or not receipt["claim"].strip():
        errors.append("claim must be a non-empty string")
    if receipt.get("type") not in CLAIM_TYPES:
        errors.append("type is not in the Receipt v1 claim enum")
    if receipt.get("replayability") not in REPLAYABILITY:
        errors.append("replayability is not in the Receipt v1 enum")
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list):
        errors.append("artifacts must be an array")
    else:
        for index, item in enumerate(artifacts):
            if not isinstance(item, dict):
                errors.append(f"artifacts[{index}] must be an object")
                continue
            for field in ("path", "kind"):
                if not isinstance(item.get(field), str) or not item[field].strip():
                    errors.append(f"artifacts[{index}].{field} is required")
            digest = item.get("sha256")
            if digest is not None and (
                not isinstance(digest, str)
                or len(digest) != 64
                or any(character not in "0123456789abcdef" for character in digest)
            ):
                errors.append(f"artifacts[{index}].sha256 must be lowercase hex sha256")
    caveats = receipt.get("caveats")
    if (
        not isinstance(caveats, list)
        or not caveats
        or any(not isinstance(item, str) or not item.strip() for item in caveats)
    ):
        errors.append("caveats must contain at least one non-empty string")
    verifier_runs = receipt.get("verifier_runs")
    if not isinstance(verifier_runs, list):
        errors.append("verifier_runs must be an array")
    else:
        for index, run in enumerate(verifier_runs):
            if not isinstance(run, dict):
                errors.append(f"verifier_runs[{index}] must be an object")
                continue
            if not isinstance(run.get("method"), str) or not run["method"].strip():
                errors.append(f"verifier_runs[{index}].method is required")
            if run.get("outcome") not in VERIFIER_OUTCOMES:
                errors.append(f"verifier_runs[{index}].outcome is not in the Receipt v1 enum")
    if not isinstance(receipt.get("environment"), dict):
        errors.append("environment must be an object")
    provenance = receipt.get("provenance")
    if not isinstance(provenance, dict):
        errors.append("provenance must be an object")
    else:
        for field in ("generated_by", "emitted_at"):
            if not isinstance(provenance.get(field), str) or not provenance[field].strip():
                errors.append(f"provenance.{field} is required")
    status = receipt.get("status")
    if not isinstance(status, dict):
        errors.append("status must be an object")
    else:
        if status.get("kind") not in STATUS_KINDS:
            errors.append("status.kind is not in the Receipt v1 enum")
        if status.get("authority") not in STATUS_AUTHORITIES:
            errors.append("status.authority is not in the Receipt v1 enum")
    fixture = receipt.get("fixture")
    if fixture is not None and fixture != {
        "kind": FIXTURE_KIND,
        "authority": "none",
        "importable": False,
    }:
        errors.append("fixture marker must deny authority and import")
    if receipt.get("attestation", {}).get("fixture_public_keys") and fixture is None:
        errors.append("fixture public keys require an explicit fixture marker")
    try:
        validate_safe_public_artifact_descriptors(receipt)
    except (TypeError, ValueError) as exc:
        errors.append(str(exc))
    scope = receipt.get("acceptance", {}).get("acceptance_scope")
    if scope not in ACCEPTANCE_SCOPES:
        errors.append("acceptance.acceptance_scope is not in the graded ladder")
    acceptance = receipt.get("acceptance") or {}
    for field in ["profile", "mechanism", "acceptor", "policyRef", "evidenceRefs"]:
        if field not in acceptance:
            errors.append(f"acceptance.{field} is required")
    if acceptance.get("mechanism") != ACCEPTANCE_MECHANISM:
        errors.append(f"acceptance.mechanism must be {ACCEPTANCE_MECHANISM}")
    if acceptance.get("evidenceLevel") is not None and acceptance.get("evidenceLevel") not in EVIDENCE_LEVELS:
        errors.append("acceptance.evidenceLevel is not in the allowed enum")
    if not isinstance(acceptance.get("evidenceRefs", []), list):
        errors.append("acceptance.evidenceRefs must be an array")
    if scope in CANON_SCOPES and not acceptance.get("acceptor"):
        errors.append("frontier or canon acceptance requires acceptance.acceptor")
    machine = receipt.get("machine") or {}
    verification = machine.get("verification") or {}
    denied = set(verification.get("trust_base", {}).get("allowed_axioms") or []) & ALLOWED_AXIOM_DENY
    if denied:
        errors.append("machine verification uses disallowed axiom or black-box proof: " + ",".join(sorted(denied)))
    status = receipt.get("status") or {}
    if status.get("authority") == "producer":
        if status.get("kind") not in {"draft", "emitted"}:
            errors.append("producer authority may emit only draft or emitted status")
    dist = receipt.get("distillation") or {}
    if scope in CANON_SCOPES:
        if dist.get("status") != "accepted":
            errors.append("frontier or canon acceptance requires accepted distillation")
        if not dist.get("uri") or not (dist.get("digest") or {}).get("sha256"):
            errors.append("frontier or canon acceptance requires distillation uri and digest")
        if not dist.get("accepted_by"):
            errors.append("frontier or canon acceptance requires distillation.accepted_by")
    if require_signed and not receipt.get("attestation", {}).get("dsse_envelope", {}).get("signatures"):
        errors.append("signed receipt expected a DSSE signature")
    envelope_signatures = receipt.get("attestation", {}).get("dsse_envelope", {}).get("signatures") or []
    signature_identities = receipt.get("signature_identities") or {}
    if require_signed and scope in CANON_SCOPES:
        if "producer" not in signature_identities or "acceptor" not in signature_identities:
            errors.append("accepted receipts require split producer and acceptor signer identities")
        if len(envelope_signatures) < 2:
            errors.append("accepted receipts require producer and acceptor DSSE signatures")
    contributors = receipt.get("contributors")
    if not isinstance(contributors, list) or not contributors:
        errors.append("contributors must contain at least one contributor")
    else:
        has_machine_producer = False
        for i, contributor in enumerate(contributors):
            roles = contributor.get("roles") if isinstance(contributor, dict) else None
            if not isinstance(roles, list) or not roles:
                errors.append(f"contributors[{i}].roles must be a non-empty array")
                continue
            unknown = sorted(set(roles) - CONTRIBUTOR_ROLES)
            if unknown:
                errors.append(f"contributors[{i}].roles contains unknown roles: {','.join(unknown)}")
            for disallowed in ["weight", "order", "payout"]:
                if disallowed in contributor:
                    errors.append(f"contributors[{i}] must not carry {disallowed} in v1")
            if "machine_producer" in roles:
                has_machine_producer = True
                if contributor.get("author") is not False:
                    errors.append("machine_producer must be originator, never author")
        if not has_machine_producer:
            errors.append("contributors must include a machine_producer")
    if errors:
        return errors
    try:
        standard_verify(receipt, require_signature=require_signed)
    except Exception as exc:
        errors.append(str(exc))
    return errors


def _verifier_run(spec: str) -> dict[str, str]:
    parts = spec.split(":", 3)
    if len(parts) < 2:
        raise argparse.ArgumentTypeError(
            "verifier run must be method:outcome[:log[:solver]]"
        )
    run = {"method": parts[0], "outcome": parts[1]}
    if len(parts) > 2:
        run["log"] = parts[2]
    if len(parts) > 3:
        run["solver"] = parts[3]
    return run


def _print_or_write_receipt(receipt: dict[str, Any], out: str | None) -> None:
    if out:
        write_json(Path(out), receipt)
    else:
        print(json.dumps(receipt, indent=2, sort_keys=True))


def cmd_validate(args: argparse.Namespace) -> int:
    receipt = load_json(Path(args.receipt))
    errors = validate_receipt(receipt, require_signed=args.require_signed)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    result = {"ok": True, **standard_verify(receipt, require_signature=args.require_signed)}
    print(json.dumps(result, sort_keys=True))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="vela-receipt-v1")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("emit", help="emit a neutral producer Receipt v1")
    p.add_argument("--claim", required=True)
    p.add_argument("--type", default="computational", dest="claim_type")
    p.add_argument("--replayability", default="unknown")
    p.add_argument("--artifact", action="append", default=[])
    p.add_argument("--caveat", action="append", default=[])
    p.add_argument("--verifier-run", type=_verifier_run, action="append", default=[])
    p.add_argument("--generated-by", default="vela-receipt-v1")
    p.add_argument("--submitter-actor")
    p.add_argument("--source-system")
    p.add_argument("--source-uri")
    p.add_argument("--run-id")
    p.add_argument("--base-dir", default=".")
    p.add_argument("--condition", action="append", default=[])
    p.add_argument("--verification-requirement", action="append", default=[])
    p.add_argument("--state-diff-json")
    p.add_argument("--out")
    p = sub.add_parser("validate")
    p.add_argument("receipt")
    p.add_argument("--require-signed", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.cmd == "emit":
            state_diff = (
                strict_json_loads(args.state_diff_json)
                if args.state_diff_json
                else {}
            )
            if not isinstance(state_diff, dict):
                raise ValueError("--state-diff-json must be a JSON object")
            receipt = emit_receipt(
                claim=args.claim,
                artifacts=args.artifact,
                caveats=args.caveat,
                claim_type=args.claim_type,
                replayability=args.replayability,
                verifier_runs=args.verifier_run,
                generated_by=args.generated_by,
                submitter_actor=args.submitter_actor,
                source_system=args.source_system,
                source_uri=args.source_uri,
                run_id=args.run_id,
                base_dir=args.base_dir,
                conditions=args.condition,
                verification_requirements=args.verification_requirement,
                state_diff=state_diff,
            )
            _print_or_write_receipt(receipt, args.out)
            return 0
        if args.cmd == "validate":
            return cmd_validate(args)
    except Exception as exc:
        print(f"ERROR {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
