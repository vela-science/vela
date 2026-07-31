#!/usr/bin/env python3
"""Build a deterministic native + RO-Crate scientific change package.

The exporter is intentionally a read-only artifact builder. It never opens a
Frontier, invokes a Vela writer, loads credentials, or resolves a remote JSON-LD
context. The existing `vela.foreign-reference.v1` file is representation A.
Representation B is an attached RO-Crate 1.3 metadata view over those same
exact files, accompanied by an explicit semantic loss report.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
PLAN = HERE / "scientific-change-package-plan.v1.json"
AMENDMENT = HERE / "scientific-change-package-plan-amendment-001.v1.json"
READER = HERE / "read_scientific_change_package.py"
NATIVE_VERIFIER = ROOT / "conformance" / "verify_foreign_reference.py"
PLAN_ROOT = "sha256:72d84fd4ceeb69c170beaf2e63dc22a801db6e99b749123c87a2f42ebbf07e42"
PLAN_BYTES_SHA256 = (
    "sha256:63efd8b095cce421cbbb5aab7f0e21f03deeb3322f7c7de33eb5a20cd0a32fd9"
)
AMENDMENT_ROOT = (
    "sha256:38d3cd699bcc4540a01852460ae218d91eb476ad40e7e2cac8886c02ff248ad8"
)
AMENDMENT_BYTES_SHA256 = (
    "sha256:787dd740eb597f437033bb89d4a8edb1e582bcc477e1ed8c6f45335182652d04"
)
GENERATED = [
    "ro-crate-metadata.json",
    "vela-loss-report.v1.json",
    "reader-result.v1.json",
    "result.v1.json",
    "SHA256SUMS",
]


class BuildError(ValueError):
    """Stable artifact-build failure."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BuildError(message)


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")


def encoded_json(value: object) -> bytes:
    return canonical_bytes(value) + b"\n"


def sha256_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def canonical_root(value: object) -> str:
    return sha256_bytes(canonical_bytes(value))


def load_frozen(path: Path, root: str, bytes_root: str) -> dict[str, object]:
    encoded = path.read_bytes()
    value = json.loads(encoded)
    require(isinstance(value, dict), f"{path.name}_not_object")
    require(sha256_bytes(encoded) == bytes_root, f"{path.name}_bytes_drift")
    require(canonical_root(value) == root, f"{path.name}_root_drift")
    return value


def load_source(package: Path, plan: dict[str, object]) -> dict[str, object]:
    source = plan["source"]
    require(isinstance(source, dict), "plan_source_invalid")
    encoded = (package / "reference.v1.json").read_bytes()
    native = json.loads(encoded)
    require(isinstance(native, dict), "native_manifest_invalid")
    require(
        sha256_bytes(encoded) == source["native_manifest_bytes_sha256"]
        and canonical_root(native) == source["native_manifest_root"],
        "native_manifest_root_mismatch",
    )
    objects = native.get("objects")
    require(
        isinstance(objects, list)
        and len(objects) == source["object_count"]
        and canonical_root(objects) == source["object_set_root"],
        "native_object_set_mismatch",
    )
    for item in objects:
        require(isinstance(item, dict), "native_object_invalid")
        relative = item.get("path")
        require(isinstance(relative, str), "native_object_path_invalid")
        path = package / relative
        require(path.is_file(), f"native_object_missing:{relative}")
        require(
            sha256_bytes(path.read_bytes()) == item.get("bytes_root"),
            f"native_object_root_drift:{relative}",
        )
    return native


