# ADR 0026: Correction benchmark and whitepaper evidence contract

- Status: Proposed
- Proposed: 2026-07-28
- Protocol effect: none while Proposed
- Product effect: none until a benchmark demonstrates user value
- Authority effect: none
- Service effect: none
- Builds on:
  [ADR 0025](0025-math-first-compounding-product-architecture.md) and the
  [protocol breakthrough benchmark](../BREAKTHROUGH_BENCHMARK.md)

## Context

ADR 0025 established the product loop:

```text
map -> target -> run -> verify -> commit -> compound
```

It also identified the missing protocol-scale result: a real correction must
produce the same bounded consequences in independent implementations, preserve
an independent support route, open the same repair work, and cross into a
second Frontier without importing foreign authority.

Vela has not demonstrated that result. It has demonstrated local admission,
scoped Verification, an authorized human Decision, deterministic repository
replay, and one matched execution evaluation. Those results justify a
benchmark campaign. They do not justify a correction algebra, hosted Registry,
Atlas, global knowledge graph, package ecosystem, or protocol-breakthrough
claim.

The current protocol already defines exact Claim correction, supersession, and
retraction through ordinary Submission and Decision. Acceptance retains the
predecessor and updates current Standing. The experiment therefore tests
whether retained Claims and relations contain enough exact information for
independent readers to derive correction impact. It does not begin by adding a
correction primitive.

The four strategy reviews dated 2026-07-28 converge on one narrow hypothesis:

> An exact scientific transition can move among heterogeneous tools and
> independently governed Frontiers while retaining causal inputs, scoped
> checks, local authority, deterministic state, and bounded correction
> consequences.

Git, the Internet end-to-end argument, build systems, transparency systems,
and append-only ledgers help locate the boundary. They do not establish that
Vela has solved it. Git supplies content-addressed byte history. Scientific
systems at the endpoints still need to judge statement fidelity, evidence,
scope, and local Standing.

The project therefore needs one registered evaluation and publication
contract. Without it, implementation choices can drift toward attractive
architecture and a whitepaper can become a retrospective narrative over
results selected after the fact.

## Decision proposed

### 1. Register one binary protocol benchmark

The benchmark is the current
[`BREAKTHROUGH_BENCHMARK.md`](../BREAKTHROUGH_BENCHMARK.md) contract plus one
canonical, machine-readable evaluation plan. The plan is a research artifact,
not a Vela protocol object. Its root binds:

- the primary fixture and held-out selection rule;
- exact source repositories, commits, trees, Claims, relations, Decisions,
  Events, Artifacts, and verifier identities;
- the proposed correction and its scientific basis;
- relation classes and their declared consequences;
- expected affected and unaffected sets;
- independent surviving routes;
- repair Obligations and discharge conditions;
- Rust and clean-room implementation identities;
- resource bounds, truncation behavior, and cycle rules;
- baselines, assignments, timing rules, interventions, and stopping rules;
- authority, custody, and nonclaim restrictions; and
- the publication policy for positive, negative, and partial results.

Once any benchmark output exists, an amendment must retain the earlier plan
root, state the reason, and invalidate confirmatory credit where the amendment
changes an expected answer, fixture, metric, or gate.

### 2. Use a real correction

The primary fixture must start from accepted mathematical Claims already
retained by a current Frontier. The correction must be supported by exact
source evidence and must narrow, supersede, or retract scientific content. A
byte mutation, metadata repair, invented toy dependency, or intentionally
false Claim does not qualify.

The minimum topology remains:

```text
A  accepted Claim that later needs correction
B  consequentially depends on A
C  has one route through A and one independent route
D  has only a non-consequential discovery relation to A
```

If no current Frontier contains that topology and a genuine correction, the
result is a failed entry gate. Vela may create a transparent, prospective
fixture through the ordinary Submission, Verification, and human Decision
path, but it may not call the fixture historical evidence.

The first audit failed this entry gate. Across the four current Frontiers,
2,831 retained Claim records contain zero current `corrects`, `supersedes`, or
`retracts` relations. Erdős problem 281 has the required dependency shape but
no correction. Erdős problem 128 has a genuine upstream source correction, but
its accepted Frontier Claim names a mutable theorem URL without binding the
source commit, path, or content root. Different retained derived sources point
to both the correct and incorrect theorem revisions. That ambiguity is an
integration defect and a benchmark result. It is not evidence that the
protocol's correction transition is ambiguous. Erdős problem 1197 retains an
exact, kernel-clean complete proof alongside an accurately labeled conditional
proof. The completion does not correct the conditional result, while retained
`replicates` and `contradicts` relations disagree about their relationship.
That case tests relation fidelity but does not qualify as scientific
correction.

The held-out fixture is selected after both implementations are frozen by a
deterministic rule registered before selection. It must use a different
scientific case and may not reuse the primary expected output.

### 3. Keep the first correction projection derived

The experiment initially lives in `vela-edge` and the language-neutral
conformance corpus. It reads retained canonical objects and emits a
root-bound, non-authoritative impact projection.

The projection may classify:

