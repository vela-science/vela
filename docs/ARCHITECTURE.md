# Vela architecture

Vela is version control for living science.

Its technical category is a Git-native protocol and CLI for governed,
replayable scientific-state transitions: the portable boundary that records
what is claimed, evidenced, checked, decided, corrected, inherited, and safe
to do next. Its long-range strategic category is scientific state and
inheritance infrastructure. That direction remains a hypothesis until
different producers and readers demonstrate cheaper continuation, correction
handling, and downstream reuse.

The product hierarchy is:

```text
protocol  -> integrity layer
map       -> product
verified frontier movement -> outcome
```

The protocol earns its complexity by making a living Frontier map exact,
current, and actionable. The map must show what is accepted, checked,
recorded, disputed, missing, and next. A transition compounds only when it
changes that map and improves a later valid action.

The product story is:

```text
map -> target -> run -> verify -> commit -> compound
```

`Commit` is product language for the existing authorized Decision, Event, and
exact before/after root transition. It is not a new canonical object or an
automatic consequence of verification, publication, or a Git merge.

A Frontier is one ordinary Git repository for a bounded scientific scope.
Vela defines the portable records, replay rules, repository authority, and
Decision boundary. Producers and readers remain replaceable.

## Scientific-state flow

```text
workbench
   |
   +-- Attempt -- Submission ------------------+
   |                                           |
verifier -- Verification Record ---------------+
                                               v
                                      pending Proposal
                                               |
                                     authorized Decision
                                               |
                                               v
                                         Event + Standing
                                               |
                                               v
                                    read-only projection
```

Each object has one job:

| Object | Meaning | Authority effect |
| --- | --- | --- |
| Attempt | Local bounded work against exact starting roots | None |
| Submission | Authenticated producer input | Requests change |
| Registration Record | Proof that exact input entered the repository | None |
| Verification Record | Scoped verifier observation over exact inputs | None |
| Claim Record | Versioned assertion and evidence identity | Standing is derived |
| Proposal | Candidate state transition | None until decided |
| Decision | Authorized accept or reject action | Determines transition |
| Event | Canonical admitted transition | Changes replay |
| Standing | Root-bound replay result | Read projection |

Git publication is not acceptance. Verification is not acceptance. A
signature authenticates exact bytes; it does not establish that a Claim is
true.

## Four architectural planes

```text
Activity plane
  Runs, branches, traces, notebooks, attempts, raw artifacts,
  external agent-session checkpoints

Scientific-state plane
  Claims, Submissions, Verification Records, Decisions, Events, Standing

Package plane
  optional schemas, corpora, verifiers, mappings, adapters, and locks

Discovery plane
  Observatory, search, graphs, rankings, and generated explanations
```

Vela owns the scientific-state boundary. Workbenches own activity. Packages
make language and capability reusable but confer no Standing. Discovery
surfaces are root-bound, rebuildable readers.

External activity recorders may preserve session context, prompts, traces,
checkpoints, or workbench history. They remain non-authoritative
activity-plane systems. Their records may support provenance, review, or
continuation, but they cannot create a Verification Record, Decision, Event,
or Standing and are never required for Vela replay.

Mathematics is the first complete domain proving ground, not a second Kernel.
Lean and other proof assistants retain proof checking, native package managers
retain dependency resolution, source communities retain their identifiers and
review processes, and each Frontier retains bounded authority. A future Vela
Math profile must earn extraction through two maintained consumers and deleted
duplication before it becomes a shared package.

## Native-system interoperability

Vela composes with mathematical and scientific systems through three distinct
operations:

```text
reference  preserve an exact native identity
snapshot   retain the exact bytes and environment needed for reproduction
admit      let one named Frontier decide a bounded proposed transition
```

Reference is not snapshot. Snapshot is not Verification. Verification is not
admission. An external object may be broadly discoverable without being copied
into Vela, and a reproducible artifact may remain unaccepted.

Vela Math is a domain profile over current objects, not a proof language,
Kernel extension, universal ontology, or second package resolver. Its first
source-local views may describe Problems, Mathematical Claims,
Formalizations, Results, Obligations, and statement-fidelity reviews while
binding native commits, paths, declarations, toolchains, manifests, and
artifacts exactly. Consequential cross-system mappings require explicit scope
and loss reports; similarity or graph proximity never transports Standing.

## Source and repository ownership

The target public topology is intentionally small:

| Repository | Sole responsibility |
| --- | --- |
| `vela-science/vela` | Product monorepo: Rust implementation, TypeScript protocol SDK, optional Canopus producer, shared schemas, conformance, releases, architecture |
| `vela-science/vela-web` | Editorial site and read-only Observatory |
| `vela-science/erdos-frontier` | Canonical Erdős Frontier |
| `vela-science/formal-conjectures-frontier` | Canonical formal-conjectures Frontier |
| `vela-science/sidon-frontier` | Canonical Sidon Frontier |
| `vela-science/quantum-codes-frontier` | Canonical quantum-codes Frontier |
| `vela-science/.github` | Organization profile, reusable workflows, security policy, repository templates |

The former `vela-science/vela-research-harness` history is preserved
unsquashed under `packages/canopus`; that repository is archived rather than
maintained as a mirror.

The former private `vela-science/vela-internal` integration repository is
archived at its final tombstone commit. Its load-bearing checks and current
documents moved to their owners, and its historical Git tree remains
reachable through `pre-decomposition/2026-07-28`. The public Erdős Frontier
contains byte-exact mirrors for every historical source object that cited the
private repository.

## Release boundaries

Each component releases independently from the product monorepo and publishes
its own version, commit, artifacts, checksums, provenance, and supported
interface versions.

- Vela releases the protocol implementation and CLI.
- The TypeScript protocol package is generated or checked against the same
  public schemas and fixtures as Rust.
- Canopus checks capabilities, invokes released Vela binaries through the
  public boundary, and pins exact binaries in every Run.
- Each Frontier verifies and reproduces its own exact state.
- Vela Web verifies its read projection against exact Frontier sources.
- Organization workflows test the compatibility matrix without becoming a
  canonical writer or synthetic ecosystem release.

An exact scientific Run still pins every binary and digest it used. Colocated
source does not grant Canopus authority access, and component versions do not
move in lockstep.

## Non-goals

The architecture does not add:

- a hosted authority or second writer;
- a canonical database or public mutation API;
- a universal ontology or work graph;
- a mandatory model runner;
- a package Registry before reusable packages exist;
- an Atlas service before exact cross-Frontier value is measured;
- a private meta-repository that outsiders must understand;
- or an integration repository without an installable distribution.

Private memos and exploratory work remain noncanonical and non-release-gating.
They do not become product infrastructure merely because they are useful.

## Product performance

Vela is not evaluated by record count, graph size, workflow completion, or
model activity. The useful performance functions remain separate:

1. verified bounded artifacts per all-in cost and expert-minute;
2. correct Decisions and correction comprehension per reviewer-minute; and
3. time to the first useful downstream action after changing producer,
   runtime, or reader; and
4. map correctness: coverage disclosure, stale-state rate, valid Target rate,
   Decision-to-remap latency, and cold-user comprehension.

The current read path is `status`, `show`, and `why`; the existing Observatory
is the public map surface. A new map, diff, or comparison feature is worth
adding only when it answers one named Frontier question and a registered
cold-use test shows that it reduces evidence-location, correction, or
continuation time without inventing a canonical object or authority layer.
