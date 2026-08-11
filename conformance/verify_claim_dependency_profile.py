#!/usr/bin/env python3
"""Verify the noncanonical claim-dependency-profile.v0 experiment."""

from __future__ import annotations

import base64
import errno
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from copy import deepcopy
from pathlib import Path

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.serialization import load_der_public_key

ROOT = Path(__file__).resolve().parent.parent
EXPERIMENT = ROOT / "conformance/experiments/claim-dependency-profile-v0"
sys.path.insert(0, str(ROOT / "conformance/readers/python"))
from canonical import canonical_bytes

SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
CLAIM_ID = re.compile(r"vcl_[0-9a-f]{64}\Z")
REPOSITORY_ID = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z"
)
COMMON_NONCLAIMS = [
    "Current Math Claims at a6a31a528ee86ab79c2aaf4e71e43fc63f4a4e98 have empty relations arrays.",
    "Dependency status does not establish Claim truth, acceptance, authority, Standing, route independence, or scientific resolution.",
    "This artifact is an experiment profile, not a Submission field, Claim field, relation object, Vela schema, or protocol byte.",
    "This synthetic counterfactual is not Class E evidence, an accepted-state Correction, or a rooted real dependent; real A0 was rejected.",
]


class ContractError(ValueError):
    pass


def fail(code: str) -> None:
    raise ContractError(code)


def canonical_root(value: object) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def exact(value: object, names: set[str], code: str) -> dict:
    if not isinstance(value, dict) or set(value) != names:
        fail(code)
    return value


def claim_ref(value: object, *, node: bool = False) -> dict:
    names = {"claim_id", "claim_root", "repository_id", "repository_origin_root"}
    item = exact(
        value, names | ({"label"} if node else set()), "profile_claim_ref_malformed"
    )
    if (
        not CLAIM_ID.fullmatch(item.get("claim_id", ""))
        or not SHA256.fullmatch(item.get("claim_root", ""))
        or not REPOSITORY_ID.fullmatch(item.get("repository_id", ""))
        or not SHA256.fullmatch(item.get("repository_origin_root", ""))
        or (node and (not isinstance(item["label"], str) or not item["label"]))
    ):
        fail("profile_claim_ref_malformed")
    return item


