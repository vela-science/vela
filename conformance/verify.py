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


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


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
    path = CONFORMANCE / "fixtures" / "exact-witness-floor-v1.json"
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


def run_check(script_name: str) -> int:
    script = CONFORMANCE / script_name
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=ROOT,
        text=True,
        timeout=120,
    )
    return result.returncode


def main() -> int:
    checks = (
        "verify_canonical_hashing.py",
        "verify_current_objects.py",
        "verify_correction_impact.py",
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
    print("\nvela conformance: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
