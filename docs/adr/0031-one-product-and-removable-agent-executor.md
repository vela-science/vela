# ADR 0031: One Vela product; native tools remain external

- Status: Accepted
- Proposed: 2026-07-30
- Accepted: 2026-07-31
- Revised: 2026-07-31
- Implementation: complete
- Protocol effect: none
- Authority effect: none
- Product effect: remove the private Agent runner and Campaign host

## Context

Canopus proved that a bounded external process can produce artifacts, run a
scoped verifier, and export a Vela Submission without receiving scientific
authority. It did not prove that Vela should own an agent runner.

The current private helper duplicated capabilities already supplied by Codex,
Claude, OpenCode, Hermes, Harbor, laboratory runtimes, and other native tools.
Its Rust delegator, helper build pin, execution bundle, Run reservations,
receipt chain, foreground Campaign host, package scripts, and separate CI added
another execution product without protecting an additional scientific or
authority invariant.

The durable boundary is smaller:

```text
native tool -> artifact -> Submission -> Verification -> human Decision -> Standing
```

Vela owns the exact Target packet, the portable scientific records, replay,
and the human Decision boundary. It does not need to own the process that does
the work.

## Decision

1. Remove the current Canopus/private Agent source, `vela agent` delegator,
   foreground Campaign host, helper-specific CI, and private Run projections.
2. Keep immutable `@vela-science/canopus@0.8.0` and `product-v0.8.0` as
   historical replay evidence. Current Vela carries no compatibility parser or
   copied source for that private product.
3. Keep `vela start` as a small ignored local lease. The current Attempt binds:
   - one Frontier and exact Target packet;
   - one producer identity;
   - expiry;
   - allowed Artifact classes; and
   - Submission, Verification, Artifact-count, and Artifact-byte budgets.
4. An Attempt may be consumed only by ordinary Submission registration and
   Verification import. It has no runner binary, execution bundle, Run budget,
   receipt chain, scheduler, signer cache, or Decision method.
5. Native agents consume the Target packet directly and call `vela submit`.
   Native verifiers emit `vela.verification-record.v1` and call
   `vela verification import`. They remain replaceable producers.
6. Private Attempt schemas are current-only. Old ignored files fail clearly and
   are deleted and recreated; no migration or compatibility layer is carried.

## Invariants

- Evidence is not Standing.
- Verification is not acceptance.
- An agent cannot accept, reject, or cancel a Proposal.
- Only an authorized human Decision changes Standing.
- Accepted transitions replay without any agent runner.
- Corrections append; current Vela does not rewrite historical evidence.

## Consequences

Vela loses a bundled execution convenience and removes an entire duplicated
runtime, package, command hierarchy, CI workflow, and private receipt model.
External tools now integrate at the narrow portable records they already need
to produce. If repeated real consumers later demonstrate a missing shared
adapter contract, that contract requires a new evidence-backed ADR; Vela will
not rebuild a general runner by default.

## Rejected alternatives

### Keep a private runner until a twelve-hour benchmark

Rejected. A deletion gate does not justify carrying an unearned subsystem.
The same product question can be measured with native tools and Harbor without
shipping a second harness in Vela.

### Merge the runner into the authority core

Rejected. Execution dependencies are unrelated to deterministic Standing
replay and would weaken the authority boundary.

### Add compatibility for private Attempt and Run files

Rejected. They are ignored local coordination from a pre-release product.
Historical public Canopus artifacts remain readable with their frozen release.
