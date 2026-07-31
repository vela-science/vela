# ADR 0031: One Vela product and a removable Agent executor

- Status: Proposed
- Proposed: 2026-07-30
- Protocol effect: none
- Authority effect: none
- Product effect: retire Canopus as a separate product identity and expose its
  proven execution subset as optional `vela agent` porcelain
- Package effect: freeze historical Canopus `0.8.0`; move the current helper
  source to an internal `@vela-science/agent` package only after the shrink gate
- Release effect: one Vela tag and manifest; no separate `product-v*` train

## Context

Vela and Canopus currently present two products even though they already live
in one repository and participate in one user loop:

```text
Canopus run -> Canopus export -> Canopus submit -> Vela review
```

That split no longer earns its cost.

Canopus is an execution helper. It invokes an external agent, isolates its
workspace, freezes artifacts, runs a separately scoped verifier, records a
local Run, replays it, and exports an authenticated Submission. Vela owns the
Target and Attempt boundary, Submission registration, Verification intake,
Decision Inbox, human Decision, Event, replay, and Standing.

The current package boundary correctly keeps the runner away from scientific
authority. The separate product, command vocabulary, release train, and
duplicated Submission checks do not protect that boundary.

The drift is measurable. Current `packages/canopus` differs from immutable
`product-v0.8.0` by dozens of files and thousands of lines while its package
version still declares `0.8.0`. Exact counts are derived from Git when an
audit needs them rather than frozen into this ADR. The duplicate Submission
writer has now been removed; domain-specific missions, coverage rules,
verifier capsules, and evaluation fixtures still make the generic package
larger and less replaceable.

The execution evidence also argues for consolidation rather than expansion.
Canopus and native Codex using the same packets both passed all four repaired
Stage A cells. Canopus showed directional token and wall-time efficiency in a
repaired first-party pilot, but the study did not score the registered all-in
cost and expert-minute metric or clear a confirmatory product or adoption gate.
It has demonstrated useful custody and replay mechanics; it has not justified
a separate branded runtime or release train.

The current strategy memos agree on the durable boundary:

- Vela is the correction-aware Standing and authority layer.
- Agent runners and scientific workbenches are replaceable producers.
- The map and scientific-state loop must remain useful when Canopus is removed.
- Generic harness optimization detached from a current mapped Target should
  stop.

## Decision proposed

### 1. Retire the product identity, preserve the process boundary

Present one user-facing product:

```text
Vela governs scientific state.
vela agent is one optional way to produce bounded evidence.
```

The executor remains a separate helper process. It is not linked into the
authority core. Vela Standing replays without it; a historical Canopus Run
still requires the exact frozen Canopus helper and its retained dependencies.
The executor receives no repository-authority material and no human scientific
key.

Retiring the duplicate product story does not wait for helper distribution.
Current daily documentation, commands, and workflows should name Vela and the
optional `vela agent` path only. The private source may retain its historical
directory name during shrink so Git history and deletion review stay legible;
that temporary implementation name is not a second supported product.

Do not port the TypeScript runtime into Rust merely to make the repository look
uniform. The first `vela agent` implementation is a thin Rust delegator to an
optional `vela-agent` helper.

### 2. Use one coherent command path

Keep the existing `vela agents sync|doctor|diff` configuration and adapter
commands until a cold-use test demonstrates confusion. Singular `vela agent`
execution porcelain does not technically conflict with plural `vela agents`.

Expose the experimental execution surface:

```text
canopus doctor  -> vela agent doctor
canopus run     -> vela agent run
canopus show    -> vela agent show
canopus replay  -> vela agent replay
canopus export  -> vela agent export
canopus submit  -> remove; use vela submit
```

The ordinary loop becomes:

```text
vela next
-> vela start
-> vela agent run
-> vela agent show | replay | export
-> vela submit
-> vela review inbox
```

During an authorized Agent Campaign, `vela agent run --attempt <vat_id>` may
hand routine Submission and Verification evidence to the bounded foreground
campaign host. It gains no Decision method.

### 3. Derive execution plans from current Vela state

Replace public Mission and Profile authoring with one private in-memory run-plan
type derived from:

- the exact Frontier Target packet;
- one live Attempt and its budget;
- the selected external runner; and
- the verifier locator and custody contract.

Keep the run plan local, implementation-private, and noncanonical. Do not
standardize a new plan schema before two independent executors require it. It
cannot create Standing or become a second Target system.

### 4. Keep only the generic execution kernel during shrink

Keep under the current private Canopus source identity during shrink and
dogfood:

- `Engine` and `EngineResult`;
- the native Codex adapter;
- isolated agent home and workspace;
- compute, token, time, artifact, and write budgets;
- artifact freezing and bounded output checks;
- separate network- and write-denied verifier execution;
- compact Run and failure projections;
- exact replay and ephemeral Submission export; and
- authority-boundary and hostile-custody tests.

Move domain-owned material into exact source-local or immutable domain
execution bundles referenced by the Target or Attempt:

- prompts, missions, profiles, coverage rules, and duplicate-work logic;
- verifier capsules and domain verifier images; and
- domain-specific evaluation tasks and expected outputs.

Do not copy large runtime assets into canonical scientific state merely because
the owning Frontier names them.

Preserve completed benchmark plans and results as paper evidence. Retain
runnable evaluation machinery only while a registered study uses it.

Delete from the current helper:

- `src/product/submit.ts`;
- Erdős-specific `src/product/coverage.ts`;
- duplicate Vela diagnostics;
- exact Vela patch-version rejection as a product-wide compatibility rule;
- public Mission and Profile compatibility surfaces after run-plan derivation;
- domain binaries and capsules;
- inactive framework experiments that failed or never reached their adoption
  gate.

