#!/usr/bin/env python3
"""Fail-closed check for the authority-free first-party handoff rehearsal."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REGISTRATION = ROOT / "registration/first-party-handoff-rehearsal-v1.json"
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_PHASES = [
    "parent_generate",
    "verifier_v1",
    "verifier_v2",
    "pending_handoff",
    "fixture_authorized_child",
    "correction_replay",
    "reader_parity",
    "standards_baseline",
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def strict_json(path: Path) -> dict[str, Any]:
    def no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            require(key not in result, f"duplicate name {key!r} in {path}")
            result[key] = value
        return result

    value = json.loads(path.read_bytes(), object_pairs_hook=no_duplicates)
    require(isinstance(value, dict), "registration must be one JSON object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def file_root(path: Path) -> str:
    return f"sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"


def main() -> None:
    registration = strict_json(REGISTRATION)
    require(
        registration.get("schema") == "vela.first-party-handoff-rehearsal.v1",
        "registration schema drift",
    )
    require(
        registration.get("run_class") == "first_party_internal_fixture",
        "run class widened",
    )
    require(
        registration.get("status") == "registered_not_run",
        "registration already records execution",
    )
    release = registration.get("release", {})
    require(release.get("tag") == "v0.800.22", "released Vela tag drift")
    require(
        release.get("commit") == "a5e5631d8fceb6a9a28522b7b9799adb74b9f232",
        "released Vela commit drift",
    )
    require(bool(COMMIT.fullmatch(registration.get("base_commit", ""))), "bad base commit")
    require(
        registration.get("supersedes_registration_root")
        == "sha256:55b14e2e1bbc6a66f476dcd99c14402381c2bc1e40671005338bfaf3a2a1f68d",
        "superseded pre-execution root drift",
    )
    require(
        all(
            SHA256.fullmatch(release.get(field, ""))
            for field in ("macos_aarch64_sha256", "linux_x86_64_sha256")
        ),
        "released binary root drift",
    )
    require(registration.get("phases") == EXPECTED_PHASES, "phase order drift")
    tools = registration.get("tools", {})
    require(
        tools.get("cadical", {}).get("version") == "3.0.0"
        and tools.get("drat_trim", {}).get("source_commit")
        == "2e3b2dc0ecf938addbd779d42877b6ed69d9a985"
        and tools.get("lrat_check", {}).get("source_commit")
        == "2e3b2dc0ecf938addbd779d42877b6ed69d9a985",
        "certificate toolchain drift",
    )
    require(
        all(
            SHA256.fullmatch(tools.get(label, {}).get("sha256", ""))
            for label in ("cadical", "drat_trim", "lrat_check")
        ),
        "certificate executable root drift",
    )
    require(
        registration.get("authority", {})
        == {
            "human_key_access": False,
            "signing_allowed": False,
            "accepted_state_claim_allowed": False,
            "fixture_authority_must_be_labeled_simulated": True,
        },
        "authority boundary widened",
    )
    require(
        registration.get("credit", {})
        == {
            "scientific": False,
            "human_authority": False,
            "independent": False,
            "external": False,
            "causal": False,
            "protocol_promotion": False,
        },
        "credit boundary widened",
    )
    require(
        registration.get("result_root") is None,
        "registration must not contain a result root before execution",
    )
    files = registration.get("files")
    require(isinstance(files, list) and files, "registered files missing")
    paths = [item.get("path") for item in files if isinstance(item, dict)]
    require(len(paths) == len(set(paths)), "duplicate registered path")
    for item in files:
        require(isinstance(item, dict), "registered file entry must be an object")
        path = ROOT / item.get("path", "")
        require(path.is_file() and not path.is_symlink(), f"missing regular file {path}")
        require(item.get("sha256") == file_root(path), f"file root drift: {path}")
    stop_conditions = registration.get("stop_conditions")
    require(
        isinstance(stop_conditions, list)
        and {
            "human_key_exposure",
            "authority_attempt",
            "historical_event_rewrite",
            "false_strict_pass",
            "parent_substitution",
            "reader_disagreement",
            "standards_fact_drift",
            "unregistered_repair",
        }.issubset(stop_conditions),
        "stop conditions weakened",
    )
    root_input = dict(registration)
    recorded_root = root_input.pop("registration_root", None)
    computed_root = f"sha256:{hashlib.sha256(canonical_bytes(root_input)).hexdigest()}"
    require(recorded_root == computed_root, "registration root drift")
    print(computed_root)


if __name__ == "__main__":
    main()
