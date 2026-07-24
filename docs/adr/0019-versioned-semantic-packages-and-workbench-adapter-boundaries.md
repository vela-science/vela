# ADR 0019: Versioned semantic packages and workbench-adapter boundaries

- Status: Proposed
- Protocol effect: None in the proposed program
- Candidate release: None reserved
- Scientific authority effect: None
- Entry gate: one exact Erdős package used by two maintained consumers without
  duplicating canonical state or weakening replay

## Context

Vela's released product boundary is intentionally small:

```text
produce -> preserve -> check -> decide -> reuse
```

Git repositories preserve exact Frontier history. Vela owns canonicalization,
replay, verification state, strict signals, policy, protected decisions, and
scientific standing. Canopus is an optional producer. The Observatory is a
read-only projection. None of those roles should be enlarged merely to make
scientific records easier to name, connect, or exchange.

ADR 0015 correctly rejected a proposed one-off Erdős RO-Crate export. The
existing reader already answered the frozen tasks, and the proposal bundled a
package manager, ontology registry, normalized database, and mapping-governance
system before any of them had a demonstrated user. That NO-GO remains valid.

Later architecture and frontier-calculus work exposes a different, narrower
need:

- the Frontier Algebra needs portable, typed meanings for exact relation and
  leaf classes without making those meanings protocol primitives;
- mathematics, physics, and quantum-information Frontiers need shared terms
  without copying schemas or collapsing domain distinctions;
- workbenches need a stable adapter boundary for producing exact Vela inputs
  and retaining loss, rather than bespoke ingestion code that silently changes
  meaning; and
- readers need to display cross-Frontier relationships without treating labels,
  embeddings, database rows, or ontology inference as scientific standing.

The Frontier Algebra sharpening work also narrows the mathematical claim. The
useful core is not a universal confidence score, consensus mechanism, canonical
epistemic geometry, or proof-carrying system. It is:

```text
explain exact current justification
correct by exact substitution and recomputation
choose under an explicit, root-bound lens
```

The first two capabilities belong to a disposable Frontier Algebra projection.
The third belongs to an optional Discovery Calculus. Neither is authority.

The design question asks whether a small, versioned language layer and a strict
adapter boundary can delete duplicated mappings while preserving Vela as the
sole authority boundary. Vela does not become an ontology platform.

## Decision

Run an evidence-gated semantic-package and adapter program outside the Vela
protocol.

The architecture is:

```text
Vela Kernel
  exact objects, roots, transitions, replay, and authority

Frontier Algebra
  exact root-bound justification and correction derivations

Semantic packages
  versioned terms, constraints, mappings, and domain charts

Workbench adapters
  replaceable translations with exact inputs, outputs, and loss

Discovery Calculus and readers
  optional root-bound lenses and navigation
```

Dependencies point downward. No upper layer may mutate, sign, accept, reject,
or silently reinterpret a lower layer.

### 1. The Kernel remains the only authority boundary

This ADR adds no:

- event kind;
- canonical semantic-package object;
- reducer transition;
- signature or actor rule;
- accepted-state rule;
- hosted registry;
- canonical database;
- automatic inference;
- public mutation API; or
- authority surface in Canopus, Web, a workbench, or an adapter.

A package, mapping, adapter result, validation report, Frontier Algebra
projection, or Discovery score is a derived or producer-side artifact. If one
of those artifacts should affect scientific standing, it must enter through an
ordinary retained artifact, Receipt, proposal, verifier, and authorized Vela
decision.

### 2. One small universal grammar, many domain charts

The universal grammar contains only structural scientific-record concepts that
are already required across domains:

```text
problem
claim
claim revision
obligation
attempt
artifact
evidence
verifier observation
proposal
decision
standing
correction
relation
source
agent attribution
```