def loss_report(
    native: dict[str, object],
) -> dict[str, object]:
    return {
        "schema": "vela.ro-crate-loss-report.v1",
        "plan_root": PLAN_ROOT,
        "plan_amendment_root": AMENDMENT_ROOT,
        "native_manifest": "reference.v1.json",
        "native_manifest_root": canonical_root(native),
        "object_set_root": native["object_set_root"],
        "ro_crate_profile": "https://w3id.org/ro/crate/1.3",
        "ro_crate_metadata_policy": (
            "Package metadata only; Vela semantics remain in the native manifest."
        ),
        "authority_boundary": {
            "source_standing": "accepted",
            "local_standing_effect": "none",
            "requires_local_decision": True,
        },
        "coverage": {
            "native_top_level_fields": {
                "authority": "retained_native",
                "completeness": "retained_native",
                "does_not_establish": "retained_native",
                "object_set_root": "mapped",
                "objects": "mapped",
                "schema": "mapped",
                "source": "retained_native",
            },
            "mapped_object_fields": [
                "bytes_root",
                "id",
                "path",
                "role",
                "root",
            ],
            "unreported_fields": [],
        },
        "semantic_losses": [
            {
                "id": "canonical_object_root_vs_file_byte_root",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "Base RO-Crate identifies files but does not distinguish a "
                    "Vela canonical object root from the SHA-256 of its stored bytes."
                ),
            },
            {
                "id": "correction_and_supersession_semantics",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "Generic dataset provenance does not carry Vela's exact "
                    "correction, supersession, and surviving-history semantics."
                ),
            },
            {
                "id": "deterministic_standing_replay",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "RO-Crate packages metadata and files; it does not replay a "
                    "Vela Event history into Standing."
                ),
            },
            {
                "id": "proposal_verification_decision_distinction",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "Base RO-Crate action and provenance terms do not preserve "
                    "Vela's Proposal, Verification, Decision, and Event planes."
                ),
            },
            {
                "id": "source_standing_vs_receiving_standing",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "RO-Crate conformance does not grant source authority or "
                    "transport accepted Standing into another Frontier."
                ),
            },
            {
                "id": "vela_authority_signature_and_keyset_validation",
                "mapping": "not_expressible_in_base_ro_crate",
                "preserved_in": "reference.v1.json and retained native bytes",
                "reason": (
                    "RO-Crate records file context but does not validate the "
                    "Vela repository-authority DSSE chain or its keyset."
                ),
            },
        ],
        "unknown_required_semantics": "fail_closed",
        "nonclaims": [
            "RO-Crate validation verifies scientific truth or Vela Standing.",
            "The source Decision changes Standing in a receiving Frontier.",
            "File identifiers replace native Vela IDs or roots.",
        ],
    }


def crate_metadata(
    package: Path,
    plan: dict[str, object],
    native: dict[str, object],
    loss_bytes: bytes,
) -> dict[str, object]:
    source = plan["source"]
    objects = native["objects"]
    entities: list[dict[str, object]] = [
        {
            "@id": "ro-crate-metadata.json",
            "@type": "CreativeWork",
            "about": {"@id": "./"},
            "conformsTo": {"@id": "https://w3id.org/ro/crate/1.3"},
            "description": "RO-Crate 1.3 metadata descriptor.",
        },
        {
            "@id": "./",
            "@type": "Dataset",
            "name": "Vela scientific change package: Erdős 424 source correction",
            "description": (
                "A portable metadata view over one accepted source-Frontier "
                "correction. Source Standing is accepted; local Standing effect is none."
            ),
            "datePublished": str(source["decision_event"]["recorded_at"])[:10],
            "license": {"@id": "https://spdx.org/licenses/MIT.html"},
            "mainEntity": {"@id": "reference.v1.json"},
            "identifier": sorted(
                {
                    str(source["frontier_id"]),
                    str(source["native_manifest_root"]),
                    str(source["object_set_root"]),
                    str(source["claim"]["id"]),
                    str(source["claim"]["root"]),
                    str(source["decision_event"]["id"]),
                    str(source["decision_event"]["root"]),
                }
            ),
            "hasPart": [
                {"@id": path}
                for path in sorted(
                    [
                        "reference.v1.json",
                        "vela-loss-report.v1.json",
                        *(str(item["path"]) for item in objects),
                    ]
                )
            ],
        },
        {
            "@id": "reference.v1.json",
            "@type": ["CreativeWork", "File"],
            "name": "Rooted native Vela manifest",
            "description": (
                "The exact native manifest for object identities, source "
                "Standing, source authority evidence, and local non-authority."
            ),
            "encodingFormat": "application/json",
            "contentSize": str((package / "reference.v1.json").stat().st_size),
            "identifier": sorted(
                {
                    "vela.foreign-reference.v1",
                    str(source["native_manifest_root"]),
                    str(source["object_set_root"]),
                }
            ),
        },
        {
            "@id": "vela-loss-report.v1.json",
            "@type": ["CreativeWork", "File"],
            "name": "Vela to RO-Crate semantic loss report",
            "description": (
                "Explicit account of Vela meanings that base RO-Crate 1.3 "
                "does not represent and that remain in the native manifest."
            ),
            "encodingFormat": "application/json",
            "contentSize": str(len(loss_bytes)),
            "identifier": sorted(
                {
                    "vela.ro-crate-loss-report.v1",
                    sha256_bytes(loss_bytes),
                }
            ),
        },
        {
            "@id": "https://spdx.org/licenses/MIT.html",
            "@type": "CreativeWork",
            "name": "MIT License",
            "description": (
                "The retained source Frontier declares the MIT License; "
                "the package adds no authority or scientific license claim."
            ),
            "url": "https://spdx.org/licenses/MIT.html",
        },
    ]
    for item in objects:
        path = package / str(item["path"])
        entities.append(
            {
                "@id": item["path"],
                "@type": "File",
                "name": f"Vela {item['role']}: {item['id']}",
                "description": (
                    f"Exact retained Vela {item['role']} bytes. The role's "
                    "scientific and authority meaning is defined only by the native manifest."
                ),
                "encodingFormat": "application/json",
                "contentSize": str(path.stat().st_size),
                "identifier": sorted(
                    {str(item["id"]), str(item["root"]), str(item["bytes_root"])}
                ),
            }
        )
    entities.sort(key=lambda entity: str(entity["@id"]))
    return {
        "@context": "https://w3id.org/ro/crate/1.3/context",
        "@graph": entities,
    }


