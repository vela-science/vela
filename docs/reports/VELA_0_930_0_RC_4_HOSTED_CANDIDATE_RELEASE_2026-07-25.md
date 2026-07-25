# Vela 0.930.0-rc.4 hosted candidate release

- Date: 2026-07-25
- Status: attested prerelease published; active migration not started
- Stable released baseline: Vela `v0.915.1`
- Candidate tag: `v0.930.0-rc.4`
- Candidate commit:
  `7aa07dfeebf0ce60de4b69c9daae6b0675855be2`
- Release:
  <https://github.com/vela-science/vela/releases/tag/v0.930.0-rc.4>
- Protocol effect: none
- Scientific-state effect: none
- Authority effect: none

## Decision

Publish `v0.930.0-rc.4` as the first installable, attested candidate for the
attributed repository-authority migration. Keep `v0.915.1` as the stable
released ecosystem component.

The candidate is qualified for:

- strict read-only replay of the exact Formal Conjectures Frontier;
- one key-free migration preview after a dedicated public
  repository-authority identity exists; and
- a later separately reviewed protected migration ceremony.

It does not authorize an active migration, expose a human key, create an
authority record, or change scientific standing.

## Defects found before the immutable tag

No tag was created until all of these candidate defects were repaired.

1. The first `rc.3` hosted run (`30148376439`) attempted a crates.io install
   for unpublished prerelease workspace packages. Stable tags still require
   registry installation; prerelease tags now require a deliberately skipped
   registry gate plus successful portable builds and smoke tests.
2. Manual run `30149969785` exposed a cross-version replay defect. Strict
   validation substituted the reader binary version into non-scientific
   derived metadata, so an untouched older Profile v1 checkout appeared stale.
   Profile v1 validation now uses the exact materializer and verifier
   identities retained in `vela.lock`.
3. Manual run `30150535583` passed strict replay but compact status reported
   the reader-local rematerialized snapshot root rather than the retained
   Profile root. Status now reports the lock-pinned root for Profile v1.
4. Manual run `30150762113` passed all platform smoke tests but exposed
   CRLF-terminated Windows checksum sidecars when independently checked on
   macOS.
5. Manual run `30151066254` proved that the first LF-only repair silently
   emitted no Windows checksum files. The Windows fresh-prefix smoke failed
   closed on the missing sidecar.
6. Final manual run `30151316892` passed all three builds and all three
   fresh-prefix smoke tests with portable checksum sidecars. Hosted
   conformance run `30151308096` and CodeQL run `30151307888` also passed.

The final tag-triggered release run is `30151755867`. It passed metadata,
Linux, macOS, and Windows build and smoke jobs, deliberately skipped
prerelease registry installation, and published the GitHub prerelease.

## Portable release assets

Every platform publishes an archive, SPDX SBOM, portable trust record, and
SHA-256 sidecar for each. The archives are:

| Platform | Archive SHA-256 | `vela` SHA-256 | `vela-signer` SHA-256 |
| --- | --- | --- | --- |
| Linux x86-64 | `e130070ea4b79cf896e1ae4dadb54c75dd3b889cfdafd0bd80f3032c773d1182` | `ec6ce569e304a2a427f32935d2034315d42c5b7821ec0094b8a9e6e52aac3501` | `790e6fee660f8fc87a70f105ba5325b8349729e7a59138ceff902502c9742c25` |
| macOS arm64 | `708ecb27292121b39f60b6dcc6ccd58a9d5890576118acd77c486791e1b2d4c0` | `2cd2589d75d476f53f659307a8d6553e5ee9aef3b2f24a1e850eaf0aa557ce63` | `8ee8584f29475a9c1baef8f0dbeb700392937efa906c50347cf2858975b333cc` |
| Windows x86-64 | `26781c3512dd9f27787d3a8d5a5fa51d21505e29a3aeb7ec1b8094291992b52c` | `5b53c6366b9dca89129ad92582eb83a4ef1658a76772999da0392eeb292fb8c4` | `10248329e67b73a1383947f11a2f8c992a79734ffc3478de51131301574a95b9` |

