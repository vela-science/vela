# ADR 0047: Native repository integration is non-authoritative

- Status: Accepted, 2026-08-13
- Reconciles: the former human-only Decision wording in ADR 0045 with ADR 0046
- Reaffirms: ADR 0039, ADR 0046
- Protocol effect: None; Protocol 1 gains no object or transition
- Authority effect: None

Approved source documents:

- Canonical architecture SHA-256 `3ac5740763db46c2c64a0d2154c6ab464def2cd8371e265d16a9be083f374ead`
- Full execution plan SHA-256 `4e499ed9703560bf8f859a709d4e8f9265980e1a089a4e3fe1427583c6a0836f`

## Context

Native scientific repositories need a small, source-owned way to expose exact
objects, checks, relations, rights, availability, and semantic loss. Putting
that machinery in one scientific authority confuses participation with
admission and makes a field look like a single authority.

## Decision

Vela uses five planes:

```text
native science
  -> integration: Manifest -> Profile -> Binding -> Method
  -> portable scientific transitions: Exact References, Submissions,
     Verification Records
  -> zero or more independent local Vela Repository authorities

read plane: projections, Problems, maps, Dossiers, search, collections
activity plane: Workspaces, tasks, attempts, sessions, canvases
```

The activity plane coordinates native work and may produce exact candidate
records. It is parallel to the authority chain and has no Standing effect.

The terms are fixed through the first two maintained integrations:

- an Integration Manifest is the root `vela.toml` instance declaration;
- a Profile defines what conformance means;
- a Binding states how one repository satisfies or exposes one exact Profile;
- a Method states how one property is checked; and
- an Exact Reference keeps native identity, exact revision, content fixity,
  and optional selector distinct.

Every integration contract declares `authority_effect = "none"`. It may
produce Exact References, Submission drafts, or Verification inputs. It cannot
mint a Decision, Event, Repository identity, authority state, or Standing.
Submission and Verification remain portable and non-authoritative. Only an
authorized, attributed Decision changes one Repository's local Standing.

Authority is plural. The same native result may be considered by zero, one, or
many independent Vela Repositories, each with its own policy, corrections,
Decisions, Events, replay, and Standing. There is no global Standing.

Custody follows one rule: **reference broadly, snapshot selectively, admit
narrowly**. Reference, snapshot, Verification, and admission remain different
operations.

Mapping relation and translation disposition remain separate. Mapping uses
`exact`, `close`, `broader`, `narrower`, or `related`. Translation uses
`preserved`, `normalized`, `derived`, `approximated`, `omitted`, `unsupported`,
`assumed`, or `unresolved`.

Human and agent work use the same provenance requirements. A responsible Agent,
the Activity performed, model and tool Entities, and the Role played remain
separate. Actor kind supplies no evidentiary rank. This ADR adopts ADR 0046's
performer-neutral Decision rule; it does not weaken authorization, exact-root
checks, the Repository authority signature, or policy.

An ordinary native repository is not a Vela Authority Repository. Authority
exists only after a separate explicit initialization and policy act. Adding a
Manifest or `.vela/bindings` and `.vela/methods` never initializes authority.

DSSE and in-toto are optional future transport projections. No shared predicate
or Profile enters Core until two maintained consumers prove the same mapping
and extraction removes more code than it adds.

The reader-facing product centers one Problem projection over reviewed, rooted
source occurrences. Native evidence and reviews, Repository-specific authority,
provisional Workspace activity, and local-workbench execution stay distinct.
Exact roots, Profile and Binding health, replay, and resolver diagnostics use
progressive disclosure. A Claim joins a Problem only through a reviewed rooted
occurrence mapping, never an alias, string match, SQL proximity, or route
exception.

## Consequences

- Source repositories retain native science, execution, Profile drafts,
  Bindings, Methods, and governance.
- Vela Core retains Protocol objects, typed roots, exact verification,
  authority, replay, and only qualified reusable integration machinery.
- A Vela Repository remains the sole local authority boundary.
- Workspaces remain mutable activity; projections remain rebuildable readers.
- Native build, CI, merge, approval, publication, signature, or a passing check
  never implies acceptance or Standing.
- Protocol 1 is unchanged by the Phase 0 and Phase 1 integration drafts.
