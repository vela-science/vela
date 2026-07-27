# Vela conformance vectors

This directory carries the portable test vectors for Vela's current pre-1.0
protocol candidates. Passing them demonstrates agreement on the named byte and
reducer contracts. It does not certify scientific truth, policy authority, or
compatibility with future candidates.

## Reducer contract

Each `fixtures/cascade-fixture-*.json` contains:

- `genesis_findings`: the initial typed state;
- `event_log`: ordered canonical events; and
- `expected_states`: the reducer-owned projection after replay.

An implementation parses the fixture, replays the event log, projects the same
effect rows, and compares them with `expected_states`. The current set contains
16 `fixture_version: 6` fixtures. Its manifest records the byte length and
SHA-256 of every cascade fixture; the verifier refuses drift.

The source of truth for mutation kinds is
`crates/vela-protocol/src/kernel/reducer.rs`'s
`REDUCER_MUTATION_KINDS`. The Rust coverage test derives its obligation from
that constant, so adding a reducer arm without a fixture fails.

Reference implementations:

- Rust: `crates/vela-protocol/src/kernel/reducer.rs`
- Python: `clients/python/vela_reducer.py`
- TypeScript: `clients/typescript/vela_reducer.ts`
- JavaScript current-object emitter:
  `clients/javascript/vela_emit.mjs`

Run the focused checks:

```bash
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
cargo test -p vela-protocol --test current_object_interop
python3 conformance/verify_current_objects.py
python3 conformance/verify.py
```

`verify.py` checks the manifest, runs the Python reducer and canonical-hashing
vectors, then runs the TypeScript reducer when Node supports native TypeScript.
The routine core surface also runs `conformance.test_verify_manifest`, which
proves the preflight rejects both unlisted fixture bytes and duplicate
manifest entries before any reducer executes.

Fixture 19 carries the portable
`vela.frontier-repository-boundary-conformance.v1` contract. Its valid case is
a two-event temporal repository-boundary chain. Its hostile cases require the
Rust, Python, and TypeScript verifiers to reject event-ID or fixed-envelope
drift, empty reasons, malformed timestamps, unsigned or corrupt signatures, a
missing parent, a fork, and anchor-count rollback. Only after that validation succeeds is
`frontier.repository_bound` reducer-neutral.

## Other public vectors

- `gate-vectors.json` pins verifier-attachment gate outcomes.
- `canonical-hashing.json` pins `vela.canonical-json/v1` bytes and digests.
- `attempt-id.json` pins deterministic attempt identifiers.
- `decision-binding.json` pins decision preimages and their consumed roots.
- `fixtures/permit-shadow-v1.json` freezes the three-case Sidon shadow
  experiment that proves AcceptancePolicy v0.1 cannot distinguish the intended
  packet/profile/capsule from same-class verifier or target substitution. Rust
  and Python both require v0.2 to Permit only the intended full-root binding.
- `fixtures/policy-scoped-producer-credential-v1.json` freezes the live Sidon
  Receipt identity binding and its full credential root. Rust and Python retain
  v0.2 registry semantics, require v0.3 to match the exact scoped credential,
  and prove that global registration cannot bypass a v0.3 allowlist.
- `fixtures/exact-witness-floor-v1.json` freezes the retained Vela-native
  witness, full byte root, exact lower-bound claim, claim-substitution cases,
  and corrupted-witness case used by AcceptancePolicy v0.2. Rust and Python
  independently rederive the same verifier and claim-fidelity outcomes.
- `fixtures/principal-capability-v1.json` freezes one stable human
  issuer-subject identity, one short-lived agent capability, one attenuated
  workload child, and the bearer-free verified claim retained by an authority
  record. Rust generates the vector. An independent Python verifier rederives
  every content address and rejects identity inference from email, human
  capability subjects, human-only actions, lifetimes over 24 hours, delegation
  broadening, parent substitution, bearer-token retention, and revocation.
  The capability is an immutable claim profile, not a new token format:
  OAuth/OIDC, SciTokens, GitHub App, or local credentials remain replaceable
  runtime adapters.
- Authority-history records retain
  `vela.authentication-observation.v1`, not a cookie, bearer token, provider
  assertion, or raw session ID. The observation binds the exact principal and
  issuer-subject, closed method and assurance, a full non-secret session root,
  authentication/observation/expiry times, presence and verification facts,
  recent-recovery context, and an optional revocation reference. Independent
  checks reject bearer retention, identity substitution, validity beyond 24
  hours, and revoked sessions.
- `pre-adr-0003-replay.json` freezes every canonical `.vela` byte from one
  pre-ADR 0003 frontier plus its strict replay roots and counts. The focused
  CLI integration test replays a temporary copy; it performs no signing,
  network access, external Lean invocation, or scientific decision.
- `spec-surface.v1.json` lists the narrow public schema and command surface.
- `vela_v09_sidon_kernel_fixture.py` and its JSON fixture exercise append,
  restrict, and observe on a small Sidon instance.
- `vela_no_hidden_state_check.py` and its pass/fail fixtures require every
  authoritative displayed value to have one replayable observation packet.

These executable vectors are protocol-surface checks. Formal claims live in
the separately scoped Lean sources and are not implied by a vector pass.

## Extending the set

When a reducer mutation kind changes:

1. update the fixture builders in
   `crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs`;
2. run that focused Rust test to regenerate the affected fixtures;
3. copy the generated fixtures into `conformance/fixtures/`;
4. regenerate `fixtures.manifest.json`;
5. remove `fixtures.manifest.sig.json` in the same change, because the prior
   human signature no longer binds the current bytes;
6. run `python3 conformance/verify.py`; and
7. leave the new manifest explicitly unsigned until a human chooses to run
   `vela sign conformance/fixtures/fixtures.manifest.json` in a separate
   signing ceremony.

The public core gate rejects a present signature that does not verify over the
exact current manifest. Git history preserves superseded signatures; they do
not remain in the active signature slot.

The fixtures are licensed under the repository's Apache-2.0 OR MIT terms.
