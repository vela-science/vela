//! `vela claims` — what stands, enumerable.
//!
//! Every other read verb on the Claim surface takes a full 64-hex `vcl_` and
//! answers about that one Claim: `show` renders it, `why` explains its
//! Standing. Nothing produced the id. `review list` reaches only the Claims
//! carried by a retained Proposal, which on a compacted repository is a handful
//! out of thousands — on the Erdős repository, eleven of 2,782. A reader who
//! wanted the first thing anyone asks a repository ("what does it hold?") had to
//! read `.vela/repository.json` by hand.
//!
//! This verb reads exactly the claim index in the verified repository manifest
//! and pages it, so the ids the rest of the read surface requires can be
//! obtained from the read surface.
//!
//! ## What it does not reach, stated once
//!
//! The manifest binds a Standing to each Claim it lists, and that index is
//! this verb's whole subject. Claim bytes can also sit under `records/claims/`
//! without appearing in it — the Claim of a Proposal that was rejected,
//! withdrawn, or is still pending is retained, but the repository binds it no
//! Standing and it is not repository state. Those are reached through
//! `vela review list --status all` and `vela review show`, which is where a
//! Proposal's own Claim belongs. This verb does not scan the directory, so it
//! never presents an undecided Claim as something the repository holds.
//!
//! It does not reach the Proposal axis either, and so does not report it. The
//! manifest token a row is built from is a Proposal-axis word, but it is the
//! token of the list the row is in, not the status of any Proposal: on a
//! compacted repository the Proposal that once admitted the Claim is gone, and
//! restating list membership under a Proposal's name would assert a Decision
//! that is not there. `vela why` and `vela show` read retained Proposals and
//! answer both axes; this verb reads the index and answers the one the index
//! binds.
//!
//! Bytes are read only for the rows a page actually returns; `total` is the
//! index count. A row whose retained bytes cannot be read at their declared
//! root is returned as itself, marked unreadable and counted, rather than
//! failing the page or being quietly dropped from it. That is not defensive
//! decoration: the manifest load does not read Claim record bytes, so a
//! manifest can bind a path whose bytes are gone or altered and still load.
//! Corrupting one Claim on a copy of the quantum-codes repository returns four
//! readable rows, one marked row, `unreadable_returned: 1`, and a `total` that
//! still says 5.
//!
//! That paging was measured across the four epoch-1 repositories while they
//! were still live: every accepted Claim came out individually readable at its
//! declared root, none was unreachable, and the last compaction retained every
//! one. It is a historical observation now and cannot be repeated with this
//! binary. All four are archived and this release refuses their layout
//! outright, as `docs/CONTINUITY.md` records. The paragraph above also carried a total,
//! 2,844, which no artifact in this repository reproduces and which disagrees
//! with both figures that are sourced: ADR 0039 records 2,782 accepted as the
//! Observatory reported them, and the four `counts.accepted_claims` in
//! historical evidence disagrees about the total. The count is dropped rather
//! than picked between.
//!
//! What still checks this verb is `crates/vela-cli/tests/claims_enumeration.rs`,
//! against a repository the test builds.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use vela_protocol::repository_origin::RepositoryOriginV1;

/// The standings this index can bind, plus the escape hatch. `accepted` is the
/// default because "what stands?" is the question.
///
/// These are the standing axis's own words, not the manifest's. The manifest
/// binds each list a Proposal-axis token — `RepositoryV4::verify` holds
/// the pending list to `pending_review` — and this verb reads those tokens onto
/// the standing axis through `crate::claim_standing`, so the filter spells what
/// the rows now report. A Claim in the pending list is `unassessed`: no ruling
/// stands over it. `review list --status` keeps the Proposal vocabulary, which
/// is the axis it lists.
const STATUS_VALUES: [&str; 3] = ["accepted", "unassessed", "all"];

/// How much of one assertion a list row shows. Long enough for the retained
/// assertions on the live repositories to arrive whole, short enough that fifty
/// rows stay a list. The full text is one `vela show <claim>` away, and
/// `--json` never truncates.
const ASSERTION_SCALARS: usize = 132;

