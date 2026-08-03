# Active Vela campaign

## Objective

> Prove or falsify Vela's action-complete Frontier thesis through one
> native-agent, correction-aware campaign: a cold researcher selects an exact
> Target, authorizes ordinary agent work once, receives bounded Submission and
> Verification evidence without repeated Vela ceremony, reviews one
> consequential Decision, replays the resulting Standing, hands the exact next
> obligation to a different producer, and measures causal lift against matched
> flat-state baselines—without adding a Vela runner, hosted Registry, graph
> authority, or automatic Decision.

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

## Current state — 2026-08-03

### Shipped and verified

- Vela `0.963.0` is the published current-contract release. It carries the
  TOML-only Frontier Profile, standards spine, direct
  Target Index v5, direct Submission lineage, and retired workflow/runtime
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
  binary-symplectic centralizer and enumerates all 1,536 phase-free
  centralizer-minus-stabilizer representatives. The exact-distance-four result,
  adversarial tests, strict replay, and
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
  accepted alongside the native Lean Claim. The Decision checkpoint is commit
  `100d0028bb5b4714ddace4812a77a7ad617ac97c`; the current Frontier replays at
  commit `2d28519aaaf1003070703ad85edf4d1d28cf5839`, repository root
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
- Erdős Proposal `vpr_635f1e6c1811f48c` retains the exact bounded negative
  result over `10430401..10430600`: 13 primes, maximum multiplicity 11 at
  `p = 10430491`, residue `4382886`. Requirement-scoped Verification
  `vvr_c99f40656e9fc08c` and the structurally independent sieve/direct-array
  Verification `vvr_66b3e2ed0278eb0b` both passed with zero accepted-event
  delta. An attributed human Decision accepted only that bounded Claim through
  event `vev_37fa3d9f9be64e58` and applied event `vev_f1f961cb81e75490`.
  Strict replay at commit `1d191a3f65bae4b85aa5db58eb4f43cb5e6b94b2`
  yields repository root
  `sha256:e9b67253e139e2fd365aa0f7dce0949f898174895f1602c59fd6c5b0d862c443`;
  the exact next nonoverlapping range is `10430601..10430800`.
- Formal cross-Frontier Proposal `vpr_7aba66544ffefd99` is accepted through
  attributed human Decision event `vev_798955d528dc3030` and applied event
  `vev_973ee78ab0fdfda4`. It retains the exact foreign package without importing
  Erdős authority. The Decision does not establish mathematical truth,
  significance, product lift over Git, a supported shared adapter contract, or
  independent consumer value.
- The current Neon Atlas head was rebuilt from all four cleaned Frontier heads
  and is exact at projection root
  `sha256:8bc68a34296b7e33bee7ca2321333bf84ea9d6b96867b55dd2c64ff85394917e`.
  Its deployed manifest binds Vela `0.963.0`, Erdős `1d191a3f`, Formal
  `2d28519`, Quantum `d02f260`, and Sidon `8c7bcbf`. The Quantum source projects five
  accepted Claims, zero pending Proposals, two scoped Verifications, and the
  exact repository root
  `sha256:cd6ccf48dc04d5d3a96a185ca16be998f456f9531d975132d7cb910334f0ecdb`.
  Database sync, stored-root verification, SELECT-only reader verification,
  source evidence retention, atomic activation, and production deployment
  passed. Formal projects 16 accepted
  Claims, zero pending Proposals, four scoped Verifications, and repository root
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
  Standing. Its exact verified producer work is no longer offered again. The
  later native `erdos:1056` range is now also producer-complete and no longer
  appears in `vela next`; its Proposal remains pending. No Astra Frontier or
  Astra-specific product was created.
