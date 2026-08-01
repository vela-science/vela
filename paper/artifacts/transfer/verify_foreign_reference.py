#!/usr/bin/env python3
"""Dependency-free clean-room verifier for a foreign-reference package."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "conformance"))

from readers.python.canonical import canonical_bytes  # noqa: E402

DEFAULT_PACKAGE = REPO_ROOT / "paper" / "artifacts" / "transfer" / "erdos-424"
REQUIRED_ROLES = {
    "applied_event",
    "authority_keyset",
    "authority_record",
    "claim",
    "current_repository_manifest",
    "decision_event",
    "proposal",
    "repository_origin",
    "submission",
    "transition_repository_manifest",
    "verification",
}


def sha256_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def sha256(value: object) -> str:
    return sha256_bytes(canonical_bytes(value))


def short_id(prefix: str, value: object, length: int = 16) -> str:
    return f"{prefix}{hashlib.sha256(canonical_bytes(value)).hexdigest()[:length]}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def require_exact_keys(value: dict[str, object], keys: set[str], label: str) -> None:
    require(set(value) == keys, f"{label} fields")


def full_sha(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r"sha256:[0-9a-f]{64}", value) is not None


def prefixed(value: object, prefix: str, length: int) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(rf"{re.escape(prefix)}[0-9a-f]{{{length}}}", value) is not None
    )


def relative_path(value: object) -> bool:
    if not isinstance(value, str) or not value or value.strip() != value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and "\\" not in value
        and all(part not in ("", ".", "..") for part in value.split("/"))
    )


def exact_ref(value: object, prefix: str, length: int, label: str) -> dict[str, object]:
    require(isinstance(value, dict), label)
    require_exact_keys(value, {"id", "root"}, label)
    require(prefixed(value.get("id"), prefix, length), f"{label} id")
    require(full_sha(value.get("root")), f"{label} root")
    return value


def repository_ref(value: object, label: str) -> dict[str, object]:
    require(isinstance(value, dict), label)
    require_exact_keys(value, {"git_commit", "git_tree", "repository_root"}, label)
    require(re.fullmatch(r"[0-9a-f]{40}", value.get("git_commit", "")) is not None, f"{label} commit")
    require(re.fullmatch(r"[0-9a-f]{40}", value.get("git_tree", "")) is not None, f"{label} tree")
    require(full_sha(value.get("repository_root")), f"{label} root")
    return value


def assess(reference: object) -> dict[str, object]:
    require(isinstance(reference, dict), "reference must be an object")
    require_exact_keys(
        reference,
        {
            "schema",
            "source",
            "objects",
            "object_set_root",
            "completeness",
            "authority",
            "does_not_establish",
        },
        "reference",
    )
    require(reference.get("schema") == "vela.foreign-reference.v1", "schema")

    source = reference.get("source")
    require(isinstance(source, dict), "source")
    require_exact_keys(
        source,
        {
            "frontier_id",
            "current_repository",
            "transition_repository",
            "repository_origin",
            "claim",
            "submission",
            "proposal",
            "verification",
            "decision_event",
            "applied_event",
            "authority_record",
            "authority_keyset_root",
            "standing",
        },
        "source",
    )
    require(prefixed(source.get("frontier_id"), "vfr_", 16), "frontier id")
    current = repository_ref(source.get("current_repository"), "current repository")
    transition = repository_ref(source.get("transition_repository"), "transition repository")
    origin = exact_ref(source.get("repository_origin"), "vro_", 16, "repository origin")
    claim = exact_ref(source.get("claim"), "vcl_", 64, "claim")
    submission = exact_ref(source.get("submission"), "vsb_", 16, "submission")
    proposal = exact_ref(source.get("proposal"), "vpr_", 16, "proposal")
    verification = exact_ref(source.get("verification"), "vvr_", 16, "verification")
    decision = exact_ref(source.get("decision_event"), "vev_", 16, "decision event")
    authority_record = exact_ref(
        source.get("authority_record"), "var_", 16, "authority record"
    )
    applied = source.get("applied_event")
    require(isinstance(applied, dict), "applied event")
    require_exact_keys(applied, {"id", "root", "semantic_id"}, "applied event")
    require(prefixed(applied.get("id"), "vev_", 16), "applied event id")
    require(full_sha(applied.get("root")), "applied event root")
    require(prefixed(applied.get("semantic_id"), "vev_", 16), "semantic event id")
    require(full_sha(source.get("authority_keyset_root")), "authority keyset root")
    require(source.get("standing") == "accepted", "source standing")

    authority = reference.get("authority")
    require(isinstance(authority, dict), "authority")
    require_exact_keys(
        authority,
        {"source_standing", "local_standing_effect", "requires_local_decision"},
        "authority",
    )
    require(authority.get("source_standing") == source["standing"], "standing mismatch")
    require(authority.get("local_standing_effect") == "none", "authority escalation")
    require(authority.get("requires_local_decision") is True, "local decision")

    objects = reference.get("objects")
    require(isinstance(objects, list), "objects")
    keys: list[tuple[str, str, str, str, str]] = []
    for item in objects:
        require(isinstance(item, dict), "object")
        require_exact_keys(item, {"role", "id", "root", "bytes_root", "path"}, "object")
        key = (
            item.get("role"),
            item.get("id"),
            item.get("root"),
            item.get("bytes_root"),
            item.get("path"),
        )
        require(all(isinstance(value, str) and value for value in key), "object field")
        require(full_sha(item["root"]), "object root")
        require(full_sha(item["bytes_root"]), "object bytes root")
        require(relative_path(item["path"]), "object path")
        keys.append(key)
    require(keys == sorted(keys), "object order")
    for index in range(5):
        require(len({key[index] for key in keys}) == len(keys), "duplicate object field")
    require(reference.get("object_set_root") == sha256(objects), "object-set root")
    by_role = {item["role"]: item for item in objects}

    completeness = reference.get("completeness")
    require(isinstance(completeness, dict), "completeness")
    require_exact_keys(completeness, {"status", "missing_roles"}, "completeness")
    missing = completeness.get("missing_roles")
    require(
        isinstance(missing, list)
        and missing == sorted(set(missing))
        and set(missing) <= REQUIRED_ROLES,
        "missing roles",
    )
    require(REQUIRED_ROLES - set(by_role) == set(missing), "completeness mismatch")
    require(
        (completeness.get("status") == "complete" and not missing)
        or (completeness.get("status") == "incomplete" and bool(missing)),
        "completeness status",
    )
    bindings = {
        "current_repository_manifest": ("current-repository-manifest", current["repository_root"]),
        "transition_repository_manifest": (
            "transition-repository-manifest",
            transition["repository_root"],
        ),
        "repository_origin": (origin["id"], origin["root"]),
        "authority_keyset": ("authority-keyset", source["authority_keyset_root"]),
        "claim": (claim["id"], claim["root"]),
        "submission": (submission["id"], submission["root"]),
        "proposal": (proposal["id"], proposal["root"]),
        "verification": (verification["id"], verification["root"]),
        "decision_event": (decision["id"], decision["root"]),
        "applied_event": (applied["id"], applied["root"]),
        "authority_record": (authority_record["id"], authority_record["root"]),
    }
    for role, (object_id, root) in bindings.items():
        if role in missing:
            continue
        item = by_role.get(role)
        require(item is not None, f"missing role {role}")
        require(item["id"] == object_id and item["root"] == root, f"binding {role}")

    nonclaims = reference.get("does_not_establish")
    require(
        isinstance(nonclaims, list)
        and bool(nonclaims)
        and len(set(nonclaims)) == len(nonclaims)
        and all(isinstance(value, str) and value and value.strip() == value for value in nonclaims),
        "nonclaims",
    )

    return {
        "schema": "vela.foreign-reference-assessment.v1",
        "status": completeness["status"],
        "reference_root": sha256(reference),
        "object_set_root": reference["object_set_root"],
        "source_frontier_id": source["frontier_id"],
        "source_current_git_commit": current["git_commit"],
        "source_current_git_tree": current["git_tree"],
        "source_current_repository_root": current["repository_root"],
        "source_transition_git_commit": transition["git_commit"],
        "source_transition_git_tree": transition["git_tree"],
        "source_transition_repository_root": transition["repository_root"],
        "source_repository_origin_id": origin["id"],
        "source_repository_origin_root": origin["root"],
        "source_claim_id": claim["id"],
        "source_claim_root": claim["root"],
        "source_submission_id": submission["id"],
        "source_submission_root": submission["root"],
        "source_proposal_id": proposal["id"],
        "source_proposal_root": proposal["root"],
        "source_verification_id": verification["id"],
        "source_verification_root": verification["root"],
        "source_decision_event_id": decision["id"],
        "source_decision_event_root": decision["root"],
        "source_applied_event_id": applied["id"],
        "source_applied_event_root": applied["root"],
        "source_applied_semantic_event_id": applied["semantic_id"],
        "source_authority_record_id": authority_record["id"],
        "source_authority_record_root": authority_record["root"],
        "source_authority_keyset_root": source["authority_keyset_root"],
        "source_standing": source["standing"],
        "local_standing_effect": authority["local_standing_effect"],
        "requires_local_decision": authority["requires_local_decision"],
        "diagnostics": [f"missing_role:{role}" for role in missing],
    }


# Minimal RFC 8032 Ed25519 verifier. It is intentionally local so this reader
# remains dependency-free and does not borrow the Rust implementation.
Q = 2**255 - 19
L = 2**252 + 27742317777372353535851937790883648493
D = (-121665 * pow(121666, Q - 2, Q)) % Q
I = pow(2, (Q - 1) // 4, Q)


def xrecover(y: int) -> int:
    xx = (y * y - 1) * pow(D * y * y + 1, Q - 2, Q) % Q
    x = pow(xx, (Q + 3) // 8, Q)
    if (x * x - xx) % Q != 0:
        x = x * I % Q
    require((x * x - xx) % Q == 0, "ed25519 point")
    return x


BY = 4 * pow(5, Q - 2, Q) % Q
BX = xrecover(BY)
if BX & 1:
    BX = Q - BX
B = (BX, BY)


def point_add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    x1, y1 = left
    x2, y2 = right
    factor = D * x1 * x2 * y1 * y2 % Q
    x3 = (x1 * y2 + y1 * x2) * pow(1 + factor, Q - 2, Q) % Q
    y3 = (y1 * y2 + x1 * x2) * pow(1 - factor, Q - 2, Q) % Q
    return x3, y3


def scalar_mult(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    result = (0, 1)
    addend = point
    while scalar:
        if scalar & 1:
            result = point_add(result, addend)
        addend = point_add(addend, addend)
        scalar >>= 1
    return result


def decode_point(encoded: bytes) -> tuple[int, int]:
    require(len(encoded) == 32, "ed25519 point length")
    integer = int.from_bytes(encoded, "little")
    y = integer & ((1 << 255) - 1)
    require(y < Q, "ed25519 y")
    x = xrecover(y)
    if (x & 1) != (integer >> 255):
        x = Q - x
    point = (x, y)
    require(scalar_mult(point, L) == (0, 1), "ed25519 subgroup")
    return point


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> None:
    require(len(public_key) == 32 and len(signature) == 64, "ed25519 lengths")
    r_bytes, s_bytes = signature[:32], signature[32:]
    scalar = int.from_bytes(s_bytes, "little")
    require(scalar < L, "ed25519 scalar")
    public = decode_point(public_key)
    r_point = decode_point(r_bytes)
    challenge = int.from_bytes(
        hashlib.sha512(r_bytes + public_key + message).digest(), "little"
    ) % L
    require(
        scalar_mult(B, scalar) == point_add(r_point, scalar_mult(public, challenge)),
        "ed25519 signature",
    )


def verify_identity(binding: dict[str, object]) -> None:
    require(binding.get("schema") == "vela.identity_binding.v0.1", "identity schema")
    preimage = dict(binding)
    preimage["binding_id"] = ""
    preimage["signature"] = ""
    expected = short_id("vib_", preimage)
    require(binding.get("binding_id") == expected, "identity id")
    verify_ed25519(
        bytes.fromhex(str(binding["public_key_hex"])),
        canonical_bytes(preimage),
        bytes.fromhex(str(binding["signature"])),
    )


def verify_signed_object(
    value: dict[str, object], id_field: str, prefix: str
) -> None:
    authentication = value.get("authentication")
    require(isinstance(authentication, dict), f"{prefix} authentication")
    binding = authentication.get("identity_binding")
    require(isinstance(binding, dict), f"{prefix} identity")
    verify_identity(binding)
    preimage = json.loads(json.dumps(value))
    preimage[id_field] = ""
    preimage["authentication"]["signature"] = ""
    require(value.get(id_field) == short_id(prefix, preimage), f"{prefix} id")
    verify_ed25519(
        bytes.fromhex(str(binding["public_key_hex"])),
        canonical_bytes(preimage),
        bytes.fromhex(str(authentication["signature"])),
    )


def canonical_json(path: Path, expected_root: str) -> dict[str, object]:
    value_bytes = path.read_bytes()
    require(sha256_bytes(value_bytes) == expected_root, f"bytes {path.name}")
    value = json.loads(value_bytes)
    require(isinstance(value, dict), f"object {path.name}")
    require(canonical_bytes(value) == value_bytes, f"canonical {path.name}")
    return value


def event_identity(event: dict[str, object]) -> tuple[str, str]:
    content = event.get("content")
    require(isinstance(content, dict), "Event content")
    require(event.get("id") == short_id("vev_", content), "authority Event id")
    semantic = {
        "schema": "vela.event.v0.1",
        "kind": content["kind"],
        "target": content["target"],
        "actor": content["actor"],
        "timestamp": content["timestamp"],
        "reason": content["reason"],
        "before_hash": content["before_hash"],
        "after_hash": content["after_hash"],
        "payload": content["payload"],
        "caveats": content["caveats"],
    }
    return event["id"], short_id("vev_", semantic)


def dsse_pae(payload_type: str, payload: bytes) -> bytes:
    return (
        b"DSSEv1 "
        + str(len(payload_type)).encode()
        + b" "
        + payload_type.encode()
        + b" "
        + str(len(payload)).encode()
        + b" "
        + payload
    )


def verify_package(reference: dict[str, object], package: Path) -> dict[str, object]:
    assessment = assess(reference)
    package = package.resolve()
    by_role = {item["role"]: item for item in reference["objects"]}
    raw: dict[str, bytes] = {}
    parsed: dict[str, dict[str, object]] = {}
    for role, item in by_role.items():
        path = (package / item["path"]).resolve()
        require(path.is_relative_to(package), f"path escape {role}")
        raw[role] = path.read_bytes()
        require(sha256_bytes(raw[role]) == item["bytes_root"], f"bytes mismatch {role}")
        value = json.loads(raw[role])
        require(isinstance(value, dict), f"object {role}")
        require(canonical_bytes(value) == raw[role], f"canonical {role}")
        parsed[role] = value
    if assessment["status"] != "complete":
        return assessment

    source = reference["source"]
    current = parsed["current_repository_manifest"]
    origin = parsed["repository_origin"]
    transition = parsed["transition_repository_manifest"]
    claim = parsed["claim"]
    submission = parsed["submission"]
    proposal = parsed["proposal"]
    verification = parsed["verification"]
    applied = parsed["applied_event"]
    decision = parsed["decision_event"]
    keyset = parsed["authority_keyset"]
    envelope = parsed["authority_record"]

    # This verifier is pinned to the exact historical transfer package. It is
    # evidence-companion code, not a compatibility reader for the current CLI.
    require(current.get("schema") == "vela.repository.v3", "current repository schema")
    require(current.get("frontier_id") == source["frontier_id"], "current frontier")
    require(sha256_bytes(raw["current_repository_manifest"]) == source["current_repository"]["repository_root"], "current root")
    require(current.get("origin_id") == source["repository_origin"]["id"], "current origin id")
    require(current.get("origin_root") == source["repository_origin"]["root"], "current origin root")
    require(
        any(
            entry.get("claim_id") == source["claim"]["id"]
            and entry.get("claim_root") == source["claim"]["root"]
            and entry.get("standing") == "accepted"
            for entry in current.get("accepted_claims", [])
            if isinstance(entry, dict)
        ),
        "current accepted Claim",
    )

    origin_body = dict(origin)
    origin_body["origin_id"] = ""
    require(origin.get("origin_id") == short_id("vro_", origin_body), "origin id")
    require(sha256_bytes(raw["repository_origin"]) == source["repository_origin"]["root"], "origin root")
    predecessor = origin.get("predecessor")
    require(origin.get("kind") == "compaction" and isinstance(predecessor, dict), "compaction origin")
    require(predecessor.get("commit") == source["transition_repository"]["git_commit"], "predecessor commit")
    require(predecessor.get("tree") == source["transition_repository"]["git_tree"], "predecessor tree")
    require(predecessor.get("repository_root") == source["transition_repository"]["repository_root"], "predecessor root")
    require(predecessor.get("authority_head_root") == source["authority_record"]["root"], "authority head")

    require(transition.get("schema") == "vela.repository.v2", "transition schema")
    require(transition.get("frontier_id") == source["frontier_id"], "transition frontier")
    require(transition.get("authority_keyset_root") == source["authority_keyset_root"], "transition keyset")
    require(sha256_bytes(raw["transition_repository_manifest"]) == source["transition_repository"]["repository_root"], "transition root")
    require(
        any(
            entry.get("claim_id") == source["claim"]["id"]
            and entry.get("claim_root") == source["claim"]["root"]
            and entry.get("standing") == "accepted"
            for entry in transition.get("accepted_claims", [])
            if isinstance(entry, dict)
        ),
        "transition accepted Claim",
    )
    for field, exact in (
        ("submissions", source["submission"]),
        ("proposals", source["proposal"]),
        ("verifications", source["verification"]),
    ):
        require(
            any(
                entry.get("id") == exact["id"] and entry.get("root") == exact["root"]
                for entry in transition.get(field, [])
                if isinstance(entry, dict)
            ),
            f"transition {field}",
        )

    claim_identity = {
        "schema": "vela.claim-identity.v1",
        "revision": claim["revision"],
        "assertion": claim["assertion"],
        "conditions": claim["conditions"],
        "evidence": claim["evidence"],
        "provenance": claim["provenance"],
    }
    require(claim.get("claim_id") == f"vcl_{hashlib.sha256(canonical_bytes(claim_identity)).hexdigest()}", "Claim id")
    require(sha256_bytes(raw["claim"]) == source["claim"]["root"], "Claim root")
    verify_signed_object(submission, "submission_id", "vsb_")
    require(sha256_bytes(raw["submission"]) == source["submission"]["root"], "Submission root")
    require(submission["claim"]["assertion"] == claim["assertion"]["text"], "Submission assertion")
    require(submission["claim"]["type"] == claim["assertion"]["kind"], "Submission kind")

    proposal_body = dict(proposal)
    proposal_body["proposal_id"] = ""
    require(proposal.get("proposal_id") == short_id("vpr_", proposal_body), "Proposal id")
    require(sha256_bytes(raw["proposal"]) == source["proposal"]["root"], "Proposal root")
    require(proposal.get("action") == "claim.revise", "Proposal action")
    require(proposal["subject"]["id"] == source["claim"]["id"], "Proposal Claim")
    require(proposal["subject"]["root"] == source["claim"]["root"], "Proposal Claim root")
    require(proposal["producer_package"]["id"] == source["submission"]["id"], "Proposal Submission")
    require(proposal["producer_package"]["root"] == source["submission"]["root"], "Proposal Submission root")

    verify_signed_object(verification, "verification_record_id", "vvr_")
    require(sha256_bytes(raw["verification"]) == source["verification"]["root"], "Verification root")
    require(verification.get("outcome") == "pass", "Verification outcome")
    require(verification["subject"]["claim_id"] == source["claim"]["id"], "Verification Claim")
    require(verification["subject"]["submission_id"] == source["submission"]["id"], "Verification Submission")
    require(verification["subject"]["submission_root"] == source["submission"]["root"], "Verification Submission root")
    require(verification["subject"]["proposal_id"] == source["proposal"]["id"], "Verification Proposal")

    applied_id, applied_semantic = event_identity(applied)
    decision_id, _ = event_identity(decision)
    require(applied_id == source["applied_event"]["id"], "applied Event")
    require(applied_semantic == source["applied_event"]["semantic_id"], "applied semantic Event")
    require(applied["content"]["kind"] == "finding.superseded", "applied kind")
    require(applied["content"]["after_hash"] == source["claim"]["root"], "applied after")
    require(applied["content"]["payload"]["claim_id"] == source["claim"]["id"], "applied Claim")
    require(applied["content"]["payload"]["proposal_id"] == source["proposal"]["id"], "applied Proposal")
    require(applied["content"]["payload"]["repository_after"] == source["transition_repository"]["repository_root"], "applied repository")
    require(decision_id == source["decision_event"]["id"], "Decision Event")
    require(decision["content"]["kind"] == "review.accepted", "Decision kind")
    require(decision["content"]["payload"]["verdict"] == "accepted", "Decision verdict")
    require(decision["content"]["payload"]["proposal_id"] == source["proposal"]["id"], "Decision Proposal")
    require(decision["content"]["payload"]["applied_event_id"] == applied_semantic, "Decision semantic link")

    require(sha256(keyset) == source["authority_keyset_root"], "authority keyset root")
    payload_type = envelope.get("payloadType")
    require(payload_type == "application/vnd.vela.authority-record.v1+json", "authority payload type")
    payload = base64.b64decode(envelope["payload"], validate=True)
    record = json.loads(payload)
    require(canonical_bytes(record) == payload, "authority payload canonical")
    require(record.get("record_id") == short_id("var_", record["content"]), "authority record id")
    require(sha256(record) == source["authority_record"]["root"], "authority record root")
    require(record["content"]["frontier_id"] == source["frontier_id"], "authority frontier")
    require(record["content"]["authority_keyset_root"] == source["authority_keyset_root"], "authority keyset binding")
    require(record["content"]["event_ids"] == [applied_id, decision_id], "authority Events")
    require(record["content"]["after_event_log_root"] == predecessor["archived_event_log_root"], "authority event log")
    require(any(item.get("action") == "review_accept" for item in record["content"]["semantic_approvals"]), "authority approval")
    expected_deltas = {
        ".vela/repository.json": source["transition_repository"]["repository_root"],
        f".vela/authority/events/{applied_id}.json": source["applied_event"]["root"],
        f".vela/authority/events/{decision_id}.json": source["decision_event"]["root"],
    }
    observed_deltas = {
        item["path"]: item["after_root"] for item in record["content"]["object_delta"]
    }
    require(all(observed_deltas.get(path) == root for path, root in expected_deltas.items()), "authority deltas")
    pae = dsse_pae(payload_type, payload)
    verified = set()
    for signed in envelope.get("signatures", []):
        key = next(
            (candidate for candidate in keyset["keys"] if candidate["key_id"] == signed["keyid"]),
            None,
        )
        require(key is not None, "authority signature key")
        sequence = record["content"]["sequence"]
        require(key["valid_from_sequence"] <= sequence, "authority key lower window")
        require(key["valid_through_sequence"] is None or sequence <= key["valid_through_sequence"], "authority key upper window")
        verify_ed25519(
            bytes.fromhex(key["public_key"]),
            pae,
            base64.b64decode(signed["sig"], validate=True),
        )
        require(signed["keyid"] not in verified, "duplicate authority signature")
        verified.add(signed["keyid"])
    require(len(verified) >= keyset["threshold"], "authority threshold")
    return assessment


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--package-root", type=Path, default=DEFAULT_PACKAGE)
    parser.add_argument("--expected", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    package = args.package_root.resolve()
    reference_path = (args.reference or package / "reference.v1.json").resolve()
    expected_path = (args.expected or package / "assessment.v1.json").resolve()
    try:
        reference = json.loads(reference_path.read_text(encoding="utf-8"))
        observed = verify_package(reference, package)
        if expected_path.exists():
            expected = json.loads(expected_path.read_text(encoding="utf-8"))
            require(observed == expected, "assessment mismatch")
        escalated = json.loads(json.dumps(reference))
        escalated["authority"]["local_standing_effect"] = "accepted"
        try:
            assess(escalated)
        except ValueError:
            pass
        else:
            raise ValueError("authority escalation passed")
        if args.json:
            print(json.dumps(observed, sort_keys=True, separators=(",", ":")))
        else:
            print(
                "foreign-reference: ok "
                f"(root={observed['reference_root']}, "
                "semantic_chain=verified, authority_signature=verified, "
                "local_standing_effect=none)"
            )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"foreign-reference: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
