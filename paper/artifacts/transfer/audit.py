#!/usr/bin/env python3
"""Audit the frozen current Vela source for a non-escalating foreign transfer contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def struct_fields(source: str, name: str) -> list[str]:
    match = re.search(
        rf"pub struct {re.escape(name)} \{{(?P<body>.*?)\n\}}",
        source,
        re.DOTALL,
    )
    require(match is not None, f"{name} is missing")
    return re.findall(r"pub ([a-z_]+):", match.group("body"))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    repo = args.repo.resolve()
    here = Path(__file__).resolve().parent
    plan_path = here / "plan.json"
    try:
        plan_bytes = plan_path.read_bytes()
        plan = json.loads(plan_bytes)
        commit = git(repo, "rev-parse", "HEAD")
        tree = git(repo, "rev-parse", "HEAD^{tree}")
        require(commit == plan["source"]["commit"], "source commit mismatch")
        require(tree == plan["source"]["tree"], "source tree mismatch")

        sources: dict[str, str] = {}
        for surface in plan["audited_surfaces"]:
            encoded = (repo / surface["path"]).read_bytes()
            require(sha256(encoded) == surface["root"], f"source root mismatch: {surface['path']}")
            sources[surface["path"]] = encoded.decode("utf-8")

        claim_fields = struct_fields(
            sources["crates/vela-protocol/src/objects/claim_record.rs"],
            "ImportedClaimSource",
        )
        proposal_fields = struct_fields(
            sources["crates/vela-protocol/src/objects/proposal.rs"],
            "ImportedProposalSource",
        )
        cli = sources["crates/vela-cli/src/server/cli_commands.rs"]
        release_contract = sources["crates/vela-protocol/tests/cli_release_contract.rs"]
        epoch = sources["crates/vela-protocol/src/objects/repository_epoch.rs"]

        required_tokens = {
            "source_frontier_id": False,
            "source_repository_root": False,
            "source_claim_root": "object_root" in claim_fields,
            "source_decision_root": False,
            "source_authority_root": False,
            "completeness_status": False,
            "foreign_has_no_local_authority": False,
        }
        current_import_command = bool(
            re.search(r"\b(Federation|Foreign|Transfer|ImportClaim)\b", cli)
        )
        migration_lineage_only = (
            claim_fields == ["era", "object_id", "object_root", "predecessor_commit"]
            and proposal_fields == ["proposal_id", "proposal_root", "predecessor_commit"]
            and "imported_claim_set_root" in epoch
        )
        federation_deliberately_absent = (
            '"federation"' in release_contract
            and "help advanced still advertises removed surface" in release_contract
        )

        result = {
            "schema": "vela.foreign-transfer-contract-audit-result.v1",
            "outcome": "gap_reproduced",
            "plan_root": sha256(plan_bytes),
            "source": {"commit": commit, "tree": tree},
            "observed": {
                "imported_claim_source_fields": claim_fields,
                "imported_proposal_source_fields": proposal_fields,
                "repository_epoch_has_imported_claim_set_root": "imported_claim_set_root" in epoch,
                "migration_lineage_only": migration_lineage_only,
                "public_foreign_transfer_command": current_import_command,
                "federation_deliberately_absent_from_help": federation_deliberately_absent,
                "required_binding_presence": required_tokens,
            },
            "finding": (
                "Current imported_from fields retain predecessor-epoch migration lineage. "
                "They do not bind a foreign Frontier, source repository root, source Decision, "
                "source authority anchor, completeness status, or explicit local non-authority. "
                "The public CLI has no foreign transfer surface."
            ),
            "unsafe_reuse": (
                "Using imported_from or imported_claim_set_root as federation would rebind "
                "migration semantics and still omit required authority and completeness inputs."
            ),
            "next_gate": (
                "After the real correction is terminal, give the exact public objects to two "
                "independent readers. Add the smallest derived transfer envelope only if both "
                "fail for this same missing contract."
            ),
            "nonclaims": plan["nonclaims"],
        }
        encoded = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n"
        if args.output:
            require(not args.output.exists(), "output already exists")
            args.output.write_text(encoded, encoding="utf-8")
        else:
            print(encoded, end="")
        return 0
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"transfer contract audit failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
