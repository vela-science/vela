# Vela theory: the formal boundary

Status: current prelaunch boundary for Vela `0.800.7`.

This document says exactly what can be inferred from Vela's protocol, code,
conformance vectors, and Lean models. It is intentionally smaller than the
research program that preceded it. Vela is an authority-aware, replayable state
layer for scientific work. It is not a mathematical theory of science and does
not turn recorded evidence into truth.

The normative wire and storage contract is [the protocol](PROTOCOL.md). The Rust
implementation is the executable reference. Lean proves selected structural
lemmas about explicit models. Conformance vectors test byte and replay agreement.
These layers support one another, but none silently upgrades the assurance of
another.

## 1. The guarantee ladder

| Layer | Establishes | Does not establish |
| --- | --- | --- |
| Protocol | the objects, byte rules, authority boundary, and replay obligations an implementation claims | that an implementation follows them |
| Rust checks | that the inspected frontier satisfies the checks implemented by this version | scientific truth or absence of implementation bugs |
| Conformance vectors | agreement on the covered bytes and cases | correctness outside the vectors |
| Lean | the stated theorem for the stated abstract or concrete Lean model | equivalence between that model and all Rust, Git, OS, or network behavior |
| Named verifier | a result for exact artifacts under a named method and environment | significance, generality, or acceptance |
| Signed policy or human decision | authority to admit a bounded transition | correctness of the underlying scientific claim |

The system should use the narrowest accurate sentence. “The Receipt parsed,”
“the reducer replayed,” “the named verifier passed,” and “a human accepted the
proposal” are different claims.

## 2. Mathematical objects

### 2.1 Canonical bytes and roots

For each protocol type `τ`, let

```text
C_τ : Object_τ -> Bytes
```

be the canonical byte function specified for that type. The subscript matters:
Receipt v1 uses its strict JCS whole-body contract, while other current objects
use the canonical JSON subset pinned by their schemas and vectors. Vela must not
pretend every historical object shared one serializer.

Let

```text
H(b) = SHA-256(b)
root_τ(x) = H(C_τ(x))
```

where a particular object may add a domain tag, prefix, projection, or truncation
defined by its schema. Therefore:

- byte equality implies root equality;
- recomputing a different root detects changed bytes, subject to the hash and
  implementation assumptions below; and
- identifier equality is not a mathematical proof of semantic equality.

The current implementations are
[`canonical.rs`](../crates/vela-protocol/src/kernel/canonical.rs) and
[`receipt_v1.rs`](../crates/vela-protocol/src/objects/receipt_v1.rs). The portable
examples are
[`canonical-hashing.json`](../conformance/canonical-hashing.json) and
[`decision-binding.json`](../conformance/decision-binding.json).

### 2.2 Receipts and proposals

A Receipt is a producer statement

```text
r = (claim, type, replayability, artifacts, caveats,
     conditions, required verification, provenance)
```

bound to its complete canonical body. It is evidence, not a decision. Landing
maps a valid Receipt to retained bytes, artifacts, a landing record, and a
proposal:

```text
L(r, S) = (retained(r), proposal(r, S), route(r, S))
```

The route is `Deny`, `Defer`, or `Permit`. `Deny` produces no canonical delta.
`Defer` retains a pending proposal. `Permit` can install accepted state only
when a previously human-signed policy verifies for the exact causal state and
produces a replay-verifiable certificate.

A human acceptance follows the same semantic boundary through a terminal key
ceremony. The private Decision Plan binds the exact proposal and current facts;
it is not a second protocol authority. See
[`decision_plan.rs`](../crates/vela-cli/src/decision_plan.rs).

### 2.3 Events and state

Let an event be

```text
e = (schema, id, kind, target, actor, time, reason,
     before, after, payload, caveats, signature_or_certificate)
```

and let `E` be the finite set of retained event bytes. Current replay orders
events by `(timestamp, id)`:

```text
L(E) = sort_(timestamp,id)(E)
```

Let `δ : State × Event -> Result State Error` be the reducer step and `S0` the
validated genesis state. Materialized state is

