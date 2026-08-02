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

## Current state — 2026-08-02

### Shipped and verified

- Vela `0.962.1` is the published release for repository v4, direct
  Target Index v5, direct Submission lineage, and the retired workflow/runtime
  surfaces. Internal Rust crates are not published as parallel products, and
  the unused TypeScript protocol package has been removed.
- Four mathematical Frontiers use the current repository contract and replay
  from clean Git checkouts.
- Real Erdős, Formal, and quantum Submissions have separate scoped
  Verifications. One Erdős bounded result, one exact Formal Lean result, and
  the exact Quantum `[[10,1,4]]` correction have completed attributed human
  Decisions. The bounded Formal cross-Frontier retention has also completed a
  separate attributed human Decision without importing Erdős authority.
- Four bounded human Decisions have changed Standing and reproduced by replay.
- The retained `[[10,1,4]]` quantum witness has a source-visible alternate
  reconstruction: the historical capsule checked bounded low-weight Pauli
  errors, while the current standard-library verifier derives the complete
  binary-symplectic centralizer and enumerates all 1,536 non-stabilizer logical
  Paulis. The exact-distance-four result, adversarial tests, strict replay, and
  two scoped Verification Records pass. Human Decision event
  `vev_16b21fe1a6d6f064` accepted that exact bounded Claim at commit
  `718de33dcdb27e97e92458530e938f2262c86fbe`. This is algorithmic
  reconstruction, not an independent organization, external participant, or
  second current implementation; the Decision establishes no optimality,
  uniqueness, novelty, classification, or broader scientific acceptance.
- Vela Web has a live root-bound Math Source Registry and read-only Math Atlas
  over the exact four repository-v4 Frontier heads. The current projection uses
  `observatory.v8`, contains no Registration contract, and retains 6,713 native
  source records and 5,844 source bindings. Source Git remains authoritative.
- Product-compression v11 completed four clean native Harbor trials with zero
  retries. Vela-guided work was exact in 2/2 trials while Git/files alone was
  exact in 0/2; median agent time fell from 239.22 to 116.16 seconds and median
  cost from $0.4359 to $0.1880. The compact result is rooted at
  `sha256:c7ebb794bd66f71e20a5eca1a427df12f52d51332610b019cdd897b9943b9063`.
  This is first-party evidence from one frozen pre-Decision quantum correction,
  not a scientific Decision, post-correction remap, independent-user, or
  general scientific-productivity claim.
- The current-head Erdős post-Decision continuation comparison also completed
  four clean native Harbor trials with zero retries. Vela-guided work was exact
  in 2/2 trials while Git/files alone was exact in 0/2; median agent time fell
  from 185.75 to 160.53 seconds and median cost from $0.4730 to $0.3591. The
  compact result is rooted at
  `sha256:e3a2bfafeae5f1573c1e5b95bee1321227fd26984e569e3b60b9ec81cafa409c`.
  This closes one current-head post-Decision continuation gate. It remains
  first-party evidence from one bounded task, not correction propagation,
  independent adoption, general scientific productivity, or an Erdős result.
- The Formal foreign-reference receiver-continuation comparison completed four
  clean native Harbor trials with zero retries. Vela-guided work was exact in
  2/2 trials while Git/files alone was exact in 0/2; median agent time fell
  from 286.07 to 135.34 seconds and median cost from $0.5098 to $0.2306. The
  compact result is rooted at
  `sha256:c0e6b316ce2b446d0b1a05b7f9d1acdb93631b32ae7c2b17d76805a8b650cfda`.
  This closes one exact receiver-continuation gate while the local Proposal
  remains pending a human Decision. It is not independent-user evidence, a
  full correction-inheritance result, or a general productivity claim.
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
- Proposal-scoped reproduction runs Vela-native witnesses directly. When a
  Frontier instead retains a domain-native replay capsule, Vela validates its
  exact Proposal and implementation bindings and returns the native command;
  it does not execute repository code or absorb the domain verifier.
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
- Formal Proposal `vpr_b81d87fce0d9c81c` was producer-withdrawn on 2026-08-01
  after its corrected successor was retained. The withdrawal changed no
  accepted Event or Standing. The corrected cross-Frontier Proposal is now
  accepted alongside the native Lean Claim. Formal replays at commit
  `100d0028bb5b4714ddace4812a77a7ad617ac97c`, repository root
  `sha256:f652b5793e2bcccd2863f24adb7dda3ff3dd707ae64e2de8ee447b37fb1c85e7`.
