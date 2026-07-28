# Vela architecture

Vela is version control for living science.

Its product category is an open scientific-state and compounding layer: the
portable boundary that records what is claimed, evidenced, checked, decided,
corrected, inherited, and safe to do next. The longer-range direction is a
federated inheritance layer for science. That direction remains a hypothesis
until different producers and readers demonstrate cheaper continuation,
correction handling, and downstream reuse.

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
  Runs, branches, traces, notebooks, attempts, raw artifacts

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

Mathematics is the first complete domain proving ground, not a second Kernel.
Lean and other proof assistants retain proof checking, native package managers
retain dependency resolution, source communities retain their identifiers and
review processes, and each Frontier retains bounded authority. A future Vela
Math profile must earn extraction through two maintained consumers and deleted
duplication before it becomes a shared package.

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
   runtime, or reader.

`status`, `show`, and `why` are the current read path. A scientific-state
comparison surface is worth adding only when a registered cold-use test shows
that it reduces evidence-location or correction time without inventing a new
canonical object or authority layer.
