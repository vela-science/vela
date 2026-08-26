# VELA-RC-1 independent R6 packaging requalification

Recorded: 2026-08-26, America/Toronto.

## Disposition

```text
HOLD — RELEASE INTEGRITY
```

VRC1-F012's archive contents are materially repaired: both supported formats
carry exact Vela license files and the same deterministic third-party notice
bundle, the selected normal-dependency union has nonempty license-text
coverage, retained package notices are present, and omission or alteration of
any required notice file fails closed. That positive result does not qualify
the integrated candidate for release.

Two independent candidate gates fail. A fresh pinned Linux release checkout
builds two identical real binaries but cannot generate notices because the
release entry point forces `cargo-about` offline before the complete locked
metadata graph has been fetched. The hosted workflow contains no preceding
locked fetch. Separately, `cli_release_contract` passes 13 of 14 tests and
fails because `R6_PACKAGING_REPAIR.md` is absent from `docs/README.md`.

There is also an unresolved packaged-evidence contradiction. The notice bundle
exactly matches `cargo tree -e normal` for the union of the two targets, while
both Syft SPDX documents claim that the archive contains 32 additional,
feature-disabled SSH cryptography subtree packages and give all of them
`NOASSERTION` licensing fields. This does not demonstrate an uncovered
selected normal dependency, so this audit does not return
`HOLD — ARCHIVE NOTICE INCOMPLETE`. It does prevent the requested exact
notice/SBOM correspondence from passing.

No product, version, tag, signature, public asset, publication, push, release,
or deployment was made. The local archives below are unsigned audit artifacts.

## Exact binding

| Field | Exact value |
| --- | --- |
| Candidate commit | `1dacaa5f1a998ac8aba4d4c46a201f0928d951ab` |
| Candidate tree | `04e84befbf54ac62ce8d789dabd4faa5331d1a49` |
| Integrated packaging commit | `cb803c8b7bff7f3ed12f229d29a74e16011d4c07` |
| Integrated packaging tree | `246ceae4c23a27dcaa9574b0db968b80ab579996` |
| Candidate status before audit | detached at the named commit; clean |
| Version | `0.977.4`, unchanged |
| Rust | `1.97.1 (8bab26f4f 2026-07-14)` |
| Notice generator | `cargo-about 0.8.4` |
| SBOM generator | Syft `1.50.0` |
| Source date epoch | `1787764924` |

The candidate's implementation tree is byte-identical to the earlier isolated
implementation tree. This audit binds the integrated supervisor commit and
does not substitute the isolated implementation commit recorded by the repair
lane.

## Finding 1: the clean release entry point is cache-dependent

A genuine `linux/amd64` Docker run started from the pinned
`rust:1.97.1-bookworm` image, cloned the exact candidate from a temporary bare
repository, installed the `x86_64-unknown-linux-musl` standard library and
musl linker, and checksum-verified the official Syft 1.50.0 Linux amd64
archive. Source commit, tree, and clean status matched before the build.

The unmodified release entry point then:

- built two real musl `vela` binaries in independent target directories;
- compared them byte-for-byte successfully;
- installed pinned `cargo-about 0.8.4`; and
- exited at the first frozen notice generation without producing an archive.

The release command uses `cargo about -L off`, which suppresses the useful
diagnostic. Repeating the exact generator invocation at information log level
gave status 1 and:

```text
`cargo metadata` exited with an error: error: failed to download
`android_system_properties v0.1.5`

Caused by:
  attempting to make an HTTP request, but --frozen was specified
```

This is not a source-dirtiness, tool-version, network-transport, QEMU, linker,
or binary-reproducibility failure. `cargo fetch --locked` downloaded the
unfetched lock entries, after which the same frozen cargo-about invocation
passed. `.github/workflows/release.yml` calls `scripts/release.sh` directly and
contains no complete locked-graph fetch, so a fresh hosted cache has the same
unmet precondition.

For positive artifact inspection only, the locked graph was explicitly
prefetched, the already-qualified container prerequisites were provisioned
(container-local `/etc/machine-id` plus Git operator identity), and the
unchanged release entry point was rerun with Docker networking disabled. The
source checkout remained clean before and after. That run completed the full
Linux release archive, SBOM, checksum, operator smoke, and manifest path. It
proves the build is locally feasible; it does not erase the clean-entry-point
failure.

Release qualification requires the repository-controlled release path to own
the locked fetch or otherwise make frozen metadata generation independent of
ambient Cargo cache, with a clean-cache regression. A diagnostic should also
survive failure instead of being hidden by `-L off`.

## Finding 2: the release contract is red

After the required locked debug CLI build:

```text
cargo test --locked -p vela-protocol --test cli_release_contract
13 passed; 1 failed
```

The exact failing assertion is:

```text
docs/README.md does not link ["campaigns/vela-rc-1/R6_PACKAGING_REPAIR.md"]
```

This audit does not repair the candidate. The audit report itself is indexed
as report evidence, while the candidate's missing packaging-repair link is
left unchanged for the owning lane.

## Archive receipts

### macOS Apple silicon

