#!/usr/bin/env python3
"""Clean-room reader for the current foreign-transfer contract inventory."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def struct_fields(source: str, name: str) -> list[str]:
    match = re.search(
        rf"pub struct {re.escape(name)} \{{(?P<body>.*?)\n\}}",
        source,
        re.DOTALL,
    )
    if match is None:
        raise ValueError(f"{name} is missing")
    return sorted(re.findall(r"pub ([a-z_]+):", match.group("body")))


def main() -> int:
    try:
        expected = json.loads(
            (
                ROOT
                / "conformance"
                / "fixtures"
                / "transfer"
                / "current-contract-gap.v1.json"
            ).read_text(encoding="utf-8")
        )
        claim_source = (
            ROOT / "crates" / "vela-protocol" / "src" / "objects" / "claim_record.rs"
        ).read_text(encoding="utf-8")
        proposal_source = (
            ROOT / "crates" / "vela-protocol" / "src" / "objects" / "proposal_v1.rs"
        ).read_text(encoding="utf-8")
        cli_source = (
            ROOT / "crates" / "vela-cli" / "src" / "server" / "cli_commands.rs"
        ).read_text(encoding="utf-8")
        release_contract = (
            ROOT / "crates" / "vela-protocol" / "tests" / "cli_release_contract.rs"
        ).read_text(encoding="utf-8")
        claim_fields = struct_fields(claim_source, "ImportedClaimSource")
        proposal_fields = struct_fields(proposal_source, "ImportedProposalSource")
        observed = {
            "schema": "vela.foreign-transfer-contract-inventory.v1",
            "claim_import_fields": claim_fields,
            "proposal_import_fields": proposal_fields,
            "migration_lineage_only": (
                claim_fields
                == ["era", "object_id", "object_root", "predecessor_commit"]
                and proposal_fields
                == ["predecessor_commit", "proposal_id", "proposal_root"]
            ),
            "public_foreign_transfer_command": any(
                candidate in cli_source
                for candidate in ("Federation", "Foreign", "Transfer", "ImportClaim")
            ),
            "federation_deliberately_absent_from_help": (
                '"federation"' in release_contract
                and "help advanced still advertises removed surface" in release_contract
            ),
            "required_binding_presence": {
                "source_frontier_id": False,
                "source_repository_root": False,
                "source_claim_root": "object_root" in claim_fields,
                "source_decision_root": False,
                "source_authority_root": False,
                "completeness_status": False,
                "foreign_has_no_local_authority": False,
            },
            "outcome": "gap_reproduced",
        }
        if observed != expected:
            print(
                json.dumps(
                    {"expected": expected, "observed": observed},
                    sort_keys=True,
                    indent=2,
                ),
                file=sys.stderr,
            )
            return 1
        print(
            "foreign-transfer-contract-gap: ok "
            "(two readers agree the current portable contract is absent)"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"foreign-transfer-contract-gap: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
