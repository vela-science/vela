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
  vela next .        ranked open targets, payload pre-loaded
  vela next . --json the agent contract

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
  vela land --work sidon:a24 --claim \"a(24) > 7179\" --packet-root sha256:…
                                               exact binding also needs profile,
                                               capsule, and result roots
  vela land receipt.json --push                commit locally AND publish now

With one active session for this actor, --work is inferred. With several,
select one explicitly. A committed Permit or Defer closes session.json; Deny
or invalid input leaves it available for repair.

The four exact roots are all-or-nothing whole-Receipt-bound evidence. They do
not grant authority; only a matching signed v0.2 policy can Permit that result.

SEE ALSO
  vela review decide   approve one exact deferred proposal";

pub const SIGN: &str = "\
EXAMPLES
  vela sign                       decide one frontier's pending proposals
  vela sign vpr_8b49… --json      render root + observation time; read no key
  vela sign vpr_8b49… --yes --confirm-root sha256:… --confirm-at 2026-07-14T12:00:00Z
                                  accept only that exact rendered set
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
  vela review preview · show    inspect pending proposal bytes";

pub const STATUS: &str = "\
EXAMPLES
  vela status .        what awaits, what is live
  vela status . --json the machine view";

pub const REVIEW: &str = "\
EXAMPLES
  vela review list . --json           compact pending queue
  vela review show . vpr_8b49… --json one exact Decision Brief
  vela review preview . vpr_8b49…     read-only Decision Brief
  vela review decide . vpr_8b49… --reject --reason \"insufficient evidence\" --json
                                        key-free exact Decision Plan preview
  vela review decide . vpr_8b49… --reject --reason \"insufficient evidence\" \\
    --confirm-root sha256:… --confirm-at 2026-07-17T12:00:00Z
                                        one exact protected decision card
  vela review withdraw . vpr_8b49… --as agent:producer --reason \"superseded\"
                                        close your own Receipt-bound proposal";

pub const MIGRATE: &str = "\
EXAMPLES
  vela migrate . --to 0.900 --check --json  preview exact touched files and roots
  vela migrate . --to 0.900 --apply --json  apply the verified derived projection";

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
  vela serve . --http   the same dispatcher over HTTP";

pub const CONFIG: &str = "\
EXAMPLES
  vela config get hub.url
  vela config set hub.url https://hub.constellate.science
  vela config list --json
  vela config unset hub.url

Layered: flag > VELA_* env > frontier .vela/config.toml > user ~/.vela/config.toml";

pub const ID: &str = "\
EXAMPLES
  vela id create        one-time: generate a key and remember the actor
  vela id protect       one-time: protect a human approval identity
  vela id show          the current identity
  vela id lock          close the bounded local approval session
  vela id pin-binary    pin this binary's hash (ceremonies verify it first)";

pub const ACTOR: &str = "\
EXAMPLES
  vela actor list .              registered actors on this frontier
  vela actor add .               bootstrap an empty registry from `vela id`
  vela actor activate . --anchor <commit> --preview --json
  vela actor activate . --anchor <commit> --yes --confirm-root <sha256:...> --json

Activation is a human-key terminal ceremony. The preview reads no key.
Unsigned anchor members remain legacy and unauthenticated.";

pub const FRONTIER: &str = "\
EXAMPLES
  vela frontier materialize .         rebuild derived views
  vela frontier diff left right       compare two frontiers
  vela frontier recover-publication --operation vop_…
                                       resume exact Git publication";

pub const FINDING: &str = "\
EXAMPLES
  vela finding show . vf_6d4a…     read one accepted finding
  vela land --claim \"…\" --artifact result.json:witness --as agent:demo
                                    submit new work as Receipt v1

Finding is read-only. Receipt v1 plus `vela land` is the only producer write
path; policy routes it, and deferred work reaches `vela review decide`.";

pub const ARTIFACT: &str = "\
EXAMPLES
  vela artifact retract . va_417333a3e62df44a --reason \"legacy unpinned pointer\" --as agent:cleanup

This is the sole direct draft-retirement exception. It creates only a pending
proposal and never an accepted event; `vela review decide` is the human decision.";

pub const POLICY: &str = "\
EXAMPLES
  vela policy draft lean-rederivation projects/formal-conjectures-lean
  vela policy test  projects/formal-conjectures-lean   dry-run, mutates nothing
  vela policy decide . --activate vap_… --reason \"bounded verifier lane\"
                                                        key-free exact plan
  vela policy decide . --revoke --reason \"close this lane\"
                                                        key-free exact plan
  vela policy retire-legacy . --reason \"prelaunch bytes\" --as agent:cleanup --json
                                                        prepare only; keyless

`policy decide` previews one root-bound action without reading a key; its exact
confirmation requests one protected human card. Everything outside the signed
policy defers to human review, while exact matching agent work needs no prompt.
`policy sign` and key flags are advanced
historical compatibility surfaces. `retire-legacy` remains prepare-only.";

pub const AGENTS: &str = "\
EXAMPLES
  vela agents sync .     regenerate CLAUDE.md/AGENTS.md/.cursor from VELA.md
  vela agents doctor .   assert the adapters are in sync (no drift)";

pub const CI: &str = "\
EXAMPLES
  vela ci verdict --frontier .   is the claimed beat real?";