- Two empty local PostgreSQL reconstructions matched every production table,
  Frontier, and source-registry root. The retained evidence artifact root is
  `sha256:6a2609b8afe15a5398b0410cb7456b6831b72cec1e67b90e9553d907c886e6d2`.
  It binds Vela `0.963.0`, the released macOS binary root, all four current
  Frontier commits, the current source-adapter artifact, and two byte-identical
  empty-database reconstructions. The frozen adapter input proves deterministic
  reconstruction, not future reacquisition from mutable upstream sources.
- Proposal sheets now project Vela's exact Decision Inbox packet:
  current and proposed Standing for corrections, protocol readiness, exact
  verified scope, limits, accept/reject consequences, next obligation, and
  copyable roots. The Observatory remains read-only and exposes no Decision
  control. Terminal Decisions expose the attributed reason and exact Decision
  event. Production commit `ab825cfe` serves this surface over the current
  rooted projection without client-console errors and resolves exact Proposal
  IDs through the root-bound search API rather than a truncated preload.
- The current Registry adapter artifact has an immutable GitHub release
  locator and independently reproduced byte root.
- External cold-user lift, correction propagation, and organizationally
  independent interoperability remain unproved. The bounded technical B8
  transfer already passes between distinct Frontier authority keysets with no
  imported Standing. Its attached view targets the current RO-Crate 1.3
  Recommendation, carries the exact source-diff, predecessor, successor, and
  full-index patch, and passes the independent Python parity and transition
  readers. A closed fixity manifest binds the package payload without defining
  a Vela-specific archive format. The retained result explicitly records that
  current `roc-validator` releases do not yet ship a 1.3 validation profile.

## Next big pass — action-complete Frontier proof

The next pass is not another architecture train. It is one causal product
demonstration built from the current system:

> A cold researcher selects one exact Target, authorizes a native agent once,
> receives bounded evidence without repeated Vela ceremony, reviews one
> consequential scientific change, replays the resulting Standing, and hands
> the exact next obligation to a different producer.

The pass must prove or falsify the stronger thesis behind the current bounded
results:

> Under matched information, tools, model, and compute, an action-complete,
> correction-aware Frontier causes more exact, reusable scientific progress
> per unit of scarce expert judgment than flat Git files and retrieval alone.

### Phase A — freeze the causal baseline

Before a new run:

1. bind the exact Vela binary, four Frontier commits and roots, Observatory
   projection root, Harbor version, model identity, native agent, environment,
   scorer, and benchmark plan;
2. record the current Erdős Target `erdos:1056`, range
   `10430601..10430800`, packet root, verifier, accepted boundary, and six
   pending Proposals;
3. record that Formal, Quantum, and Sidon currently return an explicit
   no-Target result rather than inventing producer work; and
4. freeze the matched baseline from the same canonical Git and evidence bytes.

The baseline receives the same scientific information as the Vela arm. The
only treatment is the action-complete Target, Standing, evidence, correction,
and next-obligation projection supplied by Vela.

Phase A is frozen in
[`paper/artifacts/action-complete-frontier-2026-08-03`](../paper/artifacts/action-complete-frontier-2026-08-03/README.md)
at baseline root
`sha256:46f931b202618ef6437a23f0c49f9172cafa739c1b1b69465f5171f1caa39a4c`.
The freezer independently confirmed all four strict-replaying heads, the live
read-only projection, Erdős's one exact next range, three explicit no-Target
states, Harbor custody, and the benchmark implementation roots without writing
any Frontier.

### Phase B — complete the native-agent vertical slice

Use Codex or another native agent with its normal OAuth, durable session, and
native action policy. Vela supplies no runner, lease database, scheduler,
transcript store, or approval loop.

The live slice is:

```text
choose erdos:1056
-> inspect the exact Target packet
-> authorize ordinary native work once
-> compute and retain the exact bounded artifact
-> submit producer-authenticated evidence
-> import one requirement-scoped Verification
-> inspect one consequence-bearing Decision packet
-> human accept, reject, or leave pending
-> replay and rebuild the read projection
-> hand the exact next obligation to a fresh producer
```

