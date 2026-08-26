# VELA-RC-1 second independent R6 requalification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
PASS WITH DOCUMENTED RELEASE-TIME ACTIONS
```

The auditor reproduced fixes for all three findings in the first independent
R6 requalification. The repository-controlled release entry point now fetches
the complete locked Cargo graph before frozen notice generation; both
target-specific normalized SPDX documents match the exact selected release
graph and notice inventory without the 32 feature-disabled SSH-crypto crates;
and the release documentation contract passes 14 of 14 tests.

Fresh clean-source runs produced real macOS Apple-silicon and Linux x86-64
musl bundles. On each target, two independent release binaries matched, two
normalized Syft scans matched, two deterministic archive passes matched, the
bundle executed its operator smoke, and the manifest bound the clean exact
candidate commit and tree. All produced bytes are local audit artifacts. They
are unsigned, not OIDC-attested, unpublished, and not
release-authorized.

No product behavior, Protocol semantics, schema, wire field, persisted data,
CLI semantics, Standing, version, tag, signature, public asset, publication,
push, deployment, or release changed in this lane.

## Exact candidate binding

| Field | Exact value |
| --- | --- |
| Candidate commit | `bd18d1a128eecb95dfd3bfd6cfe198f109576c78` |
| Candidate tree | `7187138a3025e391f8cd467abc634b7b5bb73ff4` |
| Exact Git-archive stream SHA-256 | `6da0f12a6f5d2a49813673ca72964534a4adba81806cbcf4904c74f1f5c6840e` |
| Candidate version | `0.977.4` |
| Rust toolchain | `1.97.1`; `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Notice generator | `cargo-about 0.8.4` |
| SBOM generator | Syft `1.50.0` |
| Source date epoch | `1787769749` |
| Protocol selection | Vela Protocol 1 release candidate |
| Conformance manifest | 77 normative, 73 informative; root `sha256:bd03d1c94a8ad27493772db656cacd4e7a0fec8a7bcf24e1f19c451135ad4b83` |
| Initial and pre-report status | exact candidate checkout; clean |

The source snapshot digest is an audit-only identity over the exact stdout of
`git archive --format=tar` for the candidate. The Git commit and tree remain
the authoritative source identity.

## Fresh-cache ordering qualification

`conformance/check-release-notice-fresh-cache.sh` used a new temporary
`CARGO_HOME`. Frozen `cargo-about` failed before the fetch and exposed a Cargo
cache-miss diagnostic. The test then invoked:

```text
scripts/release.sh --fetch-locked-graph-only
```

That release-owned path ran `cargo fetch --locked`; the same frozen notice
generation then passed. The test therefore preserves both the negative for the
old ordering and the positive for the repaired entry point.

The full Linux release run supplied a second, stronger positive. It began in a
new `linux/amd64` container from `rust:1.97.1-bookworm`, cloned the candidate
from a complete Git bundle, verified the exact commit/tree and clean status,
installed the pinned musl target, checksum-verified the official Syft 1.50.0
binary, and started with the container's empty Cargo registry cache. The
unmodified release entry point fetched the full locked graph before building
or invoking frozen notice generation and completed. No ambient
Cargo cache was required.

## Clean release artifact receipts

### macOS Apple silicon

| Item | Bytes | SHA-256 |
| --- | ---: | --- |
| `vela-macos-aarch64.zip` | 2,743,758 | `1438f0fdd8e8b14f1bde64dc025bd7fe8a52f88e26b8178eaca0dae5e1197fdc` |
| normalized SPDX SBOM | 126,515 | `9b552132e0ad6faebae0c2a87f448a6073b1ce58a7805f6a80a1987f06cbd7ad` |
| staged `vela` | 7,380,240 | `aa7e47802b7ed7b0d9cdfcbe14a5c2162d841e7e87c5be36da947376e0f3b3a7` |
| target notices | 198,547 | `61b386174cc7cfc11b844412a846a95a73c0e2dc067961935e2c619a5c95c099` |
| local release manifest | 3,672 | `856b878d6acbeb9da7957df3cad8aeeb9cea2bb983a3312b0673e2a8780f9a3b` |

The two binary builds both had the binary digest above. The emitted and check
SBOMs both had the SBOM digest above. The emitted and check archives both had
the archive digest above.

### Linux x86-64 musl

