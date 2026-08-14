#!/usr/bin/env python3
"""Validate the draft integration packet and its hostile fixtures."""

from __future__ import annotations

import copy
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from generate_fixtures import ROOT_FIELDS, SCHEMAS, root

FULL_ROOT = re.compile(r"sha256:[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
MAPPINGS = {"exact", "close", "broader", "narrower", "related"}
TRANSLATIONS = {
    "preserved",
    "normalized",
    "derived",
    "approximated",
    "omitted",
    "unsupported",
    "assumed",
    "unresolved",
}
OUTPUTS = {"exact_reference", "submission_draft", "verification_input"}


def closed(value: dict[str, object], fields: set[str], label: str) -> None:
    unknown = set(value) - fields
    missing = fields - set(value)
    if unknown or missing:
        raise ValueError(
            f"{label} fields: missing={sorted(missing)} unknown={sorted(unknown)}"
        )


def closed_with_optional(
    value: dict[str, object], required: set[str], optional: set[str], label: str
) -> None:
    unknown = set(value) - required - optional
    missing = required - set(value)
    if unknown or missing:
        raise ValueError(
            f"{label} fields: missing={sorted(missing)} unknown={sorted(unknown)}"
        )


def local_path(value: object) -> bool:
    return (
        isinstance(value, str)
        and value
        and not value.startswith(("/", "~"))
        and ".." not in Path(value).parts
    )


def validate(packet: dict[str, object]) -> None:
    manifest, profile = packet["manifest"], packet["profile"]
    binding, method = packet["binding"], packet["method"]
    check_output = packet["check_output"]
    documents = {
        "manifest": manifest,
        "profile": profile,
        "binding": binding,
        "method": method,
    }
    for kind, document in documents.items():
        if document.get("schema") != SCHEMAS[kind]:
            raise ValueError(f"unsupported {kind} schema")
        field = ROOT_FIELDS[kind]
        if not FULL_ROOT.fullmatch(str(document.get(field, ""))) or document[
            field
        ] != root(kind, document):
            raise ValueError(f"{kind} root or domain mismatch")
        if document.get("authority_effect") != "none":
            raise ValueError(f"{kind} authority effect")

    closed(
        manifest,
        {
            "schema",
            "manifest_root",
            "repository",
            "profiles",
            "bindings",
            "methods",
            "rights",
            "availability",
            "outputs",
            "authority_effect",
        },
        "manifest",
    )
    closed(
        profile,
        {
            "schema",
            "profile_root",
            "profile_id",
            "version",
            "conformance",
            "rights",
            "limitations",
            "nonclaims",
            "authority_effect",
        },
        "profile",
    )
    closed(
        binding,
        {
            "schema",
            "binding_root",
            "binding_id",
            "profile",
            "references",
            "mappings",
            "translations",
            "methods",
            "outputs",
            "authority_effect",
        },
        "binding",
    )
    closed(
        method,
        {
            "schema",
            "method_root",
            "method_id",
            "version",
            "implementation",
            "environment",
            "inputs",
            "outputs",
            "limitations",
            "nonclaims",
            "authority_effect",
        },
        "method",
    )
    closed(
        check_output,
        {
            "fixture_kind",
            "subject",
            "method_root",
            "evidence_availability",
            "outcome",
            "scope",
            "nonclaims",
            "provenance",
            "authority_effect",
        },
        "synthetic check output",
    )
    if check_output["authority_effect"] != "none":
        raise ValueError("synthetic check output authority effect")

    repository = manifest["repository"]
    closed(repository, {"identity", "revision_policy", "revision"}, "repository")
    if repository.get("revision_policy") != "exact_git_commit" or not COMMIT.fullmatch(
        str(repository.get("revision", ""))
    ):
        raise ValueError("mutable repository revision")
    closed(
        manifest["rights"],
        {"license", "dependencies", "redistribution"},
        "manifest rights",
    )
    closed(
        manifest["availability"],
        {"class", "observed_at", "retention", "access"},
        "manifest availability",
    )
    closed(profile["rights"], {"license", "redistribution"}, "Profile rights")
    if (
        not profile.get("rights")
        or not manifest.get("rights")
        or not manifest.get("availability")
    ):
        raise ValueError("rights or availability omitted")
    if profile.get("version") != "0.1" or binding["profile"].get("version") != "0.1":
        raise ValueError("unsupported Profile version")
    if method.get("version") != "0.1":
        raise ValueError("unsupported Method version")
    closed(binding["profile"], {"id", "version", "root"}, "Binding Profile")
    if binding["profile"] != {
        "id": profile["profile_id"],
        "version": profile["version"],
        "root": profile["profile_root"],
    }:
        raise ValueError("Profile root drift")
    if (
        len(manifest["profiles"]) != 1
        or len(manifest["bindings"]) != 1
        or len(manifest["methods"]) != 1
    ):
        raise ValueError("manifest inventory cardinality")
    manifest_profile = manifest["profiles"][0]
    manifest_binding = manifest["bindings"][0]
    manifest_method = manifest["methods"][0]
    closed(manifest_profile, {"id", "version", "path", "root"}, "manifest Profile")
    closed(manifest_binding, {"path", "root"}, "manifest Binding")
    closed(manifest_method, {"id", "path", "root"}, "manifest Method")
    for item in (manifest_profile, manifest_binding, manifest_method):
        if not local_path(item["path"]):
            raise ValueError("path escape")
    if manifest_profile != {
        "id": profile["profile_id"],
        "version": profile["version"],
        "path": ".vela/profiles/proof.json",
        "root": profile["profile_root"],
    }:
        raise ValueError("manifest Profile inventory drift")
    if not set(manifest["outputs"]).issubset(OUTPUTS) or not set(
        binding["outputs"]
    ).issubset(OUTPUTS):
        raise ValueError("authority output")

    if len(binding["references"]) != 1:
        raise ValueError("Exact Reference cardinality")
    reference = binding["references"][0]
    closed_with_optional(
        reference,
        {
            "schema",
            "native_identity",
            "revision",
            "content_fixity",
            "locator",
        },
        {"selector"},
        "Exact Reference",
    )
    if reference.get("schema") != "vela.exact-reference.v0.1":
        raise ValueError("unsupported Exact Reference")
    closed(
        reference["native_identity"],
        {"system", "object_kind", "identifier"},
        "native identity",
    )
    closed(reference["revision"], {"kind", "value"}, "revision")
    closed(
        reference["content_fixity"], {"media_type", "digest", "size"}, "content fixity"
    )
    selector = reference.get("selector")
    if selector is not None:
        closed(selector, {"kind", "value"}, "selector")
    closed(reference["locator"], {"uri", "mutable", "authentication"}, "locator")
    if (
        reference["revision"].get("kind") != "git_commit"
        or reference["revision"].get("value") != repository["revision"]
    ):
        raise ValueError("mutable identity or revision drift")
    if selector is not None and selector.get("value") != reference[
        "native_identity"
    ].get("identifier"):
        raise ValueError("selector drift")
    fixity = reference["content_fixity"]
    if (
        not FULL_ROOT.fullmatch(str(fixity.get("digest", "")))
        or not isinstance(fixity.get("size"), int)
        or fixity["size"] < 0
    ):
        raise ValueError("content fixity")
    for mapping in binding["mappings"]:
        closed(mapping, {"source", "target", "relation"}, "mapping")
        if mapping["relation"] not in MAPPINGS:
            raise ValueError("mapping relation")
    for translation in binding["translations"]:
        closed(translation, {"source", "target", "disposition"}, "translation")
        if translation["disposition"] not in TRANSLATIONS:
            raise ValueError("translation disposition")
    if len(binding["methods"]) != 1:
        raise ValueError("Binding Method cardinality")
    binding_method = binding["methods"][0]
    closed(binding_method, {"id", "root"}, "Binding Method")
    expected_method = {"id": method["method_id"], "root": method["method_root"]}
    if (
        binding_method != expected_method
        or {"id": manifest_method["id"], "root": manifest_method["root"]}
        != expected_method
    ):
        raise ValueError("Method identity or root drift")
    closed(method["implementation"], {"path", "digest"}, "Method implementation")
    closed(method["environment"], {"kind", "revision"}, "Method environment")
    if not local_path(method["implementation"]["path"]):
        raise ValueError("Method implementation path escape")
    if not FULL_ROOT.fullmatch(str(method["implementation"]["digest"])):
        raise ValueError("Method implementation digest")

    closed(
        check_output["provenance"],
        {"agent", "activity", "entities", "role"},
        "synthetic output provenance",
    )
    output_subject = check_output["subject"]
    closed_with_optional(
        output_subject,
        {
            "schema",
            "native_identity",
            "revision",
            "content_fixity",
            "locator",
        },
        {"selector"},
        "output Exact Reference",
    )
    for field, allowed in (
        ("native_identity", {"system", "object_kind", "identifier"}),
        ("revision", {"kind", "value"}),
        ("content_fixity", {"media_type", "digest", "size"}),
        ("locator", {"uri", "mutable", "authentication"}),
    ):
        closed(output_subject[field], allowed, f"output Exact Reference {field}")
    if "selector" in output_subject:
        closed(
            output_subject["selector"],
            {"kind", "value"},
            "output Exact Reference selector",
        )
    if output_subject != reference:
        raise ValueError("check output subject drift")
    if check_output["fixture_kind"] != "synthetic_check_output":
        raise ValueError("unsupported synthetic output fixture")
    if check_output["outcome"] not in {
        "pass",
        "fail",
        "inconclusive",
        "error",
        "unavailable",
    }:
        raise ValueError("check result presented as acceptance")
    if (
        check_output["evidence_availability"] == "unavailable"
        and check_output["outcome"] != "unavailable"
    ):
        raise ValueError("unavailable evidence converted to result")
    if not check_output["nonclaims"]:
        raise ValueError("missing result nonclaims")
    if check_output["method_root"] != method["method_root"]:
        raise ValueError("missing Method")
    if manifest_binding["root"] != binding["binding_root"]:
        raise ValueError("manifest inventory drift")


def set_path(target: object, path: str, value: object) -> None:
    parts = path.split(".")
    cursor = target
    for part in parts[:-1]:
        cursor = cursor[int(part)] if isinstance(cursor, list) else cursor[part]
    key = parts[-1]
    if value == {"$delete": True}:
        del cursor[key]
    elif isinstance(cursor, list):
        cursor[int(key)] = value
    else:
        cursor[key] = value


def mutate(packet: dict[str, object], case: list[object]) -> dict[str, object]:
    changed = copy.deepcopy(packet)
    target = changed[case[1]]
    set_path(target, case[2], case[3])
    if len(case) == 5:
        set_path(target, case[4][0], case[4][1])
    if case[1] in ROOT_FIELDS and case[0] not in {
        "wrong_root",
        "short_root",
        "wrong_root_domain",
    }:
        target[ROOT_FIELDS[case[1]]] = root(case[1], target)
    return changed


def main() -> int:
    fixture = json.loads((HERE / "fixtures.json").read_text(encoding="utf-8"))
    packet = fixture["packet"]
    validate(packet)
    without_selector = copy.deepcopy(packet)
    del without_selector["binding"]["references"][0]["selector"]
    del without_selector["check_output"]["subject"]["selector"]
    without_selector["binding"]["binding_root"] = root(
        "binding", without_selector["binding"]
    )
    without_selector["manifest"]["bindings"][0]["root"] = without_selector["binding"][
        "binding_root"
    ]
    without_selector["manifest"]["manifest_root"] = root(
        "manifest", without_selector["manifest"]
    )
    validate(without_selector)
    passed = []
    for case in fixture["hostile"]:
        try:
            validate(mutate(packet, case))
        except (KeyError, TypeError, ValueError):
            continue
        passed.append(case[0])
    if passed:
        print(
            f"integration-v0.1 hostile fixtures passed unexpectedly: {passed}",
            file=sys.stderr,
        )
        return 1
    print(f"integration-v0.1: ok ({len(fixture['hostile'])} hostile fixtures refused)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
