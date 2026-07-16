# The Vela CLI

Version control for scientific state. One sentence holds the whole
surface: **agents propose, verifiers reproduce, humans accept, git
publishes.**

Submitting one result to a frontier you do not maintain? Use the
[producer quickstart](PRODUCER_QUICKSTART.md) before this full reference.

The visible porcelain is pinned by a both-directions test
(`crates/vela-cli/src/cli/tests.rs`): a verb cannot appear or disappear
without the baseline changing on purpose. Three read-only projections
(`state`, `atlas`, `policy`) are dispatched ahead of the parser, `credit`
derives per-finding attribution, and the discovery plane nests under
`foundry`. Every porcelain verb takes `--json` and emits a stable object
with `ok` and `command` fields.

The conventions this surface holds itself to — the output contract, colour
discipline, `--json` universality, interactivity rules, the help/EXAMPLES
pattern, and the grammar rule — are codified in
[CLI_STYLE.md](CLI_STYLE.md), each backed by a test so it cannot regress.

## Setup (once)

| Verb | What it does |
|---|---|
| `id` | Your key + identity: `create`, `show`, `import`, `keygen`, `sign`. After `vela id create`, no `--key`/`--as` flags are needed for your own writes; `id sign` re-signs your historical unsigned events. |
| `init` | Initialize a git-native frontier repo: `.vela/` is committed, the CI gate, agent charter (`VELA.md`), and `.mcp.json` are scaffolded. |

## The form factor

The CLI is **agent plumbing plus one human ceremony** — by design and
by evidence. Agents drive the loop (`--json` everywhere is their
contract; MCP is their read path). Humans meet Vela in three places:
their own coding agent (the official Vela plugin ships `/vela`
commands that render state, triage the queue, and author receipts
in-chat), the hub web (read, verify, cite, review), and exactly one
terminal act — `vela sign`, the clear-signing ceremony: it renders
every claim from the canonical log (never from agent output), takes
one confirm and one key read, and refuses to run under a binary that
no longer matches your pin (`vela id pin-binary`). There is no
interactive app and no TUI: the interactivity of this era belongs to
the agent, and the pen belongs to you.

## The loop

| Verb | What it does |
|---|---|
| `next` | The offer: ranked open targets with the compounding payload pre-loaded (premises, banked routes, prior attempts, dead channels). `--json` is the agent contract. |
| `work` | Claim a target and write one typed, ignored `session.json` under `.vela/work/`. `--drop --reason <why>` signs an exact zero-TTL lease release before removing scratch. |
| `land` | Land a result (`vela.receipt.v1`): record, propose, then route by the signed policy. Permit admits, Defer parks it in the sign queue, and Deny refuses canonical admission. `--work <target>` selects a session; it is inferred only when this actor owns exactly one. A committed Permit or Defer closes `session.json`; Deny or invalid input keeps it for repair. Commits locally; `--push` publishes. |
| `sign` | The one human ceremony: every deferred decision, one session, one confirm, one key read. |
| `status` | One-screen frontier state: findings by status, verdicts, replay integrity, sign-queue count, policy state plus Permit readiness, and a `next` hint. |
| `log` | Recent signed events; `vela log <dir> <vf_>` is one finding's history. |
| `diff` | Two frontiers, or one pending proposal previewed. |
| `proposals` | Read and export the proposal store: list/show/preview/validate/export. It cannot import or decide; external work enters as Receipt v1 through `land`. |

## Verify

| Verb | What it does |
|---|---|
| `check` | The full trust gate: replay, signatures, parity. `--strict` is the same bar the hub's ingestor holds a repo to. |
| `reproduce` | Re-verify stored witnesses from scratch with the frozen verifiers. |
| `proof` | Export a proof packet; `proof verify` re-checks one, `proof explain` narrates it. |
| `gate` | Read-only claim-level verification: `grade`, `check`, and `vocab`. Evidence enters through Receipt v1 and `land`; the gate has no writer. See [RECEIPTS.md](RECEIPTS.md). |
| `ci verdict` | The whole auto-merge decision for a frontier's Action, in one call: which finding proposals a PR adds (diffed against `--base <ref>`), whether each is `machine_verified` and a genuine beat, and whether the PR only touched the append-only store. Exit 0 iff it may auto-merge, so the Action is `vela ci verdict … && gh pr merge`. |

## Publish

| Verb | What it does |
|---|---|
| `hub` | Inspect and compare the Git-derived index. `git push` publishes frontier history; Hub operators select indexed repositories in a versioned source catalog. `witness-check` detects projection divergence and `verify-chain` checks frontier governance history locally. |

## Nouns

