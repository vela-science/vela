#!/usr/bin/env python3
"""Verify the retained Math authority chain without Vela, Rust, Git, or network."""

from __future__ import annotations

import base64
import binascii
import copy
import hashlib
import json
import os
import stat
import sys
from pathlib import Path, PurePosixPath

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / "conformance/fixtures/authority/math-0.972.1"
sys.path.insert(0, str(ROOT / "conformance/readers/python"))
from canonical import canonical_bytes

COMMIT = "9bdabbcc1f77d0dd60458e3e9d91d2ffa01fd476"
TREE = "3c99d1b9c969a8559605a664bdd7280e9729169f"
LATER = "a6a31a528ee86ab79c2aaf4e71e43fc63f4a4e98"
REPOSITORY = "8115c538-7688-40b7-ab75-3c4765bf3c19"
PAYLOAD_TYPE = "application/vnd.vela.authority-record.v1+json"
ZERO_ROOT = "sha256:" + "0" * 64
ACTION = {
    "authority_initialize": ("administrator", "repository", "authority.initialized"),
    "review_accept": ("reviewer", "proposal", "review.accepted"),
    "review_reject": ("reviewer", "proposal", "review.rejected"),
}
METADATA = {
    "README.md",
    "expected.json",
    "negative-vectors.json",
    "source.json",
    "trust-anchor.json",
}
MAX_BYTES = 8 * 1024 * 1024
PINS = {
    "source.json": "sha256:d053aff53d784c57b4107fb6aa4aaf1a5374cb700ec0e24a477c8065feaa0894",
    "expected.json": "sha256:7363e5b60274e923170ebfb448bfed6d9b776aeb6c2607402b0db3a325cf98e6",
    "negative-vectors.json": "sha256:9bc7bf45b5315f7a31910aeccb60902da9a4d5faf0e8fb1c7be08f27eee49ed7",
    "trust-anchor.json": "sha256:738cebec28a246cf437522361f59ebcec340bd552ddcbfa5f8f0f99c5324b590",
}


class Failure(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def require(condition, code):
    if not condition:
        raise Failure(code)


def equal(actual, expected, code):
    require(actual == expected, code)


def fields(value, *names):
    return tuple(value[name] for name in names)


def one(items, name, value):
    return next(item for item in items if item[name] == value)


def event_with_kind(events, kind):
    return next(event for event in events if event["content"]["kind"] == kind)


def object_no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "json_duplicate_property")
        result[key] = value
    return result


