# Repository ownership boundaries

Vela is a multi-repository system. Each fact has one canonical owner; other
repositories may bind an exact commit or root, but must not copy that owner's
mutable state into a second source of truth.

| Repository | Owns | Must not own |
| --- | --- | --- |
| `vela` | Protocol semantics, Rust CLI and libraries, wire schemas, conformance fixtures, protocol-wide ADRs, release artifacts, and cross-repository evidence claims in the paper | Scientific-repository Target packets, case-specific execution artifacts, scientific Decisions, web projection code, or deployment state |
| Scientific repositories such as `vela-science/math` | Source locks, local admission policy, Target packets, Claims, Submissions, Verifications, Decisions, artifacts, replay state, and exact next obligations | Generic Vela protocol behavior, web rendering, or authority over another repository |
| `vela-web` | Root-bound read projections, Result Dossier declarations, rendering, search, SELECT-only storage, deployment manifests, and product-qualification evidence | Scientific writers, authority credentials, inferred Standing, or duplicate canonical repository records |
| Native source and execution repositories | Proofs, computations, datasets, model runs, and native package/toolchain state | Vela Standing or repository authority unless separately admitted through the protocol |
| Memos | Research input and recommendations | Canonical product, protocol, or scientific state |

## Placement test

Put a change in `vela` only when it remains meaningful with every named
scientific case removed. Put a change in a scientific Repository such as `vela-science/math` when it names a local
source, Target, verifier, Claim, Decision, artifact, or successor obligation.
Put a change in `vela-web` when it exists only to read, render, search, export,
measure, or deploy already-canonical state.

Protocol-wide research campaigns may live in `vela` when they test a generic
claim across multiple scientific repositories. Their case inputs must be exact external
references. Detailed case execution plans stay with the owning repository.

## Evidence versus authority

The Vela paper may retain immutable case-study fixtures and measurements needed
to substantiate a cross-repository system claim. Those artifacts are evidence for
the paper, not canonical scientific state. The cited repository commit, object,
and repository root remain authoritative.

Moving or rendering a record never changes Standing. Only an attributed,
authorized Decision admitted by the named repository can do that.