```text
R(S0, [])       = S0
R(S0, e :: es)  = R(δ(S0, e), es)
S               = R(S0, L(E))
```

The event-log commitment is a separate construction: current code sorts event
content by event ID and excludes signatures before hashing. This keeps content
addressing orthogonal to legitimate re-signing. It must not be confused with
replay order or the materialized-state root.

Current definitions live in
[`events.rs`](../crates/vela-protocol/src/kernel/events.rs),
[`reducer.rs`](../crates/vela-protocol/src/kernel/reducer.rs), and
[`sign.rs`](../crates/vela-protocol/src/kernel/sign.rs).

### 2.4 Authority

Let `K` be the actor registry and `P` the retained policy state. For a
truth-bearing transition, authority is a predicate over the exact causal input:

```text
Authorized(e, S_pre, K, P) :=
    ValidHumanSignature(e, K, S_pre)
    or ValidPolicyCertificate(e, P, S_pre)
```

The second disjunct does not make a service or model a signer. It means a human
previously signed a bounded policy and the deterministic evaluator re-derived
`Permit` for the exact proposal, Receipt, evidence set, policy head, and parent
event-log root. Unknown, stale, revoked, expired, widened, backdated, or
otherwise mismatched inputs fail closed.

The executable policy boundary is in
[`acceptance_policy.rs`](../crates/vela-protocol/src/policy/acceptance_policy.rs)
and
[`policy_accept.rs`](../crates/vela-protocol/src/proposals/policy_accept.rs).

### 2.5 Verification, gate, acceptance, and publication

These predicates are independent:

```text
Integrity(x)      -- canonical bytes, roots, signatures, and replay agree
Reproduced(x, v)  -- named verifier v reproduced its bound result
Gate(x, A)        -- retained claim-matched attachments A satisfy a rule
Accepted(x)       -- an authorized decision admitted x
Published(x, ref) -- the exact delta reached the intended Git ref
```

No implication is valid without an explicit rule. In particular:

```text
Reproduced(x, v) does not imply Accepted(x)
Accepted(x)      does not imply True(x)
Published(x, r)  does not imply Accepted(x)
```

## 3. Current invariants

The current contract is organized around the following invariants.

### 3.1 Deterministic replay

Given the same validated genesis inputs, event bytes, schemas, and reducer
version, implementations must produce the same covered projection. Replay reads
no wall clock. Timestamps already present in events may determine order or
expiry semantics; the reducer does not mint new time.

This is a determinism claim, not a liveness claim. It says nothing about whether
all desired events arrive or whether two Git histories contain the same set.

### 3.2 Complete state-transition evidence

Every projected mutation must be justified by a recognized event kind and its
validated payload. Mutating a materialized file without a corresponding event
does not create authority and should fail replay comparison.

For a target mutation, `before_hash` must match the prior target state and
`after_hash` the resulting target state. Audit-only events use the explicit null
boundary and do not masquerade as scientific mutations.

### 3.3 Proposal-decision parity

A proposal status is a projection, not authority. Acceptance, rejection, and
revision status must agree with the signed decision events that bind the exact
proposal. An accepted domain transition and its decision record are committed
atomically by the current decision transaction.

The parity check is implemented in
[`proposals`](../crates/vela-protocol/src/proposals/mod.rs).

### 3.4 Authority containment

Producer activity can create evidence and proposals but cannot cross the
decision boundary. A policy can authorize only its signed scope and causal
head. A policy cannot authorize its own replacement. A model can prepare review
material but cannot supply a human signature or become a verifier merely by
reporting a pass.

### 3.5 Evidence is not a verdict

Receipt claims, producer-reported runs, verifier attachments, gate projections,
human judgments, and publication receipts retain their separate provenance.
Derived displays must not collapse them into an unqualified “verified” flag.

### 3.6 Correction without erasure

Correction is append-only:

```text
S_old --correction event--> S_new
```

Retraction, supersession, caveat, and repair change the current projection while
preserving the prior event and its authority record. A Git force-push or manual
edit is not a scientific correction.

### 3.7 Derived-state non-authority