def validate_profile(value: object) -> dict:
    profile = exact(
        value,
        {
            "schema",
            "profile_version",
            "experiment_id",
            "scope",
            "nodes",
            "dependencies",
            "does_not_establish",
        },
        "profile_schema_unsupported",
    )
    if (
        profile["schema"] != "vela.claim-dependency-profile.v0"
        or profile["profile_version"] != 0
    ):
        fail("profile_schema_unsupported")
    if not isinstance(profile["experiment_id"], str) or not profile["experiment_id"]:
        fail("profile_scope_invalid")
    scope = exact(
        profile["scope"],
        {
            "repository_id",
            "repository_origin_root",
            "max_claims",
            "max_dependencies",
            "complete_claim_set",
            "complete_dependency_set",
        },
        "profile_scope_invalid",
    )
    if (
        not REPOSITORY_ID.fullmatch(scope.get("repository_id", ""))
        or not SHA256.fullmatch(scope.get("repository_origin_root", ""))
        or type(scope["max_claims"]) is not int
        or not 1 <= scope["max_claims"] <= 64
        or type(scope["max_dependencies"]) is not int
        or not 0 <= scope["max_dependencies"] <= 128
        or type(scope["complete_claim_set"]) is not bool
        or type(scope["complete_dependency_set"]) is not bool
    ):
        fail("profile_scope_invalid")
    nonclaims = profile["does_not_establish"]
    if (
        not isinstance(nonclaims, list)
        or len(nonclaims) < 3
        or any(not isinstance(item, str) or not item.strip() for item in nonclaims)
        or nonclaims != sorted(set(nonclaims))
    ):
        fail("profile_does_not_establish_invalid")
    if (
        not isinstance(profile["nodes"], list)
        or len(profile["nodes"]) > scope["max_claims"]
    ):
        fail("profile_claim_bound_exceeded")
    context = (scope["repository_id"], scope["repository_origin_root"])
    nodes: dict[str, dict] = {}
    node_order = []
    for raw_node in profile["nodes"]:
        item = claim_ref(raw_node, node=True)
        if (item["repository_id"], item["repository_origin_root"]) != context:
            fail("profile_repository_context_mismatch")
        if item["claim_id"] in nodes:
            fail("profile_node_duplicate")
        nodes[item["claim_id"]] = item
        node_order.append((item["claim_id"], item["claim_root"]))
    if node_order != sorted(node_order):
        fail("profile_node_order_invalid")
    dependencies = profile["dependencies"]
    if (
        not isinstance(dependencies, list)
        or len(dependencies) > scope["max_dependencies"]
    ):
        fail("profile_dependency_bound_exceeded")
    dependency_order = []
    for raw_dependency in dependencies:
        item = exact(
            raw_dependency,
            {"kind", "source", "target"},
            "profile_dependency_kind_unsupported",
        )
        if item["kind"] != "requires":
            fail("profile_dependency_kind_unsupported")
        source, target = claim_ref(item["source"]), claim_ref(item["target"])
        if any(
            (ref["repository_id"], ref["repository_origin_root"]) != context
            for ref in (source, target)
        ):
            fail("profile_repository_context_mismatch")
        if (source["claim_id"], source["claim_root"]) == (
            target["claim_id"],
            target["claim_root"],
        ):
            fail("profile_dependency_self_reference")
        for endpoint in (source, target):
            if endpoint["claim_id"] not in nodes:
                fail("profile_dependency_endpoint_missing")
            if endpoint["claim_root"] != nodes[endpoint["claim_id"]]["claim_root"]:
                fail("profile_dependency_endpoint_root_mismatch")
        dependency_order.append(
            (
                source["claim_id"],
                source["claim_root"],
                target["claim_id"],
                target["claim_root"],
            )
        )
    if len(set(dependency_order)) != len(dependency_order):
        fail("profile_dependency_duplicate")
    if dependency_order != sorted(dependency_order):
        fail("profile_dependency_order_invalid")
    return profile


def validate_state(value: object, profile: dict) -> dict:
    state = exact(
        value,
        {
            "schema",
            "experiment_id",
            "repository_id",
            "repository_origin_root",
            "scenario",
            "claims",
            "transition",
        },
        "state_schema_unsupported",
    )
    if (
        state["schema"] != "vela.claim-dependency-state.v0"
        or state["scenario"] != "synthetic_counterfactual_over_retained_math_anchors"
    ):
        fail("state_schema_unsupported")
    scope = profile["scope"]
    if state["experiment_id"] != profile["experiment_id"] or (
        state["repository_id"],
        state["repository_origin_root"],
    ) != (scope["repository_id"], scope["repository_origin_root"]):
        fail("state_repository_context_mismatch")
    if (
        not isinstance(state["claims"], list)
        or len(state["claims"]) > scope["max_claims"]
    ):
        fail("profile_claim_bound_exceeded")
    ids, roots, labels = [], [], []
    nodes = {item["claim_id"]: item for item in profile["nodes"]}
    for item in state["claims"]:
        claim = exact(
            item,
            {
                "label",
                "claim_id",
                "claim_root",
                "availability",
                "lifecycle",
                "verification",
            },
            "state_schema_unsupported",
        )
        if (
            not isinstance(claim["label"], str)
            or not CLAIM_ID.fullmatch(claim.get("claim_id", ""))
            or not SHA256.fullmatch(claim.get("claim_root", ""))
            or claim["availability"] not in {"available", "unavailable"}
            or claim["lifecycle"] not in {"accepted", "retired", "unaccepted"}
        ):
            fail("state_schema_unsupported")
        verification = claim["verification"]
        if verification is not None:
            exact(
                verification,
                {
                    "verification_id",
                    "verification_root",
                    "input_claim_root",
                    "property",
                },
                "state_schema_unsupported",
            )
            if (
                not isinstance(verification["verification_id"], str)
                or not SHA256.fullmatch(verification.get("verification_root", ""))
                or verification["input_claim_root"] != claim["claim_root"]
                or verification["property"] != "claim_dependency_fidelity.v0"
            ):
                fail("state_schema_unsupported")
        node = nodes.get(claim["claim_id"])
        if node is None or (node["label"], node["claim_root"]) != (
            claim["label"],
            claim["claim_root"],
        ):
            fail("state_claim_outside_profile")
        labels.append(claim["label"])
        ids.append(claim["claim_id"])
        roots.append(claim["claim_root"])
    if len(set(ids)) != len(ids) or len(set(labels)) != len(labels):
        fail("state_claim_duplicate")
    if len(set(roots)) != len(roots):
        fail("state_claim_root_duplicate")
    if ids != sorted(ids):
        fail("state_claim_order_invalid")
    transition = exact(
        state["transition"],
        {"kind", "predecessor", "successor"},
        "state_transition_invalid",
    )
    if transition["kind"] != "counterfactual_supersession":
        fail("state_transition_invalid")
    claims = {item["claim_id"]: item for item in state["claims"]}
    for side, lifecycle in (("predecessor", "retired"), ("successor", "accepted")):
        endpoint = exact(
            transition[side], {"claim_id", "claim_root"}, "state_transition_invalid"
        )
        claim = claims.get(endpoint["claim_id"])
        if claim is None or (claim["claim_root"], claim["lifecycle"]) != (
            endpoint["claim_root"],
            lifecycle,
        ):
            fail("state_transition_invalid")
    return state


