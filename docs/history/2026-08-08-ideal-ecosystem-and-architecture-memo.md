---
title: "Vela Ideal Ecosystem and Architecture"
subtitle: "A standards-first, Git-native scientific-state layer that does not rebuild the infrastructure around it"
date: "2026-08-08"
status: "Decision and architecture memo"
decision_horizon: "Final pre-1.0 simplification, then external validation"
audience:
  - "Vela maintainers"
  - "Vela Web maintainers"
  - "Scientific repository stewards"
  - "External workbench, verifier, and institutional partners"
reviewed_repository_heads:
  vela: "vela-science/vela@1ecbd15c119598bfb776acf87d6497addbc87055"
  math: "vela-science/math@a0bfdcd7d99f4bae7351713125d72fb01854f90e"
  vela_web: "vela-science/vela-web@18d72a37222b174680208ef36f201e455e359518"
protocol_release: "Vela 0.970.0"
authority_effect: "None. This memo proposes architecture and migration decisions; it changes no scientific Standing."
---

> **Retained as written, 2026-08-09.** This is the memo that governed the final
> pre-1.0 standards cut, kept at the text it had when the cut was decided. It is
> not maintained against the tree: where it describes what Vela had on
> 2026-08-08 it is now out of date on purpose, because that is the state the
> decisions were made against.
>
> What the cut delivered against it, in this repository:
>
> - §7.4 and ADR 0035 §2 — one DSSE implementation in
>   `crates/vela-protocol/src/kernel/dsse.rs`; Submission, Verification Record
>   and Proposal Withdrawal moved onto it, and the zeroed-preimage convention
>   and every nested second signature deleted.
> - §6.5 and §15.4 — full roots canonical, short handles derived. No object
>   stores its own short identity; a stored handle re-derives from a root
>   present in the same object.
> - §6.7 — repository origin reduced to genesis. `RepositoryOriginKind`,
>   `RepositoryOriginPredecessorV1` and the compaction constructor deleted.
> - §7.3 — Cedar retired. The closed Vela Authorization Profile over AuthZEN's
>   information model is the only evaluator, and the epoch-1 parity corpus that
>   nothing read is now read by a test that recomputes every retained decision.
> - Phase 0 item 5 — the Frontier-as-repository prose repaired across
>   `README.md` and `docs/`.
> - §18 Phase 2 — bundled into one wire break, so `vela-science/math`
>   re-genesises once rather than once per change.
>
> What it did not deliver: the `vela-science/math` re-genesis itself, which
> needs the authority key in a local OpenSSH agent and is an operator ceremony,
> not a repository change. Until it happens the current binary refuses the
> current `math` head, which is the intended sequencing and the release
> blocker.
>
> Everything else the memo proposes — the §12 crate consolidation, the §13
> conformance and interoperability program, the §16 external validation
> sequence — remains a proposal. Read the ADRs under `../adr/` for what is
> decided.
>
> One formatting change was made and no other. The prior Vela memos §A lists
> were inputs to the session that produced this one and are not in this
> repository; two of them were written as relative links, which would resolve
> nowhere here, so they now read as plain names like the rest of that list.

# Vela Ideal Ecosystem and Architecture

## Executive judgment

Vela should be much narrower than a scientific platform and much more consequential than another research application.

It should **not** become:

- a new version-control system;
- a Git forge;
- a package manager or hosted package registry;
- a workflow engine or agent runtime;
- an experiment tracker;
- a notebook, collaborative editor, or universal research IDE;
- an identity provider;
- an artifact store;
- a data-versioning system;
- a provenance database;
- a graph database;
- a universal ontology;
- a canonical hosted database; or
- a second implementation of standards and products that already solve those jobs.

Those layers already exist, often in multiple mature forms. Rebuilding them would consume Vela's engineering capacity while placing it in direct competition with infrastructure that has larger communities, broader deployment, and deeper operational experience.

Vela should own only the missing control point:

> **The exact, authority-scoped transition from scientific work into correction-aware scientific Standing, together with the information a later person or agent needs to understand, challenge, repair, and continue that state.**

That control point can be stated as five questions:

1. What exact scientific assertion or change is being proposed?
2. What exact evidence, artifacts, conditions, caveats, and provenance support the request?
3. What exact properties were checked, by whom, over which inputs, and what was not established?
4. Which named authority accepted or rejected the transition?
5. After later corrections, what still stands and what work is valid next?

The target architecture is therefore:

```text
existing workbenches and infrastructure
  Git, Lean, notebooks, laboratories, agents, data systems,
  native package managers, workflow engines, artifact stores,
  provenance systems, identity providers, and forges
                         |
                         | exact evidence and standard identifiers
                         v
              Vela's narrow semantic waist
  Submission -> Verification -> Decision -> Event -> Standing
                         |
                         | exact read contracts
                         v
             replaceable readers and maps
  local CLI, Observatory, Dossiers, institutional portals,
  independent indexers, and agent-facing read adapters
```

The architecture has one non-negotiable asymmetry:

```text
record broadly
resolve semantically
admit narrowly
replay exactly
```

Activity can be rich, continuous, collaborative, branchable, local-first, and tool-specific. Scientific admission must remain bounded, explicit, exact, independently checkable, and attributable.

### The direct decisions

This memo recommends the following.

1. **Keep Git. Do not build a VCS.** Git remains the byte, tree, commit, ancestry, clone, bundle, mirror, and publication substrate.

2. **Use one ordinary Git repository per scientific authority, not per topic.** `vela-science/math` is the current mathematics authority. Erdős problems, Formal Conjectures, Sidon sets, additive combinatorics, and quantum codes are sources, problems, collections, or derived Frontiers inside and around that authority, not reasons for separate authority repositories.

3. **Keep Frontier derived.** A Frontier is a query over unresolved scientific state. It owns no history, trust root, directory, or identifier. Repository is the authority boundary. Source is the provenance boundary. Problem is the scientific question. Atlas is a projection.

4. **Keep a stable repository identifier, but replace the custom 64-bit `vrepo_...` handle before 1.0.** Use an RFC 9562 UUIDv4 as the non-security logical identifier for one repository lineage. Represent it as a lowercase UUID in the wire and as `urn:uuid:<uuid>` when a URI is required. Security continues to depend on full roots and the independently obtained authority trust root, not the UUID.

5. **Make full roots canonical and short handles derived.** Immutable objects should be identified by full content roots. Truncated `vsb_`, `vvr_`, `vpr_`, and `vro_` values should become display and routing conveniences derived by readers, not separately stored self-identities, unless an object has a demonstrated semantic identity distinct from its complete bytes. The Claim identity is a plausible exception because it intentionally commits to a scientific semantic subset rather than every record field.

6. **Challenge the custom repository-authority implementation against gittuf before freezing 1.0.** Gittuf already provides forge-independent Git policy, root-of-trust metadata, approval attestations, reference-state logging, and independent verification. It cannot replace Vela's scientific Decision, Event, or Standing semantics. It may be able to replace a substantial part of Vela's generic repository policy, keyset, approval, and mutation log. Run one deletion-oriented spike. Adopt only if the result removes substantial custom machinery. Do not keep two permanent authorization systems.

7. **Complete the standards cut already implied by ADR 0035.** Use RFC 8785 JCS, DSSE, JSON Schema 2020-12, AuthZEN's information model, RO-Crate 1.3, in-toto/SLSA, SPDX, OCI/ORAS, OSV, PURL, SWHID, DOI/DataCite, ORCID, ROR, W3C Web Annotation, PROV-O, OpenLineage, and Workflow Run RO-Crate where those standards fit. Vela-specific schemas remain only where the scientific authority semantics are genuinely new.

8. **Delete general policy machinery that the actual product does not need.** The current Cedar engine and the shadow closed evaluator are a temporary dual implementation. Once parity and migration are proven, retain one small AuthZEN-shaped Vela authorization evaluator or adopt an external repository policy layer. Delete Cedar, retained policy material, engine pinning, and duplicate code from the current runtime.

9. **Use PostgreSQL and raw SQL for the scientific read projection.** The projection is a deterministic, reconstructible read model, not an application-owned record database. Its schema and queries are part of the projector contract. Drizzle is appropriate only for a separate mutable product database, such as accounts, preferences, billing, or notifications.

10. **Do not build a Vela package manager or registry.** Publish software and reusable capabilities through Cargo, crates.io, PyPI, uv, npm, Bun, Lake, Git releases, OCI, and ORAS. A Vela reader may index exact external packages by PURL, SWHID, version, and digest. It must not become another resolver, registry, or namespace authority.

11. **Keep the repository topology small.** The ideal current organization is `vela`, `math`, `vela-web`, and `.github`. Archived repositories stay archived. A new repository requires a genuinely independent authority or an independently released and operated product with real consumers.

12. **Finish the provider-loss drill.** Independent retrieval and release verification now largely exist. The remaining end-to-end test is an authorized local Decision followed by a clean projection rebuild with GitHub unavailable.

13. **Build the missing ecosystem, not duplicate the surrounding ecosystem.** The highest-value additions are exact cross-medium references, domain semantic diffs, correction impact, and inherited next work. Even these should start as source-local artifacts or profiles over existing standards and earn promotion through real reuse and net deletion.

---

## 1. Scope, evidence, and precedence

### 1.1 Sources reviewed

This memo reconciles four classes of evidence.

**Current repositories**

- `vela-science/vela`, including the current protocol, architecture, ecosystem, interoperability, continuity, repository profile, root taxonomy, ADRs, code, release machinery, and generated ecosystem status.
- `vela-science/math`, including its current `vela.toml`, re-genesis, repository manifest, authority history, and recent Submit, Verification, accept, and reject transitions.
- `vela-science/vela-web`, including the read-only product boundary, Git-to-Neon projection, source registry, release pin, SQL schema, migrations, registry, and rooted readers.
- `vela-science/.github`, including the current organization profile.
- The archived topic repositories, integration repository, site repository, and research harness as historical evidence.

**Prior Vela memos**

- `ink-and-switch-vela-research-memo.md`
- `vela-ecosystem-strategy-memo-2026-08-07.md`
- `vela-science-repository-consolidation-memo.md`
- `vela-repository-consolidation-memo.md`
- `VELA_ARCHITECTURE_MEMO.md`
- `VELA_FRONTIER_REPOSITORY_ARCHITECTURE_MEMO_2026-07-31.md`
- `VELA_ECOSYSTEM_PACKAGE_TOOLCHAIN_REGISTRY_AND_RUNTIME_ARCHITECTURE_MEMO_2026-08-02.md`
- `vela-package-registry-ecosystem-memo.md`
- `vela_protocol_adoption_and_interoperability_memo.md`
- related Vela Math, source-registry, discovery-loop, and scientific-infrastructure memos.

**External infrastructure and standards**

The review covers official specifications and primary documentation for Git, gittuf, RFC 9562 UUIDs, RFC 8785 JCS, DSSE, JSON Schema 2020-12, AuthZEN Authorization API 1.0, in-toto, SLSA 1.2, Sigstore, TUF, SCITT, OCI 1.1, ORAS, SPDX 3.0.1, OSV 1.8.0, PURL ECMA-427, SWHID ISO/IEC 18670, RO-Crate 1.3, Workflow Run RO-Crate 0.5, W3C PROV-O, W3C Web Annotation, OpenLineage, DataCite 4.7, ORCID, ROR, DataLad, git-annex, and DVC.

**Prior project decisions and corrections**

Past discussions repeatedly established that Vela is built on Git, that the missing layer is promotion from activity into accepted scientific state, and that Vela should not rebuild social feeds, general agent runtimes, laboratories, literature agents, or existing scientific tools. Earlier naming and component decisions are treated as historical when current repositories have superseded them.

### 1.2 Precedence

When sources disagree, use this order:

1. exact current repository bytes and current accepted ADRs;
2. executable schemas, conformance tests, and current release behavior;
3. current architecture and ecosystem documents;
4. current repository-specific operating documents;
5. prior memos;
6. prior conversation memory.

This matters because several older memos correctly identified durable principles but assumed a repository topology that ADR 0039 later replaced. It also matters because some current prose has not caught up with current code.

### 1.3 Fact, inference, and recommendation

This memo distinguishes:

- **Fact:** directly observed in current repository state or an external specification.
- **Inference:** the architectural consequence of comparing those facts.
- **Recommendation:** the action Vela should take.

---

## 2. The current Vela system, as it actually exists

### 2.1 Live and archived repositories

At the reviewed heads, the organization has four active repositories with distinct responsibilities.

| Repository | Current role | Correct long-term role |
| --- | --- | --- |
| `vela-science/vela` | Public Rust protocol, CLI, schemas, conformance, release, architecture | Thin scientific-state kernel and reference implementation |
| `vela-science/math` | Public scientific authority repository | One mathematics authority and its admitted state |
| `vela-science/vela-web` | Private editorial site, Observatory, projector, source registry, PostgreSQL read model | Replaceable read product and projection |
| `vela-science/.github` | Public organization profile | Minimal organization profile, security policy, and only genuinely shared workflows |

Archived repositories include the four topic-specific authority repositories, `vela-frontiers`, `prover-lane-frontier`, `vela-research-harness`, `vela-internal`, and `vela-site`. They are historical evidence, not future architecture.

### 2.2 The current repository model

A current Vela scientific repository is an ordinary Git repository containing:

```text
vela.toml
.vela/repository.json
.vela/origin.json
.vela/authority/events/
.vela/authority/records/
.vela/authority/keysets/
.vela/authority/policies/
.vela/authority/policy-material/
records/claims/sha256/
records/submissions/sha256/
records/verifications/sha256/
records/proposals/sha256/
records/proposal-withdrawals/sha256/
records/artifacts/sha256/
targets.json
domain-native source, proof, computation, and evidence files
```

