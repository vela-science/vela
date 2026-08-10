//! Stable principals and closed authority-action boundaries.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const HUMAN_ONLY_AUTHORITY_ACTIONS_V1: &[&str] = &[
    "authority_initialize",
    "authority_close",
    "authority_revoke",
    "authority_rotate",
    "bulk_correct",
    "destroy",
    "membership_manage",
    "policy_activate",
    "policy_revoke",
    "policy_rotate",
    "quorum_manage",
    "recovery_approve",
    "review_accept",
    "review_reject",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Human,
    Agent,
    Workload,
    Service,
    Institution,
}

pub fn principal_class_may_request(principal_class: PrincipalClass, action: &str) -> bool {
    !matches!(
        principal_class,
        PrincipalClass::Agent | PrincipalClass::Workload
    ) || !HUMAN_ONLY_AUTHORITY_ACTIONS_V1.contains(&action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_human_governance_action_is_structurally_forbidden_to_machines() {
        for action in HUMAN_ONLY_AUTHORITY_ACTIONS_V1 {
            assert!(!principal_class_may_request(PrincipalClass::Agent, action));
            assert!(!principal_class_may_request(
                PrincipalClass::Workload,
                action
            ));
            assert!(principal_class_may_request(PrincipalClass::Human, action));
        }
        assert!(principal_class_may_request(
            PrincipalClass::Agent,
            "evidence_submit"
        ));
    }
}
