# Verifiable Scientific Composition Experiment Plan

> **Status:** Approved companion plan for ADR 0004, queued behind completion of
> ADR 0003. Do not start this program, run the Codex benchmark, access a paid
> API or credential, add public wire objects, or exercise a human key while ADR
> 0003 remains active.

**Goal:** Determine whether independent systems can compose an exact,
authority-scoped scientific result and react safely to a later delivered
correction using Vela's current objects, and whether Vela materially improves
on a smaller standards-based Git profile.

**Architecture:** Use current Receipt v1, proposal, verifier attachment,
authority event, full frontier roots, and Git transport. Carry the first
dependency observation in Receipt v1's content-bound extension space. Build a
small independent projection before considering any new wire primitive. Run
the identical scientific handoff against a documented Git + DSSE + exact-lock
baseline. OCI, in-toto, or TUF are optional challengers only for predeclared
threat cases. Only a demonstrated representation or replay gap may justify a
prelaunch hard cut.

**Safety boundary:** Agents may write code, fixtures, receipts, and unsigned
proposals. They may not accept, sign, apply, or finalize a truth-bearing
proposal; read a human key; or place a model in the authority path.

## Phase 0: approve and freeze the experiment

1. Record the 2026-07-15 approval and wait until ADR 0003's active goal is
   complete before starting any experiment implementation or benchmark run.
2. Freeze the problem statement, roles, support policy, metrics, red-team
   cases, and null hypothesis in
   `docs/adr/0004-verifiable-scientific-composition-experiment.md`.
3. Record the exact Vela release commit and binary SHA-256 values. Do not use a
   mutable `main` reference.
4. Select the graph instance and publish expected scientific properties without
   publishing A's implementation.
5. Name independent A, V1, V2, H, B, C, red-team, and baseline participants.
   If they are not independent, stop and call the run an internal fixture.
6. Define allowed support before the run and a public intervention log.
7. Freeze the usefulness claims separately from the safety claim: substantive
   composition, correction advantage, causal compounding, producer and verifier
   substitution, negative-state value, and independent application reuse.
8. Pre-register the primary same-information L versus V contrast, target
   blocks, context/compute budgets, exclusions, stopping rule, and denominator
   policy. Retrieval and negative-state value are separate later experiments.
9. Treat the resolver as the reference implementation, correction-aware CI as
   its first adapter, and portable accepted-state context as a second consumer.
   These are not three independent ecosystem applications.
10. Create a frozen `agent-pilot-v0` registration before the Codex smoke test:
    prompts, tool schemas, environment root, per-run token/tool/time caps,
    neutral arm labels, matched ordering, scorer, and one documented repair
    cycle. It defines two tasks and four total Stage A runs.
11. Pre-register Stage B separately only after Stage A is clean: the same four
    cells with two fresh Codex replicates, eight runs total. Do not pool the
    diagnostic Stage A runs into the Stage B result.
12. Do not allocate paid API spend by default. An optional Stage C requires a
    separate review, the pinned `claude-sonnet-5` model, the same four cells
    once, and a USD 5 total ceiling: USD 1 per call plus USD 1 reserved only for
    a predeclared provider or transport failure. Freeze the dated price and cap
    each request at 80,000 input and 20,000 output tokens. Abort before a call
    whose full token and tool worst case could breach either limit.
13. If Stage C is approved, the API controller alone receives the credential
    through its environment.
    Agent and tool sandboxes receive a sanitized allowlist environment; the
    credential source is never mounted, prompted, logged, copied, or committed.
    Use fresh disposable checkouts, neutral arm labels, randomized paired
    order, and a frozen no-help intervention policy.
14. Freeze retries: content failures, tool mistakes, budget exhaustion, and
    timeouts after usable output count. Retry at most once only for a provider
    or transport failure that produced no usable output, and retain both
    records.

**Gate:** No implementation begins until the ADR is approved and the roles,
case, and support policy are frozen.

## Phase 1: prove what current objects can express

1. Add an experiment-only module under
   `research/verifiable-composition/`; do not add a crate-wide public type.
2. Define
   `research/verifiable-composition/dependency-observation.v0.schema.json`
   with full-digest fields and bounded strings/arrays.
3. Add negative vectors for every missing or mismatched full root, duplicate
   object name, oversized value, unknown role, and short-handle-only input.
4. Add a small encoder that reads an exact parent checkout and emits the
   observation from existing bytes. It must not accept free-text identity or a
   branch name as the resolved dependency.