Routine computation, artifact writes, local tests, Submission, Verification,
replay, and report generation must not ask for repository authority. Native
approval remains appropriate for external publication, destructive actions,
scope or spend escalation, and sensitive tools. Only the scientific Decision
changes Standing, and no agent performs it.

The slice passes only if:

- the agent performs no stale, duplicate, or overlapping range;
- the Claim and caveat describe the exact retained scope;
- Submission and Verification change zero accepted Events;
- the Decision packet binds current read roots and is stale-safe;
- the attributed human Decision, if made, replays exactly;
- the projection advances only after canonical Git advances; and
- a fresh producer receives the exact successor Target or an explicit blocker.

#### Live result — awaiting the human Decision

The native slice has now reached its single authority checkpoint. Vela's first
briefing exposed obsolete runner metadata; the live Erdős Target was repaired
to retain only a Frontier-owned scientific producer profile, result contract,
and frozen verifier. The prescribed model, token budget, Canopus image, Agent
mission, and execution bundle were removed. `vela start` is again a compact,
write-free Target briefing.

A native C++ producer then searched every prime in `10430601..10430800` in
2.16 seconds with no Vela runner. The retained artifact reports 15 primes,
maximum multiplicity 11 at `p = 10430729`, residue `5661996`, and root
`sha256:35c1e28a62478957014a14fb3360ee26bdff474af825b97aa0135835710f8058`.
The separately frozen Linux ARM64 verifier recomputed the range under a
network-denied, read-only container and matched the exact bytes.

Submission `vsb_50298472b83a63a0` created Proposal
`vpr_4fa1a06ca64e36e4` with zero accepted-Event delta. The scoped verifier
record also changed zero accepted Events. An accidental command retry exposed
and repaired two concrete workflow defects: failed publication preflight now
aborts its marker-free transaction automatically, and semantically identical
Verification authoring now reuses the retained record rather than minting
timestamp-only duplicates. The two already retained retry records count as one
semantic verification, not independent methods or organizations.

The exact read-only packet is retained at
[`decision-packet.erdos-1056.v1.json`](../paper/artifacts/action-complete-frontier-2026-08-03/decision-packet.erdos-1056.v1.json).
Its protocol gate is satisfied and Proposal Standing remains
`pending_review`. It makes no recommendation. Only the human repository
authority may accept, reject, or leave the Proposal pending. Replay, projection
refresh, successor handoff, and the matched Harbor evaluation remain gated on
that choice.

The post-Verification lifecycle has also been repaired and frozen. Exact
passing evidence now closes producer work without changing scientific
Standing, so the completed range is not offered again while its Proposal sits
in the Decision Inbox. At Erdős commit `4e863be985bf0153fd9b911fbc7a31e96c8b15bd`,
strict repository verification passes, Target Index root
`sha256:7bb576d03cb347e6ab3b9e8fe683641226e8e9a1dd49237db33e065d4f1f0d9a`
contains zero producer Targets, and Proposal `vpr_4fa1a06ca64e36e4`
remains `pending_review`. This is the intended pre-Decision state: no duplicate
work and no accidental acceptance.

### Phase C — preregister the matched Harbor evaluation

Retain one benchmark implementation: native Harbor task directories plus the
small Vela-specific materializer and semantic scorer already under
`benchmarks/product-compression`. Do not add a Vela harness, Canopus successor,
custom trajectory format, or parallel run database.

Use heterogeneous task classes:

1. **Target continuation** — recover the exact next bounded action after a
   Decision without crossing accepted, producer-complete, or pending ranges.
2. **Standing discrimination** — distinguish Submission, passing Verification,
   pending Proposal, accepted Decision, and current Standing.
3. **Cross-Frontier inheritance** — inspect a bounded foreign package without
   importing the origin Frontier's authority.
4. **Correction impact** — compute surviving, invalidated, and newly blocked
   obligations from a closed-ground-truth correction fixture.
5. **Explicit absence** — return the exact blocker when a Frontier has no
   configured Target instead of inventing work.

