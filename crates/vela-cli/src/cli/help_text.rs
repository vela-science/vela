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
  vela submit submission.json --push           commit locally AND publish now

Submission registers authenticated producer input as a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted-state
change. --check records only producer-reported checks.

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

pub const PROPOSAL: &str = "\
EXAMPLES
  vela proposal withdraw . vpr_8b49… --as agent:producer \\
    --reason \"superseded by corrected evidence\"
        withdraw the producer's own pending Proposal without deleting evidence

Withdrawal is a producer lifecycle action, not a review Decision. A terminal
Proposal cannot be withdrawn.";

pub const LOG: &str = "\
EXAMPLES
  vela log .   the accepted-event history, newest first";

pub const CHECK: &str = "\
EXAMPLES
  vela check .           replay-verify the frontier
  vela check . --strict  every signal is fatal";

pub const REPRODUCE: &str = "\
EXAMPLES
  vela reproduce examples/sidon-a309370   re-verify every witness from scratch

No trust required: the frozen verifiers re-derive each stored witness.";

pub const PROOF: &str = "\
EXAMPLES
  vela proof verify packet.json     re-check a proof packet
  vela proof explain vf_…           what carries this finding";

pub const GATE: &str = "\
EXAMPLES
  vela gate check --claim \"exact claim\" --attachments attachments.json --json
  vela gate grade --claim \"bounded result\" --grade improved_published_bound --json

For a whole frontier, run `vela check . --strict`; when it stores witnesses,
also run `vela reproduce .`.";

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
                                   create the minimal Git-native frontier

JSON mode requires both --name and --scope. Optional integrations are separate.";

pub const DOCTOR: &str = "\
EXAMPLES
  vela doctor                     blockers plus one next action
  vela doctor --all               full setup and tool diagnostics";

pub const SERVE: &str = "\
EXAMPLES
  vela serve .          MCP over stdio for an agent
  vela serve . --http 3741
                        the same dispatcher over loopback HTTP";

pub const CONFIG: &str = "\
EXAMPLES
  vela config get work.lease_ttl_seconds --frontier .
  vela config set work.lease_ttl_seconds 43200 --frontier .
  vela config list --json
  vela config unset work.lease_ttl_seconds --frontier .

Layered: flag > VELA_* env > user config > Frontier convention > default.
Checked-in publish.git_push = off is narrowing-only and may override user auto.
Frontier v1 uses .vela/settings.toml; legacy .vela/config.toml remains readable.";

pub const ID: &str = "\
EXAMPLES
  vela id create --agent --handle canopus
                        create a file-backed producer identity
  vela id show          inspect the optional producer identity

Human decisions use the local OS principal and repository authority. Vela does
not create or store a human signing identity.";

pub const ACTOR: &str = "\
EXAMPLES
  vela actor list .              inspect the frozen Era-0 actor registry

New authority uses attributed principals, scoped capabilities, and repository
authority records. The legacy actor registry remains replayable and read-only.
Fresh setup uses `vela authority init`; historical Frontiers retain their exact
migration boundary.";

pub const FRONTIER: &str = "\
EXAMPLES
  vela frontier trust pin . --boundary-root sha256:... --json
                                       preview an out-of-band consumer pin
  vela frontier materialize .         rebuild derived views
  vela frontier diff left right       compare two frontiers
  vela frontier recover-publication --operation vop_…
                                       resume exact Git publication

`frontier trust pin` installs a local consumer trust anchor obtained through an
independent channel. It does not write Frontier history. Historical repository
boundaries and dependencies remain replayable and read-only.";

pub const CLAIM: &str = "\
EXAMPLES
  vela claim show . vf_6d4a…       read one historical Finding-era claim
  vela submit --claim \"…\" --artifact result.json:witness --as agent:demo
                                    register new work as Submission v1

Claim inspection is read-only. A `vf_` result discloses its historical Finding
source era; Vela does not relabel it as a current Claim Record. Submission v1
plus `vela submit` is the producer write path; current authority registers it
pending review, then one direct `vela review accept` or `vela review reject`
action.";

pub const SHOW: &str = "\
EXAMPLES
  vela show . vsb_0123456789abcdef --json
  vela show . vpr_0123456789abcdef --json

Show verifies and renders one exact object. It reports the object's content
root, source era, and authority effect without changing the frontier.";

pub const WHY: &str = "\
EXAMPLES
  vela why . vf_0123456789abcdef --json

Why derives the Claim or historical Finding standing from its Proposal,
Verification, and Decision chain and binds the explanation to current roots.";

pub const ARTIFACT: &str = "\
EXAMPLES
  vela artifact retract . va_417333a3e62df44a --reason \"legacy unpinned pointer\" --as agent:cleanup

This is the sole direct draft-retirement exception. It creates only a pending
proposal and never an accepted event; direct `vela review accept|reject`
actions are the human Decisions.";

pub const POLICY: &str = "\
EXAMPLES
  vela policy show .                  inspect the frozen Era-0 policy
  vela policy test .                  replay it over retained pending proposals
  vela policy evaluate-proposal . vpr_…
                                      inspect one retained admission
  vela policy log .                   list retained policy-lane events

Policy authoring, activation, rotation, revocation, and CI auto-merge porcelain
are retired. Existing signed policies and policy-lane events remain permanently
replayable. Fresh Frontiers use `vela authority init`; historical Frontiers
retain their exact migration boundary.";

pub const AGENTS: &str = "\
EXAMPLES
  vela agents sync .     regenerate CLAUDE.md/AGENTS.md/.cursor from VELA.md
  vela agents doctor .   assert the adapters are in sync (no drift)";
