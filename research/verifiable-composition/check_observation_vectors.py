#!/usr/bin/env python3
"""Run experiment-only parser and Draft 2020-12 observation-schema vectors.

The local ``jsonschema`` package is required only by this focused research check;
it is not a dependency of Vela's hosted or protocol surfaces.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as error:  # pragma: no cover - exercised only in an incomplete env.
    raise SystemExit(
        "check_observation_vectors.py requires the 'jsonschema' package to "
        "validate dependency-observation.v0.schema.json"
    ) from error

from reference.dependency_observation import ObservationError, parse_observation


ROOT = Path(__file__).resolve().parent
FULL_IDENTITY_FIELDS = {
    "authority_id",
    "decision_event_id",
    "decision_signature",
    "finding_id",
    "parent_frontier_id",
    "parent_git_commit",
}
FULL_ROOT_FIELDS = {
    "decision_event_content_root",
    "finding_revision_root",
    "parent_event_log_root",
    "parent_git_tree",
    "parent_snapshot_root",
    "premise_digest",
    "receipt_roots",
    "verifier_attachments",
}
ATTACHMENT_IDENTITY_AND_ROOT_FIELDS = {
    "attachment_id",
    "attachment_content_root",
}
# These constraints operate on raw bytes or require uniqueness by one nested
# identity. Draft 2020-12 cannot express them without changing the wire shape;
# the strict parser remains their normative experiment check.
PARSER_ONLY_CASES = {
    "duplicate-attachment-id-with-different-root",
    "duplicate-object-name",
    "oversized-document",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def mutate(base: dict[str, object], mutation: dict[str, object]) -> bytes:
    value = copy.deepcopy(base)
    kind = mutation["kind"]
    if kind == "none":
        pass
    elif kind == "remove":
        value.pop(str(mutation["path"]))
    elif kind == "remove_attachment_field":
        attachments = value["verifier_attachments"]
        require(
            isinstance(attachments, list) and bool(attachments),
            "remove_attachment_field target must be a non-empty list",
        )
        attachment = attachments[0]
        require(
            isinstance(attachment, dict),
            "remove_attachment_field target must contain an object",
        )
        attachment.pop(str(mutation["path"]))
    elif kind in {"replace", "add"}:
        value[str(mutation["path"])] = mutation["value"]
    elif kind == "append_copy":
        items = value[str(mutation["path"])]
        require(isinstance(items, list), "append_copy target must be a list")
        items.append(copy.deepcopy(items[0]))
    elif kind == "repeat_list":
        items = value[str(mutation["path"])]
        require(isinstance(items, list), "repeat_list target must be a list")
        value[str(mutation["path"])] = [
            copy.deepcopy(items[0]) for _ in range(int(mutation["count"]))
        ]
    elif kind == "append_attachment_conflict":
        items = value[str(mutation["path"])]
        require(
            isinstance(items, list),
            "append_attachment_conflict target must be a list",
        )
        conflicting = copy.deepcopy(items[0])
        conflicting["attachment_content_root"] = mutation["root"]
        items.append(conflicting)
    elif kind == "repeat":
        value[str(mutation["path"])] = str(mutation["value"]) * int(mutation["count"])
    else:
        raise AssertionError(f"unknown vector mutation {kind!r}")
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def raw_case(case: dict[str, object], base: dict[str, object]) -> bytes:
    if "raw_json" in case:
        return str(case["raw_json"]).encode()
    if "raw_json_repeat" in case:
        repeat = case["raw_json_repeat"]
        require(isinstance(repeat, dict), "raw_json_repeat must be an object")
        return (
            str(repeat["prefix"])
            + str(repeat["unit"]) * int(repeat["count"])
            + str(repeat["suffix"])
        ).encode()
    mutation = case["mutation"]
    require(isinstance(mutation, dict), "mutation must be an object")
    return mutate(base, mutation)


def main() -> None:
    vectors = json.loads((ROOT / "vectors/observation-cases.json").read_text())
    schema = json.loads((ROOT / "dependency-observation.v0.schema.json").read_text())
    require(
        schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema",
        "dependency observation schema must remain Draft 2020-12",
    )
    Draft202012Validator.check_schema(schema)
    schema_validator = Draft202012Validator(schema)
    require(
        vectors.get("base_class") == "shape_only_placeholder",
        "vector base must remain explicitly shape-only",
    )
    base = vectors["base"]
    cases = vectors["cases"]
    removed_top_level = {
        case["mutation"]["path"]
        for case in cases
        if case.get("mutation", {}).get("kind") == "remove"
    }
    removed_attachment_fields = {
        case["mutation"]["path"]
        for case in cases
        if case.get("mutation", {}).get("kind") == "remove_attachment_field"
    }
    require(
        FULL_IDENTITY_FIELDS | FULL_ROOT_FIELDS <= removed_top_level,
        "missing-field vectors do not cover every full root and identity",
    )
    require(
        ATTACHMENT_IDENTITY_AND_ROOT_FIELDS <= removed_attachment_fields,
        "missing-field vectors do not cover attachment identity and content root",
    )
    require(
        PARSER_ONLY_CASES <= {str(case.get("id")) for case in cases},
        "parser-only schema exceptions drifted",
    )
    passed = 0
    schema_passed = 0
    for case in cases:
        raw = raw_case(case, base)
        parser_expected_valid = case.get("expected") == "valid"
        if parser_expected_valid:
            parse_observation(raw)
        else:
            try:
                parse_observation(raw)
            except ObservationError as error:
                require(
                    str(error) == case["expected_error"],
                    f"{case['id']}: got {error!s}, expected {case['expected_error']}",
                )
            else:
                raise AssertionError(f"{case['id']} unexpectedly passed")
        passed += 1

        if case["id"] not in PARSER_ONLY_CASES:
            instance = json.loads(raw)
            schema_valid = schema_validator.is_valid(instance)
            require(
                schema_valid == parser_expected_valid,
                f"{case['id']}: parser/schema validity drift",
            )
            schema_passed += 1
    print(
        f"dependency-observation shape vectors: {passed}/{passed} parser pass; "
        f"{schema_passed}/{schema_passed} Draft 2020-12 parity pass"
    )


if __name__ == "__main__":
    main()