| Item | Bytes | SHA-256 |
| --- | ---: | --- |
| `vela-linux-x86_64.tar.gz` | 3,144,104 | `f150dcc015102f3391d1d127009be5724c937b071ca7b46e0c81b187aecb92b1` |
| normalized SPDX SBOM | 130,941 | `2df9752a057fcb613ba67e0fd8198c676c7acb6d227aba98b4ff398294771ac0` |
| staged `vela` | 8,843,048 | `605c20b1bb3b1369ef60dbee191130bc7e99aee58fbd8bb45d494579133d5615` |
| target notices | 199,744 | `cddb8e3013c44ae492155e2151630c41fcde85edf643a1fe3c2d19f2938b90bb` |
| local release manifest | 3,675 | `430aa74f5424a904fc4ba24fa043473a4a8735588784716aea816246d203d6cb` |

The two binary builds, two normalized SBOMs, and two archive passes matched at
the exact digests above. The real x86-64 musl binary executed in
the `linux/amd64` container during bundle smoke. This is a real target build
under host emulation, not a tar-shape substitution.

Both manifests report candidate commit/tree
`bd18d1a...` / `7187138a...`, `dirty: false`, version `0.977.4`, the correct
target, two binary builds, two archive builds, Rust 1.97.1, cargo-auditable
0.7.5, cargo-about 0.8.4, Syft 1.50.0, and the exact selected-graph and notice
digests. Their tag is null, provider is local, provider-neutral attestation is
false, and no signature sidecar exists.

## Archive shape and package identity

Each archive contains five regular paths: `LICENSE`,
`LICENSE-APACHE`, `LICENSE-MIT`, target-specific
`THIRD-PARTY-LICENSES.txt`, and executable `vela`. Project license bytes equal
the candidate source. The Linux archive fixes owner/group to zero and both
formats use the commit-derived timestamp.

| Target | Selected total | Third party | Workspace | Contained | Build contributor | SPDX relationships |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `aarch64-apple-darwin` | 89 | 85 | 4 | 78 | 11 | 233 |
| `x86_64-unknown-linux-musl` | 89 | 85 | 4 | 77 | 12 | 240 |

For each target:

- the target-selected locked graph has 89 exact name/version identities;
- its 85 non-workspace identities exactly equal the target notice inventory;
- all 89 identities exactly equal the normalized SPDX non-root package set;
- every selected identity is uniquely bound to `Cargo.lock`, including source
  and checksum for registry packages;
- every contained package has a root `CONTAINS` relationship;
- no build contributor has a root `CONTAINS` relationship; all 11 macOS or 12
  Linux contributors have `BUILD_DEPENDENCY_OF` to the document root; and
- every relationship endpoint names the document or a retained package.

The target selections differ. macOS alone selects
`core-foundation-sys 0.8.7` and `errno 0.3.14`; Linux alone selects
`curve25519-dalek-derive 0.1.1` and `linux-raw-sys 0.12.1`.

All 32 packages from the first R6 contradiction are absent from both selected
graphs, both notice inventories, and both normalized SPDX package sets:

```text
base16ct const-oid crypto-bigint der ecdsa elliptic-curve equivalent ff group
hashbrown hmac indexmap lazy_static libm num-bigint-dig num-integer num-iter
p256 p384 p521 pkcs1 pkcs8 ppv-lite86 primeorder rand rand_chacha rfc6979 rsa
sec1 spin spki zerocopy
```

These are the feature-disabled SSH private-key/cryptography subtree. They are
not selected normal dependencies and are no longer represented as archive
contents.

## Adversarial and omission negatives

The repository reproducibility verifier refused extra, missing, and mismatched
package identities; a missing contained runtime package; a dangling
relationship; and notice/selected-graph disagreement. Independent mutations
of the real macOS normalized SBOM also produced the expected refusals for:

- one extra identity;
- one missing identity;
- one mismatched version identity;
- one dangling relationship endpoint; and
- one build contributor incorrectly attached by `CONTAINS`.

The audit harness unpacked both real archives, altered each copy, repacked it
with the deterministic archiver, and passed it to the repository smoke gate.
All ten cases refused:

| Mutation | macOS ZIP | Linux tar.gz |
| --- | --- | --- |
| missing `LICENSE` | refused | refused |
| missing `LICENSE-APACHE` | refused | refused |
| missing `LICENSE-MIT` | refused | refused |
| missing `THIRD-PARTY-LICENSES.txt` | refused | refused |
| altered `LICENSE-MIT` bytes | refused | refused |

The valid archives passed checksum validation, target notice validation,
`vela --version`, integration help, SSH-agent initialization, repository
initialization and status, install, same-version upgrade, and uninstall.

## Verification matrix

