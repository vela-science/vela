# ADR 0006: Independent correction-aware handoff and standards baseline

- Status: Deferred — inactive experiment
- Protocol release: none
- Experiment runner: Canopus `v0.2.0`
- Current disposition: Preserve the design as historical evidence. The current
  campaign does not depend on outside participants and authorizes no primitive
  from this experiment.

## Context

Vela `v0.800.22` supplies immutable Git transport, Receipt v1, proposals,
separate verifier evidence, signed authority events, correction, deterministic
replay, and full SHA-256 roots. ADR 0004 added removable experimental
composition profiles and a matched standards wrapper. Those profiles have
first-party conformance and diagnostic evidence, but no outside producer,
blind consumer, independent reader, real correction, or head-to-head baseline.

The proposed handoff problem is:

> A consumer receives an exact accepted result from another organization,
> binds a substantive child to the exact premise it used, and later classifies
> that dependency after an authorized correction without producer contact,
> maintainer repair, a central Vela service, or historical rewriting.

Current finding links and Receipt lineage declare relationships. They do not
define a normative cross-frontier dependency lock or a later-root update rule.
Vela must test whether that missing meaning requires a protocol primitive or
whether existing standards and a small profile solve the problem.

## Decision

Run one frozen blind experiment before accepting ADR 0007, 0008, or 0009.
This ADR promotes no object, event, command, signature, authority rule, or
accepted-state behavior.

The experiment uses two matched arms:

1. the current Vela profile, expressed through released objects, full roots,
   Receipt v1 namespaced extensions, Git ancestry, and ADR 0004 readers; and
2. Git plus DSSE or in-toto attestations, OCI descriptors, TUF-style update
   metadata, and a signed `science.lock`.

Both arms receive the same scientific facts, participant instructions,
intervention rules, trust assumptions, and usability constraints.

## Scientific task

Use finite graph theory with SAT/LRAT evidence.

Producer A publishes a canonical graph `G` and establishes:

```text
G is triangle-free and has chromatic number 4.
```

The evidence contains:

- canonical graph bytes;
- a 4-coloring witness;
- a SAT encoding of 3-colorability;
- an LRAT or equivalently checkable unsatisfiability certificate;
- exact verifier commands and environments; and
- a scoped human-readable claim with material conventions.

Producer B receives only the frozen instructions, exact accepted root, claim
handle, and later delivered roots. B applies the Mycielski construction and
establishes:

```text
M(G) is triangle-free and has chromatic number 5.
```

B must create a new graph and verifier-backed child. A citation, copied
artifact, rendering, or metadata-only wrapper does not qualify.

## Participants

| Role | Requirement |
| --- | --- |
| Protocol team | Freeze specification, vectors, task, and intervention rules before A starts |
| Producer A | Outside team with no Vela integration history |
| Verifier V1 | Clean reproduction, separate from A |
| Verifier V2 | Independent graph and certificate implementation |
| Human steward H | Local terminal decision with a real key |
| Producer B | Blind consumer with no contact with A or Vela maintainers |
| Reader C | Separate repository and implementation, no Vela reducer reuse |
| Red team R | Predeclared correction, rollback, fork, and mutation drills |
| Baseline team | Matched standards implementation |

Fresh model sessions from the Vela team do not satisfy these independence
requirements.

## Frozen sequence

1. Freeze the task, schemas, scorer, versions, mutation cases, and support
   policy.
2. A emits one structurally valid result without hand-editing protocol JSON.
3. V1 and V2 reproduce A from clean inputs.
4. H accepts, rejects, or requests revision through the local human boundary.
5. B receives the accepted root and works without A or maintainer contact.
6. B binds the exact parent revision and builds the substantive child.
7. R introduces a statement-fidelity correction or verifier-profile
   withdrawal.
8. H appends the authorized correction without rewriting history.
9. B receives a later root through an untrusted channel and recomputes
   dependency standing.
10. Reader C and the reference reader produce the same canonical projection.
11. The baseline team repeats the sequence.

## Adversarial contract

The frozen mutation set includes:

- changed claim text with unchanged artifact bytes;
- changed artifact bytes behind the same path;
- verifier evidence copied from another claim;
- unauthorized or revoked signer;
- short-ID collision;
- wrong full revision root;
- omitted premise or dependency role;
- reordered, deleted, duplicated, or inserted event;
- stale valid root after a newer pinned root;
- non-descendant signed fork;
- conflicting signed children;
- correction aimed at the wrong revision;
- unavailable original artifact with a valid untrusted mirror;
- map-order differences between readers; and
- an agent attempt to enter the human signing path.

Every case must fail closed or produce an explicit stale, forked, blocked,
review-required, or unresolvable result.

## Measurements

The experiment records:

- time to A's first valid Receipt;
- hand-edited protocol bytes;
- clean-clone reproduction;
- B's contact and information set;
- substantive-child verification;
- full-root completeness;
- correction propagation latency;
- Reader C projection agreement;
- offline completion;
- mutation handling;
- user steps and errors;
- implementation lines and dependencies;
- human review minutes; and
- integration effort for both arms.

The usability target remains p90 under ten minutes for Receipt production,
zero hand-edited protocol JSON, and no maintainer artifact repair.

## Decision rule

Accept a candidate Vela invariant only if:

1. outside participants encounter the gap without coaching;
2. the Vela profile completes the blind handoff;
3. Reader C agrees with the reference projection;
4. correction and fork handling remain deterministic offline; and
5. Vela provides a material safety or interoperability property the baseline
   lacks, or reduces integration effort by at least 30 percent at equal
   semantics.

Classify the result:

- **GO:** accept only the smallest demonstrated invariant.
- **PIVOT:** publish a standards-compatible profile and CLI when the baseline
  supplies equal semantics.
- **NO-GO:** stop protocol expansion when blind handoff or independent replay
  fails.

## Consequences

ADR 0007 through ADR 0009 remain Proposed until this experiment selects them.
Hub, Atlas, Canopus, Git publication, or a passing first-party fixture cannot
substitute for the required participants or human decision.

## First-party rehearsal evidence

The authority-free rehearsal at registration root
`sha256:c9afabdac6ec868f286583a995e27cdad2055c95b655bad6f91cdbcc30d11482`
completed both graph generations, two verifier paths, SAT-to-LRAT evidence, a
released-Vela deferred Receipt with zero accepted-event delta, the substantive
child, 54 correction and continuity vectors with Reader C parity, and 13
standards-wrapper vectors. A clean second output run reproduced all eight
scientific artifact roots and every registered pending-route and standing
invariant.

The rehearsal reproduced and corrected two controller path defects. It did not
reproduce a claim-identity, checkpoint-continuity, or dependency-standing
protocol gap. ADR 0007 through ADR 0009 therefore remain unimplemented.

This result does not satisfy this ADR's decision rule. The authority profile
was an explicitly simulated internal fixture, no human signed, and no
participant was independent. The exact measurements and gap classifications
are in
`research/verifiable-composition/results/first-party-handoff-gap-report-2026-07-16.md`.