def derive(profile_value: object, state_value: object) -> dict:
    profile = validate_profile(profile_value)
    state = validate_state(state_value, profile)
    claims = {item["claim_id"]: item for item in state["claims"]}
    dependencies: dict[str, list[dict]] = {}
    for item in profile["dependencies"]:
        dependencies.setdefault(item["source"]["claim_id"], []).append(item["target"])
    visiting, visited = set(), set()

    def visit(claim_id: str) -> None:
        if claim_id in visiting:
            fail("dependency_cycle")
        if claim_id in visited:
            return
        visiting.add(claim_id)
        for target in dependencies.get(claim_id, []):
            visit(target["claim_id"])
        visiting.remove(claim_id)
        visited.add(claim_id)

    for source in sorted(dependencies):
        visit(source)
    memo: dict[str, tuple[str, list[dict], int | None]] = {}

    def reduce(claim_id: str) -> tuple[str, list[dict], int | None]:
        if claim_id in memo:
            return memo[claim_id]
        source, rank, layer, evidence = claims.get(claim_id), 0, None, []
        if source is None:
            rank = 2
            evidence.append({"code": "source_missing"})
        elif source["availability"] == "unavailable":
            rank = 2
            evidence.append({"code": "source_unavailable"})
        elif source["lifecycle"] == "unaccepted":
            rank = 2
            evidence.append({"code": "source_unaccepted"})
        for target in dependencies.get(claim_id, []):
            item = claims.get(target["claim_id"])
            if item is None:
                rank = 2
                evidence.append(
                    {"code": "target_missing", "target_claim_id": target["claim_id"]}
                )
                continue
            if item["claim_root"] != target["claim_root"]:
                fail("dependency_target_root_mismatch")
            if item["availability"] == "unavailable":
                rank = 2
                evidence.append(
                    {
                        "code": "target_unavailable",
                        "target_claim_id": target["claim_id"],
                    }
                )
                continue
            if item["lifecycle"] == "unaccepted":
                rank = 2
                evidence.append(
                    {"code": "target_unaccepted", "target_claim_id": target["claim_id"]}
                )
                continue
            if item["lifecycle"] == "retired":
                rank = max(rank, 1)
                layer = layer or 0
                evidence.append(
                    {"code": "target_retired", "target_claim_id": target["claim_id"]}
                )
                continue
            nested, _, nested_layer = reduce(target["claim_id"])
            if nested == "incomplete":
                rank = 2
                evidence.append(
                    {
                        "code": "transitive_incomplete",
                        "target_claim_id": target["claim_id"],
                    }
                )
            elif nested == "review_required":
                rank = max(rank, 1)
                layer = max(layer or 0, (nested_layer or 0) + 1)
                evidence.append(
                    {
                        "code": "transitive_review_required",
                        "target_claim_id": target["claim_id"],
                    }
                )
        if not profile["scope"]["complete_claim_set"]:
            rank = 2
            evidence.append({"code": "claim_set_incomplete"})
        if not profile["scope"]["complete_dependency_set"]:
            rank = 2
            evidence.append({"code": "dependency_set_incomplete"})
        memo[claim_id] = (
            ("satisfied", "review_required", "incomplete")[rank],
            evidence,
            layer if rank == 1 else None,
        )
        return memo[claim_id]

    endpoints = {
        state["transition"][side]["claim_id"] for side in ("predecessor", "successor")
    }
    results, obligations, stale = [], [], []
    for node in profile["nodes"]:
        if node["claim_id"] in endpoints:
            continue
        claim = claims.get(node["claim_id"])
        status_value, evidence, layer = reduce(node["claim_id"])
        results.append(
            {
                "label": node["label"],
                "claim_id": node["claim_id"],
                "claim_root": node["claim_root"],
                "dependency_status": status_value,
                "evidence": evidence,
                "repair_layer": layer,
            }
        )
        if (
            status_value != "satisfied"
            and claim is not None
            and claim["verification"] is not None
        ):
            stale.append(claim["verification"]["verification_id"])
        if status_value == "review_required":
            preimage = {
                "schema": "vela.claim-dependency-repair-obligation.v0",
                "claim_id": node["claim_id"],
                "claim_root": node["claim_root"],
                "evidence": evidence,
                "discharge_condition": "Re-establish every exact requires edge against current accepted targets, narrow the Claim, or retract it.",
            }
            obligations.append(
                {
                    "label": node["label"],
                    "repair_layer": layer,
                    "obligation_root": canonical_root(preimage),
                    **{
                        key: value for key, value in preimage.items() if key != "schema"
                    },
                }
            )
    results.sort(key=lambda item: item["label"])
    obligations.sort(key=lambda item: item["label"])
    sets = {
        status: sorted(
            item["label"] for item in results if item["dependency_status"] == status
        )
        for status in ("satisfied", "review_required", "incomplete")
    }
    sets |= {
        "unaffected": list(sets["satisfied"]),
        "stale_verifications": sorted(stale),
        "repair_required": sorted(item["label"] for item in obligations),
    }
    batches = [
        {
            "batch": layer + 1,
            "repair_layer": layer,
            "labels": sorted(
                item["label"] for item in obligations if item["repair_layer"] == layer
            ),
            "obligation_roots": sorted(
                item["obligation_root"]
                for item in obligations
                if item["repair_layer"] == layer
            ),
        }
        for layer in sorted({item["repair_layer"] for item in obligations})
    ]
    overall = (
        "incomplete"
        if sets["incomplete"]
        else "review_required"
        if sets["review_required"]
        else "satisfied"
    )
    return {
        "schema": "vela.claim-dependency-projection.v0",
        "experiment_id": profile["experiment_id"],
        "profile_canonical_root": canonical_root(profile),
        "state_canonical_root": canonical_root(state),
        "overall_status": overall,
        "claims": results,
        "sets": sets,
        "repair_obligations": obligations,
        "repair_batches": batches,
        "authority_effect": "none",
    }