`frontier.json`, proof packets, indexes, graphs, Hub rows, wikis, rankings, and
AI summaries are functions of committed inputs. They may be deleted and rebuilt.
An inferred relation enters accepted state only by returning through Receipt v1,
proposal, and decision.

### 3.8 Transaction separation

A scientific transaction and Git publication are distinct. The prepared
frontier delta has one commit marker and private recovery journal. Publication
builds an isolated candidate tree and moves a ref only if the expected ref still
matches. A push failure cannot change the scientific route.

The implementation boundaries are
[`frontier_txn.rs`](../crates/vela-cli/src/frontier_txn.rs) and
[`git_publish.rs`](../crates/vela-cli/src/config/git_publish.rs).

## 4. What Lean proves

The Lean tree is a collection of structural and domain models. It is not a
formalization of the complete Rust implementation. The most directly relevant
current modules are:

| Module | Checked statement | Boundary |
| --- | --- | --- |
| [`Protocol/ReducerModel.lean`](../lean/Vela/Protocol/ReducerModel.lean) | replay is a fold; append composes; a small concrete reducer grows its log and preserves descriptors | the event and state types are deliberately small, not the Rust `Project` |
| [`Protocol/Log.lean`](../lean/Vela/Protocol/Log.lean) | equal finite model logs yield equal canonical sequences and replay; changed cores change IDs under injective hashing | canonical order and state are abstract; the file's `AtlasState` name is only a carrier name |
| [`Protocol/ReplayAppend.lean`](../lean/Vela/Protocol/ReplayAppend.lean) | replay over `a ++ b` equals replay of `a` followed by `b` | a general `foldl` law, not proof of transaction recovery |
| [`Crypto/CanonicalEventId.lean`](../lean/Vela/Crypto/CanonicalEventId.lean) | serializer-then-hash is injective if both functions are injective | SHA-256 and the Rust serializer are assumptions, not proved implementations |
| [`Governance/ProposalIdempotency.lean`](../lean/Vela/Governance/ProposalIdempotency.lean) | repeated acceptance is idempotent under the stated deduplication hypothesis | the important deduplication property is a hypothesis |
| [`Governance/GovernedQuorumSoundness.lean`](../lean/Vela/Governance/GovernedQuorumSoundness.lean) | the modeled acceptance predicate yields enough distinct eligible, unrevoked, valid signers | equivalence to every Rust governance path is not proved |
| [`Crypto/Signing.lean`](../lean/Vela/Crypto/Signing.lean) | a historical finding-signing model is invariant to one excluded cache flag | useful regression history, not the current end-to-end key-custody theorem |

The build root [`Vela.lean`](../lean/Vela.lean) imports a much broader theorem
bundle plus a Sidon certificate. Transfer, accumulation, older diff-pack,
frontier-calculus, and domain-construction modules are research or domain proofs;
their presence does not enlarge the current Vela protocol.

Some modules state abstract hash or serializer injectivity as axioms, and some
composition modules use opaque functions with preservation assumptions. This is
legitimate only when surfaced as an assumption. The
[`AxiomAudit`](../lean/Vela/AxiomAudit.lean) reports dependencies for its explicit
registry; it should not be summarized as “the whole implementation is
axiom-free.” A compiler-checked `native_decide` certificate also has a different
trusted computing base from a small kernel reduction.

The honest Lean claim is:

> Lean checks the listed statements for their listed models and assumptions.

It is not:

> Lean proves Vela, Git, SHA-256, Ed25519, every verifier, and every scientific
> conclusion correct end to end.

## 5. What conformance establishes

Conformance is executable agreement over finite vectors. The current focused
surfaces include:

- canonical JSON and content hashing;
- proposal and decision binding;
- event reducer effects across the shipped fixtures;
- cross-language replay readers;
- signature and proposal parity;
- trust-boundary invariants; and
- the task-first Receipt, policy, and human flow.

Relevant entry points are
[`conformance/`](../conformance/),
[`canonical_hashing_conformance.rs`](../crates/vela-protocol/tests/canonical_hashing_conformance.rs),
[`cross_impl_reducer_fixtures.rs`](../crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs),
[`proposal_signature_parity.rs`](../crates/vela-protocol/tests/proposal_signature_parity.rs),
[`trust_invariants.rs`](../crates/vela-protocol/tests/trust_invariants.rs), and
[`task_first_workflows.rs`](../crates/vela-cli/tests/task_first_workflows.rs).

