# Vela terminology

This is the controlled product language for Vela's current pre-1.0 writer.
Historical object names remain valid when describing their exact source era.

## Product contract

> **Vela is version control for living science.**

> **Map the territory. Advance one boundary. Leave the next researcher ahead.**

```text
init -> submit -> verify -> decide -> replay
```

Research navigation wraps that exact operator loop:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

These navigation verbs are not renamed protocol objects:

```text
map       reads exact state, dependencies, disagreement, and gaps
target    selects bounded work from current state
work      happens in any native human or machine environment
submit    retains authenticated bounded evidence and a requested Claim
verify    records a scoped check over exact inputs
decide    lets named local authority accept, reject, or correct
remap     derives current territory, blockers, and next work from the new root
```

`MAP -> ADVANCE -> REMAP` is the shorter product compression. `Frontier
Commit` remains an optional product-level description of an authorized
Decision, Event, and root transition. It is not a protocol object, Git merge,
verifier result, package publication, or new authority path.

The operator loop remains:

```text
init -> submit -> verify -> decide -> replay
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

## Research and evaluation properties

These terms describe bounded system behavior. They are not protocol objects:

| Property | Meaning |
| --- | --- |
| action-complete | Every represented unresolved item yields a fresh exact Target or an explicit blocker, within declared source coverage, compiler, relation, and resource bounds |
| correction-closed | A declared complete relation slice identifies affected, surviving, and repair-required state after a correction |
| inheritance-complete | A cold successor can recover current Standing, decisive evidence, and the next valid action without private maintainer context |
| Frontier closure | State, coverage, Targets, Decisions, corrections, and handoff each close over exact current inputs or fail explicitly |
| compounding | Inherited Frontier state measurably improves later correct scientific work under a matched comparison |

The current **Math Atlas** is the bounded first-party read product over the four
maintained mathematical Frontiers. A future federated Atlas is an unearned
cross-Frontier concept, not a current global authority or completeness claim.

## Daily commands

```text
init status next start submit show why review replay reproduce log
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
| `replay` | Validate schemas, roots, signatures, replay, and policy consistency |
| `reproduce` | Rerun retained evidence or a verifier from exact or declared inputs |
| `log` | Read canonical history |

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

Submission, as this vocabulary declares it:

```text
draft retained refused
```

**No Submission has a status.** `vela.submission.v1` has no lifecycle field,
and none of those three words is a status value anywhere in the crates —
`retained` appears only as a display label on the unchanged-error footer. A
Submission is either installed in a repository or it is not. Treat the three
words as intent for a producer-side lifecycle nothing implements yet, and do
not render them.

Proposal:

```text
pending_review accepted rejected withdrawn
```

Proposal status is derived, not stored: `vela.proposal.v1` carries no status
field either, and the read surface computes these four from the covering
authority Events and any `vela.proposal-withdrawal.v1`. `withdrawn` is the
fourth because PROTOCOL.md section 5.5 makes producer-owned withdrawal a real
transition; `vela review list --status` accepts all four plus `all`, and three
of them are live in the Erdős repository today.

Verification outcome, which is a stored field and does match:

```text
pass fail inconclusive error
```

Claim standing, as this vocabulary declares it:

```text
unassessed accepted accepted_with_conditions retracted superseded corrected
```

**The shipped CLI does not emit that set, and a consumer must not assume it
does.** What `0.966.2` actually emits as a Claim's standing is:

```text
accepted pending_review rejected withdrawn superseded
```

Four declared values — `unassessed`, `accepted_with_conditions`, `retracted`,
`corrected` — appear in no line of any crate. `corrected` has a relation behind
it (`corrects`) but no standing derivation yet; `retracted` has none, because
retraction moves through the `claim.withdraw` Proposal action.
Two emitted values are not declared here at all, and both are Proposal-status
words: a Claim whose only Proposal is pending reports `pending_review`, and one
whose Proposal was withdrawn reports `withdrawn`. That is the axis separation
this document requires, crossed by the implementation.

Reconciling the two is a protocol decision, not a documentation fix, and it has
a downstream cost already being paid: the first consumer implemented the
declared vocabulary, so it maps every non-`accepted` standing onto `unassessed`
— a word nothing emits — and treats two live values as unreachable. Until the
decision is made, read the emitted set and treat the declared set as intent.

Claim relations, which are two vocabularies wearing one field name. The
correction algebra, closed and authoritative:

```text
corrects supersedes
```

Descriptive relations, which no Decision reads and which move no Standing:

```text
contradicts depends replicates supports synthesized_from
```

**This one does match the repositories, and it is the only lifecycle
vocabulary on this page that does.** All 1,284 relations retained across the
four Frontiers fall in these two sets, and the split is visible in the records
themselves: every `supersedes` sits on a revision-2 Claim submitted through the
correction path, and every descriptive relation sits on a revision-1 Claim
imported from a corpus. The descriptive set is enumerated from those records
rather than declared ahead of them, so it is open — a Frontier may write a kind
it does not name, and that kind gains no authority by being written.

Two spellings are recognised on input and resolve to one canonical name:
`depends_on` reads as `depends`, `opposes` as `contradicts`. Producers emit the
canonical spelling; consumers resolve before matching. `revises` and `retracts`
were declared through `0.966.3` and written into no record; they are withdrawn.
`conformance/fixtures/claim-relation-vocabulary-v1.json` fixes the pair, and a
test in `vela-protocol` fails when an implementation drifts from it.

Do not read `evidence[].relation` against this vocabulary. It names the role an
Artifact plays for one Claim, not a link between two Claims, and every retained
Claim Record spells it `supports`.

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
replay verify reproduce
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
