# Vela roadmap

Vela is becoming one clear product:

> Version control for living science: map exact scientific state, turn its
> explicit boundary into valid work, and make every checked and authorized
> advance improve what the next person or agent can safely do.

The hierarchy is fixed:

```text
protocol  -> integrity layer
map       -> product
verified Frontier movement -> outcome
```

The public loop is:

```text
map -> target -> run -> verify -> commit -> compound
```

The detailed work, evidence roots, benchmarks, and stop conditions live in the
[active campaign](CAMPAIGN.md). This roadmap records only sequence and gates.

## Proven foundation

- Vela `0.950.1`, `@vela-science/protocol@0.1.0`, and
  `@vela-science/canopus@0.8.0` are released.
- The four controlled mathematical Frontiers use one current repository
  contract and replay from clean clones.
- Submission and scoped Verification do not change Standing.
- A real human Decision changes Standing once and replay reproduces it.
- One bounded Erdős result completed
  `Target -> Run -> Submission -> Verification -> Decision -> remap`.
- Canopus shows positive first-party execution evidence but no population or
  adoption result.
- One Formal/Lean artifact kernel-checks under exact retained inputs.
- One Erdős transition was retained and verified in Formal without importing
  source Standing.
- Vela Web already has one normalized, rebuildable, root-bound Neon read model
  and a SELECT-only Observatory.
- Canopus, Neon, the Observatory, hosted APIs, and the original producer
  session are not required for canonical replay.

## Current checkpoint — 2026-07-31

The planned Formal and quantum result loops plus three successive Erdős
continuations have exact requirement-matching Verification with accepted state
unchanged:

- Erdős commit `8d8f396239bd6393009426ab403ce44082f1b16a`, repository
  root
  `sha256:adefa01cea989241d96ce22274cdd778d2750054537a1956cdf2c52e041a39c8`;
  Proposals `vpr_27bce8983810f3bd`, `vpr_148c88da4d5579a9`, and the
  corrected bounded `vpr_96578d006119b322` have requirement-matching
  Verifications `vvr_b879aec074e01d16`, `vvr_18f4862fd1a2c256`, and
  `vvr_9e6664cad0970e67`; producer work through `10430200` is closed without
  accepting any Claim, and the next nonduplicate range is
  `10430201..10430400`. Earlier broader-worded Proposal
  `vpr_b4a4b9ea9c00d6e9` and Claim
  `vcl_764737221fcd251de5fcabe2836915d15160dd217976c29d30d1e641362598fe`
  remain retained and `pending_review`; they are not substituted for the
  corrected bounded Submission `vsb_895cb3913a189369`;
- Formal commit `33d12b9acef1e909c5942c1162bdf1987192f833`, Proposal
  `vpr_08a91ee1b770f5cb`, requirement-matching Verification
  `vvr_96dcaefef0617952`; its exact bounded producer Target is closed and its
  tracked source-local replay capsule passes the frozen Lean replay as
  evidence only, but its scientific Claim is not accepted; and
- Quantum commit `f5aa2fca9029f02ad1b2ca31f58545f370b0d6cc`, Proposal `vpr_8715dbb5e2a12442`,
  requirement-matching Verification `vvr_606aff748c89df76`; an actor-separated
  reconstruction verifies the exact `[[10,1,4]]` certificate, while Decision
  remains null. The intervening test-runner portability fix changed no
  scientific object, repository root, graph relation, or Standing.

Each Proposal remains `pending_review`. Protocol readiness is not a
recommendation to accept. The repository authority—not an agent—may accept,
reject, or cancel each exact Decision. The corrected cross-Frontier Formal
transfer remains pending its separate held-out consumer/value gate despite
having no protocol blocker.

Sidon remains clean at commit
`75b3392c5d2a4390065a3927914acdb552d69e8e`. Vela Web `v0.430.0` is deployed
with Observatory snapshot commit
`a7cb131c4a2bfea9038c61ef23d763bba878bf25` and unchanged editorial commit
`f85ff84b6d757ceff2b3b2dcf2a1b87176566f4d`:

