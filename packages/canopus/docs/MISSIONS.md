# Missions and profiles

Mission v1 is the current advanced execution contract. A profile selects one
bounded target, packet, objective, worker boundary, verifier capsule, result
contract, and budget. Preparing a Mission binds the exact Git and Vela roots
plus the Vela, Codex, container, packet, profile, and capsule identities.

## Current lifecycle

```text
profile → mission → run → replay → optional export → optional submit
```

Current runnable profiles are deliberately few:

- `erdos1056-k15-10429201-10429400` performs one bounded deterministic
  computation; and
- `formal-erdos-505-test-dim-one` produces one raw Lean term and checks it in
  the exact source-bound Lean 4.27.0 environment.

The Formal profile's kernel and axiom result is a scoped Verification input.
It does not establish statement fidelity or scientific Standing.

Repair missions bind both `parent_candidate` and `repair_reason`. They must be
invoked with `--repair-from <exact-candidate-file>`. Canopus verifies that the
file bytes match the full parent root, stages them at the single contracted
artifact path, and records `repair.input_bound` before the worker starts. A
hash without the corresponding bytes is not a repair handoff.

- A **Run** is local orchestration evidence. It never mutates a frontier.
- A successful Run may **export** one authenticated Vela Submission.
- **Submit** is a separate explicit action that asks Vela to register the
  Submission as a pending Proposal.
- Mechanical verifier output is not a Vela Verification Record.
- Neither Canopus nor its verifier creates a Decision, Event, or Standing.

Worker outcomes remain simple:

- `success`: every declared Artifact exists and the Run may proceed to the
  frozen verifier;
- `null`: the bounded attempt produced no valid candidate;
- `failed`: execution was incomplete or disqualified.

Only a verifier-passing `success` Run with at least one Artifact, one caveat,
and a full `vela.execution-binding.v1` can be exported.

## Isolation

The Codex worker receives only the exact packet and bounded writable workspace.
The full source checkout, host home, Vela custody, authentication files, and
verifier remain outside command-readable paths. The verifier runs separately
with network and writes denied. A clean-clone verifier replay must match before
the Run completes.

## Roles

The retained roles change the prompt, not the trust boundary:

- `producer` constructs a candidate or records a bounded null result;
- `adversary` seeks a concrete counterexample or scope failure;
- `verifier` checks correspondence while the frozen executable remains the
  mechanical verifier;
- `fidelity` checks that prose does not outrun the retained bytes.

Each role produces its own Run. No role inherits pending or merely verified
work as accepted state.

## Historical Mission v0

Mission v0 remains readable for immutable historical replay. The current branch
does not prepare or execute new v0 product missions. Use the release that
created a historical Run when exact old behavior is required.
