# Vela terminology

This is the controlled product language for Vela's current pre-1.0 writer.
Historical object names remain valid when describing their exact source era.

## Product contract

> **Vela is version control for scientific state.**

> **See what stands. Attempt what remains. Submit evidence. Verify what it
> proves. Decide what changes. Continue from the new state.**

```text
inspect -> attempt -> submit -> verify -> decide -> continue
```

The architecture ownership mnemonic is different:

```text
produce -> preserve -> check -> decide -> reuse
```

The first sequence describes user actions. The second describes which layer
owns each system responsibility.

## Canonical current vocabulary

| Term | Meaning | Authority effect |
| --- | --- | ---: |
| Frontier | One independently clonable Git repository with bounded scope, stable identity, canonical history, authority, and correction policy | Boundary only |
| Problem | A bounded scientific question organizing Claims, Obligations, Targets, and Attempts | None |
| Obligation | One unresolved requirement needed to assess or advance a Claim | None |
| Target | One machine-addressable, bounded unit of work | None |
| Offer | A derived recommendation that a Target is available | None |
| Attempt | One bounded effort against an exact Target | Coordination only |
| Run | One execution occurrence in a workbench or verifier | None |
| Claim | One exact assertion under explicit scope and conditions | None |
| Claim Record | Proposed canonical record of a Claim revision and its typed relations; separately gated and not yet writable | None |
| Finding | Editorial label for a Claim with positive standing in a named Frontier | View only |
| Artifact | Retained bytes or an immutable locator with exact identity and provenance | None |
| Evidence | A typed role played by an Artifact or Observation relative to a Claim | None |
| Submission | The portable producer package offered to a Frontier | None |
| Registration Record | Vela's record of exact Submission intake and routing | None |
| Proposal | An exact candidate transition in Frontier standing | None |
| Verification Record | A verifier's scoped observation over exact inputs under a named method | None |
| Review Packet | A derived, root-bound presentation of one Proposal | None |
| Decision | An authorized judgment over one exact Proposal | Defines transition intent |
| Event | The append-only canonical transition record | Replays authorized effect |
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
| `start` | Create or reuse an Attempt; no scientific-state change |
| `submit` | Validate and register a Submission, then route its Proposal |
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
register != verify
verify != accept
publish != accept
Git merge != scientific Decision
reproduce != endorse
correct != erase
```

A successful Submission normally means:

```text
Submission registered; review required.
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

Attempt:

```text
available active completed abandoned expired contaminated
```

Submission registration:

```text
draft submitted registered refused withdrawn
```

Proposal:

```text
pending_review accepted rejected withdrawn superseded
```

Verification outcome:

```text
pass fail inconclusive error unavailable not_run
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

## Historical source eras

Historical bytes are never renamed or rehashed:

| Historical object | Historical prefix | Current relationship |
| --- | --- | --- |
| Finding | `vf_` | Historical Claim/Finding record; never rewritten as a Claim Record |
| Receipt v1 | receipt root | Historical producer package; never relabeled as a Submission |
| Activity or landing record | `vrc_` | Historical intake/landing record; not a Registration Record |
| VerifierAttachment | `vva_` | Historical verifier evidence; not a Verification Record |

Current readers expose source schema, identifier, root, projection version,
and any semantic loss. Current writers emit only the current Submission,
Registration, and Verification era. The separately gated Claim Record writer
does not exist yet.

## Controlled verbs

Read:

```text
status show list diff why log
```

Work:

```text
next start run resume abandon
```

Intake:

```text
submit register import withdraw
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
export publish serve repair restore rebuild compact
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

## Compatibility rule

> Read every retained era that matters. Write exactly one current era. Never
> rewrite canonical history to make vocabulary cleaner.

See [ADR 0021](adr/0021-scientific-submission-and-direct-action-cli-language.md)
for the migration, conformance, and acceptance gates.
