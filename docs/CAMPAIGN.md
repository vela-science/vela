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
3. Agents cannot accept, reject, or cancel a Proposal.
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

## Current state — 2026-07-31

### Shipped and verified

- Vela `0.950.1` and `@vela-science/protocol@0.1.0` are immutable releases.
- Four mathematical Frontiers use the current repository contract and replay
  from clean Git checkouts.
- Real Erdős, Formal, and quantum Submissions have separate scoped
  Verifications while their accepted-state delta remains zero.
- A prior real human Decision changed Standing exactly once and reproduced by
  replay.
- Vela Web has a root-bound Math Source Registry and read-only Math Atlas over
  the four Frontiers. The live deployment manifest, not this document, is the
  authority for its current projection root and deployment identity.
- The corrected native Harbor product-compression result is retained at
  `paper/artifacts/product-compression-v3/`. It is first-party directional
  evidence, not adoption evidence.
- The immutable Canopus `0.8.0` release and tag remain historical evidence.

### Simplified current source

- Current Vela ships no agent runner, Campaign host, scheduler, custom Run
  receipt model, or Canopus compatibility layer.
- `vela start` creates only an ignored local Attempt lease over one exact
  Target, expiry, permitted artifact classes, and bounded evidence writes.
- Agents and workbenches consume the Target packet directly and register a
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
- The Formal cross-Frontier transfer remains pending its held-out consumer and
  value test.
- The production Atlas must be refreshed whenever canonical Frontier heads
  change; source Git remains authoritative during projection lag.
- Registry adapter artifacts need retained immutable locators before exact
  long-term reconstruction can be claimed.
- External cold-user lift, correction propagation, and independently governed
  interoperability remain unproved.

## Active gates

### Gate 1 — keep the core small and green

- Remove the bundled runner and unused capability protocol completely.
- Keep the current Attempt contract small and current-only; do not migrate
  ignored pre-release local state.
- Remove stale commands, tests, workflows, package scripts, and active docs.
- Verify protocol conformance, CLI compilation, current repository bootstrap,
  clean replay, and the TypeScript package.
- Do not release or bump versions merely to synchronize metadata.

### Gate 2 — complete one clear scientific loop

For one real pending Proposal:

1. show the exact proposed Claim and scope;
2. show its Submission, decisive artifacts, verifier requirements, and scoped
   Verification results;
3. show what Standing would change and what would remain unchanged;
4. let the human authority accept, reject, or cancel;
5. verify the exact before/after roots and replay;
6. rebuild the read projection; and
7. expose the next valid Target or explicit absence of one.

No agent performs step 4. A verifier pass cannot select the outcome.

### Gate 3 — compress the long-running workflow

The target experience is one bounded authorization followed by hours of native
agent work and one or a few consequential review items.

The minimum product seam is:

- an expiring local lease over one Frontier, Target, allowed evidence classes,
  duration, and spend/volume limits;
- continuous native work and append-only evidence lineage without a Vela-owned
  runner;
- a read-only Decision Inbox containing only changes to Standing, scope,
  budget, policy, external publication, or destructive action; and
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
bun install --frozen-lockfile
bun run check
git diff --check
```

Run the full deterministic release union only for an actual release candidate.
GitHub billing or hosted CI availability does not redefine local correctness;
record it separately as infrastructure state.

## Human checkpoints

Human action is required only for:

1. accepting, rejecting, or cancelling an exact scientific Proposal;
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
| Vela | `0.950.1`; current simplification is unreleased | compatible release only after green checks and a user-visible defect fix |
| TypeScript protocol | `0.1.0` immutable | publish only a contract change from a Vela tag |
| Canopus | `0.8.0` historical evidence | no current source or release train |
| Vela Web | root-bound Registry and Atlas deployed; source adapter contract consolidated on `main` | refresh exact production projection, then earn product changes through cold-use evidence |
| Frontiers | canonical Git sources with pending human Decisions | decide or cancel exact Proposals, replay, remap |
| Paper | bounded technical evidence exists | canonical whitepaper only after correction, continuation, and independent-reader gates |

Failure narrows or deletes the system. It does not earn another layer.
