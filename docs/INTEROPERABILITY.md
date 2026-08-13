# Interoperability

Vela standardizes the narrow transition boundary between scientific
workbenches, verifiers, canonical Repositories, and readers. It does not replace
domain tools, Git, package formats, workflow engines, or scientific ontologies.

## Public write contracts

### Submission

`vela.submission.v2`, inside a DSSE envelope of payload type
`application/vnd.vela.submission.v2+json`, is the producer boundary. The
envelope's signature is the Submission's only signature and the envelope's
canonical root is its only root. The payload carries exact:

- producer identity and the public key the envelope signature must verify
  under;
- the requested change, and for anything but `add_claim` the exact target
  Claim id and full Claim root;
- Claim assertion, type, conditions, and caveats;
- Artifact identities;
- replayability, producer-reported checks, and method facts;
- verification requirements;
- source workbench/version metadata; and
- an optional rooted execution binding.

It carries no Repository and no Target field. The receiving repository makes
that association at submit time; a Submission is portable bytes, and the
schema is closed, so an adapter that writes `frontier` or `target` keys is
rejected outright. PROTOCOL.md section 3.3 lists the complete field set.

A workbench can emit a Submission without importing Vela's Event, authority,
or repository implementation.

### Verification Record

`vela.verification-record.v2`, inside a DSSE envelope of payload type
`application/vnd.vela.verification-record.v2+json`, is the verifier boundary.
The payload carries exact:

- Claim, Submission, Proposal, and Artifact subjects;
- verifier identity and independence disclosure;
- method, implementation, environment, and execution roots;
- scoped property and nonclaims; and
- outcome.

The signature is the envelope's, not a field of the record.

A verifier never emits a Vela Decision or Event.

## Public read contracts

The current CLI exposes closed JSON for:

- status and repository roots;
- Claim-index pages;
- typed object inspection;
- Claim Standing explanations;
- Decision Inbox projections, Proposal lists, and rendered detail;
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

The [non-normative integration-profile
template](integrations/integration-profile-template.md) records these facts,
reviewer or verifier scope, rights and availability, and explicit semantic loss.
It is package-plane guidance, not another protocol object or authority surface.

Any external runner or workbench may be a producer. It may consume a
source-local work packet and emit ordinary Submissions or Verification
Records. Vela does not own or wrap the runner, publish a work catalogue or
planner, or standardize the packet briefing command. Frozen Canopus `0.8.0` remains
historical replay evidence, not a current product or interoperability
requirement.

## Semantic packages

Domain terminology and constraints remain versioned, content-addressed,
Repository-local profiles or Artifacts. Such an Artifact may contain:

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
separate exact governed transition in the target Repository.

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

Every Vela document is versioned in its own `schema` tag, and an unknown major
schema fails closed. What a version bump is *for* depends on which of two kinds
of document it is, and the two rules are opposite. Applying one rule to both is
the mistake this section exists to prevent; it was made, in the permissive
direction on signed objects it would have been fatal and in the strict direction
on read surfaces it cost three production breaks in six days.

### Rule A — signed and rooted objects

**There is no compatible change.** The set is empty, and this is not a policy
choice.

`rooted()` hashes canonical JSON *including keys*. A field added to a Submission,
a Verification Record, a Proposal, an authority record or a Claim Record
produces different canonical bytes, therefore a different root, therefore a
different object. The identifier is the bytes. Any change to the shape of one of
these is a new schema version, and the two versions name two different objects
that happen to describe the same thing.

`#[serde(deny_unknown_fields)]` on these types is the enforcement, and it stays.
A reader that accepts an unrecognized field on a signed payload is not being
lenient; it is computing a root over bytes it did not fully account for, and it
is non-conformant. Predecessor tags and source archives preserve earlier
contracts; the current binary does not retain their writer interfaces.

This is where the comparison with a server-side HTTP API stops being useful.
Such an API can assign identity independently of the bytes — a record keeps its
identifier across any change to its serialization — and can therefore afford a
long list of changes that need no version bump, because it also owns a
transform that walks a response back to whatever version the caller pinned. Vela
has no such transform and no one to run it: the producer is a binary the reader
downloads and pins by hash. The classification is worth borrowing. The mechanism
is not.

### Rule B — derived read surfaces

Read surfaces are the documents Vela emits *about* state rather than as state:
`vela.status.v4` and the other `--json` command results. Nothing in them is
hashed, nothing is signed, and everything in them is derived from objects that
already exist and are already rooted.

A change is **compatible**, and needs no version bump, when it only adds:

- a new field on an existing object;
- a new object under a new key;
- a new member of a tagged union, where the tag is the discriminant;
- a new key in an open map, such as `integrity.blockers_by_code`.

A change is **breaking**, and requires a new `schema` tag, when it removes a
field, renames one, changes a field's type, changes the meaning of a value under
an unchanged type, or removes a member of a closed vocabulary.

Every field a version names is present on every branch. A field with nothing to
report is present and null, never absent — `git.commit` is null on a repository
whose authority has not finished initializing, and the key is still there. So
the compatibility rule and the completeness rule are separable, and each is
checked by the thing that can actually check it: a dropped or renamed field
fails the schema's `required` list, and an added one is nobody's business.

**Consumers of a read surface must ignore fields they do not recognize.** This
is an obligation, not a courtesy. A reader that refuses a document because it
carries a field the reader has not been taught breaks on every compatible
change, which is what happened: `counts.withdrawn_review` arriving, `git.role`
arriving, and `actions.work.mode` becoming a two-member union each took down a
downstream projection refresh, and each was additive. Read what you know, hold
it to the type this version declares, and read past the rest.

### Retrying a write

There is no idempotency key, and a reader arriving from an HTTP API will look
for one. Vela does not need it, for the same reason Rule A is what it is: an
object's identity is its content root, so retrying `vela submit` with the same
inputs produces the same Submission rather than a second one.

Decisions are stronger than idempotent. `vela review accept` requires
`--if-entry-root`, and the applied Decision must carry
`before_hash == proposal.subject.root`. That is a compare-and-swap: an
idempotency key deduplicates an identical retry, and a compare-and-swap also
refuses a stale non-identical one, which is the case that matters when the
subject moved between reading the inbox and deciding.
