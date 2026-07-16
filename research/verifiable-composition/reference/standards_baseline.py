"""Offline Git/DSSE/in-toto exact-lock wrapper for ADR 0004.

The shared ``fact_manifest`` module owns all scientific facts and dependency
status semantics. This removable wrapper only proves that the exact same
manifest can be carried by an in-toto Statement v1, an unsigned DSSE envelope,
and a fixture-signed ``science.lock``. Its committed seed is deliberately not a
secret and confers no authority. It never uses a human key, verifies a human
decision, mutates a frontier, or turns fixture data into authority.
"""

from __future__ import annotations

import base64
import binascii
import copy
import hashlib
import json
import tempfile
from pathlib import Path
from typing import Any

from fact_manifest import (
    FACT_ENVELOPE_SCHEMA,
    FACT_MANIFEST_SCHEMA,
    ManifestError,
    build_envelope,
    canonical_bytes,
    fact_manifest_root,
    finding_revision_root,
    validate_envelope,
    validate_fact_manifest,
)
from offline_bundle_inspection import (
    InspectionError,
    event_content_root,
    event_log_root,
    inspect_bundle,
)


MAX_JSON_BYTES = 1024 * 1024
LOCK_SCHEMA = "science.lock.v0"
LOCK_PAYLOAD_SCHEMA = "science.lock.payload.v0"
LOCK_PAYLOAD_TYPE = "application/vnd.science.lock+json"
STATEMENT_TYPE = "https://in-toto.io/Statement/v1"
PREDICATE_TYPE = (
    "https://vela.science/experiments/verifiable-composition/fact-manifest/v0"
)
DSSE_PAYLOAD_TYPE = "application/vnd.in-toto+json"
PROFILE = "git-dsse-intoto-exact-lock"
FIXTURE_KEY_ID = "fixture:adr004-standards-baseline"
FIXTURE_SCOPE = "internal_fixture_non_authority"
# RFC 8032 section 7.1, test vector 1. This is deliberately public test data,
# not secret key material, and MUST NOT be reused as Vela authority.
FIXTURE_SEED = bytes.fromhex(
    "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
)
FIXTURE_PUBLIC_KEY = bytes.fromhex(
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
)
BUNDLE_CHUNKS_SCHEMA = "vela.verifiable-composition.bundle-base64-chunks.v0"
BUNDLE_INDEX_SCHEMA = "vela.verifiable-composition.bundle-index.v0"

