# Vela 0.930.0-rc.5 Target Index compatibility

- Date: 2026-07-25
- Status: attested prerelease published; active migration not started
- Stable released baseline: Vela `v0.915.1`
- Candidate tag: `v0.930.0-rc.5`
- Candidate commit:
  `f82adbe8a94b3d03f3e6e0d4488e6cf639963b7e`
- Release:
  <https://github.com/vela-science/vela/releases/tag/v0.930.0-rc.5>
- Release workflow:
  <https://github.com/vela-science/vela/actions/runs/30153237795>
- Protocol effect: none
- Scientific-state effect: none
- Authority effect: none

## Decision

Publish `v0.930.0-rc.5` as the compatibility repair for historical Target
Index v1 readers. Keep `v0.915.1` as the stable released ecosystem component.

The candidate:

- reads a historical v1 index whose unique labels are not in the later v2
  canonical byte order;
- preserves strict sorted-label validation for every v2 candidate and seal;
- keeps malformed or duplicate historical labels fail-closed;
- opens the exact historical target and packet for inspection; and
- derives the protected Profile v1 migration path without changing the
  checkout.

It does not make a historical target actionable, rewrite the derived index,
repair immutable proposal identities, create an authority record, or change
scientific standing.

## Reproduced defect and repair

The clean Quantum Codes Frontier retained a historical
`vela.target-index.v1` whose labels were:

```text
quantum-code
stabilizer
bounded-search
upstream-open
```

They are unique and valid under the historical reader, but are not sorted in
the later v2 UTF-8 byte order. The previous candidate applied the v2 ordering
rule while decoding v1. Consequently both status and the documented
`target-index repair` path stopped at:

```text
target.labels must be sorted in UTF-8 byte order
```

The repair separates historical label validation from the v2 canonical
constructor:

- v1 still enforces the label count, text bounds, and uniqueness;
- v1 does not reinterpret historical order;
- v2 still requires canonical sorted order; and
- any regenerated candidate must use v2.

No canonical object, event, proposal, Receipt, artifact, registration, policy,
or accepted-state schema changed.

## Exact Quantum regression

The published macOS archive read:

```text
commit:               be2723fe07d0e218f0370253cff93a8748690683
tree:                 b3362c302ea87fbef798abeb48d03aca7ed92553
frontier:             vfr_001f148c07eebecb
event count:          7
event-log root:       sha256:7a8d06e9c86b9437fffaa6dac9803827f9ad64ee32c34fb1603af8ca986a17ab
legacy snapshot root: sha256:0975b1b7fda4c2fee1b5cf6fe312843f3f36425151da75eab389522ee1a73e10
proposal root:        sha256:cdc0f3c3637294d70250b956867d9188df193909a104fa80bde8685e8d1e8ec0
actor-registry root:  sha256:393c47268f71d775aaec13a4f88608c46e61b059cb11e855817c36f5ad54bc5e
artifact root:        sha256:d4a485fdc3de718b4d21b6141a9cd564c4cc572a4d8b30744c9df61084efa748
Target Index root:    sha256:3944e529b5954a9aa055b9e5cfd0f2ed7108a02e54719ee8342e58d53a643243
```

`vela target-index repair . --json` now succeeds as a read-only diagnosis. It
classifies the index as historical-only and reports:

```text
target_index_event_root_mismatch
target_index_profile_upgrade_required
target_index_proposal_root_mismatch
target_index_state_root_mismatch
```

It derives the exact protected migration preview:

```bash
vela migrate . --to frontier-repo-v1 --check \
  --profile ../frontier-profile-v1.yaml \
  --target-candidate ../target-index-candidate.json \
  --as reviewer:ADMINISTRATOR \
  --reason 'Bind exact legacy repository' \
  --json
```

`vela target-index inspect . 'quantum:[[10,1,4]]' --json` opens the retained
target and its complete stabilizer-work packet. The target remains historical
and non-actionable.

The checkout was clean before and after all commands. The commit, tree, event
bytes, proposal bytes, artifacts, roots, and derived files remained
byte-identical.

