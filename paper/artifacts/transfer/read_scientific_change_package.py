#!/usr/bin/env python3
"""Dependency-free reader for one loss-explicit scientific change package.

This reader deliberately does not import the exporter, JSON-LD tooling, or a
Vela authority implementation. It checks the frozen native object set and the
small RO-Crate 1.2 packaging view. RO-Crate remains metadata; the native
manifest remains the only source of Vela semantics in this package.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path, PurePosixPath


PLAN_ROOT = "sha256:72d84fd4ceeb69c170beaf2e63dc22a801db6e99b749123c87a2f42ebbf07e42"
AMENDMENT_ROOT = (
    "sha256:dba2fc12e533c811621437b4f449ae2b243c208f6cc092bac198ebc207552c9f"
)
PRIOR_AMENDMENT_ROOT = (
    "sha256:38d3cd699bcc4540a01852460ae218d91eb476ad40e7e2cac8886c02ff248ad8"
)
RO_CRATE_CONTEXT = "https://w3id.org/ro/crate/1.2/context"
RO_CRATE_PROFILE = "https://w3id.org/ro/crate/1.2"
NATIVE_MANIFEST = "reference.v1.json"
RO_CRATE_METADATA = "ro-crate-metadata.json"
LOSS_REPORT = "vela-loss-report.v1.json"
REQUIRED_LOSSES = {
    "canonical_object_root_vs_file_byte_root",
    "correction_and_supersession_semantics",
    "deterministic_standing_replay",
    "proposal_verification_decision_distinction",
    "source_standing_vs_receiving_standing",
    "vela_authority_signature_and_keyset_validation",
}
REQUIRED_NATIVE_FIELDS = {
    "authority",
    "completeness",
    "does_not_establish",
    "object_set_root",
    "objects",
    "schema",
    "source",
}


class PackageError(ValueError):
    """Stable fail-closed diagnostic."""


def fail(code: str) -> None:
    raise PackageError(code)


def require(condition: bool, code: str) -> None:
    if not condition:
        fail(code)


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def canonical_root(value: object) -> str:
    return sha256_bytes(canonical_bytes(value))


def load_object(path: Path, code: str) -> tuple[bytes, dict[str, object]]:
    try:
        encoded = path.read_bytes()
        value = json.loads(encoded)
    except (OSError, json.JSONDecodeError):
        fail(code)
    require(isinstance(value, dict), code)
    return encoded, value


def safe_file(package: Path, relative: object, code: str) -> Path:
    require(isinstance(relative, str) and relative, code)
    path = PurePosixPath(relative)
    require(
        not path.is_absolute()
        and "\\" not in relative
        and all(part not in {"", ".", ".."} for part in path.parts),
        code,
    )
    root = package.resolve()
    candidate = package.joinpath(*path.parts)
    try:
        resolved = candidate.resolve(strict=True)
    except OSError:
        fail(code)
    require(resolved.is_file() and resolved.is_relative_to(root), code)
    return resolved


def refs(value: object, code: str) -> list[str]:
    entries = value if isinstance(value, list) else [value]
    output: list[str] = []
    for entry in entries:
        require(
            isinstance(entry, dict)
            and set(entry) == {"@id"}
            and isinstance(entry["@id"], str),
            code,
        )
        output.append(entry["@id"])
    return output


def types(value: object, code: str) -> set[str]:
    entries = value if isinstance(value, list) else [value]
    require(
        all(isinstance(entry, str) and entry for entry in entries),
        code,
    )
    return set(entries)


def read_plan(plan_path: Path, amendment_path: Path) -> tuple[dict[str, object], dict[str, object]]:
    _, plan = load_object(plan_path, "plan_unavailable")
    _, amendment = load_object(amendment_path, "plan_amendment_unavailable")
    require(canonical_root(plan) == PLAN_ROOT, "plan_root_mismatch")
    require(
        canonical_root(amendment) == AMENDMENT_ROOT,
        "plan_amendment_root_mismatch",
    )
    require(
        amendment.get("prior_plan_root") == PLAN_ROOT,
        "plan_amendment_lineage_mismatch",
    )
    require(
        amendment.get("prior_amendment_root") == PRIOR_AMENDMENT_ROOT,
        "plan_amendment_lineage_mismatch",
    )
    return plan, amendment


def read_native(
    package: Path, plan: dict[str, object]
) -> tuple[dict[str, object], dict[str, dict[str, object]]]:
    encoded, native = load_object(
        package / NATIVE_MANIFEST, "native_manifest_unavailable"
    )
    source = plan.get("source")
    require(isinstance(source, dict), "plan_source_invalid")
    require(set(native) == REQUIRED_NATIVE_FIELDS, "native_manifest_fields_invalid")
    require(native.get("schema") == "vela.foreign-reference.v1", "native_schema_invalid")
    native_source = native.get("source")
    authority = native.get("authority")
    planned_claim = source.get("claim")
    planned_decision = source.get("decision_event")
    require(
        isinstance(native_source, dict)
        and native_source.get("frontier_id") == source.get("frontier_id")
        and isinstance(planned_claim, dict)
        and native_source.get("claim") == planned_claim
        and isinstance(planned_decision, dict)
        and native_source.get("decision_event")
        == {
            "id": planned_decision.get("id"),
            "root": planned_decision.get("root"),
        },
        "native_source_binding_mismatch",
    )
    require(
        native_source.get("standing") == "accepted"
        and isinstance(authority, dict)
        and authority.get("source_standing") == "accepted"
        and authority.get("local_standing_effect") == "none"
        and authority.get("requires_local_decision") is True,
        "authority_escalation",
    )
    require(
        sha256_bytes(encoded) == source.get("native_manifest_bytes_sha256")
        and canonical_root(native) == source.get("native_manifest_root"),
        "native_manifest_root_mismatch",
    )
    objects = native.get("objects")
    require(
        isinstance(objects, list)
        and len(objects) == source.get("object_count"),
        "native_object_count_mismatch",
    )
    require(
        canonical_root(objects) == source.get("object_set_root")
        and native.get("object_set_root") == source.get("object_set_root"),
        "native_object_set_root_mismatch",
    )
    by_path: dict[str, dict[str, object]] = {}
    roles: set[str] = set()
    for item in objects:
        require(
            isinstance(item, dict)
            and set(item) == {"bytes_root", "id", "path", "role", "root"},
            "native_object_entry_invalid",
        )
        relative = item.get("path")
        role = item.get("role")
        require(
            isinstance(relative, str)
            and isinstance(role, str)
            and role
            and relative not in by_path
            and role not in roles,
            "native_object_identity_duplicate",
        )
        path = safe_file(package, relative, "native_object_unavailable")
        require(
            sha256_bytes(path.read_bytes()) == item.get("bytes_root"),
            "native_object_root_drift",
        )
        by_path[relative] = item
        roles.add(role)
    require("decision_event" in roles, "decision_missing")
    return native, by_path


def read_loss_report(
    package: Path,
    plan: dict[str, object],
    native: dict[str, object],
) -> tuple[bytes, dict[str, object]]:
    encoded, loss = load_object(package / LOSS_REPORT, "loss_report_missing")
    require(
        loss.get("schema") == "vela.ro-crate-loss-report.v1",
        "loss_report_schema_invalid",
    )
    require(
        loss.get("plan_root") == PLAN_ROOT
        and loss.get("plan_amendment_root") == AMENDMENT_ROOT
        and loss.get("native_manifest_root") == canonical_root(native)
        and loss.get("object_set_root") == native.get("object_set_root"),
        "loss_report_binding_mismatch",
    )
    authority = loss.get("authority_boundary")
    require(
        isinstance(authority, dict)
        and authority.get("source_standing") == "accepted"
        and authority.get("local_standing_effect") == "none"
        and authority.get("requires_local_decision") is True,
        "loss_report_authority_escalation",
    )
    coverage = loss.get("coverage")
    require(isinstance(coverage, dict), "loss_report_coverage_invalid")
    accounted = coverage.get("native_top_level_fields")
    require(
        isinstance(accounted, dict)
        and set(accounted) == REQUIRED_NATIVE_FIELDS
        and all(
            value in {"mapped", "retained_native"}
            for value in accounted.values()
        )
        and coverage.get("unreported_fields") == [],
        "loss_report_coverage_incomplete",
    )
    losses = loss.get("semantic_losses")
    require(isinstance(losses, list), "loss_report_semantics_invalid")
    observed_ids = {
        item.get("id")
        for item in losses
        if isinstance(item, dict)
        and item.get("mapping") == "not_expressible_in_base_ro_crate"
        and item.get("preserved_in") == "reference.v1.json and retained native bytes"
        and isinstance(item.get("reason"), str)
        and bool(item["reason"])
    }
    require(observed_ids == REQUIRED_LOSSES, "loss_report_semantics_incomplete")
    require(
        loss.get("unknown_required_semantics") == "fail_closed",
        "loss_report_unknown_semantics_policy",
    )
    return encoded, loss


def read_ro_crate(
    package: Path,
    plan: dict[str, object],
    native: dict[str, object],
    objects: dict[str, dict[str, object]],
    loss_bytes: bytes,
    loss: dict[str, object],
) -> tuple[bytes, dict[str, object]]:
    encoded, crate = load_object(
        package / RO_CRATE_METADATA, "ro_crate_metadata_missing"
    )
    require(crate.get("@context") == RO_CRATE_CONTEXT, "ro_crate_context_invalid")
    graph = crate.get("@graph")
    require(isinstance(graph, list), "ro_crate_graph_invalid")
    require(
        all(isinstance(entity, dict) for entity in graph),
        "ro_crate_entity_invalid",
    )
    ids = [entity.get("@id") for entity in graph]
    require(
        all(isinstance(value, str) and value for value in ids)
        and ids == sorted(ids)
        and len(ids) == len(set(ids)),
        "ro_crate_entity_identity_invalid",
    )
    entities = {entity["@id"]: entity for entity in graph}
    expected_ids = {
        "./",
        "ro-crate-metadata.json",
        NATIVE_MANIFEST,
        LOSS_REPORT,
        "https://spdx.org/licenses/MIT.html",
        *objects,
    }
    require(set(entities) == expected_ids, "ro_crate_entity_set_mismatch")

    descriptor = entities["ro-crate-metadata.json"]
    require(
        types(descriptor.get("@type"), "ro_crate_descriptor_type") == {"CreativeWork"}
        and refs(descriptor.get("about"), "ro_crate_descriptor_about") == ["./"]
        and refs(descriptor.get("conformsTo"), "ro_crate_descriptor_profile")
        == [RO_CRATE_PROFILE],
        "ro_crate_descriptor_invalid",
    )

    root = entities["./"]
    root_types = types(root.get("@type"), "ro_crate_root_type")
    require("Dataset" in root_types, "ro_crate_root_type")
    require(
        isinstance(root.get("name"), str)
        and bool(root["name"])
        and isinstance(root.get("description"), str)
        and "local Standing effect is none" in root["description"]
        and root.get("datePublished")
        == str(plan["source"]["decision_event"]["recorded_at"])[:10]
        and refs(root.get("license"), "ro_crate_license")
        == ["https://spdx.org/licenses/MIT.html"]
        and refs(root.get("mainEntity"), "ro_crate_main_entity")
        == [NATIVE_MANIFEST],
        "ro_crate_root_metadata_invalid",
    )
    expected_parts = sorted([NATIVE_MANIFEST, LOSS_REPORT, *objects])
    require(
        refs(root.get("hasPart"), "ro_crate_has_part") == expected_parts,
        "ro_crate_payload_mismatch",
    )

    forbidden_types = {"Action", "CreateAction", "UpdateAction"}
    forbidden_keys = {"actionStatus", "agent", "result"}
    for entity in graph:
        require(
            not (types(entity.get("@type"), "ro_crate_entity_type") & forbidden_types),
            "ro_crate_authority_conflation",
        )
        require(
            not (set(entity) & forbidden_keys),
            "ro_crate_authority_conflation",
        )

    native_entity = entities[NATIVE_MANIFEST]
    require(
        {"CreativeWork", "File"}
        <= types(native_entity.get("@type"), "native_manifest_entity_type")
        and native_entity.get("contentSize")
        == str((package / NATIVE_MANIFEST).stat().st_size)
        and set(native_entity.get("identifier", []))
        == {
            "vela.foreign-reference.v1",
            canonical_root(native),
            str(native["object_set_root"]),
        },
        "native_manifest_entity_invalid",
    )
    loss_entity = entities[LOSS_REPORT]
    require(
        {"CreativeWork", "File"}
        <= types(loss_entity.get("@type"), "loss_report_entity_type")
        and loss_entity.get("contentSize") == str(len(loss_bytes))
        and set(loss_entity.get("identifier", []))
        == {"vela.ro-crate-loss-report.v1", sha256_bytes(loss_bytes)},
        "loss_report_entity_invalid",
    )
    for relative, item in objects.items():
        entity = entities[relative]
        require(
            "File" in types(entity.get("@type"), "native_object_entity_type")
            and entity.get("contentSize") == str((package / relative).stat().st_size)
            and entity.get("encodingFormat") == "application/json"
            and set(entity.get("identifier", []))
            == {str(item["id"]), str(item["root"]), str(item["bytes_root"])}
            and str(item["role"]) in str(entity.get("description", "")),
            "native_object_entity_mismatch",
        )
    require(
        loss.get("ro_crate_metadata_policy")
        == "Package metadata only; Vela semantics remain in the native manifest.",
        "loss_report_metadata_policy_invalid",
    )
    return encoded, crate


def assess(
    package: Path,
    plan_path: Path,
    amendment_path: Path,
) -> dict[str, object]:
    plan, _ = read_plan(plan_path, amendment_path)
    native, objects = read_native(package, plan)
    loss_bytes, loss = read_loss_report(package, plan, native)
    crate_bytes, crate = read_ro_crate(
        package, plan, native, objects, loss_bytes, loss
    )
    return {
        "schema": "vela.scientific-change-package-reader-result.v1",
        "ok": True,
        "plan_root": PLAN_ROOT,
        "plan_amendment_root": AMENDMENT_ROOT,
        "native_manifest_root": canonical_root(native),
        "object_set_root": native["object_set_root"],
        "object_count": len(objects),
        "ro_crate_profile": RO_CRATE_PROFILE,
        "ro_crate_metadata_sha256": sha256_bytes(crate_bytes),
        "ro_crate_entity_count": len(crate["@graph"]),
        "loss_report_sha256": sha256_bytes(loss_bytes),
        "semantic_loss_count": len(loss["semantic_losses"]),
        "source_standing": "accepted",
        "local_standing_effect": "none",
        "requires_local_decision": True,
        "checks": [
            "frozen_plan",
            "native_manifest_and_object_roots",
            "decision_presence",
            "authority_non_escalation",
            "ro_crate_1_2_required_structure",
            "exact_native_object_parity",
            "complete_loss_report",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-root", required=True, type=Path)
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--plan-amendment", required=True, type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        result = assess(
            args.package_root.expanduser().resolve(),
            args.plan.expanduser().resolve(),
            args.plan_amendment.expanduser().resolve(),
        )
        if args.json:
            print(canonical_bytes(result).decode("utf-8"))
        else:
            print(
                "scientific-change-package: ok "
                f"(objects={result['object_count']}, "
                f"native={result['native_manifest_root']}, "
                "source_standing=accepted, local_standing_effect=none)"
            )
        return 0
    except PackageError as error:
        print(f"scientific-change-package: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