The exact release trust records bind:

```text
schema: vela.release-trust.v1
version: 0.930.0-rc.4
source commit: 7aa07dfeebf0ce60de4b69c9daae6b0675855be2
artifact class: portable
GitHub attestation: required
platform signature: absent
```

The archives are portable candidates. They are not Developer-ID-notarized or
Authenticode-signed public-beta installers.

All 18 release files passed their published SHA-256 sidecars on macOS. The
Windows sidecars therefore work across the platform boundary they document.
GitHub build-provenance verification passed for all nine archive, SBOM, and
trust-record subjects:

```bash
gh attestation verify <subject> --repo vela-science/vela
```

## Exact Formal replay

The released macOS archive was extracted into a fresh isolated prefix and
reported:

```text
vela 0.930.0-rc.4
```

It then read the exact Formal checkout:

```text
commit:                8be46caa082c63374d1b208ccbd84c1f9c351a04
tree:                  81bdd3d7f91e5d51d2c6b80614ed4d59b6ec94fa
canonical manifest:    sha256:2a7cd5c2be65b27be812e6cb7455f008a8228fb77c44a363aad663add1aa5241
event count:           35
event-log root:        sha256:b9df87525e7f4313eedeb0b65ba29b21009e04e404aa25bcb5e29bfc9cd6d3f7
scientific-state root: sha256:4924adbbea6dfe288d14af03cf3d544f73c511df6b6ef8b938c8291685101444
legacy snapshot root:  sha256:02a1cedd97356943f02d68f241fc3f93c7acf52bcd8d8a7914c2fb417facacee
proposal root:         sha256:ba47ddf5c16ed567ddf835385066e3fc294b447bc0eabd3f9820f5e707efb39e
actor-registry root:   sha256:f52d59b1db885f467c66a29335ada68544a09da5f3869723461100eed0aac79e
artifact root:         sha256:fbd7e05b185cd06bc06484e8b0216c17c5263a71d8481ca38e574e9b2c5156d8
```

Results:

```text
vela status . --json          -> ok, replay reproduced, strict pass, 0 blockers
vela check . --strict --json  -> ok, 14 valid findings, 0 errors, 0 warnings
```

The worktree was clean before and after both commands. The candidate made no
event, proposal, artifact, policy, authority, derived-view, journal, or Git
change.

Policy remains absent and `human_only`. One proposal remains pending. Neither
fact is repaired or hidden by the candidate.

## Verification

Focused local checks:

```bash
cargo test --locked -p vela-protocol \
  profile_v1_strict_layout_replays_its_pinned_materializer_version
cargo test --locked -p vela-protocol --test action_contracts
cargo test --locked -p vela-protocol --test cross_impl_reducer_fixtures
cargo test --locked -p vela-cli compact_status
python3 conformance/verify.py
```

Parent integration:

```text
./scripts/full-conformance.sh --suite core --mode=ci
30 PASS, 0 WARN, 0 FAIL, 20 intentional SKIP
```

No external Lean, Diderot, live-network, site, or unrelated broad suite ran.

## Remaining gate

The standard OpenSSH agent still exposes no identity. The next action is an
operator custody step:

1. provision one dedicated repository-authority Ed25519 identity;
2. load it through the standard OpenSSH agent;
3. expose only its stable key ID and public key; and
4. run the key-free, write-free preview against exact strict-clean Formal.

The existing Profile v1 trust-anchor variable is not an Era-1 repository
signer and must not be reused. Applying the preview remains a separate
protected ceremony.

## Atlas and read-network boundary

This release does not widen the Atlas/Tapestry result. Frontiers remain the
only authority units. The exact typed cross-Frontier packet remains
source-local and deletable; Vela Web remains the sole projector; Neon remains
a disposable read model. No Atlas service, relay, lens, MCP, public API,
second database, ontology, or protocol object follows from this candidate.
