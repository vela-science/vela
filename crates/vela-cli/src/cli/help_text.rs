//! `after_long_help` EXAMPLES blocks, one `pub const` per top-level command.
//!
//! Wired onto the clap variants (`#[command(after_long_help = …)]`) so
//! `vela <cmd> --help` leads the reader with worked examples, the way gh
//! and clig.dev prescribe. Plain text (clap strips color from help); `·`
//! is the only separator; house voice — lowercase, terse, concrete.
//!
//! Kept as data-only consts so the
//! `every_visible_command_has_examples` test can assert every visible verb
//! carries one. New verbs get a const here in the same edit that adds them
//! to the surface.

pub const SUBMIT: &str = "\
EXAMPLES
  vela submit submission.json                  retain a signed Submission v3 envelope
  vela submit --claim \"a(7) >= 22\" --type computational \
    --replayability exact --artifact w.json:witness --caveat \"bounded search\"
                                               author one signed Submission
  vela submit --claim \"corrected bounded result\" \
    --type theoretical --replayability exact --artifact diff.json:source-diff \
    --caveat \"exact source revision only\" --supersedes vcl_0123… \
    --target-root sha256:0123…
                                               request one exact supersession; no separate planning object is required
  vela submit submission.json                  retain and commit locally
  git push                                     publish with ordinary Git

Submission retains authenticated producer input as a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted-state
change. --corrects and --supersedes bind one full accepted Claim ID and root;
they never decide the Proposal and may describe an observed correction without
inventing a work-planning object. --check records only producer-reported checks.

SEE ALSO
  vela review show     inspect one exact deferred Proposal";

pub const STATUS: &str = "\
EXAMPLES
  vela status          what awaits, what is live (repository discovered upward)
  vela status . --json the machine view";

pub const REVIEW: &str = "\
EXAMPLES
  vela review inbox --json            exact consequence-only Decision Inbox
  vela review list --json             compact pending queue
  vela review show vpr_8b49… --json   one pending Proposal or terminal Decision
  vela review accept vpr_8b49… --reason \"bounded evidence is sufficient\" \
    --if-entry-root sha256:… --as agent:reviewer --session-ref native:session:… --json
                                        execute one exact attributed acceptance
  vela review reject vpr_8b49… --reason \"insufficient evidence\" --json
                                        execute one exact attributed rejection
  vela review withdraw vpr_8b49… --as agent:producer \
    --reason \"superseded by corrected work\" --json
                                        close your own pending Proposal without authority

The repository is optional on every one of these and discovered upward from the
current directory; name it first (`vela review show . vpr_8b49…`) to act on
another.

KNOWN PROPOSAL
  When a full vpr_ ID is supplied, start with `vela review show`. It returns
  either the pending Proposal, signed terminal Decision, or exact
  producer Withdrawal.
  A rejected proposal's candidate Claim is intentionally absent from
  accepted `show` and `log` views; that is not deletion.";

pub const CLAIMS: &str = "\
EXAMPLES
  vela claims                          what this repository holds, first page
  vela claims --limit 5                a short look
  vela claims --status all --json      accepted and unassessed together, machine view
  vela claims --cursor vcl_002c…       resume after the last row of the last page

Rows come out in Claim id order, which is the repository manifest's own order,
so a cursor names the same boundary on every call. Each row carries the full
vcl_ id `vela show` and `vela why` ask for.

ORIGIN ERA
  `origin` marks a Claim the repository's origin commit already bound — on a
  compacted repository, one that came through the compaction. `post_origin`
  marks one admitted by repository authority since. The reported origin count
  is its own number, not a share of the total: a repository can have retired an
  origin Claim, so the two do not subtract.

SCOPE
  This lists the claim index in the repository manifest, which is where a
  Standing is bound. A Claim belonging to a Proposal that was rejected,
  withdrawn, or is still pending is retained but holds no Standing; read those
  through `vela review list --status all`.

SEE ALSO
  vela show   one exact Claim in full
  vela why    how that Claim came to stand";

