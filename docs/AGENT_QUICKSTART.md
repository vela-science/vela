# The agent contract

Vela is built to be driven by agents the way git is driven by hands:
**agents propose, verifiers reproduce, humans accept, git publishes.**
Every agent-drafted result crosses the same Receipt v1 and proposal boundary.
A human-signed policy may Permit a bounded class; otherwise Vela defers the
proposal to a human decision. The engine refuses agent actors on human
decision verbs.

## The rules (engine-enforced; also your instructions)

Agents may:

- inspect state: `vela status . --json`, `vela next . --json`, `vela log .`,
  `vela check . --strict`, `vela state <dir> <vf_>`, `vela diff <vpr_>`
- claim work: `vela work <target> --as agent:<name> --json`; Vela writes one
  typed private `session.json` under `.vela/work/`. Do not edit or stage it.
- land session results with flags: `vela land --work <target> --claim …
  --type … --replayability … --artifact <path>:<kind> --caveat …
  --as agent:<name> --json`. Omit `--work` only when this actor owns exactly
  one active session.
- import `vela land receipt.json` only for canonical Receipt v1 emitted by a
  foreign or stateless producer
- everything lands through `vela land` — records, drafts, verifier
  evidence; the signed policy routes each landing (Permit admits
  mechanically, Defer waits in the human's sign queue)
- abandon work with `vela work <target> --drop --reason <why>
  --as agent:<name> --json`; Vela signs the exact lease release before removing
  private scratch
- run the frozen verifiers: `vela reproduce .`
- rebuild derived views: `vela frontier materialize .`

Agents may not — the engine refuses these for `agent:`/`ci:` actors:

- run `vela sign`, `vela policy sign`, or direct accept/reject commands. These
  are key-custody human ceremonies and decisions.
- sign anything with a human's key (an agent-actor `record` never
  auto-resolves the configured human key; it signs only with a key passed
  explicitly, or stays honestly unsigned)
- delete private session files as a substitute for a signed lease release

Always export `VELA_ACTOR_ID=agent:<your-name>` and pass
`--as agent:<your-name>` on writes. Never run bare decision verbs.