The unmodified release entry point ran locally against the exact candidate. It
built and compared two binaries, generated and compared notices twice, ran and
normalized Syft twice, created the ZIP twice, checked sidecars, completed the
full operator/install smoke, and emitted a manifest bound to the candidate
commit/tree with `dirty: false`.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `vela-macos-aarch64.zip` | 2,743,979 | `21d804f4e67c9d8f7d7ea8d0c8f5057f47501f6776574967b3438a4f7620cba9` |
| `vela-macos-aarch64.zip.spdx.json` | 194,324 | `3bbeeada897dc1f15a4aa98552746625d3119db3cb71076bab9d40e8c164b2ec` |
| `release-manifest.json` | 3,375 | `6ee31814d81969521e55502c882e7e567808fc36d5fcf68f520f0f7858ffb295` |
| staged `vela` | 7,380,240 | `aa7e47802b7ed7b0d9cdfcbe14a5c2162d841e7e87c5be36da947376e0f3b3a7` |

The payload hashes equal the earlier implementation-lane inventory. The ZIP
hash differs because this integrated candidate has a later commit-derived
archive epoch; its two candidate-bound archive passes agree exactly.

### Linux x86-64 musl

After the explicit locked prefetch and documented runtime preparation described
above, the actual release run was network-disabled. It built and compared two
real musl binaries, generated notices twice, ran two Syft scans, created the
tarball twice, completed the full archive execution/repository/install smoke,
and retained a clean source checkout.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `vela-linux-x86_64.tar.gz` | 3,144,298 | `f123d75f534ac641bd64b0db5167634b5753a71e125bccd921519789e535bf04` |
| `vela-linux-x86_64.tar.gz.spdx.json` | 204,553 | `64d9319512d0ab0fca867ff6da3bc328ae78a6a0f875ebdfbd5158a48285499c` |
| `release-manifest.json` | 3,362 | `a05d81e261e1d8ea31c373c307d87c3d43ebf8c4931fe32bdf5b86abbea92eb7` |
| staged `vela` | 8,843,048 | `605c20b1bb3b1369ef60dbee191130bc7e99aee58fbd8bb45d494579133d5615` |

Two separate offline release executions produced the same archive and SBOM
hashes. The first stopped during operator smoke because the unprepared base
image lacked the already-documented container-local machine identity; after
that runtime prerequisite was provisioned, the second completed. Within each
execution, both binary builds and both deterministic archive passes also
agreed.

### Exact inventory in both formats

| Path | Mode | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `LICENSE` | `0644` | 643 | `dcfa071fa15664e4f14251a964838811ecc7d0ef07f1982e9aae50330472fbea` |
| `LICENSE-APACHE` | `0644` | 11,358 | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `LICENSE-MIT` | `0644` | 1,089 | `df1a810625de2c1ef2c49ce68b6a20b9e730f03b8e490e091df853c848e13aae` |
| `THIRD-PARTY-LICENSES.txt` | `0644` | 200,364 | `6f210c1acefd5993c0c07108482ef206a8996efeb7552bb7754cbe32842c7d96` |
| macOS `vela` | `0755` | 7,380,240 | `aa7e47802b7ed7b0d9cdfcbe14a5c2162d841e7e87c5be36da947376e0f3b3a7` |
| Linux `vela` | `0755` | 8,843,048 | `605c20b1bb3b1369ef60dbee191130bc7e99aee58fbd8bb45d494579133d5615` |

The three project license payloads are byte-equal to the candidate source.
The Linux tar fixes UID/GID to 0 and all member mtimes to `1787764924`; both
formats use the modes above and the same ordered five-path inventory.

No account, worktree, Cargo registry, temporary, source, or target directory
from either host appears in the notice, SBOM, or archive binary. The binary's
intentional stable remaps such as `/build/cargo-home` and ordinary runtime
`/tmp/` vocabulary are not ambient host paths. The local Linux audit manifest
records its temporary bare remote as `/candidate.git`; the archive and SBOM do
not, and the source commit/tree/dirty binding is exact.

## Notice and locked-graph inspection

Both archives contain byte-identical notice material with:

| Field | Exact value |
| --- | --- |
| Format | `vela.third-party-notices.v1` |
| Targets | `aarch64-apple-darwin, x86_64-unknown-linux-musl` |
| Package identities | 87 rows, 85 unique package names |
| License texts | 25 nonempty harvested texts |
| Additional retained notices | 4 |
| Locked dependency graph root | `455182105e6f19fdfd7d66f063061bc7ed787b8d92fa260cdd4272500bd40f3c` |
| Notice SHA-256 | `6f210c1acefd5993c0c07108482ef206a8996efeb7552bb7754cbe32842c7d96` |

The four retained package-root notices are:

- `linux-raw-sys 0.12.1/COPYRIGHT`;
- `rand_core 0.6.4/COPYRIGHT`;
- `rustix 1.1.4/COPYRIGHT`; and
- `unicode-normalization 0.1.25/COPYRIGHT`.

Independent set comparisons established:

- the 87 notice package identities exactly equal the 87 crates in the pinned
  cargo-about JSON;
