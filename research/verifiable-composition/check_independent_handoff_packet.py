#!/usr/bin/env python3
"""Check the key-free ADR 0006 independent participant packet."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REGISTRATION = ROOT / "registration" / "independent-handoff-v1.json"
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")


class PacketError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PacketError(message)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def root(value: Any) -> str:
    return f"sha256:{hashlib.sha256(canonical_bytes(value)).hexdigest()}"


def file_root(path: Path) -> str:
    require(path.is_file() and not path.is_symlink(), f"not a regular file: {path}")
    return f"sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"


def main() -> int:
    registration = read_json(REGISTRATION)
    require(
        set(registration)
        == {
            "schema",
            "status",
            "release",
            "graph_case",
            "profiles",
            "participant_files",
            "roles",
            "support",
            "stop_conditions",
            "credit",
        },
        "registration fields drift",
    )
    require(
        registration["schema"] == "vela.independent-handoff-registration.v1",
        "registration schema drift",
    )
    require(
        registration["status"] == "blocked_until_participants_named",
        "packet must stay blocked until role declarations are bound",
    )

    release = registration["release"]
    require(release["tag"] == "v0.800.22", "release tag drift")
    require(
        release["commit"] == "a5e5631d8fceb6a9a28522b7b9799adb74b9f232",
        "release commit drift",
    )
    require(
        release["tree"] == "b97b8a92de5b05abc34de600b10c69680737cdc2",
        "release tree drift",
    )
    require(
        release["macos_aarch64_sha256"]
        == "sha256:08703dfe5193755a0a2feaafe34576f68c2769377f428e5cc7a779418b7958b9",
        "release binary hash drift",
    )
    require(
        release["linux_x86_64_sha256"]
        == "sha256:1a1bbd4fa37c1a3931f96f93d00cbe64db0e3749de585aa8da47a82cdffd6603",
        "Linux release binary hash drift",
    )

    graph = registration["graph_case"]
    graph_path = ROOT / graph["path"]
    graph_value = read_json(graph_path)
    require(file_root(graph_path) == graph["file_sha256"], "graph file drift")
    require(root(graph_value) == graph["canonical_root"], "graph registration root drift")
    graph_bytes = {
        "edges": graph_value["graph_a"]["edges"],
        "vertices": graph_value["graph_a"]["vertices"],
    }
    require(
        root(graph_bytes) == graph["canonical_graph_root"],
        "canonical graph root drift",
    )
    require(graph_value["graph_a"]["vertices"] == 11, "graph vertex count drift")
    require(len(graph_value["graph_a"]["edges"]) == 20, "graph edge count drift")
    require(
        graph_value["child_task"]["registered_properties"]["vertices"] == 23,
        "child vertex count drift",
    )

    for group in ("profiles", "participant_files"):
        for item in registration[group]:
            path = ROOT / item["path"]
            require(SHA256.fullmatch(item["sha256"]) is not None, f"bad hash: {path}")
            require(file_root(path) == item["sha256"], f"registered file drift: {path}")

    expected_roles = {
        "producer_a",
        "verifier_v1",
        "verifier_v2",
        "human_steward",
        "producer_b",
        "reader_c",
        "red_team",
        "baseline_team",
    }
    require(set(registration["roles"]) == expected_roles, "role set drift")
    require(
        set(registration["roles"].values()) == {"unassigned"},
        "participant assignment requires a new registration",
    )

    support = registration["support"]
    require(
        support["semantic_maintainer_interventions_allowed"] == 0,
        "semantic intervention must remain forbidden",
    )
    require(
        support["transport_retry_requires_no_usable_output"] is True,
        "transport retry rule drift",
    )

    stops = set(registration["stop_conditions"])
    require(
        {
            "human_key_exposure",
            "historical_event_rewrite",
            "false_strict_pass",
            "semantic_maintainer_hint",
            "mismatched_arm_fact_set",
            "false_independent_credit",
            "reader_disagreement",
            "scorer_nonreproduction",
        }
        <= stops,
        "required stop condition missing",
    )
    require(
        all(value is False for value in registration["credit"].values()),
        "unrun packet cannot claim credit",
    )

    registration_root = root(registration)
    print(
        "independent handoff packet: "
        f"blocked, files verified, registration root {registration_root}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