pub const LOG: &str = "\
EXAMPLES
  vela log               the accepted-event history, newest first
  vela log vpr_8b49…     restrict it to one object's covered history
  vela log . vpr_8b49…   the same, with the repository named";

pub const REPLAY: &str = "\
EXAMPLES
  vela replay             replay-verify the discovered repository
  vela replay . --json    verify every repository invariant";

pub const VERIFY: &str = "\
EXAMPLES
  vela verification record vpr_8b49… \\
    --profile exact-replay-v1 \\
    --method verification/method.json \\
    --outcome pass \\
    --does-not-establish \"Scientific acceptance.\" \\
    --independent-of agent:producer \\
    --as verifier:independent-check --json
        author, sign, and retain a scoped Verification Record

  vela verification import signed-verification.json \\
    --as verifier:independent-check --json
        import an already signed interoperable record

When the Submission has one verification requirement, omitting `--property`
uses that exact requirement and produces requirement-satisfying evidence. Use
`--property ... --complementary` only for an additional scoped observation.
The record binds the exact Submission, Proposal, Claim, artifacts, method,
manifest bytes, environment, scope, outcome, and verifier identity. It never
accepts the Proposal or changes Standing.

A canonical vela.review-method.v1 manifest additionally names whether the
review was performed by a human, AI model, organization, or deterministic
tool, plus the exact display identity, provider, identifier, known version,
and attesting actor. Use separate Verification Records for separate reviewers;
an AI synthesis is another scoped review, not a Decision.

The repository is optional and discovered upward; name it first
(`vela verification record . vpr_8b49… …`) to act on another.

SEE ALSO
  vela review show vpr_8b49…              exact next actions";

pub const INIT: &str = "\
EXAMPLES
  vela init ./my-repository --name \"Bounded question\" --scope \"Does X hold?\"
                                   create a signed, replayable repository

JSON mode requires both --name and --scope. Init binds the new repository to one
OpenSSH-agent Ed25519 identity and creates its signed repository origin in the
same command. It creates no Claim, Verification, Decision, or scientific
Standing. If signing is unavailable, the valid Profile is retained and the
error explains how to load the key and gives the exact recovery command.

FIRST USE
  ssh-add /path/to/private-key   load one dedicated Ed25519 key
  ssh-add -l              inspect full SHA256 fingerprints

On Linux, start ssh-agent before ssh-add. Vela never creates, reads, or stores
the private key.";

pub const SHOW: &str = "\
EXAMPLES
  vela show vcl_0123456789abcdef --json    the repository is discovered upward
  vela show vsb_0123456789abcdef --json
  vela show . vpr_0123456789abcdef --json  name it first to read another

Show verifies and renders one exact object. It reports the object's content
root, source context, and authority effect without changing the repository.";

pub const WHY: &str = "\
EXAMPLES
  vela why vcl_0123456789abcdef --json    the repository is discovered upward
  vela why . vcl_0123456789abcdef --json  name it first to read another

Why derives current or retained superseded Claim standing from covered
Proposal, Verification, Decision, and authority history and binds the
explanation to current roots.";

pub const CORRECTION: &str = "\
EXAMPLES
  vela correction impact vcl_0123456789abcdef --json
                                what accepting this correction would cost
  vela correction impact . vcl_0123456789abcdef
                                name the repository first to read another

The argument is the Claim carrying the correction — the successor holding a
`corrects` or `supersedes` relation — not the Claim being corrected. A
correction still in the review queue can be asked the same question as one
already accepted, which is the question a repository authority has before it
rules.

Impact is a read projection over the accepted claim index. It reads the two
relation kinds that carry consequence (`depends`, `supports`) and reports every
relation it excluded, so a reader can check the judgement rather than take it.
This projection is experimental: a Repository built solely with today's
writer cannot author `depends` or `supports` and correctly reports an empty
cascade.";

#[cfg(test)]
mod tests {
    use super::REVIEW;

    #[test]
    fn review_help_does_not_advertise_unimplemented_diff() {
        assert!(!REVIEW.contains("review diff"));
    }
}
