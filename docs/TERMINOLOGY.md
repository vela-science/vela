# Vela terminology

This is the controlled product language for Vela's current pre-1.0 writer.
Historical object names remain valid when describing their exact source era.

## Product contract

> **Vela is version control for living science.**

> **Map the frontier. Target what matters. Run the work. Verify the result.
> Commit what stands. Compound every gain.**

```text
map -> target -> run -> verify -> commit -> compound
```

These are product verbs, not renamed protocol objects:

```text
map       reads exact state, dependencies, disagreement, and gaps
target    selects bounded work from current state
run       creates activity and evidence
verify    records a scoped check over exact inputs
commit    summarizes an authorized Decision + Event + root transition
compound  reuses the new state, correction, or retained failed route
```

`Frontier Commit` is a product-level description of an authorized transition.
It is not a protocol object, Git merge, verifier result, package publication,
or new authority path.

The architecture ownership mnemonic remains:

```text
produce -> preserve -> check -> decide -> reuse
```

## Canonical current vocabulary

| Term | Meaning | Authority effect |
| --- | --- | ---: |
| Frontier | One independently clonable Git repository with bounded scope, stable identity, canonical history, authority, and correction policy | Boundary only |
| Frontier map | An exact, removable read projection of current territory, coverage, uncertainty, retained work, and next valid action | None |
| Problem | A bounded scientific question organizing Claims, Obligations, Targets, and native runs | None |
| Obligation | One unresolved requirement needed to assess or advance a Claim | None |
| Target | One machine-addressable, bounded unit of work | None |
| Offer | A derived recommendation that a Target is available | None |
| Attempt | Optional provenance name for an effort retained by a native workbench; Vela does not create or govern it | None |
| Run | One execution occurrence in a workbench or verifier | None |
| Claim | One exact assertion under explicit scope and conditions | None |
| Claim Record | Canonical record of a Claim revision, conditions, evidence, provenance, and typed relations | None by itself |
| Finding | Editorial label for a Claim with positive standing in a named Frontier | View only |
| Artifact | Retained bytes or an immutable locator with exact identity and provenance | None |
| Evidence | A typed role played by an Artifact or Observation relative to a Claim | None |
| Submission | The portable producer package offered to a Frontier | None |
| Proposal | An exact candidate transition in Frontier standing | None |
| Verification Record | A verifier's scoped observation over exact inputs under a named method | None |
| Review Packet | A derived, root-bound presentation of one Proposal | None |
| Decision | An authorized judgment over one exact Proposal | Defines transition intent |
| Event | The append-only canonical transition record | Replays authorized effect |
| Frontier Commit | Product term for an authorized Decision, canonical Event, exact before/after roots, and replayed Standing | No independent effect |
| Standing | The deterministic current status derived from valid Events | Resulting state |
| Observatory | A removable read-only projection | None |

## Daily commands

```text
init status next start submit show why review check reproduce log doctor
```

| Command | Meaning |
| --- | --- |
| `status` | Summarize Frontier identity, integrity, blockers, counts, authority readiness, and one next action |
| `next` | Return ranked Target Offers |
| `start` | Print a write-free briefing for one exact current Target |
| `submit` | Validate and retain a Submission, then create its Proposal |
| `show` | Inspect one current or historical typed object |
| `why` | Explain current standing from exact evidence, verification, Decisions, Events, and corrections |
| `review accept` | Accept one exact Proposal through repository authority |
| `review reject` | Reject one exact Proposal through repository authority |
| `check` | Validate schemas, roots, signatures, replay, and policy consistency |
| `reproduce` | Rerun retained evidence or a verifier from exact or declared inputs |
| `log` | Read canonical history |
| `doctor` | Diagnose operational blockers and show one repair action |

## Required distinctions

```text
submit != accept
submit != verify
verify != accept
publish != accept
Git merge != scientific Decision
reproduce != endorse
correct != erase
```

A successful Submission normally means:

```text
Submission retained; review required.
Accepted scientific state changed: no.
```

A passing Verification Record must name the exact property, method, inputs,
scope, and nonclaims. It changes no standing by itself.