- the Observatory and editorial production manifests identify the exact version,
  commit, brand root, and Vercel deployments;
- the active projection is
  `sha256:d8339a90ddcc76dd2fc208365c5f6713739ddf39ec24c60f450f60f8f3601d93`
  under Vela `0.950.1`;
- source adapter set
  `sha256:f5bcd480aafe766a1700450efd019115a2fb9f90aac8091f507595047c91b9dd`
  contains 6,700 exact adapter records, and the Registry projection contains
  9,541 native rows;
- PLBY is observed from locked commit
  `d4476dd3535ec618dee4177915741017026d26bf`, not the stale predecessor;
- the single Neon `main` branch exposes 14 exact sources, 6,307 Frontier
  bindings, 4,152 graph nodes, 2,591 edges, and 4,156 search documents through
  the SELECT-only reader; and
- production data checks confirm the root-bound 4,054-node, 2,548-edge Erdős
  projection, and the live graph API returns the exact active release root.

The retained live-read-health artifact `sha256:d447c008…` qualifies the current
active release `sha256:d8339a90…` with 14 declared and observed sources, zero
duplicate native IDs, and zero dangling Frontier bindings. The separate
bounded capacity artifact `sha256:c1ad6d1a…`
passes 100,000-record ingestion, exact root/count verification, rollback
containment, transactional activation rollback, SELECT-only reader, and
eight-way read budgets using the existing schema and JSONB writer. No COPY path
or another storage layer was earned. Clean empty-database reconstruction also
passes at `sha256:dfd38ca3…`; product lift remains open.

This is released alpha evidence, not adoption or product-lift evidence. Any
later human Decision requires another exact refresh before its changed
Standing appears in the Atlas. Hosted runner repetition is optional
corroboration, not a product or protocol gate. Do not add a feature branch or
staging database merely to repeat evidence already reproduced from exact
source roots.

## P0 — make everyday Vela obvious

- Keep one small daily CLI: `status`, `next`, `start`, `submit`, `show`,
  `why`, `review`, `check`, `reproduce`, `log`, and `doctor`.
- Make each primary surface answer: what is this, why does it stand, and what
  is safe to do next?
- Make Why this stands and Scientific Diff the signature reader interactions.
- Keep Submission, Verification, Decision, and Standing unmistakably
  different.
- Remove ceremony, duplicate explanation, and invalid defaults before adding
  commands or abstractions.
- The current private Attempt is a small ignored lease over one exact Target,
  expiry, Artifact scope, and Submission/Verification budgets. Status excludes
  expired, budget-exhausted, or Target-advanced Attempts. Vela owns no Agent
  runner, scheduler, Campaign host, or private Run receipt model.
- Require fresh-user comprehension of the repository-authority Decision path
  before calling the everyday workflow complete.
- Require a second real producer replay before accepting ADR 0021.
- Accepted ADR 0031 is complete: immutable Canopus remains historical evidence
  and current native tools integrate through Target packets, Submission, and
  Verification Records without a Vela-owned runner.

## Completed — Math Source Registry and observation path

The read-only Math Source Registry now lives inside the existing
`@vela/frontier-data` and Observatory boundaries.

It records exact source identity, source-declared publisher or maintainer,
native namespace, observed locators, license, snapshot policy, adapter root,
observed revision, coverage, omissions, tombstones, and Frontier bindings. It
distinguishes:

```text
reference -> snapshot -> admission
```

Observation runs only in the exact refresh workflow, never in a request
handler. It is deterministic, source-specific, and transactionally activated.
Unknown rights, ambiguous native IDs, missing pages, partial insertion, or root
drift fail closed. Failed runs remain outside the active Atlas release.

The alpha stops at 14 sources already used by Erdős, Formal Conjectures, Sidon,
and Quantum Codes. Add another source only to close a named Atlas gap after
its rights, identity, version, and update contracts pass.

This is a source inventory, not a theorem registry, package marketplace, or
authority system.

## Completed — working Math Atlas alpha

The existing Observatory—not another application—is the first root-bound Math
Atlas at Vela Web `v0.430.0`.