The correction fixture is a controlled product benchmark, not evidence that a
real Frontier correction with downstream topology has occurred. The existing
held-out selector remains the gate for that real claim and currently reports
no qualifying candidate.

Every comparison must:

- freeze the plan before outputs exist;
- use fresh sessions and randomized arm order;
- hold model, tools, source bytes, verifier, retry policy, and environment
  constant;
- give the baseline every underlying scientific fact;
- forbid answer-key or scorer leakage;
- retain every eligible failure; and
- allow only a recorded infrastructure retry before model output.

Two fresh runs per arm are instrumentation pilots only. They establish that
the task, scorer, and telemetry work; they cannot support a performance claim.
The confirmatory sample size is computed from blinded pilot variance for 80%
power at a two-sided 5% error rate and a preregistered 20% minimum useful
effect. There is no arbitrary token or model-call budget. Cost and observed
tokens are outcomes, while each task retains the same bounded scientific and
compute ceiling in both arms.

### Phase D — measure compounding, not activity

Primary gates:

- **Exact Transition Yield (ETY):** exact, policy-valid Frontier transitions
  per attempted task;
- **Verified Progress per All-In Cost (VPAC):** verifier-passing bounded
  results divided by compute cost plus expert minutes;
- **Frontier Inheritance Effect (FIE):** improvement when continuing from a
  rooted Frontier rather than flat source and evidence;
- **Cross-Producer Inheritance (CPI):** probability that a fresh producer takes
  the exact valid next action without hidden context; and
- **Correction resilience:** impact precision/recall, stale-state use, false
  pruning, surviving-route recovery, and repair time.

Supporting measures are time to first correct action, completion rate, exact
replay rate, stale or duplicate work, claim overstatement, expert
interruptions, expert minutes, observed tokens, wall time, and cost.

No positive Vela claim is allowed unless:

- both arms satisfy the same correctness and authority contract;
- Vela has zero verification-as-acceptance or unauthorized-Decision errors;
- the registered primary metric improves by at least 20%;
- the result survives a fresh producer or model swap; and
- the retained report includes failures, exclusions, uncertainty, and exact
  roots.

If the result is neutral or negative, publish the bounded result and simplify.
Do not respond by adding a runner, graph store, service, or ontology.

### Phase E — earn only the minimum product change

Run a cold-use observation against the existing CLI and Observatory before
changing either. The current product already exposes the exact Erdős Target,
bounded objective, verifier identity, pending Decisions, replay state, and next
range. A product change is earned only by a measured comprehension or
continuation failure.

Likely seams, in priority order, are:

1. one copyable native-agent briefing from the exact Target packet;
2. one Decision packet that makes the Standing diff, decisive evidence,
   uncertainty, consequences, and next obligation legible;
3. one post-Decision handoff showing the exact successor Target; and
4. one evidence-lineage/correction-impact view when it answers a demonstrated
   user question faster than the current record view.

The Observatory remains a read-only projection. Agent progress and
informational receipts stay in the native executor; they do not enter the
Decision Inbox. No graph position, ranking, verifier result, or model output
implies authority.

### Phase F — publication and expansion gates

After the live loop and confirmatory evaluation:

- refresh the evidence companion with exact plans, roots, failures, and
  reproducibility;
- revise the canonical whitepaper only to the claim ceiling actually earned;
- release Vela or Vela Web only for demonstrated code or product changes; and
- keep Registry, Atlas, interoperability adapters, and package extraction
  bounded by named consumers.

The first package candidate remains exact Lean replay, but it is a read-only
duplication audit rather than a package project. Extract it only after two
maintained consumers exist and the extraction deletes more duplicated code
than it adds. Do not create a package registry, package manager, new
repository, hosted service, or public CLI ahead of that evidence.

### Completion contract

The pass is complete when all of the following are true:

1. one real Target completes the native-agent evidence-to-Decision-to-remap
   loop without repeated Vela ceremony;
