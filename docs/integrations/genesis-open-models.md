# Current Math genesis and the scientific-state control point

This note is the current, checkable public case for Vela's scientific-state
boundary. It is not a model benchmark or an archive of pre-release migrations.

- **Reader:** signed Vela `v0.975.1`
- **Repository:** `vela-science/math`
- **Repository UUID:** `8138c6da-46c4-47ee-b493-5bbfbec09b1e`
- **Git commit:** `08a0e6d327e1ae9937ab2e0e5002192815eac69a`
- **Git tree:** `f58de302dcaf96e41e4836732dc5446f4eeb8c61`
- **Repository root:** `sha256:3e2236510923277c1e363d2d28c3d84d86a1d698bafd576b79308b18ae0cf0d2`
- **Current Standing:** two accepted Claims, no pending Proposal

## Why the boundary matters

A native tool can compile a source file, prove a theorem, or replay a bounded
calculation. That result is evidence, not scientific Standing. Vela retains the
authenticated Submission, a scoped Verification record, and the separately
authorized Decision that changes Standing. Git carries the exact bytes and
history; neither a build badge nor a Git host makes the Decision.

The current Math genesis contains two deliberately bounded cases:

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

These are different scientific claims with different evidence. They share one
Repository authority and one replay contract; Vela does not force them into a
universal ontology.

## Verify the current state

The canonical Math source is public. Acquisition requires no account, hosted
Vela service, or Repository-authority key:

```bash
git clone https://github.com/vela-science/math.git math
git -C math checkout 08a0e6d327e1ae9937ab2e0e5002192815eac69a
vela replay math --json
vela status math --json
vela claims math --json
vela review list math --status all --json
```

Expected replay facts:

- origin `vro_be55672495053325` /
  `sha256:be556724950533252af6aea398836ffe35717ffcda1f7d609fa6735413941e14`;
- Repository root
  `sha256:3e2236510923277c1e363d2d28c3d84d86a1d698bafd576b79308b18ae0cf0d2`;
- two current accepted Claims;
- three authenticated Submissions, three scoped Verifications, and three
  accepted Proposal transitions;
- zero pending, rejected, or withdrawn Proposals.

For strict consumer trust, obtain the sequence-one authority-record root
`sha256:978e78326c9cd0c665b958696a0255e76fd50cc2d699651fbd7edd95aed418ef`
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

- Erdős 321 or 887 is resolved.
- Occurrence grouping proves statement equivalence.
- A passing Verification accepts a Claim.
- The current bounded correction has a non-empty downstream dependency cascade.
- The internally operated producer, verifier, reader, and authority constitute
  external adoption or plural governance.
- A Web projection or hosted account is a Repository authority.

The pre-Coherence Math lineage remains reachable through ordinary Git history
and a temporary release rollback tag. It is not a live compatibility surface
and contributes no current Standing.