def load(name: str) -> dict:
    return json.loads((EXPERIMENT / name).read_text(encoding="utf-8"))


def verify_carrier(base: Path, carrier: dict) -> bytes:
    exact(carrier, {"schema", "entry"}, "carrier_schema_unsupported")
    if carrier["schema"] != "vela.claim-dependency-profile-carrier.v0":
        fail("carrier_schema_unsupported")
    entry = exact(
        carrier["entry"],
        {"path", "mode", "size_bytes", "max_bytes", "sha256"},
        "carrier_schema_unsupported",
    )
    path_text = entry["path"]
    if (
        not isinstance(path_text, str)
        or not path_text
        or "\\" in path_text
        or "\0" in path_text
    ):
        fail("carrier_path_invalid")
    relative = Path(path_text)
    if (
        relative.is_absolute()
        or len(relative.parts) != 1
        or relative.parts[0] in {".", ".."}
    ):
        fail("carrier_path_escape")
    path = base / relative
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK | getattr(os, "O_NOFOLLOW", 0),
        )
    except OSError as error:
        fail(
            "carrier_entry_symlink"
            if error.errno in {errno.ELOOP, errno.EMLINK}
            else "carrier_entry_nonregular"
        )
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            fail("carrier_entry_nonregular")
        if stat.S_IMODE(before.st_mode) != 0o644 or entry["mode"] != "100644":
            fail("carrier_entry_mode_invalid")
        if (
            type(entry["size_bytes"]) is not int
            or type(entry["max_bytes"]) is not int
            or not 1 <= entry["size_bytes"] <= entry["max_bytes"] <= 65536
            or before.st_size != entry["size_bytes"]
        ):
            fail("carrier_entry_size_mismatch")
        chunks = []
        remaining = entry["max_bytes"] + 1
        while remaining:
            chunk = os.read(descriptor, min(remaining, 65536))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after, current = os.fstat(descriptor), path.lstat()
        before_identity = (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_mtime_ns,
        )
        after_identity = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if before_identity != after_identity or (current.st_dev, current.st_ino) != (
            before.st_dev,
            before.st_ino,
        ):
            fail("carrier_entry_root_mismatch")
    finally:
        os.close(descriptor)
    if (
        len(data) != entry["size_bytes"]
        or "sha256:" + hashlib.sha256(data).hexdigest() != entry["sha256"]
    ):
        fail("carrier_entry_root_mismatch")
    return data