2. the next valid obligation survives a fresh producer or model handoff;
3. the preregistered Harbor comparison produces a valid positive, neutral, or
   negative result across the heterogeneous task classes;
4. a controlled correction benchmark passes or fails transparently, while the
   real-correction selector remains honest;
5. the CLI and read projection correctly return either one valid Target or an
   explicit blocker for every canonical Frontier;
6. no new runner, authority surface, canonical database, graph store, or
   package ecosystem is introduced; and
7. the paper, roadmap, manifests, and public claims match the retained evidence
   exactly.

## Research synthesis and work disposition — 2026-08-03

The recent architecture, product, standards, Registry, package, Astra, and
action-complete-Frontier memos converge on one program rather than several
parallel products. The durable Vela control point is the exact transition from
evidence and scoped Verification to a named authority's correction-aware
Standing, followed by a safe handoff. Every other idea is either a native-tool
integration, a disposable read projection, a benchmark, or an earned future
package.

This disposition folds in the July 29–August 2 canonical-thesis, Math Atlas,
human-agent UX, Frontier-repository, action-complete-Frontier, Astra,
standards/tooling, and package/toolchain memos. Earlier broader proposals remain
research context; this campaign is the executable intersection that survived
their deletion and evidence gates.

| Research stream | Current disposition | Evidence gate |
| --- | --- | --- |
| Action-complete Frontier | **Active product and research proof.** Finish the live Erdős Decision, replay, remap, and fresh-producer handoff. | One native end-to-end loop with no duplicate Target, no authority error, and an exact successor or blocker. |
| Human-agent workflow | **Use native agent durability and action approvals.** Vela supplies a Target briefing and a consequence-only Decision Inbox. | One authorization supports ordinary work until one meaningful scientific Decision; no Vela runner or transcript store. |
| Harbor evaluation | **Keep as the sole execution/evaluation harness.** Vela owns only exact fixture materialization and semantic scoring. | Valid matched pilots, power-derived confirmation, full failure retention, and at least two task families before a general lift claim. |
| Observatory and Math Atlas | **Keep as root-bound read projections.** Refresh after canonical Git changes; change UI only for a measured comprehension failure. | Exact source roots, SELECT-only reads, no hidden authority, and at least 20 percent improvement for any new product surface. |
| Astra and other scientific releases | **Treat as source observations, not new Frontiers.** The OpenAI release adapter and fidelity work are the current case. | Exact source binding, clean-room native checks, explicit fidelity/novelty obligations, and bounded local Decisions. |
| RO-Crate 1.3 and provenance standards | **Keep as derived, loss-explicit interoperability.** | An independent reader can inspect the package without importing origin authority; no transfer format changes Standing. |
| Vela package subsystem | **Long-range, earned layer; not part of this active pass.** Start only with a source-local exact Lean replay experiment. | Two maintained consumers, cross-platform root agreement, adversarial conformance, net deletion of duplicate code, and zero authority effect. |
| Hosted Package Registry, global graph, workflow engine, agent runtime, package marketplace | **Deferred or rejected.** | Reconsider only after repeated external demand and a named failure that native tools plus a thin adapter cannot solve. |
| Standards hardening | **Correctness lane, not a parallel product train.** Complete only changes required for portable evidence and clean replay. | Root-preserving conformance or one explicit pre-1.0 cut, clean-clone replay, then deletion of superseded runtime code. |

### Ordered work after the human Decision

1. **Replay and remap.** Verify the exact Decision transaction, preserve the
   pre-Decision evidence, rebuild the Target Index and read projection, and
   prove that the next bounded obligation begins after `10430800`.
2. **Fresh-producer handoff.** Give a different producer only the rooted
   Frontier and native tools. It must identify the exact successor or blocker
   without prior session state.
3. **Five-class Harbor pilot.** Run matched `git-files` and `vela-guided`
   tasks for Target continuation, Standing discrimination, cross-Frontier
   inheritance, controlled correction impact, and explicit Target absence.
   Two attempts per arm validate instrumentation only.
