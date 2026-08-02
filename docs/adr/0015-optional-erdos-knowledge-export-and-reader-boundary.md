# ADR 0015: Optional Erdős knowledge export and reader boundary

- Status: Rejected — no change (2026-07-22)
- Protocol target: none
- Implementation target: none
- Evidence gate outcome: the exact Erdős baseline found two smaller reader
  defects and no demonstrated need for the proposed export

## Context

Vela already owns the difficult and irreplaceable part of the system: exact
scientific objects, artifacts, verifier evidence, proposals, signed decisions,
corrections, replay, and strict signals in canonical Git histories. Canopus can
optionally bound producer work. Vela Web can project those histories for
inspection. The missing evidence is much smaller than a new universal knowledge
architecture: can an ordinary standards-based reader make one rooted Erdős
slice easier to inspect and reuse without losing Vela's distinctions?

The current Erdős frontier is a useful test because statement revisions,
conditional proofs, finite witnesses, verifier results, proposals, decisions,
and corrections are related but are not interchangeable. Existing graph and
Web projections can show those records, but their locally evolved JSON is not
yet a demonstrated interchange contract.

The earlier draft of this ADR proposed a package manager, ontology registry,
new object and control-document roots, publisher policy, attestation chain,
activation log, normalized semantic database, mapping governance system, and a
cross-domain roadmap. None of those mechanisms is required to answer the
Erdős question. Building them first would create a second product and a second
trust system before proving a user need.

The product story is deliberately linear:

```text
produce -> preserve -> check -> decide -> reuse
```

| Step | Owner | Meaning |
| --- | --- | --- |
| Produce | Any suitable research tool; optionally Canopus | Create bounded work and evidence. |
| Preserve | A canonical frontier Git repository | Retain exact objects and history. |
| Check | Vela | Replay bytes, verifiers, roots, and strict signals. |
| Decide | Existing Vela policy or protected human decision | Change scientific standing through the one authority boundary. |
| Reuse | Optional readers, including the Observatory | Inspect, cite, compare, and continue from the rooted state. |

Anything called **Vela Knowledge** belongs only to the final step. It is a
working name for optional reader infrastructure, not a public product, source
of scientific state, ontology authority, package registry, database, or hosted
service. Removing it must leave every frontier replayable and every Vela
decision unchanged.

## Outcome

The proposal was rejected before implementation. The frozen Erdős baseline at
commit `6692f284c1a34e0faf07de8bfa0dfd01db10c118` showed that the existing
root-bound Observatory already exposes the exact work target, packet, Receipt,
proposal, verifier distinction, terminal decision, standing, strict debt, and
typed graph relations needed for the representative current-work, rejection,
and conditional-result cases.

The audit found two real but narrower reader defects:

1. recorded `statement_edit` activity does not expose the exact source-Git
   revision chain and prior statement bytes; and
2. finding pages do not group typed dependencies and contradictions already
   retained in graph rows.

Neither defect requires RO-Crate, JSON-LD, PROV-O, SKOS, another package,
another database, or a Vela protocol change. If user testing prioritizes them,
they belong as ordinary read-projection and record-page improvements inside
the existing Vela Web owner. The experiment therefore stops with no adapter,
artifact, schema, package, service, repository, or release.

The exact comparison evidence is recorded in the parent integration report
`docs/reports/ERDOS_READER_BASELINE_2026-07-22.md`. Rejecting this ADR changes
no canonical frontier byte, Vela replay result, decision, or public contract.

## Proposed decision (not implemented)

Run one evidence-gated experiment: export a frozen Erdős slice as a small
RO-Crate 1.3 JSON-LD research object plus an explicit loss report. Implement
the first adapter beside the existing read projection so that it can be deleted
without affecting Vela. Extract a separately released `vela-knowledge` library
or repository only if two maintained consumers need the same adapter boundary
and extraction removes duplicated code.

This ADR adds no Vela event, Receipt field, reducer rule, signature, policy,
CLI command, accepted-state rule, authority service, or protocol version.

### 1. The candidate artifact

The experiment emits exactly three files:

```text
ro-crate-metadata.json
vela-loss-report.json
SHA256SUMS
```

`ro-crate-metadata.json` is an RO-Crate 1.3 JSON-LD document. It describes the
selected source objects and their provenance; it does not copy or replace the
canonical frontier. `vela-loss-report.json` lists every selected source kind,
field, and relation that the adapter omitted, approximated, or could not map.
`SHA256SUMS` contains full lowercase SHA-256 digests for the other two exact
files. The consuming fixture or release record pins the SHA-256 of
`SHA256SUMS` outside the crate; the file is not self-authenticating. These are
artifact hashes, not new Vela identities.

The fixture lock used to build the artifact binds:

- the frontier repository URL, exact Git commit, and tree;
- the released Vela version and binary SHA-256;
- event-log, snapshot, proposal, actor-registry, and selected artifact roots;
- the exact selected problem, finding, artifact, proposal, decision, and event
  identifiers;
