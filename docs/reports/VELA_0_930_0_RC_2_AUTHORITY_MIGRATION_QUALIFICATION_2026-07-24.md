# Vela 0.930.0-rc.2 authority-migration qualification

Date: 2026-07-24  
Scope: proposed ADR 0020 candidate, read-only active-Frontier replay, and
repository-authority migration fixtures  
Decision: **qualified as a corrected source candidate; no active Frontier
migration authorized**

## Why rc.2 exists

`v0.930.0-rc.1` passed the synthetic and composed migration suites but failed
the first read-only exercise against the active Formal and Erdős Frontiers.
The same signed Git anchor derived a different legacy snapshot root in the
product CLI than in a narrow reusable-library build.

The cause was one synthetic source commitment built from ordinary
`serde_json` object bytes. The full CLI dependency graph enables
`serde_json/preserve_order`; the narrow protocol graph does not. Map order
therefore changed the source commitment, source ID, evidence projection, and
snapshot root. This was a protocol-determinism defect, not a Frontier defect.

`rc.2` uses Vela canonical JSON for that identity preimage. The same exact
commitment and source ID are pinned in:

- the narrow `vela-protocol` test graph; and
- the full `vela-cli` graph where `preserve_order` is enabled.

The immutable `v0.930.0-rc.1` tag is retained as a failed candidate and must
not be used for a migration ceremony.

## Clean release build

The candidate was built from a new empty Cargo target:

```text
vela 0.930.0-rc.2
vela        sha256:b1130f5c6ff09fab58bac606b13e3ffdff51ff10a629c3c95fb623f7328da2df
vela-signer sha256:b340dbcc4648f60dbdcc539a63306430e6977f30a834b322eebff2371f25d751
```

`cargo package -p vela-cli --list --allow-dirty` completed with 90 package
entries. Publication was not attempted; this is a source candidate and its
workspace prerelease dependencies are not yet registry releases.

## Active-Frontier read-only replay

The clean release binary ran `vela status <frontier> --json` over the exact
local checkouts. It made no writes.

| Frontier | Git commit | Repository context | Replay | Strict result |
| --- | --- | --- | --- | --- |
| Erdős | `2c751df0c742d66ea6961106da67f9ef8dfb17f8` | pass | reproduced | blocked only by 1,511 `missing_conditions` and 81 `unsigned_registered_actor` signals |
| Formal Conjectures | `478f8932699efcebde85f55c9b8b1a826eba1250` | pass | reproduced | pass, zero blockers |
| Sidon | `825657d7e87618c0aa6fc9af7e3182e05f324750` | pass | reproduced | pass, zero blockers |
| Quantum Codes | `be2723fe07d0e218f0370253cff93a8748690683` | legacy v0.1 | reproduced | blocked only by the pre-existing unsorted Target Index labels |

The rc.1-only `repository_boundary_invalid` signal is gone. Erdős returned
exactly 1,592 existing blockers rather than 1,593. No exemption, temporal
registration, target repair, policy change, or canonical rewrite occurred.

The Sidon checkout already contained user work before this audit. Its tracked
and untracked changes were preserved byte-for-byte and were not staged,
repaired, or incorporated into this candidate.

## Verification

Passed:

```text
cargo test -p vela-protocol generated_sources_receive_local_commitment_hashes
cargo test -p vela-cli --test canonical_source_commitment
cargo test -p vela-signer
cargo test -p vela-cli
cargo test -p vela-protocol authority
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
cargo clippy -p vela-cli -p vela-signer -p vela-edge -p vela-protocol --all-targets -- -D warnings
python3 conformance/verify.py
./scripts/full-conformance.sh --suite core --mode=ci
./scripts/full-conformance.sh --suite frontier --mode=ci
```

Results:

- CLI: 331 passed, 2 explicitly ignored live-input tests, zero failures;
- signer: 43 passed, zero failures;
- authority-focused protocol tests: 31 passed, zero failures;
- cross-implementation reducer fixtures: 4 passed, zero failures;
- core conformance: 30 pass, 0 warn, 0 fail, 20 intentionally excluded;
- frontier conformance: 9 pass, 1 pre-existing human-reconciliation warning,
  0 fail, 40 intentionally excluded.

No external Lean, Diderot, live-network, or unrelated site suite ran.

## Remaining gate

This candidate does not authorize an active migration. The next ceremony gate
still requires:

1. a dedicated repository-authority Ed25519 key loaded into the standard SSH
   agent, selected by exact public key; and
2. one explicitly selected strict-clean, low-risk active Frontier.

The agent did not generate, select, read, or reuse a human key. Quantum remains
ineligible until its Target Index is repaired through the sanctioned profile
migration path. The active Sidon checkout remains excluded while it contains
unreconciled user work.