def copy_native(source: Path, destination: Path, native: dict[str, object]) -> None:
    destination.mkdir(parents=True)
    shutil.copyfile(source / "reference.v1.json", destination / "reference.v1.json")
    for item in native["objects"]:
        relative = Path(str(item["path"]))
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / relative, target)


def write_core(
    package: Path, plan: dict[str, object], native: dict[str, object]
) -> dict[str, bytes]:
    loss_bytes = encoded_json(loss_report(native))
    crate_bytes = encoded_json(crate_metadata(package, plan, native, loss_bytes))
    outputs = {
        "ro-crate-metadata.json": crate_bytes,
        "vela-loss-report.v1.json": loss_bytes,
    }
    for name, encoded in outputs.items():
        (package / name).write_bytes(encoded)
    return outputs


def run_json(command: list[str], error: str) -> dict[str, object]:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    require(result.returncode == 0, f"{error}:{result.stderr.strip()}")
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as parse_error:
        raise BuildError(f"{error}:invalid_json") from parse_error
    require(isinstance(value, dict), f"{error}:not_object")
    return value


def native_reader(package: Path) -> dict[str, object]:
    return run_json(
        [
            sys.executable,
            str(NATIVE_VERIFIER),
            "--package-root",
            str(package),
            "--json",
        ],
        "native_reader_failed",
    )


def package_reader(package: Path) -> dict[str, object]:
    return run_json(
        [
            sys.executable,
            str(READER),
            "--package-root",
            str(package),
            "--plan",
            str(PLAN),
            "--plan-amendment",
            str(AMENDMENT),
            "--json",
        ],
        "package_reader_failed",
    )