- Formal Proposal `vpr_08a91ee1b770f5cb` is accepted through attributed human
  Decision events `vev_539148811887822b` and `vev_5491fdcca74f2a98`.
  The accepted Claim is limited to kernel elaboration of the exact retained
  `Erdos835.property_iff_chromaticNumber` proof at the frozen Lean and Mathlib
  revisions. It does not prove `Erdos835.erdos_835`, answer Erdős problem 835,
  establish statement fidelity, novelty, upstream acceptance, or external
  replication.
- Erdős Proposals `vpr_b4a4b9ea9c00d6e9` and
  `vpr_96578d006119b322` bind the same exact producer execution. The latter
  states the exact 11-prime bounded result and scope limit; the former is
  therefore cleanup-only. Their historical producer signing identities are no
  longer retained locally, so producer withdrawal correctly fails closed.
  Resolving either record now requires an attributed human rejection or an
  explicit choice to leave both pending; no agent may delete or relabel them.
- Formal cross-Frontier Proposal `vpr_7aba66544ffefd99` is accepted through
  attributed human Decision event `vev_798955d528dc3030` and applied event
  `vev_973ee78ab0fdfda4`. It retains the exact foreign package without importing
  Erdős authority. The Decision does not establish mathematical truth,
  significance, product lift over Git, a supported shared adapter contract, or
  independent consumer value.
- The current production Atlas checkpoint was rebuilt from all four cleaned
  Frontier heads and is exact at projection root
  `sha256:009ce4e9ac941aa94a74cfd6a20b5a3309693691babc7803abe58cb2293026ba`.
  Its manifest binds Erdős `8428650c`, Formal `100d002`, Quantum
  `718de33d`, and Sidon `d2b7480d`. The Quantum source now projects five
  accepted Claims, zero pending Proposals, two scoped Verifications, and the
  exact repository root
  `sha256:cd6ccf48dc04d5d3a96a185ca16be998f456f9531d975132d7cb910334f0ecdb`.
  Database sync, stored-root verification, SELECT-only reader verification,
  source evidence retention, atomic activation, local build, and production
  deployment all passed. Formal projects 16 accepted Claims, zero pending
  Proposals, four scoped Verifications, and repository root
  `sha256:f652b5793e2bcccd2863f24adb7dda3ff3dd707ae64e2de8ee447b37fb1c85e7`.
- The same release adds the exact `openai/ten-proofs` source adapter to the
  existing 11-source Registry. The bounded `erdos:183:astra-fidelity` work
  now retains a source-bound producer report whose conclusion is `faithful`
  across definitions, quantifiers, hypotheses, and conclusion. Submission
  `vsb_d6301c8383af8bc5` remains pending review as Proposal
  `vpr_3635f052644495be`. Separately scoped first-party Verification
  `vvr_bee06004b4285330`, root
  `sha256:6da941b2e6946f59b85b31df1f2d4bdc2472d8357f654b79952c1b8c21e53428`,
  recomputed the retained roots and six-dimensional fidelity matrix. The
  Proposal still requires a human Decision and changed no accepted Event or
  Standing. Its exact verified producer work is no longer offered again;
  `erdos:1056` is the remaining Erdős producer Target. No Astra Frontier or
  Astra-specific product was created.
- Two empty local PostgreSQL reconstructions matched every production table,
  Frontier, and source-registry root. The retained evidence artifact root is
  `sha256:cd22899c640957cd0096386fec3e1444ab0781402898a1685a94896e97e22544`.
- Proposal sheets now project Vela's exact Decision Inbox packet:
  current and proposed Standing for corrections, protocol readiness, exact
  verified scope, limits, accept/reject consequences, next obligation, and
  copyable roots. The Observatory remains read-only and exposes no Decision
  control. Terminal Decisions expose the attributed reason and exact Decision
  event. Production commit `b99afc34` serves this surface over the current
  rooted projection without client-console errors.
