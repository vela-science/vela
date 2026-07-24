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
import base64
import copy
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from datetime import datetime
from pathlib import Path


AUTHORITY_HISTORY_FIXTURE_ROOT = (
    "sha256:d0f09ebfa3025bb453346b1cb02989ae75c772748180995c052ee62a50bdb16e"
)
AUTHORITY_RECORD_PAYLOAD_TYPE = "application/vnd.vela.authority-record.v1+json"
EVENT_PAYLOAD_TYPE = "application/vnd.vela.event+json"


def _check_manifest(fixtures_dir: Path) -> int:
    """v0.107.4: integrity preflight.

    Reads `fixtures.manifest.json`, requires it to name exactly the
    cascade fixtures present on disk, and verifies the size and SHA-256
    of every fixture against its recorded values. Refuses to run if the
    manifest is malformed, incomplete, duplicated, or any fixture has
    been tampered with. Closes THREAT_MODEL.md A12 (integrity half;
    signed-manifest variant is a future cycle).

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
    if not isinstance(manifest, dict):
        print(
            "  fail: fixtures.manifest.json must be a JSON object",
            file=sys.stderr,
        )
        return 2
    if manifest.get("schema") != "vela.conformance-fixtures-manifest.v1":
        print(
            f"  fail: fixtures.manifest.json has wrong schema: "
            f"{manifest.get('schema')!r}",
            file=sys.stderr,
        )
        return 2

    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list):
        print(
            "  fail: fixtures.manifest.json fixtures must be an array",
            file=sys.stderr,
        )
        return 2

    required_entry_keys = {"path", "sha256", "bytes"}
    fixture_name_pattern = re.compile(
        r"cascade-fixture-[A-Za-z0-9][A-Za-z0-9._-]*\.json"
    )
    manifest_names: list[str] = []
    for index, entry in enumerate(fixtures):
        if not isinstance(entry, dict) or set(entry) != required_entry_keys:
            print(
                "  fail: fixtures.manifest.json entry "
                f"{index} must contain exactly path, sha256, and bytes",
                file=sys.stderr,
            )
            return 2

        name = entry["path"]
        expected_digest = entry["sha256"]
        expected_bytes = entry["bytes"]
        if (
            not isinstance(name, str)
            or Path(name).name != name
            or fixture_name_pattern.fullmatch(name) is None
        ):
            print(
                "  fail: fixtures.manifest.json entry "
                f"{index} path must be a cascade-fixture-*.json basename",
                file=sys.stderr,
            )
            return 2
        if (
            not isinstance(expected_digest, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", expected_digest) is None
        ):
            print(
                "  fail: fixtures.manifest.json entry "
                f"{index} sha256 must be a lowercase full SHA-256 digest",
                file=sys.stderr,
            )
            return 2
        if (
            not isinstance(expected_bytes, int)
            or isinstance(expected_bytes, bool)
            or expected_bytes < 0
        ):
            print(
                "  fail: fixtures.manifest.json entry "
                f"{index} bytes must be a non-negative integer",
                file=sys.stderr,
            )
            return 2
        manifest_names.append(name)

    duplicate_names = sorted(
        name for name in set(manifest_names) if manifest_names.count(name) > 1
    )
    if duplicate_names:
        print(
            "  fail: fixtures.manifest.json contains duplicate fixture paths: "
            + ", ".join(duplicate_names),
            file=sys.stderr,
        )
        return 2

    disk_names = {
        path.name
        for path in fixtures_dir.glob("cascade-fixture-*.json")
        if path.is_file()
    }
    manifest_name_set = set(manifest_names)
    if manifest_name_set != disk_names:
        missing_from_manifest = sorted(disk_names - manifest_name_set)
        missing_on_disk = sorted(manifest_name_set - disk_names)
        print(
            "  fail: fixtures.manifest.json does not exactly match cascade fixtures "
            "on disk:",
            file=sys.stderr,
        )
        for name in missing_from_manifest:
            print(f"    - {name}: present on disk but absent from manifest", file=sys.stderr)
        for name in missing_on_disk:
            print(f"    - {name}: listed in manifest but absent from disk", file=sys.stderr)
        return 2

    drift = []
    for entry in fixtures:
        name = entry["path"]
        expected_digest = entry["sha256"]
        expected_bytes = entry["bytes"]
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
    print(f"  ok: integrity preflight ({len(fixtures)} fixtures)")
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
    parser.add_argument(
        "--authority-history-only",
        action="store_true",
        help="verify only the authority migration cross-implementation fixture",
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    if not fixtures_dir.is_dir():
        print(f"fixtures dir not found: {fixtures_dir}", file=sys.stderr)
        return 2

    if args.authority_history_only:
        authority_rc = _run_authority_history_migration(repo_root)
        if authority_rc != 0:
            print("vela conformance: FAIL  [authority-history migration]")
            return authority_rc
        print("vela conformance: ok  [authority-history migration]")
        return 0

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

    credential_rc = _run_policy_scoped_credential(repo_root)
    if credential_rc != 0:
        print("vela conformance: FAIL  [policy-scoped-credential]")
        return 1
    print("vela conformance: ok  [policy-scoped-credential v0.2/v0.3]")

    legacy_shadow_rc = _run_legacy_policy_shadow_corpus(repo_root)
    if legacy_shadow_rc != 0:
        print("vela conformance: FAIL  [legacy-policy-shadow-corpus]")
        return 1
    print("vela conformance: ok  [legacy-policy-shadow corpus metadata]")

    authority_rc = _run_authority_history_migration(repo_root)
    if authority_rc != 0:
        print("vela conformance: FAIL  [authority-history migration]")
        return 1
    print("vela conformance: ok  [authority-history migration]")

    principal_rc = _run_principal_capability(repo_root)
    if principal_rc != 0:
        print("vela conformance: FAIL  [principal-capability v1]")
        return 1
    print("vela conformance: ok  [principal-capability v1]")

    floor_rc = _run_exact_witness_floor(repo_root)
    if floor_rc != 0:
        print("vela conformance: FAIL  [exact-witness-floor]")
        return 1
    print("vela conformance: ok  [exact-witness-floor v0.2]")

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


def _run_principal_capability(repo_root: Path) -> int:
    script = repo_root / "conformance" / "verify_principal_capability.py"
    try:
        result = subprocess.run([sys.executable, str(script)], cwd=repo_root)
    except Exception as error:  # noqa: BLE001
        print(
            f"  principal-capability invocation failed: {error}",
            file=sys.stderr,
        )
        return 1
    return result.returncode


def _canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def _sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def _sha256_canonical(value: object) -> str:
    return _sha256_bytes(_canonical_bytes(value))


def _require_exact_keys(value: dict, expected: set[str], subject: str) -> None:
    if set(value) != expected:
        raise ValueError(
            f"{subject} keys differ: expected {sorted(expected)}, got {sorted(value)}"
        )


def _require_root(value: object, subject: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None:
        raise ValueError(f"{subject} is not a full lowercase SHA-256 root")
    return value


def _dsse_pae(payload_type: str, payload: bytes) -> bytes:
    encoded_type = payload_type.encode("utf-8")
    return (
        b"DSSEv1 "
        + str(len(encoded_type)).encode("ascii")
        + b" "
        + encoded_type
        + b" "
        + str(len(payload)).encode("ascii")
        + b" "
        + payload
    )


def _verify_ed25519(
    public_key_hex: str, message: bytes, signature: bytes, subject: str
) -> None:
    if re.fullmatch(r"[0-9a-f]{64}", public_key_hex) is None:
        raise ValueError(f"{subject} public key is not 32-byte lowercase hex")
    if len(signature) != 64:
        raise ValueError(f"{subject} signature is not exactly 64 bytes")

    # RFC 8410 SubjectPublicKeyInfo prefix for a raw Ed25519 public key.
    public_der = bytes.fromhex("302a300506032b6570032100") + bytes.fromhex(
        public_key_hex
    )
    with tempfile.TemporaryDirectory(prefix="vela-ed25519-") as temporary:
        directory = Path(temporary)
        key_path = directory / "key.der"
        message_path = directory / "message.bin"
        signature_path = directory / "signature.bin"
        key_path.write_bytes(public_der)
        message_path.write_bytes(message)
        signature_path.write_bytes(signature)
        try:
            result = subprocess.run(
                [
                    "openssl",
                    "pkeyutl",
                    "-verify",
                    "-pubin",
                    "-keyform",
                    "DER",
                    "-inkey",
                    str(key_path),
                    "-rawin",
                    "-in",
                    str(message_path),
                    "-sigfile",
                    str(signature_path),
                ],
                capture_output=True,
                text=True,
                timeout=10,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ValueError(f"{subject} could not invoke OpenSSL: {error}") from error
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip() or "verification failed"
            raise ValueError(f"{subject} Ed25519 verification failed: {detail}")


def _legacy_event_id(event: dict) -> str:
    content = {
        field: event[field]
        for field in (
            "schema",
            "kind",
            "target",
            "actor",
            "timestamp",
            "reason",
            "before_hash",
            "after_hash",
            "payload",
            "caveats",
        )
    }
    return "vev_" + hashlib.sha256(_canonical_bytes(content)).hexdigest()[:16]


def _legacy_event_log_root(events: list[dict]) -> str:
    stripped = []
    for event in sorted(events, key=lambda item: item["id"]):
        content = copy.deepcopy(event)
        content.pop("signature", None)
        stripped.append(content)
    return _sha256_canonical(stripped)


def _verify_legacy_bridge(
    event: dict, registry_bytes: bytes, migration: dict, frontier_id: str
) -> None:
    required_event = {
        "schema",
        "id",
        "kind",
        "target",
        "actor",
        "timestamp",
        "reason",
        "before_hash",
        "after_hash",
        "payload",
        "caveats",
        "signature",
    }
    _require_exact_keys(event, required_event, "migration event")
    if (
        event["id"] != _legacy_event_id(event)
        or event["kind"] != "authority.model_migrated"
        or event["target"] != {"type": "frontier", "id": frontier_id}
        or event["actor"].get("type") != "human"
        or event["before_hash"] != "sha256:null"
        or event["after_hash"] != "sha256:null"
        or event["reason"] != migration["reason"]
    ):
        raise ValueError("migration event shape or content address is invalid")

    migration_keys = {
        "schema",
        "frontier_id",
        "legacy_event_log_root",
        "legacy_actor_registry_root",
        "legacy_active_policy_head_root",
        "legacy_policy_store_manifest_root",
        "new_authority_keyset_root",
        "new_policy_bundle_root",
        "new_principal_id",
        "minimum_writer_version",
        "reason",
    }
    _require_exact_keys(migration, migration_keys, "migration payload")
    if (
        migration["schema"] != "vela.authority-model-migration.v1"
        or migration["frontier_id"] != frontier_id
        or not migration["new_principal_id"]
        or not migration["minimum_writer_version"]
        or not migration["reason"]
    ):
        raise ValueError("migration payload identity is invalid")
    for field in migration_keys - {
        "schema",
        "frontier_id",
        "new_principal_id",
        "minimum_writer_version",
        "reason",
    }:
        _require_root(migration[field], f"migration {field}")
    if migration["legacy_actor_registry_root"] != _sha256_bytes(registry_bytes):
        raise ValueError("migration actor-registry root does not match exact bytes")

    try:
        registry = json.loads(registry_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"legacy actor registry is invalid: {error}") from error
    if not isinstance(registry, list):
        raise ValueError("legacy actor registry must be an array")
    matching = [actor for actor in registry if actor.get("id") == event["actor"]["id"]]
    if len(matching) != 1:
        raise ValueError("migration signer is not unique in the actor registry")
    actor = matching[0]
    if actor.get("algorithm") != "ed25519":
        raise ValueError("migration signer algorithm is not Ed25519")
    revoked_at = actor.get("revoked_at")
    if revoked_at is not None and revoked_at <= event["timestamp"]:
        raise ValueError("migration signer was revoked at the bridge time")

    signature_text = event["signature"]
    if not isinstance(signature_text, str) or re.fullmatch(
        r"v1:[0-9a-f]{128}", signature_text
    ) is None:
        raise ValueError("migration signature is not a v1 Ed25519 signature")
    signing_content = {
        field: event[field]
        for field in (
            "schema",
            "id",
            "kind",
            "target",
            "actor",
            "timestamp",
            "reason",
            "before_hash",
            "after_hash",
            "payload",
            "caveats",
        )
    }
    _verify_ed25519(
        actor["public_key"],
        _dsse_pae(EVENT_PAYLOAD_TYPE, _canonical_bytes(signing_content)),
        bytes.fromhex(signature_text.removeprefix("v1:")),
        "migration bridge",
    )


def _authority_event_root(event: dict) -> str:
    _require_exact_keys(event, {"schema", "id", "content"}, "Era-1 event")
    if (
        event["schema"] != "vela.event.v1"
        or event["id"]
        != "vev_" + hashlib.sha256(_canonical_bytes(event["content"])).hexdigest()[:16]
    ):
        raise ValueError("Era-1 event schema or content address is invalid")
    content = event["content"]
    if (
        not content.get("transaction_id")
        or not content.get("principal_id")
        or content.get("authority_mode") != "repository_authority"
        or content.get("actor", {}).get("id") != content.get("principal_id")
    ):
        raise ValueError("Era-1 event attribution or transaction is invalid")
    _require_root(content.get("before_hash"), "Era-1 before_hash")
    _require_root(content.get("after_hash"), "Era-1 after_hash")
    return _sha256_canonical(event)


def _authority_event_log_root(
    legacy_root_with_bridge: str, authority_events: list[dict]
) -> str:
    roots = sorted(
        ((event["id"], _authority_event_root(event)) for event in authority_events),
        key=lambda item: item[0],
    )
    return _sha256_canonical(
        {
            "schema": "vela.authority-event-log.v1",
            "legacy_event_log_root": legacy_root_with_bridge,
            "authority_event_roots": [root for _, root in roots],
        }
    )


def _decode_and_verify_authority_envelope(
    envelope: dict,
    keyset: dict,
    frontier_id: str,
    sequence: int,
    previous_root: str | None,
) -> tuple[dict, str]:
    _require_exact_keys(
        envelope, {"payloadType", "payload", "signatures"}, "authority envelope"
    )
    if envelope["payloadType"] != AUTHORITY_RECORD_PAYLOAD_TYPE:
        raise ValueError("authority envelope payload type is invalid")
    try:
        payload = base64.b64decode(envelope["payload"], validate=True)
        record = json.loads(payload)
    except (ValueError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"authority envelope payload is invalid: {error}") from error
    if _canonical_bytes(record) != payload:
        raise ValueError("authority record payload is not canonical JSON")
    _require_exact_keys(record, {"schema", "record_id", "content"}, "authority record")
    content = record["content"]
    if (
        record["schema"] != "vela.authority-record.v1"
        or record["record_id"]
        != "var_" + hashlib.sha256(_canonical_bytes(content)).hexdigest()[:16]
        or content.get("frontier_id") != frontier_id
        or content.get("sequence") != sequence
        or content.get("previous_authority_record_root") != previous_root
        or content.get("authority_keyset_root") != _sha256_canonical(keyset)
    ):
        raise ValueError("authority record identity or chain position is invalid")

    signatures = envelope["signatures"]
    if not isinstance(signatures, list) or not signatures:
        raise ValueError("authority envelope has no signatures")
    keys = {key["key_id"]: key for key in keyset["keys"]}
    verified: set[str] = set()
    pae = _dsse_pae(envelope["payloadType"], payload)
    for signed in signatures:
        _require_exact_keys(signed, {"keyid", "sig"}, "DSSE signature")
        key_id = signed["keyid"]
        if key_id in verified or key_id not in keys:
            raise ValueError("authority envelope has duplicate or unknown signature key")
        key = keys[key_id]
        if (
            key.get("algorithm") != "ed25519"
            or key.get("purpose") != "repository_authority"
            or sequence < key.get("valid_from_sequence", 0)
            or (
                key.get("valid_through_sequence") is not None
                and sequence > key["valid_through_sequence"]
            )
        ):
            raise ValueError("authority key is invalid or outside its sequence window")
        try:
            signature = base64.b64decode(signed["sig"], validate=True)
        except ValueError as error:
            raise ValueError("authority signature is not canonical base64") from error
        _verify_ed25519(key["public_key"], pae, signature, f"authority record {sequence}")
        verified.add(key_id)
    if len(verified) < keyset["threshold"]:
        raise ValueError("authority signature threshold was not met")
    return record, _sha256_canonical(record)


def _verify_pinned_authorization(authorization: dict, bundle_root: str) -> None:
    evaluation = authorization["evaluation"]
    if (
        authorization["policy_bundle_root"] != bundle_root
        or evaluation
        != {
            "engine": "cedar-policy",
            "engine_version": "4.11.2",
            "profile": "vela.cedar-restricted.v1",
            "valid": True,
            "decision": "allow",
            "automatic_permit": False,
            "determining_policies": ["permit_repository_admin"],
            "diagnostics": [],
        }
    ):
        raise ValueError("authority record lacks a clean pinned Cedar authorization")


def _verify_authentication_observation(
    authentication: dict,
    principal: dict,
    *,
    revoked_session_roots: set[str] | None = None,
) -> None:
    _require_exact_keys(
        authentication,
        {
            "schema",
            "principal_id",
            "principal_class",
            "issuer",
            "subject",
            "method",
            "assurance",
            "session_root",
            "authenticated_at",
            "observed_at",
            "expires_at",
            "user_presence",
            "user_verification",
            "recovery_recent",
            "revocation_ref",
        },
        "authentication observation",
    )
    if (
        authentication["schema"] != "vela.authentication-observation.v1"
        or authentication["principal_id"] != principal["principal_id"]
        or authentication["principal_class"] != principal["principal_class"]
        or authentication["principal_id"]
        not in {
            f"local:{authentication['issuer']}|{authentication['subject']}",
            f"oidc:{authentication['issuer']}|{authentication['subject']}",
            f"orcid:{authentication['issuer']}|{authentication['subject']}",
        }
        or authentication["principal_id"] not in principal["account_links"]
        or authentication["method"] not in {"local_os_session", "passkey", "oidc"}
    ):
        raise ValueError("authentication observation does not bind its human principal")
    _require_root(authentication["session_root"], "authentication session root")
    if authentication["revocation_ref"] is not None:
        _require_root(authentication["revocation_ref"], "authentication revocation ref")
    if authentication["method"] == "passkey" and (
        authentication["assurance"] != "phishing_resistant"
        or authentication["user_presence"] is not True
        or authentication["user_verification"] is not True
    ):
        raise ValueError("passkey observation lacks verified user presence")
    try:
        authenticated_at = datetime.fromisoformat(
            authentication["authenticated_at"].removesuffix("Z") + "+00:00"
        )
        observed_at = datetime.fromisoformat(
            authentication["observed_at"].removesuffix("Z") + "+00:00"
        )
        expires_at = datetime.fromisoformat(
            authentication["expires_at"].removesuffix("Z") + "+00:00"
        )
    except (AttributeError, ValueError) as error:
        raise ValueError("authentication time is invalid") from error
    if (
        observed_at < authenticated_at
        or observed_at >= expires_at
        or (expires_at - authenticated_at).total_seconds() > 24 * 60 * 60
    ):
        raise ValueError("authentication observation is stale or exceeds 24 hours")
    if authentication["session_root"] in (revoked_session_roots or set()):
        raise ValueError("authentication session was revoked before use")


def _verify_authority_history_fixture(fixture: dict) -> dict:
    fixture_keys = {
        "schema",
        "frontier_id",
        "legacy_events",
        "legacy_actor_registry_base64",
        "legacy_active_policy_head_root",
        "legacy_policy_store_manifest_root",
        "authority_keyset",
        "policy_bundle",
        "authority_events",
        "authority_envelopes",
        "expected",
        "fixture_root",
    }
    _require_exact_keys(fixture, fixture_keys, "authority-history fixture")
    supplied_root = fixture["fixture_root"]
    root_body = copy.deepcopy(fixture)
    root_body.pop("fixture_root")
    if supplied_root != _sha256_canonical(root_body):
        raise ValueError("authority-history fixture root does not match its bytes")

    frontier_id = fixture["frontier_id"]
    if (
        fixture["schema"] != "vela.authority-history-conformance.v1"
        or not isinstance(frontier_id, str)
        or not frontier_id.startswith("vfr_")
    ):
        raise ValueError("authority-history fixture identity is invalid")

    legacy_events = fixture["legacy_events"]
    if not isinstance(legacy_events, list) or not legacy_events:
        raise ValueError("authority-history fixture has no legacy history")
    seen_legacy = set()
    for event in legacy_events:
        if event["id"] != _legacy_event_id(event) or event["id"] in seen_legacy:
            raise ValueError("legacy event has an invalid or duplicate content address")
        seen_legacy.add(event["id"])
    migrations = [
        event for event in legacy_events if event["kind"] == "authority.model_migrated"
    ]
    if len(migrations) != 1:
        raise ValueError("authority history must contain exactly one migration bridge")
    migration_event = migrations[0]
    migration = migration_event["payload"]
    legacy_prefix = [event for event in legacy_events if event["id"] != migration_event["id"]]
    legacy_prefix_root = _legacy_event_log_root(legacy_prefix)
    if migration["legacy_event_log_root"] != legacy_prefix_root:
        raise ValueError("migration does not bind the exact pre-migration history")
    if (
        migration["legacy_active_policy_head_root"]
        != fixture["legacy_active_policy_head_root"]
        or migration["legacy_policy_store_manifest_root"]
        != fixture["legacy_policy_store_manifest_root"]
    ):
        raise ValueError("migration does not bind the supplied legacy policy state")
    try:
        registry_bytes = base64.b64decode(
            fixture["legacy_actor_registry_base64"], validate=True
        )
    except ValueError as error:
        raise ValueError("legacy actor registry is not canonical base64") from error
    _verify_legacy_bridge(migration_event, registry_bytes, migration, frontier_id)

    keyset = fixture["authority_keyset"]
    bundle = fixture["policy_bundle"]
    _require_exact_keys(
        keyset,
        {
            "schema",
            "frontier_id",
            "generation",
            "previous_keyset_root",
            "activation_record_root",
            "threshold",
            "keys",
        },
        "authority keyset",
    )
    if (
        keyset["schema"] != "vela.authority-keyset.v1"
        or keyset["frontier_id"] != frontier_id
        or not isinstance(keyset["threshold"], int)
        or keyset["threshold"] < 1
        or keyset["threshold"] > len(keyset["keys"])
    ):
        raise ValueError("authority keyset is invalid")
    key_ids = [key["key_id"] for key in keyset["keys"]]
    if len(key_ids) != len(set(key_ids)):
        raise ValueError("authority keyset has duplicate key IDs")
    _require_exact_keys(
        bundle,
        {
            "schema",
            "frontier_id",
            "previous_bundle_root",
            "engine",
            "engine_version",
            "restricted_profile",
            "cedar_schema_root",
            "policies_root",
            "entities_root",
            "tests_root",
            "authority_summary",
        },
        "policy bundle",
    )
    if (
        bundle["schema"] != "vela.policy-bundle.v1"
        or bundle["frontier_id"] != frontier_id
        or bundle["engine"] != "cedar-policy"
        or bundle["engine_version"] != "4.11.2"
        or bundle["restricted_profile"] != "vela.cedar-restricted.v1"
    ):
        raise ValueError("policy bundle identity is invalid")
    for field in ("cedar_schema_root", "policies_root", "entities_root", "tests_root"):
        _require_root(bundle[field], f"policy bundle {field}")
    keyset_root = _sha256_canonical(keyset)
    bundle_root = _sha256_canonical(bundle)
    if (
        migration["new_authority_keyset_root"] != keyset_root
        or migration["new_policy_bundle_root"] != bundle_root
    ):
        raise ValueError("migration does not bind the supplied Era-1 authority inputs")

    authority_events = fixture["authority_events"]
    event_by_id: dict[str, dict] = {}
    transaction_ids: dict[str, set[str]] = {}
    for event in authority_events:
        _authority_event_root(event)
        if event["id"] in seen_legacy or event["id"] in event_by_id:
            raise ValueError("authority event identity is duplicated")
        event_by_id[event["id"]] = event
        transaction_ids.setdefault(event["content"]["transaction_id"], set()).add(event["id"])

    legacy_root_with_bridge = _legacy_event_log_root(legacy_events)
    current_event_root = legacy_prefix_root
    previous_record_root = None
    cumulative_events: list[dict] = []
    covered: set[str] = set()
    final_record_root = None
    for sequence, envelope in enumerate(fixture["authority_envelopes"], start=1):
        record, record_root = _decode_and_verify_authority_envelope(
            envelope, keyset, frontier_id, sequence, previous_record_root
        )
        content = record["content"]
        _verify_authentication_observation(
            content["authentication"], content["principal"]
        )
        authorization = content["authorization"]
        _verify_pinned_authorization(authorization, bundle_root)
        if content["before_event_log_root"] != current_event_root:
            raise ValueError("authority record before-event root is invalid")
        event_ids = content["event_ids"]
        if len(event_ids) != len(set(event_ids)):
            raise ValueError("authority record repeats an event ID")

        if sequence == 1:
            if (
                content["transaction_id"] == ""
                or event_ids != [migration_event["id"]]
                or content["after_event_log_root"] != legacy_root_with_bridge
                or content["principal"]["principal_id"] != migration["new_principal_id"]
            ):
                raise ValueError("authority record 1 does not exactly cover the migration")
            approvals = content["semantic_approvals"]
            if not any(
                approval["principal_id"] == migration_event["actor"]["id"]
                and approval["action"] == "authority_model_migrate"
                and approval["reason"] == migration["reason"]
                and approval["intent_digest"] == content["intent_digest"]
                for approval in approvals
            ):
                raise ValueError("authority record 1 lacks the exact semantic approval")
            expected_objects = {migration_event["id"]: _sha256_canonical(migration_event)}
            current_event_root = legacy_root_with_bridge
        else:
            transaction_id = content["transaction_id"]
            expected_ids = transaction_ids.get(transaction_id)
            if expected_ids is None or set(event_ids) != expected_ids:
                raise ValueError("authority record does not exactly cover its transaction")
            if covered.intersection(event_ids):
                raise ValueError("authority event is covered more than once")
            for event_id in event_ids:
                event = event_by_id[event_id]
                if (
                    event["content"]["principal_id"]
                    != content["principal"]["principal_id"]
                ):
                    raise ValueError("authority event principal does not match its record")
                cumulative_events.append(event)
                covered.add(event_id)
            expected_after = _authority_event_log_root(
                legacy_root_with_bridge, cumulative_events
            )
            if content["after_event_log_root"] != expected_after:
                raise ValueError("authority record after-event root is invalid")
            expected_objects = {
                event_id: _authority_event_root(event_by_id[event_id])
                for event_id in event_ids
            }
            current_event_root = expected_after

        deltas = content["object_delta"]
        if len(deltas) != len(expected_objects):
            raise ValueError("authority record object delta has unexpected entries")
        for event_id, event_root in expected_objects.items():
            expected_delta = {
                "path": (
                    f".vela/events/{event_id}.json"
                    if sequence == 1
                    else f".vela/authority/events/{event_id}.json"
                ),
                "before_root": None,
                "after_root": event_root,
                "object_kind": "event",
            }
            if deltas.count(expected_delta) != 1:
                raise ValueError("authority record lacks one exact event object delta")
        previous_record_root = record_root
        final_record_root = record_root

    if covered != set(event_by_id):
        raise ValueError("Era-1 event history lacks unique authority-record coverage")
    result = {
        "era": "repository_authority",
        "frontier_id": frontier_id,
        "legacy_event_count": len(legacy_events),
        "authority_event_count": len(authority_events),
        "authority_record_count": len(fixture["authority_envelopes"]),
        "migration_event_id": migration_event["id"],
        "final_event_log_root": current_event_root,
        "final_authority_record_root": final_record_root,
    }
    if result != fixture["expected"]:
        raise ValueError("independently derived authority-history report differs")
    return result


def _reroot_authority_fixture(fixture: dict) -> None:
    body = copy.deepcopy(fixture)
    body.pop("fixture_root", None)
    fixture["fixture_root"] = _sha256_canonical(body)


def _run_authority_history_migration(repo_root: Path) -> int:
    fixture_path = (
        repo_root / "conformance" / "fixtures" / "authority-history-migration-v1.json"
    )
    try:
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        if fixture.get("fixture_root") != AUTHORITY_HISTORY_FIXTURE_ROOT:
            raise ValueError(
                "authority-history fixture root is not the independently pinned root"
            )
        result = _verify_authority_history_fixture(fixture)

        retained_authentication = json.loads(
            base64.b64decode(fixture["authority_envelopes"][0]["payload"])
        )["content"]
        authentication_hostiles = []
        bearer = copy.deepcopy(retained_authentication)
        bearer["authentication"]["bearer_token"] = "must-not-enter-history"
        authentication_hostiles.append(("bearer authentication retention", bearer))
        identity = copy.deepcopy(retained_authentication)
        identity["authentication"]["principal_id"] = "fixture@example.com"
        authentication_hostiles.append(("authentication identity substitution", identity))
        stale = copy.deepcopy(retained_authentication)
        stale["authentication"]["expires_at"] = "2026-07-26T12:00:00Z"
        authentication_hostiles.append(("stale authentication", stale))
        for name, content in authentication_hostiles:
            try:
                _verify_authentication_observation(
                    content["authentication"], content["principal"]
                )
            except ValueError:
                continue
            raise ValueError(f"hostile case unexpectedly verified: {name}")
        try:
            _verify_authentication_observation(
                retained_authentication["authentication"],
                retained_authentication["principal"],
                revoked_session_roots={
                    retained_authentication["authentication"]["session_root"]
                },
            )
        except ValueError:
            pass
        else:
            raise ValueError("revoked authentication unexpectedly verified")

        hostile: list[tuple[str, dict]] = []
        legacy_write = copy.deepcopy(fixture)
        later = copy.deepcopy(legacy_write["legacy_events"][0])
        later["timestamp"] = "2026-07-24T12:02:00Z"
        later["reason"] = "Illegitimate legacy write after migration."
        later["id"] = _legacy_event_id(later)
        legacy_write["legacy_events"].append(later)
        _reroot_authority_fixture(legacy_write)
        hostile.append(("post-migration legacy write", legacy_write))

        missing_record = copy.deepcopy(fixture)
        missing_record["authority_envelopes"].pop()
        _reroot_authority_fixture(missing_record)
        hostile.append(("missing transaction coverage", missing_record))

        transaction_substitution = copy.deepcopy(fixture)
        transaction_substitution["authority_events"][0]["content"][
            "transaction_id"
        ] = "txn_substituted"
        transaction_substitution["authority_events"][0]["id"] = (
            "vev_"
            + hashlib.sha256(
                _canonical_bytes(transaction_substitution["authority_events"][0]["content"])
            ).hexdigest()[:16]
        )
        _reroot_authority_fixture(transaction_substitution)
        hostile.append(("transaction substitution", transaction_substitution))

        signature_tamper = copy.deepcopy(fixture)
        signature = signature_tamper["authority_envelopes"][1]["signatures"][0]["sig"]
        signature_tamper["authority_envelopes"][1]["signatures"][0]["sig"] = (
            ("A" if signature[0] != "A" else "B") + signature[1:]
        )
        _reroot_authority_fixture(signature_tamper)
        hostile.append(("DSSE signature tamper", signature_tamper))

        bundle_substitution = copy.deepcopy(fixture)
        bundle_substitution["policy_bundle"]["policies_root"] = "sha256:" + "1" * 64
        _reroot_authority_fixture(bundle_substitution)
        hostile.append(("policy bundle substitution", bundle_substitution))

        diagnostics = copy.deepcopy(fixture)
        payload = base64.b64decode(diagnostics["authority_envelopes"][1]["payload"])
        record = json.loads(payload)
        record["content"]["authorization"]["evaluation"]["diagnostics"] = [
            "hostile diagnostic"
        ]
        try:
            _verify_pinned_authorization(
                record["content"]["authorization"],
                _sha256_canonical(diagnostics["policy_bundle"]),
            )
        except ValueError:
            pass
        else:
            raise ValueError("Cedar diagnostics unexpectedly passed authorization")
        record["record_id"] = (
            "var_"
            + hashlib.sha256(_canonical_bytes(record["content"])).hexdigest()[:16]
        )
        diagnostics["authority_envelopes"][1]["payload"] = base64.b64encode(
            _canonical_bytes(record)
        ).decode("ascii")
        _reroot_authority_fixture(diagnostics)
        hostile.append(("Cedar diagnostics", diagnostics))

        for name, candidate in hostile:
            try:
                _verify_authority_history_fixture(candidate)
            except ValueError:
                continue
            raise ValueError(f"hostile case unexpectedly verified: {name}")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"  authority-history migration check failed: {error}", file=sys.stderr)
        return 1
    print(
        "  ok: authority-history fixture "
        f"{fixture['fixture_root']} "
        f"({result['legacy_event_count']} legacy, "
        f"{result['authority_event_count']} Era-1, "
        f"{result['authority_record_count']} records; "
        f"{len(hostile) + 4} hostile cases)"
    )
    return 0


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
    if schema not in {
        "vela.acceptance_policy.v0.1",
        "vela.acceptance_policy.v0.2",
        "vela.acceptance_policy.v0.3",
    }:
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
        v3_names = ["allowed_producer_credential_roots"]
        if schema == "vela.acceptance_policy.v0.1" and any(
            name in constraints for name in v2_names + v3_names
        ):
            return "deny"
        if schema == "vela.acceptance_policy.v0.2" and any(
            name in constraints for name in v3_names
        ):
            return "deny"
        blocked = []
        if context.get("has_unknown_fields"):
            blocked.append("unknown")
        if schema == "vela.acceptance_policy.v0.3":
            credentials = constraints.get("allowed_producer_credential_roots")
            if (
                not isinstance(credentials, list)
                or len(credentials) != 1
                or not all(_full_root(root) for root in credentials)
                or len(set(credentials)) != len(credentials)
            ):
                return "deny"
            if context.get("producer_credential_root") not in credentials:
                blocked.append("producer_credential")
        elif not context.get("credential_valid"):
            blocked.append("credential")
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
        if schema in {"vela.acceptance_policy.v0.2", "vela.acceptance_policy.v0.3"}:
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
                if schema == "vela.acceptance_policy.v0.3" and len(
                    set(allowlist)
                ) != len(allowlist):
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


def _run_policy_scoped_credential(repo_root: Path) -> int:
    path = (
        repo_root
        / "conformance"
        / "fixtures"
        / "policy-scoped-producer-credential-v1.json"
    )
    try:
        fixture = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"  policy-scoped credential fixture load failed: {error}", file=sys.stderr)
        return 1
    if fixture.get("schema") != "vela.policy-scoped-producer-credential-fixture.v1":
        print("  policy-scoped credential fixture schema mismatch", file=sys.stderr)
        return 1
    binding = copy.deepcopy(fixture.get("identity_binding"))
    if not isinstance(binding, dict):
        print("  policy-scoped identity binding is missing", file=sys.stderr)
        return 1
    declared_id = binding.get("binding_id")
    binding["binding_id"] = ""
    binding["signature"] = ""
    derived = "sha256:" + hashlib.sha256(_canonical_bytes(binding)).hexdigest()
    if derived != fixture.get("producer_credential_root"):
        print("  policy-scoped credential root drift", file=sys.stderr)
        return 1
    if declared_id != "vib_" + derived[7:23]:
        print("  policy-scoped credential handle drift", file=sys.stderr)
        return 1

    v2 = copy.deepcopy(fixture["policy"])
    v2["id"] = _policy_id(v2)
    v3 = copy.deepcopy(v2)
    v3["schema"] = "vela.acceptance_policy.v0.3"
    v3["rules"][0]["constraints"]["allowed_producer_credential_roots"] = [derived]
    v3["id"] = _policy_id(v3)
    for case in fixture.get("cases", []):
        v2_context = copy.deepcopy(fixture["context"])
        v2_context["credential_valid"] = case["credential_valid"]
        if _evaluate_permit_shadow(v2, v2_context) != case["expected_v0_2"]:
            print(f"  {case.get('id')}: v0.2 credential outcome mismatch", file=sys.stderr)
            return 1
        v3_context = copy.deepcopy(v2_context)
        credential = case.get("producer_credential_root")
        if credential is not None:
            v3_context["producer_credential_root"] = credential
        if _evaluate_permit_shadow(v3, v3_context) != case["expected_v0_3"]:
            print(f"  {case.get('id')}: v0.3 credential outcome mismatch", file=sys.stderr)
            return 1
    duplicate = copy.deepcopy(v3)
    duplicate["rules"][0]["constraints"]["allowed_producer_credential_roots"] = [
        derived,
        derived,
    ]
    duplicate["id"] = _policy_id(duplicate)
    if _evaluate_permit_shadow(duplicate, fixture["context"]) != "deny":
        print("  duplicate producer credentials did not fail closed", file=sys.stderr)
        return 1
    return 0


def _run_legacy_policy_shadow_corpus(repo_root: Path) -> int:
    path = (
        repo_root
        / "conformance"
        / "fixtures"
        / "legacy-policy-shadow-corpus-v1.json"
    )
    try:
        raw = path.read_bytes()
        corpus = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        print(f"  legacy policy shadow corpus load failed: {error}", file=sys.stderr)
        return 1
    if corpus.get("schema") != "vela.legacy-policy-shadow-corpus.v1":
        print("  legacy policy shadow corpus schema mismatch", file=sys.stderr)
        return 1
    canonical_root = "sha256:" + hashlib.sha256(_canonical_bytes(corpus)).hexdigest()
    if canonical_root != "sha256:9bf9c5a770d427221b68e088492b243041440e7e05408b0ae0af0a65d37c45d9":
        print("  legacy policy shadow corpus root drift", file=sys.stderr)
        return 1
    if not _full_root(corpus.get("tests_root")):
        print("  legacy policy shadow tests root is invalid", file=sys.stderr)
        return 1
    cases = corpus.get("cases")
    if not isinstance(cases, list) or len(cases) != 4:
        print("  legacy policy shadow corpus must contain four cases", file=sys.stderr)
        return 1
    case_ids = [case.get("id") for case in cases]
    if any(not isinstance(case_id, str) or not case_id for case_id in case_ids):
        print("  legacy policy shadow case ID is missing", file=sys.stderr)
        return 1
    if len(case_ids) != len(set(case_ids)):
        print("  legacy policy shadow case IDs are duplicated", file=sys.stderr)
        return 1
    source_pattern = re.compile(
        r"^vela-science/[a-z0-9-]+@[0-9a-f]{40}:"
        r"[^#]+#sha256:[0-9a-f]{64}$"
    )
    for case in cases:
        if not source_pattern.fullmatch(case.get("source", "")):
            print(f"  {case.get('id')}: source binding is invalid", file=sys.stderr)
            return 1
        observed = _evaluate_permit_shadow(case["policy"], case["context"])
        expected = case.get("expected_legacy_outcome")
        if observed != expected:
            print(
                f"  {case.get('id')}: expected legacy {expected}, observed {observed}",
                file=sys.stderr,
            )
            return 1
    return 0


def _verify_sidon_witness(witness: object) -> bool:
    """Independent exact check for the small public Sidon floor vector."""
    if not isinstance(witness, dict) or witness.get("kind") != "sidon":
        return False
    n = witness.get("n")
    points = witness.get("points")
    claimed_size = witness.get("claimed_size")
    if not isinstance(n, int) or n < 0 or not isinstance(points, list):
        return False
    if claimed_size is not None and claimed_size != len(points):
        return False
    normalized = []
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


def _sidon_claim_faithful(claim: object, witness: object) -> bool:
    """Independent claim check for the exact public Sidon floor vector."""
    if not isinstance(claim, str) or not _verify_sidon_witness(witness):
        return False
    lowered = claim.lower()
    if "sidon" not in lowered or "exactly" in lowered or "maximum" in lowered:
        return False
    dimensions = re.findall(r"\{\s*0\s*,\s*1\s*\}\s*\^\s*(\d+)", lowered)
    bounds = re.findall(r"(?:at\s+least|>=)\s*(\d+)", lowered)
    if len(dimensions) != 1 or len(bounds) != 1:
        return False
    return int(dimensions[0]) == witness.get("n") and int(bounds[0]) <= len(
        witness.get("points", [])
    )


def _run_exact_witness_floor(repo_root: Path) -> int:
    path = repo_root / "conformance" / "fixtures" / "exact-witness-floor-v1.json"
    try:
        fixture = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"  exact-witness-floor fixture load failed: {error}", file=sys.stderr)
        return 1
    if fixture.get("schema") != "vela.exact-witness-floor-fixture.v1":
        print("  exact-witness-floor fixture schema mismatch", file=sys.stderr)
        return 1
    if fixture.get("artifact_kind") != "vela-witness":
        print("  exact-witness-floor artifact kind drift", file=sys.stderr)
        return 1
    if fixture.get("replayability") != "exact":
        print("  exact-witness-floor replayability drift", file=sys.stderr)
        return 1
    witness = fixture.get("witness")
    expected_root = "sha256:" + hashlib.sha256(_canonical_bytes(witness)).hexdigest()
    if fixture.get("witness_sha256") != expected_root:
        print("  exact-witness-floor witness root drift", file=sys.stderr)
        return 1
    if not _verify_sidon_witness(witness):
        print("  exact-witness-floor intended witness did not verify", file=sys.stderr)
        return 1
    for case in fixture.get("claims", []):
        actual = _sidon_claim_faithful(case.get("text"), witness)
        if actual is not case.get("faithful"):
            print(
                f"  exact-witness-floor claim mismatch: {case.get('id')}",
                file=sys.stderr,
            )
            return 1
    if _verify_sidon_witness(fixture.get("corrupted_witness")):
        print("  exact-witness-floor corrupted witness passed", file=sys.stderr)
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