A current correction, supersession, or retraction is a new authenticated
Submission whose requested change binds the exact historical Claim ID and full
Finding root. It never edits the prior Submission, Event, Finding, or Decision.

An acceptance must name the exact Proposal, authorized human principal,
Frontier repository authority, Event, and before/after state roots.

## Lifecycle vocabularies

A native workbench may supply its own run or attempt identity as provenance.
That lifecycle remains defined by the source workbench and never becomes Vela
Standing.

Submission:

```text
draft retained refused
```

Proposal:

```text
pending_review accepted rejected
```

Verification outcome:

```text
pass fail inconclusive error
```

Claim standing:

```text
unassessed accepted accepted_with_conditions retracted superseded corrected
```

Artifact axes remain separate:

```text
availability: available | unavailable | restricted | unknown
integrity: matched | mismatched | not_checked
locator: immutable | mutable | unavailable | opaque
```

## Predecessor provenance

Imported Claim Records name their exact predecessor object ID, full root,
source era, and Git commit. The current identity is derived from the current
Claim bytes; an old identity or signature is never copied into a current
authentication field.

All predecessor bytes remain retrievable through the epoch's tag, commit,
Git-object manifest, and archive digest. They are not current writer objects.

## Controlled verbs

Read:

```text
status show list why log
```

Work:

```text
next start run resume abandon
```

Evidence:

```text
submit import reproduce
```

Evaluation:

```text
check verify reproduce
```

Authority:

```text
accept reject correct supersede retract authorize revoke
```

Distribution and recovery:

```text
export publish repair restore rebuild compact
```

Lower-power roles never receive higher-power verbs.

## Product wording

Use:

- “accepted within this Frontier”;
- “Verification passed for the declared computational property”;
- “Claim fidelity has not been assessed”;
- “Proposal rejected; evidence retained”;
- “No accepted scientific-state change”;
- “published, not accepted”; and
- “producer-reported check”.

Avoid:

- `landed finding`;
- `verified truth`;
- `accepted by verifier`;
- `AI approved`;
- `receipt accepted`;
- `confidence score` as standing;
- `immutable truth`;
- `global knowledge graph`; and
- any unqualified use of `verified`, `valid`, `approved`, or `complete`.

## Analysis and interoperability

These are read, package, and adapter concepts. They do not enter the current
writer merely because they are useful:

| Term | Meaning | Authority effect |
| --- | --- | --- |
| Frontier Algebra | A root-bound derivation of support/opposition routes, corrections, cut sets, and repair requirements | None |
| Discovery Calculus | Optional information and decision lenses for choosing research actions | None |
| Semantic package | A content-addressed set of terms, constraints, mappings, fixtures, licenses, and generated interoperability artifacts | None |
| Verification scope | The exact Claim, inputs, method, environment, and property covered by one Verification Record | None |
| Assurance profile | A versioned description of the assurance dimension addressed by a check, its prerequisites, and nonclaims | None |
| Independence disclosure | Retained common-dependency facts across producers or verifiers | None |
| Mapping | A versioned relation between exact package terms with a declared consequence tier | None by itself |
| Bridge | A maintained set of mappings between domains with every premise and scope needed for transport | None by itself |
| Adapter | A replaceable translation from an exact workbench export to a Submission and explicit loss report | None |
| Lens | A rooted view, metric, or action ordering under declared assumptions | None |
| Package | A versioned, content-addressed unit of reusable language, capability, corpus, verifier, mapping, or adapter; publication has no authority effect | None |
| Atlas | A future removable cross-Frontier navigation concept | None |

Mappings state one consequence tier. The default is `discovery`:

```text
discovery
organization
identity
logical_transport
empirical_transport
```

Shared labels, embeddings, graph proximity, `skos:exactMatch`, or
`owl:sameAs` never transport Standing by themselves.

## Epoch rule

> Verify the signed predecessor boundary. Write exactly one current epoch.
> Never rewrite canonical history to make vocabulary cleaner.

See [ADR 0033](adr/0033-direct-submission-lineage-and-registration-retirement.md)
for the current lifecycle and controlled rewrite gate.