- The current Registry adapter artifact has an immutable GitHub release
  locator and independently reproduced byte root.
- External cold-user lift, correction propagation, and organizationally
  independent interoperability remain unproved. The bounded technical B8
  transfer already passes between distinct Frontier authority keysets with no
  imported Standing. Its attached view targets the current RO-Crate 1.3
  Recommendation and passes the dependency-free parity reader; the retained
  result explicitly records that current `roc-validator` releases do not yet
  ship a 1.3 validation profile.

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

The held-out correction selector now reads the current repository-v4 Frontier
epoch and was rerun after the Quantum Decision against Erdős `8428650c`,
Formal `b706a90b`, Sidon `d2b7480d`, and Quantum `718de33d`. Its retained byte
root is
`sha256:da87f6ebd438a3ef3e46c388ee7ff379a0bab74bc4db1713cf4a46de38952b52`.
The result remains `no_qualifying_candidate`. The selector detects both the
excluded Erdős 424 writer-qualification case and the real Quantum correction;
the latter has no hard dependent, support diamond, or non-consequential
relation in the frozen graph. The campaign records the negative gate, runs no
reader trial, and does not manufacture a synthetic substitute or graph edge.

The first current native Harbor comparison passed its registered
task-specific exactness gate. A second comparison now tests current-head
continuation after a real authorized bounded transition. The next benchmark
must test a real correction with downstream inheritance or use a cold
independent participant; more harness infrastructure is not an acceptable
substitute.

The Quantum supersession comparison passed its registered task-specific gate:
all four native Harbor trials were eligible, the Vela-guided arm was exact 2/2,
and Git/files was exact 0/2. Median agent time was 116.16 seconds guided versus
239.22 seconds baseline, and median cost was $0.1880 versus $0.4359. The compact
result root is
`sha256:c7ebb794bd66f71e20a5eca1a427df12f52d51332610b019cdd897b9943b9063`.
This remains a bounded first-party pre-Decision robustness result. The later
human Decision does not retroactively turn those trials into a post-correction
or independent-participant result.

The Erdős post-Decision continuation comparison also passed its registered
task-specific gate: all four native Harbor trials were eligible, the
Vela-guided arm was exact 2/2, and Git/files was exact 0/2. Median agent time
was 160.53 seconds guided versus 185.75 seconds baseline, and median cost was
$0.3591 versus $0.4730. The compact result root is
`sha256:e3a2bfafeae5f1573c1e5b95bee1321227fd26984e569e3b60b9ec81cafa409c`.
One Git/files attempt confused the human `review.accepted` event with the
separate `finding.asserted` event. The other found the Decision but incorrectly
extended the later producer-complete range back to the accepted boundary. Vela
exposed the exact accepted range, later verified-but-pending producer
completion, and exact next Target without either error. This is a bounded
post-Decision continuation result, not the still-open correction-inheritance
or independent-participant proof.

The Formal foreign-reference receiver-continuation comparison passed the same
task-specific gate: all four native Harbor trials were eligible, the
Vela-guided arm was exact 2/2, and Git/files was exact 0/2. Median agent time
was 135.34 seconds guided versus 286.07 seconds baseline, and median cost was
$0.2306 versus $0.5098. The compact result root is
`sha256:c0e6b316ce2b446d0b1a05b7f9d1acdb93631b32ae7c2b17d76805a8b650cfda`.
The task required the agent to distinguish accepted foreign source Standing
from a pending local Proposal and recover the exact Decision packet. It closes
a second task class, but remains first-party evidence and does not satisfy the
held-out correction-impact or independent-participant gate.

The next independent-product gate is frozen in
`paper/artifacts/cold-reader-study/plan.v1.json`, byte root
`sha256:1fb12c44aa8fce0c2f3d84c70ea0a44574b6d78be92d848431d125919cf62d86`.
It uses four external readers, the accepted Quantum correction, and the
accepted Formal cross-Frontier retention in a two-task counterbalanced
Git-versus-Observatory crossover. The semantic answer key was frozen before
participant one at byte root
`sha256:25df84114e86970d8c77b7ce3774f0ad8acd2a2fcb1645e54e6c8ad2201d6f7f`.
No new runner, service, model call, or mutation surface is required. A positive
result supports only bounded external product-comprehension and continuation
claims; the failed held-out correction-topology gate still prevents a protocol-
breakthrough claim.

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

