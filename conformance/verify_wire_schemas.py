#!/usr/bin/env python3
"""Check the documented current-object JSON Schema boundary and frozen roots."""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker


ROOT = Path(__file__).resolve().parent.parent
CONFORMANCE = ROOT / "conformance"
SCHEMAS = ROOT / "schemas"
sys.path.insert(0, str(CONFORMANCE / "readers" / "python"))
from canonical import canonical_bytes  # noqa: E402


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def validator(name: str) -> Draft202012Validator:
    schema = load_json(SCHEMAS / name)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, format_checker=FormatChecker())


def expect_rejected(checker: Draft202012Validator, value: object, label: str) -> None:
    if checker.is_valid(value):
        raise AssertionError(f"schema accepted negative case: {label}")


def collect_patterns(node: object, seen: list[str]) -> list[str]:
    if isinstance(node, dict):
        pattern = node.get("pattern")
        if isinstance(pattern, str):
            seen.append(pattern)
        for value in node.values():
            collect_patterns(value, seen)
    elif isinstance(node, list):
        for value in node:
            collect_patterns(value, seen)
    return seen


def verify_patterns_are_portable() -> int:
    """Hold every published pattern to the portable regular subset.

    A pattern that needs lookahead, a backreference, or a possessive quantifier
    is one an implementer on a finite-automaton engine cannot compile, and the
    schemas are published for implementers on engines we do not choose. The
    Artifact-path rule was that pattern once; this keeps a later one from
    arriving unnoticed.
    """
    unportable = re.compile(r"\(\?[=!<]|\\[1-9]|[*+?}]\+")
    checked = 0
    for path in sorted(SCHEMAS.glob("*.schema.json")):
        for pattern in collect_patterns(load_json(path), []):
            found = unportable.search(pattern)
            if found:
                raise AssertionError(
                    f"{path.name} uses {found.group(0)!r}, "
                    f"outside the portable subset: {pattern}"
                )
            re.compile(pattern)
            checked += 1
    return checked


def verify_manifest() -> None:
    path = CONFORMANCE / "current-objects" / "manifest.v1.json"
    manifest = load_json(path)
    if not isinstance(manifest, dict):
        raise AssertionError("fixture manifest must be an object")
    declared_root = manifest.pop("manifest_root", None)
    rebuilt_root = "sha256:" + hashlib.sha256(canonical_bytes(manifest)).hexdigest()
    if declared_root != rebuilt_root:
        raise AssertionError(
            f"fixture manifest root drift: declared {declared_root}, rebuilt {rebuilt_root}"
        )
    for entry in manifest.get("files", []):
        fixture = CONFORMANCE / "current-objects" / entry["path"]
        payload = fixture.read_bytes()
        digest = "sha256:" + hashlib.sha256(payload).hexdigest()
        if len(payload) != entry["bytes"] or digest != entry["sha256"]:
            raise AssertionError(f"frozen fixture drift: {entry['path']}")


def status_document(**overrides: object) -> dict:
    """A replaying Frontier's `vela.status.v4`, as the CLI emits it."""
    document = {
        "schema": "vela.status.v4",
        "ok": True,
        "command": "status",
        "repository": {
            "id": "vrepo_0a25edabc16db1430a25edabc16db143",
            "name": "Fixture repository",
            "profile_root": "sha256:" + "1" * 64,
        },
        "git": {"role": "repository_head", "commit": "a" * 40, "tree": "b" * 40},
        "integrity": {
            "replay": "verified",
            "strict": "pass",
            "blocker_count": 0,
            "blockers_by_code": {},
        },
        "roots": {
            "origin": "sha256:" + "2" * 64,
            "repository": "sha256:" + "3" * 64,
            "authority_keyset": "sha256:" + "4" * 64,
            "authority_policy": "sha256:" + "5" * 64,
        },
        "counts": {
            "claims": 2,
            "accepted_claims": 1,
            "pending_claims": 1,
            "pending_review": 1,
            "accepted_review": 1,
            "rejected_review": 0,
            "withdrawn_review": 0,
            "submissions": 2,
            "verifications": 1,
            "artifacts": 3,
        },
        "work": {"ready_target_count": 4},
        "decision_inbox": {
            "pending_count": 1,
            "protocol_ready_count": 1,
            "protocol_blocked_count": 0,
            "projection_root": "sha256:" + "6" * 64,
            "first_entry_root": "sha256:" + "7" * 64,
        },
        "actions": {
            "review": {"pending_count": 1, "command": "vela review inbox . --json"},
            "work": {
                "mode": "target",
                "ready_target_count": 4,
                "command": "vela next . --limit 1 --json",
            },
        },
    }
    document.update(overrides)
    return document


