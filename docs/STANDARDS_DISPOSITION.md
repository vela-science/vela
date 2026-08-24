# Standards disposition

Status: informative. This document explains the current Protocol 1 boundary. It
does not rename a protocol object, define an alias, change a schema, or alter
wire, root, authority, or replay semantics.

Vela's current ownership is one narrow transition not delegated to these
general standards: a named Repository admits or rejects an exact proposed
scientific-state change through an attributed Decision, then deterministically
replays the resulting Standing and correction history. Existing standards and
native systems continue to own generic identity, provenance, packaging,
attestations, workflows, instruments, and domain meaning.

The default composition rule is therefore:

```text
reference native identity -> retain only decision-relevant exact bytes
-> assess within explicit scope -> decide under Repository authority -> replay
```

## Five conceptual views, no new object names

Phase III uses five explanatory terms. They are views over the current object
model, not synonyms that may appear in place of canonical names and not names
for new wire objects.

| Conceptual view | Current-object reading | What it does not mean |
| --- | --- | --- |
| **Proposed Delta** | One authenticated Submission, its `requested_change`, retained Artifacts, and the repository-minted pending Proposal over an exact Claim | Not a `ProposedDelta` object, wire alias, patch format, workflow task, or accepted change |
| **Scoped Assessment** | One Verification Record: exact subjects, Method, scope, outcome, independence disclosure, and `does_not_establish` nonclaims | Not an `Assessment` object, general peer review, recommendation, Decision, or truth certificate |
| **Authority Transition** | An authorized accept or reject Decision, admitted as canonical Event and authority-record changes with exact before/after roots | Not a portable approval, global consensus, signature result, or authority granted to a producer or verifier |
| **Replayable Standing** | Standing deterministically derived by strict replay of valid admitted Events under one Repository's trust root | Not a stored status field, Web badge, Git branch, database row, or Standing transported from another Repository |
| **Correction** | A new Submission and Proposal targeting an exact predecessor; acceptance of `corrects` or `supersedes` retires that predecessor while preserving both histories | Not silent mutation, a Crossmark replacement, generic PROV revision, or a claim that the successor is true |

Canonical interfaces continue to say Submission, Verification Record, Proposal,
Decision, Event, and Standing. Product prose may use the conceptual views only
when it also preserves those object and authority distinctions.

## Standards disposition

The versions below are the current official primary specifications reviewed on
2026-08-24. A standard's presence in this table does not make it a Protocol 1
dependency. Vela references or embeds standard-conforming material only when a
current producer, verifier, release, or replay requirement needs it.

