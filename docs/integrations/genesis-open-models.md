# Current Math genesis and the scientific-state control point

This note is the current, checkable public case for Vela's scientific-state
boundary. It is not a model benchmark or an archive of pre-release migrations.

- **Reader:** signed Vela `v0.977.2`
- **Repository:** `vela-science/math`
- **Repository UUID:** `3d012325-3768-4b95-a385-c94e9f2a57a6`
- **Git commit:** `84b118ed1622d34e5a1431821cf35dca91fb8720`
- **Git tree:** `87e5915227e618d30cfc6530ff218ec0b09ba7cb`
- **Repository root:** `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`
- **Current Standing:** three accepted Claims, no pending Proposal

## Why the boundary matters

A native tool can compile a source file, prove a theorem, or replay a bounded
calculation. That result is evidence, not scientific Standing. Vela retains the
authenticated Submission, a scoped Verification record, and the separately
authorized Decision that changes Standing. Git carries the exact bytes and
history; neither a build badge nor a Git host makes the Decision.

The current Math genesis contains three deliberately bounded cases:

1. **Erdős 321.** The accepted successor Claim records exact source occurrence
   identities and a candidate-answer relationship. It corrects a retained
   predecessor, whose current Standing is `superseded`. Source grouping remains
   navigation-only; semantic equivalence, implication, resolution, and
   optimality are explicitly unestablished. The next obligation is to construct
   and kernel-check the missing bridges between the terminal real-log statement
   and each fixed `Nat.log` variant.
2. **Erdős 887.** The accepted computational Claim reports exact replay of a
   repaired source against retained public compiled-cache inputs. It does not
   establish mathematical correctness, general source reproducibility, or a
   source-owned mapping for the public Problem catalogue.
3. **Erdős 94.** The accepted corrected Claim binds the exact
   `sum_multiplicity` occurrence and preserves its predecessor as superseded.
   The association is navigation-only and does not establish the cubic
   distance-multiplicity conjecture or semantic equivalence.

These are different scientific claims with different evidence. They share one
Repository authority and one replay contract; Vela does not force them into a
universal ontology.

## Verify the current state

The canonical Math source is public. Acquisition requires no account, hosted
Vela service, or Repository-authority key:

```bash
git clone https://github.com/vela-science/math.git math
git -C math checkout 84b118ed1622d34e5a1431821cf35dca91fb8720
vela replay math --json
vela status math --json
vela claims math --json
vela review list math --status all --json
```

Expected replay facts:

- origin `vro_a6a12da8762f1252` /
  `sha256:a6a12da8762f1252afe1ac1c75361ef54c583924a2b88474232ead6227873dca`;
- Repository root
  `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`;
- three current accepted Claims;
- six authenticated Submissions, six scoped Verifications, and six
  accepted Proposal transitions;
- zero pending, rejected, or withdrawn Proposals.

For strict consumer trust, obtain the sequence-one authority-record root
`sha256:efae3e02b5be6dfccf6701ebe26f87f00bb64f5b4372674e572a633844d95469`
through an independent channel and pin it locally before replay. The pin grants
no authority and changes no Repository byte.

## The control point

The system has four boundaries:

1. a producer authenticates bounded evidence in a Submission;
2. a verifier reports one exact property, method, outcome, limitations, and
   shared dependencies;
3. an authorized human or agent performer makes one exact Repository Decision;
4. replay derives current Standing and a read-only projection.

Performer kind is not a quality rank. Evidentiary weight comes from the exact
method, subject, inputs, outputs, scope, limitations, and independence or shared
dependencies. Repository Decision authorization is a separate policy-bound
axis.

## What this does not claim

- Erdős 321, 94, or 887 is resolved.
- Occurrence grouping proves statement equivalence.
- A passing Verification accepts a Claim.
- The current bounded correction has a non-empty downstream dependency cascade.
- The internally operated producer, verifier, reader, and authority constitute
  external adoption or plural governance.
- A Web projection or hosted account is a Repository authority.

The compact current Math lineage is fully carried by Submission v3 and its
root-bound correction Events. No auxiliary recovery branch or legacy
Submission envelope is a live compatibility surface or contributes current
Standing.
