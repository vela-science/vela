#!/usr/bin/env python3
"""Check the three Protocol 1 reference flows without network or authority."""

from __future__ import annotations

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
    for name in ("formal-math", "computational-science", "correction-inheritance"):
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
    recomputed = run([sys.executable, str(directory / "experiment.py"), "--check", str(result)])
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
    if authority["terminal"]["repository_manifest_root"] != flow["real_terminal_repository_root"]:
        raise AssertionError("real correction terminal root drift")
    if len(authority["terminal"]["accepted_claims"]) != flow["real_accepted_claim_count"]:
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
    if flow["repository_commit"] != "08a0e6d327e1ae9937ab2e0e5002192815eac69a":
        raise AssertionError("formal-math flow no longer pins the current compact lineage")
    if flow["repository_root"] != "sha256:3e2236510923277c1e363d2d28c3d84d86a1d698bafd576b79308b18ae0cf0d2":
        raise AssertionError("formal-math repository root drift")
    if flow["accepted_claim_count"] != 2:
        raise AssertionError("formal-math current accepted count drift")
    if flow["evidence_artifact_root"] != "sha256:789c9dc5e4c1c234450a7ebd03d7b4fb8e0ba6deab12098e2fb17b3e74bada10":
        raise AssertionError("formal-math correction evidence root drift")


def main() -> int:
    check_flow_documents()
    check_computational_flow()
    check_correction_flow()
    check_formal_math_flow()
    print("reference-flows: 3 checked; authority effect none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