Git stores the bytes, tree, commit ancestry, branches, tags, clones, bundles, and mirrors. Vela interprets a strict subset of those bytes as scientific-state objects and authority history.

The current `math` repository demonstrates the intended loop in actual commits:

```text
re-genesis
  -> submit
  -> verification import
  -> review accept or reject
  -> replayed Standing
```

That is important. Vela is no longer merely describing an architecture. It is committing exact scientific-state transitions into an ordinary Git history.

### 2.3 The current re-genesis exposes the identity model

`vela-science/math` re-genesisized under Vela 0.970.0 to retire the old Frontier vocabulary from the sealed Cedar and authority-record surface. Its current profile uses:

```text
repository_id = vrepo_8348fae157f9c447
```

Its current origin is generation 1, and its repository manifest binds the new repository identity, one accepted Claim, three Submissions, six Verification Records, three Proposals, artifacts, authority keyset, policy, and origin roots.

This reveals three facts.

1. `repository_id` identifies one Vela repository lineage, not the GitHub repository name.
2. A hard Vela re-genesis creates a new `repository_id`, even when the Git repository and human project name remain.
3. The actual security identity is not the 64-bit handle. Security relies on full roots and an independently obtained first authority-record root.

### 2.4 Cross-repository drift is already visible

At the reviewed heads, current repositories disagree about the new epoch.

| Surface | Observed value |
| --- | --- |
| Current `math/vela.toml` | `vrepo_8348fae157f9c447` |
| Current `math/.vela/origin.json` | generation 1 at Vela 0.970.0 |
| `vela-web` repository registry | predecessor `vrepo_56d3fdfcd34ff5c3` |
| `vela-web` Vela release pin | 0.969.0 |
| `vela/ecosystem-status.json` | predecessor repository ID and zero-state counts |
| `.github/profile/README.md` | old Canopus/product claims and four archived public Frontiers |
| `vela/README.md` and parts of `ARCHITECTURE.md` | still describe Frontier as an ordinary Git authority repository |
| `ECOSYSTEM.md` and ADR 0039 | Repository is authority; Frontier is derived |

This is not a criticism of a transition occurring within hours. It is evidence about architecture.

**Inference:** too many repositories and documents manually repeat the same ecosystem facts. A correct protocol cannot compensate for a composition layer that requires synchronized hand edits across the core repository, Web registry, release pin, ecosystem status, organization profile, and prose.

### 2.5 The current architecture is directionally right

The strongest current boundaries are correct:

- Git publication is not scientific acceptance.
- Submission authenticates producer intent and evidence.
- Verification reports a scoped observation and nonclaims.
- Only an authorized human Decision changes Standing.
- Corrections append rather than rewrite prior state.
- Native workbenches remain sovereign.
- Vela Web is read-only and disposable.
- Neon is a projection, not custody.
- One live mathematics authority replaced four topic repositories.
- Repository exists because there is a new authority, not because there is a new topic.
- Source, Problem, Frontier, and Atlas are separate concepts.

The task is not to redesign those distinctions. The task is to make the implementation smaller, more standard, less duplicated, and externally provable.

---

## 3. The control point Vela should own

### 3.1 Vela is not the substrate below the work

Git, object stores, databases, CRDTs, archives, data-versioning tools, workflow engines, and package managers preserve or move artifacts. Workbenches produce scientific activity. Forges coordinate collaboration. Identity systems authenticate users and workloads.

Those layers are plural and increasingly commoditized.

Vela's scarce layer is the transition contract:

```text
native work
  -> bounded evidence selection
  -> exact Submission
  -> scoped Verification
  -> authorized Decision
  -> admitted Event
  -> correction-aware Standing
  -> next valid Target or explicit blocker
```

### 3.2 The minimum Vela-specific semantics

The following concepts are genuinely Vela's.

**Submission**

A portable producer-authenticated request to add, revise, supersede, correct, or withdraw one scientific Claim, with exact evidence, caveats, conditions, provenance, and declared verification requirements.

**Verification Record**

A signed observation over exact subjects and artifacts that states one checked property, outcome, implementation, environment, independence basis, and explicit nonclaims.

**Decision**

The attributed authority act that accepts or rejects one exact Proposal against one exact current state.

**Event**

The append-only semantic transition admitted by a Decision.

**Standing**

The deterministic result of replaying admitted Events over exact objects.

**Correction and inheritance**

The rules that preserve old state, identify supersession or correction, determine downstream impact, and expose what work remains valid next.

Everything else should be adopted from existing infrastructure or implemented as a replaceable adapter or reader.

### 3.3 The architecture constitution

Use these rules as hard review gates.

1. **Git owns bytes and ancestry. Vela does not.**
2. **Native tools own execution and domain validity. Vela does not.**
3. **Package managers own dependency resolution. Vela does not.**
4. **Artifact and data systems own large-byte storage. Vela does not.**
5. **Identity providers own login and account recovery. Vela does not.**
6. **Provenance systems may record activity. They do not create Standing.**
7. **Forges may coordinate review. A forge review is not a scientific Decision.**
8. **Vela owns the exact scientific transition and correction semantics.**
9. **Readers and databases are disposable projections.**
10. **A new abstraction enters shared architecture only after two maintained consumers and net deletion.**
11. **A new repository requires a new authority or independent product lifecycle.**
12. **A new custom standard requires a demonstrated semantic gap that no mature standard can express.**
13. **Every optional component must pass a deletion test: removing it cannot change historical Standing.**
14. **Pre-1.0 migrations should cut once and delete legacy, not create permanent compatibility strata.**

### 3.4 The build, adopt, adapt, or reject test

For every proposed component, answer in order:

1. Is this behavior part of Vela's unique scientific authority semantics?
2. Does an existing standard define the data or protocol?
3. Does an existing product implement the operational job?
4. Can Vela integrate through an exact adapter, identifier, digest, or export?
5. Would a Vela implementation become another source of truth?
6. Are there two maintained consumers?
7. Does extraction delete more maintained implementation than it adds?
8. Can the component disappear without changing Standing?

The decision rule is:

| Result | Action |
| --- | --- |
| Existing standard and product solve it | Adopt |
| Existing product solves it but Vela needs exact scientific binding | Adapt |
| No existing mechanism expresses the unique scientific transition | Build narrowly |
| New component duplicates infrastructure or has no second consumer | Reject or keep source-local |

---

## 4. Git and the repository model

### 4.1 Keep Git as the custody and publication substrate

Git already supplies:

- content-addressed blobs, trees, and commits;
- exact repository snapshots;
- ancestry and forks;
- local-first operation;
- branches and tags;
- atomic ref updates;
- clone, fetch, push, bundle, and mirror;
- mature tooling and hosting;
- offline inspection;
- a broad ecosystem of forges and clients.

Vela should not reproduce any of these.

The scientific repository should remain usable with standard Git clients. Vela-specific state should remain ordinary tracked files and standard Git objects. A consumer should be able to clone, bundle, mirror, archive, and inspect a Vela repository without a custom storage service.

### 4.2 What Git does not establish

A Git commit does not establish:

- that a Claim is scientifically true;
- that evidence is sufficient;
- that a verifier checked the intended property;
- that the verifier was independent;
- that a reviewer was competent or authorized;
- that a proposed transition should be admitted;
- that a correction applies to downstream work;
- or what the current scientific Standing is.

Those are Vela semantics.

### 4.3 Git commit versus Vela Event

Keep both.

| Object | Question answered |
| --- | --- |
| Git blob | What exact file bytes exist? |
| Git tree | What exact tracked snapshot exists? |
| Git commit | Which tree and ancestry were published, by whom, with what parents? |
| Vela Decision | Which scientific transition did the authority accept or reject, and why? |
| Vela Event | Which semantic state transition entered replay? |
| Vela repository root | What exact active Vela object set does the current semantic state bind? |
| Standing | What follows from deterministic replay? |

A README edit should change the Git tree and commit without changing scientific Standing. A scientific Decision should change the Vela repository root and Standing and should also be published in an atomic Git commit. This separation is useful and should remain.

### 4.4 Transaction shape

A consequential Vela write should continue to follow this form:

```text
read exact Git commit and Vela roots
  -> construct one bounded semantic transaction
  -> validate objects, verification, authority, and expected state
  -> build an isolated candidate tree
  -> sign or authorize one exact Decision/transaction record
  -> atomically move the local Git ref only if the expected ref still matches
  -> publish through ordinary Git
```

Fail stale instead of silently rebasing or merging scientific transitions.

A Submit, Verification import, Withdrawal, accept, or reject may each produce one atomic Git commit because each changes the canonical Vela object set. That does not imply that every agent step, notebook execution, comment, branch edit, or experimental run should become a Vela commit or Event.

### 4.5 Branches, pull requests, and merges

Branches and pull requests are collaboration mechanisms.

They may carry:

- work-in-progress artifacts;
- agent changes;
- code review;
- source updates;
- a proposed Vela transaction;
- verifier results;
- and human discussion.

They do not themselves carry scientific authority.

A Git merge can publish the bytes corresponding to an accepted Vela Decision. The merge does not replace the Decision. A branch can contain scientifically invalid but useful work. A clean merge does not imply that a Claim is accepted.

### 4.6 One active writer, multiple replicas

Retain the current continuity rule:

```text
one active publication remote
multiple read replicas and archives
explicit human promotion
no automatic multi-master
```

GitHub can remain the default collaboration surface. Codeberg, bare mirrors, Git bundles, and object storage can provide independent retrieval. Automatic bidirectional writing would create ambiguous publication heads and operationally expensive reconciliation.

### 4.7 Large artifacts

Do not force large, mutable, licensed, private, or regulated scientific bytes into ordinary Git merely to keep Vela "Git-native."

Use Git for:

- small canonical Vela objects;
- manifests;
- checksums;
- source declarations;
- exact method files;
- small decisive evidence;
- proofs and code where native Git is appropriate.

Use existing systems for larger bytes:

- DataLad and git-annex for distributed scientific datasets and partial retrieval;
- DVC for Git-linked data and model versions;
- OCI and ORAS for verifier images, capsules, model artifacts, and multi-file bundles;
- S3-compatible object storage for large immutable blobs;
- institutional repositories and DOI deposits for durable research outputs;
- domain-native stores for instruments, simulations, and restricted data.

Vela stores exact digests, media types, rights, locators, availability facts, and the relationship of those bytes to a Submission or Verification. It does not become the blob store.

---

## 5. Repository, Source, Problem, Frontier, and Atlas

### 5.1 The correct boundary model

| Concept | Meaning | Owns Standing? | Identifier |
| --- | --- | ---: | --- |
| Repository | One Git repository under one authority, trust model, correction policy, and canonical history | Yes, locally | UUID plus trust roots |
| Source | Exact observation of an external system or corpus | No | Native source ID plus exact version/root |
| Problem | One bounded scientific question | No by itself | Native or repository-local scientific ID |
| Frontier | Derived unresolved state around one or more Problems under a query | No | No protocol ID |
| Atlas | Cross-repository and cross-source projection | No | Projection release/root only |

The central rule remains:

> **A repository exists because there is a new authority, never because there is a new topic.**

### 5.2 Why Frontier should have no protocol identity

A Frontier can be:

- Erdős problems with unresolved status;
- all Claims needing statement-fidelity review;
- all open formalizations;
- all Problems with a certain evidence class;
- a domain or institution-specific slice;
- a counterfactual view after a proposed correction.

These views overlap and change as state changes. Minting a permanent protocol ID for each query would turn navigation into authority-like state and recreate the old topology.

A product may use a human slug for a saved query or route. That slug is a presentation locator, not a protocol identity.

### 5.3 `problems.science`

`problems.science` should be a domain and route set within the existing Vela Web product, read model, release, and deployment architecture. It should not become:

- a new canonical problem database;
- a new authority repository;
- a separate application repository;
- a second source registry;
- or a writable knowledge graph.

It is a read projection over source-native Problems, local Claims and Standing, exact mappings, current work, and declared gaps.

### 5.4 The Atlas

The Atlas should emerge from exact retained work:

```text
source observations
+ transformations and mappings
+ Claim relations
+ evidence roles
+ Verification subjects
+ Decisions and corrections
= root-bound Atlas projections
```

Do not begin with a universal graph and then ask science to populate it. Missing edges must remain missing. Inferred similarity can help discovery but must never become evidentiary support, contradiction, identity, or authority without an ordinary Vela transition.

---

## 6. The `vrepo_` question

### 6.1 Why a logical repository ID is real

A Vela authority repository may be:

- cloned to another machine;
- mirrored to Codeberg;
- restored from a Git bundle;
- moved between organizations;
- served from a private institutional forge;
- retained in object storage;
- read after the original provider disappears.

The host URL therefore cannot be identity.

The current Git commit cannot be identity because it changes. The current Vela repository root cannot be identity because it changes. The human name cannot be identity because unrelated authorities can use the same name. The first authority root is a security trust anchor, but Vela also needs a simple internal key that travels inside repository objects.

A stable logical repository identifier is therefore justified.

### 6.2 Why the current `vrepo_<16 hex>` shape is not ideal

The current initializer hashes a canonical object containing name, scope, and fresh 256-bit entropy, then truncates the digest to 16 hexadecimal characters. The result is 64 bits.

This has four weaknesses.

1. **It is a custom identifier format where RFC 9562 UUIDs already solve decentralized opaque identity.**
2. **The retained 256-bit entropy provides no practical benefit after truncation to 64 bits.**
3. **The identifier is used as a trust-store key, so a collision is operationally expensive even though it is not a security root.**
4. **The custom prefix creates bespoke regexes, validation, documentation, and migration code.**

The current design correctly says that `vrepo_` is only a routing handle. That reduces the severity of collision risk. It does not justify inventing and maintaining the format.

