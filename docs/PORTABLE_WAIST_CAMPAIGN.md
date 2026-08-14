# Portable-waist and interoperability campaign

Status: **Complete, updated 2026-08-14.** All three cuts landed. The current
`math-coh-00` Repository was created under Vela 0.975.1 and deliberately
re-admits only two bounded current Claims: the corrected Erdős 321 successor
and the scoped Erdős 887 cache-replay result. Its four-record authority chain
strictly replays. Pre-genesis Git history is a development rollback point, not
a shipped protocol reader or Standing input.

## Objective

Replace commodity edge machinery with exact maintained standards while keeping
Vela's scientific meaning, correction, Decision, and Standing semantics
custom and Repository-local.

## Current baseline

- RFC 8785 JCS and SHA-256 are current.
- Every signed Vela object is a DSSE 1.0.2-compatible envelope, produced and
  read by one implementation, `crates/vela-protocol/src/kernel/dsse.rs`.
- Twelve generated JSON Schema 2020-12 documents cover the current portable
  objects and read contracts, including the DSSE envelope, Submission,
  Verification Record, Proposal Withdrawal, authorization request and result,
  Repository Profile and origin, and `vela.status.v4`.
- Current fixture bytes and roots are frozen.
- The closed Vela Authorization Profile is the only evaluator. Nothing runs in
  shadow.
- Submission, Verification Record, and Proposal Withdrawal are DSSE envelopes.
  There are no bespoke signature preimages left, and strict history recomputes
  every authorization rather than reading a retained one.

## Cut A — documentation and conformance

This cut may proceed without a core release:

- keep schemas checked against positive and negative fixtures;
- inventory every external JSON read/write surface;
- document loss, versioning, format assertion, and authority effect;
- freeze independent readers and emitters; and
- reject misleading support claims for MCP, A2A, packages, or hosted writes.

What landed: `docs/interop/scientific-state-profile.md` states the seven
contracts an outside implementation must satisfy and pairs each with its check.
`conformance/readers/python`, `conformance/readers/javascript`, and the two emitters,
`conformance/emitters/javascript.mjs` and `conformance/emitters/python.py`, are
frozen and run on every CI run. Canonicalization vectors run in Rust, Python,
and JavaScript. Contract 4, authority, has two deliberately distinct evidence
paths: the retained epoch-1 corpus is Rust-read evaluator compatibility across
the vocabulary migration, while
`conformance/fixtures/authority/math-coh-00/` and
`verify_authority_chain.py` are the current retained four-record
seven-Event language-independent signed-chain vector from an explicit external
anchor. It also binds the signed Erdős 321 predecessor-to-correction transition
and the two-Claim terminal state.

The JavaScript reader is deliberately narrower than Python: it independently
checks RFC 8785 bytes and SHA-256 roots, while repository reconstruction stays
in the Python reader. The emitters remain separate implementations that build
DSSE objects rather than importing either reader.

## Cut B — DSSE protocol migration

**Landed 2026-08-09.** ADR 0035 is Accepted; its implementation note records
what shipped. The list below is what the cut was required to deliver:

- shared DSSE envelope fixtures across authority, Submission, Verification,
  and Withdrawal;
- distinct payload types and closed payload schemas;
- standard and URL-safe base64, optional unauthenticated `keyid`, ignored
  unknown envelope fields, and fail-closed threshold semantics;
- exact old/new repository migration roots and rollback plan;
- independent Rust, Python, and JavaScript verification; and
- an explicit current-epoch cut.

No predecessor object is silently reinterpreted under the current contract:
every payload type and schema tag moved with the signature, so an old object
fails to parse rather than parsing differently.

All of it landed, including the explicit current-epoch cut exercised by the
current Math 0.975.1 genesis. Independent verification is Rust, Python, and JavaScript:
`conformance/emitters/python.py` and
`conformance/emitters/javascript.mjs` construct DSSE envelopes from first
principles and reproduce the frozen fixture bytes exactly.

## Cut C — authorization history

**Landed 2026-08-09.** The blocker this section named was a signature:
`vela-science/math` retained a policy bundle naming the pinned evaluator, so
retiring Cedar appeared to need a policy-bundle rotation on the live authority
first, and no rotation writer existed. ADR 0042 stated that sequence. Cut B
dissolved it — a wire break re-genesises `math`, and genesis mints the
authority chain and its model fresh, so no retained bundle survives to
contradict a Cedar-free reader. ADR 0042 is Superseded and the rotation writer
was never needed.

Before Cedar was deleted, a migration-only corpus retained the exact epoch-1
model, request, entity, engine, and profile inputs. The closed profile
recomputed every historical Allow and seven negative boundary cases. That
one-time migration evidence was removed after the current authority contract
replaced it. Current conformance now:

- verifies the current four-record, seven-Event Math chain independently from
  an explicit sequence-one anchor; `verify_authority_chain.py` checks DSSE
  signatures, continuity, authorization, signed deltas, the correction
  transition, and bounded terminal state with thirteen stable negative cases
  and no Vela, Rust, Git, or network; and
- replay the current authority from a clean clone, exercised by
  `vela-science/math@08a0e6d327e1ae9937ab2e0e5002192815eac69a`, yielding
  Repository root
  `sha256:3e2236510923277c1e363d2d28c3d84d86a1d698bafd576b79308b18ae0cf0d2`.

The deletion followed: `cedar-policy` is out of both manifests, `engine_pin.rs`
is gone, `PolicyBundleV1` is `AuthorizationModelV1` naming no engine, and
`AuthorityRecordV1` carries an `AuthorizationEvaluationV1` that strict history
recomputes instead of trusting.

The independent vector does not claim that current CLI read paths load the
local authority pin, nor that the production history verifier enforces every
fixture-level positive cross-link.

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
