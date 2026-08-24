# Vela architecture

Vela is the open protocol for replayable, authority-scoped, correction-aware
scientific state transitions. **Version control for scientific state** is the
public shorthand.

The CLI is the Git-native implementation of the portable boundary that
records what is claimed, evidenced, checked, decided, corrected, and inherited.
Its long-range role as scientific-state and inheritance infrastructure remains
a hypothesis until different producers and readers demonstrate cheaper
continuation, correction handling, and downstream reuse.

The product hierarchy is:

```text
protocol  -> integrity layer
map       -> product
verified frontier movement -> outcome
```

The protocol earns its complexity by making the current state of a Repository
exact, current, and actionable. The map must show what is accepted, checked,
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

The architecture keeps four kinds of material separate:

| Kind | Contents | Authority effect |
| --- | --- | --- |
| Normative protocol | Protocol 1, its normative schemas, and conformance vectors | Defines canonical objects, replay, and the Decision boundary |
| Derived views | Projections, indexes, Problems views, and Frontiers over current Standing | Read-only, rebuildable, and no authority |
| External activity and control | Controllers, agents, attempts, runs, campaigns, schedulers, and workflows | Source-owned activity that may produce a Submission but is not Core state |
| Speculative research | Papers, benchmarks, exploratory reducers, and unvalidated product hypotheses | Evidence or research only; never protocol semantics or Standing |

Only [Protocol 1](PROTOCOL.md) and the interoperability material it marks
normative define protocol behavior. This architecture is explanatory.

The public navigation loop around it is:

```text
MAP -> ADVANCE -> REMAP
```

**Map** reads Problems, Claims, Standing, dependencies, Corrections, and open
Obligations from exact roots. **Advance** is native human or machine work that
may produce a bounded proposed change. **Remap** replays the resulting Standing
and derives the current territory, correction consequences, blockers, and next
valid work from the new exact root. Work remains native; Submission is the
portable evidence boundary; Verification reports a scoped check; and an
authorized Decision is the only operation in this loop that can change
Standing.

A Repository is **action-complete only within declared bounds** when its
source-owning adapter or read product turns every represented unresolved item
into either an exact next obligation with a completion contract or an explicit
blocker. This is a product and evaluation property, not a protocol object. A
map, obligation generator, ranking, or learning policy remains read-only and
cannot change Standing.

A Repository is one ordinary Git repository for a bounded scientific scope,
and it is the local authority boundary. A Frontier is a derived query over
current Standing inside one or more Repositories: it has no persistent
identity, owns no records or authority, and is never a governed repository
(ADR 0039). Vela defines the portable records, replay rules, repository
authority, and Decision boundary. Producers and readers remain replaceable.

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
  problems.science, search, graphs, rankings, and generated explanations