One bounded external-release pilot is earned by OpenAI's August 2026
`ten-proofs` release because it directly exercises the existing source,
verification, and fidelity boundaries. The observed release is pinned to Git
commit `29362184c2b698c1b279bc85b3957ee813646c63` and tree
`730bf2c6a13dbb96606024c5fd681a48633fb393`; the moving upstream `main` is not
an acceptable identity. The release contains 43 Git blobs and twelve declared
Comparator profiles under Apache-2.0. Its Erdős #183 slice binds:

- `I_MulticolorTriangleRamsey.json` and its generated challenge;
- `MulticolorTriangleRamsey.lean` and the theorem family rooted at
  `ErdosProblems.MulticolourTriangleRamsey.erdos_183`;
- Lean `v4.32.0`; and
- the exact Comparator, Mathlib, Lean4Export, and Lean4Checker revisions in the
  retained Lake manifest.

The current independently acquired `teorth/erdosproblems` source is pinned to
commit `8138974387d9030542daabe67faaa33eff9356f8`, tree
`7ed44c260d7eb63a067cf5a16afdb645d494ef06`, and source-file SHA-256
`a4358d57b591fc92c75981c160a11f43a561de6b5e8478d8f9629511759a9213`.
It still labels problem 183 `open` and `unformalized`. That disagreement is a
source-state conflict for inspection; it is not evidence that either source
governs the other or that Vela may silently change Standing.

The pilot therefore:

1. observe one exact release through the existing source-adapter contract;
2. pin the native Git commit/tree, Lean toolchain, dependency lock, declared
   review state, and all twelve source-declared Comparator profiles;
3. run `lake build All`, then one Erdős #183 Comparator profile under real
   Linux Landrun with an outer offline, unprivileged container boundary;
4. record checker passage with explicit nonclaims about statement fidelity,
   novelty, community acceptance, and local Standing;
5. acquire the current Erdős #183 source through the exact adapter before
   claiming a source-state conflict; retrieval failure is recorded as
   unavailable, never inferred as `OPEN`; and
6. create one fidelity Target only if exact source acquisition succeeds.

This pilot adds no protocol object, Frontier, runner, Astra-specific product
page, universal status ladder, reviewer service, or global acceptance claim.
The local macOS Comparator development shim is not a hardened sandbox. The
Linux execution now passes under real Landrun/Landlock plus an offline,
capability-free Docker boundary, but uses one content-hashed argv wrapper to
restore the nested delimiter that Landrun strips. It is not the literal native
`systemd-run` guarantee documented by Comparator and is not an independent
scientific Decision. The other eleven profiles and a general release-manifest
export remain deferred until a real consumer earns them.

The first local reproduction is now complete: `lake build All` passed 8,666
jobs and the pinned Erdős #183 profile passed both Lean's default kernel and a
pinned Nanoda build. The exact environment, byte roots, and nonclaims are in
`paper/artifacts/astra-erdos-183/result.v1.json` (SHA-256
`cd38ac37a3abd04c045e2905886fa418155a1838cb755bc351f96341a84179cd`).
The second Linux execution passed both kernels and Comparator under real
Landrun/Landlock with no network, no capabilities, no host checkout, and an
ephemeral filesystem. Exact environment and wrapper roots are retained in the
same result. No Frontier state changed.

The August 1 Astra strategic memo is adopted only through this existing seam.
Its central observation sharpens the campaign thesis: candidate discovery is
becoming abundant, while exact source identity, statement fidelity, scoped
verification, correction-aware Standing, and a safe next obligation remain
scarce. The `ERDOS-183-ASTRA-FIDELITY` producer pass compared the pinned
teorth statement, OpenAI manuscript theorem, and Lean declaration and found
no definition, quantifier, hypothesis, or conclusion mismatch. The report is
rooted at
`sha256:dc40f2221ab2a2e0101e328026f1a4bd6a439c47e9c215677deb671ee42da368`.
Its Claim now has a separately scoped first-party Verification that recomputed
the source roots and report matrix. Verification `vvr_bee06004b4285330`
passes with explicit shared model, operator, and machine limitations. It does
not establish novelty, community acceptance, or external independence, and it
does not choose a human Decision or change Standing.

