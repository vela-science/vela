# ADR 0009: Exact dependency pins and deterministic standing

- Status: Superseded by ADR 0018
- Candidate release: Vela `v0.803.0`
- Entry gate: ADR 0006 demonstrates a dependency-standing gap after ADR 0007
  and ADR 0008
- Current disposition: Do not implement this proposed lock or command family.
  ADR 0018 owns the retained authenticated historical dependency-state
  contract.

## Context

Receipt lineage and finding links declare relationships. ADR 0004 showed how a
Receipt extension can bind an experimental full-root dependency observation.
The current protocol does not define the observation's normative child-local
lock shape or its status after a later authorized parent event.

The protocol must propagate dependency standing rather than infer that a child
claim became false.

## Proposed decision

Define the closed child-local lock entry:

```text
vela.dependency-pin.v1 {
  parent_claim_revision
  authority_event {
    id
    content_root
  }
  verifier_attachments[] {
    id
    content_root
  }
  checkpoint_root
  premise_digest
  role
}
```

Allowed roles are:

```text
hard
soft
data
method
contextual
```

The child stores pins in `science.lock`. A Receipt may carry the same entry in
a namespaced, whole-body-bound extension. The parent frontier does not store or
authorize the child's dependency.

## Commands

Add:

```bash
vela depend add \
  --resolved <resolved-parent> \
  --premise <premise-file> \
  --role <role> \
  --lock science.lock

vela depend check --lock science.lock --json

vela update \
  --lock science.lock \
  --repo <path-or-url-or-bundle> \
  --to-root <git-commit> \
  --json
```

`depend add` copies full roots from a successful resolution. It refuses free
text or a mutable branch as dependency identity.

`depend check` verifies the pinned local state without discovering a newer
root.

`update` validates the supplied checkpoint and Git/Vela continuity, then
recomputes standing.

## Status semantics

The deterministic vocabulary is:

```text
satisfied
warning
review_required
blocked
stale
forked
unresolvable
```

Rules:

- unchanged accepted parent standing is `satisfied`;
- a relevant qualification or contest produces `review_required` for hard
  dependencies and may produce `warning` for softer roles;
- withdrawal, retraction, or verifier-integrity loss produces `blocked` for a
  hard dependency;
- supersession produces `stale` until the child revalidates or repins;
- a non-descendant valid lineage produces `forked`;
- missing bytes, invalid roots, invalid signatures, or unavailable required
  verifier material produce `unresolvable`.

The status never asserts that the child is true or false. It reports whether
the declared reliance remains usable under the lock profile.

## Correction and revalidation

A parent correction appends an authority event and a later checkpoint.
Historical parent and child bytes remain unchanged.

The child may:

- remain blocked or review-required;
- rebuild against a corrected parent revision; or
- produce independent evidence that removes or replaces the dependency.

Each action creates new Receipt, verifier, or authority records. No command
silently edits the historical lock or retracts the child.

## Migration and compatibility

Old frontiers replay unchanged. Existing lineage remains declared provenance.
It does not become a strict dependency pin without an explicit `science.lock`
entry.

Older binaries may ignore `science.lock`. They cannot claim conformance with
the dependency profile.

## Adversarial cases

Conformance covers:

- missing or wrong parent revision root;
- mismatched authority event;
- verifier evidence copied from another claim;
- omitted premise digest or role;
- altered lock bytes;
- mutable branch or URL used as identity;
- stale checkpoint;
- non-descendant fork;
- missing artifact or verifier bytes;
- qualification that intersects or does not intersect the premise;
- hard and soft role differences;
- revalidation after withdrawal; and
- any automatic child-truth or accepted-state mutation.

## Conformance

```bash
cargo test -p vela-protocol dependency_pin
cargo test -p vela-edge dependency_standing
cargo test -p vela-cli --test handoff_workflows dependency
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

Rust and an independent reader must emit the same status and canonical
projection root for every vector.

## Consequences

The child gains a package-lock-style scientific dependency with correction
semantics. Vela still records scoped authority rather than computing truth, and
no parent event automatically changes child accepted state.