def parse_json(data, canonical=False):
    try:
        value = json.loads(data.decode(), object_pairs_hook=object_no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Failure("json_invalid") from error
    require(isinstance(value, dict), "json_object_required")
    if canonical:
        require(data == canonical_bytes(value), "canonical_bytes_mismatch")
    return value


def read_regular(path, limit=MAX_BYTES):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise Failure("fixture_file_unreadable") from error
    try:
        info = os.fstat(descriptor)
        require(stat.S_ISREG(info.st_mode), "fixture_file_not_regular")
        require(stat.S_IMODE(info.st_mode) == 0o644, "fixture_file_mode_invalid")
        require(info.st_size <= limit, "fixture_file_too_large")
        data = os.read(descriptor, limit + 1)
        require(len(data) == info.st_size, "fixture_file_size_changed")
        return data
    finally:
        os.close(descriptor)


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def root(value):
    return digest(canonical_bytes(value))


def blob_id(data):
    header = f"blob {len(data)}\0".encode()
    return hashlib.sha1(header + data).hexdigest()  # Git object identity, not security.


def relative(value):
    require(isinstance(value, str) and value, "source_path_invalid")
    path = PurePosixPath(value)
    valid = not path.is_absolute() and str(path) == value
    valid = valid and all(part not in ("", ".", "..") for part in path.parts)
    require(valid and "\\" not in value and "\0" not in value, "source_path_invalid")
    return value


def fixture_files():
    result = set()
    for directory, names, files in os.walk(FIXTURE, followlinks=False):
        base = Path(directory)
        require(
            all(not (base / name).is_symlink() for name in names),
            "fixture_directory_symlink",
        )
        result.update((base / name).relative_to(FIXTURE).as_posix() for name in files)
    return result


def pinned(name, limit=64 * 1024, canonical=False):
    data = read_regular(FIXTURE / name, limit)
    require(digest(data) == PINS[name], "fixture_metadata_root_mismatch")
    return parse_json(data, canonical=canonical)


def load_fixture():
    source = pinned("source.json", 256 * 1024)
    entries = source.get("files")
    require(
        isinstance(entries, list) and len(entries) == 17,
        "source_manifest_files_invalid",
    )
    paths = [
        relative(entry.get("path")) for entry in entries if isinstance(entry, dict)
    ]
    require(
        len(paths) == 17 and paths == sorted(set(paths)),
        "source_manifest_files_invalid",
    )
    terminal_path = "repository-manifests/db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc.json"
    terminal_subset = [
        path for path in paths if not path.startswith("repository-manifests/")
    ]
    terminal_subset.append(terminal_path)
    later = source.get("confirmed_terminal_subset_byte_identical_through")
    require(
        later == {"commit": LATER, "paths": terminal_subset},
        "source_manifest_later_confirmation_invalid",
    )
    require(fixture_files() == set(paths) | METADATA, "source_allowlist_mismatch")

    documents = {}
    copied = 0
    for entry in entries:
        path = relative(entry["path"])
        relative(entry["source_path"])
        size = entry["size"]
        require(entry["mode"] == "100644", "source_entry_mode_invalid")
        require(
            isinstance(size, int) and 0 < size <= MAX_BYTES, "source_entry_size_invalid"
        )
        data = read_regular(FIXTURE / path, size)
        require(len(data) == size, "source_entry_size_invalid")
        require(digest(data) == entry["raw_sha256"], "source_entry_root_mismatch")
        require(blob_id(data) == entry["git_blob_sha1"], "source_entry_blob_mismatch")
        documents[path] = parse_json(data, canonical=True)
        copied += len(data)
    require(copied == 56876, "source_copied_bytes_mismatch")

    anchor = pinned("trust-anchor.json", 4096, canonical=True)
    expected = pinned("expected.json")
    negatives = pinned("negative-vectors.json")
    return {
        "documents": documents,
        "anchor": anchor,
        "expected": expected,
        "negatives": negatives,
    }


def pae(payload):
    encoded = PAYLOAD_TYPE.encode()
    return (
        b"DSSEv1 "
        + str(len(encoded)).encode()
        + b" "
        + encoded
        + b" "
        + str(len(payload)).encode()
        + b" "
        + payload
    )


def validate_keyset(keyset, expected_root):
    require(root(keyset) == expected_root, "authority_keyset_root_mismatch")
    require(
        fields(keyset, "repository_id", "threshold", "generation") == (REPOSITORY, 1, 1)
        and len(keyset["keys"]) == 1,
        "authority_keyset_invalid",
    )
    key = keyset["keys"][0]
    try:
        public = bytes.fromhex(key["public_key"])
    except ValueError as error:
        raise Failure("authority_keyset_invalid") from error
    require(
        fields(
            key,
            "algorithm",
            "purpose",
            "valid_from_sequence",
            "valid_through_sequence",
        )
        == ("ed25519", "repository_authority", 1, None),
        "authority_keyset_invalid",
    )
    algorithm = b"ssh-ed25519"
    wire = (
        len(algorithm).to_bytes(4, "big")
        + algorithm
        + len(public).to_bytes(4, "big")
        + public
    )
    fingerprint = base64.b64encode(hashlib.sha256(wire).digest()).decode().rstrip("=")
    require(
        key["key_id"] == f"ssh-ed25519:SHA256:{fingerprint}",
        "authority_key_fingerprint_mismatch",
    )


def validate_model(model, expected_root):
    require(root(model) == expected_root, "authorization_model_root_mismatch")
    require(
        fields(model, "schema", "repository_id", "previous_model_root")
        == ("vela.authorization-model.v1", REPOSITORY, None)
        and model["members"],
        "authorization_model_invalid",
    )


def evaluate(model, request):
    resource = request.get("resource", {})
    action = request.get("action")
    require(
        action in ACTION and isinstance(resource, dict), "authorization_request_invalid"
    )
    role, resource_type, _ = ACTION[action]
    require(
        fields(
            request, "model_root", "repository_id", "principal_class", "recovery_recent"
        )
        == (root(model), model["repository_id"], "human", False)
        and fields(resource, "repository_id", "resource_type")
        == (request["repository_id"], resource_type),
        "authorization_request_invalid",
    )
    roles = {
        item["role"]
        for item in model["members"]
        if item["principal_id"] == request["principal_id"]
    }
    if not roles:
        reason = "unknown_member"
    elif role not in roles:
        reason = "role_action_mismatch"
    else:
        reason = "member_role_authorized"
    allowed = reason == "member_role_authorized"
    return {
        "schema": "vela.authorization-evaluation.v1",
        "profile": "vela.repository-authorization.v1",
        "model_root": root(model),
        "request_root": root(request),
        "decision": "allow" if allowed else "deny",
        "reason": reason,
        "matched_role": role if allowed else None,
    }


def authorize(model, request):
    evaluation = evaluate(model, request)
    if evaluation["decision"] != "allow":
        raise Failure(f"authorization_denied_{evaluation['reason']}")
    return evaluation


def verify_signature(envelope, payload, keyset, sequence):
    signatures = envelope.get("signatures")
    require(isinstance(signatures, list), "authority_signature_threshold_not_met")
    key = keyset["keys"][0]
    active = key["valid_from_sequence"] <= sequence and (
        key["valid_through_sequence"] is None
        or sequence <= key["valid_through_sequence"]
    )
    signature = next(
        (item for item in signatures if item.get("keyid") == key["key_id"]), None
    )
    require(active and signature is not None, "authority_signature_threshold_not_met")
    try:
        public = Ed25519PublicKey.from_public_bytes(bytes.fromhex(key["public_key"]))
        public.verify(base64.b64decode(signature["sig"], validate=True), pae(payload))
    except (InvalidSignature, ValueError, TypeError, binascii.Error) as error:
        raise Failure("authority_signature_threshold_not_met") from error


def decode_record(envelope, keyset, sequence):
    require(
        envelope.get("payloadType") == PAYLOAD_TYPE, "authority_payload_type_invalid"
    )
    try:
        payload = base64.b64decode(envelope["payload"], validate=True)
    except (KeyError, TypeError, binascii.Error) as error:
        raise Failure("authority_payload_invalid") from error
    record = parse_json(payload, canonical=True)
    verify_signature(envelope, payload, keyset, sequence)
    content = record.get("content")
    require(
        record.get("schema") == "vela.authority-record.v1"
        and isinstance(content, dict),
        "authority_record_invalid",
    )
    require(
        record.get("record_id") == "var_" + root(content)[7:23],
        "authority_record_content_address_invalid",
    )
    return record, root(record)


def verify_authorization(record, model, action):
    content = record["content"]
    claim = content["authorization"]
    request = claim["request"]
    require(
        fields(claim, "model_root")
        + fields(
            request, "action", "authentication_root", "principal_id", "intent_digest"
        )
        == (
            root(model),
            action,
            root(content["authentication"]),
            content["principal"]["principal_id"],
            content["intent_digest"],
        ),
        "authorization_binding_mismatch",
    )
    require(
        claim["evaluation"] == authorize(model, request),
        "authorization_evaluation_mismatch",
    )
    approval = one(content["semantic_approvals"], "action", action)
    require(
        fields(approval, "principal_id", "intent_digest", "approved_at")
        == (request["principal_id"], content["intent_digest"], content["recorded_at"]),
        "semantic_approval_mismatch",
    )


def verify_records(envelopes, keyset, model, anchor, expected):
    expected_records = expected["records"]
    require(len(envelopes) == len(expected_records), "authority_chain_position_invalid")
    records = []
    previous = None
    for sequence, (envelope, item) in enumerate(
        zip(envelopes, expected_records, strict=True), 1
    ):
        record, record_root = decode_record(envelope, keyset, sequence)
        content = record["content"]
        actual = (
            content.get("repository_id"),
            content.get("sequence"),
            content.get("previous_authority_record_root"),
            content.get("authority_keyset_root"),
            record.get("record_id"),
            record_root,
        )
        wanted = (
            REPOSITORY,
            sequence,
            previous,
            root(keyset),
            item["record_id"],
            item["record_root"],
        )
        require(
            actual == wanted and item["sequence"] == sequence,
            "authority_chain_position_invalid",
        )
        verify_authorization(record, model, item["action"])
        records.append(record)
        previous = record_root
    require(anchor.get("repository_id") == REPOSITORY, "authority_anchor_mismatch")
    require(
        anchor.get("first_authority_record_root") == root(records[0]),
        "authority_anchor_mismatch",
    )
    require(
        previous == expected.get("final_authority_record_root"),
        "authority_head_mismatch",
    )
    return records


def verify_event(event):
    content = event.get("content")
    require(
        event.get("schema") == "vela.event.v1" and isinstance(content, dict),
        "authority_event_invalid",
    )
    require(
        event.get("id") == "vev_" + root(content)[7:23],
        "authority_event_content_address_invalid",
    )
    return root(event)


def event_log_root(initial, events):
    roots = [
        verify_event(event) for event in sorted(events, key=lambda item: item["id"])
    ]
    return root(
        {
            "schema": "vela.authority-event-log.v1",
            "legacy_event_log_root": initial,
            "authority_event_roots": roots,
        }
    )


def verify_events(records, events, expected):
    require(len(events) == 5, "authority_event_coverage_mismatch")
    current = expected["initial_event_log_root"]
    cumulative = []
    covered = set()
    for record in records:
        content = record["content"]
        repository_delta = one(content["object_delta"], "path", ".vela/repository.json")
        request = content["authorization"]["request"]
        read_root = repository_delta["before_root"] or ZERO_ROOT
        equal(
            request["transaction_read_set_root"],
            read_root,
            "authorization_read_set_mismatch",
        )
        require(
            read_root != content["execution"]["transaction_read_set_root"],
            "execution_read_set_not_distinct",
        )
        equal(content["before_event_log_root"], current, "authority_event_log_mismatch")
        event_ids = content["event_ids"]
        require(
            set(event_ids) <= events.keys() and covered.isdisjoint(event_ids),
            "authority_event_coverage_mismatch",
        )
        transaction_events = [events[event_id] for event_id in event_ids]
        principal = content["principal"]["principal_id"]
        for event in transaction_events:
            event_content = event["content"]
            equal(
                fields(event_content, "transaction_id", "principal_id")
                + fields(event_content["actor"], "id"),
                (content["transaction_id"], principal, principal),
                "authority_event_coverage_mismatch",
            )
            payload = event_content["payload"]
            if "proposal_id" in payload:
                equal(
                    payload["proposal_id"],
                    request["resource"]["resource_id"],
                    "authority_event_proposal_mismatch",
                )
            if "repository_before" in payload:
                equal(
                    fields(payload, "repository_before", "repository_after"),
                    fields(repository_delta, "before_root", "after_root"),
                    "authority_event_repository_delta_mismatch",
                )
        decision_kind = ACTION[request["action"]][2]
        decision = one(
            [event["content"] for event in transaction_events], "kind", decision_kind
        )
        approval = one(content["semantic_approvals"], "action", request["action"])
        equal(
            fields(decision["target"], "id")
            + fields(decision, "reason", "timestamp", "principal_id"),
            fields(request["resource"], "resource_id")
            + fields(approval, "reason", "approved_at", "principal_id"),
            "authority_event_approval_mismatch",
        )
        covered.update(event_ids)
        cumulative.extend(transaction_events)
        current = event_log_root(expected["initial_event_log_root"], cumulative)
        equal(content["after_event_log_root"], current, "authority_event_log_mismatch")
    equal(covered, set(events), "authority_event_coverage_mismatch")
    equal(current, expected["final_event_log_root"], "authority_event_log_mismatch")
    payload = event_with_kind(events.values(), "authority.initialized")["content"][
        "payload"
    ]
    names = (
        "repository_id",
        "initial_event_log_root",
        "initial_actor_registry_root",
        "new_authority_keyset_root",
        "new_authorization_model_root",
    )
    equal(
        fields(payload, *names),
        (
            REPOSITORY,
            expected["initial_event_log_root"],
            expected["initial_actor_registry_root"],
            expected["authority_keyset_root"],
            expected["authorization_model_root"],
        ),
        "authority_initialization_mismatch",
    )


def verify_deltas(records, documents, events, keyset, model):
    manifests = {
        "sha256:" + path[21:-5]: value
        for path, value in documents.items()
        if path.startswith("repository-manifests/")
    }
    require(
        all(root(value) == name for name, value in manifests.items()),
        "repository_delta_preimage_mismatch",
    )
    known = {".vela/origin.json": root(documents["origin.json"])}
    known.update(
        {
            f".vela/authority/events/{name}.json": root(event)
            for name, event in events.items()
        }
    )
    known[f".vela/authority/keysets/{root(keyset)[7:]}.json"] = root(keyset)
    known[f".vela/authority/models/{root(model)[7:]}.json"] = root(model)
    for record in records:
        deltas = record["content"]["object_delta"]
        paths = [delta["path"] for delta in deltas]
        require(
            len(paths) == len(set(paths))
            and all(delta["before_root"] != delta["after_root"] for delta in deltas),
            "authority_object_delta_invalid",
        )
        for delta in deltas:
            path = delta["path"]
            if path == ".vela/repository.json":
                roots = fields(delta, "before_root", "after_root")
                require(
                    roots[1] in manifests
                    and (roots[0] is None or roots[0] in manifests),
                    "repository_delta_preimage_mismatch",
                )
            else:
                equal(
                    known.get(path),
                    delta["after_root"],
                    "authority_object_delta_mismatch",
                )


def verify_write_set(record):
    content = record["content"]
    commitment = {
        "schema": "vela.authority-write-set.internal.v1",
        "transaction_id": content["transaction_id"],
        "before_event_log_root": content["before_event_log_root"],
        "after_event_log_root": content["after_event_log_root"],
        "event_ids": content["event_ids"],
        "object_delta": content["object_delta"],
    }
    derived = digest(
        b"vela.authority-write-set.internal.v1\0" + canonical_bytes(commitment)
    )
    require(
        derived == content["execution"]["transaction_write_set_root"],
        "authority_write_set_root_mismatch",
    )


def semantic_event_id(event):
    content = event["content"]
    excluded = {"authority_mode", "principal_id", "transaction_id"}
    value = {key: item for key, item in content.items() if key not in excluded}
    value["schema"] = "vela.event.v0.1"
    return "vev_" + root(value)[7:23]


def verify_terminal(manifest, events, expected):
    terminal = expected["terminal"]
    equal(
        root(manifest),
        terminal["repository_manifest_root"],
        "authority_terminal_state_mismatch",
    )
    collections = {
        "accepted_claims": "accepted_claims",
        "pending_claims": "pending_claims",
        "proposals": "proposals",
        "withdrawals": "proposal_withdrawals",
        "submissions": "submissions",
        "verifications": "verifications",
        "artifacts": "artifacts",
    }
    counts = {name: len(manifest.get(key, [])) for name, key in collections.items()}
    equal(counts, terminal["counts"], "authority_terminal_state_mismatch")
    claim_root = terminal["accepted_claim_root"]
    accepted = [
        {
            "claim_id": terminal["accepted_claim_id"],
            "claim_root": claim_root,
            "path": f"records/claims/sha256/{claim_root[7:]}.json",
            "standing": "accepted",
        }
    ]
    equal(manifest["accepted_claims"], accepted, "authority_terminal_state_mismatch")
    applied = event_with_kind(events.values(), "claim.asserted")
    review = event_with_kind(events.values(), "review.accepted")
    semantic = terminal["applied_semantic_event_id"]
    content = applied["content"]
    equal(
        fields(content["target"], "id")
        + fields(content["payload"], "claim_id", "claim_root")
        + fields(content, "after_hash"),
        (
            terminal["accepted_claim_id"],
            terminal["accepted_claim_id"],
            claim_root,
            claim_root,
        ),
        "authority_terminal_state_mismatch",
    )
    equal(semantic_event_id(applied), semantic, "authority_terminal_state_mismatch")
    applied_id = review["content"]["payload"]["applied_event_id"]
    equal(applied_id, semantic, "authority_terminal_state_mismatch")


def materialize(bundle):
    documents = bundle["documents"]
    expected = bundle["expected"]
    keyset = documents[
        f"authority/keysets/{expected['authority_keyset_root'][7:]}.json"
    ]
    model = documents[
        f"authority/models/{expected['authorization_model_root'][7:]}.json"
    ]
    envelopes = [documents[item["path"]] for item in expected["records"]]
    events = {
        value["id"]: value
        for path, value in documents.items()
        if path.startswith("authority/events/")
    }
    return keyset, model, envelopes, events


def verify_positive(bundle):
    expected = bundle["expected"]
    require(
        expected.get("repository_id") == REPOSITORY, "authority_expectation_invalid"
    )
    keyset, model, envelopes, events = materialize(bundle)
    validate_keyset(keyset, expected["authority_keyset_root"])
    validate_model(model, expected["authorization_model_root"])
    records = verify_records(envelopes, keyset, model, bundle["anchor"], expected)
    verify_events(records, events, expected)
    verify_deltas(records, bundle["documents"], events, keyset, model)
    for record, item in zip(records, expected["records"], strict=True):
        verify_write_set(record)
        require(
            record["content"]["event_ids"] == item["event_ids"],
            "authority_event_coverage_mismatch",
        )
        require(
            record["content"]["execution"]["transaction_write_set_root"]
            == item["write_set_root"],
            "authority_write_set_root_mismatch",
        )
    terminal = expected["terminal"]["repository_manifest_root"]
    final_delta = next(
        item
        for item in records[-1]["content"]["object_delta"]
        if item["path"] == ".vela/repository.json"
    )
    require(final_delta["after_root"] == terminal, "authority_terminal_state_mismatch")
    verify_terminal(
        bundle["documents"][f"repository-manifests/{terminal[7:]}.json"],
        events,
        expected,
    )
    return {"records": records, "events": events}


def expect_failure(vector, function, arguments):
    try:
        function(*arguments)
    except Failure as error:
        require(error.code == vector["expected_code"], "negative_vector_wrong_failure")
        return {"id": vector["id"], "code": error.code}
    raise Failure("negative_vector_did_not_fail")


def run_negatives(bundle, positive):
    vectors = bundle["negatives"].get("vectors")
    require(
        isinstance(vectors, list) and len(vectors) == 13,
        "negative_vector_inventory_invalid",
    )
    keyset, model, envelopes, events = materialize(bundle)
    expected = bundle["expected"]
    records = positive["records"]
    bad_anchor = copy.deepcopy(bundle["anchor"])
    bad_anchor["first_authority_record_root"] = ZERO_ROOT
    bad_signature = copy.deepcopy(envelopes)
    signature = bytearray(base64.b64decode(bad_signature[3]["signatures"][0]["sig"]))
    signature[0] ^= 1
    bad_signature[3]["signatures"][0]["sig"] = base64.b64encode(signature).decode()
    bad_head = copy.deepcopy(expected)
    bad_head["final_authority_record_root"] = ZERO_ROOT
    bad_keyset = copy.deepcopy(keyset)
    public_key = bad_keyset["keys"][0]["public_key"]
    bad_keyset["keys"][0]["public_key"] = "13" + public_key[2:]
    bad_model = copy.deepcopy(model)
    del bad_model["members"][1]
    bad_request = copy.deepcopy(records[3]["content"]["authorization"]["request"])
    bad_request["principal_id"] = "local:unbound|uid:0"
    key_path = f"authority/keysets/{expected['authority_keyset_root'][7:]}.json"
    noncanonical_keyset = read_regular(FIXTURE / key_path) + b"\n"
    bad_event = copy.deepcopy(events["vev_fb652e14f2a9323f"])
    bad_event["content"]["payload"]["verdict"] = "rejected"
    missing_event = {
        key: value for key, value in events.items() if key != "vev_fb652e14f2a9323f"
    }
    bad_documents = copy.deepcopy(bundle["documents"])
    bc36 = "repository-manifests/bc36be46a09ca4aafd99b20c384bcf6a807e0094e6e7a55879d943cefbf041d5.json"
    claim = bad_documents[bc36]["pending_claims"][0]
    claim["claim_root"] = claim["claim_root"][:-1] + "7"
    bad_record = copy.deepcopy(records[3])
    bad_record["content"]["object_delta"].pop()
    terminal = expected["terminal"]["repository_manifest_root"]
    manifest = bundle["documents"][f"repository-manifests/{terminal[7:]}.json"]
    bad_terminal = copy.deepcopy(expected)
    claim_root = bad_terminal["terminal"]["accepted_claim_root"]
    bad_terminal["terminal"]["accepted_claim_root"] = claim_root[:-1] + "7"
    history = (keyset, model, bundle["anchor"], expected)
    cases = [
        (
            "wrong-trust-anchor",
            verify_records,
            (envelopes, keyset, model, bad_anchor, expected),
        ),
        ("bad-final-signature", verify_records, (bad_signature, *history)),
        (
            "missing-sequence-two",
            verify_records,
            ([envelopes[0], *envelopes[2:]], *history),
        ),
        (
            "wrong-expected-head",
            verify_records,
            (envelopes, keyset, model, bundle["anchor"], bad_head),
        ),
        (
            "wrong-keyset-public-key",
            validate_keyset,
            (bad_keyset, expected["authority_keyset_root"]),
        ),
        (
            "missing-reviewer-member",
            validate_model,
            (bad_model, expected["authorization_model_root"]),
        ),
        ("unknown-member-evaluation", authorize, (model, bad_request)),
        ("noncanonical-keyset-bytes", parse_json, (noncanonical_keyset, True)),
        ("mutated-accepted-review-event", verify_event, (bad_event,)),
        (
            "missing-accepted-review-event",
            verify_events,
            (records, missing_event, expected),
        ),
        (
            "mutated-sequence-four-preimage",
            verify_deltas,
            (records, bad_documents, events, keyset, model),
        ),
        ("mutated-sequence-four-object-delta", verify_write_set, (bad_record,)),
        (
            "wrong-terminal-claim-root",
            verify_terminal,
            (manifest, events, bad_terminal),
        ),
    ]
    equal(
        [item[0] for item in cases],
        [item["id"] for item in vectors],
        "negative_vector_order_invalid",
    )
    return [
        expect_failure(vector, function, arguments)
        for vector, (_, function, arguments) in zip(vectors, cases, strict=True)
    ]


def main():
    try:
        bundle = load_fixture()
        positive = verify_positive(bundle)
        negatives = run_negatives(bundle, positive)
        expected = bundle["expected"]
        output = {
            "schema": "vela.authority-chain-verification-result.v1",
            "fixture_id": "math-0.972.1",
            "result": "pass",
            "source_commit": COMMIT,
            "source_tree": TREE,
            "authority_record_count": len(positive["records"]),
            "authority_event_count": len(positive["events"]),
            "first_authority_record_root": bundle["anchor"][
                "first_authority_record_root"
            ],
            "final_authority_record_root": expected["final_authority_record_root"],
            "final_event_log_root": expected["final_event_log_root"],
            "terminal_repository_manifest_root": expected["terminal"][
                "repository_manifest_root"
            ],
            "negative_vectors": negatives,
            "authority_effect": "none",
        }
        sys.stdout.buffer.write(canonical_bytes(output) + b"\n")
        return 0
    except Failure as error:
        print(f"authority-chain: {error.code}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