def mutate(profile: dict, state: dict, vector: dict) -> tuple[dict, dict]:
    profile, state = deepcopy(profile), deepcopy(state)
    mutation = vector["mutation"]
    document = profile if vector["document"] == "profile" else state
    target = document
    for part in mutation["path"][:-1]:
        target = target[part]
    key, operation = mutation["path"][-1], mutation["op"]
    if operation == "set":
        target[key] = mutation["value"]
    elif operation == "append_copy":
        target[key].append(deepcopy(target[key][mutation["index"]]))
    elif operation == "swap":
        left, right = mutation["indices"]
        target[key][left], target[key][right] = target[key][right], target[key][left]
    elif operation == "remove_label":
        target[key][:] = [
            item for item in target[key] if item["label"] != mutation["label"]
        ]
    elif operation == "append_dependency":
        target[key].append(deepcopy(mutation["value"]))
        target[key].sort(
            key=lambda item: (
                item["source"]["claim_id"],
                item["source"]["claim_root"],
                item["target"]["claim_id"],
                item["target"]["claim_root"],
            )
        )
    else:
        fail("negative_vector_operation_unsupported")
    return profile, state


def expect_error(code: str, action) -> None:
    try:
        action()
    except ContractError as error:
        if str(error) == code:
            return
        raise
    raise AssertionError(f"expected {code}")


def verify_vectors(profile: dict, state: dict) -> None:
    for vector in load("negative-vectors.json")["vectors"]:
        changed = mutate(profile, state, vector)
        if "expected_error" in vector:
            expect_error(vector["expected_error"], lambda changed=changed: derive(*changed))
            continue
        projection = derive(*changed)
        for key, expected in vector["expected_sets"].items():
            if projection["sets"][key] != expected:
                raise AssertionError(f"{vector['id']}:{key}")


