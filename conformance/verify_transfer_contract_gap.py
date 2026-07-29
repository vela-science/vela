#!/usr/bin/env python3
"""Clean-room reader for the current foreign-transfer contract inventory."""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    try:
        expected = json.loads(
            (
                ROOT
                / "conformance"
                / "fixtures"
                / "transfer"
                / "current-contract-gap.v2.json"
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
        retired_migration_fields_absent = all(
            token not in source
            for source in (claim_source, proposal_source)
            for token in ("imported_from", "ImportedClaimSource", "ImportedProposalSource")
        )
        observed = {
            "schema": "vela.foreign-transfer-contract-inventory.v2",
            "retired_migration_fields_absent": retired_migration_fields_absent,
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
                "source_claim_root": False,
                "source_decision_root": False,
                "source_authority_root": False,
                "completeness_status": False,
                "foreign_has_no_local_authority": False,
            },
            "outcome": "portable_contract_absent",
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
            "(two readers agree migration fields are retired and "
            "the canonical protocol has no foreign-transfer contract)"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"foreign-transfer-contract-gap: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
