//! Derived credit view (CRediT-style): a pure projection over the event log plus
//! finding provenance. It answers "who is accountable, and who produced what"
//! from data that already exists — signatures, `contributions[]`, `extraction`,
//! and the drafting actor. Never signed, never authoritative, always
//! recomputable, and it never invents an author.
//!
//! The anti-crumple-zone property: `author_of_record` is computed from valid
//! human signatures on accepting events, not from proximity. A machine holds no
//! key and can never be accountable, so a model appears only under
//! `contributors` / `originating_agents`, never as an author. If no human
//! signature exists, the author set is empty and the view says so.

use serde::Serialize;

use crate::project::Project;

/// One disclosed producer of a finding (or a unit of it), machine or human.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Contributor {
    pub agent_id: String,
    /// `human | agent | model`.
    pub agent_kind: String,
    /// `originated | derived | formalized | extracted | reviewed | vouched | drafted`.
    pub role: String,
    /// The sub-claim this applies to, or `whole`.
    pub unit: String,
}

/// The credit view for one finding. A projection, never signed.
#[derive(Debug, Clone, Serialize)]
pub struct CreditView {
    pub finding: String,
    /// Accountable humans: exactly the actors carrying a valid human signature on
    /// an accepting event. Empty means "no accountable author yet" — never
    /// invented.
    pub author_of_record: Vec<String>,
    /// Producers disclosed, machines included, from `contributions[]`,
    /// `extraction`, and the drafting actor.
    pub contributors: Vec<Contributor>,
    /// Contributions whose role is `originated` — may be a model, disclosed, and
    /// explicitly NOT an author.
    pub originating_agents: Vec<Contributor>,
    /// A one-line, methods-style rendering.
    pub statement: String,
}

/// Compute the credit view for a finding. Pure and deterministic.
#[must_use]
pub fn credit(project: &Project, finding_id: &str) -> Option<CreditView> {
    let finding = project.findings.iter().find(|f| f.id == finding_id)?;

    // author_of_record: humans carrying a VALID human Ed25519 signature on an
    // accepting event for this finding (`finding.asserted`, or `finding.reviewed`
    // with an accept). Validity is cryptographic, not nominal — the actor must be
    // registered, its key not revoked at the event timestamp, and the signature
    // must verify against that key. A present-but-unverifiable signature confers
    // no authorship; this is the anti-crumple-zone property, enforced here rather
    // than assumed from a prior verify pass.
    let mut author_of_record: Vec<String> = Vec::new();
    for e in &project.events {
        if e.target.id != finding_id || e.actor.r#type != "human" {
            continue;
        }
        let accepting = match e.kind.as_str() {
            "finding.asserted" => true,
            "finding.reviewed" => {
                e.payload.get("status").and_then(|v| v.as_str()) == Some("accepted")
            }
            _ => false,
        };
        if !accepting || author_of_record.contains(&e.actor.id) {
            continue;
        }
        let Some(actor) = project.actors.iter().find(|a| a.id == e.actor.id) else {
            continue; // unregistered signer — no accountable key
        };
        if actor.is_revoked_at(e.timestamp.as_str()) {
            continue; // key revoked at-or-before this event
        }
        if matches!(
            crate::sign::verify_event_signature(e, &actor.public_key),
            Ok(true)
        ) {
            author_of_record.push(e.actor.id.clone());
        }
    }

    // contributors: the recorded claim-granularity attributions, then the
    // extraction model, then the drafting actor if it was an agent.
    let mut contributors: Vec<Contributor> = Vec::new();
    for c in &finding.provenance.contributions {
        contributors.push(Contributor {
            agent_id: c.agent_id.clone(),
            agent_kind: c.agent_kind.as_str().to_string(),
            role: c.role.as_str().to_string(),
            unit: c.unit.clone(),
        });
    }
    if let Some(model) = finding
        .provenance
        .extraction
        .model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
    {
        push_unique(
            &mut contributors,
            Contributor {
                agent_id: model.to_string(),
                agent_kind: "model".to_string(),
                role: "extracted".to_string(),
                unit: "whole".to_string(),
            },
        );
    }
    for e in &project.events {
        if e.target.id == finding_id
            && e.kind.as_str() == "finding.asserted"
            && e.actor.r#type == "agent"
        {
            push_unique(
                &mut contributors,
                Contributor {
                    agent_id: e.actor.id.clone(),
                    agent_kind: "agent".to_string(),
                    role: "drafted".to_string(),
                    unit: "whole".to_string(),
                },
            );
        }
    }

    let originating_agents: Vec<Contributor> = contributors
        .iter()
        .filter(|c| c.role == "originated")
        .cloned()
        .collect();

    let statement = render_statement(&author_of_record, &contributors);

    Some(CreditView {
        finding: finding_id.to_string(),
        author_of_record,
        contributors,
        originating_agents,
        statement,
    })
}

fn push_unique(v: &mut Vec<Contributor>, c: Contributor) {
    if !v.contains(&c) {
        v.push(c);
    }
}

