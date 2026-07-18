#!/usr/bin/env python3
"""
Vela conformance verifier.

Runs the canonical Python reducer (`clients/python/vela_reducer.py`)
against every fixture in `conformance/fixtures/` and reports per-fixture
pass/fail.

The Python reducer already implements the contract documented in
`conformance/README.md`: parse `(genesis_findings, event_log,
expected_states)`, apply per-kind mutation rules, build the effect-row
shape, assert deep equality with `expected_states`. This script is a
thin wrapper that exposes that contract as a public test runner an
external implementation can mirror.

Usage:
    ./verify.py
    ./verify.py --fixtures-dir <other-dir>
    ./verify.py --reducer-script <other-reducer.py>

Exit codes:
    0 = all fixtures pass
    1 = at least one fixture fails
    2 = invocation error
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path


def _check_manifest(fixtures_dir: Path) -> int:
    """v0.107.4: integrity preflight.

    Reads `fixtures.manifest.json` and verifies the SHA-256 of every
    listed fixture against its recorded digest. Refuses to run if any
    fixture has been tampered with. Closes THREAT_MODEL.md A12
    (integrity half; signed-manifest variant is a future cycle).

    Returns 0 if every fixture matches the manifest, 2 if the manifest
    is missing or any fixture's digest drifts. Skips with a one-line
    note when the manifest is absent (older fixture sets predate the
    manifest format and remain runnable; new sets ship with one).
    """
    manifest_path = fixtures_dir / "fixtures.manifest.json"
    if not manifest_path.is_file():
        print(
            f"  note: no fixtures.manifest.json at {manifest_path}; "
            f"skipping integrity preflight (older fixture set)"
        )
        return 0
    try:
        manifest = json.loads(manifest_path.read_text())
    except json.JSONDecodeError as e:
        print(f"  fail: fixtures.manifest.json is not valid JSON: {e}", file=sys.stderr)
        return 2
    if manifest.get("schema") != "vela.conformance-fixtures-manifest.v1":
        print(
            f"  fail: fixtures.manifest.json has wrong schema: "
            f"{manifest.get('schema')!r}",
            file=sys.stderr,
        )
        return 2
    drift = []
    for entry in manifest.get("fixtures", []):
        name = entry.get("path", "")
        expected_digest = entry.get("sha256", "")
        expected_bytes = entry.get("bytes", -1)
        path = fixtures_dir / name
        if not path.is_file():
            drift.append(f"{name}: missing on disk")
            continue
        bytes_on_disk = path.read_bytes()
        if len(bytes_on_disk) != expected_bytes:
            drift.append(
                f"{name}: size {len(bytes_on_disk)} != manifest {expected_bytes}"
            )
            continue
        actual_digest = "sha256:" + hashlib.sha256(bytes_on_disk).hexdigest()
        if actual_digest != expected_digest:
            drift.append(f"{name}: sha256 drift")
    if drift:
        print(
            "  fail: fixture integrity preflight detected drift:",
            file=sys.stderr,
        )
        for d in drift:
            print(f"    - {d}", file=sys.stderr)
        return 2
    print(f"  ok: integrity preflight ({len(manifest.get('fixtures', []))} fixtures)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Vela conformance verifier.")
    here = Path(__file__).resolve().parent
    repo_root = here.parent
    default_reducer = repo_root / "clients" / "python" / "vela_reducer.py"
    parser.add_argument(
        "--reducer-script",
        default=str(default_reducer),
        help="Python reducer script that implements the conformance contract",
    )
    parser.add_argument(
        "--fixtures-dir",
        default=str(here / "fixtures"),
        help="directory containing cascade-fixture-*.json",
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    if not fixtures_dir.is_dir():
        print(f"fixtures dir not found: {fixtures_dir}", file=sys.stderr)
        return 2

    reducer_script = Path(args.reducer_script)
    if not reducer_script.exists():
        print(f"reducer script not found: {reducer_script}", file=sys.stderr)
        return 2

    # v0.107.4: integrity preflight. Refuses to run if any fixture's
    # bytes drift from the recorded SHA-256 in fixtures.manifest.json.
    rc = _check_manifest(fixtures_dir)
    if rc != 0:
        return rc

    # Delegate to the canonical Python reducer's --json mode.
    cmd = [sys.executable, str(reducer_script), str(fixtures_dir), "--json"]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except Exception as e:
        print(f"failed to invoke reducer: {e}", file=sys.stderr)
        return 2

    if result.returncode not in (0, 1):
        print(f"reducer invocation failed (exit {result.returncode})", file=sys.stderr)
        if result.stderr.strip():
            print(result.stderr, file=sys.stderr)
        return 2

    try:
        report = json.loads(result.stdout)
    except Exception as e:
        print(f"failed to parse reducer output: {e}", file=sys.stderr)
        print(result.stdout, file=sys.stderr)
        return 2

    fixtures = report.get("fixtures", [])
    print(f"vela conformance · {len(fixtures)} fixtures")
    failed = 0
    for f in fixtures:
        ok = bool(f.get("ok"))
        status = "ok  " if ok else "FAIL"
        path = f.get("path", "?")
        # Compact summary line: counts + cascade depth.
        summary = (
            f"{f.get('findings', 0)}/{f.get('findings', 0)}"
            f" findings,"
            f" {f.get('events', 0)} events"
            f", cascade depth {f.get('cascade_depth', 0)}"
        )
        print(f"  {status}  {Path(path).name}  ·  {summary}")
        if not ok:
            failed += 1
            for diff in f.get("diffs", []):
                print(f"           ! {diff}")

    print()
    if failed == 0:
        print(f"vela conformance: ok ({len(fixtures)}/{len(fixtures)})  [python]")
    else:
        print(f"vela conformance: FAIL ({failed}/{len(fixtures)} failed)  [python]")
        return 1

    # Canonical-hashing conformance: the content-address form every `vev_`
    # id hashes. Pins the load-bearing Python re-verifier to the same vectors
    # the Rust id-minter is pinned to, so the two never drift apart. Runs in
    # Python-only environments too (no Node needed).
    ch_rc = _run_canonical_hashing(repo_root)
    if ch_rc != 0:
        print("vela conformance: FAIL  [canonical-hashing]")
        return 1
    print("vela conformance: ok  [canonical-hashing]")

    permit_rc = _run_permit_shadow(repo_root)
    if permit_rc != 0:
        print("vela conformance: FAIL  [permit-shadow]")
        return 1
    print("vela conformance: ok  [permit-shadow v0.1/v0.2]")

    # Second implementation: the TypeScript reducer. Gating it here is
    # what keeps it from silently drifting — an unrun reducer rots (the
    # retired `vela_reducer.mjs` fell three fixture_versions behind
    # precisely because nothing exercised it). Requires Node 23+ (native
    # TypeScript). If `node` is absent we warn and skip rather than fail,
    # so the suite still runs in Python-only environments.
    ts_rc = _run_ts_reducer(repo_root, fixtures_dir)
    if ts_rc == 2:
        print("  note: typescript reducer skipped (node not found); python-only run")
        return 0
    if ts_rc != 0:
        print("vela conformance: FAIL  [typescript]")
        return 1
    print("vela conformance: ok  [typescript]")
    print("\nvela conformance: ok — python + typescript agree with the rust reference")
    return 0


def _run_canonical_hashing(repo_root: Path) -> int:
    """Run the canonical-hashing conformance check (Python content-address path)."""
    script = repo_root / "conformance" / "verify_canonical_hashing.py"
    if not script.exists():
        print(f"  note: canonical-hashing check not found at {script}")
        return 0
    try:
        result = subprocess.run([sys.executable, str(script)], cwd=repo_root)
    except Exception as e:  # noqa: BLE001
        print(f"  canonical-hashing invocation failed: {e}", file=sys.stderr)
        return 1
    return result.returncode


def _canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def _policy_id(policy: dict) -> str:
    body = copy.deepcopy(policy)
    body["id"] = ""
    if body.get("revocation_ref") is None:
        body.pop("revocation_ref", None)
    return "vap_" + hashlib.sha256(_canonical_bytes(body)).hexdigest()[:32]


def _full_root(value: object) -> bool:
    if not isinstance(value, str) or not value.startswith("sha256:"):
        return False
    digest = value[7:]
    return len(digest) == 64 and all(char in "0123456789abcdef" for char in digest)


def _evaluate_permit_shadow(policy: dict, context: dict) -> str:
    schema = policy.get("schema")
    if schema not in {"vela.acceptance_policy.v0.1", "vela.acceptance_policy.v0.2"}:
        return "deny"
    if policy.get("id") != _policy_id(policy) or policy.get("default") == "permit":
        return "deny"
    for rule in policy.get("rules", []):
        if rule.get("effect") != "permit":
            continue
        if context.get("claim_class") not in rule.get("claim_classes", []):
            continue
        constraints = rule.get("constraints", {})
        v2_names = [
            "allowed_packet_roots",
            "allowed_profile_roots",
            "allowed_verifier_capsule_roots",
            "allowed_result_contract_roots",
            "required_replayability",
        ]
        if schema == "vela.acceptance_policy.v0.1" and any(
            name in constraints for name in v2_names
        ):
            return "deny"
        blocked = []
        if context.get("has_unknown_fields") or not context.get("credential_valid"):
            blocked.append("unknown_or_invalid")
        if context.get("assurance_level", 0) < constraints.get(
            "required_assurance_min", 4
        ):
            blocked.append("assurance")
        if context.get("changed_findings", 2**32 - 1) > constraints.get(
            "max_changed_findings", 0
        ):
            blocked.append("changed")
        if context.get("downstream_dependents", 2**32 - 1) > constraints.get(
            "max_downstream_dependents", 0
        ):
            blocked.append("downstream")
        if context.get("assertion_text_mutated") and not constraints.get(
            "allow_semantic_text_change", False
        ):
            blocked.append("text")
        if context.get("target_contested") and not constraints.get(
            "allow_contested", False
        ):
            blocked.append("contested")
        if context.get("governance_mutation") and not constraints.get(
            "allow_governance_mutation", False
        ):
            blocked.append("governance")
        if constraints.get("require_independence") and not context.get(
            "independence_satisfied"
        ):
            blocked.append("independence")
        if constraints.get("require_method_integrity") and not context.get(
            "method_integrity_sound"
        ):
            blocked.append("method")
        if schema == "vela.acceptance_policy.v0.2":
            allowlists = [
                ("packet_root", "allowed_packet_roots"),
                ("profile_root", "allowed_profile_roots"),
                ("verifier_capsule_root", "allowed_verifier_capsule_roots"),
                ("result_contract_root", "allowed_result_contract_roots"),
            ]
            for _, allowlist_name in allowlists:
                allowlist = constraints.get(allowlist_name)
                if (
                    not isinstance(allowlist, list)
                    or not allowlist
                    or not all(_full_root(root) for root in allowlist)
                ):
                    return "deny"
            if constraints.get("required_replayability") != "exact":
                return "deny"
            binding = context.get("execution_binding")
            if not isinstance(binding, dict) or binding.get("schema") != "vela.execution-binding.v1":
                blocked.append("binding")
            else:
                for field, allowlist_name in allowlists:
                    actual = binding.get(field)
                    if not _full_root(actual) or actual not in constraints[allowlist_name]:
                        blocked.append(field)
                if context.get("replayability") != constraints["required_replayability"]:
                    blocked.append("replayability")
        return "defer" if blocked else "permit"
    return policy.get("default", "deny")


def _run_permit_shadow(repo_root: Path) -> int:
    path = repo_root / "conformance" / "fixtures" / "permit-shadow-v1.json"
    try:
        fixture = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"  permit-shadow fixture load failed: {error}", file=sys.stderr)
        return 1
    if fixture.get("schema") != "vela.permit-shadow-experiment.v1":
        print("  permit-shadow fixture schema mismatch", file=sys.stderr)
        return 1
    cases = fixture.get("cases", [])
    if len(cases) != 3:
        print("  permit-shadow fixture must contain exactly three cases", file=sys.stderr)
        return 1
    for case in cases:
        for field, preimage in case.get("root_preimages", {}).items():
            expected = "sha256:" + hashlib.sha256(preimage.encode("utf-8")).hexdigest()
            if case.get("binding", {}).get(field) != expected:
                print(f"  {case.get('id')}: {field} content root drift", file=sys.stderr)
                return 1
    digests = {
        "sha256:" + hashlib.sha256(_canonical_bytes(case["policy_context"])).hexdigest()
        for case in cases
    }
    expected_digest = "sha256:05f4c43817a4301da40e476393639ba042756f593d48582718135e92b653c7ac"
    if digests != {expected_digest}:
        print("  permit-shadow v0.1 context digest drift", file=sys.stderr)
        return 1
    policy_v1 = copy.deepcopy(fixture["policy"])
    policy_v1["id"] = _policy_id(policy_v1)
    if any(
        _evaluate_permit_shadow(policy_v1, case["policy_context"])
        != case["expected_v0_1"]
        for case in cases
    ):
        print("  permit-shadow v0.1 outcome mismatch", file=sys.stderr)
        return 1
    intended = cases[0]["binding"]
    policy_v2 = copy.deepcopy(policy_v1)
    policy_v2["schema"] = "vela.acceptance_policy.v0.2"
    constraints = policy_v2["rules"][0]["constraints"]
    constraints.update(
        {
            "allowed_packet_roots": [intended["packet_root"]],
            "allowed_profile_roots": [intended["profile_root"]],
            "allowed_verifier_capsule_roots": [intended["verifier_capsule_root"]],
            "allowed_result_contract_roots": [intended["result_contract_root"]],
            "required_replayability": "exact",
        }
    )
    policy_v2["id"] = _policy_id(policy_v2)
    for case in cases:
        context = copy.deepcopy(case["policy_context"])
        context["execution_binding"] = case["binding"]
        if _evaluate_permit_shadow(policy_v2, context) != case["expected_v0_2"]:
            print(f"  {case.get('id')}: v0.2 outcome mismatch", file=sys.stderr)
            return 1
    if _evaluate_permit_shadow(policy_v2, cases[0]["policy_context"]) != "defer":
        print("  permit-shadow missing v0.2 binding did not defer", file=sys.stderr)
        return 1
    return 0


def _run_ts_reducer(repo_root: Path, fixtures_dir: Path) -> int:
    """Run the TypeScript reducer over the fixtures. Returns 0 (ok),
    1 (mismatch/error), or 2 (node unavailable → skip)."""
    ts_reducer = repo_root / "clients" / "typescript" / "vela_reducer.ts"
    if not ts_reducer.exists():
        print(f"  note: typescript reducer not found at {ts_reducer}")
        return 2
    try:
        result = subprocess.run(
            ["node", str(ts_reducer), str(fixtures_dir)],
            capture_output=True,
            text=True,
            timeout=120,
        )
    except FileNotFoundError:
        return 2
    except Exception as e:  # noqa: BLE001
        print(f"  typescript reducer invocation failed: {e}", file=sys.stderr)
        return 1
    if result.returncode != 0:
        if result.stdout.strip():
            print(result.stdout)
        if result.stderr.strip():
            print(result.stderr, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