It does not define a universal scientific ontology, global truth scale,
importance metric, confidence doctrine, or fixed hierarchy of disciplines.
Domain packages add their own terms and constraints. Mathematics, experimental
physics, and quantum information may share the grammar while retaining
different objects, verifier profiles, measurement semantics, units, and
transfer rules.

The canonical terminology for product and architecture work is maintained in
[`docs/TERMINOLOGY.md`](../TERMINOLOGY.md). Protocol names remain governed by
the normative Vela specifications.

### 3. A semantic package is a content-addressed language artifact

The first candidate manifest is closed and versioned:

```text
schema: vela.semantic-package.v1
package_id
package_version
package_kind
title
license
maintainers
imports
source_roots
toolchain
exports
```

Allowed first package kinds are:

```text
grammar
domain
vocabulary
mapping
adapter_contract
lens
```

The manifest, authored sources, generated outputs, fixtures, licenses, and
toolchain lock form one canonical package root. Exact imports bind package ID,
version, Git commit, Git tree, and package root. Mutable branches, tags, URLs,
labels, or registry rows never substitute for those values.

Packages live in ordinary Git repositories. The first implementation remains
under `research/semantic-packages/` in the parent repository. Extraction to a
public repository is permitted only when:

1. two maintained consumers use the same stable contract;
2. extraction deletes duplicated implementation or generated bytes;
3. clean-clone builds are deterministic and network-disabled; and
4. removal of the package leaves every Frontier replayable and every Vela
   decision unchanged.

There is no hosted package registry in the first program. Git releases and an
exact lockfile are sufficient.

### 4. Reuse standards for their established jobs

The authored source may use:

- LinkML for modular schema authoring and deterministic generation;
- SKOS for controlled vocabularies and explicitly typed mappings;
- SHACL for closed validation of selected RDF projections;
- PROV-O for provenance interchange;
- RO-Crate for optional research-object packaging; and
- QUDT or SOSA/SSN only in domain packages whose physics or measurement
  fixtures require them.

Use is selective, pinned, and offline. Every imported schema, context,
vocabulary, generator, and generated output is hash-bound. Remote context or
schema resolution is forbidden during validation and generation.

LinkML, SHACL, OWL, JSON-LD, RDF, and other standards technologies do not
become a Vela protocol dependency. OWL reasoning is off by default. A generated
OWL artifact is an interoperability export, not a source of Vela facts.

### 4.1 Verification-economy and operating-system boundary

The package layer may name a verifier profile, validation scope, limitation,
assurance dimension, independence disclosure, or common-mode dependency. Those
records make a check interpretable. They do not turn the check into authority
or compress heterogeneous assurance into one score.

The boundary remains:

```text
candidate artifact
  -> scoped verifier observation
  -> explicit residual uncertainty
  -> ordinary Vela proposal and route
  -> authorized standing or explicit non-admission
```

Negative, failed, and inconclusive attempts may remain addressable evidence so
that later work does not erase or repeat them. Their retention does not place
them in accepted scientific state.

This package program is a small transition envelope between existing systems,
not a universal scientific ontology or a claim that Vela is the complete
scientific operating system. Domain languages, workflow engines, repositories,
and provenance standards keep their established jobs. Adapters must state
preserved, omitted, approximated, scope-restricted, and unsupported semantics
rather than pretending every translation is exact.

No validator marketplace, reputation score, accreditation system, protocol
foundation, or multi-authority merge layer is part of this ADR. Those require
independent use and governance evidence beyond first-party package work.

### 5. Every mapping declares its consequence tier

Mappings are closed records with exact source and target package roots,
direction, evidence, maintainer, version, and one consequence tier:

```text
discovery
organization
identity
logical_transport
empirical_transport
```

The default is `discovery`.

- `discovery` permits search and navigation only.
- `organization` permits grouping under an explicit scheme.
- `identity` asserts co-reference under named evidence but does not merge
  canonical Vela objects.
- `logical_transport` requires an exact proof-producing or
  proof-checkable transformation and every declared premise.
