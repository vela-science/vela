//! Identity, policy, admin, agents, experiments.
//! Re-exported flat (`crate::cli_*`) at the crate root; file organization only.

pub(crate) mod actor_bootstrap;
pub(crate) mod actor_registration;
pub mod cli_admin;
pub mod cli_agents;
pub mod cli_identity;
pub mod cli_policy;

pub(crate) mod binary_pin;
pub(crate) mod doctor_setup;
pub(crate) mod git_publish;
pub(crate) mod settings;
pub(crate) mod workspace_registry;