### 6.3 Recommended replacement

Use RFC 9562 UUIDv4.

```toml
schema = "vela.repository-profile.v2"
repository_id = "f7ea2f6d-2f02-4cff-8d8f-1b27d9d41042"
```

Rules:

- generate once from a cryptographically secure random source;
- store lowercase canonical UUID text;
- validate with the standard UUID grammar and JSON Schema `format: uuid`;
- expose `urn:uuid:f7ea2f6d-2f02-4cff-8d8f-1b27d9d41042` when a URI is required;
- never derive scientific meaning, authority, or chronology from it;
- never use it as a substitute for a root;
- preserve the UUID across host moves, key rotation, schema migration, compaction, and any declared re-genesis that continues the same Repository;
- generate a new UUID only when the operator intentionally creates a new repository and authority lineage;
- ordinarily display the repository name and locator, not the UUID.

UUIDv4 is preferable to UUIDv7 here because sortability and embedded creation time do not help repository identity and can invite accidental temporal semantics.

### 6.4 Security identity remains separate

The repository UUID answers:

> Which logical Vela repository lineage is this?

The independent first authority-record root answers:

> Which authority history did this consumer trust?

The current repository root answers:

> What exact active Vela state is present?

The Git commit and tree answer:

> What exact repository bytes and ancestry were published?

These must remain separate.

### 6.5 Do not add an `authority_id` yet

A separate durable authority identity may eventually be useful when one institution governs multiple repositories or one governance entity survives a repository lineage change. That need is not yet demonstrated.

Adding `authority_id` now would create another lifecycle, another mapping, and another ambiguity about whether a re-genesis preserves authority. Use the repository UUID, human and institutional metadata, key history, and trust root until a second real authority relationship proves the need.

### 6.6 Simplify the rest of the identifier taxonomy

Use three classes only.

**Stable non-content identities**

- repository UUID;
- external person, organization, project, instrument, and dataset identifiers;
- Claim semantic identity where it intentionally excludes mutable record metadata.

**Full content roots**

- Submission payload;
- Verification payload;
- Proposal;
- Event;
- Decision or authority transaction;
- Artifact;
- repository manifest;
- projection release;
- policy/model bytes.

**Derived display handles**

- short Proposal, Submission, Verification, Event, and Origin labels;
- abbreviated roots shown in CLI and URLs;
- human slugs.

A display handle can be `vpr_<root-prefix>` or another readable form, but it should be derived by the reader, collision-checked, and absent from the signed preimage unless it carries independent semantics.

### 6.7 Reconsider `origin_id` and compaction machinery

The current `origin_id` is another truncated content-derived handle beside a full origin root. It should become a derived display handle or disappear.

The current origin schema also retains a generalized Compaction predecessor mode. The live `math` repository is now a fresh genesis, and the epoch-1 repositories are archived and unreadable by the current binary. Before 1.0, audit whether any live path still needs current Compaction support.

The target minimum is:

```text
repository UUID
profile root
first authority root
current repository root
Git commit/tree
```

A separate origin object should survive only if it proves a continuity property that cannot be represented by the first authority record, Git history, a signed migration statement, or an RO-Crate/in-toto predecessor package. Do not retain speculative migration machinery solely because an earlier epoch once used it.

---

## 7. Repository authority: what is unique and what may be duplicated

### 7.1 The current stack

The current authority path includes several layers:

- local principal authentication;
- a general Cedar policy engine and retained policy material;
- an AuthZEN-shaped closed evaluator in shadow;
- semantic approval and intent binding;
- a repository authority key;
- a DSSE authority record;
- ordered authority records;
- semantic Events;
- a repository before/after root;
- an exact write-set commitment;
- an atomic Git commit.

Each layer has a defensible purpose in isolation. Together they are at risk of becoming ceremony that only one operator can explain.

### 7.2 The minimum authority property

A correct Vela Decision path must prove:

1. the exact Proposal and subjects reviewed;
2. the exact repository and authority state observed;
3. the authenticated principal;
4. the authorization rule and model applied;
5. the requested action;
6. the human reason;
7. the exact before and after Vela roots;
8. the exact write set;
9. the signature or service attestation over the complete record;
10. atomic publication or a detectable failure before publication.

It does not need a general-purpose policy language, a custom identity provider, a separate grant object, a second signature over the same fact, or an internal workflow engine.

### 7.3 Complete the Cedar removal

The product currently has a fixed, small action set and two human roles. A general Cedar runtime, schema, entity snapshot, policy bundle, engine version pin, policy material store, and rotation lifecycle are disproportionate unless Vela plans to expose arbitrary policy authoring.

The correct target is:

```text
subject
action
resource
context
decision
```

This is the AuthZEN Authorization API 1.0 information model. Vela can use the model in a pure local evaluator without running a network PDP.

Recommended current actions:

```text
administrator:
  authority_initialize
  authority_rotate
  authority_close
  authority_model_update

reviewer:
  review_accept
  review_reject
```

After exact parity tests and one current-epoch cut:

- retain one deterministic closed evaluator;
- retain exact model and request roots;
- recompute authorization during strict replay;
- delete the Cedar dependency;
- delete Cedar schema, policy, entity, and engine material from current repositories;
- delete dual-path code and engine pinning;
- preserve historical verification through exact old tags and binaries.

If future institutions require complex policy, plug an external Cedar, OPA, OpenFGA, SpiceDB, or other PDP behind the AuthZEN boundary and retain the exact decision evidence. Do not make a new Vela policy language.

### 7.4 Consolidate signed object transport under DSSE

Repository authority already uses DSSE. Submission, Verification, and producer Withdrawal still retain bespoke signature preimages.

Move all portable signed objects to:

```text
DSSE envelope
  payloadType: versioned Vela media type
  payload: exact canonical Vela payload bytes
  signatures: standard DSSE signatures
```

Use one maintained DSSE implementation and official vectors. The payload schema remains closed. The outer envelope follows DSSE compatibility rules.

This removes:

- zeroed-field signing conventions;
- repeated raw signature code;
- nested signatures proving the same fact;
- object-specific envelope parsers;
- unnecessary differences between producer, verifier, withdrawal, and authority authentication.

### 7.5 Can gittuf replace generic repository authority machinery?

Gittuf is directly relevant. It is a forge-independent Git security layer developed under OpenSSF. It stores policy, attestations, and a reference-state log in native Git refs and lets consumers independently verify protected ref changes. It supports approvals, thresholds, path and ref policy, multiple signing mechanisms, and multi-repository rules.

This overlaps with Vela in:

- repository root of trust;
- key and role policy;
- approval attestations;
- append-only repository activity;
- ref-update authorization;
- protection against unauthorized or rewritten publication;
- host-independent verification.

It does **not** overlap with Vela in:

- Claim semantics;
- verification scope and nonclaims;
- scientific Proposal meaning;
- scientific Decision reason;
- correction relations;
- Event semantics;
- Standing;
- next valid work.

### 7.6 Recommended gittuf experiment

Do not add gittuf as another permanent layer beside the existing authority chain.

Create one disposable fixture that models:

```text
signed Vela Decision
  -> exact candidate Git commit
  -> gittuf approval/policy
  -> protected main ref update
  -> gittuf verify-ref
  -> Vela replay
```

Measure whether it can delete:

- Vela keyset and policy history;
- generic repository authorization code;
- custom repository activity chaining;
- custom approval duplication;
- some transaction publication checks.

The adoption gate is strict:

```text
same or stronger offline verification
+ exact scientific Decision retained
+ clean one-writer operation
+ stable metadata format
+ acceptable dependency and UX
+ substantial net code deletion
```

If the experiment requires Vela to define a large custom gittuf attestation protocol, reimplement a Go verifier in Rust, or maintain both authority histories, do not adopt it in core.

Gittuf is currently beta. That makes it a serious replacement candidate, not an automatic foundation.

### 7.7 A smaller fallback authority kernel

If gittuf does not clear the gate, the target custom kernel should be one signed transaction record, not a stack of overlapping objects.

A candidate shape is:

```text
DSSE-signed Vela Decision Transaction
  repository UUID
  proposal root
  ordered verification roots
  authenticated principal and issuer
  authorization model and request roots
  action and reason
  before Git commit/tree
  before Vela repository root
  after Vela repository root
  exact write-set root
  timestamp as observation metadata
```

The Event remains the semantic replay input. The Git commit remains publication. Avoid a separate "approval" object when the signed transaction already records the approval.

---

## 8. The standards-first architecture

### 8.1 Standards spine

| Need | Adopt | Vela-specific residue |
| --- | --- | --- |
| Byte storage and ancestry | Git | None |
| Logical repository identity | RFC 9562 UUIDv4 | Meaning: one Vela repository lineage |
| Canonical JSON | RFC 8785 JCS | Closed Vela payload schemas |
| Structural validation | JSON Schema 2020-12 | Semantic and cross-object checks |
| Signed envelopes | DSSE + Ed25519 | Vela payload types and authority meaning |
| Git policy and audit | Gittuf pilot | Vela Decision and Standing remain separate |
| Authorization request model | AuthZEN Authorization API 1.0 | Fixed Vela actions and roles |
| Human hosted authentication | OpenID Connect and WebAuthn/passkeys | Principal binding in Decision record |
| Workload identity | Platform OIDC or SPIFFE when services justify it | Scientific actor remains explicit |
| Software build attestation | in-toto Statement + SLSA 1.2 | Vela release subject references |
| Signature verification bundle | Sigstore bundle where Sigstore is used | Separate release trust policy |
| Update security | TUF only after an automatic mutable update channel exists | No current need |
| Supply-chain transparency | SCITT only when external receipts add value | Never scientific authority |
| Artifact distribution | OCI 1.1 + ORAS, Git releases, object storage | Vela artifact relationship |
| SBOM | SPDX 3.0.1 | No `vela-sbom.json` |
| Vulnerability advisory | OSV 1.8.0 | No Vela vulnerability format |
| Software package identity | PURL ECMA-427 | No Vela package coordinate |
| Software source identity | SWHID ISO/IEC 18670 | Bind exact Vela use and interpretation |
| Research output identity | DOI + DataCite 4.7 | Vela Standing remains local |
| Person and organization identity | ORCID and ROR | Authentication remains separate |
| Research-object packaging | RO-Crate 1.3 | A narrow Vela Decision-chain profile only after reuse |
| Workflow-run packaging | Workflow Run RO-Crate 0.5 | Vela references bounded decision-relevant outputs |
| Provenance exchange | W3C PROV-O | Vela authority relation extensions |
| Operational lineage | OpenLineage | Activity plane only |
| Subobject references | W3C Web Annotation selectors, JSON Pointer, media fragments, domain selectors | Exact root binding and precision/loss |
| HTTP API description | OpenAPI 3.2 | Read-only Vela resource semantics |
| HTTP errors | RFC 9457 Problem Details | Vela error codes as problem types |
| Service telemetry | OpenTelemetry | Never canonical Vela Events |
| Relational projection | PostgreSQL and SQLite | Rooted projector schema and checks |
| Large scientific data | DataLad/git-annex, DVC, object stores, domain systems | Exact digest, rights, locator, and evidence role |
| Package and toolchain resolution | Cargo, uv, npm/Bun, Lake, Conda/Pixi/Spack/Nix | Exact locks and bindings only |
| Agent tool and service protocols | MCP and A2A at the edge when needed | No authority effect |

### 8.2 The rule for custom schemas

A custom Vela schema is justified only when it expresses one of these:

- scientific Claim semantics;
- producer request semantics;
- scoped Verification and nonclaims;
- Proposal transition semantics;
- human Decision semantics;
- correction-aware Event and Standing semantics;
- repository manifest semantics needed for independent replay.

Do not create a custom Vela schema for:

- software packages;
- SBOMs;
- vulnerabilities;
- build provenance;
- generic workflow runs;
- generic provenance;
- people or organizations;
- data citation;
- subobject selectors already covered by Web Annotation or domain standards;
- HTTP errors;
- registry metadata;
- generic authentication;
- generic authorization transport;
- archives or blob manifests.

### 8.3 Standard adoption must still preserve exactness

Using a standard does not mean accepting its broadest or loosest interpretation.

Vela profiles should state:

- exact standard version;
- required fields;
- allowed extension points;
- canonicalization rules where the standard does not define identity;
- exact input and output roots;
- unsupported semantics;
- information loss;
- authority effect;
- conformance fixtures;
- independent reader behavior.

The goal is a narrow profile over a mature standard, not a Vela fork of the standard.


---

## 9. Evidence interoperability: adopt standards, build only the scientific gap

### 9.1 Reference Map should become a Web Annotation profile

The Ink & Switch memo correctly identifies stable, version-aware subobject references as a high-leverage science-translation primitive. The first draft proposed a custom `vela.reference-map.v0`.

Do not standardize that custom envelope unchanged.

W3C Web Annotation already defines:

- a target resource;
- a specific resource;
- selectors;
- states;
- motivations;
- multiple selectors for robustness;
- text quote and text position selectors;
- fragment selectors;
- CSS and XPath selectors;
- media-specific extension.

Use a Vela profile over Web Annotation.

The profile should add only what the standard does not know:

```text
exact artifact root
Vela or external object identity
resolver implementation root
selector profile version
resolution precision
selected-content root where available
known ambiguity or loss
authority_effect = none
```

Selector choices should reuse existing standards:

| Media | Selector |
| --- | --- |
| JSON | RFC 6901 JSON Pointer |
| Text | Web Annotation TextQuoteSelector and TextPositionSelector |
| Audio/video | Media Fragments |
| HTML/XML | Fragment, CSS, XPath, and text selectors |
| PDF | page/region selector profile with exact PDF root |
| Images | SVG or fragment region selectors |
| Tables | domain profile using row/column keys, primary keys, or Arrow schema identities |
| Lean | domain selector for declaration name, syntax node, or expression root |
| Notebooks | cell identity plus output identity |
| CAD and scene graphs | native stable node identity plus exact scene root |