Only after the dogfood gate passes and Protocol publication has moved, rename
the earned helper and remove the Canopus brand, product README, and separate
release workflow. If the gate fails, delete the helper instead.

Recorded Runs continue to bind exact binary and dependency identities. Runtime
compatibility uses explicit schema and capability checks, not a second
hard-coded Vela release matrix.

### 5. Freeze history instead of carrying compatibility code

Freeze:

- `@vela-science/canopus@0.8.0`;
- Git tag `product-v0.8.0`; and
- its exact package bytes and replay instructions.

Do not republish that version or rewrite historical Run bytes.

Only after the dogfood gate, move earned current source with Git history:

```text
packages/canopus -> packages/agent
@vela-science/canopus -> internal @vela-science/agent
canopus -> vela-agent
```

During shrink and dogfood, reuse the current self-contained `canopus.run.v2`
bundle. Do not introduce a new Run schema or filesystem migration merely for
the product rename. If the helper earns distribution, a later ADR may name a
successor schema from demonstrated missing fields. Current Vela does not gain a
compatibility parser or rewrite old records.

### 6. Use one release identity

The current `product-v*` workflow also publishes
`@vela-science/protocol`. Before retiring it, move Protocol publication and
verification into the Vela-tag release workflow or explicitly freeze that
package.

One Vela `v*` tag and one release manifest identify the exact changed
components. Public packages retain their own component versions, but are built,
attested, and published from that one source tag. The optional Agent helper is
built and attached from the same tag only if it earns distribution. Unchanged
packages do not require artificial version churn.

Keep the helper private/source-only until distribution is earned. Do not claim
one-binary installation until macOS, Linux, and Windows packaging passes from
clean machines.

## Adoption gate

The helper earns public distribution only if one real dogfood campaign proves:

1. one bounded authorization supports twelve hours of routine work without
   repeated repository-signing prompts;
2. the worker and verifier receive neither repository-authority nor human keys;
3. Run, Artifact, verifier, Submission, and clean-clone replay roots match;
4. routine evidence changes no accepted state and reaches only
   `pending_review`;
5. the Decision Inbox contains only consequence-bearing review items; and
6. against native Codex with the same Target packet, the helper either improves
   verifier-passing output per all-in cost and reviewer minute by at least
   20 percent, or supplies a distinct custody or replay guarantee that the
   control cannot reproduce without rebuilding the same machinery.

If the gate is neutral or negative, delete the helper and keep Vela's external
producer contract.

The current implementation cannot yet claim this test. Attempt v7 permits and
retains one Agent Run, the helper caps one mission at one hour, and the
foreground Campaign host has no durable resume contract. The minimum
prerequisite is multiple root-linked Run receipts and restart-safe budget
replay under the same bounded private Attempt. This remains private
coordination state; it does not justify a scheduler, daemon, workflow graph, or
canonical Campaign object.

## Migration sequence

1. **Freeze and guard.** Verify immutable Canopus `0.8.0` package and tag;
   prevent current HEAD from being packaged under the same identity.
2. **Shrink.** Remove the duplicate Canopus product CLI, README, public
   exports, package-install CI, and `canopus submit` path; move domain
   material, remove coverage and duplicate diagnostic logic, derive an
   implementation-private run plan, and retain generic execution tests.
3. **Expose experimentally.** Add a thin `vela agent` delegator without moving
   the package or changing the Run schema. The delegator requires an explicit
   absolute `VELA_AGENT_BIN`, resolves it to one canonical executable, exposes
   only `doctor`, `run`, `show`, `replay`, and `export`, binds the helper to the
   invoking binary through `VELA_BIN`, and strips known SSH and
   repository-authority key environment variables before launch. It never
   invokes a shell. This avoids accidental credential forwarding; it does not
   sandbox the trusted controller helper, which still runs as the current OS
   user. Canopus continues to supply the actual worker and verifier isolation.
   Windows remains gated on a native helper executable rather than a
   shell-backed package-manager shim.
4. **Complete the minimum execution seam.** Add root-linked multi-run receipts
   and restart-safe private budget replay without adding an execution service
   or authority surface.
5. **Dogfood.** Run the twelve-hour Agent Campaign and matched native control.
6. **Rename, release, or delete.** Move and distribute the helper only if it
   clears the gate. Otherwise delete it. Move Protocol publishing before
   retiring the old package workflow.

Each phase is independently revertible and must leave Vela replay and all
Frontier repositories valid.

## Rejected alternatives

### Merge the executor into the authority core

Rejected. It would couple agent and verifier dependencies to deterministic
Standing replay and weaken removability.

### Rewrite the executor in Rust now

Rejected. Language uniformity does not justify translating approximately
8,900 lines of working TypeScript or changing its custody boundary.

### Keep Canopus as a separate public product

Rejected. Current evidence supports a useful optional execution seam, not a
second product story, command hierarchy, or release ceremony.

### Delete all executor code immediately

Rejected until the custody/replay deletion test runs. The isolation, artifact,
budget, and verifier mechanics may provide value even when the orchestration
does not outperform native Codex.

## Invariants

- Evidence is not Standing.
- Verification is not acceptance.
- An agent cannot accept, reject, or cancel a Proposal.
- Only an authorized Decision changes Standing.
- Current Standing replays without the Agent helper.
- Historical Canopus evidence remains inspectable with the exact frozen
  Canopus artifact, not a growing compatibility layer in current Vela.
- Removing the helper leaves the Vela product and every Frontier valid.
