#!/usr/bin/env python3
"""Deterministically regenerate the held candidate's dependent roots."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

PACKAGE = Path(__file__).resolve().parent
REGISTRATION = PACKAGE / "registration.json"
TOOL_POLICY = PACKAGE / "tool-policy.json"
ARTIFACT_ROOT = PACKAGE / "artifact-root.json"


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(canonical_bytes(value))


def pretty_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def main() -> None:
    registration = json.loads(REGISTRATION.read_bytes())
    tool_root = digest(TOOL_POLICY.read_bytes())
    boundary_root = canonical_root(
        {
            "stage_a_directory_tree": registration["stage_a_binding"][
                "pilot_directory_tree"
            ],
            "response_schema_sha256": registration["provider_schema_derivation"][
                "authoritative_schema_sha256"
            ],
            "tool_policy_sha256": tool_root,
        }
    )
    for configuration in registration["participant_configurations"]:
        configuration["tool_policy_sha256"] = tool_root
        configuration["information_boundary_root"] = boundary_root
        body = dict(configuration)
        body.pop("configuration_root", None)
        configuration["configuration_root"] = canonical_root(body)
    REGISTRATION.write_bytes(pretty_bytes(registration))

    entries = []
    for path in sorted(PACKAGE.iterdir()):
        if path.is_file() and path.name != ARTIFACT_ROOT.name:
            raw = path.read_bytes()
            entries.append(
                {"path": path.name, "bytes": len(raw), "sha256": digest(raw)}
            )
    artifact = {
        "schema": "vela.lean-correspondence-stage-a-runtime-qualification-artifact-root.v1",
        "entries": entries,
    }
    artifact["artifact_root"] = canonical_root(artifact)
    ARTIFACT_ROOT.write_bytes(pretty_bytes(artifact))


if __name__ == "__main__":
    main()
