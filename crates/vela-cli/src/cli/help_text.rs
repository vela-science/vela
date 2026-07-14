//! `after_long_help` EXAMPLES blocks, one `pub const` per top-level command.
//!
//! Wired onto the clap variants (`#[command(after_long_help = …)]`) so
//! `vela <cmd> --help` leads the reader with worked examples, the way gh
//! and clig.dev prescribe. Plain text (clap strips color from help); `·`
//! is the only separator; house voice — lowercase, terse, concrete.
//!
//! Kept as data-only consts (the `HELP_KEY`/`HELP_AS` precedent) so the
//! `every_visible_command_has_examples` test can assert every visible verb
//! carries one. New verbs get a const here in the same edit that adds them
//! to the surface.

pub const NEXT: &str = "\
EXAMPLES
  vela next examples/sidon-sets        ranked open targets, payload pre-loaded
  vela next examples/sidon-sets --json the agent contract

SEE ALSO
  vela work   claim one of these targets";

pub const WORK: &str = "\
EXAMPLES
  vela work erdos:443 --as agent:demo --json
                                claim the lease and write one private session.json
  vela work erdos:443 --drop --reason \"switching approaches\" --as agent:demo
                                sign the exact lease release, then remove scratch

SEE ALSO
  vela next   the ranked offer this claims from
  vela land   land the result; use --work when more than one is open";

pub const LAND: &str = "\
EXAMPLES
  vela land receipt.json                       record → propose → route by policy
  vela land --work erdos:443 --claim \"a(7) >= 22\" --artifact w.json:witness
                                               build from the selected work session
  vela land receipt.json --push                commit locally AND publish now

With one active session for this actor, --work is inferred. With several,
select one explicitly. A committed Permit or Defer closes session.json; Deny
or invalid input leaves it available for repair.

SEE ALSO
  vela sign   decide what the policy deferred to you";

pub const SUBMIT: &str = "\
EXAMPLES
  vela submit witness.json --frontier examples/sidon-sets
  vela submit witness.json --dry-run           verify + preview, write nothing

The one-step producer path: verify, land, bind, drive the exact lane.";

pub const SIGN: &str = "\
EXAMPLES
  vela sign                       decide one frontier's pending proposals
  vela sign vpr_8b49… --json      render root + observation time; read no key
  vela sign vpr_8b49… --yes --confirm-root sha256:… --confirm-at 2026-07-14T12:00:00Z
                                  accept only that exact rendered set
  vela sign --batch fidelity.json separate fidelity-attestation lane
  vela sign --reset               discard a saved session and start clean

In the interactive session: a accept · r reject · s skip. Vela then builds
and shows the exact semantic set and transaction root before one confirm and
one key read. Run inside a frontier or pass --frontier; frontiers commit
independently.

Scripted decisions are two-step: the first invocation only renders the set,
root, and exact RFC3339 observation time. Mutation requires --yes plus both
echoed --confirm-root and --confirm-at values. Mismatch or drift stops pre-key.
The echoed time is valid for 15 minutes (with 60 seconds of future clock skew);
after that, render a fresh preview.

SEE ALSO
  vela proposals accept · reject   the store-level plumbing sign drives";

pub const STATUS: &str = "\
EXAMPLES
  vela status examples/sidon-sets        what awaits, what is live
  vela status examples/sidon-sets --json the machine view";

pub const LOG: &str = "\
EXAMPLES
  vela log examples/sidon-sets   the accepted-event history, newest first";

pub const DIFF: &str = "\
EXAMPLES
  vela diff vpr_8b49… --frontier projects/formal-conjectures-lean
  vela diff vpr_8b49… --frontier . --json";

pub const CHECK: &str = "\
EXAMPLES
  vela check examples/sidon-sets           replay-verify the frontier
  vela check examples/sidon-sets --strict  every signal is fatal";

pub const REPRODUCE: &str = "\
EXAMPLES
  vela reproduce examples/sidon-sets   re-verify every witness from scratch

No trust required: the frozen verifiers re-derive each stored witness.";

pub const REPRODUCE_EXTERNAL: &str = "\
EXAMPLES
  vela reproduce-external https://github.com/owner/repo.git <full-commit> Namespace.theorem \\
    --source-path Path/File.lean --out receipt.json --as agent:demo --json
                                             verify and emit Receipt v1 only
  vela reproduce-external https://github.com/owner/repo.git <full-commit> Namespace.theorem \\
    --source-path Path/File.lean --land-work erdos:443 --frontier . \\
    --as agent:demo --json                 verify, build, and land from the session

The installed adapter pins the source before fail-closed sandbox execution.
Lean checking is evidence about the formal declaration, not acceptance or a
claim that the translation is faithful or significant.";

pub const PROOF: &str = "\
EXAMPLES
  vela proof verify packet.json     re-check a proof packet
  vela proof explain vf_…           what carries this finding";

