# ADR 0005: Retained per-proposal withdrawal capabilities

- Status: Accepted
- Supersession: retained as historical `v0.3.0` behavior; ADR 0009 proposes
  retiring the Canopus key-retention and withdrawal product surface
- Release gate: satisfied 2026-07-17 by released Vela `v0.901.0`, the final
  protected-signer/root audit, focused capability tests, and package verification
- Target release: Canopus `v0.3.0`
- Depends on: Vela ADR 0011 and released Vela `v0.901.0`
- Authority effect: none beyond the Receipt-bound producer withdrawal defined
  by Vela; Canopus never gains human review authority

## Context

Canopus correctly destroys its isolated Vela home after a run. That home also
contains the auto-minted agent seed which Receipt v1 binds to the landed
proposal. Destroying the seed means the producer cannot later use Vela's
`proposal.withdrawn` event to close an abandoned or superseded proposal.

Keeping the whole run home would be worse. It expands secret lifetime, retains
unrelated state, and risks mounting credentials into a later worker or
verifier. The useful capability is exactly one 32-byte producer seed for
exactly one pending proposal.

## Decision

After all of these have succeeded:

1. the worker produced a bounded candidate;
2. the separate frozen verifier passed;
3. Vela landed one Receipt-bound pending proposal;
4. exact Receipt/proposal roots were rechecked; and
5. clean-clone reproduction matched,

Canopus copies only that proposal's producer seed into:

```text
~/.canopus/capabilities/<proposal-id>/private.key
```

The file is mode `0600` inside a mode `0700` directory. A public
`canopus.withdrawal-capability.v1` manifest binds:

```text
state: available | consumed
proposal_id and full proposal root
Receipt root
identity-binding ID
actor and public key
frontier path and final Git/Vela roots
exact strict-signal baseline retained by the successful run
pinned Vela binary, version, and SHA-256
created_at
optional consumed_at and consumed_reason
```

The manifest is a local capability inventory, not a Vela object, scientific
claim, verifier result, or authority certificate. The secret and manifest are
never mounted into the Codex worker or verifier, forwarded through environment
variables, or included in run evidence. The activity log records only that a
proposal-scoped capability was retained.

`canopus inspect` derives whether withdrawal is available from the local public
manifest and secret-presence invariant.

Canopus adds:

```bash
canopus withdraw [frontier] [--run <run.json|latest>] --reason <text>
```

The command:

1. verifies the run, closed manifest, current proposal bytes, Vela-canonical
   proposal and Receipt roots, the Receipt identity's self-signature, secret
   permissions, and pinned Vela binary;
2. requires the current strict classification to equal the exact baseline
   retained by the successful run;
3. clones the exact clean source head into a disposable workspace;
4. installs the one producer seed into a disposable isolated Vela home;
5. calls `vela review withdraw` for the full proposal ID;
6. requires no new strict blocker, a new event/Git root, and exact clean-clone
   replay;
7. fast-forwards the still-clean source checkout; and
8. deletes the secret and atomically records the public manifest as consumed.

Pending review is a successful Canopus outcome and is never auto-withdrawn.
Withdrawal requires an explicit command and reason.

If the source proposal is already accepted, applied, rejected, or
revision-requested, Canopus does not invoke Vela or mutate the frontier. It
deletes the obsolete producer secret and records `human_decision_observed` only
after Vela replay and the retained strict baseline verify exactly one matching
signed terminal event. If the proposal is already validly withdrawn, it
records `withdrawn`. A mutable proposal-status field alone never consumes the
secret. Any mismatch, dirty source, binary drift, strict-signal drift, failed
replay, publication race, or invalid capability stops with the secret retained.

Canopus-owned manifests retain their historical newline-terminated canonical
format. Vela proposal and Receipt roots use Vela's compact canonical JSON
bytes, without that newline. Mixing those digest domains is a hard failure.
Capability manifests are closed records; unknown fields, malformed roots,
permission drift, path escape, altered producer bindings, and a secret that no
longer derives the bound public key fail closed.

## Alternatives

### Keep the entire Vela run home

Rejected. It retains unrelated mutable state and expands the secret surface.

### Put the seed in run evidence

Rejected. Run evidence is portable and inspectable; a producer secret is
neither. The public manifest is sufficient for inspection.

### Auto-withdraw after a timeout or failed review

Rejected. Pending review is not failure. Time and queue pressure do not grant
Canopus authority to close producer work.

### Ask a human to withdraw producer work

Rejected. It adds human queue labor to a capability the producer already
proves through Receipt v1.

## Acceptance gate

Canopus `v0.3.0` requires:

```bash
pnpm check
pnpm build
node --test dist/tests/withdrawal-capability.test.js
node --test dist/tests/product-withdraw.test.js
node --test dist/tests/integration/released-vela.test.js
pnpm pack --dry-run
```

The released-Vela test must exercise a real deferred proposal, retain only the
proposal-scoped seed, withdraw in a disposable clone, prove accepted-state
delta zero and strict replay, fast-forward the source, and consume the secret.
The release stops on secret leakage into evidence, worker/verifier access,
wrong-proposal use, false strict pass, accepted-state change, or replay drift.
