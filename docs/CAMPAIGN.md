# Active Vela campaign

## Objective

> Ship Vela as a clear, usable scientific-state product: let a researcher
> choose one valid Target, use any native tool to produce exact evidence,
> distinguish Verification from acceptance, make one consequential human
> Decision, replay the resulting Standing, and expose the next valid
> obligation.

This is the detailed active execution document. [ROADMAP.md](ROADMAP.md) holds
the shorter sequence; Git history and retained artifacts hold historical
campaign detail.

## Product thesis

Vela owns one narrow control point:

> A named Frontier's correction-aware, replayable Decision about what may now
> be inherited as scientific Standing.

The product loop is:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

Native tools own work. Git stores canonical Frontier history. Harbor owns
benchmark execution. Lean, notebooks, solvers, containers, laboratories, and
artifact stores retain their native responsibilities. The Observatory and Math
Atlas are disposable, root-bound readers.

## Invariants

These are the reasons Vela exists and are not simplification targets:

1. Evidence is not Standing.
2. Verification is not acceptance.
3. Agents cannot accept or reject a Proposal. A producer may withdraw only its
   own exact still-pending Proposal with the Submission identity that created
   it.
4. Only an authorized Decision changes Standing.
5. Accepted transitions replay from exact Git state.
6. Corrections append; they do not erase prior accepted state.
7. A projection, graph, database, ranking, or model output is never authority.

Everything else must earn its cost.

## Deletion rule

Before retaining a component, ask:

1. Does it protect evidence integrity, exact replay, or the Decision boundary?
2. Does it make that boundary materially easier for a real user to understand
   or operate?
3. Can a native tool plus a thin adapter do the same job?
4. Is there measured evidence that the component improves correctness, time,
   or expert effort?

If the first two answers are no, or the third is yes without a measured gap,
delete or defer it. Failed experiments leave compact evidence, not active
infrastructure.

## Current state — 2026-08-01

### Shipped and verified

- Vela `0.961.0` is the published binary release for repository v4, direct
  Target Index v5, direct Submission lineage, and the retired workflow/runtime
  surfaces. Internal Rust crates are not published as parallel products, and
  the unused TypeScript protocol package has been removed.
- Four mathematical Frontiers use the current repository contract and replay
  from clean Git checkouts.
- Real Erdős, Formal, and quantum Submissions have separate scoped
  Verifications while their accepted-state delta remains zero.
- A prior real human Decision changed Standing exactly once and reproduced by
  replay.
- Vela Web has a live root-bound Math Source Registry and read-only Math Atlas
  over the exact four repository-v4 Frontier heads. The current projection uses
  `observatory.v8`, contains no Registration contract, and retains 6,701 native
  source records and 5,845 source bindings. Source Git remains authoritative.
- Product-compression v9 completed four clean native Harbor trials with zero
  retries. Vela-guided work was exact in 2/2 trials while Git/files alone was
  exact in 0/2; median agent time fell from 296.86 to 72.13 seconds and median
  cost from $0.7275 to $0.1097. The compact result is rooted at
  `sha256:970ed8a122d08ba1b9e25cee513fee7141cb54e1321c3d38f0ba22d51fc2e077`.
  This is first-party evidence from one frozen task, not an independent-user or
  general scientific-productivity claim.
- The immutable Canopus `0.8.0` release and tag remain historical evidence.

### Simplified current source

- Current Vela ships no agent runner, Campaign host, scheduler, custom Run
  receipt model, or Canopus compatibility layer.
- `vela start` is a write-free exact Target briefing. It creates no Attempt,
  lease, budget, counter, or authorization and is not required to author
  signed evidence.
- Agents and workbenches consume the Target packet directly and submit a
  Submission through `vela submit`.
- Verifiers emit the standard Verification Record and import it through
  `vela verification import`.
- Unused capability-grant and delegated-authority protocol machinery is gone.
  Current authority records preserve only the canonical `delegation: null`
  boundary and fail closed on non-null input.
