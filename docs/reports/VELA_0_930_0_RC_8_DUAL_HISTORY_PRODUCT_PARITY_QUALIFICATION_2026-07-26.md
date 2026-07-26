# Vela 0.930.0-rc.8 dual-history product parity qualification

- Date: 2026-07-26
- Status: source candidate qualified
- Stable released baseline: Vela `v0.915.1`
- Candidate tag: `v0.930.0-rc.8`
- Candidate commit:
  `c282d34304dd75608a19dec1a1394a43b8501b34`
- Previous candidate:
  `v0.930.0-rc.7` at
  `0f01d6643bae9515eba7b69edfe8939f81407375`
- Product-parity repair:
  `12318254426aa1ce5f5665a50ab31faedfbab2df`
- Governing decision: ADR 0020 remains Proposed
- Scientific-state effect: none

## Decision

Qualify `v0.930.0-rc.8` as the source candidate that preserves routine Vela
operation across a verified legacy event history and repository-authority
history. Keep `v0.915.1` as the stable released ecosystem component.

The candidate does not accept ADR 0020 or claim outside adoption. It qualifies
the proposed repository-authority product against one real protected human
decision, the current producer queue, strict verification, derived
materialization, clean-clone replay, and the bounded parent conformance union.

## Reproduced product defects

The first live repository-authority review decision exposed three concrete
composition defects:

1. proposal parity derived standing from the legacy event log alone, so a
   verified repository-authority terminal decision appeared unauthorized;
2. legitimate post-migration materialization of recognized derived Profile
   views was denied by the canonical write barrier; and
3. stale work-coordination projections could hide the current first producer
   offer after a completed authority transaction.

The repair introduces no additional authority surface. It:

- verifies the closed repository-authority DSSE history before using it;
- derives proposal standing over the verified legacy and authority histories;
- keeps old callers fail closed when no verified authority history is supplied;
- permits only recognized `WriteClass::Derived` post-migration maintenance;
- continues to deny event, proposal, authority, registry, policy, and
  canonical-evidence writes to that lane; and
- threads the verified context through status, strict check, Target Index,
  `next`, `work`, landing revalidation, and materialization.

Focused regressions cover a terminal authority event following an anchored
proposal and the exact allowlist for post-migration derived writes.

## Live Erdős vector

The candidate replayed the published Erdős head:

```text
commit:                   fae0e6644d4e27923df9c95624366ad289b85c02
tree:                     71b2246088d3162ff22fd1ab05178623bb2e51a7
legacy event count:       2,193
legacy event root:        sha256:d35b11555988458d28a971b0c882c6f42c27e0d4ca47df3080bc9872d51c7096
scientific-state root:    sha256:540d4967071425f77c693e61f62053208b07d67667490dcb9eeef62ec3f1d316
derived snapshot root:    sha256:845ed13214db0f8a1fbdb81805bc17ed3d459da040814c35f89eda5e66cf3cf8
proposal root:            sha256:19e79fc93ddc3059864a6729554754ace818a05257be28bd4ed604dc7bdb091e
repository-authority ID:  var_a5f7cc584b32e74f
first producer offer:     erdos:1056
open available targets:  646
pending proposals:        14
```

Proposal `vpr_4a9068064a0c441c` is rejected by authority event
`vev_7b630aacf5cb5e63`, covered by authority record
`var_a5f7cc584b32e74f`. The protected rejection changed no scientific root and
left the Receipt and artifacts intact.

Strict mode remains fail closed with the same retained historical debt:

```text
missing_conditions:           1,511 blockers
unsigned_registered_actor:       81 blockers
graph warnings:                    9 nonblocking
```

Repository context and state integrity pass. No blocker was waived, re-signed,
temporalized, or rewritten.

## Clean-clone replay

A fresh standalone clone at `fae0e664…` independently reproduced:

- the same Git tree and canonical roots;
- 2,770 findings and 2,193 legacy events;
- 646 available producer targets with `erdos:1056` ranked first;
- 14 pending proposals; and
- all 38 frozen witnesses.

The Target Index matched its 1,217-target seal without a write.

## Bounded release qualification

The parent routine conformance union passed:

```text
40 selected gates passed
0 warnings
0 failures
10 intentionally unselected gates
```

The unselected gates are external Lean, live-network, and formal-only checks
outside this candidate train. The selected union includes:

- all Vela workspace tests;
- formatting, clippy, and dependency audit;
- authority-history and capability conformance;
- Rust, Python, and TypeScript reducer parity;
- Receipt v1, DSSE, in-toto, status-event, and red-team contracts;
- active-document and generated-agent guidance checks;
- source-map, Frontier Kit, frontier-calculus, and Frontier Fabric checks;
- Git-bundle and training-frontier portability; and
- deterministic machine exports.

The first run identified two stale generated Frontier Kit skill adapters.
`vela agents sync` regenerated only those disposable leaves from their
canonical `VELA.md`. The isolated gate then passed, and the complete bounded
union passed on rerun.

The local release-profile binaries used for qualification report:

```text
vela 0.930.0-rc.8
vela-signer 0.930.0-rc.8
```

Their local SHA-256 values are:

```text
vela:        91d5b323fd84a3c59d1d082adfc04a64ffac6cb31fcaa8a49ae19006389f4065
vela-signer: 47bb0ec734983027c9e2043d091e92203c00a6432d387184817d866fd301c09d
```

These development binaries are local qualification evidence only. Hosted
portable artifacts, checksums, and build attestations are a separate gate.

## Consequence

`v0.930.0-rc.8` is eligible for hosted portable artifact qualification. It
does not by itself promote `0.930.0` to stable, authorize another protected
ceremony, qualify automated scientific acceptance, or justify removing a
custody or recovery surface that remains reachable.
