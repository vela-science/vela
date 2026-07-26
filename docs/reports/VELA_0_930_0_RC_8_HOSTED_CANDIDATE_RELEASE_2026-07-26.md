# Vela 0.930.0-rc.8 hosted candidate release

- Date: 2026-07-26
- Status: attested prerelease published
- Stable released baseline: Vela `v0.915.1`
- Candidate tag: `v0.930.0-rc.8`
- Candidate commit:
  `c282d34304dd75608a19dec1a1394a43b8501b34`
- Release:
  <https://github.com/vela-science/vela/releases/tag/v0.930.0-rc.8>
- Release workflow:
  <https://github.com/vela-science/vela/actions/runs/30209004073>
- Governing decision: ADR 0020 remains Proposed
- Scientific-state effect: none

## Decision

Publish `v0.930.0-rc.8` as the attested portable candidate containing the
verified dual-history product-parity repair. Keep `v0.915.1` as the stable
released ecosystem component.

The exact source qualification is recorded in
`VELA_0_930_0_RC_8_DUAL_HISTORY_PRODUCT_PARITY_QUALIFICATION_2026-07-26.md`.
That evidence proves that a real protected repository-authority rejection,
legacy replay, proposal standing, strict verification, Target Index freshness,
producer offers, and recognized derived maintenance compose without changing
scientific state or weakening the write barrier.

This hosted gate adds portable cross-platform artifacts, fresh-prefix smoke
tests, checksums, SBOMs, trust records, and GitHub build provenance. It does
not accept ADR 0020, promote `0.930.0` to stable, or authorize another
protected decision.

## Hosted workflow

Release workflow `30209004073` passed:

- exact tag and workspace-version metadata;
- signer contracts and platform profiles;
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
| Linux x86-64 | `a9f459b64d375655793ffd6b690c83abd94a1469d16fbb110d1f7cd1af08f004` |
| macOS arm64 | `f09d71ab1a9132eb84db9dd99e7b5c1171969dffeca5bab5a352e41bcf6ecb1d` |
| Windows x86-64 | `c985a0123ca2f90c48264a851fd4132c3a4218006e9aa78dacb698b0e0ca9d6b` |

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
version: 0.930.0-rc.8
source commit: c282d34304dd75608a19dec1a1394a43b8501b34
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
vela 0.930.0-rc.8
vela-signer 0.930.0-rc.8
```

Binary SHA-256:

```text
vela:        8e42a68717f92ac5b64b36aef087a60147e734f4a4a3f2ffd850fd125bd6ce3d
vela-signer: bd28038ab29cabaa7201a497db39839da2d299286776a1df6aa9fdacc74011a4
```

The hosted binaries differ from local development binaries as expected
because the release workflow builds them in an independent release
environment.

## Current product state

Formal, Sidon, Quantum, and Erdős have completed the first
repository-authority migration sequence with unchanged scientific roots. The
Erdős protected rejection proves the exceptional human-decision path and
preserves all retained historical strict blockers. Routine work exposes 646
available producer targets with `erdos:1056` first.

The next evidence gate is ordinary producer and read-only consumer use against
this exact candidate. Scientific acceptance remains eligible only for a
naturally qualified proposal whose exact Decision Brief and strict Engine gate
permit it. First-party operation alone does not justify stable promotion or
acceptance of ADR 0020.
