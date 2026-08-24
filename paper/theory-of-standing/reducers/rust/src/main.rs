//! Independent Rust reducer for the proof-history interchange format.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::process::ExitCode;

const INPUT_FORMAT: &str = "theory-of-standing.proof-history.v1";
const RESULT_FORMAT: &str = "theory-of-standing.proof-result.v1";
const REJECTION_FORMAT: &str = "theory-of-standing.proof-rejection.v1";
const MAX_NAT: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    format: String,
    repository: u64,
    authorized_performers: Vec<u64>,
    initial_versions: BTreeMap<String, u64>,
    descriptive_dependencies: Vec<Dependency>,
    records: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dependency {
    dependent: u64,
    depends_on: u64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    Submission {
        claim: u64,
        producer: u64,
        scope: u64,
        authenticated: bool,
    },
    Verification {
        claim: u64,
        scope: u64,
        property: u64,
        outcome: Outcome,
    },
    Decision {
        id: u64,
        repository: u64,
        authority_label: u64,
        performer: u64,
        expected_root: u64,
        read_set: BTreeMap<String, u64>,
        action: Action,
    },
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Pass,
    Fail,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Accept {
        claim: u64,
    },
    Reject {
        claim: u64,
    },
    Correct {
        prior_decision: u64,
        predecessor: u64,
        replacement: u64,
    },
}

struct Event {
    action: Action,
    authority_label: u64,
    decision_id: u64,
    performer: u64,
    repository: u64,
}

struct State {
    events: Vec<Event>,
    root: u64,
    standing: BTreeMap<u64, &'static str>,
    submissions: Vec<(u64, u64)>,
    verifications: Vec<(u64, Outcome)>,
    versions: BTreeMap<u64, u64>,
}

fn canonical_resource_map(input: &BTreeMap<String, u64>) -> Result<BTreeMap<u64, u64>, String> {
    let mut output = BTreeMap::new();
    for (key, version) in input {
        let resource = key
            .parse::<u64>()
            .map_err(|_| "resource id must be a decimal u64".to_owned())?;
        if resource.to_string() != *key {
            return Err("resource id must use canonical decimal spelling".to_owned());
        }
        if resource > MAX_NAT || *version > MAX_NAT {
            return Err("resource ids and versions must be safe integers".to_owned());
        }
        output.insert(resource, *version);
    }
    Ok(output)
}

fn validate(history: &History) -> Result<BTreeMap<u64, u64>, String> {
    if history.format != INPUT_FORMAT {
        return Err("unsupported history format".to_owned());
    }
    if history.repository > MAX_NAT
        || history
            .authorized_performers
            .iter()
            .any(|value| *value > MAX_NAT)
        || history
            .descriptive_dependencies
            .iter()
            .any(|dependency| dependency.dependent > MAX_NAT || dependency.depends_on > MAX_NAT)
    {
        return Err("identifiers must be nonnegative safe integers".to_owned());
    }
    if !history
        .authorized_performers
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err("authorized performers must be sorted and unique".to_owned());
    }
    let versions = canonical_resource_map(&history.initial_versions)?;
    let mut decision_ids = BTreeSet::new();
    for record in &history.records {
        match record {
            Record::Submission {
                claim,
                producer,
                scope,
                ..
            } if [claim, producer, scope]
                .iter()
                .any(|value| **value > MAX_NAT) =>
            {
                return Err("Submission identifiers must be safe integers".to_owned());
            }
            Record::Verification {
                claim,
                scope,
                property,
                ..
            } if [claim, scope, property]
                .iter()
                .any(|value| **value > MAX_NAT) =>
            {
                return Err("Verification identifiers must be safe integers".to_owned());
            }
            Record::Decision {
                id,
                repository,
                authority_label,
                performer,
                expected_root,
                read_set,
                action,
            } => {
                if [id, repository, authority_label, performer, expected_root]
                    .iter()
                    .any(|value| **value > MAX_NAT)
                    || match action {
                        Action::Accept { claim } | Action::Reject { claim } => *claim > MAX_NAT,
                        Action::Correct {
                            prior_decision,
                            predecessor,
                            replacement,
                        } => {
                            *prior_decision > MAX_NAT
                                || *predecessor > MAX_NAT
                                || *replacement > MAX_NAT
                        }
                    }
                {
                    return Err("Decision identifiers must be safe integers".to_owned());
                }
                if !decision_ids.insert(*id) {
                    return Err("Decision ids must be unique".to_owned());
                }
                canonical_resource_map(read_set)?;
            }
            _ => {}
        }
    }
    Ok(versions)
}

fn projection(history: &History, state: &State) -> Vec<Value> {
    let corrected: BTreeSet<u64> = state
        .events
        .iter()
        .filter_map(|event| match &event.action {
            Action::Correct { predecessor, .. } => Some(*predecessor),
            _ => None,
        })
        .collect();
    state
        .standing
        .keys()
        .map(|claim| {
            let needs_reassessment = history.descriptive_dependencies.iter().any(|dependency| {
                dependency.dependent == *claim && corrected.contains(&dependency.depends_on)
            });
            json!({
                "claim": claim,
                "status": if needs_reassessment { "needs_reassessment" } else { "unaffected" },
            })
        })
        .collect()
}