def external_reader(executable: Path) -> dict[str, object]:
    version = subprocess.run(
        [str(executable), "--version"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    require(
        version.returncode == 0 and "0.11.2" in version.stdout,
        "external_reader_version_mismatch",
    )
    present = subprocess.run(
        [
            str(executable),
            "profiles",
            "describe",
            "ro-crate-1.2",
            "--no-paging",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    missing = subprocess.run(
        [
            str(executable),
            "profiles",
            "describe",
            "ro-crate-1.3",
            "--no-paging",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    require(
        present.returncode == 0
        and missing.returncode != 0
        and 'identifier "ro-crate-1.3" could not be found'
        in f"{missing.stdout}\n{missing.stderr}",
        "external_reader_profile_observation_mismatch",
    )
    return {
        "id": "roc-validator-0.11.2",
        "status": "unsupported_profile",
        "required_profile": "ro-crate-1.3",
        "available_base_profiles": ["ro-crate-1.1", "ro-crate-1.2"],
        "profile_substitution_performed": False,
        "validation_performed": False,
        "observation": (
            "The installed external validator has no RO-Crate 1.3 base profile; "
            "the 1.2 profile was not substituted."
        ),
    }


def reader_diagnostic(package: Path) -> str:
    result = subprocess.run(
        [
            sys.executable,
            str(READER),
            "--package-root",
            str(package),
            "--plan",
            str(PLAN),
            "--plan-amendment",
            str(AMENDMENT),
            "--json",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    require(result.returncode != 0, "mutation_unexpectedly_passed")
    prefix = "scientific-change-package: "
    diagnostic = result.stderr.strip()
    require(diagnostic.startswith(prefix), "mutation_diagnostic_invalid")
    return diagnostic.removeprefix(prefix)


def mutation_results(package: Path) -> list[dict[str, object]]:
    mutations = [
        ("dropped_decision", "native_object_unavailable"),
        ("root_drift", "native_object_root_drift"),
        ("authority_escalation", "authority_escalation"),
        ("missing_loss_report", "loss_report_missing"),
    ]
    results: list[dict[str, object]] = []
    for mutation, expected in mutations:
        with tempfile.TemporaryDirectory(prefix=f"vela-{mutation}-") as raw:
            candidate = Path(raw) / "package"
            shutil.copytree(package, candidate)
            native_path = candidate / "reference.v1.json"
            native = json.loads(native_path.read_bytes())
            if mutation == "dropped_decision":
                decision = next(
                    item
                    for item in native["objects"]
                    if item["role"] == "decision_event"
                )
                (candidate / decision["path"]).unlink()
            elif mutation == "root_drift":
                claim = next(
                    item for item in native["objects"] if item["role"] == "claim"
                )
                path = candidate / claim["path"]
                path.write_bytes(path.read_bytes() + b"\n")
            elif mutation == "authority_escalation":
                native["authority"]["local_standing_effect"] = "accepted"
                native_path.write_bytes(canonical_bytes(native))
            elif mutation == "missing_loss_report":
                (candidate / "vela-loss-report.v1.json").unlink()
            observed = reader_diagnostic(candidate)
            require(observed == expected, f"mutation_diagnostic_drift:{mutation}")
            results.append(
                {
                    "id": mutation,
                    "outcome": "rejected",
                    "diagnostic": observed,
                    "standing_effect": "none",
                }
            )
    return results


def sha256_manifest(package: Path, native: dict[str, object]) -> bytes:
    paths = [
        "reader-result.v1.json",
        "reference.v1.json",
        "result.v1.json",
        "ro-crate-metadata.json",
        "vela-loss-report.v1.json",
        *(str(item["path"]) for item in native["objects"]),
    ]
    lines = [
        f"{hashlib.sha256((package / path).read_bytes()).hexdigest()}  {path}"
        for path in sorted(paths)
    ]
    return ("\n".join(lines) + "\n").encode("utf-8")


def publish_outputs(destination: Path, outputs: dict[str, bytes]) -> None:
    for name, expected in outputs.items():
        path = destination / name
        if path.exists():
            require(
                path.is_file() and path.read_bytes() == expected,
                f"stale_generated_output:{name}",
            )
        else:
            path.write_bytes(expected)


def build(
    source_package: Path,
    publish_to: Path,
    external_validator: Path,
) -> dict[str, object]:
    require(
        source_package.resolve() == publish_to.resolve(),
        "publish_destination_must_be_exact_source_package",
    )
    plan = load_frozen(PLAN, PLAN_ROOT, PLAN_BYTES_SHA256)
    load_frozen(AMENDMENT, AMENDMENT_ROOT, AMENDMENT_BYTES_SHA256)
    native = load_source(source_package, plan)
    with (
        tempfile.TemporaryDirectory(prefix="vela-change-package-a-") as raw_a,
        tempfile.TemporaryDirectory(prefix="vela-change-package-b-") as raw_b,
    ):
        first = Path(raw_a) / "package"
        second = Path(raw_b) / "package"
        copy_native(source_package, first, native)
        copy_native(source_package, second, native)
        first_core = write_core(first, plan, native)
        second_core = write_core(second, plan, native)
        require(first_core == second_core, "core_rebuild_nondeterministic")

        native_result = native_reader(first)
        clean_room_result = package_reader(first)
        external_result = external_reader(external_validator)
        mutations = mutation_results(first)
        reader_suite = {
            "schema": "vela.scientific-change-package-reader-suite-result.v1",
            "plan_root": PLAN_ROOT,
            "plan_amendment_root": AMENDMENT_ROOT,
            "native": {
                "reader": "vela-python-clean-room",
                "status": "pass",
                "result_root": canonical_root(native_result),
                "reference_root": native_result["reference_root"],
                "semantic_chain": "verified",
                "authority_signature": "verified",
                "local_standing_effect": native_result["local_standing_effect"],
            },
            "ro_crate": {
                "reader": "scientific-change-package-clean-room",
                "status": "pass",
                "result_root": canonical_root(clean_room_result),
                "profile": clean_room_result["ro_crate_profile"],
                "exact_native_object_parity": True,
                "local_standing_effect": clean_room_result[
                    "local_standing_effect"
                ],
            },
            "external_ro_crate": external_result,
            "overall": "pass_with_external_profile_gap",
            "standing_effect": "none",
        }
        reader_bytes = encoded_json(reader_suite)
        for package in (first, second):
            (package / "reader-result.v1.json").write_bytes(reader_bytes)

        result = {
            "schema": "vela.scientific-change-package-interoperability-result.v1",
            "outcome": "baseline_complete_with_external_validator_gap",
            "plan_root": PLAN_ROOT,
            "plan_bytes_sha256": PLAN_BYTES_SHA256,
            "plan_amendment_root": AMENDMENT_ROOT,
            "source": {
                "frontier_id": plan["source"]["frontier_id"],
                "claim": plan["source"]["claim"],
                "decision_event": {
                    "id": plan["source"]["decision_event"]["id"],
                    "root": plan["source"]["decision_event"]["root"],
                },
                "source_standing": "accepted",
                "local_standing_effect": "none",
            },
            "representations": {
                "native": {
                    "file": "reference.v1.json",
                    "schema": "vela.foreign-reference.v1",
                    "root": canonical_root(native),
                    "object_set_root": native["object_set_root"],
                    "object_count": len(native["objects"]),
                },
                "ro_crate": {
                    "file": "ro-crate-metadata.json",
                    "profile": "https://w3id.org/ro/crate/1.3",
                    "bytes_sha256": sha256_bytes(first_core["ro-crate-metadata.json"]),
                    "same_native_object_set": True,
                },
                "loss_report": {
                    "file": "vela-loss-report.v1.json",
                    "bytes_sha256": sha256_bytes(
                        first_core["vela-loss-report.v1.json"]
                    ),
                    "semantic_loss_count": 6,
                    "unreported_fields": 0,
                },
            },
            "readers": {
                "suite_file": "reader-result.v1.json",
                "suite_root": canonical_root(reader_suite),
                "native": "pass",
                "ro_crate_clean_room": "pass",
                "external_ro_crate": "unsupported_profile",
            },
            "mutations": mutations,
            "determinism": {
                "clean_rebuilds": 2,
                "generated_outputs_byte_identical": True,
                "network_during_generation": False,
                "compared_files": sorted(GENERATED),
                "sha256_manifest_entry_count": len(native["objects"]) + 5,
            },
            "authority": {
                "frontier_mutation": False,
                "proposal_or_verification_import": False,
                "credential_or_key_access": False,
                "source_standing": "accepted",
                "local_standing_effect": "none",
                "requires_local_decision": True,
            },
            "nonclaims": plan["nonclaims"],
        }
        result_bytes = encoded_json(result)
        for package in (first, second):
            (package / "result.v1.json").write_bytes(result_bytes)
            (package / "SHA256SUMS").write_bytes(
                sha256_manifest(package, native)
            )

        for name in GENERATED:
            require(
                (first / name).read_bytes() == (second / name).read_bytes(),
                f"generated_output_nondeterministic:{name}",
            )
        final_reader = package_reader(first)
        require(
            final_reader == clean_room_result,
            "final_reader_result_drift",
        )
        publish_outputs(
            publish_to,
            {name: (first / name).read_bytes() for name in GENERATED},
        )
        return {
            "schema": "vela.scientific-change-package-materialization.v1",
            "ok": True,
            "plan_root": PLAN_ROOT,
            "plan_amendment_root": AMENDMENT_ROOT,
            "native_manifest_root": canonical_root(native),
            "object_set_root": native["object_set_root"],
            "object_count": len(native["objects"]),
            "ro_crate_metadata_sha256": sha256_bytes(
                (first / "ro-crate-metadata.json").read_bytes()
            ),
            "loss_report_sha256": sha256_bytes(
                (first / "vela-loss-report.v1.json").read_bytes()
            ),
            "reader_result_sha256": sha256_bytes(
                (first / "reader-result.v1.json").read_bytes()
            ),
            "result_sha256": sha256_bytes((first / "result.v1.json").read_bytes()),
            "sha256_manifest_sha256": sha256_bytes(
                (first / "SHA256SUMS").read_bytes()
            ),
            "external_ro_crate_profile": "unsupported",
            "local_standing_effect": "none",
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-package", required=True, type=Path)
    parser.add_argument("--publish-to", required=True, type=Path)
    parser.add_argument("--roc-validator", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = build(
            args.source_package.expanduser().resolve(),
            args.publish_to.expanduser().resolve(),
            args.roc_validator.expanduser().resolve(),
        )
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (
        BuildError,
        OSError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"build scientific change package: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
