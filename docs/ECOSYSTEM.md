# Vela ecosystem boundary

Vela is version control for scientific state. Core is the narrow integrity
layer inside a wider ecosystem of sovereign scientific repositories, native
tools, verifiers, workbenches, and rebuildable readers.

The complete Core loop is:

```text
init -> submit -> verify -> decide -> replay
```

Everything outside that loop either produces bounded input for it or reads its
exact result. Shipping beside Core, being first-party, or appearing in a public
product does not grant an object authority or make it part of the Protocol.

## Ownership

| Surface | Owner | Vela boundary | Authority effect |
| --- | --- | --- | --- |
| Native science | Source repositories, proof assistants, simulators, workflow systems, laboratories | Exact references, retained Artifacts, and source-owned methods | None |
| Activity | Workbenches and agent/session systems | Optional opaque run or session provenance; selected exact outputs may become Artifacts | None |
| Scientific state | Vela Repositories | Submission, Verification Record, attributed Decision, Event, correction, replay, Standing | Only an authorized Decision changes local Standing |
| Discovery | Problems and other root-bound readers | Deterministic projection of exact Repository state and source observations | None |
| Evaluation | Independent studies and institutional processes | Scoped evidence about methods, programs, or outcomes | None unless separately admitted as ordinary scientific evidence |

A Repository exists because there is an independent authority boundary, not
because there is a new topic, corpus, campaign, model, or source. Standing is
always local to one Repository. There is no global Standing.

## Scientific state and activity stay separate

Core owns signed scientific state:

```text
Submission -> Proposal -> Verification Record(s)
           -> authorized attributed Decision -> Event -> replay -> Standing
```

Workbenches own mutable activity: orientation, queries, branches, experiments,
attempts, sessions, checkpoints, traces, retries, budgets, queues, and partial
results. An activity system may produce a bounded Submission, exact Artifacts,
or a scoped Verification Record. It cannot mint a Decision, Event, Repository
identity, authority state, or Standing.

Generic Git and agent-session provenance remains owned by systems built for
that purpose, including Entire. Vela does not maintain a second transcript,
checkpoint, session, or research-memory stack. `provenance.source_run` and
Decision `session_ref` are opaque references to source-owned state; Vela does
not interpret their lifecycle.

The same rule applies to work packets and execution contracts. Source tools may
use them, but Vela defines no Packet, Attempt, ResearchMemory, proof-state,
query-language, or universal operation object. Selected exact manifests,
methods, inputs, and outputs cross the boundary through existing Artifact,
Evidence, Submission, and Verification semantics.

If a consumer later retains external-query evidence, deterministic provider,
query, source, and result identity must remain separate from acquisition
receipts. A wall-clock field such as `retrieved_at` may describe custody or
observation, but must not destabilize a semantic or Core root.

## Provenance kinds are peers

Humans, AI models or agents, organizations, and deterministic tools are peer
provenance kinds. Category supplies no evidentiary rank. Weight follows the
fitness of the named method, exact inputs, independence or shared dependencies,
retained outputs, scope, outcome, and limitations.

A Verification Record reports that provenance; verification is not inherently
independent merely because it is recorded. Repository policy decides which
disclosures are sufficient for a Decision. Protocol 1's current acceptance
policy requires a passing record that declares independence from the producer,
while the object remains able to report shared dependencies exactly.

Verification remains separate from authority. A passing check, model vote,
human review, organization endorsement, Git merge, signature, or publication
does not change Standing. The separately authorized and attributed Repository
Decision is the only admission operation.

## Native integration

Native repositories remain sovereign. The reusable integration sequence is:

```text
Manifest -> Profile -> Binding -> Method
```

Every integration contract declares `authority_effect = "none"`.

- A Manifest declares one source-owned integration instance.
- A Profile defines the exact conformance question.
- A Binding states how one native repository exposes or satisfies that Profile.
- A Method states how one property is checked.

Bindings preserve native identity, exact revision, selectors, content fixity,
rights, availability, and semantic loss. They do not initialize a Vela
Repository or admit source state. Reference, snapshot, Verification, and
admission remain distinct:

```text
reference broadly
snapshot selectively
admit narrowly
```

A shared integration abstraction enters Core only after two maintained
consumers agree on exact behavior and extraction deletes more maintained code
than it adds.