The resulting Astra scorecard is deliberately narrower than the release:

| Layer | Exact result | Claim ceiling |
| --- | --- | --- |
| Source identity | one immutable OpenAI commit/tree, one exact teorth observation, and one retained Erdős statement snapshot | the observations are source- and time-bound; neither source governs the other |
| Reproduction | 1 of 12 declared Comparator profiles passed Lean, Nanoda, and the retained hardened Linux path | no claim about the other eleven profiles or the whole release |
| Statement fidelity | 1 of 1 selected statements received a six-part producer matrix, zero recorded discrepancies, and one separately scoped first-party Verification | shared model family, operator, and machine do not establish external independence |
| Novelty and mathematical review | 0 external novelty reviews and 0 independent mathematical re-proofs | kernel passage and fidelity review do not establish novelty, importance, or consensus |
| Authority | 0 human Decisions, 0 accepted Events, and 0 Standing changes | the pending Proposal is evidence awaiting an attributed human choice, not an accepted result |
| Product value | four first-party cold-agent Harbor trials completed; 0 of 4 exact | the registered gate failed, so no usability, adoption, or reviewer-efficiency claim |

The registered product test preserved those denominators. It measured whether
a cold agent could locate the exact source observation, distinguish reproduced
checking from statement fidelity and Standing, identify the source-timing
disagreement, and recover the pending Proposal's Decision packet plus next
valid obligation. For this class of external release, the operating rule is:
reference broadly, snapshot selectively, admit narrowly.

The frozen Harbor study completed with all four trials eligible but neither arm
exact. The Vela-guided arm was 51.09 percent faster and 66.12 percent cheaper
at the median, but it receives no product-lift credit. Both guided answers
recovered nearly all frozen facts; one omitted the explicit `pending_review`
field, the other added a truthful limit, and both paraphrased a prose field that
the frozen scorer required byte-for-byte. The result remains failed. A future
study must encode semantic distinctions as bounded fields before execution;
the scorer cannot be relaxed after outputs exist. The compact result root is
`sha256:371f341311d1f1a3bbc850594a90dd0a1627e655308635d2ffa87b3081a2e823`.

Do not create an Astra Frontier, universal status ladder, reviewer service,
source-monitoring daemon, graph store, or standalone Scientific Release
Manifest schema. Do not run the other eleven Comparator profiles for a vanity
count. A broader export or product surface is earned only by a named second
consumer. Do not rerun the failed task merely to seek a favorable score.

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
| Vela | `0.962.1` is published with direct producer withdrawal, compacted-lineage reconstruction, and current-root Decision checks; release checks are green | keep one train and release only for demonstrated changes |
| Cross-language readers | standalone Python and JavaScript conformance readers | keep package-free until a real external consumer requires a library |
| Canopus | `0.8.0` historical evidence | no current source or release train |
| Vela Web | production Registry/Atlas uses repository v4 and `observatory.v8`; deployment commit `e0e874e0` binds projection root `sha256:009ce4e9ac941aa94a74cfd6a20b5a3309693691babc7803abe58cb2293026ba`, 11 exact sources, 6,713 native records, current/proposed Standing Decision packets, the terminal Quantum and Formal Decisions, and current-head clean-room artifact `sha256:cd22899c640957cd0096386fec3e1444ab0781402898a1685a94896e97e22544` | keep the exact source projection; add no Astra-specific UI or broader execution until a real consumer earns it |
| Frontiers | canonical Git sources; the Quantum correction and native Formal Lean result are accepted and replayed, the obsolete Formal duplicate is withdrawn with zero accepted delta, and remaining Proposals await attributed human Decisions or cancellation | decide or cancel exact remaining Proposals, replay, remap |
| Paper | bounded technical evidence exists | canonical whitepaper only after correction, continuation, and independent-reader gates |

Failure narrows or deletes the system. It does not earn another layer.
