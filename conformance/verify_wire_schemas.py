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

    patterns = verify_patterns_are_portable()
    verify_manifest()
    print(
        f"wire-schemas: ok (4 schemas, 10 positive objects, 16 negative cases, "
        f"{patterns} portable patterns)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
