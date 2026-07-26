# ADR 0009: Ephemeral producer identities and withdrawal-capability retirement

- Status: Proposed
- Implementation candidate: Canopus `main` after `v0.6.5`
- Protocol effect: None
- Supersedes for new runs: ADR 0005
- Vela compatibility: historical `proposal.withdrawn` events and direct
  producer withdrawal remain protocol-readable

## Context

Canopus gives one bounded worker an isolated Vela home and a temporary
Receipt-bound agent identity. ADR 0005 retained that identity's 32-byte
Ed25519 seed after a successful landing so Canopus could later withdraw the
exact pending proposal.

The retained capability worked as designed, but it is the wrong default
product boundary:

1. a successful non-authoritative run leaves a long-lived private key behind;
2. every pending proposal creates another local secret lifecycle;
3. `inspect` and the CLI gain capability-store semantics unrelated to producing
   or replaying research;
4. withdrawal duplicates a simpler terminal path: the proposal can remain
   pending until Vela policy or a human reviewer decides it; and
5. removal of Canopus should not require a separate inventory of producer
   secrets.

The July 25 audit found five available proposal-scoped seeds retained under
`~/.canopus/capabilities`. Their proposal and Receipt facts already live in
canonical Frontier history. The local seeds add control surface, not scientific
evidence.

## Decision

New Canopus runs use ephemeral producer identities.

After landing one Receipt and reproducing the exact result from a clean clone,
Canopus:

1. records the proposal, Receipt, verifier, artifact, Git, event, and
   scientific-state roots in the existing run evidence;
2. destroys the isolated Vela home and its agent seed;
3. retains no withdrawal key, capability manifest, or second control channel;
   and
4. reports the proposal's observed standing without implying authority.

Remove from the current product:

- `~/.canopus/capabilities` creation;
- `canopus.withdrawal-capability.v1` writers and readers;
- `canopus withdraw`;
- withdrawal availability in `canopus inspect`;
- the Canopus Vela-client withdrawal wrapper; and
- the capability and product-withdraw modules and tests.

Retain:

- immutable historical source, release evidence, ADR 0005, and activity records
  containing `withdrawal_capability.retained`;
- Vela's protocol parsing and verification of historical
  `proposal.withdrawn` events;
- Vela's direct producer-withdrawal primitive for a producer that still holds
  its own live key; and
- every canonical proposal, Receipt, artifact, verifier result, and decision.

This ADR removes Canopus key custody. It does not delete, accept, reject,
withdraw, or otherwise change any proposal.

## Migration

Before deleting an existing local capability store:

1. enumerate only its public manifests;
2. record the manifest byte roots and proposal IDs in a local cleanup ledger;
3. verify every proposal and Receipt still exists in its canonical Frontier;
4. delete the private-key files and then the non-canonical local store; and
5. verify the four source Frontiers remain clean and byte-identical.

The cleanup ledger contains no seed, private-key digest, environment value,
home-path detail, or authority claim. Canonical proposal and Receipt bytes are
the durable record.

Available capabilities are not converted to `consumed`: doing so would falsely
claim withdrawal or a human terminal decision. They are retired by product
policy and removed locally.

## Replay and compatibility

Run v0/v1 records do not embed withdrawal secrets or capability manifests and
replay unchanged. The historical activity event type remains accepted so old
activity chains retain their exact digest.

Released Canopus `v0.3.0` through `v0.6.5` remain available for exact historical
source replay. A new version no longer offers `withdraw` and no longer reports
local withdrawal availability.

Vela remains the sole authority boundary. A pending proposal stays pending.
Verifier success, key deletion, capability retirement, and Canopus removal
cannot change scientific standing.

## Adversarial cases

- A run must fail if its isolated Vela home cannot be removed after successful
  landing and reproduction.
- No run, evidence bundle, publication projection, environment variable, or
  installed package may contain a producer seed.
- Historical activity parsing must not restore, infer, or request a key.
- A missing local capability must never be reported as withdrawal, rejection,
  or acceptance.
- Removing Canopus must leave every canonical Frontier root unchanged.
- A future request for producer withdrawal must use a currently held producer
  key through Vela directly; Canopus must not recreate or recover one.

## Acceptance gate

```bash
bun install --frozen-lockfile
bun run check
bun run pack:check
git diff --check
```

Focused tests must prove:

- successful landing removes the isolated Vela home;
- no capability directory or producer private key is retained;
- the installed package omits the capability module;
- ordinary and subcommand help omit `withdraw`;
- `inspect` remains a read-only run projection without capability state;
- historical `withdrawal_capability.retained` activity bytes still parse; and
- released-Vela composition retains Defer, accepted-event delta zero, artifact
  binding, and clean-clone replay.

Acceptance requires removal of the preexisting local seed store with a public
manifest-root ledger and zero canonical Frontier changes.

## Consequences

Canopus returns to one responsibility: produce bounded, replayable evidence and
hand it to Vela. It owns no durable signing identity and no post-landing
authority-adjacent lifecycle.

The tradeoff is deliberate: after key destruction, Canopus cannot withdraw its
old proposal. Pending work remains inspectable and can be decided through the
ordinary Vela authority path. This is smaller, safer, and easier to explain
than retaining one secret per proposal indefinitely.
