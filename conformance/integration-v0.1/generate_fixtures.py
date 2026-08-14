#!/usr/bin/env python3
"""Generate the draft integration packet and hostile mutation corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "readers" / "python"))
from canonical import canonical_bytes

SCHEMAS = {
    "manifest": "vela.integration-manifest.v0.1",
    "profile": "vela.integration-profile.v0.1",
    "binding": "vela.integration-binding.v0.1",
    "method": "vela.integration-method.v0.1",
}
ROOT_FIELDS = {kind: f"{kind}_root" for kind in SCHEMAS}


def root(kind: str, value: dict[str, object], domain: bytes | None = None) -> str:
    rooted = dict(value)
    rooted[ROOT_FIELDS[kind]] = ""
    framing = SCHEMAS[kind].encode() + b"\0" if domain is None else domain
    return "sha256:" + hashlib.sha256(framing + canonical_bytes(rooted)).hexdigest()


def finish(kind: str, value: dict[str, object]) -> dict[str, object]:
    value[ROOT_FIELDS[kind]] = root(kind, value)
    return value


def packet() -> dict[str, object]:
    commit = "1" * 40
    profile = finish(
        "profile",
        {
            "schema": SCHEMAS["profile"],
            "profile_root": "",
            "profile_id": "https://example.invalid/profiles/proof/v0.1",
            "version": "0.1",
            "conformance": ["exact identity", "scoped native check"],
            "rights": {"license": "CC0-1.0", "redistribution": "permitted"},
            "limitations": ["synthetic fixture"],
            "nonclaims": ["Conformance is not scientific acceptance."],
            "authority_effect": "none",
        },
    )
    method = finish(
        "method",
        {
            "schema": SCHEMAS["method"],
            "method_root": "",
            "method_id": "native-check",
            "version": "0.1",
            "implementation": {
                "path": "tools/check.py",
                "digest": "sha256:" + "2" * 64,
            },
            "environment": {"kind": "exact", "revision": commit},
            "inputs": ["native source"],
            "outputs": ["scoped check result"],
            "limitations": ["Checks only the selected declaration."],
            "nonclaims": ["A pass is not acceptance or Standing."],
            "authority_effect": "none",
        },
    )
    reference = {
        "schema": "vela.exact-reference.v0.1",
        "native_identity": {
            "system": "git",
            "object_kind": "declaration",
            "identifier": "Example.theorem",
        },
        "revision": {"kind": "git_commit", "value": commit},
        "content_fixity": {
            "media_type": "text/plain",
            "digest": "sha256:" + hashlib.sha256(b"theorem source\n").hexdigest(),
            "size": 15,
        },
        "selector": {"kind": "declaration", "value": "Example.theorem"},
        "locator": {
            "uri": "https://example.invalid/repo",
            "mutable": True,
            "authentication": "public",
        },
    }
    binding = finish(
        "binding",
        {
            "schema": SCHEMAS["binding"],
            "binding_root": "",
            "binding_id": "example-proof",
            "profile": {
                "id": profile["profile_id"],
                "version": "0.1",
                "root": profile["profile_root"],
            },
            "references": [reference],
            "mappings": [
                {
                    "source": "Example.theorem",
                    "target": "problem:1",
                    "relation": "exact",
                }
            ],
            "translations": [
                {
                    "source": "native statement",
                    "target": "external statement",
                    "disposition": "preserved",
                }
            ],
            "methods": [{"id": "native-check", "root": method["method_root"]}],
            "outputs": ["exact_reference", "verification_input"],
            "authority_effect": "none",
        },
    )
    manifest = finish(
        "manifest",
        {
            "schema": SCHEMAS["manifest"],
            "manifest_root": "",
            "repository": {
                "identity": "https://example.invalid/repo",
                "revision_policy": "exact_git_commit",
                "revision": commit,
            },
            "profiles": [
                {
                    "id": profile["profile_id"],
                    "version": "0.1",
                    "path": ".vela/profiles/proof.json",
                    "root": profile["profile_root"],
                }
            ],
            "bindings": [
                {"path": ".vela/bindings/proof.json", "root": binding["binding_root"]}
            ],
            "methods": [
                {
                    "id": "native-check",
                    "path": ".vela/methods/native-check.json",
                    "root": method["method_root"],
                }
            ],
            "rights": {
                "license": "Apache-2.0",
                "dependencies": "disclosed",
                "redistribution": "permitted",
            },
            "availability": {
                "class": "public",
                "observed_at": "2026-08-13T00:00:00Z",
                "retention": "source Git history",
                "access": "anonymous clone",
            },
            "outputs": ["exact_reference", "submission_draft", "verification_input"],
            "authority_effect": "none",
        },
    )
    # This is a synthetic value used only to exercise output refusal behavior.
    # INT-00 defines no shared check-result document or root domain.
    check_output = {
        "fixture_kind": "synthetic_check_output",
        "subject": reference,
        "method_root": method["method_root"],
        "evidence_availability": "available",
        "outcome": "pass",
        "scope": "selected declaration syntax",
        "nonclaims": ["This result is not acceptance or Standing."],
        "provenance": {
            "agent": "service:fixture",
            "activity": "native check",
            "entities": ["tool:fixture"],
            "role": "verifier",
        },
        "authority_effect": "none",
    }
    hostile = [
        ["wrong_root", "manifest", "manifest_root", "sha256:" + "0" * 64],
        ["short_root", "binding", "binding_root", "sha256:1234"],
        [
            "mutable_identity_as_immutable",
            "binding",
            "references.0.revision.kind",
            "git_branch",
        ],
        ["revision_drift", "binding", "references.0.revision.value", "3" * 40],
        ["selector_drift", "binding", "references.0.selector.value", "Example.other"],
        ["path_escape", "manifest", "bindings.0.path", "../proof.json"],
        ["missing_method", "binding", "methods.0.root", "sha256:" + "4" * 64],
        ["unsupported_schema", "method", "schema", "vela.integration-method.v9"],
        ["unsupported_method_version", "method", "version", "9"],
        [
            "short_method_implementation_digest",
            "method",
            "implementation.digest",
            "sha256:1234",
        ],
        ["unsupported_profile_version", "binding", "profile.version", "9"],
        ["binding_method_identity_drift", "binding", "methods.0.id", "other-check"],
        ["manifest_method_identity_drift", "manifest", "methods.0.id", "other-check"],
        [
            "manifest_profile_root_drift",
            "manifest",
            "profiles.0.root",
            "sha256:" + "5" * 64,
        ],
        [
            "wrong_root_domain",
            "profile",
            "profile_root",
            root("profile", profile, b"wrong-domain\0"),
        ],
        ["rights_omission", "manifest", "rights", {"$delete": True}],
        ["availability_omission", "manifest", "availability", {"$delete": True}],
        [
            "mapping_translation_collapse",
            "binding",
            "mappings.0.translation",
            "preserved",
        ],
        ["authority_field", "check_output", "decision", "accept"],
        [
            "output_authority_effect",
            "check_output",
            "authority_effect",
            "standing",
        ],
        ["nested_authority_field", "binding", "references.0.decision", "accept"],
        ["build_as_acceptance", "check_output", "outcome", "accepted"],
        ["review_as_standing", "binding", "outputs.0", "standing"],
        ["unavailable_as_pass", "check_output", "evidence_availability", "unavailable"],
        [
            "unavailable_as_fail",
            "check_output",
            "evidence_availability",
            "unavailable",
            ["outcome", "fail"],
        ],
        [
            "unavailable_as_zero",
            "check_output",
            "evidence_availability",
            "unavailable",
            ["outcome", 0],
        ],
    ]
    return {
        "schema": "vela.integration-conformance-fixtures.v0.1",
        "packet": {
            "manifest": manifest,
            "profile": profile,
            "binding": binding,
            "method": method,
            "check_output": check_output,
        },
        "hostile": hostile,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = json.dumps(packet(), indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    path = HERE / "fixtures.json"
    if args.check:
        if not path.exists() or path.read_text(encoding="utf-8") != encoded:
            print("integration-v0.1 fixtures drift; regenerate", file=sys.stderr)
            return 1
        print("integration-v0.1 fixture regeneration: ok")
        return 0
    path.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
