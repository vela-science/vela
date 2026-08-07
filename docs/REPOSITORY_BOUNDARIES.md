# Repository ownership boundaries

Vela is a multi-repository system. Each fact has one canonical owner; other
repositories may bind an exact commit or root, but must not copy that owner's
mutable state into a second source of truth.

| Repository | Owns | Must not own |
| --- | --- | --- |
| `vela` | Protocol semantics, Rust CLI and libraries, wire schemas, conformance fixtures, protocol-wide ADRs, release artifacts, and cross-Frontier evidence claims in the paper | Frontier Target packets, case-specific execution artifacts, scientific Decisions, web projection code, or deployment state |
| Scientific repositories such as `vela-science/math` | Source locks, local admission policy, Target packets, Claims, Submissions, Verifications, Decisions, artifacts, replay state, and exact next obligations | Generic Vela protocol behavior, web rendering, or authority over another repository |
| `vela-web` | Root-bound read projections, Result Dossier declarations, rendering, search, SELECT-only storage, deployment manifests, and product-qualification evidence | Scientific writers, authority credentials, inferred Standing, or duplicate canonical Frontier records |
| Native source and execution repositories | Proofs, computations, datasets, model runs, and native package/toolchain state | Vela Standing or Frontier authority unless separately admitted through the protocol |
| Memos | Research input and recommendations | Canonical product, protocol, or scientific state |

## Placement test

Put a change in `vela` only when it remains meaningful with every named
scientific case removed. Put a change in a Frontier when it names a local
source, Target, verifier, Claim, Decision, artifact, or successor obligation.
Put a change in `vela-web` when it exists only to read, render, search, export,
measure, or deploy already-canonical state.

Protocol-wide research campaigns may live in `vela` when they test a generic
claim across multiple Frontiers. Their case inputs must be exact external
references. Detailed case execution plans stay with the owning Frontier.

## Evidence versus authority

The Vela paper may retain immutable case-study fixtures and measurements needed
to substantiate a cross-Frontier system claim. Those artifacts are evidence for
the paper, not canonical scientific state. The cited Frontier commit, object,
and repository root remain authoritative.

Moving or rendering a record never changes Standing. Only an attributed,
authorized Decision admitted by the named Frontier can do that.
