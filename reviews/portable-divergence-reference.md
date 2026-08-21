# Independent re-review: Vela portable divergence reference

## Verdict

**PASS** for producer commit `e54824044921e6782a97aca80a35d0cf4dcdc553`
and tree `130940068d80f7eed6cf095b4499f0cbf24fd74c`.

The retained reference demonstrates its bounded claim: byte-identical,
authenticated portable Submission input enters two independently initialized
Repositories; each authenticates a distinct synthetic local authority
principal, applies its own authorized Decision, and deterministically replays
to different local Standing. It does not transport Standing or claim global
consensus, scientific truth, adoption, utility, or release status.

This verdict has no authority or Standing effect. It is review evidence only
and does not merge or modify producer bytes.

## Exact binding

- Producer ref: `origin/codex/protocol1-portable-divergence-reference`
- Producer commit/tree:
  `e54824044921e6782a97aca80a35d0cf4dcdc553` /
  `130940068d80f7eed6cf095b4499f0cbf24fd74c`
- Base commit/tree:
  `1a2e0328620b4e8c4584c3d4baf257adb11f3d45` /
  `1bd8ed4e11d3745f159b32f23539f5174fd44803`
- Reviewed range: `1a2e0328620b4e8c4584c3d4baf257adb11f3d45..e54824044921e6782a97aca80a35d0cf4dcdc553`
- Delta: 13 paths, 1120 insertions, 15 deletions
- Live `origin/main` commit/tree:
  `2b8d43ed50a9639dfc18c5f6f21677021f70a4b2` /
  `2a15e11af6aab2fc4574df940ec78de4ba29fdd8`
- Clean merge-tree: `13c5e0cf2e64be907cee4c0fd740ab0027118e13`
- Protocol 1 root: `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`
- Review time: `2026-08-21T16:13:19Z`

The remote ref was fetched and the commit reconstructed in a fresh full clone.
Commit, tree, parent, range, changed paths, diff size, and live-main merge-tree
matched the handoff.

## Resolved findings

### PD-1: distinct authenticated principals

Resolved. `VELA_TEST_DEVICE_IDENTIFIER` exists only under the existing
non-default `test-support` feature, validates an exact nonzero 32-character
lowercase hexadecimal value, and leaves the production/default runtime path
unchanged.

The retained authority chains bind these independently recomputed principals:

- accept:
  `local:device-sha256:8a83665f3798727f14f92ad0e6c99fdab08ee731d6cd644c131223fd2f4fed2a|uid:501`
- reject:
  `local:device-sha256:0d6ba19b62531ccb0deb8804313eca283c69560f66f1b7b8a2c1592ae8c35c6b|uid:501`

Within each history, initialization and Decision records carry the same
expected principal; across histories they differ. Repository IDs/resources,
service keys, keysets, authorization models, performers, session references,
and Decisions also differ.

### PD-2: immutable replay evidence

Resolved. Complete Git bundles and `expected.json` bind every required Git,
authority, Event-log, Repository, projection, and Standing result.

- Accept: bundle
  `sha256:2a92803cdb30e2f16d0f3a9b41fcbc24be39fc9693f7abc6f04f2f261a0dd0ba`;
  Git `887c4daf94605bc5468c91df350d53af1c01b47b` /
  `f16c91239380f2eb9b01133eea0ce98d9f52043d`; Repository
  `sha256:cd9d73e81841bb74802e07de80c07425ecbfbe81156cbfbca11848a885753b31`;
  projection `sha256:ba89cf9164ab7283d09bbe4551525b728a419c9e267766ac3a40223afff9b8e8`;
  Standing `accepted`, one accepted and zero pending Claims.
- Reject: bundle
  `sha256:144de0583805ef53ea8116c3a6ba65eb7870be48563b85d8b8a0819ecea25c9a`;
  Git `26b194afba5cea01ed4d632d1d43af019870285f` /
  `b079b1814acfe4e0e4a6c8d3c92122a2a21e73dd`; Repository
  `sha256:a2ff58f7425ea65bf87c0eb96462fc9d2124fd7923bdbd784bfc00e841617e4b`;
  projection `sha256:83e9488e54cfac87f5fea686f9eefbdf3c3668c73c23ab2252597666f3f5c346`;
  Standing `unassessed`, zero accepted and zero pending Claims.

Both bundles pass `git bundle verify` and `git fsck --full`, clone cleanly,
and contain no private-key or seed-like paths. Independent reconstruction
matched every Event byte root, both sequence-one and both Decision payload
roots, terminal Event-log roots, Git identities, and the roots above.
The independent Python reader also recomputed both Repository roots.

## Portable bytes and semantic containment

Both histories retain byte-identical Submission bytes at
`sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a`
and Claim bytes at
`sha256:e865c5a2aafd459d52d9b1c8a7734104b1e2d8d1c047c5400684f01505f83632`.
The producer remains `agent:independent-js` with key
`03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8`.

Accept imports one scoped synthetic Verification pass. Reject imports no local
Verification and declines admission without its own check. Decisions and
Standing remain Repository-local. The fixtures explicitly exclude global
consensus, transported Standing, scientific truth, external adoption or
utility, and Protocol 1.0 release. They are informative, synthetic, and
non-authoritative. No current schema, generic policy language, authority
semantics, or production identity behavior is expanded.

## Independent checks

All passed against the freshly reconstructed corrective commit:

```text
cargo fmt --all -- --check
cargo test --locked -p vela-cli --features test-support --test portable_divergence
  2 passed; 0 failed
cargo test --locked -p vela-protocol --test object_interop
  4 passed; 0 failed
cargo clippy --locked -p vela-cli --features test-support --all-targets -- -D warnings
uv run --project conformance --locked ruff check conformance/verify_reference_flows.py
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python conformance/verify.py
  PASS; 77 normative; 39 informative
git diff --check 1a2e0328620b4e8c4584c3d4baf257adb11f3d45..e54824044921e6782a97aca80a35d0cf4dcdc553
```

Additional independent checks matched both bundle roots, `expected.json` root
`sha256:858019d298f55295fe92989bb23a343ce73b6976338f36c7c637c82272274041`,
all five Event roots, all four authority-record payload roots, both device
hashes, exact Submission/Claim bytes, bundle completeness, and clean source
identities.