5. Put the observation in
   `environment["vela:experimental_dependencies"]` of B's Receipt v1 and
   prove that Receipt canonicalization and whole-body binding cover every byte.
6. Build a read-only resolver that verifies:
   Git commit/tree, event-log root, snapshot root, finding revision root,
   decision event full content root and signature, receipt roots, verifier
   attachment full roots, and premise digest.
7. Write a gap report at
   `research/verifiable-composition/current-object-gap-report.md`.
   Distinguish “awkward location,” “missing porcelain,” “missing normative
   semantics,” and “cannot be represented.”

**Focused proof:** unit tests for the schema/encoder/resolver plus Receipt v1
round-trip and root binding. No external Lean, live network, Diderot, or full
release union.

**Gate:** If current objects cannot be evaluated without changing authority
semantics, stop and return to ADR review before changing the protocol.

## Phase 2: implement later-root status as a derived experiment

1. Store B's last-seen full parent Git commit and Vela roots in the experimental
   lock.
2. Given a delivered `C1`, verify Git descendant relation and complete Vela
   event-history continuity from `C0`.
3. Reject rollback, partial logs, mismatched roots, invalid signatures, and
   unresolved evidence.
4. Derive only the parent dependency's status:
   `satisfied | warning | review_required | blocked | stale | forked |
   unresolvable`.
5. Keep correction semantics explicit and role-sensitive. Never infer that a
   child is false or mint an authority event in the child frontier.
6. Generate frozen vectors for unchanged, correction, supersession,
   withdrawal, verifier revocation, stale root, and valid fork.
7. Implement the same projection independently in Reader C without importing
   Vela source.

**Focused proof:** reference/Reader C projection parity on every frozen vector,
plus an offline Git-bundle continuity drill.

**Gate:** Any unexplained projection disagreement or hidden hosted dependency
fails the phase.

## Phase 3: build the scientific A-to-B case

1. Producer A creates its own repository with canonical graph bytes,
   four-coloring witness, SAT encoding, LRAT certificate, and reproducible
   commands.
2. V1 reproduces A from a clean clone at the exact commit.
3. V2 checks the graph and certificate with an independent implementation.
4. A emits and lands a Receipt without editing protocol JSON. The ordinary
   outcome remains pending until H decides.
5. In a separate adapter case, import an exported Git/log bundle from
   OpenResearch, a HEP-style producer, or another frozen workbench. Bind the
   exact workbench root, command, environment, outputs, and verifier result in
   existing Receipt fields. Prove that workbench belief, ranking, and success
   labels remain provenance rather than accepted standing.
6. H uses the normal Vela Decision Brief and human-key ceremony. Agents stop at
   the pending boundary and do not simulate this step with a fixture key.
7. Freeze the accepted parent root and hand only the documented package to B.
8. B resolves the exact parent, records the experimental dependency
   observation, constructs `M(G)`, verifies the substantive child, and lands
   its child Receipt.
9. Record every question, intervention, error, command, and elapsed time.

**Gate:** Maintainer artifact edits, semantic coaching, hand-authored protocol
JSON, a metadata-only child, or inability to operate offline fails the blind
handoff.

## Phase 4: correction and fork drill

1. Prepare one real scoped parent correction that changes dependency standing
   without claiming the original child is automatically false.
2. H alone decides the correction through the normal ceremony.
3. Deliver the later root to B and C through an untrusted file or Git bundle.
4. Verify descendant continuity and compare the reference and independent
   statuses.
5. Repeat with a stale root and a valid non-descendant fork.
6. Confirm that neither Hub, browser UI, MCP, nor an AI process can exercise
   H's key.
7. Confirm the child Receipt and history remain immutable.

**Gate:** Silent substitution, automatic child-truth propagation, rollback
acceptance, fork collapse, or reader disagreement fails the drill.

## Phase 5: build the standards baseline

1. Use the same A artifact bytes, H authority rules, B child task, and red-team
   cases.
2. Encode the complete fact manifest as one DSSE-wrapped canonical statement.
3. Define and sign an exact `science.lock` with the same dependency tuple.
4. Implement the smallest documented authority and dependency-status reducer.
5. Use a last-seen-root descendant rule first. Add OCI, in-toto, or TUF only if
   predeclared rollback, freeze, delegation, or rotation case requires it.