fn output(history: &History, state: &State, code: Option<&str>) -> String {
    let events: Vec<Value> = state
        .events
        .iter()
        .map(|event| {
            json!({
                "action": event.action,
                "authority_label": event.authority_label,
                "decision_id": event.decision_id,
                "performer": event.performer,
                "repository": event.repository,
            })
        })
        .collect();
    let standing: Vec<Value> = state
        .standing
        .iter()
        .map(|(claim, status)| json!({"claim": claim, "status": status}))
        .collect();
    let mut value = json!({
        "events": events,
        "format": if code.is_some() { REJECTION_FORMAT } else { RESULT_FORMAT },
        "reassessment": projection(history, state),
        "repository": history.repository,
        "root": state.root,
        "standing": standing,
    });
    if let Some(rejection) = code {
        value
            .as_object_mut()
            .expect("result is an object")
            .insert("code".to_owned(), json!(rejection));
    }
    format!(
        "{}\n",
        serde_json::to_string(&value).expect("result serializes")
    )
}

fn eligible(state: &State, action: &Action) -> bool {
    match action {
        Action::Accept { claim } => {
            state
                .submissions
                .iter()
                .any(|(submitted, _)| submitted == claim)
                && state
                    .verifications
                    .iter()
                    .any(|(verified, outcome)| verified == claim && *outcome == Outcome::Pass)
        }
        Action::Reject { claim } => state
            .submissions
            .iter()
            .any(|(submitted, _)| submitted == claim),
        Action::Correct { replacement, .. } => {
            state
                .submissions
                .iter()
                .any(|(submitted, _)| submitted == replacement)
                && state
                    .verifications
                    .iter()
                    .any(|(verified, outcome)| verified == replacement && *outcome == Outcome::Pass)
        }
    }
}

fn reduce(history: &History, versions: BTreeMap<u64, u64>) -> (String, u8) {
    let mut state = State {
        events: Vec::new(),
        root: 0,
        standing: BTreeMap::new(),
        submissions: Vec::new(),
        verifications: Vec::new(),
        versions,
    };
    for record in &history.records {
        match record {
            Record::Submission {
                claim,
                producer,
                scope,
                authenticated,
            } => {
                let _ = producer;
                if *authenticated {
                    state.submissions.push((*claim, *scope));
                    state.standing.entry(*claim).or_insert("unassessed");
                    state.root += 1;
                }
            }
            Record::Verification {
                claim,
                scope,
                property,
                outcome,
            } => {
                let _ = property;
                if state
                    .submissions
                    .iter()
                    .any(|(submitted, submitted_scope)| {
                        submitted == claim && submitted_scope == scope
                    })
                {
                    state.verifications.push((*claim, *outcome));
                    state.root += 1;
                }
            }
            Record::Decision {
                id,
                repository,
                authority_label,
                performer,
                expected_root,
                read_set,
                action,
            } => {
                let code = if *repository != history.repository {
                    Some("wrong_repository")
                } else if !history.authorized_performers.contains(performer) {
                    Some("unauthorized")
                } else if authority_label != performer {
                    Some("misattributed")
                } else if expected_root != &state.root {
                    Some("stale_root")
                } else if canonical_resource_map(read_set)
                    .expect("read set validated")
                    .iter()
                    .any(|(resource, version)| state.versions.get(resource) != Some(version))
                {
                    Some("stale_read_set")
                } else if !eligible(&state, action) {
                    Some("ineligible")
                } else if let Action::Correct {
                    prior_decision,
                    predecessor,
                    ..
                } = action
                {
                    let event_exists = state.events.iter().any(|event| {
                        event.decision_id == *prior_decision
                            && event.repository == *repository
                            && matches!(
                                &event.action,
                                Action::Accept { claim } if claim == predecessor
                            )
                    });
                    if !event_exists || state.standing.get(predecessor) != Some(&"accepted") {
                        Some("invalid_correction_reference")
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(rejection) = code {
                    return (output(history, &state, Some(rejection)), 2);
                }

                match action {
                    Action::Accept { claim } => {
                        state.standing.insert(*claim, "accepted");
                    }
                    Action::Reject { .. } => {}
                    Action::Correct {
                        predecessor,
                        replacement,
                        ..
                    } => {
                        state.standing.insert(*predecessor, "superseded");
                        state.standing.insert(*replacement, "accepted");
                    }
                }
                state.events.push(Event {
                    action: action.clone(),
                    authority_label: *authority_label,
                    decision_id: *id,
                    performer: *performer,
                    repository: *repository,
                });
                state.root += 1;
            }
        }
    }
    (output(history, &state, None), 0)
}

fn invalid_format(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    println!(
        "{}",
        serde_json::to_string(&json!({
            "code": "invalid_format",
            "format": REJECTION_FORMAT,
        }))
        .expect("fixed error serializes")
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: theory-standing-rust-reducer HISTORY.json");
        return ExitCode::from(64);
    }
    let source = match fs::read_to_string(&args[1]) {
        Ok(value) => value,
        Err(error) => return invalid_format(error),
    };
    let history: History = match serde_json::from_str(&source) {
        Ok(value) => value,
        Err(error) => return invalid_format(error),
    };
    let versions = match validate(&history) {
        Ok(value) => value,
        Err(error) => return invalid_format(error),
    };
    let (bytes, code) = reduce(&history, versions);
    print!("{bytes}");
    ExitCode::from(code)
}