## Orientation and retrieval are disposable

Problems, search, source queries, overlap checks, structural similarity,
rankings, and next-work suggestions are orientation surfaces. They must be
versioned when their interpretation matters, rooted in exact inputs, explicit
about incompleteness, and disposable without changing Repository state.

Structural or semantic similarity is advisory. It never transports identity,
equivalence, Verification, Decision, or Standing. Exact rooted identity remains
authoritative.

The useful consumer loop is:

```text
orient -> check overlap -> work natively -> record bounded output -> remap
```

That loop belongs in a source repository, workbench, or read product and
compiles to existing Vela reads and portable records. TheoremDB and similar
systems may inform product flow; they do not justify a TheoremDB integration,
generic query language, similarity authority, or agent runtime in Core.

## Projection boundary

`vela projection --json` is the narrow state-to-discovery export. It is
deterministic, versioned, root-bound, and authority-neutral. It composes facts
Core has already verified; it does not choose Git history, join source-specific
records, rank work, or define presentation.

Consumers may add source observations and product-specific views only under
their own exact contracts. A projection row must disclose the Repository and
source roots from which it was derived. A generated database, graph, index,
page, or explanation is replaceable and cannot become a second canonical
writer.

Problems is the first-party public scientific frame. Vela Web owns its
presentation and disposable read model. Core contains no Web compatibility
branch, provider configuration, deployment logic, or browser authority path.

## Implementation boundaries

```text
  kernel  crates/vela-protocol
          ↑ canonical objects, roots, Events, replay, and Standing
  auth    crates/vela-authority
          ↑ restricted Repository authorization and service signing
  durable crates/vela-repository
          ↑ policy-neutral transactions and recovery
  operator crates/vela-cli
           ↑ 16 verbs: replay status projection claims log verification
             correction integration recover authority init review show why submit completions
  readers conformance/readers/python, conformance/readers/javascript,
          conformance/emitters/javascript.mjs, conformance/emitters/python.py
          ↑ independent implementations of the same exact bytes
```

No reverse dependency is authorized. Canonical replay does not depend on a
reader, projection, native source, Web, model, or hosted service.

## Current repository topology

The controlled public topology is intentionally small:

| Repository | Responsibility |
| --- | --- |
| `vela-science/vela` | Protocol, CLI, schemas, conformance, exact readers, and releases |
| `vela-science/math` | One current mathematics authority under its own Repository policy |
| `vela-science/vela-web` | Problems and other read-only projections |
| `vela-science/.github` | Organization profile, reusable workflows, and security policy |

Archived predecessor repositories and retired packages remain in Git history
and release tags. Their existence does not authorize a compatibility reader in
the current binary. A historical format is read with the pinned release of its
era unless a current replay or security requirement explicitly proves a narrow
reader must remain.

## Evidence classifications

These states must not be collapsed:

| Classification | Meaning |
| --- | --- |
| Local implementation | Code and focused checks exist in the owning repository |
| Controlled interoperability | First-party or same-operator components agree on exact bytes or roots |
| External activity | A separately operated producer, verifier, or reader returned scoped evidence |
| Independent validation | Control, method, and relevant dependencies are materially independent |
| Plural authority | A separately governed Repository with independent key custody and a real capacity to disagree made its own Decision |
| External adoption | A separately maintained consumer relies on the contract for its own work |

Core completion can establish the first two. It cannot fabricate the latter
four with extra agents, repositories, fixtures, or prose. External waits are
recorded by their owning program and do not stay open as internal software
tasks.

## Non-goals

Core does not add:

- a research runner, scheduler, queue, task catalogue, or credential service;
- an agent transcript, checkpoint, memory, or proof-state store;
- Attempt, Packet, Target, Campaign, Dossier, or research-object Protocol types;
- a universal scientific operation or ontology;
- a TheoremDB-specific integration or generic similarity authority;
- a hosted package registry, source warehouse, or package resolver;
- a canonical database, graph store, or public mutation API;
- reviewer counts, panels, enrollment gates, or a second approval system; or
- Web presentation, deployment, provider configuration, or source campaigns.

The deletion rule is the ordinary architecture rule: keep current exact
semantics, remove superseded implementations and unconsumed abstractions, and
preserve old behavior only when current replay, accepted data, or a concrete
security boundary requires it.
