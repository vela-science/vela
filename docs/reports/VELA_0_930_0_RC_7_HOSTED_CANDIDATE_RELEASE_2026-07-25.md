# Vela 0.930.0-rc.7 hosted candidate release

- Date: 2026-07-25
- Status: attested prerelease published
- Stable released baseline: Vela `v0.915.1`
- Candidate tag: `v0.930.0-rc.7`
- Candidate commit:
  `0f01d6643bae9515eba7b69edfe8939f81407375`
- Release:
  <https://github.com/vela-science/vela/releases/tag/v0.930.0-rc.7>
- Release workflow:
  <https://github.com/vela-science/vela/actions/runs/30169459204>
- Protocol effect: none
- Scientific-state effect: none
- Authority effect: none

## Decision

Publish `v0.930.0-rc.7` as the attested portable candidate containing the
derived-statistics determinism repair. Keep `v0.915.1` as the stable released
ecosystem component.

The exact source qualification is recorded in
`VELA_0_930_0_RC_7_DERIVED_STATS_DETERMINISM_QUALIFICATION_2026-07-25.md`.
That evidence proves strict replay of the migrated Formal Frontier and current
Sidon Frontier, repeatable fresh-clone Sidon materialization, and preservation
of the existing Erdős and Quantum fail-closed classifications.

This hosted gate adds portable cross-platform artifacts, fresh-prefix smoke
tests, checksums, SBOMs, trust records, and GitHub build provenance. It does
not authorize another protected migration or change the stable ecosystem lock.

## Hosted workflow

Release workflow `30169459204` passed:

- exact tag and workspace-version metadata;
- Linux x86-64 build and fresh-prefix smoke;
- macOS arm64 build and fresh-prefix smoke;
- Windows x86-64 build and fresh-prefix smoke;
- archive, SPDX SBOM, and portable trust-record generation;
- SHA-256 sidecar generation;
- GitHub artifact attestation; and
- immutable prerelease publication.

The prerelease registry job was intentionally skipped.

## Portable release assets

| Platform | Archive SHA-256 |
| --- | --- |
| Linux x86-64 | `f8bf2dfeaa20b1ca23efc54774655cc1b924232a6db99d6b3345432278b3970c` |
| macOS arm64 | `f1171cf1ae6e021d981ea6a257e7c233259d6b29e5b24ad2fa33ecfa52afb22a` |
| Windows x86-64 | `a7acb9c43913ccb1d6c5c697cb440295be5ca931b51c0bf08f48d358c9ecc810` |

All 18 published archive, SBOM, trust-record, and checksum files were
downloaded after publication. Every one of the nine archive, SBOM, and trust
subjects matched its SHA-256 sidecar. GitHub build-provenance verification
passed independently for all nine subjects:

```bash
gh attestation verify <subject> --repo vela-science/vela
```

Each portable trust record binds:

```text
schema: vela.release-trust.v1
version: 0.930.0-rc.7
source commit: 0f01d6643bae9515eba7b69edfe8939f81407375
artifact class: portable
GitHub attestation: required
platform signature: absent
```

The archives are portable candidates. They are not Developer-ID-notarized or
Authenticode-signed public-beta installers.

## Independent macOS smoke

The published macOS archive was extracted into a fresh temporary prefix after
the hosted workflow completed:

```text
vela 0.930.0-rc.7
vela-signer 0.930.0-rc.7
```

Binary SHA-256:

```text
vela:        7cf6f0eb8c43bcf684fa9f5719e8f4ca4a850e2f3bbeedf01412186e76e4ea57
vela-signer: 6013ea2231b3dbb2df07fd544319797fcc571d5aa25bf49f6fe704c28e6b2695
```

The hosted binary differs from the local development binary as expected
because it is built in the release profile by the attested workflow.

## Remaining authority gate

Formal is already migrated and strict-clean. Sidon is the next technically
eligible Profile v1 candidate, but this release does not authorize its human
ceremony. Quantum remains behind five immutable proposal logical-ID conflicts.
Erdős remains behind 1,511 `missing_conditions` blockers and 81
`unsigned_registered_actor` blockers.

Any later active migration requires its own exact key-free preview and
explicitly authorized protected ceremony. No prior approval is reusable.
