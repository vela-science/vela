# T2 — Receipts + Replay worker contract

Read root `AGENTS.md`, current protocol/architecture/continuity/evidence and
repository-boundary docs, and all campaign controls before work.

## Objective

Qualify typed content-addressed evidence references and deterministic scientific
state replay over the one existing kernel. Explicitly distinguish deterministic
state replay from stochastic or physical computational rerun.

## Ownership

- current replay, integrity, read, and receipt-resolution paths in
  `crates/vela-cli` and their direct protocol interfaces;
- focused replay/receipt conformance tests;
- `docs/CONTINUITY.md` or `docs/EVIDENCE.md` only if the public guarantee is
  incomplete;
- a lane report under this campaign directory.

Do not change Submission/Verification/Proposal/Decision/Event/Standing
semantics, build a second state engine, add a workflow runner, or add a universal
receipt ontology. Native PROV/RO-Crate/workflow/instrument/model records remain
source-owned; Vela copies only decision-relevant exact bindings.

## Required audit

Prove or identify the smallest missing behavior for replay from genesis on the
same checkout, fresh clone, clean generated state, correction history,
supersession history, rejection-preserving history, resolvable receipts,
missing/corrupt receipts, changed tool/environment identity, and changed
authority metadata.

Target invariant:

```text
digest(replay(authoritative history)) == digest(materialized Standing)
```

Document what can be reconstructed exactly versus merely rerun or attempted.
If the current implementation already satisfies an item, add no duplicate
machinery; bind the existing test/evidence.

## Finish

Commit on `campaign/compose1-replay`. Report design/audit matrix, exact files,
tests/commands/results, limitations, and commit. Do not merge.