- Routine Submission and Verification intake now verifies the producer or
  verifier signature and publishes append-only evidence without a
  repository-authority key. A later human Decision checkpoints the exact
  evidence preimage and publishes one local Git commit.

### Open truth

- Several technically eligible Proposals still require an attributed human
  Decision or cancellation. Eligibility is not a recommendation.
- The Formal cross-Frontier transfer is technically Decision-eligible. A
  held-out consumer and measured value test remain required only before
  promoting the derived envelope into a supported shared contract or claiming
  independent product value. A human Decision made earlier must retain that
  limitation explicitly.
- The production Atlas is current at projection root
  `sha256:43d5e4ab8b034ba12677a6423a3228e7a0537003e4c351915a00c9d0df178fbc`.
  It must be refreshed whenever canonical Frontier heads change; source Git
  remains authoritative during projection lag.
- The current Registry adapter artifact has an immutable GitHub release
  locator and independently reproduced byte root.
- External cold-user lift, correction propagation, and independently governed
  interoperability remain unproved.

## Active gates

### Gate 1 — keep the core small and green

The runner, unused capability protocol, and local Attempt policy engine are
gone. Remove stale code when encountered and use focused checks during
development. Run the full release union only for a real release candidate; do
not bump versions merely to synchronize metadata.

The remaining mechanism budget is explicit:

| Mechanism | Disposition | Why |
| --- | --- | --- |
| Producer and verifier signatures | Keep | Authenticate exact evidence without granting Standing. |
| One repository-authority DSSE record per human Decision | Keep | Binds the attributed Decision and exact replayable transition. |
| Retained Cedar policy material | Freeze | Existing authority history depends on it. Add no policy product, ordinary-work gate, or administration UI; replace it only in a future current-schema cut that deletes more code than it adds. |
| Crash journal and Git compare-and-swap publication | Keep | Protect partial-write recovery and concurrent publication, not workflow ceremony. |
| Harbor jobs, OAuth, retries, trajectories, timing, and cost | External | Harbor already owns benchmark execution. Vela keeps only its task fixture and semantic scorer. |
| Neon branches | Delete/defer | One `main` read projection is enough; rehearse locally in disposable PostgreSQL. |
| Agent Campaign runtime, scheduler, or transcript store | Reject | Native agents own execution and durable approval state. Vela exposes consequential pending Decisions only. |

No retained mechanism may create another prompt for routine computation,
artifact creation, Submission, or Verification. A human interruption is
justified only by a change to Standing, policy/schema/authority, scope or risk,
external publication, or a destructive action.

### Gate 2 — complete one clear scientific loop

For one real pending Proposal:

1. show the exact proposed Claim and scope;
2. show its Submission, decisive artifacts, verifier requirements, and scoped
   Verification results;
3. show what Standing would change and what would remain unchanged;
4. let the human authority accept or reject, or let the exact producer
   withdraw obsolete pending work without authority;
5. verify the exact before/after roots and replay;
6. rebuild the read projection; and
7. expose the next valid Target or explicit absence of one.

No agent performs step 4. A verifier pass cannot select the outcome.

This scientific gate is separate from product promotion. External adoption,
cold-user lift, and a second maintained consumer do not make a scoped Claim
more or less true. They gate claims about Vela's usefulness and whether a
derived transfer envelope deserves a maintained shared contract.

### Gate 3 — compress the long-running workflow

The target experience is one bounded authorization followed by hours of native
agent work and one or a few consequential review items.

The minimum product seam is:

- a scoped authorization expressed through the native executor's durable
  action policy, not through `vela start` or a Vela-owned runner;
- continuous native work and append-only evidence lineage without a Vela-owned
  runner;
- native action approvals for scope, budget, external publication, and
  destructive operations;
- a read-only Vela Decision Inbox containing only proposed changes to
  scientific Standing; and
- one exact human Decision transaction bound to current read roots.

Informational receipts belong in a digest, not an approval queue. This design
must reuse standard tool/action approval and resumable-run patterns where a
native agent platform supplies them. It must not add a daemon, hosted signer,
second writer, or generic workflow engine.