def verify_status_read_surface() -> tuple[int, int]:
    """Hold `status-v4.schema.json` to the two documents `vela status` emits.

    This schema describes a read surface rather than a signed object, so there
    is no canonical-bytes fixture behind it and no signature to check. What
    there is instead is a second implementation — the Observatory reads this
    document and nothing else to build its projection — and the cases below
    are the ones that consumer would be broken by.

    The negative cases are all one rule: a field whose value is null on the
    branch that cannot fill it is still a field, and a document that drops the
    key is a different document. Three field-shape changes reached the
    Observatory as fail-closed breaks in one week; a dropped key would reach it
    as a field silently read as absent.

    The positive at the end is the other half of that rule, and it used to be a
    negative. This schema asserted `additionalProperties: false` on every
    object, so a document carrying a field the schema does not name was
    rejected — and the three breakages above were all exactly that, upstream
    adding a field. Closure detected nothing the required lists do not already
    detect and refused three additive changes, so it is gone. A conformant
    reader of this document reads past what it does not recognize;
    `docs/INTEROPERABILITY.md` states the rule and its opposite, which governs
    signed preimages and stays closed.
    """
    check = validator("status-v4.schema.json")

    replaying = status_document()
    check.validate(replaying)

    # A Frontier whose repository authority has not finished initializing
    # answers the same document, with the anchors it does not have yet null.
    bootstrapping = status_document(
        git={"role": "repository_head", "commit": None, "tree": None},
        integrity={
            "replay": "not_initialized",
            "strict": "blocked",
            "blocker_count": 1,
            "blockers_by_code": {"repository_authority_uninitialized": 1},
        },
        roots={
            "origin": None,
            "repository": None,
            "authority_keyset": None,
            "authority_policy": None,
        },
        counts=dict.fromkeys(status_document()["counts"], 0),
        work={"ready_target_count": 0},
        decision_inbox={
            "pending_count": 0,
            "protocol_ready_count": 0,
            "protocol_blocked_count": 0,
            "projection_root": None,
            "first_entry_root": None,
        },
        actions={
            "review": None,
            "work": {
                "mode": "authority_uninitialized",
                "ready_target_count": 0,
                "command": "vela init . --json",
                "note": "Resume `vela init`.",
            },
        },
    )
    check.validate(bootstrapping)

    negatives = 0

    for absent in ("commit", "tree"):
        mutated = copy.deepcopy(bootstrapping)
        del mutated["git"][absent]
        expect_rejected(check, mutated, f"status git without {absent}")
        negatives += 1

    for absent in ("projection_root", "first_entry_root"):
        mutated = copy.deepcopy(bootstrapping)
        del mutated["decision_inbox"][absent]
        expect_rejected(check, mutated, f"status decision inbox without {absent}")
        negatives += 1

    mutated = copy.deepcopy(bootstrapping)
    del mutated["roots"]["origin"]
    expect_rejected(check, mutated, "status roots without origin")
    negatives += 1

    mutated = copy.deepcopy(bootstrapping)
    del mutated["actions"]["review"]
    expect_rejected(check, mutated, "status actions without review")
    negatives += 1

    mutated = copy.deepcopy(replaying)
    mutated["schema"] = "vela.status.v1"
    expect_rejected(check, mutated, "status under a retired schema tag")
    negatives += 1

    mutated = copy.deepcopy(replaying)
    mutated["integrity"]["replay"] = "passed"
    expect_rejected(check, mutated, "status replay outside its vocabulary")
    negatives += 1

    mutated = copy.deepcopy(replaying)
    mutated["actions"]["work"]["mode"] = "inspect"
    expect_rejected(check, mutated, "status work mode outside its vocabulary")
    negatives += 1

    mutated = copy.deepcopy(replaying)
    mutated["git"]["commit"] = "sha256:" + "a" * 64
    expect_rejected(check, mutated, "status git commit as a Vela root")
    negatives += 1

    # An added field, at the top level and inside the tagged work action, which
    # is the shape all three Observatory breakages took. Both were negatives
    # until 2026-08-07 and are the compatible change this document promises.
    additive = copy.deepcopy(replaying)
    additive["counts_by_source"] = {"source:fixture": 1}
    additive["actions"]["work"]["explanation"] = "why this Target is next"
    check.validate(additive)

    return 3, negatives


