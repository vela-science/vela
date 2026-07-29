use serde_json::{Value, json};

#[test]
fn current_public_contract_reproduces_the_registered_transfer_gap() {
    let claim_source = include_str!("../src/objects/claim_record.rs");
    let proposal_source = include_str!("../src/objects/proposal_v1.rs");
    let retired_migration_fields_absent = !claim_source.contains("imported_from")
        && !claim_source.contains("ImportedClaimSource")
        && !proposal_source.contains("imported_from")
        && !proposal_source.contains("ImportedProposalSource");
    let cli = include_str!("../../vela-cli/src/server/cli_commands.rs");
    let release_contract = include_str!("cli_release_contract.rs");
    let public_foreign_transfer_command = ["Federation", "Foreign", "Transfer", "ImportClaim"]
        .iter()
        .any(|candidate| cli.contains(candidate));
    let federation_deliberately_absent = release_contract.contains("\"federation\"")
        && release_contract.contains("help advanced still advertises removed surface");

    let observed = json!({
        "schema": "vela.foreign-transfer-contract-inventory.v2",
        "retired_migration_fields_absent": retired_migration_fields_absent,
        "public_foreign_transfer_command": public_foreign_transfer_command,
        "federation_deliberately_absent_from_help": federation_deliberately_absent,
        "required_binding_presence": {
            "source_frontier_id": false,
            "source_repository_root": false,
            "source_claim_root": false,
            "source_decision_root": false,
            "source_authority_root": false,
            "completeness_status": false,
            "foreign_has_no_local_authority": false
        },
        "outcome": "portable_contract_absent"
    });
    let expected: Value = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/transfer/current-contract-gap.v2.json"
    ))
    .unwrap();
    assert_eq!(observed, expected);
}