```

Vela owns the scientific-state boundary. Workbenches own activity. Packages
make language and capability reusable but confer no Standing. Discovery
surfaces are root-bound, rebuildable readers.

## Protocol and ecosystem

The **Vela Protocol** is the narrow scientific-state waist: canonical Claims,
authenticated Submissions, scoped Verification Records, Proposals, authorized
Decisions, Events, exact roots, replay, correction, and derived Standing. The
**Vela ecosystem** is wider. It includes sovereign workbenches, source-owning
Repositories, verifiers, package ecosystems, the CLI, and read products. An
ecosystem component does not enter the Protocol merely because Vela ships or
links to it.

The [standards disposition](STANDARDS_DISPOSITION.md) maps the explanatory
conceptual waist to these current objects and records which generic facts stay
owned by native systems and existing standards.

First-party products are ordinary components with the same boundaries as
third-party ones. The Problems surface is a rebuildable root-bound projection;
source registries and generated graphs are disposable readers; and any
first-party workbench owns activity only.
Deleting or replacing one changes no canonical object, Decision, or Standing.
The CLI is the one operator product over the Protocol, not a second authority.

## Three graphs

The ecosystem has three related conceptual graph views, not one universal
knowledge graph. They are ownership separations, not canonical Vela objects,
wire fields, required stores, or a mandate to materialize three databases.

| Graph | Contents | Owner and authority effect |
| --- | --- | --- |
| Working graph | Problem, gap, hypothesis, experiment, result, candidate Claim | A native workbench or laboratory; mutable activity with no Standing effect |
| Standing graph | Claim, Evidence, Verification, Decision, Standing, dependency, Correction, Reassessment | Deterministic readers over Repository state; derived, rebuildable, root-bound, and authority-aware |
| Metascience graph | Program design, review process, resources, intervention, and measured outcomes | An evaluation system; evidence for institutional judgment, never scientific Standing |

The crossings are controlled. A working graph may export a signed bounded
Submission. A root-bound Problem or Obligation view may orient native work. A
metascience result may inform a separate institutional process. Workbench
success, a favorable metric, or funding never becomes scientific Standing
without the ordinary Verification and Decision boundary.

## Four clocks

The layers also run on four conceptually distinct clocks. These are ordering
and governance boundaries, not protocol timestamps, wire fields, service-level
agreements, or prescribed schedules:

| Clock | Ordering responsibility |
| --- | --- |
| Research iteration | Native branches, experiments, and hypotheses may change fastest |
| Verification and Decision | Scoped checks and deliberate review follow bounded producer activity |
| Correction and inheritance | Dependency reassessment follows an admitted change while historical Standing remains inspectable |
| Institutional learning | Programs and governance change only through their own measured evidence and review |

The architecture does not force all layers onto the research clock. Activity
may be continuous; scientific authority remains deliberate; institutional
rules change under their own evidence and review.

External activity recorders may preserve session context, prompts, traces,
checkpoints, or workbench history. They remain non-authoritative
activity-plane systems. Their records may support provenance, review, or
continuation, but they cannot create a Verification Record, Decision, Event,
or Standing and are never required for Vela replay.

## Rust runtime boundaries

The workspace has one semantic kernel and one separate durability boundary:

```text
vela-protocol <- vela-repository <- vela-cli
```

`vela-protocol` owns canonical scientific objects, bytes, roots, events, and
replay. `vela-repository` owns policy-neutral, path-bound filesystem
transactions and their private recovery journal. It knows no Fresh, Routine,
RepositoryAuthority, signer, keyset, model, Event, Decision, or Standing
semantics. The CLI owns those concrete write policies and supplies one
move-only, in-memory authorization with exactly two lifecycle checks: bind the
verified plan before journal bytes and revalidate immediately before the
commit marker. That capability is never serialized; once a valid marker
exists, exact idempotent installation is policy-free.

Production recovery reaches that same engine through one advanced action,
`vela recover --repo <PATH> <OPERATION_ID>`. The CLI supplies parsing and
rendering only; `vela-repository` acquires the repository-wide lock, opens the
exact named journal, aborts an uncommitted Prepared transaction only when the
marker is definitely absent, or completes the exact marker-authorized
installation without Vela policy. The action is terminal and idempotent: it
does not resume a semantic command, start another write, or publish Git state.

Fresh initialization has one CLI-owned post-transaction continuation, not a
runtime policy. After the runtime proves the exact named operation Completed
and returns only its root-validated delta, read set, and closed private-residue
census, the CLI matches those facts to the signed sequence-one authority
record, reconstructs every retained scaffold byte, revalidates the local
account/key/reason, and creates the one deterministic parentless Git commit and
trust pin. No signer or write authorization participates. `vela recover`
merely names that later exact `vela init` action and never performs the tail.
The private `vela-cli` `config::authority_trust` module owns the trust schema,
OS-account-local path, exact loading, installation, and rebind. Those mechanics
are neither CLI-owned derived analysis nor `vela-repository` transaction state.

The three `WriteClass` spellings remain frozen journal vocabulary because
renaming them would change durable roots. The runtime orders those labels but
does not attach Vela authority or scientific meaning to them.

`vela-authority` remains the restricted authorization and service-signing
implementation. Derived correction analysis and small
non-authoritative process adapters live directly with their sole `vela-cli`
consumer. All workspace crates are internal implementation boundaries released
through the single `vela` product identity; the crate split creates neither a
plugin system nor another public product.

Mathematics is the first complete domain proving ground, not a second Kernel.
Lean and other proof assistants retain proof checking, native package managers
retain dependency resolution, source communities retain their identifiers and
review processes, and each repository retains bounded authority. A future Vela
Math profile must earn extraction through two maintained consumers and deleted
duplication before it becomes a shared package.

## Native-system interoperability

Vela composes with mathematical and scientific systems through three distinct
operations:

```text
reference  preserve an exact native identity
snapshot   retain the exact bytes and environment needed for reproduction
admit      let one named Repository decide a bounded proposed transition
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
observation without copying or rewriting it. Repository bindings are separate,
release-scoped relations that state whether one Repository object references,
snapshots, or admits a native record. They do not alter the observation or
transport Standing.