def verify_carrier_adversarial(carrier: dict) -> None:
    with tempfile.TemporaryDirectory() as directory:
        base, path = Path(directory), Path(directory) / "profile.json"
        shutil.copyfile(EXPERIMENT / "profile.json", path)
        path.chmod(0o644)
        verify_carrier(base, carrier)
        for code, change in (
            ("carrier_path_invalid", {"path": "bad\0name"}),
            ("carrier_path_escape", {"path": "../profile.json"}),
            ("carrier_entry_nonregular", {"path": "missing.json"}),
            (
                "carrier_entry_size_mismatch",
                {"size_bytes": carrier["entry"]["size_bytes"] + 1},
            ),
            ("carrier_entry_root_mismatch", {"sha256": "sha256:" + "0" * 64}),
        ):
            changed = deepcopy(carrier)
            changed["entry"].update(change)
            expect_error(code, lambda changed=changed: verify_carrier(base, changed))
        original = path.read_bytes()
        path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        path.chmod(0o644)
        expect_error(
            "carrier_entry_root_mismatch", lambda: verify_carrier(base, carrier)
        )
        path.write_bytes(original)
        path.chmod(0o755)
        expect_error(
            "carrier_entry_mode_invalid", lambda: verify_carrier(base, carrier)
        )
        path.unlink()
        os.symlink(EXPERIMENT / "profile.json", path)
        expect_error("carrier_entry_symlink", lambda: verify_carrier(base, carrier))
        path.unlink()
        path.mkdir()
        expect_error("carrier_entry_nonregular", lambda: verify_carrier(base, carrier))
        path.rmdir()
        os.mkfifo(path, 0o644)
        expect_error("carrier_entry_nonregular", lambda: verify_carrier(base, carrier))


