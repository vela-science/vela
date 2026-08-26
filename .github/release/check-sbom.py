#!/usr/bin/env python3
"""Normalize Syft SPDX to the selected release graph and enforce notice equality."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
from urllib.parse import quote

REQUIRED = {
    "vela-authority",
    "vela-cli",
    "vela-protocol",
    "vela-repository",
}
SELECTED_FORMAT = "vela.release-selected-packages.v1"
NOTICE_FORMAT = "vela.third-party-notice-inventory.v1"


def identity(package: dict[str, object]) -> tuple[str, str] | None:
    name = package.get("name")
    version = package.get("versionInfo")
    if isinstance(name, str) and isinstance(version, str):
        return name, version
    return None


def load_inputs(
    selected_path: pathlib.Path, notice_path: pathlib.Path
) -> tuple[dict[str, object], dict[tuple[str, str], dict[str, object]]]:
    selected = json.loads(selected_path.read_text(encoding="utf-8"))
    notice = json.loads(notice_path.read_text(encoding="utf-8"))
    if selected.get("format") != SELECTED_FORMAT:
        raise SystemExit(f"{selected_path}: unexpected selected-graph format")
    if notice.get("format") != NOTICE_FORMAT:
        raise SystemExit(f"{notice_path}: unexpected notice-inventory format")
    if selected.get("target") != notice.get("target"):
        raise SystemExit("SBOM correspondence: selected and notice targets differ")

    selected_rows = selected.get("packages")
    notice_rows = notice.get("packages")
    if not isinstance(selected_rows, list) or not isinstance(notice_rows, list):
        raise SystemExit("SBOM correspondence: package inventory is not a list")
    selected_by_id: dict[tuple[str, str], dict[str, object]] = {}
    for row in selected_rows:
        if not isinstance(row, dict):
            raise SystemExit("SBOM correspondence: malformed selected package")
        key = (row.get("name"), row.get("version"))
        if not all(isinstance(value, str) for value in key) or key in selected_by_id:
            raise SystemExit("SBOM correspondence: invalid or duplicate selected identity")
        if row.get("classification") not in {"contained", "build-contributor"}:
            raise SystemExit(f"SBOM correspondence: {key} has no exact classification")
        if not isinstance(row.get("workspace"), bool):
            raise SystemExit(f"SBOM correspondence: {key} has no workspace classification")
        selected_by_id[key] = row

    notice_by_id: dict[tuple[str, str], dict[str, object]] = {}
    for row in notice_rows:
        if not isinstance(row, dict):
            raise SystemExit("SBOM correspondence: malformed notice package")
        key = (row.get("name"), row.get("version"))
        if not all(isinstance(value, str) for value in key) or key in notice_by_id:
            raise SystemExit("SBOM correspondence: invalid or duplicate notice identity")
        notice_by_id[key] = row

    selected_third_party = {
        key for key, row in selected_by_id.items() if not row["workspace"]
    }
    if set(notice_by_id) != selected_third_party:
        missing = sorted(selected_third_party - set(notice_by_id))
        extra = sorted(set(notice_by_id) - selected_third_party)
        raise SystemExit(
            "SBOM correspondence: notices differ from selected third-party graph; "
            f"missing={missing}, extra={extra}"
        )
    workspace = {
        key[0] for key, row in selected_by_id.items() if row["workspace"]
    }
    if workspace != REQUIRED:
        raise SystemExit(
            "SBOM correspondence: unexpected workspace packages: "
            + ", ".join(sorted(workspace))
        )
    return selected, notice_by_id


def synthetic_package(
    row: dict[str, object], notice: dict[str, object]
) -> dict[str, object]:
    name = str(row["name"])
    version = str(row["version"])
    source = str(row["source"])
    suffix = hashlib.sha256(f"{source}\0{name}\0{version}".encode()).hexdigest()[:16]
    safe_name = "".join(character if character.isalnum() else "-" for character in name)
    return {
        "SPDXID": f"SPDXRef-Package-rust-build-crate-{safe_name}-{suffix}",
        "comment": (
            "Selected locked normal-dependency build contributor. It contributes "
            "during compilation and is not claimed as a contained runtime package; "
            "its distributable license material is in THIRD-PARTY-LICENSES.txt."
        ),
        "copyrightText": "NOASSERTION",
        "downloadLocation": "NOASSERTION",
        "externalRefs": [
            {
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceLocator": f"pkg:cargo/{quote(name)}@{quote(version)}",
                "referenceType": "purl",
            }
        ],
        "filesAnalyzed": False,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": str(notice["declared"]),
        "name": name,
        "supplier": "NOASSERTION",
        "versionInfo": version,
    }


def canonicalize(arguments: argparse.Namespace) -> int:
    source = pathlib.Path(arguments.input)
    document = json.loads(source.read_text(encoding="utf-8"))
    if document.get("spdxVersion") != "SPDX-2.3":
        raise SystemExit(f"{source}: expected SPDX-2.3")
    packages = document.get("packages")
    if not isinstance(packages, list):
        raise SystemExit(f"{source}: SPDX document has no package inventory")
    roots = [
        package
        for package in packages
        if isinstance(package, dict)
        and isinstance(package.get("SPDXID"), str)
        and package["SPDXID"].startswith("SPDXRef-DocumentRoot-")
    ]
    if len(roots) != 1:
        raise SystemExit(f"{source}: expected exactly one Syft document-root package")

    selected, notices = load_inputs(
        pathlib.Path(arguments.selected_graph), pathlib.Path(arguments.notice_inventory)
    )
    selected_rows = {
        (str(row["name"]), str(row["version"])): row
        for row in selected["packages"]
    }
    raw_by_id: dict[tuple[str, str], dict[str, object]] = {}
    for package in packages:
        if not isinstance(package, dict) or package is roots[0]:
            continue
        key = identity(package)
        if key is None:
            continue
        if key in raw_by_id:
            raise SystemExit(f"{source}: duplicate Syft package identity {key}")
        raw_by_id[key] = package

    root = roots[0]
    old_root_id = str(root["SPDXID"])
    retained: list[dict[str, object]] = []
    for key, row in selected_rows.items():
        package = raw_by_id.get(key)
        if package is None:
            if row["classification"] != "build-contributor" or row["workspace"]:
                raise SystemExit(f"{source}: Syft omitted contained selected package {key}")
            package = synthetic_package(row, notices[key])
        retained.append(package)
    root["name"] = arguments.root_name
    root["SPDXID"] = arguments.root_id
    retained.append(root)
    retained.sort(key=lambda package: str(package["SPDXID"]))
    document["packages"] = retained

    kept_ids = {str(package["SPDXID"]) for package in retained}
    relationships = document.get("relationships")
    if not isinstance(relationships, list):
        raise SystemExit(f"{source}: SPDX document has no relationships")
    normalized_relationships = []
    build_ids = {
        str(package["SPDXID"])
        for package in retained
        if identity(package) in selected_rows
        and selected_rows[identity(package)]["classification"] == "build-contributor"
    }
    for relationship in relationships:
        if not isinstance(relationship, dict):
            raise SystemExit(f"{source}: SPDX relationship is not an object")
        normalized = dict(relationship)
        for field in ("spdxElementId", "relatedSpdxElement"):
            if normalized.get(field) == old_root_id:
                normalized[field] = arguments.root_id
        left = normalized.get("spdxElementId")
        right = normalized.get("relatedSpdxElement")
        valid_ids = kept_ids | {"SPDXRef-DOCUMENT"}
        if left not in valid_ids or right not in valid_ids:
            continue
        if (
            normalized.get("relationshipType") == "CONTAINS"
            and left == arguments.root_id
            and right in build_ids
        ):
            continue
        normalized_relationships.append(normalized)
    for package_id in sorted(build_ids):
        normalized_relationships.append(
            {
                "relatedSpdxElement": arguments.root_id,
                "relationshipType": "BUILD_DEPENDENCY_OF",
                "spdxElementId": package_id,
            }
        )
    normalized_relationships.sort(
        key=lambda row: (
            str(row.get("spdxElementId")),
            str(row.get("relationshipType")),
            str(row.get("relatedSpdxElement")),
        )
    )
    document["relationships"] = normalized_relationships

    creation = document.get("creationInfo")
    if not isinstance(creation, dict):
        raise SystemExit(f"{source}: SPDX document has no creation information")
    document["name"] = arguments.name
    document["documentNamespace"] = arguments.namespace
    creation["created"] = arguments.created

    rendered = json.dumps(
        document, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    )
    if old_root_id != arguments.root_id and old_root_id in rendered:
        raise SystemExit(
            f"{source}: stale Syft document-root identity remains after normalization"
        )
    output = pathlib.Path(arguments.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered + "\n", encoding="utf-8")
    return check(output, pathlib.Path(arguments.selected_graph), pathlib.Path(arguments.notice_inventory))


def check(
    path: pathlib.Path,
    selected_path: pathlib.Path | None = None,
    notice_path: pathlib.Path | None = None,
) -> int:
    document = json.loads(path.read_text(encoding="utf-8"))
    packages = document.get("packages")
    relationships = document.get("relationships")
    if not isinstance(packages, list) or not isinstance(relationships, list):
        raise SystemExit(f"{path}: SPDX document has no package graph")
    names = {
        package.get("name")
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }
    missing = sorted(REQUIRED - names)
    if missing:
        raise SystemExit(f"{path}: missing Vela packages: {', '.join(missing)}")
    if selected_path is None or notice_path is None:
        if len(names) < 20:
            raise SystemExit(
                f"{path}: only {len(names)} package names; dependency recovery failed"
            )
        print(f"SBOM dependency graph: ok ({len(names)} package names)")
        return 0

    selected, _ = load_inputs(selected_path, notice_path)
    expected = {
        (str(row["name"]), str(row["version"])): row
        for row in selected["packages"]
    }
    roots = [
        package
        for package in packages
        if isinstance(package, dict)
        and str(package.get("SPDXID", "")).startswith("SPDXRef-DocumentRoot-")
    ]
    if len(roots) != 1:
        raise SystemExit(f"{path}: expected one normalized document root")
    observed = {
        key: package
        for package in packages
        if isinstance(package, dict) and (key := identity(package)) is not None
    }
    if set(observed) != set(expected):
        missing_ids = sorted(set(expected) - set(observed))
        extra_ids = sorted(set(observed) - set(expected))
        raise SystemExit(
            "SBOM correspondence: SPDX differs from selected release graph; "
            f"missing={missing_ids}, extra={extra_ids}"
        )
    package_ids = {str(package["SPDXID"]) for package in packages}
    root_id = str(roots[0]["SPDXID"])
    relation_rows = {
        (
            str(row.get("spdxElementId")),
            str(row.get("relationshipType")),
            str(row.get("relatedSpdxElement")),
        )
        for row in relationships
        if isinstance(row, dict)
    }
    for row in relationships:
        if not isinstance(row, dict):
            raise SystemExit(f"{path}: malformed SPDX relationship")
        for field in ("spdxElementId", "relatedSpdxElement"):
            if row.get(field) not in package_ids | {"SPDXRef-DOCUMENT"}:
                raise SystemExit(f"{path}: dangling SPDX relationship endpoint")
    for key, row in expected.items():
        package_id = str(observed[key]["SPDXID"])
        contained = (root_id, "CONTAINS", package_id) in relation_rows
        build_only = (package_id, "BUILD_DEPENDENCY_OF", root_id) in relation_rows
        if row["classification"] == "contained" and not contained:
            raise SystemExit(f"SBOM correspondence: contained package lacks CONTAINS {key}")
        if row["classification"] == "build-contributor" and (contained or not build_only):
            raise SystemExit(
                f"SBOM correspondence: build contributor has incorrect relationship {key}"
            )
    print(
        "SBOM/notice correspondence: ok "
        f"({len(expected)} exact selected package identities)"
    )
    return 0


def main() -> int:
    if len(sys.argv) >= 2 and sys.argv[1] != "--canonicalize":
        parser = argparse.ArgumentParser(prog="check-sbom")
        parser.add_argument("input", type=pathlib.Path)
        parser.add_argument("--selected-graph", type=pathlib.Path)
        parser.add_argument("--notice-inventory", type=pathlib.Path)
        arguments = parser.parse_args()
        if (arguments.selected_graph is None) != (arguments.notice_inventory is None):
            parser.error("--selected-graph and --notice-inventory must be used together")
        return check(arguments.input, arguments.selected_graph, arguments.notice_inventory)
    parser = argparse.ArgumentParser(prog="check-sbom")
    parser.add_argument("--canonicalize", action="store_true", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--namespace", required=True)
    parser.add_argument("--created", required=True)
    parser.add_argument("--root-name", required=True)
    parser.add_argument("--root-id", required=True)
    parser.add_argument("--selected-graph", required=True)
    parser.add_argument("--notice-inventory", required=True)
    return canonicalize(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