| Existing owner | Official specification reviewed | Native responsibility | Vela disposition |
| --- | --- | --- | --- |
| W3C PROV | [PROV-DM](https://www.w3.org/TR/2013/REC-prov-dm-20130430/) and [PROV-O](https://www.w3.org/TR/2013/REC-prov-o-20130430/), W3C Recommendations, 2013-04-30 | Domain-agnostic entities, activities, agents, derivation, attribution, association, delegation, and provenance interchange | Reference or retain a PROV document as an Artifact. Do not grow a Vela activity graph or general provenance ontology. Vela relations and authority remain narrower protocol facts. |
| RO-Crate community | [RO-Crate Metadata Specification 1.3](https://www.researchobject.org/ro-crate/specification/1.3/index.html), Recommendation, 2026-06-22 | Research-object packaging, JSON-LD metadata, data/contextual entities, identifiers, licenses, and profiles | Use an RO-Crate as an external package or metadata view. Preserve Vela objects as native payloads and exact roots; do not make RO-Crate a Submission alias or create a second Vela archive format. |
| Workflow Run RO-Crate working group | [Workflow Run Crate 0.5](https://w3id.org/ro/wfrun/workflow/0.5), 2024 | Computational-workflow execution, inputs, outputs, parameters, engine, environment, and optional step provenance | Keep run state native. Reference the crate or retain it as an Artifact and carry at most one opaque `provenance.source_run`; do not add Run, Step, Parameter, or Workflow objects to Core. |
| Secure Systems Lab | [DSSE Protocol 1.0.2](https://github.com/secure-systems-lab/dsse/blob/v1.0.2/protocol.md), 2024-05-10, and [Envelope 1.0.2](https://github.com/secure-systems-lab/dsse/blob/v1.0.2/envelope.md) | Payload-type binding, pre-authentication encoding, signature envelope, and signature hints | Continue the selected signed transport. Vela owns payload schemas, accepted algorithms and keys, trust, and payload meaning; DSSE does not. |
| in-toto project | [in-toto Specification 1.0.0](https://github.com/in-toto/specification/blob/v1.0/in-toto-spec.md), 2023-06-02; [Attestation Framework 1.2](https://github.com/in-toto/attestation/tree/v1.2.0/spec) | Generic software-supply-chain layouts, steps, materials/products, link metadata, and artifact attestations | Use native in-toto attestations for builds and release supply chains. Reference their subjects/digests when decision-relevant; do not reproduce a generic layout, link, predicate, or attestation framework in Vela scientific state. |
| DataCite | [Metadata Schema 4.7](https://schema.datacite.org/meta/kernel-4.7/), released 2026-03-03 | DOI-oriented citation and discovery metadata for research outputs, including creators, titles, publishers, dates, resource types, rights, descriptions, and related identifiers | Store or reference the registered DOI and exact source metadata. Do not turn Claim provenance into a competing bibliographic registry or copy mutable deposit metadata as a second source of truth. |
| Crossref | [Crossmark participation and update model](https://www.crossref.org/documentation/crossmark/participating-in-crossmark/), updated 2026-08-12; [Crossref deposit schema 5.4.0](https://www.crossref.org/documentation/schema-library/metadata-deposit-schema-5-4-0/), updated 2025-03-17 | Publisher update policies, DOI-linked corrections, retractions, withdrawals, versions, and reader-facing current-status notices | Reference Crossmark/DOI status where relevant. Do not infer Vela Standing from it or duplicate the global publisher update service. Crossmark publishes service/schema versions, not a separately versioned scientific-authority model. |
| SWHID community and Software Heritage | [SWHID Specification 1.2](https://www.swhid.org/swhid-specification/v1.2/), 2025-04-23 | Persistent intrinsic identifiers and qualified references for archived software objects | Prefer an exact SWHID when it is the native archival identity. Do not mint a Vela software identifier or claim that a Vela SHA-256 root is an SWHID. |
| Git project | [`git hash-object`](https://git-scm.com/docs/git-hash-object) and [Git objects](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects) | Native blob, tree, commit, and tag identity and complete repository history | Preserve full repository-native object names and distinguish commits from trees. Vela SHA-256 canonical roots bind Vela bytes; they neither replace nor reinterpret Git object IDs. |

Three version seams stay explicit. Workflow Run Crate 0.5 says it uses
RO-Crate 1.1 terminology, while the current base specification is RO-Crate 1.3;
an adapter must pin and test the exact contexts rather than claim combined 1.3
conformance by association. The in-toto 1.0.0 core and Attestation Framework
1.2 are separately maintained specifications. Crossmark has current service
documentation and a versioned deposit schema, but no standalone versioned
scientific-authority specification. None of these seams needs a Vela mapping
object.

## Field ownership and copy rules

This matrix covers the generic-looking data carried by current Claims,
Submissions, Verification Records, Proposals, Review Methods, Repository
identity, authority records, replay, and releases. Several fields share a row
only when they have the same external owner and the same reference/copy rule.

`Reference` means retain the exact native identifier plus enough fixity and
selector information to resolve what was assessed. `Copy` means the value is
part of the Vela transition or replay proof and therefore must remain in the
canonical Vela object even if an external record also describes it.

| Current Vela datum | Native owner or standard | Exact Vela need | Reference versus copy | Vela must not duplicate |
| --- | --- | --- | --- | --- |
| `schema`, payload type, profile and version tags | Defining format; DSSE owns `payloadType` framing | Parse exact bytes under a closed current contract and separate incompatible shapes | **Copy** Vela schema tags; **copy** the selected DSSE payload type | A universal schema registry, content negotiation service, or aliases for retired tags |
| Repository `repository_id`, `name`, `summary`, bounded `scope` | The source-owning Vela Repository | Identify one lineage and state its local scientific bounds | **Copy** because these define the Vela authority boundary | DOI, project, organization, topic, or global scientific-space identity |
| Repository maintainers and content/code/data license expressions | Native maintainers and SPDX license expressions | Disclose local stewardship and the use conditions for retained material | **Reference** native people/organization IDs where available; **copy** the current names and SPDX expressions in the Profile | A contributor directory, identity proof, rights clearance, or new license vocabulary |
| Origin `generation`, Profile root, empty initial-object-set root, and reason | The source-owning Vela Repository | Prove the one current genesis and bind its retained Profile and empty starting set | **Copy** because these define the lineage start | Predecessor archives, migration equivalence, or a general lineage/archival system |
| Repository manifest object sets and their ID/root/path references | Vela replay over the ordinary Git tree | Prove exact parity between indexed current objects and retained canonical bytes | **Copy** the closed active sets and exact references | A package inventory, search index, workflow catalogue, or scientific database |
| `claim_id`, Claim `revision`, object handles, Vela roots | Vela canonical-object rules | Bind exact Claims and route exact retained objects; full roots remain security identity | **Copy** and recompute; never substitute a native identifier | DOI, SWHID, Git ID, or domain accession namespace |
| Git commit, tree, blob, tag, remote, and path | Git | Bind the exact repository bytes in which a native fact or Vela record appeared | **Reference** full native object IDs; copy only the exact binding needed for replay or evidence | A Git object model, branch lifecycle, remote registry, or shortened security identity |
| Software release/source identity, SWHID, PURL, package version | Native package registry; SWHID for archived software | Identify the exact interpreting software or dependency | **Reference** the native ID and immutable version/digest; snapshot only when loss risk justifies it | A package namespace, resolver, acquisition command, or software archive |
| DOI and other scholarly identifiers | DataCite/Crossref or the owning registry | Locate the exact cited research output and its registered metadata | **Reference** the identifier; retain a fixed metadata snapshot only when the Decision evidence requires it | A DOI registry, citation graph, or mutable bibliographic mirror |
| Claim `assertion.text`, `assertion.kind`, Submission Claim `assertion`, `type`, `conditions` | Source domain and source-owning Repository | State the bounded proposition the Repository is being asked to govern | **Copy** the exact bounded assertion and conditions; reference the full native statement beside it | A domain ontology, theorem language, instrument model, or claim of semantic equivalence |
| `caveats`, `does_not_establish`, limitations and nonclaims | Producer, verifier, Method, or Repository as attributed | Keep the boundary of each assertion and check attached to the exact object | **Copy** when it constrains interpretation of the Vela object | General ethics, reporting, or discipline-wide policy vocabularies |
| Claim `provenance.kind/title/locator/authors/year` | PROV plus DataCite/Crossref/native source | Attribute and locate the exact source used for this Claim | Prefer **reference** by persistent native ID; copy only the minimal fixed citation needed to interpret retained bytes | A full PROV graph, contributor registry, or complete DataCite record |
| Submission `provenance.producer/source_system/source_run/emitted_at` | Native producer/workbench; PROV or Workflow Run RO-Crate | Authenticate who emitted the request and bind it to one opaque source execution when available | **Copy** producer and emission fact; **reference** `source_run` and source system version | Sessions, prompts, checkpoints, run events, step graphs, schedulers, or workflow state |
| Signer `actor_id`, `actor_class`, public key and `declared_at` | Native identity provider/key system; DSSE only frames signatures | Verify the exact producer/verifier key and distinguish the declared actor class | **Copy** the self-declared key binding inside the signed payload; reference richer identity records externally | Personhood, ORCID/ROR accounts, PKI, global identity, or proof that two keys are independent people |
| Authority principal ID/class, display name, affiliation, account links | Repository authorization and the principal's native account/identity system | Attribute the authenticated performer of one exact local action | **Copy** authenticated principal and class needed for replay; **reference** account evidence | An identity provider, organization directory, employment record, or global role grant |
| Repository authority key IDs/public keys, validity, keyset and predecessor roots | The named Vela Repository and its standard OpenSSH agent provider | Verify the exact service signature and keyset transition active for each authority record | **Copy** the public verification material and rooted transitions; private-key custody remains native | Private keys, a signer daemon, PKI, global trust, or a second approval system |
| Repository members, roles, actions, authorization request/evaluation | The named Vela Repository | Recompute whether one closed Vela action was authorized under the exact local model | **Copy** the complete model/request/evaluation needed for fail-closed replay | General RBAC/ABAC, workflow permissions, panels, quorums, or cross-Repository authority |
| DSSE `payload`, `payloadType`, `signatures[].sig/keyid` | DSSE 1.0.2; application owns trust and keys | Authenticate exact Submission, Verification, Withdrawal, or authority-record bytes | **Copy** the envelope exactly; treat `keyid` only as an unauthenticated hint | Canonicalization inside DSSE, key management, PKI, payload validity, or Decision semantics |
| Artifact `kind`, `path`, digest/root, optional ID; verification output Artifact IDs | Native file/package/workflow store; RO-Crate for package description | Retain or resolve the exact bytes considered by a Submission or Verification | **Reference** content already durably available; **copy/snapshot** decision-relevant bytes when availability requires it | An archive format, object store, file catalogue, package manifest, or claim that fixity proves meaning |
| Claim `evidence[].relation` | Source domain/profile | State the bounded role an exact Artifact plays for one Claim | **Copy** the local role plus exact Artifact binding | A universal evidence ontology or authority effect |
| Claim descriptive `relations[].kind` | Source domain/profile; PROV may express derivation/revision | Preserve source-local context without changing Standing | Prefer **reference** to a domain profile; copy only the current bounded edge needed by a consumer | A universal knowledge graph or implied consequence/authority semantics |
| `corrects`, `supersedes`, requested-change kind/target, Proposal action/subject | Vela correction and transition algebra | Bind the exact predecessor and candidate transition on which a Repository may decide | **Copy** because replay acts on these exact values | Crossmark status, generic PROV revision, source-control diff semantics, or acceptance inferred from an external update |
| Proposal actor, reason, producer-package ID/root/path, creation time | Vela receiving Repository plus exact Submission | Bind the pending local transition to its authenticated request | **Copy** the exact association and review rationale | A pull request, workflow ticket, mutable queue record, or producer-written status |
| Proposal Withdrawal actor/reason/time and Proposal/Submission roots | Vela producer-owned pending queue | Prove the exact producer closed one still-pending Proposal without changing Standing | **Copy** for exact lifecycle verification | A Decision, Event, correction, source deletion, or generic cancellation workflow |
| `replayability` and producer checks (`method`, `outcome`, `producer_reported`) | Native producer and method/workflow system | Disclose the producer's bounded reproduction expectation and self-checks | **Copy** only the declared summary; **reference** full native logs and methods | A workflow runner, Verification Record, reproducibility certificate, or scientific acceptance |
| `verification_requirements` | Producer-declared Vela Submission contract | Name the scoped properties that must have eligible exact passing records before acceptance | **Copy** the bounded property names because current Decision eligibility reads them | A general review policy language, universal checklist, or automatic recommendation |
| Verification subject IDs/roots and Artifact scope | Vela objects plus native Artifacts | Prove exactly what the verifier observed | **Copy** Vela roots; **reference** native artifact identities with fixity | Broad relevance, unobserved inputs, or equivalence to another version |
| Verification identity and independence disclosure | Verifier plus its native identity/organization evidence | Attribute one scoped observation and expose declared dependence | **Copy** the signed declaration needed for eligibility; reference supporting identity evidence | Proof of organizational independence, reviewer enrollment, panels, or global credentials |
| Review Method profile/property/question, performer identifier/provider/version, procedure, required output | Native method registry, instrument/tool, and domain profile | Make the scoped check interpretable and bind the exact retained Method bytes | **Reference** the native method/instrument/version; copy the bounded Vela eligibility contract and Method root | A method registry, model catalogue, instrument schema, or generic execution specification |
| Verification implementation, environment root, and start/completion times | Native runtime; PROV or Workflow Run RO-Crate | Bind the execution facts material to interpreting one scoped outcome | **Reference** a native run/crate where possible; copy only the implementation label, exact environment root, and observation interval required by the record | Step traces, resource telemetry, environment resolver, or workflow chronology |
| Verification scope property, `does_not_establish`, outcome, output IDs | Verifier and source-owned Method; Vela defines the closed outcome vocabulary | Record one exact scoped observation and its explicit ceiling | **Copy** because Decision eligibility reads these exact fields | Truth, completeness, acceptance, recommendation, or Standing |
| Decision action/reason/performer/time, exact read set and canonical delta | The named Vela Repository | Attribute one local scientific judgment and bind its complete stale-write guard | **Copy** in the admitted authority transition | A portable approval, global authority, consensus system, or claim that authorization supplies judgment |
| Event IDs/kinds, before/after hashes, object delta, event-log roots | Vela replay | Reconstruct the admitted transition and fail on omission, fork, rollback, or mutation | **Copy** as canonical replay evidence | Generic event sourcing, activity logs, workflow events, or a public mutation bus |
| Authority sequence/previous root, operation/transaction IDs, intent/read/write-set roots, execution binary digest and completion time | Vela repository transaction and authority replay | Prove a contiguous authorized write and bind it to the exact implementation and complete canonical delta | **Copy** because strict replay and recovery validate these facts | A generic transaction protocol, distributed workflow engine, execution trace store, or software identity namespace |
| Standing, accepted/pending sets, correction impact and explanations | Deterministic Vela readers over one Repository | Show the current authority-scoped consequence of admitted Events | **Derive**, never copy as authority; projections reference exact Repository roots | A mutable status source, global truth state, cached authority, or Standing transported between Repositories |
| Timestamps not covered above | Native source for activity time; Vela for observation/admission time | Order and attribute only the transition facts for which time is semantically required | **Reference** native activity times; copy Vela emission, observation, Decision, and authority-record times | A universal clock, chronology of the whole research process, or proof of causality |
| Repository and Artifact licenses/rights | Native source, package, or registry; RO-Crate/DataCite can describe them | Prevent an exact reference or snapshot from losing its use conditions | **Reference** canonical license/rights IDs and source terms; copy a fixed disclosure when required for inheritance | License interpretation, rights clearance, or a second mutable rights registry |
| Namespaced Claim `extensions` and package-plane profiles | Source-owning domain/profile | Carry canonical domain detail without granting authority or changing Core meaning | **Reference** the profile root and copy only explicitly namespaced current detail | Undotted global fields, Standing/Decision keys, universal ontology, or Core semantics by extension |
| Release version, binary digest, checksums, SBOM and build provenance | Cargo/native release system; SPDX; in-toto/provider attestations | Identify the exact Vela binary and prove the published asset set used by operators | **Reference** native attestations/SBOMs and **copy** the provider-neutral manifest facts the installer verifies | A package registry, general attestation framework, or equivalence between equal version strings and unequal binaries |

## Deletion and externalization candidates

No deletion is authorized by this document. Each candidate requires a separate
current-consumer audit, migration design for retained bytes, and protocol review.
If a persistent shape changes, Vela must make an explicit versioned cut rather
than read old and new forms indefinitely.

1. **Bibliographic copies.** `ClaimSource` title, authors, and year are generic
   citation metadata. A future Claim schema could prefer an exact DOI, SWHID,
   or other native identifier plus the minimum frozen display facts, if current
   readers do not require the copied fields.
2. **Performer catalog data.** Review Method display name, provider, and version
   can drift independently of the exact Method. A maintained native method or
   instrument profile could own that catalogue while Vela retains its root,
   scoped property, procedure, and explicit nonclaims.
3. **Descriptive graph edges.** Open descriptive Claim relations that have no
   replay consumer may move into source-owned profiles or read projections.
   The closed correction algebra cannot: replay acts on `corrects` and
   `supersedes`.
4. **Workflow detail.** Any pressure to add inputs, steps, parameters, resource
   use, logs, or environment inventories should be discharged into PROV or
   Workflow Run RO-Crate. The current `source_run`, Artifact roots, and bounded
   Verification Method facts are the Core ceiling.
5. **Package inventories.** Artifact digests and canonical repository paths are
   required at the admission boundary; general package membership, contextual
   metadata, creators, and licenses should live in RO-Crate or native package
   manifests.
6. **Supply-chain attestations.** Vela's provider-neutral release manifest keeps
   only installer and asset-integrity facts that current release verification
   consumes. Generic build steps and predicates should remain in in-toto or
   provider attestations rather than expand that manifest.

The serialized `delegation: null` authority-record slot is not a candidate in
the current epoch: retained authority records commit to it. Its lack of current
behavior does not permit an in-place deletion from signed historical bytes.

## Domain profiles and adapters

A domain profile may define terminology, constraints, mappings, fixtures,
consequence tiers, and Methods. An adapter may identify native objects, bind
exact revisions and selectors, disclose transformations and loss, and emit an
ordinary Submission or Verification Record. Neither changes Protocol 1 or
acquires authority.

Computational science transfers through that existing boundary without a new
Core concept:

1. The native workflow engine, notebook, laboratory system, instrument, or
   simulation repository owns execution and domain semantics.
2. PROV or Workflow Run RO-Crate records the run when a portable provenance or
   workflow package is useful.
3. The adapter references the native run and exact inputs/outputs, retaining
   only decision-relevant bytes as Artifacts.
4. A Submission proposes one bounded Claim change; a Verification Record
   reports one scoped check over exact subjects.
5. The receiving Repository alone decides. Another Repository must make its own
   Decision; a profile, mapping, crate, or passing run cannot transport
   Standing.

This works for mathematics, computation, empirical data, and instrument output
because the scientific content remains native while the authority transition
is domain-independent. A second domain demonstrates adapter portability; it
does not justify Run, Dataset, Instrument, Workflow, DOI, or ontology objects in
Core.

## Explicit nonclaims and evidence ceiling

Vela does not:

- establish scientific or mathematical truth;
- make a signature, successful run, passing check, Git commit, publication, or
  Crossmark status into acceptance;
- replace PROV, RO-Crate, DataCite, Crossmark, SWHID, Git, in-toto, package
  managers, workflow systems, instruments, or domain methods;
- create global authority, global Standing, consensus, or authority over a
  source system; or
- prove adoption, productivity, company value, broad scientific lift, or
  external validation.

The Phase III proceed decision is bounded by exactly the P0 evidence supplied
for it: one fresh held-out D separation, one self-hosted nonmath correction,
and one cold succession PASS. Those observations support publishing this
conceptual and standards disposition. They do not support a protocol expansion
or any broader empirical, adoption, productivity, or company claim.