The identity grammar, in full: `--as <actor>` is THE acting-identity flag
on every write verb. `--author` exists only on `finding add`/`finding
supersede` (the claim's author, distinct from who is acting), and
`--verifier-actor` names the mechanical identity a frozen-verifier
attachment is drafted for (e.g. `agent:vela-verify`). Nothing else names
an identity.

## The loop, end to end

```bash
export VELA_ACTOR_ID=agent:demo

vela status . --json      # where the frontier stands: findings by status,
                          # verdict distribution, replay integrity, inbox,
                          # and a `next` hint
vela next . --json        # ranked targets and their compounding context
vela work sidon:a17 --as agent:demo --json
                          # exact lease + briefing + typed private session
vela land --work sidon:a17 \
  --claim "a(17) >= 292 for the Sidon frontier" \
  --type computational \
  --replayability exact \
  --artifact witnesses/a17.json:witness \
  --caveat "lower bound only; optimality not established" \
  --as agent:demo \
  --json
                          # -> records/vrc_<id>.json (content-addressed,
                          #    head-pinned, artifact-hashed) -> pending
                          #    proposal -> routed by the signed policy:
                          #    Permit admits, Defer waits for `vela sign`;
                          #    both installed routes close session.json
vela check . --strict     # the full trust gate, locally
git push                  # publication: CI re-derives the frontier and the
                          # hub re-indexes from the repo
```

Do not ask a human to confirm the agent-authored receipt before this landing.
Receipt authoring carries producer authority. A human runs `vela sign` only for
the proposals Vela reports as `deferred`; the ceremony re-renders the exact
decision before key access. `policy_admitted` means a prior human-signed policy
authorized the bounded route. Deny or input error preserves the private session
for repair.

## MCP: the same loop for tool-calling agents

No clone at all? The public hub IS an MCP server: add
`https://hub.constellate.science/mcp` (streamable HTTP, no auth) to any
MCP client and you get the read-only tool surface over every live
frontier. Writes and verifier runs still happen in a clone — the hosted
endpoint cannot mutate state under any configuration.

Any frontier scaffolded by `vela init` ships `.mcp.json`; any client
opening the repo gets the read-only profile:

```json
{ "mcpServers": { "vela": { "command": "vela",
    "args": ["serve", ".", "--profile", "read-only"] } } }
```

The `read-only` profile exposes no mutating tool (a unit test enforces this);
`draft` adds only the nonfinalizing `propose` and `work` tools. The deprecated
`maintainer` name is a warning alias for `draft` and adds no capability.
Human decisions remain terminal-only through `vela sign`.

The surface is nine tools; each one answers an agent question:

| Question | Tool |
|---|---|
| Where am I / what should I work on? | `orient` — stats, open targets, gaps, recent events; pass `problem` for the full task briefing (the agent entry contract) |
| What exactly does this finding say? | `finding` — one vf_ with optional `include`: history, dependents, neighborhood |
| Where is X discussed? | `search` — findings, sources, evidence atoms; cursor-paginated |
| What is contested / what breaks if X falls? | `graph` — mode=contradictions, mode=impact (blast radius + retraction cascade), mode=traverse |
| Does the frontier pass the gate / do witnesses reproduce? | `verify` — mode=strict (the same bundle the hub's ingestor enforces), mode=witness (frozen-verifier re-check) |
| How do I submit work? | `propose` (draft profile) — kind=review/note/apply_note/revise_confidence/retract, always pending; `work` — action=claim/land/drop/deposit |
| What agent objects exist here? | `objects` — packs, attestations, evaluations, conflicts, tool descriptors |
| Is this novel / shareable? | `external` — service=pubmed prior-art count, service=nanopub export |

## JSON contracts

Every porcelain verb takes `--json` and emits a stable object with `ok`,
`command`, and command-specific fields. The two an agent reads most:

- `vela status . --json` → `findings.by_status` (accepted / contested /
  retracted / superseded — never one green check), `judgment.by_verdict`,
  `replay.ok`, `inbox.pending_total`, `proof.status`, `next`.
- `vela check . --strict --json` → the gate verdict with signals and the
  review queue.

## Discovery (optional, off-porcelain)

The verifier-gated discovery engine lives under `vela foundry`:
`foundry campaign search <kind> --n <n>` searches, the frozen verifier is
the gate, `--propose` lands the result as a pending proposal. Attempts,
transfers, Lean anchoring, and experiment receipts are
`foundry attempt|transfer|lean|experiment …`.

## Swarms (many agents, one frontier)

The loop scales by composition, not new machinery:

1. **Claim before long work**: `vela work <target> --as agent:<name> --json`
   or MCP `work` action=claim
   leases an obligation under your OWN agent key — minted automatically
   at `~/.vela/agents/<actor>/` from your `VELA_ACTOR_ID` the first time
   you claim, no key step needed (`VELA_AGENT_KEY_HEX` overrides). A
   live competing lease returns `already_claimed_by` — route around it.
   A lease coordinates; it never decides. Vela writes one typed private
   session bound to the exact lease and task contract. A committed Permit or
   Defer closes `session.json`; Deny and error retain it. Abandoned work uses
   `work --drop --reason <why>`, which appends a signed same-owner zero-TTL
   `attempt.claimed` update before removing scratch. An expired lease is
   ignored by the next claimer. Obligation ids may be frontier-external and namespaced
   (`erdos:443`); strict replay treats such leases as coordination, not
   orphaned targets.
2. **Watch, don't poll blind**: `GET /entries/{vfr}/events/stream?cursor=<event_id>`
   (SSE, cursor-resumable) streams what changed.
3. **One receipt per result**: use `vela land --work <target> --claim …
   --type … --replayability … --artifact <path>:<kind> --caveat …`.
   Vela builds one Receipt v1 from the typed session. Use file import for a
   receipt emitted by another producer, not for hand-authored plugin scratch.
4. **Policy-bound lanes**: when the frontier carries a signed acceptance
   policy (`vela status . --json → .policy.mode == "live"`), mechanical
   kinds (repairs, artifact provenance) auto-admit under the sealed
   `vap_` policy id — the policy can only tighten the frozen verifier
   floor, and truth-bearing claims stay human-keyed no matter what.

## Python SDK

`clients/python/vela_agent` wraps the read surface and proposal drafting
for Python-native agents. Note: its hub-publish helper predates the
git-native cutover (git push is publication now); prefer the CLI/MCP loop
above for writes.

## Doctrine, one paragraph

Activity is not state. A model run, notebook, record, or search hit remains
source material until it crosses the Receipt and proposal boundary. Accepted
state needs replayable human-key authority: either a prior signed policy
certificate or a direct human decision. Your job as an agent is to provide
hash-bound artifacts, honest caveats, and reproducible verifier runs. You do
not make the decision.

See also: [PROTOCOL.md](PROTOCOL.md) (the object model and record spec),
[VERIFICATION.md](VERIFICATION.md) (the gate), [HUB.md](HUB.md)
(git-native publication), [THREAT_MODEL.md](THREAT_MODEL.md).
