# Vela theory: the current boundary

Status: current pre-1.0 boundary.

Vela records a narrow chain of scientific state:

```text
Target -> native run -> Submission -> Verification -> Decision -> Standing
```

It does not decide whether science is true. It preserves the exact objects,
actors, checks, and authorized transition needed to understand why a Claim has
its current Standing.

The normative wire and storage contract is [PROTOCOL.md](PROTOCOL.md). The Rust
implementation is the executable reference. The TypeScript package and
language-neutral vectors check the portable producer boundary.

## 1. Guarantee ladder

| Layer | Establishes | Does not establish |
| --- | --- | --- |
| Canonical object | exact schema, bytes, identity, and root | truth or authority |
| Repository verification | exact object membership and authority-chain validity | scientific correctness |
| Verification Record | one named verifier's result over exact inputs | acceptance or generality |
| Human Decision | an authorized accept or reject action over one Proposal | universal truth |
| Git publication | that exact repository bytes reached a ref | acceptance by itself |

Use the narrow sentence supported by the evidence. “The Submission parsed,”
“the named verifier passed,” “the Proposal was accepted,” and “the commit was
published” are different statements.

## 2. Canonical objects

For protocol object type `τ`, let:

```text
C_τ : Object_τ -> Bytes
H(b) = SHA-256(b)
root_τ(x) = H(C_τ(x))
```

The type subscript matters. Each schema defines its own closed projection,
canonical bytes, identity preimage, and size bounds. A readable `vcl_`, `vsb_`,
`vvr_`, or `vpr_` handle is not a security digest. Security-sensitive bindings
use the full `sha256:` root and retained bytes.

The current implementations are:

- Rust canonicalization in
  [`canonical.rs`](../crates/vela-protocol/src/kernel/canonical.rs);
- independent Python and JavaScript readers plus portable vectors in
  [`conformance`](../conformance).

## 3. Repository state

A current Frontier is one Git repository whose active scientific index is the
closed `vela.repository.v4` manifest.

Let:

```text
M = (
  profile_root,
  origin_root,
  accepted_claims,
  pending_claims,
  submissions,
  verifications,
  proposals,
  artifacts,
  authority_keyset_root,
  authority_policy_root
)
```

Every member is an exact path, object identity, and full root. Verification
re-reads every indexed object, checks its closed schema and canonical bytes,
recomputes the set roots and repository root, and verifies the repository
authority chain from an independently installed sequence-one trust anchor.

There is no predecessor scientific reducer or alternate manifest reader in the
current runtime. Compacted origin records bind their historical tag, Git
objects, archive digest, equivalence report, and authority root. Current
Standing is represented directly by the manifest's accepted and pending Claim
sets and the terminal Proposal and Decision evidence that justifies them.

## 4. Submission and verification

A Submission is an authenticated producer package:

```text
s = (
  target,
  requested Claim,
  artifacts,
  conditions,
  caveats,
  replayability,
  verification requirements,
  provenance,
  producer authentication
)
```

Submitting `s` retains the exact Submission, Artifacts, Claim Record, and
pending Proposal in one routine-evidence transaction
authenticated by the producer signature:

```text
Submit(s, M) -> (M', pending Proposal)
```

Submission changes no accepted Claim.

Routine evidence intake reads no repository-authority key. It may append exact
content-addressed evidence and rebuild deterministic projections; it may not
write a Decision, Event, policy, authority state, or accepted Standing.

A Verification Record binds the exact Frontier, Claim, Submission, Proposal,
Artifacts, verifier, method, environment, scoped property, outcome, and
explicit nonclaims:

```text
Verify(v, s) -> observation
```

Importing that observation changes no accepted Claim. A passing verifier is
evidence available to a reviewer, not a Decision.

## 5. Decision and authority

One human semantic action targets one exact pending Proposal:

```text
d = (
  action,
  proposal_root,
  claim_root,
  submission_root,
  verification_set_root,
  repository_root,
  authority_event_log_root,
  policy_bundle_root,
  principal,
  reason,
  observed_at
)
```

The Decision Plan root commits to all of those values. The human authenticates
the semantic action through the local runtime session. A separate
repository-authority key signs the complete transaction envelope. Neither the
model nor the repository-authority signer supplies scientific judgment.

The transaction must revalidate all inputs immediately before writing. It then
atomically commits:

- the terminal Proposal;
- the accepted or rejected Claim Standing;
- the semantic Decision event;
- the repository manifest;
- the authority record and its exact write-set root; and
- the recoverable publication journal.

Cancellation or drift writes nothing. Verification never selects the Decision.

## 6. Core invariants

### Exact membership

Every current object indexed by the repository manifest must exist at its exact
path and rederive its identity and full root. Unindexed canonical objects,
missing members, path substitution, and shortened roots fail closed.

### Authority containment

Producer authentication can submit evidence. Verifier authentication can
report a scoped result. Only an authenticated human action authorized by the
current repository policy can decide a Proposal. The repository authority key
can attest the transaction but cannot choose its semantics.

### Evidence is not Standing

```text
VerifierPass(x) does not imply Accepted(x)
Published(x)    does not imply Accepted(x)
Accepted(x)     does not imply universally true(x)
```

### Append-only correction

Prior canonical records are not edited into a new conclusion. A corrected
Claim, Submission, Verification, Proposal, or Decision is a new exact object
with explicit relations to the earlier record.

### Derived-state non-authority

Targets, rankings, graphs, search indexes, web projections, packets, summaries,
and caches are rebuildable readers. They can suggest work but cannot change
Standing.

### Transaction and publication separation

The scientific transaction first creates an exact candidate repository state.
Git publication then moves the intended ref only if its expected head still
matches. A push failure cannot change the semantic outcome, and the recovery
journal preserves the exact publication operation.

## 7. Conformance

The current finite corpus checks:

- canonical JSON and content roots;
- optional source-workbench run or attempt provenance;
- principals and delegated capabilities;
- independent JavaScript emission of Submission and Verification bytes; and
- exact witness and bounded-Claim agreement.

Run:

```bash
cargo test -p vela-protocol
python3 conformance/verify.py
./conformance/check-core.sh
```

Conformance proves agreement on these vectors, not correctness outside them.
Historical reducer, policy, Receipt, and Finding experiments remain in Git
history rather than the active runtime or conformance package.

## 8. Trusted assumptions

Vela's guarantees depend on:

1. correct canonicalization and schema validation;
2. adequate SHA-256 and Ed25519 security;
3. correct repository and authority-chain verification;
4. sound named verifiers for their exact scoped properties;
5. sufficient environment and artifact capture for replay;
6. filesystem and Git behavior matching the transaction model;
7. correct trust-anchor and policy installation; and
8. humans protecting their authentication session and applying judgment.

Vela does not prove novelty, importance, ethics, completeness, personhood,
institutional legitimacy, global consensus, verifier soundness, or the truth
of a scientific Claim. It makes the evidence and authority path inspectable,
replayable, and correctable.