A selector does not assert that two objects mean the same thing. It only identifies a subobject under an exact resolver.

### 9.2 Semantic Diff is a legitimate Vela ecosystem gap

No universal standard can explain every scientifically meaningful change. A theorem, dataset, trial protocol, simulation, image analysis, and biological assay have different invariants.

Vela should define only a small non-authoritative envelope:

```text
before root
after root
domain profile
implementation root
typed structural changes
unresolved or unsupported comparisons
optional narrative presentation
authority effect: none
```

The domain profile owns the change vocabulary.

Examples:

```text
Lean
  statement changed
  hypothesis strengthened
  proof changed without statement change
  axiom introduced
  dependency or toolchain pin changed

Dataset
  rows added, removed, or reclassified
  schema or unit changed
  exclusion rule changed
  missingness or distribution changed
  lineage changed

Experiment
  intervention, control, or endpoint changed
  sample size changed
  instrument or calibration changed
  preprocessing changed
  uncertainty widened or narrowed

Vela Claim
  assertion changed
  conditions widened or narrowed
  evidence added or removed
  provenance changed
  correction relation added
```

The raw byte diff and exact roots remain available. A semantic diff helps a reviewer understand consequences. It never accepts the change.

Start with one Lean profile and one Claim profile in `math`. Do not build a universal semantic-diff SDK until a second genuinely different consumer uses the same envelope and extraction deletes code.

### 9.3 Derivation Trace should use PROV-O and Workflow Run RO-Crate

The prior custom `vela.derivation-trace.v0` sketch largely describes established provenance semantics:

- entities as inputs and outputs;
- activities or executions;
- agents and implementations;
- derivation;
- association;
- environment;
- workflow steps;
- completeness limits.

Use:

- W3C PROV-O for general entity, activity, agent, use, generation, derivation, and attribution;
- Workflow Run RO-Crate for packaged retrospective run provenance;
- OpenLineage for operational job, run, and dataset lineage where a pipeline already emits it;
- RO-Crate 1.3 as the transfer and research-object container.

Vela adds exact links from provenance entities to:

- Submission;
- Verification Record;
- Claim;
- Proposal;
- Decision;
- Event;
- current Standing;
- correction impact.

The provenance graph remains incomplete unless its declared scope proves otherwise. A complete run trace proves lineage, not scientific truth.

### 9.4 Research-object transfer

Use RO-Crate 1.3 when a bounded Vela result must travel with:

- native source files;
- data;
- software;
- workflows;
- people and organizations;
- licenses;
- citations;
- exact Vela objects;
- checksums;
- reproduction instructions;
- known omissions.

Do not create a Vela archive format.

The retained Erdős decision-chain RO-Crate experiment is the right source of evidence. Promote a Vela profile only after an independent consumer reads it and a correction behaves correctly.

### 9.5 External identifiers

Use the identifier owned by the native system.

| Object | Identifier |
| --- | --- |
| Software source artifact | SWHID and Git object ID |
| Software package | PURL plus native version and registry |
| Research output or dataset | DOI and DataCite metadata |
| Researcher | ORCID |
| Organization | ROR |
| Formal declaration | native system, repository commit, path, declaration name, and expression root |
| Dataset version | native snapshot/version plus digest |
| Container or arbitrary OCI artifact | OCI digest |
| Vela repository | repository UUID plus authority roots |
| Vela immutable object | full Vela content root |

External identifiers are anchors, not proofs. ORCID does not authenticate a Decision. DOI does not establish scientific Standing. A PURL does not establish package safety. A SWHID does not say how software was interpreted.

---

## 10. Native tools and products Vela should use, not rebuild

### 10.1 Version control and forges

Use:

- Git for storage and ancestry;
- GitHub for default public collaboration and discovery;
- Codeberg or another independent forge for read replication;
- Forgejo or GitLab only when a partner needs a self-hosted collaboration control plane;
- Git bundles and object storage for provider-independent retention;
- gittuf as a replacement candidate for Git security policy;
- standard Git signing, SSH signatures, GPG, or Sigstore where appropriate.

Do not build:

- Vela branches;
- Vela merges;
- Vela pull requests;
- Vela issues;
- a Vela forge;
- a Vela distributed Git network;
- a Vela object database;
- a Vela replacement for Entire, Cursor Origin, Radicle, Forgejo, or GitLab.

### 10.2 Workbenches and agent activity

Use or integrate:

- Lean, Lake, Mathlib, Physlib, and native theorem-prover ecosystems;
- Jupyter, Quarto, R, Python, Julia, MATLAB, and domain notebooks;
- OpenResearch, laboratory software, workflow engines, and simulation tools;
- Codex, Claude, OpenCode, and future agents;
- Entire or similar systems for session history and checkpoints;
- Ink & Switch-style or CRDT workspaces for collaboration;
- native experiment trackers where a team already uses them.

Vela accepts exact evidence from these systems. It does not launch them, schedule them, manage their branches, preserve every token, or become the execution control plane.

### 10.3 Package managers and registries

Use:

- Cargo and crates.io for Rust;
- uv and PyPI for Python;
- Bun/npm for TypeScript;
- Lake and exact Git revisions for Lean;
- Conda, Pixi, Spack, or Nix for scientific environments;
- OCI and ORAS for arbitrary immutable artifacts and attestations;
- native model and data hubs where appropriate.

Do not build:

- `vela package`;
- `vela install` as a package resolver;
- `vela://`;
- a Vela package coordinate;
- a Vela lockfile for native dependencies;
- a hosted Vela package registry;
- a package marketplace;
- a Vela toolchain manager;
- a Vela universal environment solver.

A Vela read surface may index existing packages and exact capabilities. Indexing is not resolving or hosting.

### 10.4 Data and artifact systems

Use:

- Git for small exact records;
- DataLad/git-annex for distributed scientific datasets;
- DVC for Git-linked data and model versions;
- OCI/ORAS for images, models, capsules, and verifier bundles;
- Parquet and Arrow for tables;
- Zarr for multidimensional arrays;
- institutional data repositories and DOI deposits;
- S3-compatible stores for immutable large objects;
- domain repositories and access systems for regulated data.

Do not build a universal Vela artifact store or force all evidence bytes into `.vela/`.

### 10.5 Workflow and provenance systems

Use:

- Workflow Run RO-Crate;
- PROV-O;
- OpenLineage;
- CWL, WDL, Nextflow, Snakemake, DVC pipelines, or native workflow formats;
- OpenTelemetry for service traces.

Do not build:

- a Vela workflow language;
- a Vela scheduler;
- a Vela run database;
- a Vela generic provenance ontology;
- a Vela observability protocol.

### 10.6 Identity and access

Use:

- OpenSSH agent and Ed25519 for local offline authority where appropriate;
- OpenID Connect and WebAuthn/passkeys for hosted human sessions;
- GitHub Apps and short-lived installation tokens for GitHub automation;
- cloud workload OIDC or SPIFFE only when persistent distributed services justify it;
- ORCID and ROR as scholarly metadata.

Do not build:

- a Vela identity provider;
- a Vela account-recovery system;
- a Vela DID method;
- a custom capability cryptosystem;
- a permanent user key registry inside every scientific object.

Scientific actor identity, authentication, authorization, and expertise are separate. A login proves account control. It does not prove scientific competence or truth.

### 10.7 Database and graph infrastructure

Use:

- PostgreSQL for hosted projections;
- SQLite for local read indexes;
- normalized SQL relation tables;
- full-text search and ordinary indexes;
- pgvector only when a measured product query needs embeddings;
- in-memory graph libraries for bounded neighborhoods.

Do not add:

- a graph database;
- a vector database;
- an event broker as canonical state;
- a distributed database;
- a blockchain;
- a global consensus service;
- a mutable Atlas database as scientific authority.

Add a specialized store only after a measured workload fails the simpler design.

---

## 11. Package architecture: close the question for now

### 11.1 Reconcile the prior package memos

Earlier package memos performed a valuable survey of Cargo, crates.io, npm, PyPI, Julia registries, Nix, OCI, Artifact Hub, MCP Registry, Hugging Face, and research packaging. The durable findings are:

- packages must remain non-authoritative;
- native systems should remain sovereign;
- releases should be immutable;
- intent and exact resolution are different;
- registries index and distribute but do not own semantics;
- exact roots outrank locators;
- promotion requires two consumers and net deletion;
- OCI is useful for arbitrary immutable artifacts and attestations;
- publication never creates Standing.

The conclusion that Vela should eventually build functional equivalents of Cargo and crates.io is superseded by current evidence.

The package qualification experiment failed its own promotion gates. The current ecosystem document closes the hosted package-registry destination. The active product has no independently published Vela-native package that cannot be distributed through an existing ecosystem.

### 11.2 Current decision

There is no general Vela package system.

A reusable artifact belongs in its native ecosystem:

| Reusable artifact | Distribution |
| --- | --- |
| Rust library | crates.io when external API exists |
| Python adapter/tool | PyPI or immutable release artifact |
| TypeScript library | npm when cross-repository consumers exist |
| Lean library/verifier | Lake and exact Git revision |
| Container or verifier capsule | OCI registry by digest |
| Schema and conformance corpus | Git release or OCI artifact |
| Workflow | native workflow registry or repository |
| Research transfer bundle | RO-Crate through archive, repository, DOI deposit, or OCI |
| MCP server | MCP Registry or native service catalog |
| Agent service | A2A discovery where needed |
| Dataset | native data repository, DataLad, DVC, DOI deposit, or object store |

### 11.3 What a future index may do

The existing Vela Web application may eventually provide a read-only page that says:

- this adapter lives at this Git/SWHID;
- this package is identified by this PURL;
- this exact release has this digest;
- this verifier claims this interface;
- these conformance results exist;
- these known failures and advisories apply;
- these scientific repositories use it.

That page is a catalog. It does not resolve dependencies, mirror all content, assign global names, accept publishers, or govern scientific state.

### 11.4 Reopening gate

Reopen a Vela-native package layer only when all are true:

1. a reusable capability cannot be represented cleanly in an existing package or artifact ecosystem;
2. two independent maintained consumers need the same exact contract;
3. independent implementations agree on its root;
4. extraction deletes maintained duplication;
5. the package can be removed without changing Standing;
6. a native registry cannot distribute it without losing required semantics;
7. an external publisher exists.

Until then, source-local profiles and native packages are the correct architecture.

---

## 12. The current repository topology

### 12.1 `vela-science/vela`

Own:

- Vela-specific scientific-state wire semantics;
- canonicalization and root rules;
- Submission, Verification, Proposal, Decision, Event, and Standing semantics;
- strict replay;
- authority and transaction reference implementation;
- generated JSON Schemas;
- conformance vectors and clean-room readers/emitters;
- the CLI;
- release construction;
- current protocol ADRs and architecture;
- generic cross-domain correction and read contracts.

Do not own:

- scientific Claims or Decisions;
- topic campaigns;
- agent execution;
- a Web app;
- a hosted API;
- source-specific ingestion;
- a package registry;
- a universal ontology;
- native package resolution;
- private integration state.

The current five Rust crates are implementation boundaries, not products. Review them after the Cedar and signature simplification:

- merge `vela-authority` if it becomes only a small pure evaluator;
- keep `vela-verify` separate only if dependency-light frozen verification remains a real boundary;
- keep `vela-edge` only if it owns substantial derived analysis used by multiple CLI paths;
- do not preserve a crate because its name once appeared in architecture.

### 12.2 `vela-science/math`

Own:

- the mathematics authority profile;
- its repository UUID and trust root;
- source declarations and locks;
- local scientific Claims, Submissions, Verifications, Proposals, Decisions, Events, and Standing;
- native mathematical evidence;
- exact local Target packets;
- local statement-fidelity reviews;
- source-local experiments for semantic diff, references, and provenance;
- local correction obligations.

Do not own:

- a mathematics library competing with Mathlib;
- a universal mathematical ontology;
- an index of all mathematics;
- authority over external sources;
- generic Vela implementation;
- a package registry;
- topic subrepositories.

The repository name `math` describes subject matter. Its justification is the authority, not the topic.

### 12.3 `vela-science/vela-web`

Own:

- `www.vela.space`;
- `app.vela.space`;
- `problems.science` routes;
- the read-only Observatory;
- Git and source acquisition for projection;
- PostgreSQL schema, migrations, and projector;
- root-bound search, graph, Dossier, source, Problem, and Frontier views;
- static editorial snapshots;
- deployment manifests and last-known-good behavior;
- product UX and evaluation.

Do not own:

- a signer;
- a scientific mutation API;
- a hosted Decision service in the current architecture;
- canonical Claims;
- authority credentials;
- inferred Standing;
- a package resolver;
- a canonical source database;
- scientific truth in Neon.

### 12.4 `vela-science/.github`

The current repository contains only the organization profile. Its public profile is stale and describes archived Frontiers and removed products.

Choose one honest role.

**Minimum role**

- current organization profile;
- SECURITY policy;
- CONTRIBUTING and support defaults;
- repository templates only when reused.

**Expanded role, only after two consumers**

- genuinely reusable workflows;
- organization-level dependency-update policy;
- a host-neutral, noncanonical ecosystem catalog used to generate the profile and validate declarations.

Do not claim reusable workflows or security policy exist until files actually exist.

### 12.5 Archived repositories

Keep archived repositories:

- immutable;
- publicly readable where licensing permits;
- mirrored;
- tied to their historical Vela binary;
- absent from current projection and current protocol compatibility;
- available as provenance for deliberate re-admission.

