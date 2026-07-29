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
  vela start  begin an Attempt against one of these targets";

pub const START: &str = "\
EXAMPLES
  vela start erdos:443 --as agent:demo --json
                                claim the lease and start one bounded Attempt
  vela start erdos:443 --drop --reason \"switching approaches\" --as agent:demo
                                abandon the Attempt and release its lease

SEE ALSO
  vela next   the ranked offer this claims from
  vela submit register one authenticated producer Submission";

pub const SUBMIT: &str = "\
EXAMPLES
  vela submit submission.json                  register a signed Submission v1
  vela submit --attempt vat_0123… --claim \"a(7) >= 22\" --type computational \
    --replayability exact --artifact w.json:witness --caveat \"bounded search\"
                                               author from one active Attempt
  vela submit --attempt vat_0123… --claim \"corrected bounded result\" \
    --type theoretical --replayability exact --artifact diff.json:source-diff \
    --caveat \"exact source revision only\" --supersedes vcl_0123… \
    --target-root sha256:0123…
                                               request one exact supersession; no synthetic work target is required
  vela submit submission.json --push           commit locally AND publish now

Submission registers authenticated producer input as a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted-state
change. --corrects and --supersedes bind one full accepted Claim ID and root;
they never decide the Proposal and may describe an observed correction without
inventing a ranked work target. New Claims still require an active Attempt.
--check records only producer-reported checks.

SEE ALSO
  vela review show     inspect one exact deferred Proposal";

pub const STATUS: &str = "\
EXAMPLES
  vela status .        what awaits, what is live
  vela status . --json the machine view";

pub const REVIEW: &str = "\
EXAMPLES
  vela review list . --json           compact pending queue
  vela review show . vpr_8b49… --json one pending Review Packet or terminal Decision
  vela review diff . vpr_8b49…        read-only proposed state change
  vela review reject . vpr_8b49… --reason \"insufficient evidence\" --json
                                        execute one exact attributed rejection

KNOWN PROPOSAL
  When a full vpr_ ID is supplied, start with `vela review show`. It returns
  either the pending Review Packet or the signed terminal Decision record.
  A rejected proposal's candidate Claim is intentionally absent from
  accepted `claim show` and `log` views; that is not deletion.";

pub const LOG: &str = "\
EXAMPLES
  vela log .   the accepted-event history, newest first";

pub const CHECK: &str = "\
EXAMPLES
  vela check .           replay-verify the frontier
  vela check . --strict  every signal is fatal";

pub const REPRODUCE: &str = "\
EXAMPLES
  vela reproduce .                         re-verify this Frontier from scratch

No trust required: the frozen verifiers re-derive each stored witness.";

pub const VERIFY: &str = "\
EXAMPLES
  vela verification import . verification.json \\
    --as verifier:independent-check --json
        retain a signed Verification Record without accepting the Proposal

The record binds the exact Submission, Proposal, Claim, artifacts, method,
environment, scope, outcome, and verifier identity.

SEE ALSO
  vela reproduce . --proposal vpr_8b49…   replay only pending evidence
  vela review show . vpr_8b49…             exact next actions";

pub const INIT: &str = "\
EXAMPLES
  vela init ./my-frontier --name \"Bounded question\" --scope \"Does X hold?\"
                                   create a Profile v2 bootstrap

JSON mode requires both --name and --scope. The next action is one explicit
repository-authority initialization; no scientific state or old event log is created.";

pub const DOCTOR: &str = "\
EXAMPLES
  vela doctor                     blockers plus one next action
  vela doctor --all               full setup and tool diagnostics";

pub const CONFIG: &str = "\
EXAMPLES
  vela config get work.lease_ttl_seconds --frontier .
  vela config set work.lease_ttl_seconds 43200 --frontier .
  vela config list --json
  vela config unset work.lease_ttl_seconds --frontier .

Layered: flag > VELA_* env > user config > Frontier convention > default.
Checked-in publish.git_push = off is narrowing-only and may override user auto.
Frontiers use the single closed .vela/settings.toml contract.";

pub const ID: &str = "\
EXAMPLES
  vela id create --agent --handle canopus
                        create a file-backed producer identity
  vela id show          inspect the optional producer identity

Human decisions use the local OS principal and repository authority. Vela does
not create or store a human signing identity.";

pub const CLAIM: &str = "\
EXAMPLES
  vela claim show . vcl_6d4a…      read one current Claim
  vela submit --claim \"…\" --artifact result.json:witness --as agent:demo
                                    register new work as Submission v1

Claim inspection is read-only. Historical Finding bytes remain available from
the repository's pinned predecessor and historical Vela release. Submission v1
plus `vela submit` is the current producer write path.";

pub const SHOW: &str = "\
EXAMPLES
  vela show . vsb_0123456789abcdef --json
  vela show . vpr_0123456789abcdef --json

Show verifies and renders one exact object. It reports the object's content
root, source era, and authority effect without changing the frontier.";

pub const WHY: &str = "\
EXAMPLES
  vela why . vcl_0123456789abcdef --json

Why derives current Claim standing from its Proposal, Verification, and
Decision chain and binds the explanation to current roots.";

pub const AGENTS: &str = "\
EXAMPLES
  vela agents sync .     regenerate CLAUDE.md/AGENTS.md/.cursor from VELA.md
  vela agents doctor .   assert the adapters are in sync (no drift)";
