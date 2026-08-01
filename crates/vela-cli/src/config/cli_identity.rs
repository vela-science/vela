//! Resolve the actor for locally authored producer records.
//!
//! Vela does not maintain a second identity profile. An explicit `--as` wins,
//! then `VELA_ACTOR_ID`. The producer/verifier key resolver mints one local
//! per-actor key on first use. Imported signed objects carry their own actor.

use crate::cli::fail_return;

const SETUP_HINT: &str =
    "locally authored work requires --as agent:<name> or VELA_ACTOR_ID=agent:<name>";

pub(crate) fn resolve_actor(flag: Option<&str>) -> String {
    if let Some(actor) = flag.map(str::trim).filter(|actor| !actor.is_empty()) {
        return actor.to_string();
    }
    if let Ok(actor) = std::env::var("VELA_ACTOR_ID") {
        let actor = actor.trim();
        if !actor.is_empty() {
            return actor.to_string();
        }
    }
    fail_return(SETUP_HINT)
}