4. **Power-derived confirmation.** Use blinded pilot variance to determine
   repetitions. Confirm on at least two scientific task families and one
   producer or model-family swap. Retain all eligible failures.
5. **Real correction-and-inheritance case.** Select a qualifying correction
   with closed downstream ground truth. Measure affected-set precision and
   recall, surviving-route recall, stale-use rate, repair time, and the next
   valid obligation. Do not substitute a synthetic fixture for this claim.
6. **Product compression test.** Observe a cold researcher using the existing
   CLI and Observatory. Earn only the smallest change that reduces time to the
   exact evidence, Standing diff, or next obligation by at least 20 percent.
7. **Evidence and publication.** Publish a concise result—positive, neutral,
   or negative—with exact roots, costs, failures, uncertainty, and scope. Only
   then revise the canonical whitepaper and activate an earned follow-on goal.

### Thesis promotion gates

The current active goal remains the bounded action-complete proof. A broader
"compounding scientific state" claim is promoted only after all of the
following hold:

- positive Frontier Inheritance Effect with uncertainty excluding no
  improvement in at least two task families;
- at least 20 percent VPAC lift over the strongest same-information baseline;
- positive cross-producer inheritance under a producer or model-family swap;
- exact recovery from one real correction with no stale accepted use, false
  pruning, or lost independent route;
- one consequential scientific result or correction and one separately
  controlled reproducer; and
- no simpler flat packet or retrieval arm matching the whole result.

Until those gates pass, Vela may claim exact scientific-state, replay,
correction, and bounded task-comprehension value. It may not claim a general
scientific-discovery breakthrough, external adoption, or a need for a hosted
Registry.

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
| Cedar policy engine and material | Delete in the pre-1.0 cut | Replace with the closed Vela Authorization Profile over AuthZEN's standard information model. Historical Git and the pinned predecessor binary retain old verification; no dual runtime. |
| Crash journal and Git compare-and-swap publication | Keep | Protect partial-write recovery and concurrent publication, not workflow ceremony. |
| Harbor jobs, OAuth, retries, trajectories, timing, and cost | External | Harbor already owns benchmark execution. Vela keeps only its task fixture and semantic scorer. |
| Neon branches | Delete/defer | One `main` read projection is enough; rehearse locally in disposable PostgreSQL. |
| Agent Campaign runtime, scheduler, or transcript store | Reject | Native agents own execution and durable approval state. Vela exposes consequential pending Decisions only. |

The proposed pre-1.0 standards cut is recorded in
[ADR 0035](adr/0035-commodity-encoding-signing-and-wire-contracts.md). It
replaces `vela.canonical-json/v1` with exact RFC 8785 JCS, replaces bespoke
signed-record preimages with a common DSSE envelope, publishes JSON Schema
2020-12 for portable objects, replaces Cedar with a two-role closed authority
profile using AuthZEN's subject/action/resource/context/decision shape, and
retains RO-Crate 1.3 as a derived evidence-transfer adapter. It must begin with a
dual-encoding audit of every retained Frontier object; it may not silently
rewrite roots or old signatures.