Do not reopen them for:

- new scientific work;
- package experiments;
- compatibility code;
- current source adapters;
- architecture memos;
- migration convenience.

### 12.6 Rule for creating another repository

Create a new scientific repository only for:

- an independently governed authority;
- a materially different maintainer or reviewer set;
- a materially different correction or admission policy;
- a confidentiality boundary that cannot safely share one repository;
- a legal or institutional boundary that needs independent custody.

Create a new product repository only for:

- an independently deployed product;
- an independent release and security lifecycle;
- a clear owner;
- a boundary that cannot remain a package in an existing monorepo.

Do not create a repository for:

- a topic;
- a problem;
- a source;
- an adapter;
- an experimental profile;
- a campaign;
- an SDK with one consumer;
- an index;
- a memo;
- a package registry plan;
- a new route or domain.

---

## 13. Vela Web, PostgreSQL, and the projection boundary

### 13.1 Keep the read-only architecture

The existing product boundary is strong:

```text
authority Git repository
  -> clean exact checkout
  -> pinned Vela replay and source validation
  -> deterministic projection candidate
  -> one PostgreSQL transaction
  -> table-root verification
  -> atomic current-release pointer
  -> read-only Web deployment
```

A failed refresh leaves the previous release current. A read-only database role serves the application. The projector can reconstruct a database from an empty state.

Keep this.

### 13.2 Keep raw SQL for scientific projection data

Raw SQL is the correct default for `packages/frontier-data` because:

- the SQL schema is a read-model specification;
- migrations are content-addressed and immutable;
- reconstruction must produce the same database shape;
- queries explicitly bind release roots;
- graph, search, and retention behavior are easier to audit in SQL;
- PostgreSQL capabilities are part of the design;
- there is no mutable domain model whose TypeScript object graph should become authoritative;
- adding an ORM creates another schema representation and drift channel.

Keep SQL concentrated behind typed functions. Do not scatter SQL through React routes.

The desired boundary is:

```text
raw parameterized SQL
  -> explicit row parsing and validation
  -> typed semantic read functions
  -> Web routes and components
```

### 13.3 Where Drizzle fits

Use Drizzle only for a separate product-state database if Vela Web later owns:

- accounts;
- organizations;
- memberships;
- preferences;
- saved views;
- notifications;
- billing;
- feature flags;
- product audit logs.

That state must be physically and conceptually separate from scientific projections.

```text
scientific projection
  raw SQL
  read-only
  reconstructible
  root-bound
  noncanonical

product state
  Drizzle is reasonable
  mutable
  account-owned
  service-authoritative for product behavior
  no scientific authority
```

### 13.4 Rename legacy Frontier storage concepts

`packages/frontier-data` and `frontier_slug` were created when Frontier meant repository authority. After ADR 0039, that vocabulary is wrong.

Recommended migration:

- rename package to `@vela/observatory-data` or another product-bound name;
- key authority-derived rows by `repository_id`;
- represent saved Frontier views as query definitions or noncanonical view slugs;
- keep Source, Problem, and Repository dimensions separate;
- migrate root-bearing SQL columns once, with exact old/new root comparison;
- do not preserve aliases indefinitely.

A URL such as `/frontiers/erdos` may remain a product route if it clearly means a derived view. It must not imply a repository, trust root, or canonical state owner.

### 13.5 Consolidate the ecosystem declaration

Current cross-repository drift should be fixed without reviving `vela-internal`.

Use one small host-neutral organization catalog in the existing `.github` repository, or another clearly owned noncanonical location, containing only:

```text
repository name
role
status
human description
primary and replica locators
expected repository UUID where applicable
release compatibility declaration
owner
```

Generate or validate:

- the organization profile;
- `vela` ecosystem status;
- the Vela Web repository source list;
- current documentation tables.

The catalog is not a trust root and not required for replay. The independent authority root still arrives through a separate channel.

Do not build a registry service. This is a checked declaration file.

### 13.6 Public read interoperability

Vela Web may remain a product implementation rather than the standard. External readers must be able to reproduce the essential state from public Vela contracts.

Publish and maintain:

- JSON Schemas;
- CLI JSON read contracts;
- OpenAPI 3.2 for hosted read endpoints;
- RFC 9457 Problem Details for HTTP errors;
- conformance fixtures;
- at least one independent clean-room reader;
- root-bound static exports or local reader paths for critical Dossiers.

No consumer should need the private Vela Web source to verify Standing.

---

## 14. Ink & Switch and universal version control

### 14.1 The correct lesson

Ink & Switch does not imply that Vela should build:

- a local-first editor;
- a CRDT;
- a universal version-control engine;
- a canvas;
- a collaborative document model;
- a branching UI;
- a new Git replacement.

Its durable lesson is that meaningful version control operates on domain objects and operations rather than only byte diffs.

The correct boundary is:

```text
Ink & Switch and workbench territory
  continuous history
  branches
  comments
  local ownership
  collaborative editing
  stable pointers
  domain-aware operations
  semantic diffs
  provenance

                 exact evidence boundary

Vela territory
  Submission
  scoped Verification
  authorized Decision
  Event
  correction-aware Standing
  replay
  inherited next work
```

### 14.2 Adopt the principles through standards and adapters

| Ink & Switch principle | Vela action |
| --- | --- |
| User-owned durable data | Git clones, mirrors, bundles, provider-independent releases |
| Dynamic history | Keep in workbench/activity system |
| Formality on demand | Structure at Submission boundary |
| Stable pointers | Web Annotation profile plus exact roots |
| Domain-aware diff | Source-local semantic-diff profiles |
| Provenance as project map | PROV/RO-Crate/OpenLineage projection |
| Branchable alternatives | Git/workbench branches and counterfactual read views |
| Malleable tools | Open read and write contracts, no mandatory Vela IDE |
| CRDT convergence | Activity only, never scientific acceptance |
| Material, low-latency UX | Fast local CLI and rooted Web reads |

### 14.3 The Reference Map, Semantic Diff, and Derivation Trace experiments

Retain the three experiments, but modernize them.

**Reference**

Build a Vela Web Annotation profile over exact artifacts and standard selectors.

**Semantic Diff**

Build one Lean and one Claim profile as source-local artifacts.

**Derivation**

Use PROV-O and Workflow Run RO-Crate. Add Vela links only where generic provenance ends.

Render all three in one Result Dossier. Promote none until a second maintained consumer and net deletion exist.

### 14.4 Atlas and counterfactuals

Ink & Switch's possibility-space and dynamic-document work is useful for read surfaces.

A read-only system may show:

- what would become stale if a Claim were corrected;
- what Targets would open if a Proposal were accepted;
- which evidence routes survive an assumption change;
- alternative mappings;
- unresolved conflicts;
- the minimum repair set.

Every counterfactual must name:

- the current source root;
- the hypothetical change;
- the projector implementation;
- completeness limits;
- authority effect `none`.

A counterfactual must never change `vela next`, Standing, or repository history without an ordinary Decision.


---

## 15. Delete, replace, consolidate, or retain: the pre-1.0 machinery audit

The right pre-1.0 question is not whether each current mechanism can be defended in isolation. It is whether the mechanism remains necessary after mature infrastructure and standards are placed around the narrow Vela semantic core.

Every component should receive one of four dispositions:

```text
retain       unique Vela semantics or proven product value
replace      a mature standard or product already owns the job
consolidate  the job is valid, but it is represented more than once
remove       ceremonial, legacy, dead, or unproven machinery
```

### 15.1 Retain: the irreducible Vela kernel

These are the parts Vela should continue to own.

| Capability | Why Vela owns it |
| --- | --- |
| Claim semantics | Existing systems do not define Vela's correction-aware scientific assertion contract |
| Submission boundary | This is the portable request to change authority-scoped scientific state |
| Scoped Verification | Existing checker outcomes need an exact scientific subject, scope, nonclaims, independence disclosure, and outcome |
| Scientific Proposal | A bounded candidate transition is distinct from a Git branch or pull request |
| Scientific Decision | Only Vela defines the attributed act that admits or rejects the scientific transition |
| Event and reducer | Current Standing and correction history need deterministic scientific semantics |
| Standing | This is authority-local admitted state, not a repository status, verifier result, or confidence score |
| Correction impact | Determining what remains valid after correction is central to scientific inheritance |
| Target derivation | The next valid work must follow from exact current state without acquiring authority |
| Interoperability profiles specific to the above | Standards can carry the objects, but they do not define these semantics |

The kernel must be implementable and independently readable without Vela Web, GitHub, Neon, Drizzle, a hosted identity provider, a package registry, a graph database, or an agent runtime.

### 15.2 Replace: custom machinery with mature standards

| Current or potential custom machinery | Replacement |
| --- | --- |
| Custom JSON canonicalization | RFC 8785 JCS |
| Raw or zeroed-field signing conventions | DSSE envelopes with distinct payload types |
| Handwritten external shape documentation | Generated and checked JSON Schema 2020-12 |
| Generic authorization request shape | AuthZEN Subject, Action, Resource, Context, Decision information model |
| General research-object archive format | RO-Crate 1.3 plus ordinary archive, repository, deposit, or OCI transport |
| Custom provenance vocabulary | PROV-O, Workflow Run RO-Crate, and OpenLineage |
| Custom subobject pointer envelope | W3C Web Annotation plus domain selectors |
| Custom build provenance | in-toto Statement plus SLSA provenance |
| Custom signature bundle or transparency format for releases | Sigstore bundle where operationally suitable, otherwise DSSE plus independently distributed keys |
| Custom SBOM | SPDX 3.0.1 or CycloneDX |
| Custom vulnerability format | OSV |
| Custom package coordinate | PURL when a package ecosystem actually exists |
| Custom software source identifier | Git object ID plus SWHID where durable archival identity matters |
| Custom research publication identifier | DOI/DataCite |
| Custom person and organization identifiers | ORCID and ROR |
| Custom artifact registry | OCI 1.1 plus ORAS when Git releases cease to be sufficient |
| Custom update-security framework | TUF only if Vela later operates a mutable automatic update channel |
| Custom data-versioning layer | DataLad/git-annex, DVC, object stores, or domain stores |
| Custom workflow language | CWL, WDL, Nextflow, Snakemake, native notebooks, or laboratory systems |
| Custom telemetry protocol | OpenTelemetry |
| Custom workload identity | OIDC, SPIFFE/SPIRE, cloud workload identity, or forge application tokens |
| Custom Git backup format | `git bundle`, bare mirrors, ordinary clones, and signed archive manifests |

The rule is not "use every standard." It is:

> Use the smallest established standard that removes Vela-owned semantics without importing a larger platform than the actual requirement.

### 15.3 Consolidate: duplicate representations of one fact

The current ecosystem has repeatedly drifted because the same fact is declared in multiple repositories and documents.

Examples observed at the reviewed heads include:

- `vela-science/math` has re-genesisized under Vela 0.970.0 with repository ID `vrepo_8348fae157f9c447`;
- `vela-web` still pins the earlier repository ID `vrepo_56d3fdfcd34ff5c3` and Vela 0.969.0;
- `ecosystem-status.json` reports the older mathematics identity and empty object counts;
- the organization profile still presents Canopus as current and lists four public Frontiers;
- current README and architecture prose still contain several Frontier-as-repository formulations after ADR 0039 made Frontier derived.

This is not a reason to add a stronger central database. It is evidence that cross-repository declarations need one generated, non-authoritative catalog and strict reconciliation.

#### Recommended declaration flow

```text
source repositories
  vela release metadata
  math vela.toml and repository roots
  vela-web deployment metadata
  GitHub repository status
          |
          v
provider-neutral reconciliation script
          |
          v
one generated ecosystem-status artifact
          |
          +--> organization profile checks
          +--> web projection checks
          +--> documentation assertions
          +--> release and mirror checks
```

The catalog may live in `vela-science/.github` because it describes organization-level topology. It must be generated from source repositories, have no scientific authority, and never be required to replay a Repository.

A hand-maintained copy of the mathematics repository ID in `vela-web` should be temporary. Prefer acquisition that reads `vela.toml`, checks the independently configured expected authority root, and then records the observed repository ID. If a deployment must permit only named repositories, bind a full repository trust configuration generated from the catalog rather than repeating one short ID in application source.

### 15.4 Remove: legacy and ceremonial surfaces

The following should be removed when current consumers no longer require them.

#### Epoch terminology and compatibility

- Retired `Frontier` spellings that refer to a Repository.
- `frontier_slug` database columns when they actually key Repository projections.
- `frontier-data` as a package name when the package projects Repositories, Sources, Problems, Claims, and derived Frontiers. Rename it once to `observatory-data` or `projection-data` during the root-moving SQL migration.
- Current-runtime readers, aliases, fixtures, or code paths for archived epochs. Historical binaries and exact tags preserve old repositories.
- Old Canopus and research-harness descriptions on current public surfaces.

#### Redundant identifiers

- Stored short `origin_id` if the full `origin_root` identifies the origin exactly.
- Stored short IDs for Submission, Verification, and Proposal when they can be deterministically derived from the full object root or signed preimage and are not semantically distinct.
- A custom `vrepo_` generator after the repository identity migrates to UUIDv4.
- Any resolver that silently selects one object from a truncated-ID collision. Ambiguity must fail.

#### Origin and compaction ceremony

The current `RepositoryOriginV1` models genesis and one compacted predecessor with a large custom predecessor block. No live Repository at the reviewed head needs an active compaction path.

Before 1.0, decide whether compaction is a supported recurring operation. If the answer is no:

- reduce current origin to repository UUID, generation, profile root, initial state root, and reason;
- express predecessor continuity as a separately signed in-toto or DSSE migration attestation over exact Git commits, trees, Vela roots, archive digests, and equivalence report;
- retain that attestation as evidence rather than expanding the permanent core origin schema;
- delete the unused compaction writer and reader code from the current runtime.

