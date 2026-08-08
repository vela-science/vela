# Vela architecture

Vela is version control for scientific state.

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

The protocol earns its complexity by making the current state of a Frontier exact,
current, and actionable. The map must show what is accepted, checked,
recorded, disputed, missing, and next. A transition compounds only when it
changes that map and improves a later valid action.

In product language, the observable universe is exact, attributed, scoped,
inspectable scientific state. The unobservable frontier is the set of explicit
Obligations that follow from that state. Unknown unknowns remain unknown; Vela
does not manufacture completeness by turning missing source coverage into
graph nodes.

The exact operator loop is:

```text
init -> submit -> verify -> decide -> replay
```

The research-navigation story around it is:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

The three-word compression is `MAP -> ADVANCE -> REMAP`. Work remains native;
Submission is the portable evidence boundary; Verification reports a scoped
check; Decision is the only authority boundary; and remapping derives the
current territory and next valid work from the new exact root.

A Frontier is **action-complete only within declared bounds** when every
represented unresolved item yields either a fresh, root-bound Target with an
exact completion contract or an explicit blocker explaining why no current
Target can be offered. This is a product and evaluation property, not a new
protocol object. A map, Target generator, ranking, or learning policy remains
read-only and cannot change Standing.

A Frontier is one ordinary Git repository for a bounded scientific scope.
Vela defines the portable records, replay rules, repository authority, and
Decision boundary. Producers and readers remain replaceable.

## Scientific-state flow