## Problems read boundary

The Problems surface is the first-party root-bound public reader. Git Repositories
remain canonical. `@vela/projection-data` acquires and validates source-native
observations, projects their exact state, and loads one disposable PostgreSQL
read model.

Candidate loads use bounded 1,000-row PostgreSQL recordsets inside one
transaction. The projector checks chunk counts, table roots, release
membership, and Repository bindings before moving `current_release`. It does not
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
| `vela-science/vela-web` | problems.science and its read-only projection |
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
reachable through `pre-decomposition/2026-07-28`. The public Erdős repository
contains byte-exact mirrors for every historical source object that cited the
private repository.

## Release boundaries

Changed Vela product artifacts release from one Vela source tag and manifest.
Public components retain their own versions, artifacts, checksums, provenance,
and supported interface versions; unchanged components do not churn merely to
match the source tag.

- Vela releases the protocol implementation and CLI.
- Independent Python and JavaScript readers check the canonical-byte boundary
  without creating another package or release surface. Two clean-room emitters,
  `conformance/emitters/javascript.mjs` and `conformance/emitters/python.py`,
  build DSSE envelopes from PAE, base64 and Ed25519 and write Submission and
  Verification objects. The readers in `conformance/readers/python` and
  `conformance/readers/javascript` independently check the RFC 8785 vector
  corpus; the Python reader also reconstructs repository roots.
- Immutable Canopus `0.8.0` remains frozen for historical Runs that bind its
  exact bytes. Current Vela contains no executor or separate runner release.
- Each Repository verifies and reproduces its own exact state.
- Vela Web verifies its read projection against exact Repository sources.
- Organization workflows test the compatibility matrix without becoming a
  canonical writer or synthetic ecosystem release.

An exact scientific Run still pins every binary and digest it used. Colocated
source does not grant an executor authority access, and component versions do
not move in lockstep. One Vela source tag and manifest coordinate changed
artifacts without erasing their component versions.

Reusable software and capabilities stay in their native package ecosystems:
Cargo and crates.io, Python and PyPI or uv, JavaScript and npm, Lean and Lake,
Git releases, and OCI or ORAS where those are the natural distribution
surfaces. Vela may retain their exact PURLs, SWHIDs, versions, and digests, but
it does not add another resolver, package namespace, hosted registry, or
package-acquisition command surface.

A repeated source-local contract may move into shared Vela code only after two
maintained consumers need the same semantics, independent readers agree on its
exact bytes, and the extraction deletes more maintained duplication than it
adds. That is a code-reuse gate, not a roadmap toward a Vela package manager or
registry. Shared bytes, generated documentation, and discovery records remain
replaceable artifacts and confer no Standing.

Accepted ADR 0030 defines the current Math Source Registry because exact
observation must bind native source identity, rights, snapshots, adapters,
coverage, and omissions before any Atlas view is trustworthy. That deployed
inventory does not distribute semantic packages or confer Standing.

The first-party Problems projection reads the one live
mathematics authority, `vela-science/math`, and the registered native sources.
It read "the four declared Frontiers" until this line was corrected, which bound
the Atlas to the four repositories ADR 0039 archived. A later federated Atlas
requires independently governed external Repositories and exact cross-Repository
correction evidence. Both are read projections, never canonical databases or
writers.

## Non-goals

The architecture does not add:

- a hosted authority or second writer;
- a canonical database or public mutation API;
- a universal ontology or work graph;
- a mandatory model runner;
- preemptive partitioning, graph-database, vector, or streaming infrastructure;
- a package Registry before reusable packages exist;
- an Atlas service before exact cross-Repository value is measured;
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
4. map correctness: coverage disclosure, stale-state rate, valid
   next-obligation rate, Decision-to-remap latency, and cold-user comprehension.

The current read path is `status`, `show`, and `why`; problems.science
is the public map surface. A new map, diff, or comparison feature is worth
adding only when it answers one named scientific question and a registered
cold-use test shows that it reduces evidence-location, correction, or
continuation time without inventing a canonical object or authority layer.