- `empirical_transport` requires an explicit causal or measurement model,
  scope, uncertainty, and calibration evidence.

`skos:closeMatch`, `skos:exactMatch`, `owl:sameAs`, a shared label, embedding
similarity, database equality, or graph proximity never automatically upgrades
the consequence tier. Heuristic analogy cannot transport standing.

Semantic validity, mapping evidence, package governance, verifier success, and
scientific authority remain distinct.

### 6. Frontier Algebra consumes exact relations, not inferred truth

ADR 0017 remains the mathematical boundary. A portable Frontier Algebra
projection binds:

- exact Frontier identity, Git commit and tree;
- event, scientific-state, proposal, actor, artifact, and dependency roots
  applicable to the selected claim;
- exact claim-revision root;
- complete source-object roots;
- semantic package and mapping roots;
- closed derivation-rule root;
- circuit canonicalization policy;
- projector version and implementation root; and
- completeness and truncation limits.

Packages may define the type and admissible consequence of a retained relation.
They cannot fabricate the relation, turn a dependency pin into evidence, or
turn a verifier result or decision into scientific support.

The projector returns the ADR 0017 planes separately:

```text
scientific support and opposition
artifact integrity
verifier reproduction
statement faithfulness
transfer validity
Kernel authority standing
```

It may derive minimal routes, shared origins, cut sets, blast radius, surviving
routes, and bounded repairs. It may not multiply those planes into one
confidence score.

The current source-local projector and Erdős 646 diagnosis are valid research
evidence. They are not portable because they do not yet carry a versioned
semantic package, complete retained inputs, or a successful correction vector.

### 7. Discovery remains an explicit lens

A Discovery lens binds a rooted state, resolution space, action catalogue,
outcome model, verifier model, authority outcomes, cost, risk, uncertainty,
and optional utility. It reports only:

```text
best under lens L
at exact state R
under assumptions A
with uncertainty U
```

Information, utility, cost, verification burden, time, risk, and uncertainty
remain a vector unless the named lens defines an optimization rule. A lens
cannot rewrite Target Index ordering, `vela next`, standing, or accepted
state.

The Sidon finite-information fixture remains the first candidate. Failure of
that fixture rejects or narrows the Discovery program without invalidating the
Frontier Algebra.

### 8. Workbench adapters are replaceable producer edges

An adapter contract binds:

```text
adapter_id and version
source workbench and export version
input byte roots
semantic package and mapping roots
output Receipt/artifact schema versions
output byte roots
loss report root
commands and environment
implementation and dependency roots
```

An adapter must:

1. operate on exact, retained source exports;
2. validate against the pinned package and mapping roots;
3. preserve source identity, attribution, caveats, and unknown fields in a
   machine-readable loss report;
4. emit ordinary Vela-compatible producer artifacts or a diagnostic failure;
5. rebuild byte-identically in a clean, network-disabled environment; and
6. remain removable without changing canonical Frontier history.

An adapter must never:

- read or invoke a human key;
- sign a Vela decision;
- infer accepted standing;
- treat a verifier pass as acceptance;
- silently select a different target or source revision;
- use a mutable database as canonical input;
- require raw private agent history when an explicit export suffices; or
- become a general research IDE.

Canopus may host a bounded adapter profile when the adapter is part of a
mission. A source workbench may host its own exporter. Neither location changes
the contract or grants authority.

## Migration and compatibility

There is no Vela protocol migration in the Proposed phase.

- Existing canonical events, findings, artifacts, Receipts, proposals,
  policies, actor records, signatures, and Git history remain byte-identical.
- Repository Profile v1 remains closed and correctly excludes `packages` and
  `adapters`. Semantic package selection belongs in a non-authoritative
  workspace or analysis lock, not `frontier.yaml`.
- Existing readers and source-local transforms remain until package-backed
  parity is proved. Each duplicated transform is deleted only after exact
  output and loss comparison.
