#!/usr/bin/env python3
"""Receipt-v1 conformance helpers.

This script is intentionally dependency-light. The only optional dependency is
the standard Python in-toto package, used by the validation commands to prove
that Vela receipts are normal DSSE-wrapped in-toto Statements.
"""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import shutil
import subprocess
import sys
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
STATUS_PREDICATE_TYPE = "https://vela.science/receipt/statusEvent/v1"
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
STATUS_EVENT_TYPES = {"supersedes", "withdraws", "revokes", "challenges", "deprecates", "restores"}
AUTHORITY_BASIS = {"original_issuer", "acceptor", "profile_authority", "third_party"}
STATUS_EVENT_AUTHORITY = {
    "supersedes": {"original_issuer"},
    "withdraws": {"original_issuer"},
    "revokes": {"acceptor"},
    "restores": {"acceptor"},
    "deprecates": {"profile_authority"},
    "challenges": {"third_party", "original_issuer", "acceptor", "profile_authority"},
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


def load_json(path: Path) -> dict[str, Any]:
    data = strict_json_load_bytes(path.read_bytes())
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


def validate_safe_public_artifact_descriptors(receipt: dict[str, Any]) -> None:
    """Use the portable Receipt v1 validator as the artifact-policy canon."""
    package_root = str(ROOT / "tools" / "receipt-v0")
    inserted = package_root not in sys.path
    if inserted:
        sys.path.insert(0, package_root)
    try:
        from vela_receipt_v0.core import (
            validate_safe_public_artifact_descriptors as validate_descriptors,
        )
    finally:
        if inserted:
            sys.path.remove(package_root)
    validate_descriptors(receipt)


def write_json(path: Path, data: dict[str, Any]) -> None:
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


def default_signature_identities(*, include_acceptor: bool) -> dict[str, Any]:
    identities: dict[str, Any] = {
        "producer": {
            "role": "producer",
            "signatureRef": None,
            "mechanism": "sigstore_keyless_oidc_fixture",
            "oidcIssuer": "https://token.actions.githubusercontent.com",
            "subject": "repo:vela-science/receipt-v1-demo:workflow:receipt-v1.yml",
            "orcid": "https://orcid.org/0000-0000-0000-0000",
            "note": "Fixture identity models keyless OIDC plus ORCID binding. It is not a human acceptance key.",
        }
    }
    if include_acceptor:
        identities["acceptor"] = {
            "role": "acceptor",
            "signatureRef": None,
            "mechanism": "ed25519_key_custody_ceremony_fixture",
            "subject": "reviewer:will-blair",
            "note": "Fixture signature models the existing human key-custody ceremony without using Will's key.",
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
            "subject": [{"name": a["path"], "digest": {"sha256": a.get("sha256", "")}} for a in artifacts],
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
        "signature_identities": default_signature_identities(include_acceptor=acceptance_scope in CANON_SCOPES),
    }
    attach_statement(receipt)
    return receipt


def prov_for_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    agent_id = receipt["provenance"].get("submitter", {}).get("actor") or receipt["provenance"]["generated_by"]
    activity_id = receipt.get("claim_id") or sha256_text(receipt["claim"])[:16]
    entities: dict[str, Any] = {
        "claim": {"prov:type": "vela:claim", "vela:claim": receipt["claim"]},
    }
    for item in receipt.get("artifacts", []):
        entities[f"artifact:{item['path']}"] = {
            "prov:type": "vela:artifact",
            "vela:kind": item.get("kind"),
            "vela:sha256": item.get("sha256"),
        }
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
        return "legacy_unbound"
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


def sign_receipt_for_fixture(receipt: dict[str, Any], *, include_acceptor: bool | None = None) -> dict[str, Any]:
    Envelope, CryptoSigner = require_intoto()
    receipt["fixture"] = {
        "kind": FIXTURE_KIND,
        "authority": "none",
        "importable": False,
    }
    producer = CryptoSigner.generate_ed25519()
    producer_keyid = producer.public_key.keyid
    acceptor = None
    acceptor_keyid = None
    if include_acceptor is None:
        include_acceptor = receipt.get("acceptance", {}).get("acceptance_scope") in CANON_SCOPES
    if include_acceptor:
        acceptor = CryptoSigner.generate_ed25519()
        acceptor_keyid = acceptor.public_key.keyid
    identities = receipt.setdefault("signature_identities", default_signature_identities(include_acceptor=include_acceptor))
    identities.setdefault("producer", {})["signatureRef"] = producer_keyid
    if include_acceptor and acceptor_keyid:
        identities.setdefault("acceptor", {})["signatureRef"] = acceptor_keyid
    attach_statement(receipt)
    statement = receipt["attestation"]["statement"]
    envelope = Envelope(canonical_json(statement), PAYLOAD_TYPE, {})
    envelope.sign(producer)
    if acceptor:
        envelope.sign(acceptor)
    receipt["attestation"]["dsse_envelope"] = envelope.to_dict()
    keys = {"producer": producer.public_key.to_dict()}
    keys["producer"]["keyid"] = producer_keyid
    if acceptor:
        keys["acceptor"] = acceptor.public_key.to_dict()
        keys["acceptor"]["keyid"] = acceptor_keyid
    receipt["attestation"]["fixture_public_keys"] = keys
    receipt["attestation"]["fixture_public_key"] = keys["producer"]
    receipt["attestation"]["signature_identities"] = identities
    receipt["attestation"]["signing_note"] = (
        "Conformance fixture signatures only. Producer fixture models keyless OIDC. "
        "Acceptor fixture models the human Ed25519 ceremony without using a human key."
    )
    return receipt


def standard_verify(receipt: dict[str, Any], require_signature: bool) -> dict[str, Any]:
    Envelope, _ = require_intoto()
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


def status_statement_from_event(event: dict[str, Any]) -> dict[str, Any]:
    return {
        "_type": INTOTO_STATEMENT,
        "subject": [{
            "name": event["target"]["claim_id"],
            "digest": {"sha256": event["target"]["receiptDigest"]},
        }],
        "predicateType": STATUS_PREDICATE_TYPE,
        "predicate": {
            "schema": "vela.receipt.statusEvent.predicate.v1",
            "statusEvent": event["statusEvent"],
            "target": event["target"],
            "lineage": event["lineage"],
            "provenance": event["provenance"],
        },
    }


def attach_status_statement(event: dict[str, Any]) -> None:
    statement = status_statement_from_event(event)
    event["attestation"] = {
        "format": "in-toto-statement",
        "statement": statement,
        "dsse_envelope": unsigned_envelope(statement),
        "prov": statement["predicate"]["provenance"],
    }


def make_status_event(
    *,
    target: dict[str, Any],
    event_type: str,
    authority_basis: str,
    actor: str,
    reason_code: str,
    policy_ref: str,
    evidence_refs: list[str] | None = None,
    replacement_receipt: str | None = None,
) -> dict[str, Any]:
    target_digest = sha256_json(target)
    claim_id = target.get("claim_id") or sha256_text(target["claim"])[:16]
    created_at = utc_now()
    event = {
        "schema": "vela.receipt.statusEvent.v1",
        "target": {
            "claim_id": claim_id,
            "receiptDigest": target_digest,
            "predicateType": PREDICATE_TYPE,
        },
        "statusEvent": {
            "type": event_type,
            "authorityBasis": authority_basis,
            "actor": actor,
            "reasonCode": reason_code,
            "policyRef": policy_ref,
            "evidenceRefs": evidence_refs or [],
            "replacementReceipt": replacement_receipt,
            "createdAt": created_at,
        },
        "lineage": {
            "wasDerivedFrom": [claim_id],
            "wasInvalidatedBy": [f"statusEvent:{event_type}:{created_at}"] if event_type in {"withdraws", "revokes", "deprecates"} else [],
        },
        "provenance": {
            "prefix": {
                "prov": "http://www.w3.org/ns/prov#",
                "vela": "https://vela.science/ns#",
            },
            "activity": {
                "statusEvent": {
                    "prov:type": "vela:status-event",
                    "prov:startedAtTime": created_at,
                }
            },
            "agent": {
                actor: {
                    "prov:type": "prov:Agent",
                    "vela:authorityBasis": authority_basis,
                }
            },
            "wasDerivedFrom": {
                "_:statusDerived": {
                    "prov:generatedEntity": f"statusEvent:{event_type}",
                    "prov:usedEntity": claim_id,
                }
            },
        },
    }
    attach_status_statement(event)
    return event


def sign_status_event_for_fixture(event: dict[str, Any]) -> dict[str, Any]:
    Envelope, CryptoSigner = require_intoto()
    signer = CryptoSigner.generate_ed25519()
    statement = event["attestation"]["statement"]
    envelope = Envelope(canonical_json(statement), PAYLOAD_TYPE, {})
    envelope.sign(signer)
    event["attestation"]["dsse_envelope"] = envelope.to_dict()
    event["attestation"]["fixture_public_key"] = signer.public_key.to_dict()
    event["attestation"]["fixture_public_key"]["keyid"] = signer.public_key.keyid
    event["attestation"]["signing_note"] = "Conformance fixture signature only. It is not a human key ceremony."
    return event


def standard_verify_status_event(event: dict[str, Any], require_signature: bool) -> dict[str, Any]:
    Envelope, _ = require_intoto()
    statement = event.get("attestation", {}).get("statement")
    if not isinstance(statement, dict):
        raise ValueError("status event statement missing")
    if statement.get("_type") != INTOTO_STATEMENT:
        raise ValueError("status event is not in-toto Statement v1")
    if statement.get("predicateType") != STATUS_PREDICATE_TYPE:
        raise ValueError("unexpected status event predicateType")
    envelope_data = copy.deepcopy(event.get("attestation", {}).get("dsse_envelope"))
    if not isinstance(envelope_data, dict):
        raise ValueError("status event DSSE envelope missing")
    if envelope_data.get("signatures"):
        env = Envelope.from_dict(envelope_data)
        key = event.get("attestation", {}).get("fixture_public_key")
        if require_signature and not key:
            raise ValueError("signed status event needs fixture_public_key for verification")
        if key:
            env.verify_signature(key)
        round_trip = strict_json_load_bytes(env.payload)
    else:
        if require_signature:
            raise ValueError("status event DSSE envelope has no signature")
        round_trip = strict_json_load_bytes(
            base64.b64decode(envelope_data["payload"], validate=True)
        )
    if round_trip != statement:
        raise ValueError("status event DSSE payload does not match statement")
    return {
        "standard_tool": "in-toto",
        "signature_verified": bool(envelope_data.get("signatures")),
        "predicate_type": STATUS_PREDICATE_TYPE,
    }


def validate_status_event(event: dict[str, Any], *, require_signed: bool = False) -> list[str]:
    errors: list[str] = []
    if event.get("schema") != "vela.receipt.statusEvent.v1":
        errors.append("status event schema must be vela.receipt.statusEvent.v1")
    status_event = event.get("statusEvent") or {}
    event_type = status_event.get("type")
    authority_basis = status_event.get("authorityBasis")
    if event_type not in STATUS_EVENT_TYPES:
        errors.append("statusEvent.type is invalid")
    if authority_basis not in AUTHORITY_BASIS:
        errors.append("statusEvent.authorityBasis is invalid")
    if event_type in STATUS_EVENT_AUTHORITY and authority_basis not in STATUS_EVENT_AUTHORITY[event_type]:
        errors.append(f"{authority_basis} may not {event_type}")
    if event_type == "supersedes" and not status_event.get("replacementReceipt"):
        errors.append("supersession requires replacementReceipt")
    if event_type == "challenges" and authority_basis != "third_party":
        errors.append("challenge test receipts require third_party authority")
    if not status_event.get("reasonCode"):
        errors.append("statusEvent.reasonCode is required")
    if require_signed and not event.get("attestation", {}).get("dsse_envelope", {}).get("signatures"):
        errors.append("signed status event expected a DSSE signature")
    if errors:
        return errors
    try:
        standard_verify_status_event(event, require_signature=require_signed)
    except Exception as exc:
        errors.append(str(exc))
    return errors


def resolve_status_chain(original: dict[str, Any], events: list[dict[str, Any]]) -> dict[str, Any]:
    current = {
        "claim_id": original.get("claim_id"),
        "state": "active",
        "replacementReceipt": None,
        "challenges": [],
        "appliedEvents": [],
        "rejectedEvents": [],
    }
    for event in events:
        errors = validate_status_event(event, require_signed=True)
        event_type = event.get("statusEvent", {}).get("type")
        if errors:
            current["rejectedEvents"].append({
                "type": event_type,
                "errors": errors,
            })
            continue
        status_event = event["statusEvent"]
        current["appliedEvents"].append(event_type)
        if event_type == "challenges":
            current["challenges"].append({
                "actor": status_event["actor"],
                "reasonCode": status_event["reasonCode"],
            })
        elif event_type == "supersedes":
            current["state"] = "superseded"
            current["replacementReceipt"] = status_event.get("replacementReceipt")
        elif event_type == "withdraws":
            current["state"] = "withdrawn"
        elif event_type == "revokes":
            current["state"] = "revoked"
        elif event_type == "deprecates":
            current["state"] = "deprecated"
        elif event_type == "restores":
            current["state"] = "active"
    return current


def validate_receipt(receipt: dict[str, Any], *, require_signed: bool = False) -> list[str]:
    errors: list[str] = []
    for field in [
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
    ]:
        if field not in receipt:
            errors.append(f"{field} is required")
    if receipt.get("schema") != SCHEMA:
        errors.append(f"schema must be {SCHEMA}")
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


def accepted_fixture_receipt(out: Path) -> dict[str, Any]:
    fixture = ROOT / "scripts" / "fixtures" / "receipt-v1"
    dist_path = fixture / "distillation.md"
    artifact_path = fixture / "accepted-sidon.witness.json"
    evidence_path = fixture / "source-evidence.json"
    frontier = load_json(fixture / "frontier.json")
    finding = frontier["findings"][0]
    event = load_json(fixture / "accepted-event.json")
    dist = distillation_block(
        uri=dist_path.relative_to(ROOT).as_posix(),
        digest=sha256_file(dist_path),
        status="accepted",
        accepted_by="reviewer:conformance-fixture",
    )
    dist["signature_refs"] = ["scripts/fixtures/receipt-v1/accepted-event.json"]
    receipt = make_receipt(
        claim_id=finding["id"],
        claim=finding["assertion"]["text"],
        claim_type="computational",
        replayability="exact",
        artifacts=[
            artifact(artifact_path, "sidon_witness", ROOT),
            artifact(evidence_path, "source_evidence", ROOT),
            artifact(dist_path, "distillation", ROOT),
        ],
        verifier_runs=[{
            "method": "vela reproduce",
            "outcome": "pass",
            "log": "Sidon verified: 3 points, 6 pairwise sums all distinct",
            "replay_command": "target/release/vela reproduce scripts/fixtures/receipt-v1/accepted-sidon.witness.json --json",
        }],
        caveats=["Synthetic conformance fixture only. It carries no scientific authority."],
        generated_by="scripts/receipt_v1.py accepted-fixture",
        submitter="agent:receipt-v1-conformance",
        acceptance_scope="frontier_accepted",
        acceptance_status="reviewer:conformance-fixture",
        acceptance_authority="human_key",
        acceptance_profile="vela.frontier.sidon.v1",
        policy_ref="docs/RECEIPT_GOVERNANCE.md#signing-and-acceptance",
        evidence_refs=[
            "scripts/fixtures/receipt-v1/accepted-sidon.witness.json",
            "scripts/fixtures/receipt-v1/accepted-event.json",
        ],
        evidence_level="local_signoff",
        distillation=dist,
        lineage={
            "frontier": "scripts/fixtures/receipt-v1",
            "frontier_node_id": finding["id"],
            "accepted_event_id": event["id"],
            "source_refs": [
                "https://example.invalid/vela/conformance/accepted-sidon",
            ],
            "derived_from": ["fixture:accepted-sidon"],
            "parents": [],
            "supersedes": [],
        },
        contributors=[
            {
                "id": "fixture:receipt-producer",
                "roles": ["machine_producer", "software"],
                "credit_taxonomy": "CRediT+Vela",
                "author": False,
                "note": "Synthetic producer identity for conformance only.",
            },
            {
                "id": "agent:receipt-v1-conformance",
                "roles": ["data_curation", "software"],
                "credit_taxonomy": "CRediT+Vela",
                "author": False,
                "note": "Relay ingested and content-addressed external artifacts.",
            },
            {
                "id": "reviewer:conformance-fixture",
                "roles": ["reviewer", "acceptor"],
                "credit_taxonomy": "CRediT+Vela",
                "author": False,
            },
        ],
        environment={"ro_crate": None},
        provenance_extra={"source_record": "Committed accepted receipt fixture"},
    )
    receipt["signature_identities"]["acceptor"]["subject"] = "reviewer:conformance-fixture"
    sign_receipt_for_fixture(receipt)
    write_json(out, receipt)
    return receipt


def build_ro_crate(crate_dir: Path, files: list[Path]) -> dict[str, Any]:
    crate_dir.mkdir(parents=True, exist_ok=True)
    graph = [
        {"@id": "ro-crate-metadata.json", "@type": "CreativeWork", "about": {"@id": "./"}},
        {"@id": "./", "@type": "Dataset", "name": "Vela Receipt-v1 demo crate"},
    ]
    for path in files:
        target = crate_dir / path.name
        shutil.copy2(path, target)
        graph.append({"@id": path.name, "@type": "File", "sha256": sha256_file(target)})
    crate = {"@context": "https://w3id.org/ro/crate/1.1/context", "@graph": graph}
    write_json(crate_dir / "ro-crate-metadata.json", crate)
    return {"path": crate_dir.as_posix(), "metadata": "ro-crate-metadata.json", "sha256": sha256_file(crate_dir / "ro-crate-metadata.json")}


def make_demo_workspace(workdir: Path) -> dict[str, Path]:
    workdir.mkdir(parents=True, exist_ok=True)
    witness = workdir / "sidon-witness.json"
    witness.write_text(
        json.dumps({
            "kind": "sidon",
            "claimed_size": 3,
            "n": 2,
            "points": [[0, 0], [1, 0], [0, 1]],
        }, indent=2) + "\n",
        encoding="utf-8",
    )
    dist = workdir / "distillation.md"
    dist.write_text(
        "# Demo Sidon receipt\n\nThis is a small exact Sidon witness used to demonstrate Receipt-v1 emission.\n",
        encoding="utf-8",
    )
    return {"witness": witness, "distillation": dist}


def github_action_demo(workdir: Path) -> dict[str, Any]:
    paths = make_demo_workspace(workdir)
    verify = subprocess.run(
        [str(ROOT / "target/release/vela"), "reproduce", str(paths["witness"]), "--json"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if verify.returncode != 0:
        raise RuntimeError(verify.stderr or verify.stdout)
    crate = build_ro_crate(workdir / "ro-crate", [paths["witness"], paths["distillation"]])
    dist = distillation_block(
        uri="distillation.md",
        digest=sha256_file(paths["distillation"]),
        status="accepted",
        accepted_by="test-maintainer:fixture",
        audience="outside math producer",
        level="short explainer",
    )
    receipt = make_receipt(
        claim_id="vf_demo_sidon_receipt_v1",
        claim="The demo producer supplies a 3 point Sidon witness.",
        claim_type="computational",
        replayability="exact",
        artifacts=[artifact(paths["witness"], "sidon_witness", workdir), artifact(paths["distillation"], "distillation", workdir)],
        verifier_runs=[{
            "method": "vela reproduce",
            "outcome": "pass",
            "log": strict_json_loads(verify.stdout)["results"][0]["message"],
            "replay_command": "vela reproduce sidon-witness.json --json",
        }],
        caveats=["Demo receipt only. It is not accepted into a live frontier."],
        generated_by="github-action:vela-receipt-v1-demo",
        submitter="github-actions[bot]",
        acceptance_scope="frontier_accepted",
        acceptance_status="test-maintainer:fixture",
        acceptance_authority="human_key",
        distillation=dist,
        lineage={"frontier": "demo/sidon", "parents": [], "derived_from": [], "supersedes": [], "source_refs": []},
        environment={"ro_crate": crate},
    )
    draft = workdir / "receipt-draft.json"
    write_json(draft, receipt)
    sign_receipt_for_fixture(receipt)
    signed = workdir / ".vela/receipts/vf_demo_sidon_receipt_v1.json"
    write_json(signed, receipt)
    imported = import_claim_from_receipt(receipt, allow_fixture=True)
    return {
        "ok": True,
        "draft_receipt": draft.as_posix(),
        "signed_receipt": signed.as_posix(),
        "ro_crate": crate["path"],
        "imported_claim_id": imported["claim_id"],
        "fixture_only": imported["fixture_only"],
        "signature_verified": standard_verify(receipt, require_signature=True)["signature_verified"],
    }


def github_action_draft(workdir: Path) -> dict[str, Any]:
    paths = make_demo_workspace(workdir)
    verify = subprocess.run(
        [str(ROOT / "target/release/vela"), "reproduce", str(paths["witness"]), "--json"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if verify.returncode != 0:
        raise RuntimeError(verify.stderr or verify.stdout)
    crate = build_ro_crate(workdir / "ro-crate", [paths["witness"], paths["distillation"]])
    dist = distillation_block(
        uri="distillation.md",
        digest=sha256_file(paths["distillation"]),
        status="draft",
        accepted_by=None,
        audience="outside math producer",
        level="short explainer",
    )
    receipt = make_receipt(
        claim_id="vf_demo_sidon_receipt_v1",
        claim="The demo producer supplies a 3 point Sidon witness.",
        claim_type="computational",
        replayability="exact",
        artifacts=[artifact(paths["witness"], "sidon_witness", workdir), artifact(paths["distillation"], "distillation", workdir)],
        verifier_runs=[{
            "method": "vela reproduce",
            "outcome": "pass",
            "log": strict_json_loads(verify.stdout)["results"][0]["message"],
            "replay_command": "vela reproduce sidon-witness.json --json",
        }],
        caveats=["Draft receipt only. Human acceptance and signature are separate."],
        generated_by="github-action:vela-receipt-v1-draft",
        submitter="github-actions[bot]",
        acceptance_scope="hypothesis_only",
        acceptance_status="not_assessed",
        acceptance_authority="producer",
        acceptance_profile="producer.emission.v1",
        policy_ref="urn:vela:policy:none",
        evidence_level=None,
        distillation=dist,
        lineage={"frontier": "demo/sidon", "parents": [], "derived_from": [], "supersedes": [], "source_refs": []},
        environment={"ro_crate": crate},
    )
    draft = workdir / "receipt-draft.json"
    write_json(draft, receipt)
    return {
        "ok": True,
        "draft_receipt": draft.as_posix(),
        "ro_crate": crate["path"],
        "signature_requested": False,
        "signed": False,
    }


def import_claim_from_receipt(
    receipt: dict[str, Any], *, allow_fixture: bool = False
) -> dict[str, Any]:
    errors = validate_receipt(receipt, require_signed=bool(receipt.get("attestation", {}).get("dsse_envelope", {}).get("signatures")))
    if errors:
        raise ValueError("; ".join(errors))
    if receipt.get("fixture") and not allow_fixture:
        raise ValueError("conformance fixture receipts cannot enter accepted state")
    return {
        "claim_id": receipt.get("claim_id"),
        "claim": receipt["claim"],
        "scope": receipt["acceptance"]["acceptance_scope"],
        "frontier": receipt["lineage"].get("frontier"),
        "distillation_uri": receipt["distillation"].get("uri"),
        "fixture_only": bool(receipt.get("fixture")),
    }


def outside_demo(workdir: Path) -> dict[str, Any]:
    result = github_action_demo(workdir)
    receipt = load_json(Path(result["signed_receipt"]))
    imported = import_claim_from_receipt(receipt, allow_fixture=True)
    return {
        "ok": True,
        "producer_time_budget": "under one day",
        "validate_command": f"python3 scripts/receipt_v1.py validate {result['signed_receipt']} --require-signed",
        "imported_claim": imported,
    }


def status_roundtrip(workdir: Path) -> dict[str, Any]:
    base_path = workdir / "accepted-fixture.receipt.json"
    original = accepted_fixture_receipt(base_path)
    supersession = sign_status_event_for_fixture(make_status_event(
        target=original,
        event_type="supersedes",
        authority_basis="original_issuer",
        actor="agent:openresearch-vela-relay",
        reason_code="fixture-successor-receipt",
        policy_ref="docs/RECEIPT_GOVERNANCE.md#append-only-status-receipts",
        evidence_refs=["status-fixture:supersession"],
        replacement_receipt="urn:sha256:" + sha256_text("replacement fixture"),
    ))
    challenge = sign_status_event_for_fixture(make_status_event(
        target=original,
        event_type="challenges",
        authority_basis="third_party",
        actor="reviewer:independent-third-party",
        reason_code="counter-attestation",
        policy_ref="docs/RECEIPT_GOVERNANCE.md#append-only-status-receipts",
        evidence_refs=["status-fixture:challenge"],
    ))
    rejected_revoke = sign_status_event_for_fixture(make_status_event(
        target=original,
        event_type="revokes",
        authority_basis="third_party",
        actor="reviewer:independent-third-party",
        reason_code="unauthorized-revoke-attempt",
        policy_ref="docs/RECEIPT_GOVERNANCE.md#append-only-status-receipts",
        evidence_refs=["status-fixture:third-party-revoke"],
    ))
    write_json(workdir / "status-supersedes.json", supersession)
    write_json(workdir / "status-challenges.json", challenge)
    write_json(workdir / "status-third-party-revoke.json", rejected_revoke)
    resolver = resolve_status_chain(original, [supersession, challenge, rejected_revoke])
    errors = []
    for event in [supersession, challenge]:
        errors.extend(validate_status_event(event, require_signed=True))
    rejected_errors = validate_status_event(rejected_revoke, require_signed=True)
    if not rejected_errors:
        errors.append("third-party revoke was not rejected")
    if resolver["state"] != "superseded":
        errors.append("resolver did not preserve superseded current state")
    if len(resolver["challenges"]) != 1:
        errors.append("resolver did not record the third-party challenge")
    if not resolver["rejectedEvents"]:
        errors.append("resolver did not record the rejected third-party revoke")
    return {
        "ok": not errors,
        "errors": errors,
        "original_digest_before": sha256_json(original),
        "original_digest_after": sha256_json(load_json(base_path)),
        "supersession_signed": True,
        "challenge_signed": True,
        "third_party_revoke_rejected": bool(rejected_errors),
        "resolver": resolver,
        "status_receipts": {
            "supersedes": (workdir / "status-supersedes.json").as_posix(),
            "challenges": (workdir / "status-challenges.json").as_posix(),
            "third_party_revoke": (workdir / "status-third-party-revoke.json").as_posix(),
        },
    }


def base_red_team_receipt(tmp: Path) -> dict[str, Any]:
    result = github_action_demo(tmp)
    return load_json(Path(result["signed_receipt"]))


def assess_red_team(name: str, receipt: dict[str, Any], maintainers: set[str]) -> dict[str, Any]:
    visible: list[str] = []
    revoked = False
    verification = receipt.get("machine", {}).get("verification", {})
    trust = verification.get("trust_base", {})
    denied = sorted(set(trust.get("allowed_axioms") or []) & ALLOWED_AXIOM_DENY)
    if denied:
        visible.append("disallowed_axiom:" + ",".join(denied))
        revoked = True
    accepted_by = receipt.get("acceptance", {}).get("claim_acceptance", {}).get("accepted_by")
    if accepted_by and accepted_by not in maintainers:
        visible.append("wrong_acceptor")
        revoked = True
    scope = receipt.get("acceptance", {}).get("acceptance_scope")
    significance = receipt.get("acceptance", {}).get("claim_acceptance", {}).get("significance", "material")
    if scope == "canon_accepted" and significance == "insignificant":
        visible.append("scope_too_high_for_significance")
        receipt["acceptance"]["acceptance_scope"] = "machine_verified"
    if receipt.get("distillation", {}).get("status") == "rejected":
        visible.append("distillation_rejected")
        revoked = True
    if verification.get("status") in {"fail", "error"}:
        visible.append("machine_verification_failed")
        revoked = True
    if receipt.get("lineage", {}).get("replication_status") == "failed":
        visible.append("replication_failed_supersession_required")
        revoked = True
    if receipt.get("provenance", {}).get("hidden"):
        visible.append("provenance_incomplete_blocks_canon")
        receipt["acceptance"]["acceptance_scope"] = "machine_verified"
    if receipt.get("status", {}).get("authority") == "producer" and scope in CANON_SCOPES:
        visible.append("producer_receipt_cannot_self_accept")
        revoked = True
    return {
        "case": name,
        "visible": visible,
        "revocable": revoked,
        "laundered": not visible,
    }


def red_team(workdir: Path) -> dict[str, Any]:
    maintainers = {"test-maintainer:fixture", "reviewer:will-blair"}
    base = base_red_team_receipt(workdir / "base")
    cases: dict[str, dict[str, Any]] = {}
    cases["compiled_but_cheated"] = copy.deepcopy(base)
    cases["compiled_but_cheated"]["machine"]["verification"]["trust_base"]["allowed_axioms"] = ["sorryAx"]
    cases["accepted_by_wrong_human"] = copy.deepcopy(base)
    cases["accepted_by_wrong_human"]["acceptance"]["claim_acceptance"]["accepted_by"] = "github:drive-by"
    cases["true_but_insignificant"] = copy.deepcopy(base)
    cases["true_but_insignificant"]["acceptance"]["acceptance_scope"] = "canon_accepted"
    cases["true_but_insignificant"]["acceptance"]["claim_acceptance"]["significance"] = "insignificant"
    cases["good_proof_bad_distillation"] = copy.deepcopy(base)
    cases["good_proof_bad_distillation"]["distillation"]["status"] = "rejected"
    cases["good_distillation_bad_proof"] = copy.deepcopy(base)
    cases["good_distillation_bad_proof"]["machine"]["verification"]["status"] = "fail"
    cases["biology_replication_fails"] = copy.deepcopy(base)
    cases["biology_replication_fails"]["type"] = "empirical"
    cases["biology_replication_fails"]["lineage"]["replication_status"] = "failed"
    cases["provenance_hidden"] = copy.deepcopy(base)
    cases["provenance_hidden"]["provenance"]["hidden"] = True
    cases["receipt_spam"] = copy.deepcopy(base)
    cases["receipt_spam"]["status"]["authority"] = "producer"
    results = [assess_red_team(name, receipt, maintainers) for name, receipt in cases.items()]
    failures = [r for r in results if r["laundered"]]
    return {"ok": not failures, "cases": results, "failures": failures}


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


def cmd_import_claim(args: argparse.Namespace) -> int:
    receipt = load_json(Path(args.receipt))
    print(json.dumps(import_claim_from_receipt(receipt), sort_keys=True))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("accepted-fixture")
    p.add_argument("--out", required=True)
    p = sub.add_parser("validate")
    p.add_argument("receipt")
    p.add_argument("--require-signed", action="store_true")
    p = sub.add_parser("import-claim")
    p.add_argument("receipt")
    p = sub.add_parser("github-action-demo")
    p.add_argument("--workdir", required=True)
    p = sub.add_parser("github-action-draft")
    p.add_argument("--workdir", required=True)
    p = sub.add_parser("red-team")
    p.add_argument("--workdir", required=True)
    p = sub.add_parser("outside-demo")
    p.add_argument("--workdir", required=True)
    p = sub.add_parser("status-roundtrip")
    p.add_argument("--workdir", required=True)
    args = parser.parse_args(argv)
    try:
        if args.cmd == "accepted-fixture":
            receipt = accepted_fixture_receipt(Path(args.out))
            print(json.dumps({"ok": True, "receipt": args.out, "claim_id": receipt["claim_id"]}, sort_keys=True))
            return 0
        if args.cmd == "validate":
            return cmd_validate(args)
        if args.cmd == "import-claim":
            return cmd_import_claim(args)
        if args.cmd == "github-action-demo":
            print(json.dumps(github_action_demo(Path(args.workdir)), sort_keys=True))
            return 0
        if args.cmd == "github-action-draft":
            print(json.dumps(github_action_draft(Path(args.workdir)), sort_keys=True))
            return 0
        if args.cmd == "red-team":
            result = red_team(Path(args.workdir))
            print(json.dumps(result, sort_keys=True))
            return 0 if result["ok"] else 1
        if args.cmd == "outside-demo":
            print(json.dumps(outside_demo(Path(args.workdir)), sort_keys=True))
            return 0
        if args.cmd == "status-roundtrip":
            result = status_roundtrip(Path(args.workdir))
            print(json.dumps(result, sort_keys=True))
            return 0 if result["ok"] else 1
    except Exception as exc:
        print(f"ERROR {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
