# Portable-waist and interoperability campaign

Status: **Cut A largely landed; Cut B and Cut C have not begun.** Cut B waits on
ADR 0035, which is still Proposed. Cut C waits on recomputing historical
authorization against the closed profile, and the evaluator is still called only
from tests while `cedar-policy` remains a dependency of the active writer. The
two blocked cuts are the reason this document is still open; neither is stalled
on anything written here.

## Objective

Replace commodity edge machinery with exact maintained standards while keeping
Vela's scientific meaning, correction, Decision, and Standing semantics
custom and Frontier-local.

## Current baseline

- RFC 8785 JCS and SHA-256 are current.
- Authority records use a DSSE 1.0.2-compatible envelope.
- Eight JSON Schema 2020-12 documents cover the authority envelope, Submission,
  Verification Record, Proposal Withdrawal, Claim Record, Proposal, repository
  origin, and `vela.status.v4`. This line said four for as long as `schemas/`
  has held eight.
- Current v1 fixture bytes and roots are frozen.
- The closed Vela Authorization Profile exists in shadow mode.
- Submission, Verification, and Withdrawal still use bespoke v1 signature
  preimages; historical authorization cannot yet be fully recomputed.

## Cut A — documentation and conformance

This cut may proceed without a core release:

- keep schemas checked against positive and negative fixtures;
- inventory every external JSON read/write surface;
- document loss, versioning, format assertion, and authority effect;
- freeze independent readers and emitters; and
- reject misleading support claims for MCP, A2A, packages, or hosted writes.

What landed: `docs/interop/scientific-state-profile.md` states the seven
contracts an outside implementation must satisfy and pairs each with its check.
`conformance/readers/python` and the two emitters,
`conformance/emitters/javascript.mjs` and `conformance/emitters/python.py`, are
frozen and run on every CI run. The reader half is still one language: the
canonicalization vectors run in Rust and Python, and the emitters do not read
the vector corpus at all. Contract 4, authority, still has no
language-independent vector, which the profile states about itself.

The fourth item read "freeze independent JavaScript and Python readers". What
exists in JavaScript is an emitter; `scripts/ecosystem-status.py` declares
`conformance/readers/javascript` absent, so the old wording asked for a surface
whose absence is checked.

## Cut B — DSSE v2 protocol migration

This is a separate core-release campaign and begins only after ADR 0035 is
accepted with:

- shared DSSE envelope fixtures across authority, Submission, Verification,
  and Withdrawal;
- distinct payload types and closed payload schemas;
- standard and URL-safe base64, optional unauthenticated `keyid`, ignored
  unknown envelope fields, and fail-closed threshold semantics;
- exact old/new repository migration roots and rollback plan;
- independent Rust, Python, and JavaScript verification; and
- an explicit current-epoch cut.

No v1 object is silently reinterpreted as v2.

## Cut C — authorization history

This list is about evidence, and all of it could pass while Cedar stayed
unremovable. The blocker is a signature: `vela-science/math` retains a policy
bundle naming the pinned evaluator, and the reader refuses any bundle that
disagrees with the compiled-in constants. Retiring Cedar therefore needs a
policy-bundle rotation on the live authority first, and no rotation writer
exists. ADR 0042 states the sequence.

Before deleting Cedar or changing the writer:

- retain exact model, request, entity, engine, and profile inputs;
- recompute every historical Allow result with the closed profile;
- prove parity and negative boundary cases across all canonical Frontiers; and
- replay every Frontier from a clean clone.

## Optional read edge

An MCP, A2A, or other protocol edge may start only for a named consumer whose
task is not served by CLI JSON or static HTTP. The first edge is read-only,
root-aware, source-local, removable, and exposes no authority credential or
Decision tool. Verify the live primary specification before implementation.

## Package gate

The exact Lean replay contract is the first package candidate. It remains
source-local until two maintained consumers produce net deletion. No package
CLI, resolver, Registry, or hosted index is built before that evidence.
The first experiment failed the net-deletion gate and is retained in Git
history; it does not remain an active campaign.

## Stop conditions

Stop a cut on root drift, ambiguous migration, incomplete historical replay,
authority widening, or a consumer that does not remove maintained complexity.
