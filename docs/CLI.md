# The Vela CLI

Version control for scientific state. One sentence holds the whole
surface: **agents propose, verifiers reproduce, humans accept, git
publishes.**

The porcelain is 25 visible verbs, pinned by a both-directions test
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
| `submit` | The producer path in one verb: frozen-verify a witness, land it, bind it to its finding, drive the exact lane to `machine_verified`, materialize. `--dry-run` previews; commits locally, `--push` publishes. Replaces bespoke submit scripts. |
| `sign` | The one human ceremony: every deferred decision, one session, one confirm, one key read. |
| `status` | One-screen frontier state: findings by status, verdicts, replay integrity, sign-queue count, policy mode, and a `next` hint. |
| `log` | Recent signed events; `vela log <dir> <vf_>` is one finding's history. |
| `diff` | Two frontiers, or one pending proposal previewed. |
| `proposals` | The full proposal store: list/show/preview/import/validate/export/accept/reject. |

## Verify

| Verb | What it does |
|---|---|
| `check` | The full trust gate: replay, signatures, parity. `--strict` is the same bar the hub's ingestor holds a repo to. |
| `reproduce` | Re-verify stored witnesses from scratch with the frozen verifiers. |
| `proof` | Export a proof packet; `proof verify` re-checks one, `proof explain` narrates it. |
| `gate` | Claim-level verification gate: grade/check/vocab/backfill/attach/auto-admit. `gate attach --from inspect --log <eval.json> --finding <vf_>` ingests an Inspect-AI eval log as an `eval_harness` verifier attachment bound to the claim — evidence, not a verdict (`method_integrity: unattested`; a lone one fails the gate's independence check and never auto-admits). See [RECEIPTS.md](RECEIPTS.md). |
| `ci verdict` | The whole auto-merge decision for a frontier's Action, in one call: which finding proposals a PR adds (diffed against `--base <ref>`), whether each is `machine_verified` and a genuine beat, and whether the PR only touched the append-only store. Exit 0 iff it may auto-merge, so the Action is `vela ci verdict … && gh pr merge`. |

## Publish

| Verb | What it does |
|---|---|
| `hub` | The index: `register-git` binds a repo to its `vfr_` once (the one owner-signed act), after which `git push` IS publication. `witness-check`, `verify-chain`, `verify-log` hold hubs honest. |

## Nouns

| Verb | What it does |
|---|---|
| `finding` | The core primitive: add/show/supersede/note/caveat/revise/review/reject/retract/contribution/link. `review <f> --status accepted --as <you> [--confidence 0.9] --apply` records a human review verdict; an accept sets `review_state = Accepted` (with `--confidence` lifting it above the fragile floor in the same command), which the frontier state derives to `Established`. `contribution <f> --unit <ref> --role <role> --agent-kind <human\|agent\|model> --agent-id <id> [--apply]` appends claim-granularity attribution (who produced which unit); it is descriptive provenance an agent may self-apply — it never touches confidence or acceptance, and a `vouched` role requires a human. `add`/`supersede` are sourced by `--author`; the truth-bearing mutation verbs (`revise`/`review`/`reject`/`retract`) route to a reviewer via `--as`. These write with a human key and are CLI-only — they are not on the MCP agent surface. |
| `frontier` | Repo-level: new/materialize/add-dep/list-deps/diff/release/releases/audit/rank. `rank` orders OPEN findings by accumulating structural support (which is a verifier-run from done) with the popularity baseline + inspectable evidence — a solvability projection, advice not authority. |
| `actor` | Frontier-registered identities: add/list/rotate. |
| `agents` | `VELA.md` charter adapters: sync/doctor/diff (AGENTS.md, CLAUDE.md, .mcp.json are generated, never hand-edited). |
| `serve` | The frontier as an MCP server (stateless streamable HTTP or stdio) with nine agent-first tools: `read-only` exposes seven and `draft` adds only the two nonfinalizing write tools. `maintainer` is a deprecated warning alias for `draft`. Tools carry MCP annotations (`readOnlyHint` lets a client run the read tools in parallel; `work` is conservatively `destructiveHint:true` because its owner-checked `drop` action signs a coordination-only release and then removes private session scratch) and the high-traffic tools declare an `outputSchema` and return `structuredContent`, so a typed client reads a validated object instead of parsing JSON from text. Human decisions are terminal-only through `vela sign`. The hub hosts the clone-free subset at `hub.constellate.science/mcp`. |
| `doctor` | First-user diagnosis of checkout/frontier/proof/serve. |
| `foundry` | The discovery plane: `campaign`, `lean-*`, `attempt`, `transfer`, `experiment`. Search proposes; the frozen verifier is the gate. |

## Projections (read-only, dispatched ahead of the parser)

| Verb | What it does |
|---|---|
| `state` | Claim-state cell for one finding; `state trust`, `state pack`, `state diff` (Evidence Diff), anchors; `--as-of <RFC3339>` answers "what did we hold on this date". |
| `atlas` | Cross-frontier math-atlas projections. |
| `policy` | Governance policy: show/suggest/draft/test/sign/revoke/log. `evaluate-proposal <frontier> <vpr_>` is the CI-callable verdict on one proposal — `{admitted, verdict, is_beat, mergeable}` — that the frontier's auto-merge Action reads to merge a gate-clean beat unattended (re-verifies the recorded policy decision; never signs). |
| `credit` | Derived attribution view for one finding: `author_of_record` (humans who signed the assertion or an accepting review), `contributors`, and `originating_agents` (disclosed, never authors). A machine can originate a unit and be credited for it; it never enters `author_of_record`. |

## Decisions self-publish

Once your key has signed, everything that follows is mechanical
consequence, and the verb finishes it: `sign`, `proposals reject`,
and the policy auto-admit lane end by
materializing derived views, committing the store with a canonical
message that binds the signed event ids, and pushing. One intention,
one act — the signed decision can never again rot uncommitted on one
machine. `--no-commit` / `--no-push` hold publication per-call;
`vela id` config (`git_commit` / `git_push`: `auto` | `off`) sets the
default; `VELA_NO_PUBLISH=1` disables globally (the conformance gate
sets it). Nothing is ever auto-signed: publication only carries events
a key already signed. `vela status` warns about any store state that
predates this (`unpublished: N store file(s)…`), and `vela next`
ranks stranded state above all other work.

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
  `.vela/work/`. Do not edit or stage that record. Release without landing via
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
server, no ceremony (`maintainer` is a deprecated alias for `draft`). `vela
policy` is the other trust level: a signed,
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
the fidelity discipline made automatic — the policy opens the lane, the
gate keeps it honest.

`vela doctor` carries a setup lane for the operator machine: identity +
key permissions, binary pin state (including the workshop-build warning
— a cargo `target/` binary churns the pin), hub reachability, policy
freshness (14-day expiry warning), adapter sync, and registry health —
each row with the one command that fixes it.

Retired spellings and their successors: `inbox` → sign/next ·
`propose`/`record`/`pack`/`attach` → land · `accept`/`review` → sign ·
`id sign` → sign (hygiene lane) · `frontier next` → next.

## Configuration

Four layers, one doctrine: **plain config may change how Vela speaks to
you and how mechanical consequences execute; it may never change what
enters the record, who may decide, or where signatures go.** Those live
in `vela id` (identity + keys) and `vela policy` (signed rules) — no
scope of `vela config` can reach them.

- `vela config list` — the WHOLE closed key set, effective values, and
  where each came from (default / user / frontier / env / legacy).
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
`--author` exists only on `finding add`/`finding supersede` (the claim's
author, distinct from who is acting). `--verifier-actor` names the
mechanical identity a frozen-verifier attachment is drafted for. Nothing
else names an identity. The engine refuses `agent:`/`ci:` actors on
`sign` and `proposals reject` — decisions are
key-custody human acts.

## Worked example: next → work → land → sign

```bash
# the agent's session (VELA_ACTOR_ID=agent:demo)
vela next examples/sidon-sets --json   # ranked targets, payload pre-loaded
vela work sidon:a17 --as agent:demo     # claim the lease, load the briefing
vela land --work sidon:a17 \
  --claim "a(17) >= 292 for the Sidon frontier" \
  --artifact witnesses/a17.json:witness \
  --caveat "lower bound only; optimality not established" \
  --as agent:demo
                                # records, proposes, routes by the signed
                                # policy; a gate-clean witness auto-admits,
                                # otherwise it defers to the sign queue

# the human's session (their key)
vela sign                       # everything awaiting your key: one
                                # session, one confirm, one key read —
                                # self-publishes (materialize, commit, push)
```

## Policy tiers (shadow / staged / live)

A frontier may carry a sealed acceptance policy (`vap_` id,
`policies/active.json`). `vela status . --json` reports
`policy.mode`:

- **shadow** — no sealed policy on the frontier; the engine's built-in
  conservative kind-allowlist is the only mechanical lane.
- **staged** — a sealed policy sits at `.vela/policies/active.json` but
  is unsigned; advisory only, one human signature activates it.
- **live** — a `PolicySignatureRecord` (`active.sig.json`) signed by a
  human reviewer key activates it; mechanical proposal kinds (span
  repairs, artifact provenance) auto-admit with the `vap_` id stamped
  into the event.

A policy can only TIGHTEN the frozen-verifier floor. Truth-bearing
claims stay human-keyed in every mode; there is no configuration in
which an agent's proposal becomes accepted state without a human key.

## See also

[AGENT_QUICKSTART.md](AGENT_QUICKSTART.md) (the agent contract),
[PROTOCOL.md](PROTOCOL.md) (events, objects, ids),
[VERIFICATION.md](VERIFICATION.md) (what the gate holds),
[HUB.md](HUB.md) (the index and git-native publication).