The alpha must expose:

- the complete declared current Claim inventory and Frontier-local Standing;
- all 1,217 Erdős problems;
- registered native sources and exact observations;
- Artifacts, Submissions, Verifications, Proposals, Decisions, and Targets;
- source coverage, omissions, and inaccessible material;
- Why this stands and Scientific Diff;
- valid nonduplicate work and exact reproduction;
- an answer-first ledger as the primary map; and
- graph and search as secondary exact-root lenses.

Erdős 1056 is the flagship complete problem map. Formal must separate kernel
checking, statement fidelity, Verification, and acceptance. Quantum must
correctly classify the retained `[[10,1,4]]` witness and open question.

ADR 0030 is earned only for this bounded alpha implementation and
reconstruction result; adoption and beta claims remain separate.

## P1 — complete genuine result Decisions and remap

The Erdős `10429601..10429800`, `10429801..10430000`, and corrected
`10430001..10430200` results, Formal Erdős 835, and retained quantum
`[[10,1,4]]` mission have preserved their failures, scoped their Claims, and
imported separate Verifications. Complete each through a human Decision or
explicit cancellation, replay from clean clones, rebuild the Atlas, and prove
that Standing changes only through Decision. Erdős producer work already
closed independently of Standing and now offers `10430201..10430400`; do not
regress that separation by making duplicate-work prevention wait on
acceptance.

## P1 — measure whether the product works

Evaluate separately:

- product-comprehension lift: native Codex through Harbor against Vela's
  read-only interface versus Git and the same files;
- state lift: exact evidence and Standing recovery;
- inheritance lift: correct continuation from only the new root;
- correction integrity: affected, surviving, unaffected, and unknown paths;
  and
- interoperability: foreign transfer versus a plain rooted manifest.

Any positive product claim requires full authority-critical correctness, zero
verification-as-acceptance errors, and at least 20 percent median improvement
over Git plus identical evidence.

First-party sessions debug methods but earn no adoption credit. Adoption
requires at least five uncoached non-maintainers and one independently
controlled reproducer.

If no real correction fixture qualifies, record the failed entry gate. Do not
manufacture a scientific correction to satisfy the benchmark.

## P2 — earn shared infrastructure

A source-local Math profile may be extracted only after the Erdős, Formal, and
Atlas implementations reproduce one stable shared need.

A shared module or package requires:

- two maintained consumers;
- deterministic offline roots;
- native identity and semantic sovereignty;
- explicit loss and unsupported meaning;
- no replay or authority effect; and
- deletion of more maintained duplication than it adds.

The longer ladder remains:

```text
source-local recurrence
-> immutable shared package
-> static package index
-> hosted package Registry
-> federated read-only Atlas
```

The Math Source Registry and first-party Atlas alpha are complete. The active
gate is cold-use and product lift: prove that a researcher can locate decisive
evidence, act on one exact Target, and continue from the resulting obligation
faster and more accurately than from the same native sources alone. The
semantic-package Registry and federated global Atlas remain gated by external
reuse, independently governed Frontiers, correction propagation, and cold-user
lift.

## Release posture

- Keep Vela at `0.950.1` unless a generic product defect requires a compatible
  `0.951.0` release.
- Keep immutable Canopus `0.8.0` and its tag as historical replay evidence.
  Current Vela carries no runner, compatibility layer, or second release path.
- Keep Vela Web `0.430.0` as the released exact source inventory and Math Atlas
  alpha.
- Earn Vela Web `0.440.0` only through real multi-shape loops and external
  product evidence.
- Keep ADR 0026 and the protocol-breakthrough paper Proposed until a real
  correction, held-out case, independent reproduction, non-escalating
  transfer, external cold use, and public artifact package pass.
- Do not schedule Vela `1.0.0`.

## Deferred

No global truth database, universal ontology, graph database, hosted
authority, second writer, scheduler, mandatory orchestration framework,
package marketplace, reputation score, agent operating system, new Atlas or
Registry repository, or biology expansion is scheduled.

Failure to demonstrate lift causes simplification and deletion, not another
layer.