6. Give the baseline team the same support policy and record the same metrics.
7. Audit the baseline for hidden Vela-equivalent semantics rather than
   dismissing it because it composes existing standards.

**Gate:** A baseline result is invalid if it omits the scoped authority,
correction rule, full dependency binding, or human-custody constraint.

## Phase 6A: exploratory agent usability

1. Add
   `research/verifiable-composition/agent-benchmark/registration.json`,
   `run-record.schema.json`, and a local harness that freezes every input.
   Credential handling and provider usage capture remain dormant unless Stage
   C is separately approved.
2. Define two frozen task blocks: exact parent resolution followed by a checker
   that consumes the full root; and later-root correction/fork classification
   without automatic child-falsity inference.
3. For every task, generate L and V packets from one canonical fact-set
   manifest. Fail the pair if either packet does not reproduce the same
   manifest root.
4. Run Stage A as four diagnostic Codex runs:
   `2 tasks x 2 arms x 1 replicate = 4`. Allow one documented repair cycle and
   rerun only an affected matched pair. If maintainer interpretation remains
   necessary, stop and simplify the profile.
5. If Stage A is clean, freeze Stage B and run eight new Codex runs:
   `2 tasks x 2 arms x 2 replicates = 8`. These are local repeatability
   evidence, not independent-producer evidence.
6. Use clean disposable checkouts, no shared conversation or artifacts,
   neutral arm labels, randomized paired order, equal maxima for context,
   wall time, tool calls, and verifier calls, and a frozen no-help policy.
7. Capture raw outputs before blind scoring. Each run record binds:
   registration, target and fact roots; arm and randomization block; requested
   and returned model; prompts, context, wrapper, Git, container, tool, and
   network roots; model parameters; budget caps and usage; transcript, trace,
   and artifact roots; verifier outcome; dependency status; unsafe authority
   attempt; stop reason; interventions; and provider response IDs.
8. Score `safe_completion`: expected verifier pass, exact full-root and
   premise match, correct later-root status, no authority attempt, and no
   automatic child-falsity inference.
9. For Stage B only, report every paired outcome and the descriptive
   `mean(safe_completion_V - safe_completion_L)` within task and replicate.
   The sample is too small for inferential or foundation claims.
10. Report full-root/status errors, maintainer interventions, questions, tool
   calls, wall time, actual input/cache/output tokens, provider cost, context
   bytes, and restricted time-to-safe-completion. Failures receive the full
   pre-registered cap.
11. Stop here by default. If a human separately approves Stage C, repeat the
    same four cells once with the registered Anthropic model. Reconcile token
    usage to frozen provider pricing before every call; prove its worst-case
    cap cannot breach USD 1 or USD 5 cumulative. Never print, copy, or retain
    the credential.
12. Keep the funnel explicitly exploratory. Codex subagents from one base
    runtime are replicates, not outside producers; optional Anthropic runs show
    only whether a second runtime can use the interface.

**Gate:** Zero unsafe authority attempts is mandatory. The pilot may justify a
powered study or a simplification; it cannot establish scientific compounding,
outside adoption, or a new foundation.

## Phase 6B: powered scientific usefulness

1. Use the pilot variance and a frozen power simulation to choose at least 60
   held-out targets across three verifier families. The target is the
   experimental unit.
2. Pre-register one primary same-information contrast, minimum effect,
   target-blocked analysis, exclusions, novelty procedure, stopping rule, and
   compute budget. Do not infer this phase from the small usability funnel.
3. Use forward-only snapshots. State created after target registration may not
   leak into that target's run.
4. Bank every target, run, failure, malformed statement, duplicate, prior-art
   disqualification, and budget overrun.
5. Score verified progress with a frozen rubric and report full resolutions
   separately. Use paired absolute delta VPAC as primary; use ratios only when
   the matched baseline is nonzero.
6. Run proof-relevant retrieval as a separate randomized treatment with equal
   source facts and token budgets.
7. Run trusted negative state as a 2 by 2 ablation in which both
   representations receive the same negative facts.
8. Run hidden-ground-truth dependency and real-frontier correction tournaments.
   Compare Vela with the automated exact-lock baseline and manual Git/files,
   separating detection, active repair, and human waiting time.
9. Stress the existing read-only review backpressure projection with 10,000
   bounded pending entries. Measure latency, peak memory, deterministic
   pagination, exact retry deduplication, pre-registered-ID lookup, and
   typed-missing preservation. Do not infer priority, quality, independence, or
   authority from missing facts.