- retained predecessor state;
- directly affected Claims;
- surviving independent routes;
- unaffected discovery relations;
- unknown or incomplete consequences; and
- bounded repair Obligations.

It may not write Standing, mutate a Frontier, infer a Decision, or convert a
foreign Decision into local authority.

No new Event kind, canonical Obligation, universal relation algebra,
`Frontier Commit` schema, resolver, or federation service is authorized. A
protocol change requires a reproduced ambiguity that prevents two
implementations from agreeing under the current public objects and explicit
fixture rules.

### 4. Require two genuinely separate implementations

The reference implementation is Rust. The clean-room implementation uses a
different language, imports no Vela Rust code, reads only the published
contract and fixture bytes, and produces its own canonical output.

Colocation in this repository establishes implementation diversity, not
organizational independence. Protocol-breakthrough credit still requires an
external implementer or reproducer who receives no private semantic hints.

Both implementations must agree byte-for-byte on complete projections and on
bounded diagnostics for incomplete projections. Unknown consequential
relations fail closed.

### 5. Measure ten separate properties

The registered suite reports:

1. transition-channel fidelity;
2. correction detection, localization, repair, and propagation;
3. causal integrity and independent-route survival;
4. authority non-escalation;
5. implementation and reader removability;
6. failure of a hosted hub or read model without replay loss;
7. support-route diversity;
8. exact transfer across a second Frontier;
9. observability of known, missing, inaccessible, and truncated state; and
10. inheritance lift against Git plus the same files, evidence, and verifier.

Correctness gates are binary. Latency or usability cannot compensate for a
wrong affected set, false local Standing, lost support route, hidden
truncation, or replay divergence.

### 6. Separate execution, state, inheritance, and adoption claims

The paper and product report four result families independently:

```text
execution lift    verifier-passing artifacts per cost and expert-minute
state lift        correct decisions and corrections per reviewer-minute
inheritance lift  time to correct useful continuation after substitution
adoption          recurring use by people and institutions outside the team
```

A Canopus execution win does not prove Vela state lift. A deterministic
correction does not prove adoption. A package download, Git star, graph node,
or verifier pass does not prove scientific acceptance.

### 7. Earn the whitepaper

The working whitepaper may be drafted before the experiments finish, but every
result section remains an explicit placeholder until its artifact root exists.
Publication as a protocol paper requires:

- both preregistered fixtures;
- Rust and clean-room agreement;
- the full adversarial suite;
- a second Frontier with no authority escalation;
- exact clean-clone reproduction;
- the matched Git baseline;
- at least one external participant per fixture;
- a public artifact package that reproduces every result table and figure;
  and
- a limitations section that reports failed fixtures, intervention, selection
  effects, and unsupported claims.

If a hard gate fails, publish the falsification or narrower engineering result.
Do not publish the protocol-breakthrough title, abstract, or claim.

Git, Linux, Bitcoin, TCP/IP, and other foundational systems are standards of
clarity, minimality, threat modeling, and independent implementation. They are
not impact comparators. Vela may claim equivalence only if future evidence
supports the specific compared property.

### 8. Delete what the evaluation does not earn

- Remove a correction projection that cannot outperform a clear Git baseline
  or cannot achieve exact affected-set correctness.
- Remove relation classes that do not change a declared consequence.
- Keep a source-local domain mapping local until two maintained consumers use
  it and extraction deletes maintained duplication.
- Do not build Registry or Atlas surfaces until exact cross-Frontier reuse
  creates a measured discovery problem.
- Do not add a framework, service, or database to compensate for an
  underspecified protocol boundary.

## Acceptance gate

Accept this ADR only after:

1. the primary fixture has a verified scientific correction rather than a
   convenient graph shape;
2. the primary evaluation plan and source audit have stable full roots;
3. the held-out selection rule is fixed;
4. implementation ownership and clean-room restrictions are recorded; and
5. the artifact and paper release policy is public.

Acceptance authorizes the experiment, not a protocol change or breakthrough
claim.

## Rejected alternatives

- **Write the whitepaper now and backfill evidence.** Rejected because it
  permits result selection and overclaiming.
- **Use a synthetic dependency diamond as the primary result.** Rejected
  because it tests code, not scientific correction.
- **Promote the historical depth-capped Finding cascade.** Rejected because it
  conflates relation meanings and loses independent-route semantics.
- **Build a universal scientific graph first.** Rejected because the benchmark
  needs only a bounded typed causal slice.
- **Use the Observatory or Neon as the second implementation.** Rejected
  because both are disposable readers and must not become replay dependencies.
- **Treat one first-party clean-room reader as external replication.** Rejected
  because implementation diversity and organizational independence are
  different claims.

## Consequences

- The next engineering work is a preregistered experiment, not another product
  train.
- The existing protocol remains the normative starting point.
- The historical corpus cannot earn the primary-fixture gate until an exact
  Claim-level source binding and a real accepted correction coexist.
- A whitepaper can be written continuously without allowing prose to outrun
  evidence.
- Failure produces a useful narrowing or deletion decision.
- No release is required to adopt this research contract.
