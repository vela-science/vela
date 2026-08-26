# Vela 0.977.5 release record

Status: `RELEASED`

Published: 2026-08-26

This record was added after publication. It is an audit record, not part of
the tagged release source.

## Qualified lineage

- VELA-RC-1 qualified commit:
  `1a30cf065b4b74592d2e58457aa6884b639649a3`
- VELA-RC-1 qualified tree:
  `23a4f67a4d245ac1c47a366aaa83edad8ee11cca`
- release commit and tag target:
  `9cf13af9fd687db88e562842fd6dd641e10bae6a`
- release tree:
  `5863c283ad3a3efb76d365e5936544923851fb4a`
- merge commit on `main`:
  `700864bb897881f30b3f6a782f68f3bede6dd073`
- merge tree:
  `5863c283ad3a3efb76d365e5936544923851fb4a`
- signed annotated tag: `v0.977.5`
- tag object: `0afe844862186cbf01a4ba91c4e6ad2129a8fcbc`
- Protocol: `1`
- Submission schema: `v3`
- migration: none

The merge tree is byte-identical to the independently qualified release tree.
The release-only delta from the RC-1 candidate changes version and release
metadata, release notes, current release documentation, and the transitive
version-bound informative fixture roots. It does not change Protocol objects,
schemas, canonical serialization, authority, Decision, Event, Standing,
replay, or CLI behavior.

## Final qualification

The exact release commit passed:

- the complete Protocol 1 core conformance union;
- authority, incomplete-acceptance, rejection-preservation, correction,
  supersession, replay, and Artifact-integrity negative cases;
- formatting, workspace tests, Clippy with warnings denied, and dependency
  policy;
- documentation contracts and both release-facing external fixtures;
- a clean-checkout locked release build and clean installation workflow;
- deterministic local macOS and isolated Linux release builds;
- hosted macOS and Linux release builds, provenance attestations, and bundle
  smoke tests.

The hosted release workflow is:
<https://github.com/vela-science/vela/actions/runs/33016849063>.

## Published artifacts

| Artifact | SHA-256 |
| --- | --- |
| `vela-linux-x86_64.tar.gz` | `3ae19f4d76a14d40b42cd7102ac6b1a57948ed797eaf53200137cc91d571fff7` |
| `vela-linux-x86_64.tar.gz.release-manifest.json` | `f01a705fc82c06ce6dce99801284ba80f494994a34d39e6e0dff3085c9020508` |
| `vela-linux-x86_64.tar.gz.release-manifest.json.sig` | `d42b66ccbca87fe7382eb966560eedd7cde9f83195ec50118e3be17466d36b8f` |
| `vela-linux-x86_64.tar.gz.spdx.json` | `e47c1483e9e700b403f7eacb43bd041db109a6e4e0e52fcbaab649dc7c6fa76b` |
| `vela-macos-aarch64.zip` | `e2a8d2ca909ea856e749525c00d7fb99d135f9b9abff5150a2b3cd0d2bfc7f02` |
| `vela-macos-aarch64.zip.release-manifest.json` | `541c6b64691938fafc6b9977ce86bc4412d2d5309752511a56de105f8dc70f7f` |
| `vela-macos-aarch64.zip.release-manifest.json.sig` | `124587c1aac7685101823c0f825de0de40a3f28ccf03f98650a8515c7b7186d1` |
| `vela-macos-aarch64.zip.spdx.json` | `6d844a8bd765c50124930c484bce18eebf37989f4dc02d49f9a3d3154724b76d` |

The detached manifests verify as `release@vela.space`, namespace
`vela-release`, with distribution-key fingerprint
`SHA256:MX3Eo1o9S5pLnx2kiNyAy2aME7PAWDtvqtUBljJst1M`.

Public release:
<https://github.com/vela-science/vela/releases/tag/v0.977.5>.

## Post-publication verification

- the public tag peels to the release commit and its exact qualified tree;
- both detached manifest signatures verify against `allowed_signers`;
- both archives match the hashes in their signed manifests;
- both GitHub provenance attestations verify;
- the signed-manifest-required public installer installed the Apple-silicon
  bundle and the installed CLI reported `vela 0.977.5`;
- `conformance/protocol-1.json` identifies `Vela Protocol 1`, software release
  `0.977.5`, status `released`, and manifest root
  `sha256:d3af662374c2940329016ffdeccdc406f30a5cf412c4b0b565ee5ee58e223af5`.

## Explicit limitations

- cumulative workflow advantage is not established;
- no cumulative-intelligence, autonomous-discovery, scientific-truth,
  workflow-productivity, Level-2 cumulative-science, or external-adoption
  claim is made;
- replay reconstructs governed scientific state; it does not imply bit-for-bit
  rerun of external stochastic, model, instrument, or physical processes;
- the deployed Problems projection remains a separate, legacy/unqualified
  system until its own workflow pins this release;
- no external human adoption or production scientific workflow was observed
  by RC-1.

## Problems/WebMCP handoff

```text
Vela release: 0.977.5
Protocol: 1
Release commit: 9cf13af9fd687db88e562842fd6dd641e10bae6a
Release tree: 5863c283ad3a3efb76d365e5936544923851fb4a
Artifact/package: vela-macos-aarch64.zip / vela-linux-x86_64.tar.gz
Artifact hash: e2a8d2ca909ea856e749525c00d7fb99d135f9b9abff5150a2b3cd0d2bfc7f02 / 3ae19f4d76a14d40b42cd7102ac6b1a57948ed797eaf53200137cc91d571fff7
Required projection/demo pin: 0.977.5
```

## Research status

`FOUNDATIONAL TOP-DOWN SEARCH: CLOSED`
