# T2 replay and receipt qualification

Recorded: 2026-08-26, America/Toronto.

```text
Lane: T2 Receipts + Replay
Branch: campaign/compose1-replay
Phase-0 root: 6a0f5adee55f9c50e7e154ac8d118662809d3323
Semantic change required: no
Protocol objects changed: none
Campaign anomaly: none
Scientific experiments run: 0
```

## Result

The current implementation qualifies deterministic scientific-state replay and
operation-typed, content-addressed evidence binding over the one existing
kernel. No replay implementation fix, wire change, receipt object, state
engine, or runner is needed.

One evidence gap was real: current documentation did not explicitly separate
state replay from native computational or physical rerun, and the CLI lifecycle
suite did not exercise missing/corrupt retained Artifact bytes or source-owned
Review Method availability/drift at that boundary. The smallest change is one
continuity section plus assertions in the existing clean-clone lifecycle test.

The implemented equality is not between two mutable Standing stores because
there is no second store. Accepted and pending Claim indexes are canonical
repository-manifest fields covered by the repository root. Strict replay
validates them against admitted Events, exact objects, authority history, and
Git ancestry:

```text
root(strict replay of authoritative history and exact objects)
  == current canonical repository root, including its Standing indexes
```

## Code-path audit

The audit followed one read path rather than constructing a second reducer:

1. `cmd_replay_repository` calls `verify_repository_at(..., true)` and reports
   the verified repository root and exact object counts.
2. `verify_repository_at` loads every manifest-bound Claim, Proposal,
   Submission, Verification Record, Withdrawal, and Artifact through its
   declared full root, checks canonical parsers and cross-object references,
   and then enters authority verification.
3. `verify_repository_authority` verifies genesis, the independently pinned
   sequence-one record, the contiguous signed authority chain, semantic Event
   coverage/linkage, repository-manifest deltas, retained record coverage, and
   current Claim/Proposal Standing parity.
4. `verify_routine_evidence_ancestry` replays Git-retained manifest versions
   between signed Decision checkpoints and permits only append-only routine
   evidence overlays that cannot alter accepted Standing or authority.
5. `projection_review_method` separately resolves a source-owned Review Method
   against the exact `method.environment_root`. This resolution is not an input
   to the Standing reducer and performs no method execution.

## Exact audit matrix

| Required surface | Exact evidence | Result | Boundary or limitation |
| --- | --- | --- | --- |
| Genesis, same checkout | `genesis::fresh_current_repository_replays_from_a_clean_clone` runs replay twice and compares `repository_root` | PASS | Verifies current genesis only; predecessor layouts require their historical readers |
| Complete fresh clone | The same test and `review_acceptance` clone committed state and reproduce the original roots; partial/promisor storage is refused without writes | PASS | Requires complete ordinary Git history and the independent trust pin |
| Clean generated state | `review_acceptance` derives the repository projection twice, from two checkout paths, and at the exact pre-Decision commit | PASS | Projection is read-only and rebuildable; it is not Standing authority |
| Accepted Standing | `review_acceptance` proves Verification leaves Standing unchanged, the authorized Decision admits linked Events, and clean-clone replay matches the resulting root | PASS | Scientific truth is not established by replay |
| Correction history | `correction_impact` admits two successive `corrects` transitions, retains both predecessors, and repeats an identical multi-generation projection | PASS | Derived correction-impact propagation is separate from authoritative Standing |
| Supersession history | The authority-chain fixture replays retained `claim.superseded` Events; the independent correction fixture binds `supersede_claim` and reproduces projection root `sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6` | PASS | `corrects` and `supersedes` share the admitted `claim.revise` path but retain distinct relation text |
| Rejection-preserving history | `disposable_rejection_lifecycle` retains the failed Verification and rejected Proposal, replays zero accepted/pending Claims, and leaves the Claim `unassessed` with Proposal status `rejected` | PASS | Rejection changes Proposal status, not accepted Standing |
| Divergent authority histories | `portable_divergence` replays identical Submission/Claim bytes under independent accept and reject Decisions, including frozen bundle roots and exact authority receipts | PASS | Standing is repository-local, not transported global truth |
| Resolvable canonical Artifacts | `genesis::current_submission_and_verification_replay_without_changing_accepted_state` removes the transport copy, resolves retained input/output Artifacts by full SHA-256, and replays them from a clean clone | PASS | Exact bytes do not supply native interpretation |
| Missing canonical Artifact | The extended `genesis` clean-clone test removes the retained Artifact and `vela replay` fails closed | PASS | Nothing can reconstruct bytes no complete copy retains |
| Corrupt canonical Artifact | The same test substitutes bytes and replay refuses the declared-root mismatch; restoring exact bytes restores the same repository root | PASS | SHA-256 integrity is byte identity, not scientific validity |
| Missing Review Method | The extended test removes the source-owned method; state replay still matches while projection reports every binding `unavailable` | PASS | Method availability is a rerun/read-resolution fact, not a Standing input |
| Changed tool/environment identity | The extended test substitutes method bytes; projection refuses `Review Method root drift`, while state replay continues to validate the exact historical method identity recorded in the Verification | PASS | Vela neither acquires nor runs the named tool/environment |
| Changed authority metadata | The clean-room authority reader passes the six-record chain and refuses all 13 frozen mutations, including trust anchor, signature, chain position, head, key, model membership, canonical bytes, Event, preimage, write set, and terminal Claim root changes | PASS | The fixture is verification-only and does not create a Decision |
| State replay versus computational rerun | `docs/CONTINUITY.md` now defines the operation-typed reconstruction/rerun inventory and states that Submission `replayability` is producer disclosure | PASS | No computation, model call, proof check, instrument action, assay, or physical replication was run in T2 |

