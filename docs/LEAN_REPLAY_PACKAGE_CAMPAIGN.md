# Source-local Lean replay package campaign

## Objective

Determine whether one exact, non-authoritative Lean replay contract can serve
the retained Formal replay and one real Erdős replay while deleting more
maintained contract and fixture logic than it adds.

This is a Level 0 package-evidence experiment under ADR 0019. It does not
authorize a package release, registry, package CLI, resolver, hosted index,
Frontier lock, or Vela protocol object.

## Frozen boundary

The candidate lives at
[`research/lean-replay-contract/`](../research/lean-replay-contract/) and uses
the provisional coordinate:

```text
vela-science/lean-replay-contract@0.0.0-source-local
```

The coordinate is not exact identity. Each consumer binds:

- the Vela source commit and tree;
- the source-local path;
- the RFC 8785 JCS package descriptor bytes;
- the resulting SHA-256 package root; and
- every included file's media type, size, and SHA-256 root.

The package has `authority_effect: none`. It contains generic replay mechanics,
closed request/result schemas, and conformance vectors. It contains no theorem,
Claim, Verification Record, Decision, Event, authority credential, or mutable
alias.

## Consumers

1. Formal Conjectures Frontier: the retained Erdős 835 exact replay.
2. Erdős Frontier: the accepted Erdős 264 part-i repair replay.

Each consumer keeps its theorem boundary, exact source, native Lean/Mathlib
locks, time budget, artifacts, scientific records, limitations, and Standing
interpretation. The shared contract may validate and execute replay mechanics;
it cannot infer acceptance.

## Implementation cuts

### Cut 0 — rooted candidate

- closed candidate manifest and JSON Schemas;
- maintained RFC 8785 package-root builder;
- exact file descriptors and consumer-reference verifier;
- positive and fail-closed axiom vectors; and
- explicit native dependency and authority boundaries.

### Cut 1 — two exact consumers

- bind one package root from both repositories;
- replace duplicated hashing, Git identity, toolchain, Mathlib, axiom, network,
  and subprocess contract logic where semantics genuinely match;
- retain case-specific code locally; and
- run current validation and full replay paths without changing any retained
  scientific object.

### Cut 2 — independent and clean reconstruction

- compute the same root with maintained Python and Rust or JavaScript readers;
- reconstruct on macOS arm64 and Linux x86_64 with network disabled;
- detect wrong roots, omitted files, path traversal, symlinks, dirty or stale
  sources, wrong toolchains, wrong Mathlib revisions, forbidden axioms,
  unsupported sandbox profiles, timeouts, and missing axiom reports; and
- retain platform, command, input, output, and failure roots.

The independent Rust reader is implemented as a focused Vela protocol test and
must reproduce the frozen Python/JCS root exactly. This establishes reader
agreement only; the Linux network-denied native Lean replay and net-deletion
gates remain separate.

### Cut 3 — extraction decision

Measure maintained lines and fixtures before and after, cold setup time, failure
diagnosis time, consumer-specific exceptions, and false positives. Promote to a
shared immutable Git package only if two maintained consumers remain, both
reconstruct cleanly, independent readers agree, and net deletion is positive.

If the gate fails, keep useful generic helpers source-local or delete the
candidate. Do not compensate by building a registry or moving duplicated code
into a new repository.

## Nonclaims

- No released Vela package or supported package ecosystem.
- No `vela package` command, `vela.lock`, `vela://` identifier, PURL type,
  package Web page, OCI artifact, or hosted registry.
- No replacement of Elan, Lake, Lean, Mathlib, Cargo, uv, Nix, or Git.
- No package conformance, signature, replay, or use changes scientific Standing.
- No broad replay portability, external adoption, or productivity result.

## Exit gate

Level 1 is earned only when all of these pass:

1. the exact same package root serves both maintained consumers;
2. at least two maintained readers agree on the root and fixture meaning;
3. macOS and Linux network-disabled clean reconstructions pass;
4. corrupted roots, omitted obligations, wrong environments, and unsupported
   platforms fail closed;
5. native toolchains remain native;
6. removal or upgrade has zero authority effect; and
7. the measured maintained-code and fixture balance is net deletion.
