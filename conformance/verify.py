#!/usr/bin/env python3
"""Run Vela's current, implementation-independent conformance checks."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFORMANCE = ROOT / "conformance"
sys.path.insert(0, str(CONFORMANCE / "readers" / "python"))
from canonical import canonical_bytes

DECISION_INBOX_ENTRY_SCHEMA = "vela.decision-inbox-entry.v3"
DECISION_INBOX_ENTRY_DOMAIN = b"vela.decision-inbox-entry.v3\0"
DECISION_INBOX_SCHEMA = "vela.decision-inbox.v3"
DECISION_INBOX_DOMAIN = b"vela.decision-inbox.v3\0"


def rooted_canonical_json(domain: bytes, value: dict[str, object], field: str) -> str:
    rooted = dict(value)
    rooted[field] = ""
    return "sha256:" + hashlib.sha256(domain + canonical_bytes(rooted)).hexdigest()


def validate_decision_inbox_read_surface(envelope: object) -> str | None:
    if not isinstance(envelope, dict):
        return "CLI envelope must be an object"
    if envelope.get("ok") is not True or envelope.get("command") != "review.inbox":
        return "CLI envelope metadata drift"
    if envelope.get("schema") != DECISION_INBOX_SCHEMA:
        return "unsupported projection schema"

    entries = envelope.get("entries")
    if not isinstance(entries, list):
        return "entries must be an array"
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            return f"entry {index} must be an object"
        if entry.get("schema") != DECISION_INBOX_ENTRY_SCHEMA:
            return f"entry {index} uses an unsupported schema"
        if entry.get("entry_root") != rooted_canonical_json(
            DECISION_INBOX_ENTRY_DOMAIN, entry, "entry_root"
        ):
            return f"entry {index} root drift"
        standing_delta = entry.get("standing_delta")
        if not isinstance(standing_delta, dict):
            return f"entry {index} standing delta must be an object"
        if not all(key in standing_delta for key in ("before", "if_accept", "if_reject")):
            return f"entry {index} omits a required hypothetical state"
        readiness = entry.get("readiness")
        if not isinstance(readiness, dict) or readiness.get("attributed_decision_required") is not True:
            return f"entry {index} must preserve the attributed Decision boundary"

    projection = {
        key: value for key, value in envelope.items() if key not in {"ok", "command"}
    }
    if envelope.get("projection_root") != rooted_canonical_json(
        DECISION_INBOX_DOMAIN, projection, "projection_root"
    ):
        return "projection root drift"
    return None


def verify_decision_inbox_read_surface() -> int:
    path = CONFORMANCE / "fixtures" / "read-surfaces" / "decision-inbox-v3.json"
    try:
        fixture = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"decision-inbox-v3: fixture load failed: {error}", file=sys.stderr)
        return 1

    error = validate_decision_inbox_read_surface(fixture)
    if error is not None:
        print(f"decision-inbox-v3: {error}", file=sys.stderr)
        return 1

    unsupported = dict(fixture)
    unsupported["schema"] = "vela.decision-inbox.v2"
    if validate_decision_inbox_read_surface(unsupported) is None:
        print("decision-inbox-v3: unsupported schema passed", file=sys.stderr)
        return 1

    tampered = json.loads(json.dumps(fixture))
    tampered["entries"][0]["standing_delta"]["transition"] = "forged transition"
    if validate_decision_inbox_read_surface(tampered) is None:
        print("decision-inbox-v3: rooted mutation passed", file=sys.stderr)
        return 1

    print("decision-inbox-v3: ok")
    return 0


def verify_sidon_witness(witness: object) -> bool:
    if not isinstance(witness, dict) or witness.get("kind") != "sidon":
        return False
    n = witness.get("n")
    points = witness.get("points")
    claimed_size = witness.get("claimed_size")
    if not isinstance(n, int) or n < 0 or not isinstance(points, list):
        return False
    if claimed_size is not None and claimed_size != len(points):
        return False
    normalized: list[tuple[int, ...]] = []
    for point in points:
        if (
            not isinstance(point, list)
            or len(point) != n
            or any(value not in (0, 1) for value in point)
        ):
            return False
        normalized.append(tuple(point))
    if len(set(normalized)) != len(normalized):
        return False
    sums: set[tuple[int, ...]] = set()
    for left_index, left in enumerate(normalized):
        for right in normalized[left_index:]:
            pair_sum = tuple(a + b for a, b in zip(left, right, strict=True))
            if pair_sum in sums:
                return False
            sums.add(pair_sum)
    return True


def claim_matches_witness(claim: object, witness: object) -> bool:
    if not isinstance(claim, str) or not verify_sidon_witness(witness):
        return False
    lowered = claim.lower()
    if "sidon" not in lowered or "exactly" in lowered or "maximum" in lowered:
        return False
    dimensions = re.findall(r"\{\s*0\s*,\s*1\s*\}\s*\^\s*(\d+)", lowered)
    bounds = re.findall(r"(?:at\s+least|>=)\s*(\d+)", lowered)
    return (
        len(dimensions) == 1
        and len(bounds) == 1
        and int(dimensions[0]) == witness.get("n")
        and int(bounds[0]) <= len(witness.get("points", []))
    )


def verify_exact_witness_floor() -> int:
    path = CONFORMANCE / "fixtures" / "exact-witness-floor.json"
    try:
        fixture = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"exact-witness-floor: fixture load failed: {error}", file=sys.stderr)
        return 1
    if (
        fixture.get("schema") != "vela.exact-witness-floor-fixture.v1"
        or fixture.get("artifact_kind") != "vela-witness"
        or fixture.get("replayability") != "exact"
    ):
        print("exact-witness-floor: contract metadata drift", file=sys.stderr)
        return 1
    witness = fixture.get("witness")
    expected_root = "sha256:" + hashlib.sha256(canonical_bytes(witness)).hexdigest()
    if fixture.get("witness_sha256") != expected_root:
        print("exact-witness-floor: witness root drift", file=sys.stderr)
        return 1
    if not verify_sidon_witness(witness):
        print("exact-witness-floor: intended witness failed", file=sys.stderr)
        return 1
    for case in fixture.get("claims", []):
        if claim_matches_witness(case.get("text"), witness) is not case.get("faithful"):
            print(
                f"exact-witness-floor: claim mismatch: {case.get('id')}",
                file=sys.stderr,
            )
            return 1
    if verify_sidon_witness(fixture.get("corrupted_witness")):
        print("exact-witness-floor: corrupt witness passed", file=sys.stderr)
        return 1
    print("exact-witness-floor: ok")
    return 0


def run_script(script: Path, *arguments: str) -> int:
    result = subprocess.run(
        [sys.executable, str(script), *arguments],
        cwd=ROOT,
        text=True,
        timeout=180,
        check=False,
    )
    return result.returncode


def run_check(script_name: str) -> int:
    return run_script(CONFORMANCE / script_name)


def main() -> int:
    checks = (
        "verify_protocol_1.py",
        "verify_canonical_hashing.py",
        "verify_current_objects.py",
        "verify_wire_schemas.py",
        "verify_correction_impact.py",
        "verify_claim_dependency_profile.py",
        "verify_authority_chain.py",
        "verify_reference_flows.py",
        "verify_release_reproducibility.py",
    )
    for script in checks:
        print(f"\n== {script.removeprefix('verify_').removesuffix('.py')} ==")
        result = run_check(script)
        if result != 0:
            print(f"vela conformance: FAIL ({script}, exit {result})", file=sys.stderr)
            return result
    print("\n== exact_witness_floor ==")
    if verify_exact_witness_floor() != 0:
        print("vela conformance: FAIL (exact-witness-floor)", file=sys.stderr)
        return 1
    print("\n== decision_inbox_v3 ==")
    if verify_decision_inbox_read_surface() != 0:
        print("vela conformance: FAIL (decision-inbox-v3)", file=sys.stderr)
        return 1
    print("\nvela conformance: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