| Verb | What it does |
|---|---|
| `finding` | Read one accepted finding with `show`. There are no direct finding writers: assertions, evidence, notes, reviews, confidence changes, attribution, and relations enter through Receipt v1 and `land`, then follow the signed policy. Deferred work reaches the ordinary `vela sign` queue. |
| `artifact` | `retract` is the sole direct draft-retirement exception. It creates only a pending proposal, never an accepted event; the human decides it through `vela sign`. |
| `frontier` | Repo-level: new/materialize/list-deps/diff/release/releases/audit/rank. Dependencies are read-only projections of accepted state; external producers add dependency evidence through Receipt v1. `rank` orders OPEN findings by accumulating structural support (which is a verifier-run from done) with the popularity baseline + inspectable evidence — a solvability projection, advice not authority. |
| `actor` | Frontier-registered identities: one-time `add` bootstrap from the configured identity, then `list`. Established membership changes require signed governance; there is no direct rotate writer. |
| `agents` | `VELA.md` charter adapters: sync/doctor/diff (AGENTS.md, CLAUDE.md, .mcp.json are generated, never hand-edited). |
| `serve` | The frontier as an MCP server (stateless streamable HTTP or stdio) with eight agent-first tools: `read-only` exposes seven and `draft` adds only `work` (`claim`, Receipt-v1 `land`, owner-checked signed `drop`). Tools carry MCP annotations (`readOnlyHint` lets a client run read tools in parallel; `work` is conservatively `destructiveHint:true` because `drop` removes private session scratch) and high-traffic tools declare an `outputSchema` and return `structuredContent`. Human decisions are terminal-only through `vela sign`. The hub hosts the clone-free subset at `hub.constellate.science/mcp`. |
| `doctor` | First-user diagnosis of checkout/frontier/proof/serve. |
| `foundry` | The discovery plane: `campaign`, `lean-*`, `attempt`, `transfer`, `experiment`. Search produces activity and witness artifacts; Receipt v1 plus `land` is the canonical crossing. |

## Projections (read-only, dispatched ahead of the parser)

| Verb | What it does |
|---|---|
| `state` | Read-only claim-state projection for one finding: `state trust`, `state pack`, `state diff` (Evidence Diff), and `state anchors`; `--as-of <RFC3339>` answers "what did we hold on this date". |
| `atlas` | Read-only cross-frontier math-atlas projections. Atlas source adapters emit artifacts through `vela.receipt.v1`; use `vela land` for the only canonical write path. |
| `policy` | Governance policy: show/suggest/draft/test/sign/revoke/log. `evaluate-proposal <frontier> <vpr_>` is the CI-callable verdict on one proposal — `{admitted, verdict, is_beat, mergeable}` — that the frontier's auto-merge Action reads to merge a gate-clean beat unattended (re-verifies the recorded policy decision; never signs). |
| `credit` | Derived attribution view for one finding: `author_of_record` (humans who signed the assertion or an accepting review), `contributors`, and `originating_agents` (disclosed, never authors). A machine can originate a unit and be credited for it; it never enters `author_of_record`. |

## Decisions self-publish

Every durable route uses the same exact publication transaction. `land`
records the Receipt, proposal, and signed-policy result together; `sign`
publishes the exact human-reviewed event set. Both materialize derived views
and bind the resulting commit to the accepted event ids. `land --push`
publishes immediately; otherwise it commits locally for an explicit `git
push`. Nothing is auto-signed, and there is no second broad worktree publisher.
`VELA_NO_PUBLISH=1` disables publication in conformance runs.

`vela init` scaffolds versioned git hooks (`.vela/hooks`, activated via
`core.hooksPath`): pre-commit re-materializes views when events are
staged (committed store can never lead its views), pre-push holds the
push to the same strict bar CI enforces.

## The loop

The daily grammar is four verbs; everything else is reading or plumbing:

```text
next -> work -> land -> sign
```

- `vela next` — the offer: ranked open targets with the compounding
  payload pre-loaded (premises you may build on, banked routes, prior
  attempts, dead channels). A frontier may add a rich campaign target with
  campaign-declared source and verifier coordinates. Those coordinates are
  advisory coordination, not a source-integrity or verifier verdict. Vela
  replaces any supplied fixed base and authority-shaped fields with the live
  frontier root and the producer-only ceiling before it reaches `next`, MCP
  `orient`, or `work`. Checked surfaces reject oversized campaign bytes or
  tasks, unsafe target IDs, and duplicate resolved IDs before offering or
  claiming work. `--json` is the agent contract.
- `vela work <target> --as agent:<you> --json` — claim the lease, return the
  briefing and task contract, and write one typed private `session.json` under
  `.vela/work/`. The signed lease event exact-publishes in its own local Git
  commit before the private session is handed back, and JSON reports that
  publication result under `claim.publication`. Do not edit or stage the
  session record. Release without landing via
  `vela work <target> --drop --reason <why> --as agent:<you>`; Vela commits the
  signed lease release before deleting scratch.