Do not keep a permanent feature because it was once needed to repair a pre-release architecture.

#### Dual authorization implementation

- Delete Cedar after the replacement is selected and parity is proven.
- Do not retain Cedar and the closed evaluator as permanent defense in depth. Two policy engines create two interpretations.
- If gittuf replaces generic Git governance, delete the overlapping Vela key-distribution, file-policy, approval, and reference-history machinery.
- If gittuf does not clear the adoption gate, keep only the closed Vela evaluator and scientific Decision chain. Do not retain a general policy language.

#### Release duplication

A Vela release manifest should not restate every fact already present in standard attestations.

The ideal release set is:

```text
source Git commit and tree
binary and archive digests
SPDX SBOM
in-toto/SLSA provenance
signature bundle or DSSE signature
small provider-neutral installer index
```

The installer index may point to assets and their attestations. It should not become a competing provenance, SBOM, package, or transparency format.

#### Unused package and registry plans

Remove active-roadmap language for:

- `vela-packages`;
- a Vela package lock;
- package acquisition commands;
- a hosted package registry;
- `vela://` package coordinates;
- a Vela runtime or environment resolver.

Retain the historical memos as research evidence. Do not carry failed experiments as active architecture.

### 15.5 Consolidate implementation crates without erasing useful boundaries

The current Rust workspace has five private crates. Private crates are implementation boundaries, not products. The correct criterion is build and ownership clarity, not an analogy to a public ecosystem.

Recommended review:

| Crate | Decision test |
| --- | --- |
| `vela-protocol` | Retain as the portable semantic and wire-contract core |
| `vela-verify` | Retain if it remains dependency-light and independently useful; otherwise merge read-only verification into protocol |
| `vela-edge` | Retain only for genuinely derived analysis and adapters; move repository mutation or protocol semantics out |
| `vela-authority` | Merge or delete after Cedar removal if the remaining closed evaluator is small |
| `vela-cli` | Retain as product shell and local transaction edge |

Do not publish internal crates until a third party has a supported library use. One binary and one release identity remain the default.

### 15.6 A deletion budget for every new feature

Every proposed feature should include:

```text
new source lines
new schemas
new persistent objects
new public commands
new services
new secrets
new operational runbooks
old code and concepts deleted
```

A feature that adds a permanent subsystem and deletes nothing needs unusually strong evidence. A standards adoption that deletes custom code should be preferred even if the initial migration is harder.

---

## 16. The ideal target architecture

### 16.1 The ecosystem layers

```text
┌──────────────────────────────────────────────────────────────────────┐
│ 1. Native work and activity                                           │
│                                                                      │
│ Git branches, worktrees, Entire, Patchwork, notebooks, Lean, Python, │
│ Julia, R, instruments, laboratory systems, workflow engines, agents  │
│                                                                      │
│ Owns: execution, exploration, collaboration, rich local history       │
│ Vela authority effect: none                                           │
└────────────────────────────────┬─────────────────────────────────────┘
                                 │
                                 │ exact exports, standard identifiers,
                                 │ native locks, artifacts, provenance
                                 v
┌──────────────────────────────────────────────────────────────────────┐
│ 2. Evidence interpretation                                            │
│                                                                      │
│ Web Annotation references, semantic diffs, PROV/RO-Crate derivation, │
│ explicit loss reports, source-native adapters                         │
│                                                                      │
│ Owns: interpretation of exact native evidence                         │
│ Vela authority effect: none                                           │
└────────────────────────────────┬─────────────────────────────────────┘
                                 │
                                 │ portable DSSE/JCS Vela objects
                                 v
┌──────────────────────────────────────────────────────────────────────┐
│ 3. Vela scientific-state kernel                                       │
│                                                                      │
│ Submission -> Claim -> Proposal -> Verification -> Decision -> Event │
│ -> reducer -> correction-aware Standing                              │
│                                                                      │
│ Owns: admission semantics and deterministic scientific inheritance    │
│ Authority effect: authorized Decision only                            │
└────────────────────────────────┬─────────────────────────────────────┘
                                 │
                                 │ Git commit/tree plus Vela roots
                                 v
┌──────────────────────────────────────────────────────────────────────┐
│ 4. Custody, governance, and distribution                              │
│                                                                      │
│ Git, one active writer, mirrors, bundles, optional gittuf, release    │
│ attestations, OCI/object storage for large bytes                       │
│                                                                      │
│ Owns: byte custody, ref publication, retention, generic Git policy    │
│ Scientific authority effect: none beyond carrying signed Vela state   │
└────────────────────────────────┬─────────────────────────────────────┘
                                 │
                                 │ strict replay and exact projection
                                 v
┌──────────────────────────────────────────────────────────────────────┐
│ 5. Readers and discovery                                              │
│                                                                      │
│ local CLI, PostgreSQL projection, Observatory, problems.science,      │
│ Dossiers, institutional portals, independent indexers, agent APIs     │
│                                                                      │
│ Owns: explanation, search, maps, counterfactuals, next-work views     │
│ Authority effect: none                                                │
└──────────────────────────────────────────────────────────────────────┘
```

### 16.2 The one canonical lifecycle

```text
current Standing
      |
      v
root-bound Target
      |
      v
native work in any tool
      |
      v
portable Submission plus exact Artifacts
      |
      v
pending scientific Proposal
      |
      +--> scoped Verification Record(s)
      |
      v
authorized human Decision
      |
      v
scientific Event
      |
      v
deterministic replay
      |
      v
new Standing, correction impact, and next valid Target
```

No other path changes Standing.

In particular:

- a Git merge does not change Standing by itself;
- a pull-request approval does not change Standing by itself;
- a gittuf policy pass does not change Standing by itself;
- a verifier pass does not change Standing by itself;
- an agent branch does not change Standing by itself;
- a Web database write does not change Standing;
- a graph edge, embedding, ranking, or model summary does not change Standing;
- package publication does not change Standing.

### 16.3 Repository internals after simplification

An ideal scientific Repository remains understandable as an ordinary Git tree.

```text
vela.toml
.vela/
  origin.json
  state.json or repository.json
  decisions/ or events/
  authority/                     only the minimum retained authority material
records/
  claims/sha256/
  submissions/sha256/
  verifications/sha256/
  proposals/sha256/
  artifacts/sha256/
targets.json                     optional derived index
sources.yaml
sources.lock.json
native scientific files and evidence
```

The exact directory spelling is secondary to five invariants:

1. canonical objects are content-addressed and never hand-edited;
2. the current state snapshot is derivable from retained semantic history;
3. private work and operation journals are ignored;
4. every tracked canonical byte is stable under Git checkout and attributes;
5. ordinary domain files remain native and are not forced into `.vela/`.

### 16.4 State snapshot versus semantic history

The repository currently carries both semantic authority history and a current repository manifest. That is acceptable only if their relationship is explicit.

Recommended invariant:

```text
semantic history + retained objects + authority model
                |
                v
       deterministic reducer
                |
                v
      exact current state snapshot
```

The snapshot may be tracked for fast reads, atomic transaction checks, and publication. It must never be an independently editable source of truth. Strict replay must rebuild it and compare exact bytes or roots.

If a future design can delete the tracked snapshot without harming performance or safety, prefer deletion. Do not add a second event stream, materialization log, or database journal.

### 16.5 Multiple authorities and federation

The mature ecosystem should contain multiple independently governed repositories, not one universal repository and not multiple Vela-owned replicas pretending to be independent.

```text
Repository A, UUID A, authority A
Repository B, UUID B, authority B
Repository C, UUID C, authority C
             |
             v
independent readers and federated Atlas projections
```

Cross-repository transfer works by reference and local admission:

```text
foreign Claim, Submission, Verification, or source observation
                     |
                     v
exact retained reference and loss report
                     |
                     v
new local Proposal
                     |
                     v
local Verification and local Decision
                     |
                     v
local Standing
```

No foreign Standing is imported automatically. Agreement between authorities is evidence of agreement, not global consensus.

### 16.6 Local-first without local-first authority

Vela should adopt local-first properties where they improve custody and continuation:

- complete local copies;
- offline reads and replay;
- provider-independent identity;
- ordinary branches and forks;
- local execution;
- replaceable servers;
- independently retained releases;
- explicit sync and failover.

It should not adopt optimistic merge as the semantics of scientific authority. Accepted transitions remain exact compare-and-swap operations against one named authority state.

### 16.7 Large artifacts

Git should keep:

- canonical small Vela objects;
- manifests;
- source locks;
- critical small evidence;
- exact external identifiers and digests;
- enough metadata to retrieve and validate larger content.

Large public or controlled bytes may live in:

- DataLad/git-annex;
- DVC plus a suitable remote;
- OCI registries;
- S3-compatible object stores;
- institutional data repositories;
- domain archives;
- Zarr, Parquet, HDF5, or other native formats.

Every external artifact reference should state:

```text
identity and full digest
media type and size
retrieval locators
rights and access class
mirror or archive status
last successful retrieval where useful
reproduction impact if unavailable
```

A digest proves bytes after retrieval. It does not provide availability or rights.

---

## 17. Build-versus-adopt decision framework

### 17.1 The default answer is adopt

For any proposed component, ask these questions in order.

1. Does a maintained standard already define the wire representation?
2. Does a maintained product already perform the operational job?
3. Can Vela integrate through exact files, CLI calls, APIs, or attestations?
4. Is the missing requirement genuinely scientific-state semantics rather than convenience?
5. Does building the component create a new writer, secret, service, registry, resolver, scheduler, or trust root?
6. Can a source-local adapter test the requirement before a shared subsystem exists?
7. Will promotion delete duplicated maintained code in at least two consumers?

Build only when the first three answers are no, the fourth is yes, and the last two provide evidence.

### 17.2 What Vela should build

Vela should build:

- the portable scientific Submission and Verification contracts;
- the scientific Proposal, Decision, Event, reducer, Standing, and correction model;
- exact read explanations such as `why` and correction impact;
- domain-adapter interfaces for semantic reference and diff evidence;
- conformance vectors and independent readers for Vela-specific semantics;
- a local operator CLI;
- a first-party read-only Observatory and Dossier product;
- a minimal provider-neutral catalog of authority repositories and source locators;
- source-local experiments where no standard profile yet expresses the needed scientific relationship.

### 17.3 What Vela should integrate

Vela should integrate:

- Git for storage and ancestry;
- GitHub, Codeberg, Forgejo, GitLab, Entire, or Radicle as optional hosts and replication layers;
- native languages and scientific systems for execution;
- existing package managers and registries;
- existing workflow and data-versioning tools;
- existing identity, workload identity, and authorization infrastructure;
- existing release, provenance, SBOM, and advisory standards;
- existing archive and research-object standards;
- PostgreSQL for read projection;
- object stores and OCI for large artifacts;
- OpenAPI, JSON Schema, and Problem Details for hosted read interfaces;
- Web Annotation, PROV, RO-Crate, and OpenLineage for references and derivation.

### 17.4 What Vela should index

Indexing is appropriate when Vela adds scientific navigation without taking over distribution or authority.

Vela may index:

- native scientific sources;
- Claims and their current local Standing;
- exact external packages and software versions;
- source observations;
- Problems;
- Verifications and nonclaims;
- Decisions and corrections;
- provenance routes;
- available artifacts and mirrors;
- independently operated authority repositories.

The index must always point back to the native source, exact root, or authority repository. It should not proxy every byte, assign universal names, or become a required resolver.

### 17.5 What Vela should never build by default

- a replacement for Git;
- a GitHub clone;
- a multi-master forge;
- a universal data lake;
- an agent scheduler;
- a general workflow runtime;
- a universal notebook;
- a collaborative editor;
- a model hub;
- a package manager;
- a hosted package registry;
- an identity provider;
- a universal authorization server;
- a custom object store;
- a custom SBOM, advisory, provenance, archive, or dataset standard;
- a universal ontology;
- a global truth graph;
- a blockchain or token system;
- a central hosted scientific writer.

---

## 18. Migration and execution sequence

The migration should be one coherent simplification program, not a permanent compatibility layer.

### Phase 0: restore ecosystem truth immediately

1. Update `vela-web` to the current Vela 0.970.0 release and current mathematics repository identity.
2. Rebuild and verify the projection from the current `math` head.
3. Regenerate `ecosystem-status.json` from current repository state.
4. Replace the stale organization profile with the four-repository topology.
5. Correct current README and architecture prose that still says a Frontier is a Repository.
6. Add CI that fails when the generated catalog, Web registry, release pin, repository profile, and observed GitHub state disagree.
7. Finish the provider-loss drill steps for local Decision and projection rebuild.

This phase changes no protocol semantics.

### Phase 1: run the authority deletion experiments

Run two concrete implementations against the same fixture repository and scientific loop.

#### Candidate A: simplified native Vela authority

- UUIDv4 repository identity;
- one closed AuthZEN-shaped authority model;
- DSSE-signed scientific Decision;
- exact before and after Vela roots;
- local Git compare-and-swap;
- one authority history sufficient for replay;
- no Cedar;
- no generic policy language;
- no duplicate nested signature.

#### Candidate B: Vela scientific Decision plus gittuf

- Vela DSSE Decision binds Proposal and scientific before/after roots;
- gittuf owns generic repository root of trust, key lifecycle, path/ref policy, approvals, and reference-state history;
- strict Vela replay checks the scientific objects and Decisions;
- gittuf verification checks the Git publication path;
- no overlapping Vela generic repository policy or key-distribution layer.

Measure:

