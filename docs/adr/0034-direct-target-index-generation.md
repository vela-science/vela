# ADR 0034: Direct Target Index generation

- Status: Accepted — 2026-08-01
- Protocol effect: replaces `vela.target-index.v4` with the smaller derived
  `vela.target-index.v5` contract
- Scientific authority effect: None

## Context

The Target Index is a disposable producer-work projection. V4 nevertheless
gave it a second lifecycle: a domain generator wrote an ignored candidate,
then hidden Vela porcelain checked, sealed, applied, inspected, and diagnosed
the final file. The ceremony duplicated ordinary Git review without protecting
scientific Standing. It also retained paused, blocked, done, and retired rows
inside a catalogue whose only runtime purpose is to identify work available
now.

The durable invariant is narrower. A producer may start work only when the
current tracked index, its declared source inputs, its packet, and the current
repository binding agree exactly. Existing `check`, `next`, `start`, and
Submission revalidation already enforce that boundary.

## Decision

`vela.target-index.v5` is the only current Target Index schema.

- A Frontier-owned domain adapter writes canonical `targets.json` directly.
- Every entry has `presence: "open"`. Unavailable entries are removed.
- `targets.json`, declared inputs, and packets must be unchanged tracked files
  at `HEAD`.
- Inputs must also match their exact source commit and tree.
- Repository transactions continue to rebind the repository and index roots
  automatically without changing target semantics.
- `check`, `next`, `start`, and Submission revalidation remain the runtime
  gates.
- The `target-index seal|repair|inspect` command family, candidate schema,
  write helper, and dedicated CLI tests are deleted.

Git history is the inspection and rollback mechanism. Vela does not create a
parallel maintenance workflow for a derived file.

## Consequences

The trusted product boundary is smaller: a generator produces one reviewable
diff, and Vela either accepts the exact tracked bytes as fresh work advice or
returns no Offer. No derived projection gains authority by passing validation.

Frontiers must update their domain adapters from v4 to v5. This pre-release
transition intentionally has no compatibility parser or migration command.
