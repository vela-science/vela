#!/usr/bin/env python3
"""Actual offline bundle verify/fetch and continuity drill for ADR 0004."""

from __future__ import annotations

import copy
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REFERENCE = ROOT / "reference"
sys.path.insert(0, str(REFERENCE))
sys.path.insert(0, str(ROOT))

from check_fact_manifest_projections import base_manifest  # noqa: E402
from fact_manifest import (  # noqa: E402
    build_envelope,
    canonical_bytes,
    resolve_envelope,
    sha256_root,
)
from offline_bundle_inspection import (  # noqa: E402
    INSPECTION_STATE_SCHEMA,
    InspectionError,
    event_content_root,
    event_log_root,
    inspect_bundle,
)


STATE_PATH = "inspection/state.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def root(label: str) -> str:
    return f"sha256:{hashlib.sha256(label.encode()).hexdigest()}"


def event(label: str, *, effect: str | None = None) -> dict[str, Any]:
    payload: dict[str, Any] = {"label": label}
    if effect is not None:
        payload["dependency_effect"] = effect
    value = {
        "schema": "vela.event.v0.1",
        "id": "",
        "kind": "finding.revised" if effect else "review.accepted",
        "target": {"type": "finding", "id": "vf_1111111111111111"},
        "actor": {"type": "human", "id": "reviewer:fixture"},
        "timestamp": f"2026-07-16T00:01:{len(label):02d}Z",
        "reason": label,
        "before_hash": root(f"{label}:before"),
        "after_hash": root(f"{label}:after"),
        "payload": payload,
        "caveats": ["internal offline bundle fixture"],
        "signature": "v1:" + hashlib.sha512(label.encode()).hexdigest(),
    }
    preimage = {
        key: value[key]
        for key in (
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
    value["id"] = f"vev_{sha256_root(canonical_bytes(preimage))[7:23]}"
    require(event_content_root(value)[7:23] == value["id"][4:], "event id drift")
    return value


def run(command: list[str], *, cwd: Path | None = None) -> str:
    environment = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": "/nonexistent",
        "LANG": "C",
        "LC_ALL": "C",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0",
    }
    result = subprocess.run(
        command,
        cwd=cwd,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed {command!r}: {result.stderr.strip()}")
    return result.stdout.strip()


def write_state(
    repository: Path,
    *,
    snapshot: dict[str, Any],
    events: list[dict[str, Any]],
) -> None:
    path = repository / STATE_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        canonical_bytes(
            {
                "schema": INSPECTION_STATE_SCHEMA,
                "snapshot": snapshot,
                "events": events,
            }
        )
        + b"\n"
    )


def commit(repository: Path, message: str) -> str:
    run(["git", "add", "."], cwd=repository)
    run(
        [
            "git",
            "-c",
            "user.name=ADR4 Bundle Fixture",
            "-c",
            "user.email=adr4@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
        cwd=repository,
    )
    return run(["git", "rev-parse", "HEAD"], cwd=repository)


def state_from_inspection(result: dict[str, Any], prefix: str) -> dict[str, str]:
    return {
        "git_commit": result[f"{prefix}_git_commit"],
        "git_tree": result[f"{prefix}_git_tree"],
        "event_log_root": event_log_root(result[f"{prefix}_events"]),
        "snapshot_root": sha256_root(canonical_bytes(result[f"{prefix}_snapshot"])),
    }


def manifest_for(
    inspection: dict[str, Any],
    *,
    effect: str | None,
) -> dict[str, Any]:
    manifest = base_manifest()
    result = inspection["result"]
    manifest["delivery_inspection"] = copy.deepcopy(inspection)
    manifest["last_seen"] = state_from_inspection(result, "last_seen")
    manifest["delivered"] = state_from_inspection(result, "delivered")
    dependency = manifest["dependency"]
    dependency["parent_git_commit"] = manifest["last_seen"]["git_commit"]
    dependency["parent_git_tree"] = manifest["last_seen"]["git_tree"]
    dependency["parent_event_log_root"] = manifest["last_seen"]["event_log_root"]
    dependency["parent_snapshot_root"] = manifest["last_seen"]["snapshot_root"]
    accepted = result["last_seen_events"][0]
    dependency["decision_event_id"] = accepted["id"]
    dependency["decision_event_content_root"] = event_content_root(accepted)
    dependency["decision_signature"] = accepted["signature"]
    manifest["standing"]["decision_event_content_root"] = dependency[
        "decision_event_content_root"
    ]
    if effect is not None:
        change = result["delivered_events"][-1]
        manifest["standing"]["finding_status"] = effect
        manifest["standing"]["change_event"] = {
            "event_id": change["id"],
            "event_content_root": event_content_root(change),
            "event_signature": change["signature"],
            "authority_id": change["actor"]["id"],
            "effect": effect,
            "inspection_result_root": inspection["inspection_root"],
        }
    return manifest


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="vela-adr4-bundle-drill-") as raw:
        temporary = Path(raw)
        repository = temporary / "producer"
        repository.mkdir()
        run(["git", "init", "-q", "-b", "main"], cwd=repository)

        accepted = event("accepted")
        base_snapshot = {"accepted": ["vf_1111111111111111"], "revision": 1}
        write_state(repository, snapshot=base_snapshot, events=[accepted])
        base = commit(repository, "base accepted state")

        correction = event("correction", effect="corrected")
        corrected_snapshot = {
            "accepted": ["vf_1111111111111111"],
            "revision": 2,
        }
        write_state(
            repository,
            snapshot=corrected_snapshot,
            events=[accepted, correction],
        )
        descendant = commit(repository, "authorized correction")

        run(["git", "checkout", "-q", "-b", "fork", base], cwd=repository)
        fork_event = event("fork-note")
        write_state(
            repository,
            snapshot={"accepted": ["fork"], "revision": 2},
            events=[accepted, fork_event],
        )
        fork = commit(repository, "valid non-descendant fork")

        run(["git", "checkout", "-q", "-b", "bad-prefix", base], cwd=repository)
        write_state(
            repository,
            snapshot={"accepted": ["bad"], "revision": 2},
            events=[event("replacement-without-prefix")],
        )
        bad_prefix = commit(repository, "invalid event-history replacement")

        bundle = temporary / "frontier.bundle"
        run(["git", "bundle", "create", str(bundle), "--branches"], cwd=repository)

        same = inspect_bundle(
            bundle,
            last_seen_commit=base,
            delivered_commit=base,
            state_path=STATE_PATH,
        )
        later = inspect_bundle(
            bundle,
            last_seen_commit=base,
            delivered_commit=descendant,
            state_path=STATE_PATH,
        )
        stale = inspect_bundle(
            bundle,
            last_seen_commit=descendant,
            delivered_commit=base,
            state_path=STATE_PATH,
        )
        forked = inspect_bundle(
            bundle,
            last_seen_commit=descendant,
            delivered_commit=fork,
            state_path=STATE_PATH,
        )
        require(same["result"]["git_relation"] == "same", "same relation drift")
        require(
            later["result"]["git_relation"] == "descendant"
            and later["result"]["event_relation"] == "descendant",
            "descendant continuity drift",
        )
        require(
            stale["result"]["git_relation"] == "ancestor"
            and stale["result"]["event_relation"] == "ancestor",
            "stale continuity drift",
        )
        require(
            forked["result"]["git_relation"] == "forked"
            and forked["result"]["event_relation"] == "forked",
            "fork continuity drift",
        )

        cases = (
            (same, None, "satisfied"),
            (later, "corrected", "review_required"),
            (stale, None, "stale"),
            (forked, None, "forked"),
        )
        for inspection, effect, expected in cases:
            manifest = manifest_for(inspection, effect=effect)
            result = resolve_envelope(build_envelope(manifest))
            require(
                result["dependency_status"] == expected,
                f"{expected}: resolver produced {result['dependency_status']}",
            )
            require(
                result["delivery_inspection_root"] == inspection["inspection_root"],
                f"{expected}: inspection root drift",
            )

        try:
            inspect_bundle(
                bundle,
                last_seen_commit=base,
                delivered_commit=bad_prefix,
                state_path=STATE_PATH,
            )
        except InspectionError as error:
            require(
                error.code == "mismatch:git_event_continuity",
                f"bad prefix failed as {error.code}",
            )
        else:
            raise RuntimeError("non-prefix descendant event history was accepted")

        corrupt = temporary / "corrupt.bundle"
        raw_bundle = bytearray(bundle.read_bytes())
        raw_bundle[len(raw_bundle) // 2] ^= 0x01
        corrupt.write_bytes(raw_bundle)
        try:
            inspect_bundle(
                corrupt,
                last_seen_commit=base,
                delivered_commit=base,
                state_path=STATE_PATH,
            )
        except InspectionError as error:
            require(
                error.code == "git:command_failed",
                f"corrupt bundle failed as {error.code}",
            )
        else:
            raise RuntimeError("corrupt bundle was accepted")

    print(
        "offline bundle inspection: verify/fetch passed; "
        "same/descendant/stale/fork derived; event prefix and corrupt bundle refused"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
