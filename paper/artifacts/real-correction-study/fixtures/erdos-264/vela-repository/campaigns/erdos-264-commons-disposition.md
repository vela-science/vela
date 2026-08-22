# Erdős 264 frontier-to-commons disposition

## Disposition

**Retain the current proof as source-local evidence. Do not open a Mathlib or
Formal Conjectures pull request from this artifact in its current form.**

The native destination for a finished theorem is Formal Conjectures, not
Mathlib. The current artifact completes one source-specific declaration but is
not reviewer-ready: it replaces one `sorry` with 319 lines, keeps 33 helper
facts local inside the theorem, and exposes no general declaration or known
maintained consumer.

This is a useful negative foundry result. A native build and an accepted local
Claim establish exact bounded value. They do not make every passing proof a
good library contribution.

## Exact contribution under review

- Source: `google-deepmind/formal-conjectures`
- Source commit: `e6d6b867dc85eec2f88bc47496b4314c623f9f92`
- Source path: `FormalConjectures/ErdosProblems/264.lean`
- Declaration: `Erdos264.erdos_264.parts.i`
- Intended native owner: Formal Conjectures
- Candidate: `artifacts/erdos264-parts-i-proof-repair/264.lean`
- Candidate root:
  `sha256:9ba4b0c8aa144985aac8df40ee070c0ffe4ab7b59915d9b44eb90b42f96935e8`
- Lean: `leanprover/lean4:v4.27.0`
- Mathlib: `a3a10db0e9d66acbebf76c5e6a135066525ac900`
- Import: the source file's existing `FormalConjecturesUtil` import
- Axioms: `Classical.choice`, `Quot.sound`, and `propext`

The accepted local Claim is
`vcl_930b7bb4b4bb11cc3b35de01690ff106ab47c464e828bdafb18d26d3998a1616`.
Verification `vvr_3c05f6340fee38be` binds the exact candidate, source,
toolchain, unchanged declaration signature, unchanged surrounding source,
unlimited-heartbeat contract, native Lean pass, and permitted axiom set. The
Decision at `vev_7abd13c53ee521f6` accepts only that exact formal declaration
and environment.

## Existing API and overlap audit

The candidate uses existing Mathlib summability, geometric-series, order,
interval, and finite-case APIs through `FormalConjecturesUtil`. The pinned
Mathlib tree contains no `IsIrrationalitySequence`, Ahmes-series, or Erdős 264
declaration to extend.

The proof's possible abstractions remain coupled to this problem:

- a gap inequality for powers of two and offsets one through four;
- two source-specific tail sums;
- an interval-cover induction for those four offsets; and
- construction of one bounded integer perturbation with rational reciprocal
  sum.

Extracting those local facts now would add names and review surface without a
second consumer. Keeping them local is the smaller maintenance contract.

Two earlier Formal Conjectures solve proposals, PRs 2488 and 2865, closed on
March 3, 2026. They used a short `research formally solved` shortcut and
predate the merged integer-valued source correction in PR 4289. The current
source and current main branch still retain `by sorry` for `parts.i`. Those
closed proposals do not supply a current compatible proof and should not be
reopened as evidence that this 319-line repair is ready.

## What the evidence establishes

- The exact candidate proves the corrected Formal Conjectures declaration in
  the pinned Lean and Mathlib environment.
- The proof changes no source bytes outside `erdos_264.parts.i` and preserves
  its signature.
- The local Erdős Frontier accepted that bounded Claim through an attributed
  Decision.

## Nonclaims

- This is not a full solution of every Erdős 264 part or informal variant.
- It does not establish statement fidelity beyond the retained formalization.
- It is not a new mathematical discovery.
- It does not show that any local helper belongs in Mathlib.
- It does not establish external review, merge readiness, or maintenance.
- The shared operator, machine, source, Lean kernel, and Mathlib dependencies
  remain explicit.

## Reopening gate

Prepare a Formal Conjectures upstream dossier only after one of these occurs:

1. the proof is reduced enough that a maintainer can review the source-specific
   argument without accepting a 319-line monolith; or
2. a second real theorem needs one of the local lemmas and extraction produces
   a small documented declaration with an obvious native home.

The next pass should try proof minimization and declaration factoring in a
throwaway native branch. It should stop if the result only renames local proof
steps or shifts the same review burden into Mathlib.

## Reproduction

```bash
python3 execution/erdos-264-proof-repair/verify.py \
  --workspace <clean-formal-conjectures-at-e6d6b867> \
  --candidate artifacts/erdos264-parts-i-proof-repair/264.lean \
  --json

vela why . \
  vcl_930b7bb4b4bb11cc3b35de01690ff106ab47c464e828bdafb18d26d3998a1616 \
  --json
```

This document is a source-local reuse disposition. It creates no Claim,
Verification, Decision, external review state, or authority effect.