/// Methods-style: "Authored by <humans>. Contributions: <who> <role> <unit>; …"
fn render_statement(authors: &[String], contributors: &[Contributor]) -> String {
    let authored = if authors.is_empty() {
        "No accountable author yet".to_string()
    } else {
        format!("Authored by {}", authors.join(", "))
    };
    if contributors.is_empty() {
        return format!("{authored}.");
    }
    let parts: Vec<String> = contributors
        .iter()
        .map(|c| format!("{} {} {}", c.agent_id, c.role, c.unit))
        .collect();
    format!("{authored}. Contributions: {}.", parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{AgentKind, Contribution, ContributionRole, ContributionUnitType};
    use crate::events::{FindingEventInput, StateEvent, compute_event_id, new_finding_event};
    use crate::project::assemble;
    use crate::project::reverse_dep_index_tests::synth_finding;
    use crate::sign::ActorRecord;
    use ed25519_dalek::SigningKey;

    /// A deterministic keypair; returns the signing key and its hex pubkey.
    fn keypair(seed: u8) -> (SigningKey, String) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pubkey = hex::encode(sk.verifying_key().to_bytes());
        (sk, pubkey)
    }

    fn actor(id: &str, pubkey: &str) -> ActorRecord {
        ActorRecord {
            id: id.to_string(),
            public_key: pubkey.to_string(),
            algorithm: "ed25519".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        }
    }

    /// An accepting review event, signed by `key` (id computed before signing so
    /// the signature verifies against the canonical preimage).
    fn signed_human_review(finding: &str, human: &str, key: &SigningKey) -> StateEvent {
        let mut e = new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: finding,
            actor_id: human,
            actor_type: "human",
            reason: "accept",
            before_hash: "",
            after_hash: "",
            payload: serde_json::json!({ "status": "accepted" }),
            caveats: vec![],
            timestamp: Some("2026-01-01T00:00:00Z"),
        });
        e.id = compute_event_id(&e);
        e.signature = Some(crate::sign::sign_event(&e, key).unwrap());
        e
    }

    #[test]
    fn credit_names_human_signer_and_discloses_model_originator() {
        let mut f = synth_finding(0, vec![]);
        f.provenance.contributions = vec![Contribution {
            unit: "number-field-towers lemma".into(),
            unit_type: ContributionUnitType::Step,
            agent_kind: AgentKind::Model,
            agent_id: "openai/o5".into(),
            model: Some("openai/o5".into()),
            model_version: None,
            role: ContributionRole::Originated,
            basis: String::new(),
        }];
        let fid = f.id.clone();
        let mut project = assemble("credit", vec![f], 0, 0, "test");
        let (key, pubkey) = keypair(7);
        project.actors.push(actor("reviewer:will-blair", &pubkey));
        project
            .events
            .push(signed_human_review(&fid, "reviewer:will-blair", &key));

        let view = credit(&project, &fid).expect("finding exists");
        // The accountable author is the human signer, never the model.
        assert_eq!(
            view.author_of_record,
            vec!["reviewer:will-blair".to_string()]
        );
        assert!(!view.author_of_record.iter().any(|a| a.contains("openai")));
        // The model is disclosed as the originator, not as an author.
        assert!(
            view.originating_agents
                .iter()
                .any(|c| c.agent_id == "openai/o5" && c.agent_kind == "model"),
            "model must appear as an originating agent: {view:?}"
        );
        assert!(view.statement.contains("Authored by reviewer:will-blair"));
    }

    #[test]
    fn credit_invents_no_author_without_a_signature() {
        // A review event with NO signature must not confer authorship.
        let f = synth_finding(1, vec![]);
        let fid = f.id.clone();
        let mut project = assemble("credit", vec![f], 0, 0, "test");
        let (key, pubkey) = keypair(9);
        project.actors.push(actor("reviewer:x", &pubkey));
        let mut unsigned = signed_human_review(&fid, "reviewer:x", &key);
        unsigned.signature = None;
        project.events.push(unsigned);

        let view = credit(&project, &fid).expect("finding exists");
        assert!(view.author_of_record.is_empty());
        assert!(view.statement.contains("No accountable author yet"));
    }

    #[test]
    fn credit_rejects_a_forged_signature_from_an_unregistered_actor() {
        // A present, well-formed signature whose actor is not registered (no
        // accountable key) must not confer authorship — validity is cryptographic.
        let f = synth_finding(2, vec![]);
        let fid = f.id.clone();
        let mut project = assemble("credit", vec![f], 0, 0, "test");
        let (key, _pubkey) = keypair(11);
        // Signed by a real key, but the actor is never registered in project.actors.
        project
            .events
            .push(signed_human_review(&fid, "reviewer:ghost", &key));

        let view = credit(&project, &fid).expect("finding exists");
        assert!(
            view.author_of_record.is_empty(),
            "an unregistered signer is not an accountable author: {view:?}"
        );
    }

    #[test]
    fn credit_rejects_a_signature_that_does_not_verify() {
        // Actor registered, but the event's signature is from a different key.
        let f = synth_finding(3, vec![]);
        let fid = f.id.clone();
        let mut project = assemble("credit", vec![f], 0, 0, "test");
        let (wrong_key, _) = keypair(13);
        let (_right_key, registered_pubkey) = keypair(14);
        project.actors.push(actor("reviewer:y", &registered_pubkey));
        // Signed with wrong_key, but the registry holds registered_pubkey.
        project
            .events
            .push(signed_human_review(&fid, "reviewer:y", &wrong_key));

        let view = credit(&project, &fid).expect("finding exists");
        assert!(
            view.author_of_record.is_empty(),
            "a signature that does not verify against the registered key confers no authorship"
        );
    }
}