ED_P = 2**255 - 19
ED_L = 2**252 + 27742317777372353535851937790883648493
ED_D = (-121665 * pow(121666, ED_P - 2, ED_P)) % ED_P
ED_I = pow(2, (ED_P - 1) // 4, ED_P)


class BaselineError(ValueError):
    """One stable fail-closed wrapper result."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(code)
        self.code = code
        self.detail = detail


def _duplicate_guard(label: str):
    def guard(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise BaselineError(f"duplicate:{label}_object_name")
            value[key] = item
        return value

    return guard


def strict_json_bytes(raw: bytes, *, label: str) -> dict[str, Any]:
    if len(raw) > MAX_JSON_BYTES:
        raise BaselineError(f"oversized:{label}")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BaselineError(f"invalid:{label}_utf8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_duplicate_guard(label),
            parse_constant=lambda _: (_ for _ in ()).throw(
                BaselineError(f"invalid:{label}_number")
            ),
        )
    except BaselineError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise BaselineError(f"invalid:{label}_json") from error
    if not isinstance(value, dict):
        raise BaselineError(f"invalid:{label}_document")
    return value


def document_bytes(value: Any) -> bytes:
    """Canonical JSON with exactly one repository text-file newline."""

    return canonical_bytes(value) + b"\n"


def sha256_bytes(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def _ed_xrecover(y: int) -> int:
    xx = (y * y - 1) * pow(ED_D * y * y + 1, ED_P - 2, ED_P)
    x = pow(xx, (ED_P + 3) // 8, ED_P)
    if (x * x - xx) % ED_P != 0:
        x = (x * ED_I) % ED_P
    if x & 1:
        x = ED_P - x
    return x


ED_B_Y = (4 * pow(5, ED_P - 2, ED_P)) % ED_P
ED_B = (_ed_xrecover(ED_B_Y), ED_B_Y)
ED_IDENTITY = (0, 1)


def _ed_add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    x1, y1 = left
    x2, y2 = right
    product = ED_D * x1 * x2 * y1 * y2
    x3 = (x1 * y2 + x2 * y1) * pow(1 + product, ED_P - 2, ED_P)
    y3 = (y1 * y2 + x1 * x2) * pow(1 - product, ED_P - 2, ED_P)
    return x3 % ED_P, y3 % ED_P


def _ed_scalar(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    result = ED_IDENTITY
    addend = point
    while scalar:
        if scalar & 1:
            result = _ed_add(result, addend)
        addend = _ed_add(addend, addend)
        scalar >>= 1
    return result


def _ed_encode(point: tuple[int, int]) -> bytes:
    x, y = point
    encoded = y | ((x & 1) << 255)
    return encoded.to_bytes(32, "little")


def _ed_decode(raw: bytes) -> tuple[int, int]:
    if len(raw) != 32:
        raise BaselineError("invalid:fixture_ed25519_point")
    encoded = int.from_bytes(raw, "little")
    y = encoded & ((1 << 255) - 1)
    if y >= ED_P:
        raise BaselineError("invalid:fixture_ed25519_point")
    x = _ed_xrecover(y)
    if (x & 1) != (encoded >> 255):
        x = ED_P - x
    point = (x, y)
    if (-x * x + y * y - 1 - ED_D * x * x * y * y) % ED_P != 0:
        raise BaselineError("invalid:fixture_ed25519_point")
    if _ed_encode(point) != raw:
        raise BaselineError("invalid:fixture_ed25519_point")
    return point


def _fixture_keypair() -> tuple[int, bytes, bytes]:
    digest = hashlib.sha512(FIXTURE_SEED).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public_key = _ed_encode(_ed_scalar(ED_B, scalar))
    if public_key != FIXTURE_PUBLIC_KEY:
        raise BaselineError("invalid:rfc8032_fixture_key")
    return scalar, digest[32:], public_key


def _dsse_pae(payload_type: str, payload: bytes) -> bytes:
    encoded_type = payload_type.encode("ascii")
    return (
        b"DSSEv1 "
        + str(len(encoded_type)).encode("ascii")
        + b" "
        + encoded_type
        + b" "
        + str(len(payload)).encode("ascii")
        + b" "
        + payload
    )


def _fixture_sign(payload: bytes) -> tuple[bytes, bytes]:
    scalar, prefix, public_key = _fixture_keypair()
    message = _dsse_pae(LOCK_PAYLOAD_TYPE, payload)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % ED_L
    encoded_r = _ed_encode(_ed_scalar(ED_B, nonce))
    challenge = (
        int.from_bytes(
            hashlib.sha512(encoded_r + public_key + message).digest(), "little"
        )
        % ED_L
    )
    encoded_s = ((nonce + challenge * scalar) % ED_L).to_bytes(32, "little")
    return public_key, encoded_r + encoded_s


def _fixture_verify(public_key: bytes, signature: bytes, payload: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    try:
        authority = _ed_decode(public_key)
        encoded_r = signature[:32]
        point_r = _ed_decode(encoded_r)
    except BaselineError:
        return False
    scalar_s = int.from_bytes(signature[32:], "little")
    if scalar_s >= ED_L:
        return False
    message = _dsse_pae(LOCK_PAYLOAD_TYPE, payload)
    challenge = (
        int.from_bytes(
            hashlib.sha512(encoded_r + public_key + message).digest(), "little"
        )
        % ED_L
    )
    left = _ed_scalar(ED_B, scalar_s)
    right = _ed_add(point_r, _ed_scalar(authority, challenge))
    return _ed_encode(left) == _ed_encode(right)


def _fixture_root(label: str) -> str:
    return sha256_bytes(label.encode("utf-8"))


def _state_from_inspection(result: dict[str, Any], prefix: str) -> dict[str, str]:
    return {
        "git_commit": result[f"{prefix}_git_commit"],
        "git_tree": result[f"{prefix}_git_tree"],
        "event_log_root": event_log_root(result[f"{prefix}_events"]),
        "snapshot_root": sha256_bytes(
            canonical_bytes(result[f"{prefix}_snapshot"])
        ),
    }


def build_manifest_for_inspection(
    inspection: dict[str, Any],
    *,
    finding_status: str = "accepted",
) -> dict[str, Any]:
    """Build the shared fact manifest from one verified offline inspection.

    This is deliberately a pure adapter: all Git and event relations come from
    ``offline_bundle_inspection.inspect_bundle``. It neither accepts a caller's
    relation string nor creates authority.
    """

    result = inspection.get("result")
    if not isinstance(result, dict) or result.get("verification") != "verified":
        raise BaselineError("invalid:delivery_inspection")
    last_events = result.get("last_seen_events")
    delivered_events = result.get("delivered_events")
    if not isinstance(last_events, list) or not last_events:
        raise BaselineError("invalid:last_seen_events")
    if not isinstance(delivered_events, list) or not delivered_events:
        raise BaselineError("invalid:delivered_events")

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
    last_seen = _state_from_inspection(result, "last_seen")
    delivered = _state_from_inspection(result, "delivered")
    accepted = last_events[0]
    decision_root = event_content_root(accepted)
    receipt_roots = sorted(
        [_fixture_root("receipt-b"), _fixture_root("receipt-a")]
    )
    attachments = sorted(
        [
            {
                "attachment_id": "vva_6666666666666666",
                "attachment_content_root": _fixture_root("attachment-b"),
            },
            {
                "attachment_id": "vva_5555555555555555",
                "attachment_content_root": _fixture_root("attachment-a"),
            },
        ],
        key=lambda item: (
            item["attachment_id"],
            item["attachment_content_root"],
        ),
    )
    dependency = {
        "schema": "vela.experimental-dependency-observation.v0",
        "parent_frontier_id": "vfr_aaaaaaaaaaaaaaaa",
        "parent_git_commit": last_seen["git_commit"],
        "parent_git_tree": last_seen["git_tree"],
        "parent_event_log_root": last_seen["event_log_root"],
        "parent_snapshot_root": last_seen["snapshot_root"],
        "finding_id": finding["id"],
        "finding_revision_root": finding_revision_root(finding),
        "decision_event_id": accepted["id"],
        "decision_event_content_root": decision_root,
        "decision_signature": accepted["signature"],
        "authority_id": accepted["actor"]["id"],
        "receipt_roots": receipt_roots,
        "verifier_attachments": attachments,
        "premise_digest": _fixture_root("premise"),
        "role": "hard",
    }
    standing: dict[str, Any] = {
        "selected_finding_revision_root": dependency["finding_revision_root"],
        "decision_event_content_root": decision_root,
        "authority_id": dependency["authority_id"],
        "receipt_roots": copy.deepcopy(receipt_roots),
        "verifier_attachments": copy.deepcopy(attachments),
        "premise_digest": dependency["premise_digest"],
        "finding_status": finding_status,
        "decision_status": "valid",
        "verifier_status": "valid",
        "evidence_status": "available",
        "change_event": None,
    }
    if finding_status != "accepted":
        change = delivered_events[-1]
        standing["change_event"] = {
            "event_id": change["id"],
            "event_content_root": event_content_root(change),
            "event_signature": change["signature"],
            "authority_id": change["actor"]["id"],
            "effect": finding_status,
            "inspection_result_root": inspection["inspection_root"],
        }
    manifest = {
        "schema": FACT_MANIFEST_SCHEMA,
        "dependency": dependency,
        "accepted_finding": finding,
        "last_seen": last_seen,
        "delivered": delivered,
        "delivery_inspection": copy.deepcopy(inspection),
        "standing": standing,
    }
    _shared_validate(validate_fact_manifest, manifest, label="fact_manifest")
    return manifest


def build_statement(manifest: dict[str, Any]) -> dict[str, Any]:
    """Build an in-toto Statement v1 carrying one exact shared manifest."""

    _shared_validate(validate_fact_manifest, manifest, label="fact_manifest")
    root = fact_manifest_root(manifest)
    return {
        "_type": STATEMENT_TYPE,
        "predicate": {
            "fact_manifest": copy.deepcopy(manifest),
            "fact_manifest_root": root,
            "representation_claim": "same_information_only",
        },
        "predicateType": PREDICATE_TYPE,
        "subject": [
            {
                "digest": {"sha256": root[7:]},
                "name": "fact-manifest.json",
            }
        ],
    }


def build_dsse_envelope(statement: dict[str, Any]) -> dict[str, Any]:
    """Build the intentionally unsigned DSSE envelope shape."""

    return {
        "payload": base64.b64encode(canonical_bytes(statement)).decode("ascii"),
        "payloadType": DSSE_PAYLOAD_TYPE,
        "signatures": [],
    }


def build_lock(
    manifest: dict[str, Any],
    statement: dict[str, Any],
    envelope: dict[str, Any],
    *,
    semantics_root: str,
) -> dict[str, Any]:
    """Build one exact lock signed only by the public fixture identity."""

    payload = {
        "dependency": copy.deepcopy(manifest["dependency"]),
        "delivery_inspection": copy.deepcopy(manifest["delivery_inspection"]),
        "delivered": copy.deepcopy(manifest["delivered"]),
        "dsse_envelope_root": sha256_bytes(canonical_bytes(envelope)),
        "fact_manifest_root": fact_manifest_root(manifest),
        "last_seen": copy.deepcopy(manifest["last_seen"]),
        "profile": PROFILE,
        "schema": LOCK_PAYLOAD_SCHEMA,
        "semantics_root": semantics_root,
        "standing": copy.deepcopy(manifest["standing"]),
        "statement_root": sha256_bytes(canonical_bytes(statement)),
    }
    public_key, signature = _fixture_sign(canonical_bytes(payload))
    return {
        "lock": payload,
        "schema": LOCK_SCHEMA,
        "signatures": [
            {
                "algorithm": "ed25519",
                "key_id": FIXTURE_KEY_ID,
                "payload_type": LOCK_PAYLOAD_TYPE,
                "public_key_hex": public_key.hex(),
                "scope": FIXTURE_SCOPE,
                "signature_hex": signature.hex(),
            }
        ],
    }


def _exact_fields(value: Any, fields: set[str], *, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise BaselineError(f"invalid:{label}")
    missing = sorted(fields - set(value))
    if missing:
        raise BaselineError(f"missing:{label}.{missing[0]}")
    extra = sorted(set(value) - fields)
    if extra:
        raise BaselineError(f"unexpected:{label}.{extra[0]}")
    return value


def _shared_validate(call, value: Any, *, label: str) -> None:
    try:
        call(value)
    except ManifestError as error:
        raise BaselineError(f"invalid:{label}", f"{error.code}: {error.detail}") from error


def validate_statement(
    value: Any, expected_manifest: dict[str, Any]
) -> dict[str, Any]:
    statement = _exact_fields(
        value,
        {"_type", "predicate", "predicateType", "subject"},
        label="statement",
    )
    if statement["_type"] != STATEMENT_TYPE:
        raise BaselineError("invalid:statement_type")
    if statement["predicateType"] != PREDICATE_TYPE:
        raise BaselineError("invalid:predicate_type")
    subject = statement["subject"]
    if not isinstance(subject, list) or len(subject) != 1:
        raise BaselineError("invalid:statement_subject")
    subject_entry = _exact_fields(
        subject[0], {"digest", "name"}, label="statement_subject"
    )
    digest = _exact_fields(
        subject_entry["digest"], {"sha256"}, label="statement_subject.digest"
    )
    if subject_entry["name"] != "fact-manifest.json":
        raise BaselineError("invalid:statement_subject.name")
    expected_root = fact_manifest_root(expected_manifest)
    if digest["sha256"] != expected_root[7:]:
        raise BaselineError("mismatch:statement_subject_digest")
    predicate = _exact_fields(
        statement["predicate"],
        {"fact_manifest", "fact_manifest_root", "representation_claim"},
        label="statement.predicate",
    )
    if predicate["representation_claim"] != "same_information_only":
        raise BaselineError("invalid:representation_claim")
    _shared_validate(
        validate_fact_manifest,
        predicate["fact_manifest"],
        label="statement_fact_manifest",
    )
    if canonical_bytes(predicate["fact_manifest"]) != canonical_bytes(
        expected_manifest
    ):
        raise BaselineError("mismatch:statement_fact_manifest")
    if predicate["fact_manifest_root"] != expected_root:
        raise BaselineError("mismatch:statement_fact_manifest_root")
    return statement


def validate_dsse_envelope(value: Any, statement: dict[str, Any]) -> dict[str, Any]:
    envelope = _exact_fields(
        value, {"payload", "payloadType", "signatures"}, label="dsse_envelope"
    )
    if envelope["payloadType"] != DSSE_PAYLOAD_TYPE:
        raise BaselineError("invalid:dsse_payload_type")
    if envelope["signatures"] != []:
        raise BaselineError("invalid:dsse_signatures")
    if not isinstance(envelope["payload"], str):
        raise BaselineError("invalid:dsse_payload")
    try:
        decoded = base64.b64decode(envelope["payload"], validate=True)
    except (binascii.Error, ValueError) as error:
        raise BaselineError("invalid:dsse_payload") from error
    if decoded != canonical_bytes(statement):
        raise BaselineError("mismatch:dsse_payload")
    return envelope


def validate_lock(
    value: Any,
    manifest: dict[str, Any],
    *,
    statement_root: str,
    envelope_root: str,
    semantics_root: str,
) -> dict[str, Any]:
    envelope = _exact_fields(
        value,
        {"lock", "schema", "signatures"},
        label="science.lock",
    )
    if envelope["schema"] != LOCK_SCHEMA:
        raise BaselineError("invalid:science.lock.schema")
    signatures = envelope["signatures"]
    if not isinstance(signatures, list) or len(signatures) != 1:
        raise BaselineError("invalid:science.lock.signatures")
    signature = _exact_fields(
        signatures[0],
        {
            "algorithm",
            "key_id",
            "payload_type",
            "public_key_hex",
            "scope",
            "signature_hex",
        },
        label="science.lock.signature",
    )
    if signature["algorithm"] != "ed25519":
        raise BaselineError("invalid:science.lock.signature.algorithm")
    if signature["key_id"] != FIXTURE_KEY_ID:
        raise BaselineError("invalid:science.lock.signature.key_id")
    if signature["payload_type"] != LOCK_PAYLOAD_TYPE:
        raise BaselineError("invalid:science.lock.signature.payload_type")
    if signature["scope"] != FIXTURE_SCOPE:
        raise BaselineError("invalid:science.lock.signature.scope")
    try:
        public_key = bytes.fromhex(signature["public_key_hex"])
        signature_bytes = bytes.fromhex(signature["signature_hex"])
    except (TypeError, ValueError) as error:
        raise BaselineError("invalid:science.lock.signature.encoding") from error
    expected_public_key = _fixture_keypair()[2]
    if public_key != expected_public_key:
        raise BaselineError("mismatch:science.lock.fixture_public_key")
    lock = _exact_fields(
        envelope["lock"],
        {
            "dependency",
            "delivery_inspection",
            "delivered",
            "dsse_envelope_root",
            "fact_manifest_root",
            "last_seen",
            "profile",
            "schema",
            "semantics_root",
            "standing",
            "statement_root",
        },
        label="science.lock.payload",
    )
    if lock["schema"] != LOCK_PAYLOAD_SCHEMA:
        raise BaselineError("invalid:science.lock.payload.schema")
    if not _fixture_verify(public_key, signature_bytes, canonical_bytes(lock)):
        raise BaselineError("invalid:science.lock.fixture_signature")
    if lock["profile"] != PROFILE:
        raise BaselineError("invalid:science.lock.profile")
    if lock["fact_manifest_root"] != fact_manifest_root(manifest):
        raise BaselineError("mismatch:lock_fact_manifest_root")
    if lock["statement_root"] != statement_root:
        raise BaselineError("mismatch:lock_statement_root")
    if lock["dsse_envelope_root"] != envelope_root:
        raise BaselineError("mismatch:lock_dsse_envelope_root")
    if lock["semantics_root"] != semantics_root:
        raise BaselineError("mismatch:lock_semantics_root")
    for field in (
        "dependency",
        "last_seen",
        "delivered",
        "delivery_inspection",
        "standing",
    ):
        if canonical_bytes(lock[field]) != canonical_bytes(manifest[field]):
            raise BaselineError(f"mismatch:lock_{field}")
    return envelope


def validate_bundle_values(
    values: dict[str, dict[str, Any]],
    *,
    raw: dict[str, bytes],
    semantics_raw: bytes,
    actual_inspection: dict[str, Any] | None = None,
) -> dict[str, Any]:
    manifest = values["fact_manifest"]
    _shared_validate(validate_fact_manifest, manifest, label="fact_manifest")
    if raw["fact_manifest"] != document_bytes(manifest):
        raise BaselineError("invalid:fact_manifest_noncanonical")

    profile = values["vela_profile"]
    _shared_validate(validate_envelope, profile, label="vela_profile")
    if profile["schema"] != FACT_ENVELOPE_SCHEMA:
        raise BaselineError("invalid:vela_profile.schema")
    if raw["vela_profile"] != document_bytes(profile):
        raise BaselineError("invalid:vela_profile_noncanonical")
    if canonical_bytes(profile["fact_manifest"]) != canonical_bytes(manifest):
        raise BaselineError("mismatch:vela_profile_fact_manifest")
    if profile["fact_manifest_root"] != fact_manifest_root(manifest):
        raise BaselineError("mismatch:vela_profile_fact_manifest_root")

    statement = validate_statement(values["statement"], manifest)
    if raw["statement"] != document_bytes(statement):
        raise BaselineError("invalid:statement_noncanonical")
    if canonical_bytes(statement) != canonical_bytes(build_statement(manifest)):
        raise BaselineError("mismatch:statement_builder")
    envelope = validate_dsse_envelope(values["envelope"], statement)
    if raw["envelope"] != document_bytes(envelope):
        raise BaselineError("invalid:dsse_envelope_noncanonical")
    if canonical_bytes(envelope) != canonical_bytes(build_dsse_envelope(statement)):
        raise BaselineError("mismatch:dsse_envelope_builder")

    statement_root = sha256_bytes(canonical_bytes(statement))
    envelope_root = sha256_bytes(canonical_bytes(envelope))
    semantics_root = sha256_bytes(semantics_raw)
    if actual_inspection is not None:
        if canonical_bytes(manifest["delivery_inspection"]) != canonical_bytes(
            actual_inspection
        ):
            raise BaselineError("mismatch:offline_delivery_inspection")
        if canonical_bytes(manifest) != canonical_bytes(
            build_manifest_for_inspection(actual_inspection)
        ):
            raise BaselineError("mismatch:fact_manifest_builder")
    lock = validate_lock(
        values["lock"],
        manifest,
        statement_root=statement_root,
        envelope_root=envelope_root,
        semantics_root=semantics_root,
    )
    if raw["lock"] != document_bytes(lock):
        raise BaselineError("invalid:science.lock_noncanonical")
    if canonical_bytes(lock) != canonical_bytes(
        build_lock(
            manifest,
            statement,
            envelope,
            semantics_root=semantics_root,
        )
    ):
        raise BaselineError("mismatch:science.lock_builder")
    return {
        "dsse_envelope_root": envelope_root,
        "fact_manifest_root": fact_manifest_root(manifest),
        "git_commit": manifest["dependency"]["parent_git_commit"],
        "git_tree": manifest["dependency"]["parent_git_tree"],
        "lock_root": sha256_bytes(canonical_bytes(lock)),
        "lock_signature_scope": FIXTURE_SCOPE,
        "semantic_fact_manifest_equal": True,
        "semantics_root": semantics_root,
        "statement_root": statement_root,
    }


def load_fixture_bundle(fixture: Path) -> tuple[bytes, dict[str, Any]]:
    """Decode and root-check the checked-in public Git bundle."""

    chunks = strict_json_bytes(
        (fixture / "frontier.bundle.chunks.json").read_bytes(),
        label="bundle_chunks",
    )
    _exact_fields(
        chunks,
        {"schema", "encoding", "chunks"},
        label="bundle_chunks",
    )
    if (
        chunks["schema"] != BUNDLE_CHUNKS_SCHEMA
        or chunks["encoding"] != "base64"
        or not isinstance(chunks["chunks"], list)
        or not chunks["chunks"]
        or not all(isinstance(item, str) and item for item in chunks["chunks"])
    ):
        raise BaselineError("invalid:bundle_chunks")
    try:
        bundle_raw = base64.b64decode("".join(chunks["chunks"]), validate=True)
    except (binascii.Error, ValueError) as error:
        raise BaselineError("invalid:bundle_base64") from error

    index = strict_json_bytes(
        (fixture / "bundle-index.json").read_bytes(),
        label="bundle_index",
    )
    _exact_fields(
        index,
        {"schema", "bundle_bytes", "bundle_root", "commits", "state_path"},
        label="bundle_index",
    )
    if index["schema"] != BUNDLE_INDEX_SCHEMA:
        raise BaselineError("invalid:bundle_index.schema")
    _exact_fields(
        index["commits"],
        {"base", "corrected", "descendant", "fork"},
        label="bundle_index.commits",
    )
    if index["bundle_bytes"] != len(bundle_raw):
        raise BaselineError("mismatch:bundle_bytes")
    if index["bundle_root"] != sha256_bytes(bundle_raw):
        raise BaselineError("mismatch:bundle_root")
    return bundle_raw, index


def inspect_fixture_case(
    fixture: Path,
    *,
    last_seen: str,
    delivered: str,
) -> dict[str, Any]:
    """Inspect one named pair from the checked-in offline bundle."""

    bundle_raw, index = load_fixture_bundle(fixture)
    commits = index["commits"]
    if last_seen not in commits or delivered not in commits:
        raise BaselineError("invalid:bundle_case")
    with tempfile.TemporaryDirectory(prefix="vela-adr4-standards-bundle-") as raw:
        bundle = Path(raw) / "frontier.bundle"
        bundle.write_bytes(bundle_raw)
        try:
            return inspect_bundle(
                bundle,
                last_seen_commit=commits[last_seen],
                delivered_commit=commits[delivered],
                state_path=index["state_path"],
            )
        except InspectionError as error:
            raise BaselineError(
                f"unresolvable:{error.code}",
                error.detail,
            ) from error


def load_fixture_inspection(fixture: Path) -> dict[str, Any]:
    """Inspect the baseline's exact same-revision delivery."""

    return inspect_fixture_case(fixture, last_seen="base", delivered="base")


def load_bundle(root: Path, *, check_bundle: bool = True) -> dict[str, Any]:
    fixture = root / "fixtures/standards-baseline"
    paths = {
        "fact_manifest": fixture / "fact-manifest.json",
        "vela_profile": fixture / "vela-profile.json",
        "statement": fixture / "in-toto-statement.json",
        "lock": fixture / "science.lock",
    }
    raw = {label: path.read_bytes() for label, path in paths.items()}
    values = {
        label: strict_json_bytes(content, label=label)
        for label, content in raw.items()
    }
    values["envelope"] = build_dsse_envelope(values["statement"])
    raw["envelope"] = document_bytes(values["envelope"])
    return validate_bundle_values(
        values,
        raw=raw,
        semantics_raw=(fixture / "semantics.md").read_bytes(),
        actual_inspection=load_fixture_inspection(fixture) if check_bundle else None,
    )
