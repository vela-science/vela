#!/usr/bin/env python3
"""Check the four Protocol 1 reference flows without network or authority."""

from __future__ import annotations

import base64
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXAMPLES = ROOT / "examples"
sys.path.insert(0, str(ROOT / "conformance/readers/python"))
from canonical import canonical_bytes


def load(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"{path} must contain one object")
    return value


def digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )


def check_flow_documents() -> None:
    for name in (
        "formal-math",
        "computational-science",
        "correction-inheritance",
        "portable-divergence",
    ):
        flow = load(EXAMPLES / name / "flow.json")
        if flow.get("schema") != "vela.reference-flow.v1":
            raise AssertionError(f"{name}: wrong reference-flow schema")
        if flow.get("authority_effect") != "none":
            raise AssertionError(f"{name}: examples may not carry authority")
        nonclaims = flow.get("does_not_establish")
        if not isinstance(nonclaims, list) or not nonclaims:
            raise AssertionError(f"{name}: missing explicit nonclaims")


def check_computational_flow() -> None:
    directory = EXAMPLES / "computational-science"
    flow = load(directory / "flow.json")
    result = directory / "result.json"
    if digest(result) != flow["result_sha256"]:
        raise AssertionError("computational result digest drift")
    recomputed = run(
        [sys.executable, str(directory / "experiment.py"), "--check", str(result)]
    )
    if recomputed.returncode != 0:
        raise AssertionError(recomputed.stderr)

    node = shutil.which("node")
    if node is None:
        raise AssertionError("node is required for the foreign-producer flow")
    with tempfile.TemporaryDirectory(prefix="vela-reference-flow-") as temporary:
        work = Path(temporary)
        seed = work / "producer.seed.hex"
        shutil.copyfile(ROOT / "conformance/current-objects/producer.seed.hex", seed)
        seed.chmod(0o600)
        outputs = []
        for language, command in (
            ("python", [sys.executable, str(ROOT / "conformance/emitters/python.py")]),
            ("javascript", [node, str(ROOT / "conformance/emitters/javascript.mjs")]),
        ):
            output = work / f"{language}-submission.json"
            emitted = run(
                [
                    *command,
                    "submission",
                    "--draft",
                    str(directory / "submission-draft.json"),
                    "--seed-file",
                    str(seed),
                    "--actor",
                    flow["producer"],
                    "--actor-class",
                    "agent",
                    "--declared-at",
                    "2026-08-11T00:00:00Z",
                    "--output",
                    str(output),
                ]
            )
            if emitted.returncode != 0:
                raise AssertionError(emitted.stderr)
            if digest(output) != flow["submission_root"]:
                raise AssertionError(f"{language} submission root drift")
            outputs.append(output.read_bytes())
        if outputs[0] != outputs[1]:
            raise AssertionError("foreign producers emitted different bytes")

        for command in (
            [node, str(ROOT / "conformance/readers/javascript/object.mjs")],
            [sys.executable, str(ROOT / "conformance/readers/python/object.py")],
        ):
            read = run([*command, str(work / "python-submission.json")])
            if read.returncode != 0:
                raise AssertionError(read.stderr)
            summary = json.loads(read.stdout)
            if (summary["root"], summary["id"]) != (
                flow["submission_root"],
                flow["submission_id"],
            ):
                raise AssertionError("foreign reader result drift")


def check_correction_flow() -> None:
    flow = load(EXAMPLES / "correction-inheritance/flow.json")
    authority = load(ROOT / flow["real_authority_fixture"] / "expected.json")
    if (
        authority["terminal"]["repository_manifest_root"]
        != flow["real_terminal_repository_root"]
    ):
        raise AssertionError("real correction terminal root drift")
    if (
        len(authority["terminal"]["accepted_claims"])
        != flow["real_accepted_claim_count"]
    ):
        raise AssertionError("real correction accepted Claim count drift")
    input_value = load(ROOT / flow["cascade_fixture"])
    input_root = "sha256:" + hashlib.sha256(canonical_bytes(input_value)).hexdigest()
    if input_root != flow["cascade_input_root"]:
        raise AssertionError("correction cascade input root drift")
    expected = load(ROOT / "conformance/fixtures/correction/diamond-expected.json")
    if expected["projection_root"] != flow["cascade_projection_root"]:
        raise AssertionError("correction cascade projection root drift")


def check_formal_math_flow() -> None:
    flow = load(EXAMPLES / "formal-math/flow.json")
    if flow["repository_commit"] != "f9b28280881472ccb9c4b1b35d8e741745f0bd99":
        raise AssertionError(
            "formal-math flow no longer pins the current compact lineage"
        )
    if (
        flow["repository_root"]
        != "sha256:45640c5eea54693df444eada6dd1a7c1f5a4b4ef266fddf79cf51d083233ebba"
    ):
        raise AssertionError("formal-math repository root drift")
    if flow["accepted_claim_count"] != 3:
        raise AssertionError("formal-math current accepted count drift")
    if (
        flow["evidence_artifact_root"]
        != "sha256:789c9dc5e4c1c234450a7ebd03d7b4fb8e0ba6deab12098e2fb17b3e74bada10"
    ):
        raise AssertionError("formal-math correction evidence root drift")