## Typed evidence inventory

This is an operation inventory, not a universal receipt ontology:

- Submission binds authenticated producer bytes, a requested change, exact
  canonical Artifact roots, caveats, replayability disclosure, producer checks,
  verification requirements, and opaque source provenance.
- Verification binds an exact Claim/Submission/Proposal subject, input and
  output Artifact IDs, verifier identity, scope and nonclaims, outcome, and one
  method profile/path/environment root.
- Decision authority binds the authenticated principal, authorization request
  and result, exact read/write roots, admitted Events, and repository before and
  after roots.
- Git retains complete object history and exact source-owned files. A Review
  Method can be checked against its bound root when present, but its tools,
  models, workflow state, credentials, instruments, and physical conditions
  remain native and source-owned.

## Verification run

All commands ran from `campaign/compose1-replay` after the scoped changes.

```text
cargo test --locked -p vela-cli --test genesis --test review_acceptance \
  --test disposable_rejection_lifecycle --test correction_impact \
  --test portable_divergence
  PASS: correction_impact 1, disposable_rejection_lifecycle 1,
        genesis 4, review_acceptance 1
  NOTE: portable_divergence is feature-gated and correctly ran 0 in this command

cargo test --locked -p vela-cli --features test-support \
  --test portable_divergence
  PASS: 2 passed

uv run --project conformance --locked python \
  conformance/verify_authority_chain.py
  PASS: 6 authority records, 11 Events, 13/13 negative vectors refused
  terminal repository root:
  sha256:45640c5eea54693df444eada6dd1a7c1f5a4b4ef266fddf79cf51d083233ebba

uv run --project conformance --locked python \
  conformance/verify_correction_impact.py
  PASS: correction-impact projection root
  sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6

cargo fmt --all -- --check
  PASS

cargo clippy --locked -p vela-cli --all-targets -- -D warnings
  PASS

git diff --check
  PASS
```

## Files changed

- `crates/vela-cli/tests/genesis.rs`: narrow clean-clone assertions for
  missing/corrupt canonical Artifacts and Review Method availability/drift.
- `docs/CONTINUITY.md`: public distinction between exact state replay and
  source-owned computational or physical rerun, with the operation-typed
  evidence inventory.
- `docs/campaigns/vela-compose-1/README.md`: lane-report navigation.
- `docs/campaigns/vela-compose-1/T2_REPLAY_REPORT.md`: this audit record.

## Limitations and supervisor disposition

- This is Level-1 engineering qualification only. It is not external
  validation, a cumulative-science result, or an anomaly.
- The new test and documentation are branch-local until supervisor integration;
  the signed `v0.977.4` binary and published docs are unchanged.
- Strict replay validates retained current state. It cannot recreate native
  activity that was not retained, and it intentionally does not execute a
  retained method.
- `replayability: exact` remains an authenticated producer expectation, not a
  Vela Verification result or a guarantee that dependencies remain available.
- No semantic conflict or required protocol change was found. T2 therefore
  requests ordinary supervisor review; it does not request a semantic decision.