def verify_manifest_and_baseline() -> None:
    manifest = load("manifest.json")
    exact(manifest, {"schema", "files"}, "fixture_manifest_invalid")
    if manifest["schema"] != "vela.claim-dependency-profile-fixture-manifest.v0":
        fail("fixture_manifest_invalid")
    expected_paths = [
        "baseline/raw-source.json",
        "baseline/review-record.json",
        "baseline/ro-crate-metadata.json",
        "carrier.json",
        "dependency-semantics.json",
        "expected.json",
        "negative-vectors.json",
        "participant-task.json",
        "preregistration.json",
        "profile.json",
        "state.json",
    ]
    entries = manifest["files"]
    if (
        not isinstance(entries, list)
        or any(not isinstance(entry, dict) for entry in entries)
        or [entry.get("path") for entry in entries] != expected_paths
    ):
        fail("fixture_manifest_invalid")
    for entry in entries:
        exact(
            entry,
            {"path", "mode", "size_bytes", "sha256"},
            "fixture_manifest_invalid",
        )
        relative = Path(entry["path"])
        if relative.is_absolute() or ".." in relative.parts:
            fail("fixture_manifest_path_invalid")
        path = EXPERIMENT / relative
        info = path.lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o644
            or entry["mode"] != "100644"
        ):
            fail("fixture_manifest_mode_mismatch")
        if (
            type(entry["size_bytes"]) is not int
            or info.st_size != entry["size_bytes"]
            or info.st_size > 1048576
        ):
            fail("fixture_manifest_root_mismatch")
        data = path.read_bytes()
        if (
            len(data) != entry["size_bytes"]
            or "sha256:" + hashlib.sha256(data).hexdigest() != entry["sha256"]
        ):
            fail("fixture_manifest_root_mismatch")
    raw_source = load("baseline/raw-source.json")
    exact(
        raw_source,
        {
            "schema",
            "experiment_id",
            "synthetic_counterfactual",
            "statements",
            "claim_roots",
            "does_not_establish",
        },
        "baseline_raw_source_invalid",
    )
    if (
        raw_source["schema"] != "vela.claim-dependency-raw-source.v0"
        or not raw_source["synthetic_counterfactual"]
        or raw_source["does_not_establish"] != COMMON_NONCLAIMS
    ):
        fail("baseline_raw_source_invalid")
    review = load("baseline/review-record.json")
    if (
        not review["fixture_only"]
        or review["authority_effect"] != "none"
        or "profile" in review["review"]["reviewed_roots"]
        or review["review_canonical_root"] != canonical_root(review["review"])
    ):
        fail("baseline_review_record_invalid")
    reviewed_roots = {
        "raw_source": canonical_root(raw_source),
        "ro_crate": canonical_root(load("baseline/ro-crate-metadata.json")),
        "participant_task": canonical_root(load("participant-task.json")),
        "dependency_semantics": canonical_root(load("dependency-semantics.json")),
        "state": canonical_root(load("state.json")),
    }
    if review["review"]["reviewed_roots"] != reviewed_roots:
        fail("baseline_review_record_invalid")
    try:
        public_key = load_der_public_key(
            base64.b64decode(review["public_key_spki_base64"])
        )
        public_key.verify(
            base64.b64decode(review["signature_base64"]),
            canonical_bytes(review["review"]),
        )
    except (ValueError, TypeError, InvalidSignature) as error:
        raise ContractError("baseline_review_signature_invalid") from error
    preregistration = load("preregistration.json")
    if (
        preregistration["shared"]["facts_root"] != reviewed_roots["raw_source"]
        or preregistration["shared"]["participant_task_root"]
        != reviewed_roots["participant_task"]
        or preregistration["shared"]["semantics_root"]
        != reviewed_roots["dependency_semantics"]
    ):
        fail("preregistration_common_context_mismatch")
    if preregistration["common_context"] != [
        "baseline/raw-source.json",
        "state.json",
        "participant-task.json",
        "dependency-semantics.json",
        "baseline/ro-crate-metadata.json",
        "baseline/review-record.json",
    ] or [
        (arm["id"], arm["supplemental_context"]) for arm in preregistration["arms"]
    ] != [
        ("disciplined-git-ro-crate", []),
        ("rooted-source-plus-profile", ["profile.json"]),
    ]:
        fail("preregistration_arm_mismatch")
    for name, observation in preregistration["observations"].items():
        expected = (
            "not_computable"
            if name == "correct_reusable_scientific_state_transitions_per_expert_minute"
            else "not_measured"
        )
        if (
            observation["status"] != expected
            or observation["value"] is not None
            or observation["denominator"] is not None
            or observation["source_run_ids"] != []
        ):
            fail("preregistration_observation_invalid")


def main() -> int:
    profile, state, expected, carrier = (
        load("profile.json"),
        load("state.json"),
        load("expected.json"),
        load("carrier.json"),
    )
    if json.loads(verify_carrier(EXPERIMENT, carrier)) != profile:
        raise AssertionError("carrier profile parse drift")
    projection = derive(profile, state)
    wrapper = {
        "schema": "vela.claim-dependency-profile-expected.v0",
        "profile_canonical_root": canonical_root(profile),
        "state_canonical_root": canonical_root(state),
        "projection_canonical_root": canonical_root(projection),
        "projection": projection,
    }
    if expected != wrapper:
        raise AssertionError("positive expected projection drift")
    verify_vectors(profile, state)
    verify_carrier_adversarial(carrier)
    verify_manifest_and_baseline()
    node = shutil.which("node")
    if node is None:
        raise AssertionError("node is required")
    result = subprocess.run(
        [
            node,
            ROOT / "conformance/readers/javascript/claim_dependency_profile.mjs",
            EXPERIMENT,
        ],
        cwd=ROOT,
        timeout=30,
        check=False,
    )
    if result.returncode:
        return result.returncode
    print(
        f"claim-dependency-profile-v0: {canonical_root(profile)} {canonical_root(projection)} ok"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ContractError, AssertionError, OSError, json.JSONDecodeError) as error:
        print(f"claim-dependency-profile-v0: FAIL: {error}", file=sys.stderr)
        raise SystemExit(1) from error