| Gate | Result |
| --- | --- |
| Exact commit/tree and clean-source binding | `PASS` |
| Fresh-cache old-order negative and entry-point positive | `PASS` |
| Full macOS release entry point | `PASS` |
| Full Linux x86-64 musl release entry point | `PASS` |
| Two-build binary identity, both targets | `PASS` |
| Two-scan normalized-SBOM identity, both targets | `PASS` |
| Two-pass archive identity, both targets | `PASS` |
| Selected graph / target notices / SPDX / relationships | `PASS` |
| 32 feature-disabled SSH-crypto crates absent | `PASS` |
| Five adversarial SBOM graph negatives | `PASS` |
| Ten real archive omission/alteration negatives | `PASS` |
| `cli_release_contract` | `PASS`; 14/14 |
| `cargo deny --locked check` | `PASS`; advisories, bans, licenses, sources |
| Protocol conformance | `PASS`; 77 normative, 73 informative; root `sha256:bd03d1c9...5ad4b83` |
| Release reproducibility verifier | `PASS` |
| Provider-independent installer | `PASS`; 5/5 |
| Complete Core union, including `vela-cli/test-support` | `PASS` |
| Workspace clippy with `-D warnings` | `PASS` |
| Rust formatting, focused Ruff, shell syntax, workflow YAML, diff check | `PASS` |

The complete Core union also reran Protocol conformance and the release
reproducibility suite before both all-target workspace test passes and Rustdoc
tests. External Lean was not selected, matching the Core entry point's stated
boundary.

## Cache, path, state, and drift checks

The macOS and Linux source checkouts remained clean after their release runs.
Searches over both staged bundles and normalized SBOMs found no audit scratch,
account-home, Cargo-home, source-checkout, worktree, campaign-document, or
R6-report path. The stable compiler remaps remain intentional. Each local
manifest openly records its acquisition remote as provenance; that field is
not packaged into the archive, binary, notices, or SBOM and is not a build
input.

The candidate is absent from current `origin` refs, no local tag points at it,
and the audit output directories contain no signature. No campaign state or
dirty-tree byte enters either release package.

`scripts/release.sh --print-version` reports `0.977.4`; all workspace packages
inherit that version and Rust 1.97.1. Cargo manifests, `Cargo.lock`, and
`schemas/` are byte-identical to public `v0.977.4`. The follow-up repair changes
release machinery, release tests, one documentation index, and conformance
inventory only. It changes no Protocol object, schema, wire, canonical rule,
persisted-data generation, CLI behavior, or Protocol number. The conformance
manifest root changes because it binds the repaired release machinery and two
new `normative: false` release-verifier entries; the normative file count
remains 77, and the manifest-verifier edit only enumerates those informative
paths.

## Residual limitations

- These are local qualification artifacts. They are not the future release
  assets and have no hosted OIDC provenance or distribution signature.
- The Linux x86-64 run used Docker's `linux/amd64` emulation on an Apple-silicon
  host. It built and executed the real musl binary, but it is not a hosted
  native-amd64 run.
- Automated dependency and license-text correspondence is evidence under the
  repository's accepted policy, not legal advice.
- The evidence report commit that follows this candidate changes the source
  commit/tree and does not replace the qualified product candidate.

## Required release-time actions

These actions remain outside this audit and require separate authorization:

1. Integrate accepted evidence, then prepare one bounded `0.977.5` release
   commit. Public `v0.977.4` is an immutable ancestor, so the candidate must not
   be released under the occupied version. Update the aligned workspace
   version, lockfile, current install/citation metadata, and changelog without
   rewriting historical fixtures or reports.
2. Bind the final release-preparation commit/tree and rerun the full locked
   Core union, release-candidate clippy, fresh-cache notice gate, both supported
   release entry points, package correspondence, installer, and hosted
   conformance on that exact source.
3. Only after explicit release authorization, create the SSH-signed annotated
   `v0.977.5` tag at that exact commit. Let the pinned hosted workflow build the
   two draft bundles and supply OIDC provenance.
4. Before publication, verify both draft manifests, archive/SBOM/notice
   checksums, exact commit/tree, tag, and attestations; sign the exact manifests
   with the distribution identity; publish through the existing signing gate;
   and reinstall both signed bundles by exact digest.
5. Keep downstream accepted-generator repinning separate. A same-version or
   local rebuild is not an accepted generator without its exact
   declared digest.

Subject to those release-time actions, the second independent R6 lane returns:

```text
PASS WITH DOCUMENTED RELEASE-TIME ACTIONS
```