Focused commands are:

```bash
cargo test -p vela-protocol --test canonical_hashing_conformance
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
cargo test -p vela-protocol --test proposal_signature_parity
cargo test -p vela-protocol --test trust_invariants
cargo test -p vela-cli --test task_first_workflows
python3 conformance/verify.py
```

The independent reader may require its documented local runtime. These are
repository checks, not live partner tests. No Diderot service, external network,
unrelated Lean campaign, public mirror, or Hub deployment is part of this
formal boundary.

For Lean itself:

```bash
cd lean
lake build
lake env lean Vela/AxiomAudit.lean
```

Those commands may be expensive on a cold toolchain. A claim that a revision is
green requires a recorded successful run; this document does not substitute for
one.

## 6. Assumptions and trusted computing base

The guarantees above are conditional on at least these assumptions:

1. **Canonicalization.** Each implementation emits and parses the schema's exact
   canonical byte form, rejects ambiguity such as duplicate names where required,
   and uses the correct preimage projection.
2. **Hash security.** SHA-256 has adequate collision and second-preimage
   resistance for the use. Truncated object IDs retain less collision margin than
   full roots; retained bytes and full bindings remain necessary.
3. **Signature security.** Ed25519 verification and the versioned signing input
   behave as assumed, and private keys remain under the stated custodian's
   control.
4. **Reducer correctness.** The Rust and independent reducers correctly
   implement every current event arm and reject unsupported shapes.
5. **Verifier soundness.** A named verifier, solver, proof assistant, instrument,
   or replay environment is sound for the exact claim it reports.
6. **Environment fidelity.** Frozen code, dependencies, configuration, hardware
   assumptions, and artifact bytes are sufficient to reproduce the result.
7. **Storage and Git.** Filesystem durability, locking, atomic replacement, and
   Git compare-and-swap behavior meet the transaction model. Detecting a deleted
   history also requires a retained trusted root, ref, or copy to compare against.
8. **Governance configuration.** Actor keys, revocations, policy heads, scopes,
   and eligibility rules were established correctly by the authorized humans.
9. **Human judgment.** Human reviewers understand the evidence and protect their
   keys. Vela records their decision; it does not prove the decision wise.

## 7. Explicit non-guarantees

Vela does not prove:

- that a scientific claim is true, novel, important, ethical, or complete;
- that a paper, dataset, experiment, model, or person is honest;
- that passing one verifier generalizes beyond its exact claim and environment;
- that a reviewer or signed policy chose the right scientific outcome;
- personhood, Sybil resistance, institutional identity, or social consensus;
- one global canon, distributed consensus, CRDT convergence across different
  event sets, or a federation authority protocol;
- confidentiality merely because a public descriptor is opaque;
- availability of remote artifacts or long-term operation of a Git host;
- correctness of inferred graph edges, generated summaries, rankings, or model
  outputs until separately evidenced and accepted;
- completeness of the scientific frontier or discovery of unknown unknowns;
- the soundness of SHA-256, Ed25519, Lean's kernel, Mathlib, the compiler, the
  operating system, or hardware from first principles; or
- end-to-end refinement between the Lean models and every executable code path.

In particular, presheaves, graded epistemic calculi, provenance semirings,
proof-carrying knowledge, generalized transfer categories, Atlas/Constellate
object systems, and autonomous federation may remain useful research ideas.
They are not current protocol guarantees unless they return as small, implemented,
tested objects at the Receipt-to-event boundary.

## 8. The boundary in one statement

For exact committed inputs and under the assumptions above, Vela aims to make
the following independently checkable:

```text
what bytes were offered,
what verification was reported or reproduced,
what transition was proposed,
which key or prior signed policy authorized it,
what event entered the log,
and what state deterministic replay derives now.
```

That is the formal core. Scientific truth remains corrigible, plural, and
outside the software's authority.