def main() -> int:
    submission_check = validator("submission-v1.schema.json")
    verification_check = validator("verification-record-v1.schema.json")
    withdrawal_check = validator("proposal-withdrawal-v1.schema.json")
    authority_envelope_check = validator("authority-envelope-v1.schema.json")

    submission = load_json(CONFORMANCE / "current-objects" / "submission.json")
    verification = load_json(CONFORMANCE / "current-objects" / "verification.json")
    submission_check.validate(submission)
    verification_check.validate(verification)

    withdrawal = {
        "schema": "vela.proposal-withdrawal.v1",
        "withdrawal_id": "vpw_0123456789abcdef",
        "proposal_id": "vpr_0123456789abcdef",
        "proposal_root": "sha256:" + "1" * 64,
        "submission_id": "vsb_0123456789abcdef",
        "submission_root": "sha256:" + "2" * 64,
        "actor": "agent:fixture",
        "reason": "The producer withdraws this pending fixture.",
        "created_at": "2026-08-03T00:00:00Z",
        "authentication": {"algorithm": "ed25519", "signature": "3" * 128},
    }
    withdrawal_check.validate(withdrawal)
    authority_envelope = {
        "payloadType": "application/vnd.vela.authority-record.v1+json",
        "payload": "e30=",
        "signatures": [{"sig": "YWJj", "future": "ignored"}],
        "future": "ignored",
    }
    authority_envelope_check.validate(authority_envelope)

    mutated = copy.deepcopy(submission)
    mutated["unexpected"] = True
    expect_rejected(submission_check, mutated, "submission unknown field")
    mutated = copy.deepcopy(submission)
    mutated["requested_change"]["target"] = {
        "claim_id": "vcl_" + "a" * 64,
        "claim_root": "sha256:" + "b" * 64,
    }
    expect_rejected(submission_check, mutated, "add_claim with target")
    mutated = copy.deepcopy(submission)
    mutated["artifacts"][0]["digest"] = "sha256:short"
    expect_rejected(submission_check, mutated, "short artifact root")
    for escape in ("/absolute", "..", "../escape", "a/../b", "a/..", " a", "a\n/.."):
        mutated = copy.deepcopy(submission)
        mutated["artifacts"][0]["path"] = escape
        expect_rejected(submission_check, mutated, f"artifact path {escape!r}")
    # `artifacts/` ends on a component that is empty, not on whitespace, and
    # the reader admits it; the published pattern has to admit it too.
    for safe in (
        "a",
        "artifacts/result.json",
        ".vela/work/run/report.json",
        "a/./b",
        "..a",
        "artifacts/",
    ):
        mutated = copy.deepcopy(submission)
        mutated["artifacts"][0]["path"] = safe
        submission_check.validate(mutated)

    mutated = copy.deepcopy(verification)
    mutated["subject"]["submission_id"] = "vsb_"
    expect_rejected(verification_check, mutated, "submission reference with no body")
    mutated = copy.deepcopy(verification)
    mutated["outcome"] = "accepted"
    expect_rejected(verification_check, mutated, "verification implies acceptance")
    mutated = copy.deepcopy(verification)
    mutated["scope"]["does_not_establish"] = []
    expect_rejected(verification_check, mutated, "missing verification nonclaim")
    mutated = copy.deepcopy(withdrawal)
    mutated["authentication"]["algorithm"] = "none"
    expect_rejected(withdrawal_check, mutated, "withdrawal without Ed25519")
    mutated = copy.deepcopy(authority_envelope)
    mutated["signatures"] = []
    expect_rejected(authority_envelope_check, mutated, "authority envelope without signatures")

    mutated = copy.deepcopy(withdrawal)
    mutated["withdrawal_id"] = "vpw_"
    expect_rejected(withdrawal_check, mutated, "withdrawal reference with no body")

    positive, negative = verify_status_read_surface()

    patterns = verify_patterns_are_portable()
    verify_manifest()
    schemas = len(list(SCHEMAS.glob("*.schema.json")))
    print(
        f"wire-schemas: ok ({schemas} schemas, {10 + positive} positive objects, "
        f"{16 + negative} negative cases, {patterns} portable patterns)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