- the expected replay result and strict-signal totals by code;
- the adapter source commit and dependency lock; and
- exact local copies and hashes of every JSON-LD context used during an offline
  build.

The fixture lock is test configuration, not a new protocol schema. Existing
full Vela identifiers and roots remain the only scientific identities. Short
IDs, database keys, labels, timestamps, URLs, graph coordinates, and search
rank never substitute for them.

### 2. Reuse standards before adding vocabulary

The adapter uses existing standards only for the jobs they already perform:

| Need | Representation | Boundary |
| --- | --- | --- |
| Package a research object and its metadata | RO-Crate 1.3 | The crate is a portable reader artifact, not canonical state. |
| Serialize linked records | JSON-LD 1.1 | Contexts are copied and hash-pinned; builds never resolve the network. |
| Describe entities, activity, attribution, derivation, revision, and invalidation | PROV-O | `prov:Agent` and attribution never imply Vela actor registration or decision authority. |
| Describe topic and catalog navigation | SKOS | Concepts and broader/narrower links organize discovery only; they are not claims about truth. |
| Describe generic files and works | RO-Crate/Schema.org terms | Exact Vela IDs and roots remain visible as identifiers and properties. |

No OWL inference, remote context loading, `owl:sameAs`, or automatic
`skos:exactMatch` is applied. A source co-reference or topic mapping may be
displayed only as an attributed source assertion. It cannot merge records,
transport standing, discharge an obligation, or create a Vela relation.

The candidate profile may define only the few Vela-specific property names
needed to retain exact source references and keep these observed planes
separate:

```text
source object identity and content root
source frontier and Git identity
catalog or source assertion
verifier result
statement or formalization fidelity
proposal and decision standing
reproduction observation
correction, withdrawal, and supersession observation
frontier replay and strict-integrity observation
```

Those properties describe what the exact source already says. They contain no
rules and support no inference. If a standard term is lossless, the adapter
uses it. If it is not, the source fact stays explicit and the loss report names
the gap rather than inventing a general ontology.

### 3. Deterministic, offline construction

The adapter is a pure reader over one exact checkout:

1. Refuse a dirty checkout, wrong commit/tree, missing selected object, or root
   mismatch.
2. Run the released Vela replay/check surfaces and compare the result with the
   fixture lock.
3. Preserve known strict blockers as visible source observations. A
   strict-blocked frontier may be exported when its exact expected blocker set
   matches; the adapter must not relabel it as clean.
4. Read only selected retained bytes. Do not execute embedded commands,
   Markdown, HTML, proof text, RDF, or artifact payloads.
5. Map source facts through one versioned table, produce the loss report, and
   serialize UTF-8 JSON with stable key and record ordering and one final LF.
6. Build with network disabled from hash-pinned contexts and dependencies.
7. Rebuild in a second clean temporary path and require byte-identical output.

Event order, explicit predecessor/correction references, and proven Git
ancestry establish sequence. `created_at`, file mtime, database insertion time,
and backdated source timestamps do not.

### 4. Reader behavior

Adapter validation is independent of `vela check --strict`:

- **Strict adapter validation** fails on input-root drift, an unexpected replay
  or strict result, missing source records, unknown required source kinds,
  unreported mapping loss, context/dependency drift, dangling references,
  state-plane conflation, nondeterministic bytes, or hash mismatch. It emits no
  artifact advertised as valid.
- **Diagnostic mode** may emit machine-readable errors and a draft loss report.
  It may not emit a valid crate, apply a mapping, infer authority, or hide the
  failed record.
- **Readers** must show unavailable or unsupported data explicitly. They may
  not silently fall back to a newer source root or a mutable database row.

A semantic export failure never changes or invalidates canonical Vela history.
It means only that this optional reader artifact could not be rebuilt.

### 5. Migration and compatibility

There is no frontier migration. Canonical events, proposals, Receipts,
registrations, policies, artifacts, verifier evidence, and Git history remain
byte-identical. Existing Vela binaries replay them unchanged.

The first adapter is pre-1.0 experiment code in the current read owner. It may
replace existing ad hoc reader transforms after parity is proven. It must not
create a second active projector: the Web-owned normalized Neon model remains
a disposable cache populated from canonical Git and may store the export for
queries, but it is not an input to the export and never becomes authority.

If the experiment is removed, old artifacts remain ordinary content-addressed
evidence at their recorded commit. No compatibility alias, remote resolver, or
request-time migration is required.

## Adversarial and failure cases

The fixture must include or synthesize focused cases for:

- a verifier pass presented as scientific acceptance;
- a `prov:Agent` attribution presented as a registered Vela reviewer;
- a SKOS or source co-reference used to merge distinct statement revisions;
- a conditional proof displayed as unconditional resolution;
- a correction, rejection, withdrawal, or retraction hidden by a stale reader;
- a backdated post-correction record ordered by timestamp;
- a remote JSON-LD context changing behind the same URL;
- an omitted source field or relation absent from the loss report;
- a partial database refresh or old database row returned for a new source
  root;