- Existing Frontiers replay without semantic packages. Missing packages make
  optional analysis unavailable; they do not alter Kernel standing.
- Existing ADR 0015 evidence and artifacts remain historical. ADR 0019 does
  not retroactively implement the rejected export.
- A later protocol proposal is permitted only if a reproduced integrity or
  authority gap cannot be solved by exact artifacts, locks, and ordinary Vela
  decisions. Interoperability convenience is not sufficient.

## Adversarial and failure cases

The package, projector, and adapter validators fail closed on:

- mutable, missing, dirty, shallow, forked, or root-mismatched source inputs;
- short digests or labels substituted for exact roots;
- unpinned or remotely resolved imports and JSON-LD contexts;
- cyclic package imports;
- undeclared generated files or generator drift;
- unknown state-carrying terms or mapping consequence tiers;
- automatic standing transport through `sameAs`, `exactMatch`, embeddings, or
  graph position;
- an adapter dropping caveats, attribution, source revisions, negative
  results, corrections, or unknown fields without recording loss;
- a logical transfer without a checkable transformation and all premises;
- an empirical transfer without scope, uncertainty, and calibration evidence;
- a Frontier Algebra circuit that merges authority, verification,
  faithfulness, or scientific-evidence planes;
- a lens whose score is presented without its state, model, assumptions, or
  uncertainty;
- nondeterministic generation or output;
- stale package locks;
- database rows presented as canonical objects; or
- any package, adapter, reader, or lens claiming to accept scientific state.

Diagnostic mode may report errors and draft loss. It may not emit a valid
package, portable projection, or successful adapter result.

## Exact evidence and conformance gates

### Gate A: terminology and package kernel

- The terminology document contains no duplicate or circular ownership.
- A minimal LinkML-authored grammar generates pinned JSON Schema and SHACL
  outputs byte-identically in two clean, network-disabled paths; otherwise a
  documented LinkML NO-GO selects the smaller hand-authored closed contract,
  which must pass the same path-independence and offline checks.
- The canonical package root changes for every semantic source, import,
  generated-output, license, or toolchain mutation.
- Remote imports, cycles, unknown fields, wrong roots, and unreported output
  fail.

### Gate B: exact Erdős vertical slice

- Freeze one statement, proof/fidelity record, correction, proposal, decision,
  and downstream relation at exact released roots.
- Build one Erdős domain package and one mapping package.
- Use the same package roots in the Frontier Algebra projector and the
  Observatory reader.
- Require identical typed relation identity and a complete loss report.
- Show one successful correction where minimal routes, cut sets, standing
  change, surviving routes, and repair requirements rederive from exact bytes.
- Preserve the current negative Erdős 646 result as a separate regression.

### Gate C: two-consumer extraction

- Two maintained consumers depend on one stable package contract.
- Extraction removes their duplicated schema or mapping code.
- Package removal breaks only optional analysis or display, never Vela replay.
- Clean-clone builds and fixture outputs agree across macOS and Linux.

Only after this gate may a separate public semantic-package repository and
`v0.1.0` release be proposed.

### Gate D: cross-domain bridge

- Add one mathematics package and one physics or quantum-information package.
- Freeze an explicit bridge with a declared consequence tier.
- Reject label-based, embedding-based, and wrong-tier substitutions.
- Demonstrate that the bridge improves a measured reuse task without
  transporting standing or authority.

Evidence result on 2026-07-24: PASS for one exact
mathematics-to-quantum-information slice. The source-local `vela.math`,
`vela.quantum`, and `vela.mapping.math-quantum` packages bind the clean
`quantum:[[10,1,4]]` witness and five retained source roots. A dependency-free
test rederives commutation, rank, encoded dimension, and the complete 3,675
error bounded check. Eight label, embedding, identity, premise, verifier,
measurement-context, root, and standing substitutions fail closed. The
retained Receipt remains `not_assessed`; the proposal remains
`pending_review`. No physics package is created without a selected measurement
fixture. No release or extraction is authorized, and this ADR remains
Proposed.