pub const GATE: &str = "\
EXAMPLES
  vela gate examples/sidon-sets   the one bar: check replay-verified AND
                                  reproduce all witnesses green";

pub const CREDIT: &str = "\
EXAMPLES
  vela credit vf_6d4a…            who contributed, in which role";

pub const INIT: &str = "\
EXAMPLES
  vela init ./my-frontier         scaffold a git-native frontier + hooks

Registers the frontier so the gate and `vela sign` discover it.";

pub const DOCTOR: &str = "\
EXAMPLES
  vela doctor                     check identity, binary pin, workspace health";

pub const SERVE: &str = "\
EXAMPLES
  vela serve examples/sidon-sets          MCP over stdio for an agent
  vela serve examples/sidon-sets --http   the same dispatcher over HTTP";

pub const CONFIG: &str = "\
EXAMPLES
  vela config get hub.url
  vela config set hub.url https://hub.constellate.science
  vela config list --json
  vela config unset hub.url

Layered: flag > VELA_* env > frontier .vela/config.toml > user ~/.vela/config.toml";

pub const ID: &str = "\
EXAMPLES
  vela id create        one-time: generate a key, remember actor + hub
  vela id show          the current identity
  vela id pin-binary    pin this binary's hash (ceremonies verify it first)";

pub const ACTOR: &str = "\
EXAMPLES
  vela actor list --frontier .            registered actors on this frontier
  vela actor add vib_… --frontier .       vouch a self-minted binding (your key)";

pub const FRONTIER: &str = "\
EXAMPLES
  vela frontier materialize examples/sidon-sets         rebuild derived views
  vela frontier materialize examples/sidon-sets --json";

pub const HUB: &str = "\
EXAMPLES
  vela hub register-git <vfr> <git-url>   register a frontier's canonical repo
  vela hub witness-check <vfr>            do the mirrors agree on the bytes?";

pub const PUBLICATION: &str = "\
EXAMPLES
  vela publication recover --operation vop_8b49…
  vela publication recover --operation vop_8b49… --push

Resumes one retained, path-exact Git transaction after rechecking the
frontier, checkout, caller index, worktree bytes, and target ref. It never
signs or changes scientific authority.";

pub const PROPOSALS: &str = "\
EXAMPLES
  vela proposals accept . vpr_8b49… --reason \"supported\" --json
  vela proposals accept . vpr_8b49… --reason \"supported\" --yes --confirm-root sha256:… --confirm-at 2026-07-14T12:00:00Z
  vela proposals reject . vpr_ed84… --reason \"superseded\" --yes --confirm-root sha256:… --confirm-at 2026-07-14T12:00:00Z
  vela proposals import . pending.json     import pending records only

An interactive terminal renders the exact semantic set and asks once.
Scripted and JSON use first preview key-free, then require --yes plus the exact
--confirm-root and --confirm-at pair within the 15-minute review window.
Prefer `vela sign` for the resumable proposal ceremony. Import never trusts
decision status fields; decided records
need a separately verified signed-authority/event importer.";

pub const FINDING: &str = "\
EXAMPLES
  vela finding add . --assertion \"…\" --author \"A. Researcher\"
                                                propose a new finding
  vela finding show . vf_6d4a…                 read one finding
  vela finding note . vf_6d4a… --text \"…\" --author agent:notes
                                                propose an annotation

Finding mutations create pending proposals. The legacy --apply flag is kept
only for parser compatibility and is always refused; humans decide with
`vela sign`.";

pub const ARTIFACT: &str = "\
EXAMPLES
  vela artifact retract . va_417333a3e62df44a --reason \"legacy unpinned pointer\" --as agent:cleanup

Retraction is draft-only here. `vela sign` is the human decision.";

pub const POLICY: &str = "\
EXAMPLES
  vela policy draft lean-rederivation projects/formal-conjectures-lean
  vela policy test  projects/formal-conjectures-lean   dry-run, mutates nothing
  vela policy sign  projects/formal-conjectures-lean   your key opens the lane
  vela policy revoke --reason \"…\"                     close the lane

Signing a policy delegates a class of gated work; everything else defers
to `vela sign`. `--json` requires `--yes`.";

pub const FOUNDRY: &str = "\
EXAMPLES
  vela foundry run --kind sidon --n 7 --seeds 20   search → frozen-verify
  vela foundry lean-targets --lean-dir ./lean      surface tractable gaps";

pub const AGENTS: &str = "\
EXAMPLES
  vela agents sync .     regenerate CLAUDE.md/AGENTS.md/.cursor from VELA.md
  vela agents doctor .   assert the adapters are in sync (no drift)";

pub const CI: &str = "\
EXAMPLES
  vela ci verdict --frontier examples/sidon-sets   is the claimed beat real?";