- maintained lines deleted;
- number of canonical objects and roots;
- independent-reader burden;
- clean-clone verification time;
- ability to verify without a forge;
- recovery and key-rotation behavior;
- transaction race resistance;
- policy expressiveness actually needed;
- beta dependency risk;
- migration cost.

Choose one. Do not ship both as permanent current paths.

Because gittuf is currently beta, adoption requires a stronger burden than conceptual overlap. If it cannot become the sole owner of the generic Git-security role, keep it optional and ship the simplified native Vela path.

### Phase 2: make one final pre-1.0 wire cut

Bundle the breaking changes that otherwise each force another re-genesis.

1. Migrate `repository_id` to RFC 9562 UUIDv4.
2. Simplify origin and remove unused compaction machinery if the audit confirms no current need.
3. Move portable signed objects to one DSSE implementation and versioned payload types.
4. Make full roots canonical and demote removable short self-identities to derived handles.
5. Generate JSON Schema 2020-12 from the Rust source of truth and verify it against independent emitters and readers.
6. Select one authorization architecture from Phase 1.
7. Re-genesis or explicitly migrate `math` once under the selected contract.
8. Re-admit current valuable state through exact, reviewable transitions where required.
9. Preserve predecessor Git tags, bundles, release binaries, and migration attestations.
10. Delete old current writers, aliases, policy engines, identifier code, and compatibility paths immediately after verification.

Do not make another break merely to rename prose. Make this cut only for structural simplification and standards alignment.

### Phase 3: repair projection vocabulary and ownership

1. Rename `frontier_slug` to `repository_slug` or, preferably, key canonical projection joins by repository UUID and use a separate human route slug.
2. Rename `frontier-data` to `observatory-data` or `projection-data`.
3. Keep Frontier views as saved, derived queries with no identifier.
4. Rebuild row roots once under the corrected schema.
5. Preserve old deployed releases as historical projections rather than supporting both column vocabularies forever.
6. Keep raw SQL and immutable migrations.
7. Generate a clean-room schema and verify byte-identical or root-equivalent projection results under declared rules.

### Phase 4: implement the missing science-translation profiles

In `vela-science/math`, select one exact source-to-formalization case and produce:

1. a Web Annotation based Reference profile;
2. a deterministic Lean or Claim Semantic Diff;
3. a PROV/Workflow Run RO-Crate derivation record;
4. an explicit semantic loss report;
5. one root-bound Result Dossier in `vela-web`.

Measure the cold-reader baseline before adding the new reader.

Do not add a package repository, registry, universal pointer service, graph database, or protocol object.

### Phase 5: prove external infrastructure value

The defining external proof should include:

- one producer or workbench Vela does not control;
- one independently implemented reader or verifier;
- one separately governed authority Repository;
- one cross-repository transfer that preserves source provenance and requires a local Decision;
- one correction cascade;
- one provider-loss continuation;
- one cold successor who identifies the current state, decisive evidence, limitations, and next valid work without private maintainer explanation.

This is the threshold between an internally rigorous system and infrastructure for science.

---

## 19. Acceptance tests and falsification

### 19.1 The no-reimplementation test

For every Vela-owned subsystem, reviewers must be able to answer:

```text
Which existing standards and products were evaluated?
Why can they not satisfy the requirement through an adapter?
What uniquely scientific semantic remains?
Which old code or concept does the new subsystem delete?
```

A missing answer blocks promotion.

### 19.2 Independent wire conformance

At least two implementations outside the primary Rust code path must agree on:

- RFC 8785 bytes;
- full roots;
- DSSE PAE and signature verification;
- JSON Schema structure;
- Submission and Verification semantics required for reading;
- Decision and Event interpretation;
- deterministic Standing for the conformance repository.

They must not import the Rust implementation or generated expected output from the writer under test.

### 19.3 Authority verification

The chosen authority architecture must prove:

- independently obtained trust root;
- member and key rotation;
- wrong-authority rejection;
- stale-base rejection;
- exact Proposal and before/after-root binding;
- changed read-set rejection;
- changed intent rejection;
- no agent access to human authority credentials;
- no forge requirement for verification;
- no verifier pass becoming a Decision;
- no valid Git commit becoming accepted state without the Vela Decision path.

If gittuf is used, verify both layers independently:

```text
gittuf: was the Git ref transition policy-compliant?
Vela:   was the scientific transition validly decided and reduced?
```

Neither answer substitutes for the other.

### 19.4 Provider-loss test

With GitHub unavailable:

1. install Vela from independently retained signed assets;
2. retrieve `vela` and `math` from a mirror or bundle;
3. obtain the authority trust root through a separate channel;
4. run strict replay and reproduction;
5. create a Submission and Verification Record;
6. make one authorized local Decision;
7. publish to a promoted writer or retain the local canonical commit under the runbook;
8. rebuild the PostgreSQL projection;
9. compare repository and projection roots;
10. serve or export a usable Dossier without the old provider.

The test passes only when it is an exercised artifact, not a plausible runbook.

### 19.5 Repository identity test

- Clones and mirrors of one Repository carry one UUID.
- A fork that preserves history but intentionally creates a new authority receives a new UUID and trust root.
- Changing host, owner, repository name, or primary remote does not change UUID or Standing.
- Re-genesis or origin generation does not silently mint a new repository UUID unless the operator is intentionally creating a new authority lineage.
- Security checks never rely on UUID alone.

### 19.6 Projection reconstruction test

From retained canonical repositories and source locks, with Neon and Vercel absent:

- apply schema and immutable migrations to clean PostgreSQL;
- build the candidate release;
- verify every table and row root;
- activate only after verification;
- reproduce the same normalized root under the same projector version;
- explain any nondeterministic wall-clock fields as outside release identity;
- prove the Web application has no scientific write authority.

### 19.7 Semantic-reference test

For each Reference annotation:

- resolve the exact subject root;
- report exact, approximate, ambiguous, or unresolved precision;
- preserve selector and resolver identity;
- fail visibly rather than attach to the wrong subobject;
- test correspondence across at least one real revision;
- retain the original artifact bytes.

### 19.8 Semantic-diff test

Against expert review, measure:

- recall of scientifically material changes;
- false consequence flags;
- unsupported and ambiguous rate;
- review time versus raw Git diff;
- deterministic agreement across runs;
- whether the diff changed a review decision or found a missed caveat.

A generated prose summary is not part of the deterministic fact set.

### 19.9 Correction-impact test

In a controlled case, correct or withdraw one accepted Claim and measure:

- affected Claims and artifacts correctly identified;
- unaffected routes preserved;
- false stale flags;
- missed dependencies;
- required re-verification;
- surviving evidence routes;
- minimum repair set;
- declared incompleteness of the derivation graph.

### 19.10 Promotion and deletion test

A source-local experiment becomes shared only when:

```text
two maintained consumers
+ one exact shared contract
+ independent agreement
+ measurable review or continuation benefit
+ more maintained duplication deleted than abstraction added
```

A new service additionally requires a real operational need that static files, Git releases, existing registries, or an existing provider cannot meet.

### 19.11 Cold inheritance test

Give a new researcher or agent only the public repository, reader, trust root, and documented tools. Measure time and errors for:

- finding current Standing;
- locating decisive evidence;
- explaining verifier scope and nonclaims;
- understanding the Decision;
- identifying corrections;
- locating exact native source;
- reproducing the result;
- identifying the next valid action;
- continuing with a different workbench.

The target metric is inherited scientific agency, not record count.

### 19.12 Kill criteria

Stop or delete a program when:

- a mature existing product performs the job adequately;
- the Vela implementation adds a second resolver, registry, writer, or source of truth;
- no second maintained consumer appears;
- the abstraction increases maintained code rather than deleting duplication;
- a hosted service becomes necessary for replay;
- scientific meaning is silently inferred from transport, verification, popularity, or graph proximity;
- the user cannot distinguish current state from a hypothetical projection;
- the provider-loss drill depends on the provider being tested;
- the feature does not improve review, correction, or continuation in a measured case.

---

## 20. Risks and safeguards

### 20.1 Risk: overcorrecting into a generic standards wrapper

Using standards does not mean Vela should disappear into generic provenance and signing formats. PROV does not express scientific Standing. DSSE does not express scientific admission. Git does not express correction-aware scientific state.

**Safeguard:** retain explicit Vela payload semantics and reducer rules where they answer the five control-point questions.

### 20.2 Risk: importing an immature dependency into the trust core

Gittuf overlaps strongly with generic repository governance, but it is currently beta.

**Safeguard:** make the pilot deletion-oriented and isolated. Do not depend on it for Vela 1.0 unless it can be the sole owner of its role, passes Vela's adversarial fixtures, and has an acceptable stability and maintenance posture.

### 20.3 Risk: another identity migration with little product value

Replacing `vrepo_` with UUID creates a real pre-1.0 migration cost.

**Safeguard:** make the change only as part of the final standards cut, and only because it removes custom generation, expands collision margin, uses standard libraries and URNs, and simplifies external integrations. Do not change the identifier again after 1.0.

### 20.4 Risk: replacing one ceremony with another standards stack

A standards list can become architecture theater.

**Safeguard:** adopt a standard only where an actual producer, consumer, archive, verifier, or operational path uses it. No empty standards layer, registry, or export should ship merely to claim compliance.

### 20.5 Risk: raw SQL becoming scattered and unsafe

Raw SQL is correct for the projection, but unbounded query strings across application routes would create drift and injection risk.

**Safeguard:** keep all SQL inside one typed projection package, use parameterized queries, validate every returned row, expose semantic read functions, retain query budgets, and prohibit application-local database access.

### 20.6 Risk: the Atlas becomes the record

Rich graphs and Dossiers can look more authoritative than the repositories they project.

**Safeguard:** every view names exact source roots, projector version, freshness, coverage, inference class, and authority effect. Deleting the database must not lose Standing.

### 20.7 Risk: authority and publication collapse

A forge rule, signed commit, gittuf verification, or release attestation can be mistaken for a scientific Decision.

**Safeguard:** present publication integrity, verification outcome, Proposal status, Claim Standing, and repository integrity as separate axes in CLI and Web surfaces.

### 20.8 Risk: one mathematics repository becomes a universal mathematics authority

The current `math` repository is one authority, not all mathematics.

**Safeguard:** state the authority, scope, maintainer set, policy, coverage, and omissions explicitly. Make independent mathematics repositories first-class in the future Atlas. Do not imply consensus from one Repository.

### 20.9 Risk: fake federation

Creating a second Vela-controlled repository with the same operator and policy does not prove authority containment.

**Safeguard:** require genuinely separate governance, independent trust roots, and a real capacity to disagree.

### 20.10 Risk: standard identifiers become false equivalence

A DOI, SWHID, PURL, ORCID, ROR, or Web Annotation target identifies something. It does not establish that two scientific interpretations are equivalent.

**Safeguard:** mappings carry exact source versions, relation type, method, precision, loss, and authority effect.

### 20.11 Risk: large-byte references become dead links

A content digest without retained bytes is not inheritance.

**Safeguard:** monitor availability, mirror counts, rights, retrieval health, and reproduction impact. Create bounded reproducibility bundles for decisive evidence.

### 20.12 Risk: permanent pre-1.0 archaeology

Compatibility code can outlive every consumer.

**Safeguard:** historical tags and binaries read historical epochs. The current runtime reads one current contract. Migrations end with deletion.

---

## 21. Final recommendation

Vela should be built as a **small semantic kernel inside a large existing ecosystem**.

The ecosystem already has:

- Git for exact byte history and distribution;
- forges for collaboration;
- workbenches and agents for activity;
- proof assistants and scientific software for native verification;
- package managers and registries for software distribution;
- workflow engines for execution;
- data-versioning systems and object stores for large bytes;
- identity systems for people and workloads;
- authorization systems for access decisions;
- standards for signatures, schemas, provenance, research objects, SBOMs, vulnerabilities, source identity, and publication identity;
- PostgreSQL and search systems for derived reads.

Vela should not compete with them.

The missing layer is narrower:

```text
exact proposed scientific change
+ exact evidence and caveats
+ scoped independent checks
+ named local authority
+ authorized Decision
+ append-only correction
+ deterministic Standing
+ inherited next work
```

That is enough to justify Vela as a protocol and product. It is not enough to justify rebuilding every layer around it.

The ideal strategic posture is:

```text
use Git, do not remake Git
use native tools, do not remake the workbench
use native registries, do not remake package distribution
use standards, do not restate them
use existing identity and policy systems at their proper boundary
use Postgres as a disposable projection, not a canonical database
build only the scientific admission, correction, and inheritance gap
```

### The immediate decisions

1. Correct current ecosystem drift across `math`, `vela-web`, `ecosystem-status`, public docs, and the organization profile.
2. Keep Git and the one-authority-per-repository rule.
3. Keep Frontier derived and identifier-free.
4. Replace `vrepo_` with UUIDv4 in the final pre-1.0 cut, while retaining full roots and independent trust as security identities.
5. Replace bespoke signing with DSSE and external shape prose with generated JSON Schema.
6. Remove Cedar after one replacement architecture is selected and proven.
7. Pilot gittuf to delete generic repository-security machinery, but do not place a beta dependency in the core merely because the concepts overlap.
8. Keep raw SQL for the scientific projection and Drizzle only for a future separate product database.
9. Close the package-manager and hosted-registry question. Use native ecosystems.
10. Build the Reference, Semantic Diff, Derivation, correction-impact, and Dossier capabilities only as standards-based, source-local experiments first.
11. Finish provider-loss continuation and prove a genuinely independent second authority.
12. Make deletion, independent reading, and cold scientific continuation the gates for every new abstraction.

### The compact thesis

```text
Work happens anywhere.
Git and native systems preserve the work.
Standards carry identity, evidence, and provenance.
Vela admits scientific state deliberately.
Corrections remain visible.
Readers remain replaceable.
The next person or agent inherits an exact, explainable starting point.
```