/// The Claim ids the repository's origin commit already bound.
///
/// This fails the verb rather than degrading to an unknown era, because there
/// is no state in which only this read fails: the manifest load performed
/// first resolves and verifies the same origin commit, so a repository that
/// cannot answer here never reached this line. Verified on a copy of the
/// quantum-codes repository given two commits carrying identical origin bytes —
/// `status`, `review list`, and `claims` all refuse it identically, at the
/// load.
fn origin_claim_ids(repository_path: &Path, origin: &RepositoryOriginV1) -> BTreeSet<String> {
    let initial = crate::repository::initial_repository(repository_path, origin)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    initial
        .accepted_claims
        .iter()
        .chain(&initial.pending_claims)
        .map(|reference| reference.claim_id.clone())
        .collect()
}

/// Which side of the origin a Claim entered on. A Claim the origin manifest
/// already bound came through the repository's last compaction; everything else
/// was admitted by repository authority since.
fn era_label(origin_ids: &BTreeSet<String>, claim_id: &str) -> &'static str {
    if origin_ids.contains(claim_id) {
        "origin"
    } else {
        "post_origin"
    }
}

/// One assertion as a single terminal line: control characters made visible by
/// the shared presentation boundary, then cut to a list-sized budget.
fn one_line(text: &str, budget: usize) -> String {
    let rendered = crate::cli::safe_text::inline(text);
    if rendered.chars().count() <= budget {
        return rendered;
    }
    rendered.chars().take(budget).collect::<String>() + "…"
}

