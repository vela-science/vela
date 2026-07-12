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
  vela work erdos:443           claim the lease and load the briefing
  vela work erdos:443 --drop    release the lease without landing

SEE ALSO
  vela next   the ranked offer this claims from";

pub const LAND: &str = "\
EXAMPLES
  vela land receipt.json                       record → propose → route by policy
  vela land --claim \"a(7) >= 22\" --artifact w.json   land without a receipt file
  vela land receipt.json --push                commit locally AND publish now

SEE ALSO
  vela sign   decide what the policy deferred to you";

pub const SUBMIT: &str = "\
EXAMPLES
  vela submit witness.json --frontier examples/sidon-sets
  vela submit witness.json --dry-run           verify + preview, write nothing

The one-step producer path: verify, land, bind, drive the exact lane.";

pub const SIGN: &str = "\
EXAMPLES
  vela sign                       decide everything awaiting your key
  vela sign vpr_8b49… --yes       scripted single accept (no session)
  vela sign --batch verdicts.json one key read over many verdicts
  vela sign --reset               discard a saved session and start clean

In the interactive session: a accept · r reject · s skip · then one
confirm. The final summary is editable (edit one · reset all) before the
single key read.

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

pub const PROPOSALS: &str = "\
EXAMPLES
  vela proposals accept vpr_8b49… --frontier .   the plumbing `vela sign` drives
  vela proposals reject vpr_ed84… --frontier . --reason \"superseded\"

Prefer `vela sign` — the human ceremony over everything awaiting your key.";

pub const FINDING: &str = "\
EXAMPLES
  vela finding add --author \"A. Researcher\" …   propose a new finding
  vela finding show vf_6d4a…                    read one finding
  vela finding note vf_6d4a… \"…\"                annotate (does not decide)";

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