### Gate E: workbench adapter

- Select one stable, licensed, source-local export from an external workbench.
- Record exact source, adapter, package, mapping, output, and loss roots.
- Rebuild in a clean network-disabled environment.
- Land only through an ordinary Receipt and retain `pending_review`, Defer, or
  the already authorized policy result.
- Show that removing the adapter leaves the Frontier and Vela replay intact.

Evidence result on 2026-07-24: PIVOT. Entire CLI `v0.8.42` provides one real
MIT-licensed, metadata-only checkpoint export from a public release commit. A
dependency-free source-local adapter binds the released binary, source commit
and tree, checkpoint ref/commit/tree, 704 source bytes, result, and complete
loss report. It rejects mutations, partial exports, count drift, unsafe paths,
network access, and Vela mutation edges.

The adapter correctly classifies the export as process provenance with no
scientific or authority effect. It emits no Receipt because the source has no
scientific claim, retained artifact, verifier execution, statement-fidelity
record, proposal, or decision. The translation half of Gate E passes; the
scientific Receipt/proposal half remains NO-GO. OpenScience `v1.3.4` fails the
released-binary and explicit-export-version criteria, and OpenResearch
`v0.1.77` has no license at the exact tag. No protocol, package, adapter
release, or authority change follows, and this ADR remains Proposed.

Focused checks for the candidate implementation are:

```bash
bun test research/semantic-packages
bun test research/workbench-adapters
python3 -m unittest discover -s research/frontier-calculus/tests -p 'test_*.py'
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
git diff --check
```

External Lean is used only for a named Frontier Algebra or domain-package
theorem gate. Broad release suites, live-network tests, and unrelated domain
verifiers are excluded until an actual release boundary selects them.

## Alternatives

### Keep every schema local

This remains the fallback if two consumers never need the same contract. It is
simple, but repeated domain and relation mappings become drift once calculus,
adapters, and readers need identical meanings.

### Put domain terms in the Vela protocol

Rejected. It would couple protocol compatibility and authority replay to
evolving scientific vocabularies and generators.

### Build a universal ontology and hosted registry now

Rejected. No evidence justifies a global hierarchy, inference service, custom
resolver, canonical semantic database, or second governance system.

### Use only embeddings or an unrestricted property graph

Rejected for state-carrying relationships. They are useful discovery aids but
cannot provide exact identity, declared consequence, deterministic replay, or
fail-closed transport.

### Vendor workbenches into Vela

Rejected. Vela should connect existing research systems through exact exports
and replaceable adapters, not clone their workflow, process-data, or IDE
surfaces.

### Treat standards validation as scientific verification

Rejected. Schema and shape validation prove structural conformance only.
Scientific verification and authority remain explicit Vela lanes.

## Why this reduces friction without weakening authority

Scientists and agents can use stable domain terms, exact mappings, and familiar
workbench exports without learning Vela's internal storage or rebuilding
bespoke transforms. Calculus and readers can share one typed relation contract.
Domain packages can evolve independently of the protocol.

The authority boundary does not move. Packages say what terms and mappings
mean. Adapters say how bytes were translated and what was lost. Frontier
Algebra says what exact current routes exist under closed rules. Discovery
lenses say what action is preferred under a declared model. Only Vela replay
and an already authorized decision say what scientific standing is.

## References

- [LinkML modular imports](https://linkml.io/linkml/schemas/imports.html)
- [LinkML generators](https://linkml.io/linkml/generators/dashboard.html)
- [W3C SKOS Reference](https://www.w3.org/TR/skos-reference)
- [W3C SHACL](https://www.w3.org/TR/shacl/)
- [W3C PROV-O](https://www.w3.org/TR/prov-o/)