pub(crate) fn cmd_claims(
    repository_path: &Path,
    status: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("claims", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let status = status.unwrap_or("accepted");
    if !STATUS_VALUES.contains(&status) {
        crate::cli::fail_kind(
            crate::ui::ErrorKind::Usage,
            "claims status must be accepted, unassessed, or all",
        );
    }
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    /* The same load `review list` uses: the manifest is only worth listing if
    repository authority covers it. */
    let repository = crate::repository::load_repository_at(&repository_path, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let origin_bytes = fs::read(repository_path.join(".vela/origin.json"))
        .unwrap_or_else(|error| crate::cli::fail_return(&format!("read current origin: {error}")));
    let origin = RepositoryOriginV1::parse(&origin_bytes)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let origin_ids = origin_claim_ids(&repository_path, &origin);

    /* Both manifest lists are already strictly sorted by Claim id and the
    protocol holds them that way, so this is the repository's own order, not
    one invented here — which is what makes the cursor stable across calls. */
    let mut references = repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .filter(|reference| {
            status == "all"
                || crate::claim_standing::from_proposal_status(&reference.standing) == status
        })
        .collect::<Vec<_>>();
    references.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

    let page = crate::cli::page::paginate("claims", "Claim", references, limit, cursor, |row| {
        Some(row.claim_id.as_str())
    });

    let mut unreadable = 0usize;
    let items = page
        .items
        .iter()
        .map(|reference| {
            let era = era_label(&origin_ids, &reference.claim_id);
            let standing = crate::claim_standing::from_proposal_status(&reference.standing);
            match crate::repository::read_claim(&repository_path, reference) {
                Ok(claim) => json!({
                    "claim_id": reference.claim_id,
                    "claim_root": reference.claim_root,
                    "path": reference.path,
                    "standing": standing,
                    "origin_era": era,
                    "readable": true,
                    "assertion_kind": claim.assertion.kind,
                    "assertion": claim.assertion.text,
                    "created_at": claim.created_at,
                    "revision": claim.revision,
                }),
                Err(error) => {
                    unreadable += 1;
                    json!({
                        "claim_id": reference.claim_id,
                        "claim_root": reference.claim_root,
                        "path": reference.path,
                        "standing": standing,
                        "origin_era": era,
                        "readable": false,
                        "unreadable_reason": error,
                        "assertion_kind": Value::Null,
                        "assertion": Value::Null,
                        "created_at": Value::Null,
                        "revision": Value::Null,
                    })
                }
            }
        })
        .collect::<Vec<_>>();

    let payload = json!({
        "schema": "vela.claims.v1",
        "ok": true,
        "command": "claims",
        "repository_id": repository.repository_id,
        "repository_root": repository_root,
        "origin_id": repository.origin_id,
        "generation": origin.generation,
        "status": status,
        "order": "claim_id_asc",
        /* Keyed by the standings the rows report and `--status` takes, so a
        caller reading `indexed` and a caller filtering are spelling one
        vocabulary. Keying it `pending_review` put a Proposal-status word on
        the standing axis in a counter, which is the same collapse the rows
        used to carry. */
        "indexed": {
            "accepted": repository.accepted_claims.len(),
            "unassessed": repository.pending_claims.len(),
        },
        /* Stated as its own count, never as a share of `total`. The two do not
        subtract: a repository can have retired an origin Claim, which is how
        quantum-codes holds 5 accepted Claims over an origin that bound 6. */
        "origin_claims": origin_ids.len(),
        "total": page.total,
        "returned": items.len(),
        "unreadable_returned": unreadable,
        "next_cursor": page.next_cursor,
        "items": items,
    });

    if json_out {
        crate::cli::print_json(&payload);
        return;
    }
    render(&payload, status);
}

fn render(payload: &Value, status: &str) {
    let total = payload["total"].as_u64().unwrap_or_default();
    let returned = payload["returned"].as_u64().unwrap_or_default();
    let unreadable = payload["unreadable_returned"].as_u64().unwrap_or_default();
    println!(
        "claims · {total} {status} · {}",
        payload["repository_id"].as_str().unwrap_or("")
    );
    println!("  {returned} shown, ordered by Claim id");
    println!(
        "  origin era · the repository origin bound {} Claim(s); each row says which side of it that Claim entered on",
        payload["origin_claims"].as_u64().unwrap_or_default()
    );
    if unreadable > 0 {
        println!(
            "  unreadable · {unreadable} of the {returned} row(s) shown have no readable bytes at their declared root"
        );
    }
    for item in payload["items"].as_array().into_iter().flatten() {
        println!(
            "  {}  {} · {}",
            item["claim_id"].as_str().unwrap_or(""),
            item["standing"].as_str().unwrap_or(""),
            item["origin_era"].as_str().unwrap_or("")
        );
        let assertion = match item["assertion"].as_str() {
            Some(text) => format!(
                "{} · {}",
                item["assertion_kind"]
                    .as_str()
                    .unwrap_or("kind not recorded"),
                one_line(text, ASSERTION_SCALARS)
            ),
            None => format!(
                "unreadable · {}",
                one_line(
                    item["unreadable_reason"]
                        .as_str()
                        .unwrap_or("reason not recorded"),
                    ASSERTION_SCALARS
                )
            ),
        };
        println!("      {assertion}");
    }
    if let Some(cursor) = payload["next_cursor"].as_str() {
        println!("  more · resume with --cursor {cursor}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filter has to spell what the rows report, or it matches nothing and
    /// reports zero — which reads exactly like an empty queue. The manifest's
    /// pending token is `pending_review`; the rows built from it now say
    /// `unassessed`, so that is what `--status` takes.
    #[test]
    fn the_status_filter_spells_the_standings_the_rows_report() {
        let manifest = vela_protocol::repository::ClaimStandingRefV1 {
            claim_id: format!("vcl_{}", "a".repeat(64)),
            claim_root: format!("sha256:{}", "b".repeat(64)),
            standing: "pending_review".into(),
            path: "records/claims/sha256/b.json".into(),
        };
        assert!(
            STATUS_VALUES.contains(&crate::claim_standing::from_proposal_status(
                &manifest.standing
            ))
        );
        assert!(STATUS_VALUES.contains(&"accepted"));
        assert!(!STATUS_VALUES.contains(&"pending"));
        assert!(
            !STATUS_VALUES.contains(&"pending_review"),
            "the Claim filter must not take a Proposal-status word"
        );
    }

    /// A Claim id the origin manifest never bound arrived through the current
    /// authority chain. This is the whole era rule, and getting it backwards
    /// would describe every post-compaction Claim as ancient.
    #[test]
    fn era_separates_the_origin_set_from_what_came_after() {
        let origin = BTreeSet::from(["vcl_old".to_string()]);
        assert_eq!(era_label(&origin, "vcl_old"), "origin");
        assert_eq!(era_label(&origin, "vcl_new"), "post_origin");
    }

    /// A genesis repository's origin bound nothing, so everything it holds
    /// arrived after it. An empty origin set is not a missing one.
    #[test]
    fn an_empty_origin_set_labels_everything_as_later() {
        assert_eq!(era_label(&BTreeSet::new(), "vcl_new"), "post_origin");
    }

    #[test]
    fn one_line_escapes_terminal_control_and_marks_its_cut() {
        assert_eq!(one_line("a\nb", 32), "a\\nb");
        assert!(!one_line("\u{1b}[2Jwiped", 32).contains('\u{1b}'));
        let cut = one_line(&"α".repeat(400), 16);
        assert_eq!(cut.chars().count(), 17);
        assert!(cut.ends_with('…'));
    }
}