- malicious HTML, Markdown, RDF, command text, or path traversal in retained
  data;
- short-digest collision or database-ID substitution; and
- a crate, hash, citation, or popularity signal presented as a truth verdict.

Every unknown authority implication fails closed. Restricted or embargoed
payloads are out of scope; the candidate contains only source material already
approved for the public Erdős frontier.

## Exact conformance contract

Before this ADR can move from Proposed, freeze a three-to-five-problem Erdős
fixture that includes:

- `erdos:1056` and its exact current work/proposal evidence;
- at least two distinct revisions of one statement;
- one kernel-clean or mechanically verified result whose scientific scope is
  conditional;
- one terminal rejection, withdrawal, correction, or supersession; and
- one relation with more than one premise or source, so a binary display edge
  cannot erase joint context.

The fixture passes only when all of these tests pass:

1. Every exported object resolves to its full source ID, source bytes, Git
   identity, and applicable Vela roots.
2. Two clean offline builds produce byte-identical metadata, loss report, and
   SHA-256 entries.
3. RO-Crate 1.3 validation and an independent JSON-LD 1.1 parser accept the
   artifact without network access.
4. Catalog assertion, verification, fidelity, proposal standing, decision
   standing, reproduction, correction, and strict integrity remain separately
   queryable.
5. The conditional result cannot satisfy the unconditional resolution view.
6. The correction case changes only the derived reader observation; historical
   records remain addressable, and Vela standing changes only when canonical
   Vela history already records it.
7. PROV attribution and SKOS organization change no registration, authority,
   accepted state, obligation, or source identity.
8. Every selected source field and relation is either mapped exactly or named
   in the loss report; an injected unknown required field makes validation
   fail.
9. Backdating, remote-context substitution, missing source bytes, wrong roots,
   and stale database rows fail before a valid artifact is emitted.
10. One Observatory object page and one off-the-shelf RO-Crate inspection path
    read the same artifact and show the same exact identities and state planes.
11. Removing the adapter output, Web projection, and Neon database leaves the
    source frontier replay and strict classifications unchanged.
12. A timed, observed user task—identify the current statement, evidence,
    standing, correction effect, and next reproducible action—has fewer errors
    or materially less integration work than the existing ad hoc projection.

The initial checks are limited to the adapter and its consumers. Existing Vela
protocol fixtures run unchanged; this ADR alone does not justify a Vela
release, a broad conformance suite, external Lean, or live-network testing.

## Acceptance and extraction gates

ADR 0015 remains Proposed until the exact fixture passes all twelve tests and
the result improves the measured reader task. If it does not, keep the existing
rooted projection and reject this ADR without creating a new repository.

Even after acceptance, extraction into a public `vela-knowledge` repository or
package requires:

- two maintained consumers using the same rooted adapter contract;
- a code move that deletes duplication rather than adds another copy;
- one clear maintainer and replacement/retirement path; and
- proof that removing the package changes no Vela replay or decision.

A second scientific domain, general mapping governance, protocol-bound
semantic references, a package registry, publisher trust policy, broad Atlas,
or dedicated database each requires separate evidence and, where applicable,
a separate ADR. None is implied here.

## Alternatives

### Keep only the current ad hoc JSON projection

This is the fallback if the fixture finds no interoperability or usability
gain. It is preferable to an unused semantic layer.

### Build a universal Vela ontology and package system now

Rejected. It adds governance and compatibility machinery before a single
reader task proves the need.

### Put semantic identities or mappings in the Vela protocol

Rejected. The experiment reads existing state and demonstrates no missing
authority invariant.

### Make RDF, a graph database, or Neon canonical

Rejected. They are useful reader representations and caches, not Vela's
replayable accepted-state store.

### Launch Vela Knowledge as another product

Rejected. Users should understand one connected system: tools produce,
frontiers preserve, Vela checks and decides, and optional readers enable reuse.

## Consequences

The proposal is now cheap to falsify. It can show whether standards-based
reuse helps without committing the ecosystem to a new platform. It preserves
the one Vela authority boundary and makes all semantic loss visible.

The limitation is intentional: one Erdős export does not establish a universal
scientific grammar, cross-domain transfer, or a durable standalone package.
Those claims require evidence that does not yet exist.

## Primary references

- [PROV-O: The PROV Ontology](https://www.w3.org/TR/prov-o/)
- [JSON-LD 1.1](https://www.w3.org/TR/json-ld11/)
- [SKOS Reference](https://www.w3.org/TR/skos-reference/)
- [RO-Crate Metadata Specification 1.3](https://www.researchobject.org/ro-crate/specification/1.3/)
- [Verified scientific-infrastructure landscape and reuse matrix](../../../../docs/reports/VELA_SCIENTIFIC_INFRASTRUCTURE_LANDSCAPE_2026-07-22.md)