def check_portable_divergence_flow() -> None:
    flow = load(EXAMPLES / "portable-divergence/flow.json")
    fixture = flow["fixture"]
    submission_path = ROOT / fixture["submission_path"]
    artifact_path = ROOT / fixture["artifact_path"]
    if digest(submission_path) != fixture["submission_root"]:
        raise AssertionError("portable divergence Submission root drift")
    if digest(artifact_path) != fixture["artifact_root"]:
        raise AssertionError("portable divergence Artifact root drift")
    envelope = load(submission_path)
    payload_bytes = base64.b64decode(envelope["payload"], validate=True)
    payload = json.loads(payload_bytes)
    if canonical_bytes(payload) != payload_bytes:
        raise AssertionError("portable divergence payload is not canonical JSON")
    if payload["identity"]["actor_id"] != fixture["producer"]:
        raise AssertionError("portable divergence producer drift")
    if payload["identity"]["public_key_hex"] != fixture["producer_public_key"]:
        raise AssertionError("portable divergence producer key drift")
    if payload["claim"]["assertion"] != flow["derived_claim"]["assertion"]:
        raise AssertionError("portable divergence Claim assertion drift")
    if payload["verification_requirements"] != [fixture["verification_requirement"]]:
        raise AssertionError("portable divergence verification requirement drift")
    if envelope["signatures"][0]["keyid"] != fixture["producer_public_key"]:
        raise AssertionError("portable divergence envelope key drift")
    if fixture["submission_id"] != "vsb_" + fixture["submission_root"][7:23]:
        raise AssertionError("portable divergence Submission handle drift")

    frozen = flow["frozen_histories"]
    frozen_path = ROOT / frozen["expected_path"]
    if digest(frozen_path) != frozen["expected_root"]:
        raise AssertionError("portable divergence frozen expectations drift")
    roots = load(frozen_path)
    if roots["schema"] != "vela.portable-divergence-fixture.v1":
        raise AssertionError("portable divergence frozen schema drift")
    if roots["authority_effect"] != "none":
        raise AssertionError("portable divergence fixtures may not carry authority")
    for key in ("submission_id", "submission_root"):
        if roots[key] != fixture[key]:
            raise AssertionError(f"portable divergence frozen {key} drift")
    for key in ("claim_id", "claim_root"):
        if roots[key] != flow["derived_claim"][key]:
            raise AssertionError(f"portable divergence frozen {key} drift")
    if roots["does_not_establish"] != flow["does_not_establish"]:
        raise AssertionError("portable divergence frozen nonclaims drift")

    root_fields = (
        "bundle_root",
        "decision_record_root",
        "event_log_root",
        "keyset_root",
        "model_root",
        "origin_root",
        "projection_root",
        "repository_root",
        "sequence_one_record_root",
    )
    for name in ("accept", "reject"):
        history = roots[name]
        bundle_path = ROOT / frozen[f"{name}_bundle_path"]
        if history["bundle"] != bundle_path.name:
            raise AssertionError(f"portable divergence {name} bundle path drift")
        if digest(bundle_path) != history["bundle_root"]:
            raise AssertionError(f"portable divergence {name} bundle drift")
        if history["bundle_root"] != frozen[f"{name}_bundle_root"]:
            raise AssertionError(f"portable divergence {name} bundle binding drift")
        verified = run(["git", "bundle", "verify", str(bundle_path)])
        if verified.returncode != 0:
            raise AssertionError(verified.stderr)
        for key in root_fields:
            value = history[key]
            if not isinstance(value, str) or len(value) != 71 or not value.startswith(
                "sha256:"
            ):
                raise AssertionError(f"portable divergence {name} {key} malformed")
        if history["git_commit"] == history["git_tree"]:
            raise AssertionError(f"portable divergence {name} Git binding malformed")
        if not history["events"]:
            raise AssertionError(f"portable divergence {name} events missing")
        for event in history["events"]:
            if len(event["root"]) != 71 or not event["root"].startswith("sha256:"):
                raise AssertionError(f"portable divergence {name} Event root malformed")

    if roots["accept"]["principal_id"] == roots["reject"]["principal_id"]:
        raise AssertionError("portable divergence principals must be distinct")
    for name in ("accept", "reject"):
        device = flow["local_histories"][name]["synthetic_device"]
        device_hash = hashlib.sha256(device.encode("ascii")).hexdigest()
        if not roots[name]["principal_id"].startswith(
            f"local:device-sha256:{device_hash}|uid:"
        ):
            raise AssertionError(f"portable divergence {name} principal drift")
    if roots["accept"]["standing"] != "accepted":
        raise AssertionError("portable divergence accepted Standing drift")
    if roots["reject"]["standing"] != "unassessed":
        raise AssertionError("portable divergence rejected Standing drift")
    if roots["accept"]["repository_root"] == roots["reject"]["repository_root"]:
        raise AssertionError("portable divergence terminal roots must differ")

    expected = flow["expected"]
    if (
        expected["global_consensus_required"]
        or expected["standing_transports_between_repositories"]
    ):
        raise AssertionError("portable divergence must keep consensus and Standing local")


def main() -> int:
    check_flow_documents()
    check_computational_flow()
    check_correction_flow()
    check_formal_math_flow()
    check_portable_divergence_flow()
    print("reference-flows: 4 checked; authority effect none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
