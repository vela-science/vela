# ADR 0037: Removable protocol edges and adoption order

- Status: Accepted
- Accepted: 2026-08-03
- Protocol effect: none
- Scientific effect: none
- Authority effect: none

## Context

Vela now has enough native producer, verifier, Decision, replay, and read-model
surface to attract interoperability work. The dangerous failure mode is to
confuse transport adoption with scientific authority: a tool call, schema
match, signature, package resolution, or green read-model label must never
become acceptance by implication.

The current product proof is also unfinished. The Erdős 264 Result Dossier
recovers the registered facts exactly, but its first three frozen usability
iterations did not reach the registered reviewer-time gate. That is evidence
against starting a broad protocol or integration train.

## Decision

Adopt the following ownership boundary and order:

```text
native systems    execution, proof checking, data, model runs, native packages
portable waist    exact encoding, roots, signatures, closed structure
Vela              Claim lineage, Verification scope, Decision, correction, Standing
Frontier          local admission policy and human authority
read projections  root-bound orientation with authority_effect = none
```

Current protocol objects may receive checked JSON Schema 2020-12 descriptions
and frozen conformance fixtures. Those schemas document structure; the Rust
reader remains authoritative for signatures, roots, references, semantic
constraints, repository invariants, Decision authority, and Standing.

External edges must be source-local, read-only, root-aware, removable, and
honestly named until a second maintained consumer proves reuse. MCP, A2A,
hosted writers, package commands, registries, and network authorization
services are not current Vela capabilities. No protocol surface may expose or
delegate `review accept`, `review reject`, repository-authority credentials,
or an automatic Standing change.

The implementation order is:

1. qualify reusable Result Dossier cases, then prove the projection materially
   usable through the [Vela Web multi-case human gate](https://github.com/vela-science/vela-web/blob/main/docs/result-dossier-qualification.md);
2. publish and check the current portable read/write structure without changing
   object bytes or writer behavior;
3. complete a separately reviewed DSSE v2 and authorization-history cut only
   after its fixtures and migration boundary are frozen;
4. add one removable read-only edge after a concrete consumer appears; and
5. extract a reusable package only after two maintained consumers produce net
   deletion of case-specific code.

ADR 0035 remains Proposed. This decision does not accept its pending DSSE v2,
authorization-history, or epoch-cut work.

## Promotion gates

An edge may move beyond an experiment only when it has:

- one exact versioned contract and fixture root;
- deterministic offline reconstruction;
- explicit loss, nonclaims, and `authority_effect = none`;
- no authority credential or automatic Decision path;
- at least two maintained consumers or one measured user task that cannot be
  served by the existing CLI/HTTP read surface; and
- a deletion plan that restores the pre-edge product if the experiment fails.

Package publication additionally requires two consumers and net deletion.
Hosted writes require a later authority and threat-model decision; this ADR
does not authorize them.

## Consequences

Vela can adopt commodity standards without turning their vocabulary into its
scientific semantics. The immediate work stays small: checked schemas,
inventory, frozen roots, and reusable read cases. Separately scoped scientific
qualification follows ADR 0038; confirmatory autonomous-research benchmarks,
a package subsystem, and biomedical expansion remain queued behind product and
reuse evidence.
