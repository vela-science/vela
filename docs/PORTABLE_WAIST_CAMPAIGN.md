# Portable-waist and interoperability campaign

## Objective

Replace commodity edge machinery with exact maintained standards while keeping
Vela's scientific meaning, correction, Decision, and Standing semantics
custom and Frontier-local.

## Current baseline

- RFC 8785 JCS and SHA-256 are current.
- Authority records use a DSSE 1.0.2-compatible envelope.
- Four JSON Schema 2020-12 documents cover the authority envelope, Submission,
  Verification Record, and Proposal Withdrawal structures.
- Current v1 fixture bytes and roots are frozen.
- The closed Vela Authorization Profile exists in shadow mode.
- Submission, Verification, and Withdrawal still use bespoke v1 signature
  preimages; historical authorization cannot yet be fully recomputed.

## Cut A — documentation and conformance

This cut may proceed without a core release:

- keep schemas checked against positive and negative fixtures;
- inventory every external JSON read/write surface;
- document loss, versioning, format assertion, and authority effect;
- freeze independent JavaScript and Python readers; and
- reject misleading support claims for MCP, A2A, packages, or hosted writes.

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

## Stop conditions

Stop a cut on root drift, ambiguous migration, incomplete historical replay,
authority widening, or a consumer that does not remove maintained complexity.
