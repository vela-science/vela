use std::collections::BTreeSet;

use serde_json::{Value, json};
use vela_protocol::{claim_record::ImportedClaimSource, proposal_v1::ImportedProposalSource};

fn fields(value: Value) -> Vec<String> {
    value
        .as_object()
        .expect("fixture type must serialize as an object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn current_public_contract_reproduces_the_registered_transfer_gap() {
    let claim_fields = fields(
        serde_json::to_value(ImportedClaimSource {
            era: "fixture".into(),
            object_id: "fixture".into(),
            object_root: format!("sha256:{}", "1".repeat(64)),
            predecessor_commit: "2".repeat(40),
        })
        .unwrap(),
    );
    let proposal_fields = fields(
        serde_json::to_value(ImportedProposalSource {
            proposal_id: "vpr_fixture".into(),
            proposal_root: format!("sha256:{}", "3".repeat(64)),
            predecessor_commit: "4".repeat(40),
        })
        .unwrap(),
    );
    let cli = include_str!("../../vela-cli/src/server/cli_commands.rs");
    let release_contract = include_str!("cli_release_contract.rs");
    let public_foreign_transfer_command = ["Federation", "Foreign", "Transfer", "ImportClaim"]
        .iter()
        .any(|candidate| cli.contains(candidate));
    let federation_deliberately_absent = release_contract.contains("\"federation\"")
        && release_contract.contains("help advanced still advertises removed surface");

    let observed = json!({
        "schema": "vela.foreign-transfer-contract-inventory.v1",
        "claim_import_fields": claim_fields,
        "proposal_import_fields": proposal_fields,
        "migration_lineage_only": claim_fields
            == ["era", "object_id", "object_root", "predecessor_commit"]
            && proposal_fields
                == ["predecessor_commit", "proposal_id", "proposal_root"],
        "public_foreign_transfer_command": public_foreign_transfer_command,
        "federation_deliberately_absent_from_help": federation_deliberately_absent,
        "required_binding_presence": {
            "source_frontier_id": false,
            "source_repository_root": false,
            "source_claim_root": claim_fields.contains(&"object_root".into()),
            "source_decision_root": false,
            "source_authority_root": false,
            "completeness_status": false,
            "foreign_has_no_local_authority": false
        },
        "outcome": "gap_reproduced"
    });
    let expected: Value = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/transfer/current-contract-gap.v1.json"
    ))
    .unwrap();
    assert_eq!(observed, expected);
}
