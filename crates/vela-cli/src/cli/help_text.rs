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

pub const NEXT: &str = "\
EXAMPLES
  vela next .        ranked open targets, payload pre-loaded
  vela next . --json the agent contract

SEE ALSO
  vela start  inspect one exact target briefing";

pub const START: &str = "\
EXAMPLES
  vela start erdos:443 --json
                                inspect the exact target, packet, and read roots

Start is stateless and write-free. It validates the current repository and
Target Index, prints the exact packet and read roots, and includes the exact
Submission binding when the packet declares rooted execution contracts. It
creates no lease, Attempt, counter, lock, Event, or Standing change.

SEE ALSO
  vela next   the ranked offer this claims from
  vela submit retain one authenticated producer Submission";

pub const SUBMIT: &str = "\
EXAMPLES
  vela submit submission.json                  retain a signed Submission v1
  vela submit --claim \"a(7) >= 22\" --type computational \
    --replayability exact --artifact w.json:witness --caveat \"bounded search\"
                                               author one signed Submission
  vela submit --claim \"corrected bounded result\" \
    --type theoretical --replayability exact --artifact diff.json:source-diff \
    --caveat \"exact source revision only\" --supersedes vcl_0123… \
    --target-root sha256:0123…
                                               request one exact supersession; no synthetic work target is required
  vela submit submission.json                  retain and commit locally
  git push                                     publish with ordinary Git

Submission retains authenticated producer input as a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted-state
change. --corrects and --supersedes bind one full accepted Claim ID and root;
they never decide the Proposal and may describe an observed correction without
inventing a ranked work target. --check records only producer-reported checks.

SEE ALSO
  vela review show     inspect one exact deferred Proposal";

pub const STATUS: &str = "\
EXAMPLES
  vela status .        what awaits, what is live
  vela status . --json the machine view";

pub const REVIEW: &str = "\
EXAMPLES
  vela review inbox . --json          exact consequence-only Decision Inbox
  vela review list . --json           compact pending queue
  vela review show . vpr_8b49… --json one pending Review Packet or terminal Decision
  vela review reject . vpr_8b49… --reason \"insufficient evidence\" --json
                                        execute one exact attributed rejection
  vela review withdraw . vpr_8b49… --as agent:producer \
    --reason \"superseded by corrected work\" --json
                                        close your own pending Proposal without authority

KNOWN PROPOSAL
  When a full vpr_ ID is supplied, start with `vela review show`. It returns
  either the pending Review Packet, signed terminal Decision, or exact
  producer Withdrawal.
  A rejected proposal's candidate Claim is intentionally absent from
  accepted `show` and `log` views; that is not deletion.";

pub const LOG: &str = "\
EXAMPLES
  vela log .   the accepted-event history, newest first";

pub const CHECK: &str = "\
EXAMPLES
  vela check .           replay-verify the frontier
  vela check . --json    verify every repository invariant";

pub const REPRODUCE: &str = "\
EXAMPLES
  vela reproduce .                         re-verify this Frontier from scratch

No trust required: the frozen verifiers re-derive each stored witness.";

pub const VERIFY: &str = "\
EXAMPLES
  vela verification record . vpr_8b49… \\
    --profile exact-replay-v1 \\
    --method verification/method.json \\
    --property \"Replay the exact retained artifact.\" \\
    --outcome pass \\
    --does-not-establish \"Scientific acceptance.\" \\
    --independent-of agent:producer \\
    --as verifier:independent-check --json
        author, sign, and retain a scoped Verification Record

  vela verification import . signed-verification.json \\
    --as verifier:independent-check --json
        import an already signed interoperable record

The record binds the exact Submission, Proposal, Claim, artifacts, method,
manifest bytes, environment, scope, outcome, and verifier identity. It never
accepts the Proposal or changes Standing.

SEE ALSO
  vela reproduce . --proposal vpr_8b49…   replay only pending evidence
  vela review show . vpr_8b49…             exact next actions";

pub const INIT: &str = "\
EXAMPLES
  vela init ./my-frontier --name \"Bounded question\" --scope \"Does X hold?\"
                                   create a Profile v2 bootstrap

JSON mode requires both --name and --scope. The next action is one explicit
repository-authority initialization; no scientific state or old event log is created.";

pub const SHOW: &str = "\
EXAMPLES
  vela show . vcl_0123456789abcdef --json
  vela show . vsb_0123456789abcdef --json
  vela show . vpr_0123456789abcdef --json

Show verifies and renders one exact object. It reports the object's content
root, source context, and authority effect without changing the frontier.";

pub const WHY: &str = "\
EXAMPLES
  vela why . vcl_0123456789abcdef --json

Why derives current or retained superseded Claim standing from covered
Proposal, Verification, Decision, and authority history and binds the
explanation to current roots.";

#[cfg(test)]
mod tests {
    use super::REVIEW;

    #[test]
    fn review_help_does_not_advertise_unimplemented_diff() {
        assert!(!REVIEW.contains("review diff"));
    }
}
