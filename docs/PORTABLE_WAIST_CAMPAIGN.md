# Portable-waist and interoperability campaign

Status: **Complete, 2026-08-09.** All three cuts landed, and Cut B and Cut C
shipped together as one wire break because separately each would have forced
another `vela-science/math` re-genesis. The operator then completed that one
re-genesis under Vela 0.972.1 and re-admitted only the bounded corrected Claim.
The current Math head strictly replays with no blocker; the signed 0.971.0
predecessor remains retained and contributes no Standing to the new genesis.

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
and JavaScript. Contract 4, authority, is held by the generated request and
evaluation schemas plus the language-independent authorization parity corpus.

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
Math 0.972.1 genesis. Independent verification is Rust, Python, and JavaScript:
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

The evidence this cut required, and where it is:

- retain exact model, request, entity, engine, and profile inputs —
  `conformance/fixtures/epoch1/authorization-profile-parity.json`;
- recompute every historical Allow result with the closed profile —
  `crates/vela-authority/tests/authorization_profile_parity.rs`, which drives
  `evaluate_authorization_v1` over all seven retained transactions;
- prove parity and negative boundary cases across the retained epoch-1 corpus;
  the same test checks seven negative cases for their exact fail-closed
  reasons; and
- replay the current authority from a clean clone, exercised by
  `vela-science/math@130fc283b99b8c55dea51b5f8f959a6c33a679f6`, yielding
  Repository root
  `sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc`.

The deletion followed: `cedar-policy` is out of both manifests, `engine_pin.rs`
is gone, `PolicyBundleV1` is `AuthorizationModelV1` naming no engine, and
`AuthorityRecordV1` carries an `AuthorizationEvaluationV1` that strict history
recomputes instead of trusting.

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
