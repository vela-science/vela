# ADR 0043: Experiment first with exact, artifact-backed Claim dependencies

- Status: Retired after the noncanonical experiment, 2026-08-17
- Supersedes: ADR 0040, whose historical wire-first options remain preserved
- Current disposition: the experiment and its duplicate interpreters are
  deleted. Exact relations already retained in Claims and evidence remain the
  only current representation; no canonical dependency field was added.
- Protocol effect: none. The experiment moved no canonical bytes.
- Product effect: none. The current CLI does not read, author, or admit it.
- Authority effect: none. A profile, reducer result or passing check cannot
  make a Decision or change Standing
- Relates to: ADR 0004, ADR 0009, ADR 0035, and the synthetic correction-impact
  vectors under `conformance/fixtures/correction/`

## Context

ADR 0040 correctly found a representation gap and asked the wrong next
question. At the time, the gap still existed after that wire migration:
`vela.submission.v2` could not author a consequential Claim dependency. The
production writer creates an empty `relations` array for a new Claim and one
`corrects` or `supersedes` relation for a revision. It does not copy `depends`
or `supports` from producer input.

The migration opportunity ADR 0040 contemplated has passed. SubmissionV2 is
already the current signed envelope, the current Math repository has been
re-genesisised under it, and another preimage cut now needs evidence of its own.
Adding a field because a migration once happened would be a second migration,
not a free amendment.

The existing correction-impact projection does not provide that evidence. Its
diamond is synthetic, includes `supports` route semantics, and is intentionally
outside current Repository replay. Current producer-built Repositories cannot
author its non-empty dependency input. In particular, the retained Erdős 321
case has two relation-empty Claims: a first Proposal was rejected, and a
separate corrected successor was later accepted. It is useful source evidence,
not an accepted-state Correction, rooted dependent, or Class E cascade.

What needs testing first is smaller than a new relation object or wire field:
can a source-owned artifact state an exact hard dependency, can independent
readers agree on its bytes and consequence, and can the artifact remain
unambiguously bound after the source Claim already exists?

## Decision

Freeze `claim-dependency-profile.v0` as a bounded, noncanonical experiment
under `conformance/experiments/claim-dependency-profile-v0/`.

The profile permits one meaning only:

```text
source Claim requires exact target Claim
```

Each edge binds:

- the source Claim ID and full root;
- the local Repository UUID and origin root;
- the target Claim ID and full root;
- the same Repository UUID and origin root for the first experiment; and
- profile version, a closed bounded scope, deterministic ordering, and explicit
  nonclaims.

The exact target root is mandatory. An ID alone cannot distinguish the state a
source consumed. Because the profile is a separate artifact, it may be created
after the source Claim exists; that post-hoc path is a requirement for any
future normative representation.

The profile contains no Standing, acceptance, authority, Decision, signature,
truth, probability, confidence, contradiction, or support-independence field.
It is not a Claim relation object, Vela schema, support-route law, or hidden
admission path. Current SubmissionV2, Claim records, Proposal planning,
Verification, Decision, Event construction and replay remain byte-identical.

The reducer consumes the profile plus an explicitly bounded state input and
returns only derived review context:

```text
satisfied
review_required
incomplete
```

It distinguishes missing, malformed, unavailable, unaccepted, retired and
root-mismatched targets. A Correction or Withdrawal of a required target makes
the source and its transitive dependents `review_required`; it does not revoke
their historical Decision or silently alter Standing. Missing or unavailable
state is `incomplete`, never unaffected. Cycles, duplicate IDs or edges,
unsupported kinds, foreign Repository context and exceeded bounds fail closed.

The first frozen graph is synthetic and counterfactual over exact Erdős 321
source anchors. A0 and A1 use retained roots, but the scenario explicitly
records that A0 was rejected in the real Repository and that both real Claims
have empty relations. Synthetic B requires A0, synthetic E requires B, and D
has no `requires` edge and remains unaffected. No independent-support route is
part of v0.

The experiment also freezes a matched disciplined-Git baseline before any
participant observation. Baseline and treatment receive the same rooted source
facts, task, environment and success criteria. Both receive an RO-Crate view
and a deterministic fixture-signed review record; only treatment receives the
structured dependency profile. The fixture signature is public test material,
has no authority, and is not a Vela Verification Record. Unrun measurements
are `not_measured` with null values, never inferred from historical timestamps.

## Validation and promotion gates

The experimental package must prove:

1. Python and JavaScript independently agree on canonical profile bytes,
   profile root, projection bytes and projection root. A Rust test agrees on
   the frozen profile and state roots and maps `requires` into the existing
   correction-impact reducer to confirm the same B/E/D consequence classes.
2. The committed positive graph produces the frozen affected, unaffected,
   stale-Verification, incomplete and repair sets.
3. Wrong source or target roots, Repository UUID or origin, duplicate nodes or
   edges, missing endpoints, unsupported relation kinds and cycles fail closed
   with stable diagnostics.
4. File readers reject path escape, symlink, nonregular file, wrong mode, size
   drift and content-root drift without reading ambient network state.
5. Profile and reducer output have zero Standing and authority effect.
6. The matched baseline carries the same facts and its limitations remain
   visible.

None of those checks promotes the experiment. Promotion requires a real
accepted Correction with a real rooted dependent, an externally produced
profile, independently verified relation fidelity, a clean-room reader, a real
local Decision, frozen expected sets, a matched baseline and a materially
different second maintained producer. A direct Submission or Claim field is
considered only if the artifact approach repeatedly fails through ambiguity,
unsafe post-hoc binding, divergent independent readers, or more maintained
machinery than a direct field would require.

`supports` and inferred alternative routes stay out. A different file, signer,
model or artifact root does not establish independence. Route sufficiency and
shared premises need their own scoped verifier before any support algebra can
be normative.

## Consequences

Current claims about `vela correction impact` remain narrow: it is a
deterministic experimental projection over an input current producers cannot
author through the protocol. Its synthetic diamond proves implementation
agreement, not a non-empty current Repository cascade.

The v0 package can establish byte-level feasibility, deterministic consequence
classification and clean-room implementation agreement. It cannot establish
scientific truth, user value, reviewer-economy improvement, external
independence, accepted-state Correction, provider recurrence, federation, or a
reason to enlarge the kernel.

If the baseline is equally safe and simpler, or no real qualifying dependent
appears, the profile remains an external interoperability artifact or is
retired. The null result is allowed to win.

## Alternatives rejected

### Add `relations` to SubmissionV2 now

Rejected. It moves the signed preimage after the migration window and has no
real accepted dependent demonstrating necessity.

### Add a signed Claim-relation object now

Rejected. It creates a new authoring, proposal, withdrawal and correction
lifecycle before one source-local artifact has shown that a lifecycle is
needed.

### Reuse all correction-impact relation kinds

Rejected. `supports` embeds route sufficiency and independence semantics the
current evidence does not establish. `discovery` is nonconsequential context
and needs no normative dependency edge. V0 is `requires` only.

### Infer dependencies from shared artifacts or citations

Rejected. Shared inputs, proximity and discovery do not prove that one Claim
consumes another. The profile must be an explicit source-owned assertion over
exact roots.

### Treat the profile as accepted because it is signed or checked

Rejected. A signature attributes bytes, and a reducer reports context. Only an
authorized Decision changes Standing, and this experiment has no Decision
path.