- `vela land --work <target> --claim <result> --artifact <path>:<kind>
  --caveat <limit> --as agent:<you> --json` — build and land the existing
  **Vela Receipt** from the selected session. `--work` may be omitted only when
  this actor has exactly one active session. A **Vela Receipt**
  (`vela.receipt.v1`) is the portable JSON ANY tool exports — a Claude
  Science artifact, a notebook, a Codex run, a foundry search. Stateless and
  foreign producers may still pass `vela land <receipt.json>`:

  ```json
  {
    "schema": "vela.receipt.v1",
    "claim": "what is now known / bounded / refuted",
    "type": "computational | theoretical | empirical | negative",
    "artifacts": [{"path": "witness.json", "kind": "witness"}],
    "caveats": ["what this does NOT establish"],
    "verifier_runs": [{"method": "…", "outcome": "pass", "log": "…"}]
  }
  ```

  Landing records (artifacts hashed at land time, head pinned), lands a
  PENDING proposal, then routes by the frontier's signed policy:
  **Permit** admits canonically with no key ceremony (the human's
  authority arrived once, as the policy signature; the event carries
  the certificate, replay-verified); **Defer** parks it in the sign
  queue; **Deny** refuses canonical admission. Until the transactional write
  edge lands, clients inspect the structured result rather than assuming a
  zero-delta Deny.
- `vela sign` — the one human ceremony: everything exceeding policy,
  one session, one confirm, one key read.

Coming from Claude Code or Codex: what those tools call *permissions*
(which tools an agent may call, auto-accept, bypass) maps to Vela's
**MCP profiles** — `read-only` or nonfinalizing `draft`, enforced by the
server, no ceremony. `vela policy` is the other trust level: a signed,
expiring, content-addressed rule about what may become CANONICAL STATE
without your key — branch protection, except auditable in the log
forever. Profiles gate activity; policies gate state. Two words because
they are two different delegations.

The ceremony shrinks itself. `vela policy suggest` folds every ask that
reached your key into a histogram of claim classes and, when a class
recurs (3+), SHOWS the one rule whose signature would absorb it — a
named template when one covers the class, else a conservative bespoke
rule sealable with `vela policy draft --from-suggest`. Suggest never
seals and never signs; the sign session ends with the same hint when a
pattern exists. Rules you sign are branch protection; the residue at
your key is judgment, not friction.

The policy template ladder (`vela policy draft <template>`), ordered by
how much a signature delegates: `witness-rederivation` (exact witnesses
the frozen gate re-derived, A3, independent), `statement-drafts`
(theoretical receipts, A2), `search-witness` (frozen-verified
computational witnesses — bounds and finite confirmations from
`vela land type=computational` — A2, the autonomy lane for a harvest),
`notes-threshold` (notes, A0). A signed policy admits only GATE-CLEAN
results: even where a template permits a class, the engine gate still
requires zero new review warnings, so a fresh lone claim defers to a
human glance while a corroborated, gate-clean witness auto-lands. That is
the fidelity discipline made automatic — a ready policy authorizes Permit, the
gate keeps it honest.

### Prelaunch legacy-policy recovery

One narrow recovery command exists for a frontier whose unlaunched policy
bytes predate the hardened canonical policy format and therefore make ordinary
`policy show` or `policy revoke` fail closed:

```bash
vela policy retire-legacy . \
  --reason "retire unsupported prelaunch policy bytes" \
  --as agent:cleanup \
  --json
```

This command is prepare-only. It accepts neither `--key` nor `--yes`, reads no
private key, validates no legacy signature as current authority, deletes
nothing, and creates only a content-addressed pending
`governance.policy_legacy_retirement` proposal. The payload binds the stored
`vap_` ID and SHA-256 roots of the exact active policy and signature bytes. It
also records whether the fixed same-ID snapshot pair exists and is byte-for-byte
identical; callers cannot supply deletion paths.

Preparation and review fail closed unless the frontier has intact replay, no
signed policy head, no policy-lane history for the pair, conservatively no
unattributed legacy auto-admission history, a complete bounded non-symlink
active pair with matching stored IDs, and either no same-ID snapshots or one
exact duplicate pair. The existing isolated
`vela sign` Decision Plan rechecks and binds those files before any key access.
Human acceptance atomically appends the ordinary signed `review.accepted` event
and deletes only the fixed bound pair; rejection preserves every byte. A
frontier with no registered reviewer must first complete the existing human
identity bootstrap (`vela id show`, then `vela actor add <frontier>`). Agents do
not perform that ceremony.

`vela doctor` is local and offline: it checks identity + key permissions,
binary pin state (including the workshop-build warning — a cargo `target/`
binary churns the pin), policy freshness (14-day expiry warning), adapter
sync, and registry health. Hub reachability belongs to explicit hub commands,
so first-run diagnostics never wait on or depend on an external service.