- Linux has 83 unique selected normal-dependency names;
- macOS has 83 unique selected normal-dependency names;
- their union has 85 unique names; and
- that union exactly equals the notice inventory, with no missing or extra
  package name.

Every cargo-about crate is bound to an exact `Cargo.lock` name, version,
registry source, and checksum. Every one is covered by at least one nonempty
license text. `about.toml`'s accepted list equals `deny.toml`'s allow list, and
the manifest records the pinned generator plus exact digests of `Cargo.lock`,
`about.toml`, `cargo-about-version`, `deny.toml`, the normalizer, and the notice
output.

## Notice/SBOM discrepancy

The Linux SPDX contains 116 package records and 115 unique names; the macOS
SPDX contains 111 package records and 110 unique names. Both SPDX documents
claim 32 third-party names that are absent from both target-specific normal
dependency trees and from the exact notice union:

```text
base16ct const-oid crypto-bigint der ecdsa elliptic-curve equivalent ff group
hashbrown hmac indexmap lazy_static libm num-bigint-dig num-integer num-iter
p256 p384 p521 pkcs1 pkcs8 ppv-lite86 primeorder rand rand_chacha rfc6979 rsa
sec1 spin spki zerocopy
```

These form the feature-disabled private-key/cryptography subtree beneath
`ssh-key`; Vela configures that crate with `default-features = false` and only
`std`. `cargo tree -p vela-cli --target <target> -e normal` has no reverse path
to them. Nevertheless the canonical SPDX relationships label them as
`CONTAINS` and, for example, `rsa` and `p256` as dependencies of `ssh-key`.
Their declared license, concluded license, and copyright fields remain
`NOASSERTION`.

The notice evidence is exact for the selected normal graph, but the two
packaged release records disagree about what the archive contains. R6 cannot
call notice/SBOM correspondence exact until the SBOM extraction or notice
policy is made consistent and a cross-check enforces the chosen truthful
graph. This is an integrity hold, not evidence that the current notice omitted
an actually selected normal dependency.

## Negative and smoke evidence

The repository reproducibility test passed its deterministic tar/ZIP, notice,
SBOM, and manifest checks. It also refused an absent lock graph, uncovered
license text, each of four missing required files, and an altered project
license.

The audit additionally modified the real macOS and Linux archives outside the
repository. For both ZIP and tar, all ten cases failed with the expected exact
reason:

- missing `LICENSE`;
- missing `LICENSE-APACHE`;
- missing `LICENSE-MIT`;
- missing `THIRD-PARTY-LICENSES.txt`; and
- altered `LICENSE-MIT` bytes.

The valid macOS and prepared Linux archives passed checksum validation, notice
validation, `vela --version`, integration help, SSH-agent initialization,
repository init/status assertions, install, same-version upgrade, and
uninstall. The built x86-64 binary executed successfully inside the
`linux/amd64` container; this was not a tar-shape substitution.

## Verification matrix

| Gate | Result |
| --- | --- |
| Candidate commit/tree/status binding | `PASS` |
| macOS full release archive | `PASS` |
| Linux two real musl binaries | `PASS` |
| Linux clean-cache entry point | `FAIL` — frozen cargo metadata needs an unfetched lock entry |
| Linux prefetched, runtime-prepared, network-disabled full archive | `PASS` |
| Project licenses and notices in both formats | `PASS` |
| Notice package/text coverage and retained notices | `PASS` |
| Ten real archive omission/alteration negatives | `PASS` |
| Notice/SBOM exact correspondence | `FAIL` — 32-package contradiction |
| `uv run --project conformance --locked python conformance/verify_release_reproducibility.py` | `PASS` |
| `uv run --project conformance --locked python conformance/test_release_install.py` | `PASS` — 5 tests |
| `cargo deny --locked check` | `PASS` — advisories, bans, licenses, sources |
| `cargo build --locked -p vela-cli` | `PASS` |
| `cargo test --locked -p vela-protocol --test cli_release_contract` | `FAIL` — 13/14 |
| `uv run --project conformance --locked python conformance/verify.py` | `PASS` — 77 normative, 71 informative; root `sha256:c3d9b95946d82c34c3ca01c911845df9598c74804f5b4c6def7a4b6c4181d2aa` |
| focused Ruff, `cargo fmt --all -- --check`, shell syntax, and `git diff --check` | `PASS` |

The provider-independent installer cases passed signed install, wrong signer
refusal, unsigned strict-mode refusal, archive/manifest digest mismatch
refusal, and generator/installer digest agreement.

## Limits and required requalification

- The audit archives are local, unsigned, not OIDC-attested, unpublished, and
  not release-authorized bytes.
- No GitHub-hosted release workflow was dispatched. The workflow was inspected
  and has no locked-graph prefetch before the failing entry point.
- Automated license harvesting and policy agreement are evidence, not legal
  advice.
- The exact release entry point and release-contract test must both be green,
  and the notice/SBOM graph contradiction must be resolved or precisely
  constrained, before R6 can pass.
- Any repair requires a fresh independent R6 audit of both archive formats and
  a genuinely fresh-cache Linux run.

The independent auditor therefore returns:

```text
HOLD — RELEASE INTEGRITY
```