```text
workbench
   |
   +-- native run -- Submission ---------------+
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
| Native run | Activity retained by an external agent or scientific tool | None |
| Submission | Authenticated producer input | Requests change |
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

The default integration posture is:

```text
reference broadly
snapshot selectively
admit narrowly
```

Adapters preserve native identifiers, source revisions, content roots,
licenses, the exact interpreting implementation, and an explicit account of
preserved, omitted, approximated, and unsupported meaning. They reconcile
candidate identities without silently merging disagreement. Vela does not
operate an ingest-all scientific warehouse.

Vela Math is a domain profile over current objects, not a proof language,
Kernel extension, universal ontology, or second package resolver. Its first
source-local views may describe Problems, Mathematical Claims,
Formalizations, Results, Obligations, and statement-fidelity reviews while
binding native commits, paths, declarations, toolchains, manifests, and
artifacts exactly. Consequential cross-system mappings require explicit scope
and loss reports; similarity or graph proximity never transports Standing.

The Math Source Registry uses real source-native adapters. Each adapter owns
the source's identifier, revision, pagination, completeness, deletion,
tombstone, rights, and snapshot rules. Shared transport and hashing helpers may
reduce code, but a generic mathematical-record importer cannot replace these
source contracts.

Adapters emit immutable rooted observations. Observation identity does not
include a Vela Web release, so later read releases can reference the same
observation without copying or rewriting it. Frontier bindings are separate,
release-scoped relations that state whether one Frontier object references,
snapshots, or admits a native record. They do not alter the observation or
transport Standing.

## Math Atlas read boundary

The existing Observatory is the first-party Math Atlas. Git Frontiers remain
canonical. `@vela/frontier-data` acquires and validates source-native
observations, projects exact Frontier state, and loads one disposable
PostgreSQL read model.

Candidate loads use bounded 1,000-row PostgreSQL recordsets inside one
transaction. The projector checks chunk counts, table roots, release
membership, and Frontier bindings before moving `current_release`. It does not
issue one insert per record or store a source as a giant JSONB document.

Collection reads use keyset pagination over a stable sort key and full object
ID. Each cursor binds the release root and filters. Graph reads return bounded
typed neighborhoods with returned and hidden counts, plus an equivalent
keyset-paginated ledger. Ordinary routes never load the full graph.

Neon has one branch: `main`. Schema rehearsal and scale benchmarks use local
or ephemeral PostgreSQL. Immutable release rows and the atomic release pointer
provide rollback; database branch ceremony is not part of the pre-release
product.

The current alpha gate includes a rooted 100,000-record load and read
benchmark. A separate 1,000,000-record benchmark must pass before Vela makes a
scalability claim. Table partitioning, graph databases, vector or embedding
stacks, streaming ingestion, and a second read store require a measured failed
budget in the simpler PostgreSQL design.

## Source and repository ownership

The target public topology is intentionally small:

| Repository | Sole responsibility |
| --- | --- |
| `vela-science/vela` | Product monorepo: Rust implementation, independent conformance readers and fixtures, releases, architecture |
| `vela-science/vela-web` | Editorial site and read-only Observatory |
| `vela-science/math` | The mathematics authority: one repository, one trust root |
| `vela-science/.github` | Organization profile, reusable workflows, security policy, repository templates |

Archived, preserved exactly as signed and no longer developed:
`vela-science/erdos-frontier`, `vela-science/sidon-frontier`,
`vela-science/formal-conjectures-frontier` and
`vela-science/quantum-codes-frontier`. They existed because there were four
topics, not four authorities — one maintainer and one decision model between
them. ADR 0039 states the rule they broke: a repository exists because there is
a new authority, never because there is a new topic. Tooling from `0.967`
onward does not read them.

The former `vela-science/vela-research-harness` repository and immutable
Canopus release remain archived historical evidence. Current source does not
carry a copy or compatibility layer.

The former private `vela-science/vela-internal` integration repository is
archived at its final tombstone commit. Its load-bearing checks and current
documents moved to their owners, and its historical Git tree remains
reachable through `pre-decomposition/2026-07-28`. The public Erdős Frontier
contains byte-exact mirrors for every historical source object that cited the
private repository.

## Release boundaries

Changed Vela product artifacts release from one Vela source tag and manifest.
Public components retain their own versions, artifacts, checksums, provenance,
and supported interface versions; unchanged components do not churn merely to
match the source tag.

- Vela releases the protocol implementation and CLI.
- An independent Python reader checks the portable object waist without
  creating a second package or release surface. Two clean-room emitters,
  `conformance/emitters/javascript.mjs` and `conformance/emitters/python.py`,
  sign and write Submission and Verification objects; neither reads the vector
  corpus. `conformance/readers/` holds `python` alone, and
  `scripts/ecosystem-status.py` declares `conformance/readers/javascript`
  absent and fails if it appears.
- Immutable Canopus `0.8.0` remains frozen for historical Runs that bind its
  exact bytes. Current Vela contains no executor or separate runner release.
- Each Frontier verifies and reproduces its own exact state.
- Vela Web verifies its read projection against exact Frontier sources.
- Organization workflows test the compatibility matrix without becoming a
  canonical writer or synthetic ecosystem release.

An exact scientific Run still pins every binary and digest it used. Colocated
source does not grant an executor authority access, and component versions do
not move in lockstep. One Vela source tag and manifest coordinate changed
artifacts without erasing their component versions.

Package and network surfaces follow a strict maturity ladder:

| Level | Surface | Entry gate |
| --- | --- | --- |
| 0 | Source-local profile | One reproduced semantic or adapter recurrence |
| 1 | Shared immutable package | Two maintained consumers and net deletion of maintained duplication |
| 2 | Static read-only index | Released package reuse and measured integration benefit |
| 3 | Hosted Registry | External publishing demand and Git-release friction |
| 4 | Federated read-only Atlas | Exact cross-Frontier correction, independent agreement, and cold-user lift |

Only level 0 of this **package** ladder is active. A package carries reusable
language or capability but confers no Standing. A package Registry distributes
packages but is not scientific authority.

### Rust ecosystem comparison

Rust separates the Cargo client and resolver, the crates.io publication
service, the immutable registry index, docs.rs derived documentation, and
rustup toolchain distribution because each has an independently demonstrated
operational boundary. Vela adopts that separation of responsibilities without
copying the repository or service count prematurely:

- native package managers continue to resolve native dependencies;
- source-local Vela profiles remain in their owning repositories at level 0;
- if level 1 is earned, reusable package sources move to one
  `vela-science/vela-packages` repository with exact Git releases and content
  roots, not to a repository named `vela-registry`;
- package inspection, validation, and acquisition remain subcommands of the
  existing `vela` executable rather than a second package-manager binary;
- if level 2 is earned, the existing Vela Web deployment generates and serves
  a root-bound sparse read index over released packages; and
- a stateful publication service is level 3 work, justified only by external
  publishers who require namespace delegation, authentication, yanking,
  advisories, and operational ownership.

Package bytes, index entries, generated documentation, and discovery records
remain replaceable distribution artifacts. None is a Frontier, Decision,
Event, or source of Standing. Until level 1 is earned, there is no new package
repository, package command surface, index, database, or service to maintain.

Accepted ADR 0030 defines the current Math Source Registry because exact
observation must bind native source identity, rights, snapshots, adapters,
coverage, and omissions before any Atlas view is trustworthy. That deployed
inventory does not distribute semantic packages or confer Standing.

The first-party Math Atlas is the existing Observatory over the one live
mathematics authority, `vela-science/math`, and the registered native sources.
It read "the four declared Frontiers" until this line was corrected, which bound
the Atlas to the four repositories ADR 0039 archived. Level 4 means a later
federated Atlas over independently governed external Frontiers. Both are read
projections, never canonical databases or writers.

## Non-goals

The architecture does not add:

- a hosted authority or second writer;
- a canonical database or public mutation API;
- a universal ontology or work graph;
- a mandatory model runner;
- preemptive partitioning, graph-database, vector, or streaming infrastructure;
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