## Configuration

Four layers, one doctrine: **plain config may change how Vela speaks to
you and how mechanical consequences execute; it may never change what
enters the record, who may decide, or where signatures go.** Those live
in `vela id` (identity + keys) and `vela policy` (signed rules) — no
scope of `vela config` can reach them.

- `vela config list` — the WHOLE closed key set, effective values, and
  where each came from (default / user / frontier / env).
- `vela config set <key> <value>` writes `~/.vela/config.toml` (user
  scope, the default). `--frontier` writes the shared, committed
  `.vela/config.toml` — allowlisted keys only, and safety-adjacent keys
  (`publish.*`) may only NARROW there: a cloned frontier can turn
  publishing off, never on, and can never name your hub (the git
  protected-configuration / Codex base-url rule).
- Every key has a `VELA_*` env alias that beats both files; flags beat
  everything. The CLI reads NO `.env` from the working tree — a cloned
  repo cannot configure its operator.

## The output contract

One grammar, enforced by one module (`crates/vela-cli/src/ui.rs`):

- **Frontier discovery**: the `[frontier]` positional is optional on the
  daily verbs — omitted, it is discovered by walking upward from the
  current directory, exactly like git finds `.git`. An object id in the
  frontier slot shifts automatically (`vela sign vpr_x --yes` works).
- **Exit codes**: 0 ok · 1 domain failure (gate red, verify fail) ·
  2 usage · 3 not found · 4 custody refused · 5 already exists. An agent
  that knows WHY a call failed can self-correct without parsing prose.
- **JSON guarantee**: under `--json`, every outcome — including every
  failure — is one JSON object `{ok, command, error?{kind,message,hint}}`.
  No prose ever leaks into a `--json` stream.
- **Hints**: errors carry a `hint:` line naming the exact next command;
  `--quiet` or `VELA_ADVICE=0` silences hints without touching messages.
- **Flags mean one thing**: `--as` = acting identity (all writes);
  `--key` = path to an Ed25519 private key, hex seed (defaults to your
  `vela id`); `--as-of` = RFC3339 instant. The same help text renders on
  every verb that carries the flag.

## Identity grammar

`--as <actor>` is THE acting-identity flag on every write verb.
Producer attribution is carried by Receipt v1 rather than a second finding
writer identity flag. `--verifier-actor` names the
mechanical identity recorded on an explicit frozen-verifier record. Nothing
else names an identity. The engine refuses `agent:`/`ci:` actors on
`sign` — decisions are
key-custody human acts.

## Worked example: next → work → land → sign

```bash
# the agent's session (VELA_ACTOR_ID=agent:demo)
vela next <frontier> --json            # ranked targets, payload pre-loaded
vela work sidon:a17 --as agent:demo     # claim the lease, load the briefing
vela land --work sidon:a17 \
  --claim "a(17) >= 292 for the Sidon frontier" \
  --artifact witnesses/a17.json:witness \
  --caveat "lower bound only; optimality not established" \
  --as agent:demo
                                # records, proposes, routes by the signed
                                # policy; Permit admits within this transaction,
                                # otherwise Defer parks it in the sign queue

# the human's session (their key)
vela sign                       # everything awaiting your key: one
                                # session, one confirm, one key read —
                                # self-publishes (materialize, commit, push)
```

## Policy state, Permit readiness, and evaluator outcome

A frontier may carry a sealed acceptance policy (`vap_` id,
`policies/active.json`). `vela status . --json` reports three separate facts:

- `policy.state` is `absent`, `staged_unsigned`, `active`, or `broken`.
  `active` means only that the content-addressed bytes and detached signature
  verify; it does not claim standing authority.
- `policy.permit_readiness` is `ready`, `human_only`, or `blocked`, with stable
  `reason_codes`. A missing, mismatched, or revoked causal policy head; a
  finite or expired wall-clock policy; or unresolved human signer authority is
  `human_only`. Malformed policy bytes or policy-head history is `blocked`.
- A proposal evaluation remains `permit`, `defer`, or `deny`. Only an
  intentional evaluator Deny refuses the proposal. A would-be Permit whose
  standing authority is `human_only` defers to `vela sign`; unavailable
  infrastructure is never relabeled as a policy Deny.

A policy can only TIGHTEN the frozen-verifier floor. Truth-bearing
claims stay human-keyed in every state; there is no configuration in
which an agent's proposal becomes accepted state without a human key.

## See also

[AGENT_QUICKSTART.md](AGENT_QUICKSTART.md) (the agent contract),
[PROTOCOL.md](PROTOCOL.md) (events, objects, ids),
[VERIFICATION.md](VERIFICATION.md) (what the gate holds),
[HUB.md](HUB.md) (the index and git-native publication).