Vela's long-term leverage does not come from owning every tool scientists touch. It comes from becoming the trusted boundary that lets work survive the tool, agent, institution, repository host, and original maintainer that produced it.

---

# Appendix A. Current observed repository snapshot and immediate drift

## A.1 Reviewed heads

| Repository | Reviewed head | Role |
| --- | --- | --- |
| `vela-science/vela` | `1ecbd15c119598bfb776acf87d6497addbc87055` | Protocol, CLI, schemas, conformance, release |
| `vela-science/math` | `a0bfdcd7d99f4bae7351713125d72fb01854f90e` | Current mathematics authority |
| `vela-science/vela-web` | `18d72a37222b174680208ef36f201e455e359518` | Editorial site and read-only Observatory |
| `vela-science/.github` | current `main` profile | Organization presentation |

## A.2 Current mathematics identity

At the reviewed `math` head:

```text
repository_id  vrepo_8348fae157f9c447
origin_id      vro_d7629ccd39c25ff4
origin kind    genesis
origin reason  Re-genesis at 0.970.0
accepted       1 Claim
proposals      3
submissions    3
verifications  6
```

The recent Git history visibly contains Submit, Verification, accept, and reject commits. This is real current state, not an empty template.

## A.3 Current stale consumers

At the reviewed `vela-web` head:

```text
required Vela release  0.969.0
repository_id           vrepo_56d3fdfcd34ff5c3
```

The generated `ecosystem-status.json` and organization profile also describe predecessor topology or state.

## A.4 Required correction

Treat this as a priority correctness defect:

```text
current authority changed
        |
        v
registry and release pin stale
        |
        v
projection can read the wrong epoch or fail
        |
        v
public product describes obsolete state
```

The repair should include data rebuild, declaration regeneration, and a recurring cross-repository check so the same class cannot recur silently.

---

# Appendix B. Component disposition table

| Component or proposal | Disposition | Owner or replacement |
| --- | --- | --- |
| Git repository and commits | Retain | Git |
| GitHub primary collaboration | Retain as default, not root | GitHub |
| Codeberg mirrors | Retain and exercise | Codeberg plus mirror workflow |
| Signed Git bundles | Add | Git plus release/distribution signing |
| Vela protocol kernel | Retain and narrow | `vela-science/vela` |
| `vela` CLI | Retain | `vela-science/vela` |
| Scientific Repository | Retain | each independent authority |
| Topic-specific authority repositories | Reject | derived Frontiers and Sources |
| `vrepo_` custom identity | Replace | RFC 9562 UUIDv4 |
| Full SHA-256 roots | Retain | Vela schemas and JCS |
| Short object IDs | Derive where possible | reader/UI |
| Custom canonical JSON | Replace | RFC 8785 JCS |
| Bespoke portable signatures | Replace | DSSE |
| Cedar policy runtime | Remove after migration | closed AuthZEN-shaped evaluator or proven gittuf layer |
| Gittuf | Pilot, optional until mature | external Git security layer |
| Vela scientific Decision | Retain | Vela |
| Generic Git path/ref governance | Prefer external | forge rules or gittuf |
| Custom repository authority chain | Simplify; delete overlapping generic parts | Vela semantic Decision plus selected Git policy layer |
| PostgreSQL projection | Retain | `vela-web` |
| Raw SQL migrations and queries | Retain and centralize | projection package |
| Drizzle in scientific projection | Reject | raw SQL |
| Drizzle in future product DB | Permitted | separate product domain |
| Neon | Retain as current hosted Postgres | replaceable deployment |
| Vela Web scientific writer | Reject | local Repository authority only |
| Observatory | Retain read-only | `vela-web` |
| Global mutable Atlas database | Reject | root-bound projections |
| Package manager | Reject | native managers |
| Hosted Vela package registry | Reject on current evidence | native registries and optional index |
| Vela package coordinate/URI | Reject now | PURL and HTTPS when needed |
| Custom artifact store | Reject | OCI/ORAS, object storage, archives |
| Custom data versioning | Reject | DataLad, git-annex, DVC, native systems |
| Custom workflow engine | Reject | native workflow systems |
| Agent runner in Vela | Reject | external workbenches and agents |
| MCP | Edge adapter only | MCP implementations |
| A2A | Only for actual separate agent services | A2A implementations |
| Reference Map custom envelope | Replace with profile | W3C Web Annotation |
| Semantic Diff facts | Build source-locally | domain adapters over exact roots |
| Derivation Trace custom ontology | Replace with profile | PROV-O and Workflow Run RO-Crate |
| Custom research archive | Reject | RO-Crate plus ordinary transport |
| Custom SBOM | Reject | SPDX or CycloneDX |
| Custom vulnerability format | Reject | OSV |
| Custom source identifiers | Reject | SWHID, DOI, PURL, native IDs |
| Organization catalog | Add as generated non-authority artifact | `.github` or release automation |
| Archived epoch readers in current binary | Reject | historical binary and tags |
| Archived repositories | Preserve, do not develop | Git archives and historical releases |

---

# Appendix C. Reconciliation with prior Vela memos

## C.1 Durable conclusions retained

Across the prior architecture, package, Git outage, standards, consolidation, and Ink & Switch memos, the following conclusions remain correct:

- Git is the custody substrate, not scientific authority.
- Vela's leverage is the narrow transition from evidence into admitted state.
- Production, verification, and admission must remain separate.
- Hosted projections must remain read-only and disposable.
- Native scientific systems remain sovereign.
- One active writer plus independent read replicas is the correct default.
- Webhooks are notifications, not the source of truth.
- Rich activity histories belong outside canonical scientific state.
- Domain semantic diff is more valuable than a replacement VCS.
- Exact references and provenance routes are core science-translation infrastructure.
- Package, Registry, Atlas, and service layers must be earned.
- A second maintained consumer and net deletion are the extraction gate.
- External recurrence, independent readers, independent authority, correction, and cold inheritance matter more than more internal primitives.

## C.2 Conclusions superseded by current repository state

The following older conclusions are superseded or narrowed:

- Frontier is no longer the repository authority object.
- Topic-specific Frontier repositories are no longer the target topology.
- Canopus is no longer a current product component.
- `vela-internal` is not an active integration layer.
- A Vela package manager and registry are not current destinations.
- A separate Atlas application is unnecessary; the Observatory is the first-party projection.
- Carina and other earlier layer names should not be revived without current implementation and a demonstrated boundary.
- Current product documentation must not use historical topology as current architecture.

## C.3 Ink & Switch conclusion retained

The uploaded `ink-and-switch-vela-research-memo.md` has one strongest principle that remains the governing product rule:

```text
record broadly
resolve semantically
admit narrowly
replay exactly
```

Its recommendation to preserve the narrow Vela object model while experimenting with Reference, Semantic Diff, and Derivation capabilities remains correct. This memo tightens the implementation by preferring Web Annotation, PROV, Workflow Run RO-Crate, and native domain tooling over new Vela-wide formats.

---

# Appendix D. Standards and infrastructure references

## D.1 Current Vela repositories

- [Vela repository](https://github.com/vela-science/vela)
- [Vela 0.970.0 reviewed commit](https://github.com/vela-science/vela/commit/1ecbd15c119598bfb776acf87d6497addbc87055)
- [ADR 0039: Repository authority and derived Frontiers](https://github.com/vela-science/vela/blob/main/docs/adr/0039-repository-authority-and-derived-frontiers.md)
- [ADR 0035: Commodity encoding, signing, and wire contracts](https://github.com/vela-science/vela/blob/main/docs/adr/0035-commodity-encoding-signing-and-wire-contracts.md)
- [Vela protocol](https://github.com/vela-science/vela/blob/main/docs/PROTOCOL.md)
- [Vela ecosystem](https://github.com/vela-science/vela/blob/main/docs/ECOSYSTEM.md)
- [Repository profile](https://github.com/vela-science/vela/blob/main/docs/REPOSITORY_PROFILE.md)
- [Root taxonomy](https://github.com/vela-science/vela/blob/main/docs/ROOTS.md)
- [Interoperability](https://github.com/vela-science/vela/blob/main/docs/INTEROPERABILITY.md)
- [Continuity](https://github.com/vela-science/vela/blob/main/docs/CONTINUITY.md)
- [Repository ownership boundaries](https://github.com/vela-science/vela/blob/main/docs/REPOSITORY_BOUNDARIES.md)
- [Vela Mathematics](https://github.com/vela-science/math)
- [Vela Mathematics reviewed commit](https://github.com/vela-science/math/commit/a0bfdcd7d99f4bae7351713125d72fb01854f90e)
- [Vela Web](https://github.com/vela-science/vela-web)
- [Vela Web reviewed commit](https://github.com/vela-science/vela-web/commit/18d72a37222b174680208ef36f201e455e359518)

## D.2 Git and Git security

- [Git documentation](https://git-scm.com/docs/git)
- [Git repository layout](https://git-scm.com/docs/gitrepository-layout)
- [Git bundles](https://git-scm.com/docs/git-bundle)
- [Git signature format](https://git-scm.com/docs/gitformat-signature)
- [gittuf](https://gittuf.dev/)
- [gittuf repository](https://github.com/gittuf/gittuf)

## D.3 Identity, encoding, schemas, and signatures

- [RFC 9562: UUIDs](https://www.rfc-editor.org/rfc/rfc9562.html)
- [RFC 8785: JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785.html)
- [JSON Schema 2020-12](https://json-schema.org/draft/2020-12)
- [DSSE](https://github.com/secure-systems-lab/dsse)
- [OpenID AuthZEN Authorization API 1.0](https://openid.net/specs/authorization-api-1_0.html)

## D.4 Supply chain, releases, and artifacts

- [in-toto Attestation Framework](https://github.com/in-toto/attestation)
- [SLSA provenance 1.2](https://slsa.dev/spec/v1.2/provenance)
- [Sigstore bundles](https://docs.sigstore.dev/about/bundle/)
- [The Update Framework](https://theupdateframework.github.io/specification/latest/)
- [SCITT architecture, RFC 9943](https://www.rfc-editor.org/rfc/rfc9943.html)
- [OCI Image and Distribution 1.1](https://opencontainers.org/posts/blog/2024-03-13-image-and-distribution-1-1/)
- [ORAS](https://oras.land/)
- [SPDX 3.0.1](https://spdx.github.io/spdx-spec/)
- [OSV schema](https://ossf.github.io/osv-schema/)
- [Package URL, ECMA-427](https://ecma-international.org/publications-and-standards/standards/ecma-427/)
- [Software Heritage persistent identifiers](https://www.softwareheritage.org/software-hash-identifier-swhid/)

## D.5 Research objects, provenance, references, and identity

- [RO-Crate 1.3](https://www.researchobject.org/ro-crate/specification/1.3/index.html)
- [Workflow Run RO-Crate](https://www.researchobject.org/workflow-run-crate/profiles/workflow_run_crate/)
- [W3C PROV-O](https://www.w3.org/TR/prov-o/)
- [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
- [OpenLineage object model](https://openlineage.io/docs/spec/object-model)
- [DataCite Metadata Schema 4.7](https://datacite-metadata-schema.readthedocs.io/en/4.7/)
- [ORCID](https://info.orcid.org/)
- [ROR](https://ror.org/registry/)

## D.6 Data, execution, APIs, and operations

- [DataLad](https://docs.datalad.org/)
- [git-annex](https://git-annex.branchable.com/)
- [DVC](https://dvc.org/)
- [OpenAPI 3.2](https://spec.openapis.org/oas/v3.2.0.html)
- [RFC 9457: Problem Details for HTTP APIs](https://www.rfc-editor.org/rfc/rfc9457.html)
- [OpenTelemetry](https://opentelemetry.io/docs/specs/)
- [SPIFFE](https://spiffe.io/docs/latest/spiffe-about/overview/)

---

# Appendix E. Pre-1.0 architecture checklist

Before Vela 1.0, require explicit answers to the following.

## Identity and roots

- Is Repository identity standard and stable across hosts?
- Does every security comparison use a full root rather than a short handle?
- Can every derived handle collision fail visibly?
- Is origin simpler than the pre-release migration history that produced it?

## Authority

- Is there exactly one current authorization implementation?
- Is generic Git governance separated from scientific Decision semantics?
- Can a clean independent reader verify every current Decision?
- Can authority continue without GitHub?
- Can agents never reach human authority credentials?

## Wire contracts

- Are canonical bytes RFC 8785?
- Are portable signed objects DSSE?
- Are schemas JSON Schema 2020-12 and generated or checked from one source of truth?
- Are read surfaces additive and tolerant while signed objects remain closed?

## Ecosystem ownership

- Does each active repository have one responsibility?
- Are archived repositories absent from current product topology?
- Is cross-repository status generated rather than copied by hand?
- Is every public page aligned with current repository state?

## Existing infrastructure

- Is Vela using Git rather than extending into a new VCS?
- Is it using native package managers and registries?
- Is it using existing artifact, workflow, data, identity, provenance, and supply-chain systems?
- Does every remaining custom subsystem answer a uniquely Vela scientific question?

## Projection

- Can the entire projection be rebuilt from exact retained state?
- Does the application have no scientific signer or writer?
- Is raw SQL centralized and typed at the boundary?
- Are Frontier views derived queries rather than stored authorities?

## Product proof

- Has one external producer emitted a portable object?
- Has one independent reader agreed on roots and Standing?
- Has one genuinely independent authority participated?
- Has one correction cascade been understood and repaired?
- Has provider loss been exercised end to end?
- Has a cold successor continued the science without private context?

A negative answer does not always block 1.0. It must, however, be stated as a limitation rather than hidden behind more architecture.