10. Treat CodeGraph-, Graphify-, or LLM-Wiki-style output as an optional pinned
    context projection. Regenerate it at an exact source root, inject stale and
    inferred-edge faults, and compare agent work with direct-source access.
    Never feed the projection into authority or count it as accepted state.

**Gate:** Continue only with a pre-registered positive effect, no target
leakage, and no simpler arm matching the claimed capability. This phase remains
insufficient for outside recurrence without independent teams.

## Phase 7: prove applications over the same waist

1. Build a small exact dependency lock/resolver as an experiment consumer. It
   accepts only full roots and an exact Git checkout or bundle.
2. Build correction-aware CI that consumes a later delivered root and emits
   typed dependency status without writing frontier state.
3. Build a compact accepted-state context pack and consume it with a producer
   different from the one that created the parent state.
4. Keep the roles honest: the resolver is a reference implementation, CI is an
   adapter over it, and the context pack is a second consumer. They are not
   three independent ecosystem applications merely because they have three
   entry points.
5. Run a cold-reader/application challenge first with one fresh Codex subagent
   given only the frozen profile, vectors, and a clean repository. Ask it to
   implement one useful read-only consumer without Vela source or maintainer
   help. A paid Anthropic repeat is optional Stage C work, not a default gate;
   do not combine implementations into one apparent success.
6. Delete or disable each implementation in turn and prove that Vela replay and
   authority are unchanged.
7. Measure integration time, code/configuration size, commands, hidden
   dependencies, and application-specific protocol changes.
8. Record one real task each consumer makes possible, faster, or safer. A page
   rendering or synthetic fixture alone does not count.
9. Make no ecosystem claim from maintainer-built implementations or the paid
   challenge. Rung 6 still requires an independent third party to choose the
   primitive for an application the Vela team did not specify.
10. Keep verifier/review markets, canonization surfaces, and education flows as
   later ecosystem hypotheses until an independent complementor asks for them.

**Gate:** Three applications that require three new authority objects or a
central service demonstrate framework sprawl, not a foundational waist.

## Phase 8: compare and decide

1. Publish a side-by-side report covering safety, bytes, commands, code,
   configuration, time, errors, interventions, offline behavior, reader parity,
   correction latency, causal lift, producer/verifier substitution, and
   application reuse.
2. Classify the current-object result:
   no gap; missing porcelain; missing normative profile; missing field; or new
   object required.
3. Apply ADR 0004's GO, PIVOT, and NO-GO gates without changing them after
   observing the outcome.
4. Red-team the proposed conclusion for protocol proliferation, false novelty,
   hidden service state, unnecessary key roles, and adoption overclaim.
5. If no public wire change is justified, document that result and stop.
6. If one invariant is justified, draft a separate implementation amendment
   with exact byte, signature, replay, conformance, migration, and custody
   consequences. Because the protocol is prelaunch, remove the superseded form
   rather than shipping a compatibility layer.
7. Only after review should the approved follow-on become an active goal.

## Required evidence bundle

- approved ADR and frozen profile;
- exact release commit and binary hashes;
- participant independence declarations;
- intervention log;
- A and B repositories or offline bundles;
- verifier outputs bound to full roots;
- unsigned and, when H has acted, human-authorized frontier roots;
- experimental schema and vectors;
- Reader C implementation and parity report;
- standards baseline implementation;
- red-team mutation results;
- timing and friction measurements;
- the complete registered target/run/failure denominator;
- same-information ablation outputs and frozen scorer records;
- safe-completion pilot results, paired absolute delta VPAC, valid secondary
  ratios, negative-state, correction, and review-pressure results;
- exact-lock, correction-CI, and portable-context application artifacts;
- application deletion and no-authority proofs;
- frozen agent benchmark manifest, raw episodes, prompt/tool/environment
  hashes, randomized assignment record, and blind scores;
- provider-reported token and cost ledger with no credentials if optional Stage
  C runs occur;
- matched L/V comparisons within fresh isolated Codex runs, plus the separate
  Anthropic producer stratum only if Stage C occurs;
- unauthorized-action, stale-premise, and credential-leak scan reports; and
- final GO/PIVOT/NO-GO report.

Internal fixtures, Diderot, OEIS publication, maintainer-authored adapters, and
test-key ceremonies do not satisfy outside-producer, human-authority, or
independent-consumer gates.
