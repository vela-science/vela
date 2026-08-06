# Interoperability

Vela standardizes the narrow transition boundary between scientific
workbenches, verifiers, canonical Frontiers, and readers. It does not replace
domain tools, Git, package formats, workflow engines, or scientific ontologies.

## Public write contracts

### Submission

`vela.submission.v1` is the producer boundary. It carries exact:

- producer identity and authentication;
- the requested change, and for anything but `add_claim` the exact target
  Claim id and full Claim root;
- Claim assertion, type, conditions, and caveats;
- Artifact identities;
- replayability, producer-reported checks, and method facts;
- verification requirements;
- source workbench/version metadata; and
- an optional rooted execution binding.

It carries no Frontier and no Target field. The receiving repository makes the
Frontier association at submit time; a Submission is portable bytes, and the
schema is closed, so an adapter that writes `frontier` or `target` keys is
rejected outright. PROTOCOL.md section 3.3 lists the complete field set.

A workbench can emit a Submission without importing Vela's Event, authority,
or repository implementation.

### Verification Record

`vela.verification-record.v1` is the verifier boundary. It carries exact:

- Claim, Submission, Proposal, and Artifact subjects;
- verifier identity and independence disclosure;
- method, implementation, environment, and execution roots;
- scoped property and nonclaims; and
- outcome and signature.

A verifier never emits a Vela Decision or Event.

## Public read contracts

The current CLI exposes closed JSON for:

- status and repository roots;
- ranked Target Offers;
- write-free Target briefings;
- typed object inspection;
- Claim Standing explanations;
- Proposal lists and Review Packets;
- strict checks and reproduction; and
- authority and repository verification.

Readers must preserve object IDs, full roots, source commit/tree, repository
origin, source schema, and authority effect.

## Adapter rule

An adapter:

1. names its source system and exact version;
2. binds source object IDs and roots;
3. declares transformations and semantic losses;
4. emits only a Submission or Verification Record;
5. retains explicit caveats and nonclaims; and
6. never infers acceptance from tool success.

Any external runner or workbench may be a producer. It consumes native Target
packets and emits ordinary Submissions or Verification Records. Vela does not
own or wrap the runner. Frozen Canopus `0.8.0` remains historical replay
evidence, not a current product or interoperability requirement.

## Semantic packages

Domain terminology and constraints remain versioned, content-addressed
Frontier-local packages. A package may contain:

- terms and identifiers;
- constraints and validation shapes;
- mappings and consequence tiers;
- fixtures and conformance examples;
- licenses and provenance; and
- generated interoperability artifacts.

Shared labels, embeddings, proximity, or ontology predicates do not transport
Standing.

## Cross-domain bridges

A bridge binds:

- exact source and target package roots;
- versioned mappings;
- assumptions and information loss;
- consequence tier;
- source and target authority;
- validation evidence; and
- correction behavior.

Consequence tiers are explicit:

```text
discovery
organization
identity
logical_transport
empirical_transport
```

The default is `discovery`. Transport of scientific Standing requires a
separate exact governed transition in the target Frontier.

## Distribution

Git bundles, ordinary clones, OCI artifacts, archives, and content-addressed
stores may carry Vela bytes. Transport integrity does not create Vela
authority. The consumer verifies the exact Git objects, repository origin,
sequence-one authority trust root, canonical objects, and replay.

Derived databases, Web pages, graphs, and search indexes are rebuildable and
must name their source roots.

The retained Erdős 424 RO-Crate 1.3 experiment is a base metadata view over a
Decision-chain transfer package. It is not a supported Submission profile or
complete workflow crate. The attached crate now carries the exact source-diff,
predecessor and successor sources, and full-index patch needed to reproduce the
bounded correction, while preserving Vela objects and authority as native
payloads. Its closed `SHA256SUMS` file is the fixity boundary. Standard archive,
repository, OCI, or deposit tooling may transport that file set; Vela does not
maintain a second archive format or extraction runtime. Promotion still
requires a real independent consumer. Until then, no general interchange SDK,
profile registry, or import command is warranted.

## Versioning

Current schemas are closed and versioned. Unknown major schemas fail closed.
Predecessor tags and source archives preserve earlier contracts; the current
binary does not retain their writer interfaces.