## Remaining strict debt stays visible

After the Target Index reader succeeds, strict checking exposes five
pre-existing immutable proposal logical-ID conflicts:

```text
vpr_1f4196a6758e1b4b -> vpr_504f52d40c021d66
vpr_48bd2c2afdb23008 -> vpr_77b25142f8947737
vpr_7313028c1077b829 -> vpr_a983996ded85c97b
vpr_ab419cca4a99a8ea -> vpr_3f0d07014f23371e
vpr_bbf2f812da2779c8 -> vpr_2c1316d5c28bd8ff
```

They are reported as five strict `check_error` signals under one compact
`strict_check_failed` blocker. Replay still reproduces five valid findings,
and the one current proposal remains pending. The candidate neither masks nor
repairs this Profile v1 boundary debt.

## Verification

Focused checks:

```bash
cargo test -p vela-edge target_index
cargo test -p vela-cli --test target_index_cli
cargo test -p vela-cli --test product_09
cargo clippy -p vela-edge -p vela-cli --all-targets -- -D warnings
python3 conformance/verify.py
```

Results:

```text
vela-edge target index: 21 passed
vela-cli Target Index:  10 passed
vela-cli product 0.9:   5 passed
clippy:                 passed
cross-language:         16 fixtures and 7 canonical vectors passed
```

The deterministic parent release union passed:

```text
./scripts/full-conformance.sh --suite full --mode=ci
42 PASS, 1 known WARN, 0 FAIL, 7 intentional SKIP
```

The warning is the existing copied-Frontier human reconciliation requirement.
External Lean, Diderot, and live-network suites were not selected.

Hosted workflow `30153237795` passed:

- metadata;
- Linux x86-64 build and fresh-prefix smoke;
- macOS arm64 build and fresh-prefix smoke;
- Windows x86-64 build and fresh-prefix smoke; and
- immutable prerelease publication.

The prerelease registry job was intentionally skipped.

## Portable release assets

| Platform | Archive SHA-256 |
| --- | --- |
| Linux x86-64 | `524a1318c29eb0e384d718385eba6d0a4697eab448a2678929d4a098228fbe11` |
| macOS arm64 | `3d85eba5e1d0568eafe0dcd8d7f1c530ebbcb7a7d05405850f896a3d826dc0a9` |
| Windows x86-64 | `4cccffc60494e2cf6c4ec2ad188fe998afac6138f59e64f8c05d5c713cdec95f` |

All nine archive, SBOM, and trust-record subjects matched their nine published
SHA-256 sidecars. GitHub build provenance verification passed for all nine
subjects. Each trust record binds:

```text
schema: vela.release-trust.v1
version: 0.930.0-rc.5
source commit: f82adbe8a94b3d03f3e6e0d4488e6cf639963b7e
artifact class: portable
GitHub attestation: required
platform signature: absent
```

The downloaded macOS archive also passed a second local fresh-prefix smoke and
reported:

```text
vela 0.930.0-rc.5
vela-signer 0.930.0-rc.5
```

These remain portable candidates, not notarized or Authenticode-signed public
installers.

## Remaining authority gate

The standard OpenSSH agent still exposes no identity. Formal migration still
requires the operator to:

1. provision one dedicated repository-authority Ed25519 identity;
2. expose only its stable key ID and public key through the standard agent;
3. run the key-free, write-free preview against exact strict-clean Formal; and
4. review the later protected ceremony separately.

The Profile v1 trust-anchor variable is not the Era-1 repository signer.

Quantum likewise remains on its exact protected Profile v1 migration path.
Neither migration is authorized by this candidate report.

## Atlas and read-network boundary

This compatibility release does not widen the Atlas/Tapestry result. Frontiers
remain the only authority units. The exact typed cross-Frontier packet remains
source-local and deletable; Vela Web remains the sole projector; Neon remains
a disposable read model. No Atlas service, relay, Lens, MCP, public API,
second database, ontology, repository, subdomain, or protocol object follows
from this candidate.