Success means a cold researcher can:

```text
choose Target -> authorize once -> inspect progress -> review outcome
-> decide -> see next obligation
```

without repeated prompts that protect no scientific or authority boundary.

### Gate 4 — measure product value with Harbor directly

The repository retains only Vela-specific task materialization, semantic
answer contracts, and scoring. Standard Harbor task directories own agent,
container, test, verifier, retry, trajectory, and reward execution.

Required comparisons:

- Vela-guided state recovery versus Git plus identical evidence;
- Decision/evidence location time and correctness;
- continuation from an exact changed Frontier root;
- correction impact comprehension; and
- bounded export inspection by an independent reader or Frontier.

A positive claim requires zero authority errors and at least 20 percent median
improvement on its registered primary metric. First-party runs debug the
method; they do not establish adoption or independence.

The first current native Harbor comparison passed its registered
task-specific exactness gate. The next benchmark must test continuation after
a real correction or use a cold independent participant; more harness
infrastructure is not an acceptable substitute.

### Gate 5 — keep Registry and Atlas exact, not expansive

- Git Frontiers remain canonical.
- Source adapters retain native identity, revision, rights, coverage, loss,
  and exact roots.
- The read model is rebuildable and SELECT-only.
- Every projected edge names its source, relation, scope, status, and evidence.
- Atlas views answer only: why does this stand, what does this correction
  affect, and what is the next valid obligation?
- Do not add a graph database, vector store, universal ontology, hosted
  Registry, or new repository without a measured failed query or consumer.

## Verification

Use focused checks during development:

```bash
cargo test -p vela-protocol principal
cargo test -p vela-protocol authority_record
cargo test -p vela-cli --test current_genesis
python3 conformance/verify.py
cargo check -p vela-cli
python3 -m unittest discover -s benchmarks/product-compression -p 'test_*.py'
git diff --check
```

Run the full deterministic release union only for an actual release candidate.
GitHub billing or hosted CI availability does not redefine local correctness;
record it separately as infrastructure state.

## Human checkpoints

Human action is required only for:

1. accepting or rejecting an exact scientific Proposal;
2. changing repository authority, policy, or schema;
3. expanding a bounded campaign's scope, budget, risk, or external effect;
4. publishing or deploying a release; and
5. approving claims of external adoption or independence.

Routine artifact generation, local computation, formatting, replay, scoped
verification, read projection, and report generation should not require
repeated human signatures.

## Stop conditions

Stop the affected work on:

- canonical-history rewriting or unexplained root drift;
- verification presented as acceptance;
- an agent attempting a scientific Decision;
- hidden failed runs or post-output benchmark changes;
- stale, duplicate, or overlapping Targets;
- request-time mutable external data in canonical replay;
- authority credentials reaching an agent or verifier;
- a projection or database becoming hidden authority;
- a new subsystem without a reproduced gap; or
- a public claim not reproduced by its retained evidence.

## Explicit non-goals

Do not build a Vela agent runner, workflow engine, laboratory runtime, theorem
prover, artifact store, graph database, vector database, global truth graph,
universal ontology, hosted authority, second writer, package marketplace,
reputation score, or `1.0.0` schedule.

## Release ledger

| Surface | Current posture | Next earned change |
| --- | --- | --- |
| Vela | `0.961.0` published; focused and release checks green | keep one binary release train; release only for a demonstrated change |
| Cross-language readers | standalone Python and JavaScript conformance readers | keep package-free until a real external consumer requires a library |
| Canopus | `0.8.0` historical evidence | no current source or release train |
| Vela Web | live exact Registry/Atlas on repository v4 and `observatory.v8`; manifest binds deployed commit `f507aa49` and the current projection root | keep projection refresh independent of app version; release web code only for a demonstrated product change |
| Frontiers | canonical Git sources with pending human Decisions | decide or cancel exact Proposals, replay, remap |
| Paper | bounded technical evidence exists | canonical whitepaper only after correction, continuation, and independent-reader gates |

Failure narrows or deletes the system. It does not earn another layer.