The frozen 2026-08-02 parsed-value shadow audit recursively rejected duplicate
JSON properties and bound exact commits, trees, counts, raw exceptions, and a
canonical result root in `conformance/jcs-shadow-audit.v1.json`. On those four
clean Frontier heads, 3,158 of 3,161 parsed tracked JSON values and all seven
decoded authority payloads were JCS-byte-identical. Three exact raw Erdős
evidence artifacts differed and remain byte-preserved evidence rather than JCS
protocol objects; one contains all 17 observed unsafe integers. Vela now uses
the pinned RFC 8785 implementation in production, rejects duplicate properties
recursively and unsafe protocol integers before hashing, and has an independent
uv-locked Python reader. Authority verification follows DSSE 1.0.2 envelope
parsing and threshold rules. Exact replay preserved all four Frontier roots.
Rust 1.97.1, Node 24, uv, Ruff, and zizmor are now exact locked build inputs.
TOML is the sole current Frontier Profile encoding; the four canonical profile
and repository roots survived the one-time file cut unchanged, and retained
`frontier.yaml` now fails closed. The dependency-free closed Authorization
Profile now passes shadow parity against all seven current Cedar-backed
transactions and seven fail-closed negative cases. Its fixture independently
reproduces every retained legacy request root and freezes the candidate model
and request roots without changing the runtime or any Frontier. The remaining
order is bounded strict JSON at every protocol entry point, portable JSON
Schema 2020-12 with stable error codes, a common DSSE signed-object boundary
for Submission and Verification, retained exact authorization model/request
inputs, and one current-epoch cut followed by clean-clone replay and Cedar
deletion. There is no dual writer or dual current runtime.

The read projection is intentionally small operationally: one Neon project,
one `main` branch, and one active `vela_observatory` application database. The
unused `vela_projection` database, empty Registration-era tables, and all
legacy Hub/Registry tables have been removed. The inert `neondb` shell contains
no user tables. This cleanup did not move the active release pointer or
projection root.

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
- bounded export inspection by a separately governed Frontier.

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

Three current native Harbor comparisons passed their registered task-specific
exactness gates. The next benchmark must test a real correction with downstream
inheritance; more harness infrastructure is not an acceptable substitute.

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
proof. No external-user evidence was collected or claimed.

The Formal foreign-reference receiver-continuation comparison passed the same
task-specific gate: all four native Harbor trials were eligible, the
Vela-guided arm was exact 2/2, and Git/files was exact 0/2. Median agent time
was 135.34 seconds guided versus 286.07 seconds baseline, and median cost was
$0.2306 versus $0.5098. The compact result root is
`sha256:c0e6b316ce2b446d0b1a05b7f9d1acdb93631b32ae7c2b17d76805a8b650cfda`.
The task required the agent to distinguish accepted foreign source Standing
from a pending local Proposal and recover the exact Decision packet. It closes
a second task class, but remains first-party evidence and does not satisfy the
held-out correction-impact gate.

The unrun external cold-reader study was removed from the active tree before
collecting any response. Vela therefore makes no external-user, adoption, or
organizational-independence claim. Product evidence remains the registered
first-party Harbor comparisons; the next protocol-level gate is a real
correction with downstream inheritance, not another runner or participant
ceremony.

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
uv run --project conformance --locked python conformance/verify.py
cargo check -p vela-cli
uv run --project conformance --locked python -m unittest discover \
  -s benchmarks/product-compression -p 'test_*.py'
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
| Vela | `0.963.0` is published with the TOML-only Frontier contract, standards spine, and exact cross-platform release smokes | keep one release path and release only for demonstrated changes |
| Cross-language readers | standalone JavaScript reader and uv-locked Python reader | keep the reader surface small until a real external consumer requires a published library |
| Canopus | `0.8.0` historical evidence | no current source or release train |
| Vela Web | deployed `0.430.0` Registry/Atlas head uses `observatory.v8`; projection root `sha256:8bc68a34296b7e33bee7ca2321333bf84ea9d6b96867b55dd2c64ff85394917e` binds Vela `0.963.0`, all four current Frontier tips, 11 exact sources, 6,713 native records, and 5,844 explicit bindings | add no broader graph, registry, or execution surface until a named consumer earns it |
| Frontiers | canonical Git sources; the Quantum correction, native Formal Lean result, and exact Erdős range through `10430600` are accepted and replayed, the obsolete Formal duplicate is withdrawn with zero accepted delta, and remaining Proposals await attributed human Decisions or cancellation | decide or cancel exact remaining Proposals, replay, remap |
| Paper | bounded technical evidence exists | canonical whitepaper only after the real correction-and-inheritance gate |

Failure narrows or deletes the system. It does not earn another layer.
