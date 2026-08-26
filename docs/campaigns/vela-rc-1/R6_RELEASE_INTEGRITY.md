# VELA-RC-1 R6 packaging and release-integrity qualification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
HOLD — RELEASE INTEGRITY
```

The release lineage and artifact-traceability design are adequate, but the
current package cannot be qualified for distribution. Both supported archives
stage only the `vela` executable. They include none of Vela's `LICENSE`,
`LICENSE-APACHE`, or `LICENSE-MIT` texts and no deterministic third-party
notice bundle. The separately published SPDX documents inventory the Rust
packages but report `NOASSERTION` for every package's declared license,
concluded license, and copyright text. `cargo deny` proves that the locked
dependency graph satisfies the repository's policy; it does not make the
distributed archive carry the applicable notices.

R6 therefore cannot establish package-license completeness from the bytes a
consumer downloads. The unchanged release entry point would reproduce the
same omission for RC-1. This is a release-integrity blocker, not a claim of a
particular legal conclusion. A bounded packaging repair must retain the
project license texts, produce deterministic third-party notice material from
the locked graph, and make the archive smoke gate refuse their absence before
fresh independent qualification.

The separate version-lineage issue also requires a release-time action. The
signed and immutable public `v0.977.4` tag and artifacts identify an ancestor,
while the qualified candidate still reports `0.977.4`. R6 recommends exactly:

```text
PATCH BUMP
```

No tag, push, publication, signing, version edit, release asset, or release was
created. R7 and release remain unauthorized.

## Exact audit binding

| Field | Exact value |
| --- | --- |
| Delegated supervisor commit | `d8ae06b1faf2903d234ab6c380bd9cadd8c6065f` |
| Delegated supervisor tree | `2fce1bb6e5baa72b1a9a91ea70266afd6aa8d72d` |
| Initial and final audit checkout | detached at the exact supervisor commit; clean |
| Candidate source snapshot | SHA-256 `667b6f14f3cc736a0ff7714696f333a3df4002b3c9b4566a08cd1aefa08a8bcc` |
| Snapshot construction | exact stdout of `git archive --format=tar d8ae06b1faf2903d234ab6c380bd9cadd8c6065f` |
| Candidate version source | workspace `Cargo.toml`, `0.977.4` |
| Candidate toolchain | Rust `1.97.1`, edition 2024; `rustfmt` and `clippy` pinned |
| Protocol selection | Vela Protocol 1 release candidate |
| Current merged Protocol root | `sha256:553c2bf5b495506e5297027c47abd68e058f1a34136900fc4e4606c81d311a17` |
| Local locked release-build binary | SHA-256 `b23ffd6dd9f6d01235369386e4582b55350cd18af70a4129bd414b8b1e16803d` |
| Candidate hosted runs | none; the exact commit is not advertised by a public ref |
| Release authority | not authorized |

The source snapshot digest is an audit-only identity over one deterministic
Git archive stream. It is not a release artifact or a substitute for the Git
commit and tree. The local binary digest identifies only this source build; it
is not an accepted release digest and is not claimed reproducible across build
paths or hosts.

## Public ancestor lineage

The public tag is an annotated SSH-signed tag object
`388c3a5d1b71a8b6dacfcfa17ffcd395710f3858`. Its signature verified locally,
and it peels to:

| Field | Public `v0.977.4` value |
| --- | --- |
| Commit | `1a2e0328620b4e8c4584c3d4baf257adb11f3d45` |
| Tree | `1bd8ed4e11d3745f159b32f23539f5174fd44803` |
| Release workflow | run `32447842087`, successful |
| Publication | 2026-08-21T04:48:24Z; non-draft, non-prerelease, immutable |
| Assets | 12: two archives, two SPDX SBOMs, four checksum sidecars, two release manifests, and two manifest signatures |
| Linux archive / binary | `sha256:8ce4f50e...90d7cf` / `sha256:f73e2378...03c64` |
| macOS archive / binary | `sha256:023bf4d9...25d65` / `sha256:06f912d1...bd05e` |

Both provider-neutral manifest signatures verified against `allowed_signers`
as `release@vela.space` in namespace `vela-release`. Every archive and SBOM
matched the digest in its signed manifest. GitHub OIDC provenance verified for
each archive, SBOM, and manifest and bound workflow `.github/workflows/release.yml`,
tag `v0.977.4`, and source commit `1a2e032...`. The two manifests independently
record that commit, tree `1bd8ed4e...`, Rust `1.97.1`, two compared binary
builds, two compared archive passes, and one fixed `SOURCE_DATE_EPOCH`.

This establishes that the public bytes are traceable to the public ancestor.
It is also conclusive evidence that they are not RC-1 candidate bytes.

## Version recommendation and change classification

`PATCH BUMP` is required before any later authorized release. Under Vela's
`0.RRR.P` policy, the release train remains `0.977` and the next available
compatible patch is `0.977.5`. `v0.977.4` is already public and immutable. A
second source tree that prints the same version would make version text
ambiguous, and the release entry point correctly refuses `--tag v0.977.5`
while the workspace still reports `0.977.4`.

| Change class from signed `v0.977.4` | Classification | Evidence and version consequence |
| --- | --- | --- |
| Documentation-only | present | Architecture, Protocol explanation, first-user material, campaigns, papers, and examples changed. These do not alone require a software minor or Protocol bump. |
| Bug fix | present | Governed reads now enforce the already normative independent sequence-one pin and fail closed on missing, malformed, or mismatched OS-account trust. This is the release-driving patch fix. |
| Schema / wire | none | `schemas/`, all Cargo manifests and the lockfile are byte-identical to `v0.977.4`; Submission remains v3, Verification Record v2, Proposal v1, Proposal withdrawal v2, Repository v4, Status v4, Claim Record v1, and Decision Inbox v3. |
| Persisted data | none | No canonical object, root rule, journal vocabulary, repository layout, or accepted data changes. |
| Migration | none | No migration writer or dual reader was added; current persistent data needs no migration. |
| CLI breaking | no command or JSON-shape break; one intentional fail-closed precondition | Consumers that relied on the old permissive unpinned reads must install the independently published pin. That observable tightening repairs a semantic defect rather than selecting a new CLI or wire generation. |
| Protocol | no Protocol-number or selection change | Normative text now makes the existing trust-selection requirement operationally explicit and the manifest records a fourth informative flow. Protocol 1 remains a release candidate. |
| Packaging | blocker | The prospective archives omit license and third-party notice material. Fixing package contents remains compatible and belongs in the same patch release. |

`KEEP VERSION` is impossible because the signed immutable tag already owns
`0.977.4`. `MINOR/PRE-1.0 BUMP` is disproportionate because no new product
surface, wire generation, or persisted-data contract was introduced.
`PROTOCOL BUMP` would misstate the qualified semantic result. `HOLD` is the
current release verdict because of packaging integrity, not the version class.

## Qualification matrix

| Surface | Result | Exact evidence or required action |
| --- | --- | --- |
| Version source and workspace alignment | `PASS WITH RELEASE-TIME ACTION` | All four workspace packages report `0.977.4`; `scripts/release.sh --print-version` agrees. Prepare one bounded `0.977.5` release commit later. |
| Locked Rust and Python dependencies | `PASS` | `Cargo.lock`, `conformance/uv.lock`, Cargo manifests, and toolchain pin are unchanged from the signed tag and latest green hosted inputs; locked metadata resolved. |
| Dependency policy | `PASS` | `cargo deny --locked check`: advisories, bans, licenses, and sources passed. This does not close archive notice completeness. |
| Release entry point | `PASS` | One script owns version/tag agreement, pinned toolchain, two path-remapped auditable builds, deterministic SBOM normalization, two archive passes, checksums, smoke, and the release manifest. |
| Reproducibility machinery | `PASS` | `verify_release_reproducibility.py` reproduced path/time-independent tar.gz, zip, SBOM, and manifest bytes. Local release build from the exact candidate passed. |
| Release manifest | `PASS` | It records clean Git source commit and tree, binary and asset SHA-256, target, toolchain, build command, fixed epoch, and compared-build counts. |
| Hosted provenance | `PASS WITH RELEASE-TIME ACTION` | Workflow uses pinned Actions and OIDC attestation. Public provenance verified for the ancestor; exact RC-1 source has no hosted run and must be rerun after the final patch-prep commit. |
| Checksums and signature support | `PASS` | Archive/SBOM checksums, OpenSSH manifest signatures, tag signature, and all public ancestor assets verified. Signing remains out-of-band and draft-before-publication. |
| Tag semantics | `PASS WITH RELEASE-TIME ACTION` | Hosted metadata binds `vX.Y.Z` to the workspace version; signing verifies tag commit equals manifest source commit; publication verifies the tag and starts as an invisible draft. Use a new signed `v0.977.5` tag only after authorization. |
| Installer | `PASS` | Five provider-independent cases passed: valid signed install, wrong signer refusal, unsigned non-provenance, asset mismatch refusal, and generator/installer digest agreement. |
| Archive license and notices | `BLOCKER` | Both published platform archives contain only `vela`. No project license files or third-party notice bundle is packaged. |
| SPDX licensing fields | `BLOCKER INPUT` | Linux inventories 116 packages and macOS 111, but every declared/concluded license and copyright field is `NOASSERTION`; there is no extracted-license text. |
| Protocol / schemas | `PASS` | No schema or wire delta; current merged Protocol root reproduced by the prior accepted gate. |
| Migration | `PASS` | None required or present. |
| Changelog / release notes | `RELEASE-TIME ACTION` | `CHANGELOG.md` has an empty `Unreleased` section; current release docs truthfully still name the public ancestor. Draft content is below. |
| Hosted CI | `RELEASE-TIME ACTION` | Latest conformance is green at public `main` commit `23c2eb86...`, run `32794177685`; no run exists for `d8ae06b1...`. |
| Repository cleanliness | `PASS` | Exact worktree remained clean; `git diff --check`, formatting, and `git fsck --strict` passed. Dangling unreachable objects were informational, not corruption. |
| Ignored `dist/` residue | `NOT EVIDENCE` | The R6 worktree contains no `dist/` file. Baseline checkout residue is an ignored macOS-only `v0.977.2` bundle for commit `c1a34373...`; it is excluded from every conclusion. |
| Candidate artifact traceability | `SUPPORTED, NOT YET EXERCISED` | No candidate release artifact exists. After the blocker and patch bump, the established manifest, tag, provenance, digest, and signature chain can bind exact bytes to the final qualified source. |

## Smallest reproduced checks

Passed on the exact candidate unless a different source is stated:

```text
scripts/release.sh --print-version
0.977.4

scripts/release.sh --tag v0.977.5 --out <temporary>
expected refusal before build: tag v0.977.5 does not match workspace v0.977.4

uv run --project conformance --locked \
  python conformance/verify_release_reproducibility.py
PASS

uv run --project conformance --locked \
  python conformance/test_release_install.py
PASS; 5 tests

cargo build --locked --release -p vela-cli --bin vela
PASS; local binary sha256:b23ffd6d...e16803d

cargo test --locked -p vela-protocol --test cli_release_contract
PASS; 14 tests

cargo deny --locked check
PASS; advisories, bans, licenses, sources

cargo fmt --all -- --check
git diff --check
git fsck --strict
PASS
```

The first documentation-contract invocation occurred before this fresh target
tree contained a `vela` binary. Its ten file-only assertions passed and four
executable-help assertions failed with `ENOENT`. After the planned locked
release build, the same unmodified target passed all 14 tests. This was an
audit setup error, not a product failure.

R6 did not run the full release entry point because it would generate a local
bundle under the already occupied version and add no evidence beyond the
focused deterministic-package, installer, locked-build, and live published-
artifact checks. It did not rerun the full Core union or workspace clippy,
which prior exact-candidate gates already passed and which the final repaired
release-prep commit must rerun.

## Required packaging repair before requalification

The smallest credible repair remains inside the current release machinery:

1. Stage Vela's exact `LICENSE`, `LICENSE-APACHE`, and `LICENSE-MIT` files in
   both supported archives.
2. Produce deterministic third-party license/notice material from the exact
   locked dependency graph. Record the generator and its pinned version in the
   release process; do not treat `cargo deny` output or `NOASSERTION` SBOM
   fields as notices.
3. Make `scripts/release.sh` and `.github/release/smoke-bundle.sh` refuse an
   archive missing any required project or third-party notice file.
4. Add the narrow reproducibility and negative tests that prove those files
   have stable bytes and cannot silently disappear.
5. Independently inspect both repaired archives and their SPDX/notice
   correspondence. Keep the release on hold until that passes.

This repair changes package contents and release tests only. It must not alter
Protocol objects, canonical bytes, repository authority, Standing, or source-
owned scientific artifacts.

## Release-time checklist after the blocker closes

These are actions for a later explicitly authorized release process. They are
not performed or authorized by this report.

1. Integrate accepted R5/R6 evidence and the bounded packaging repair, then
   independently requalify its exact commit and tree.
2. Prepare one bounded `0.977.5` metadata commit. Update the workspace version,
   `Cargo.lock`, current citation/release metadata, current install examples,
   and `CHANGELOG.md`; leave exact historical fixture versions and historical
   release statements unchanged.
3. Regenerate the informative Protocol manifest only for actual selected-file
   drift. Confirm the normative schema/wire selection remains unchanged.
4. Record the final commit, tree, clean status, and a freshly computed exact
   source snapshot digest. This final release source cannot be named today
   because the required packaging and version commits do not yet exist.
5. Run the complete locked Core union, release-candidate clippy gate, both
   supported package builds, archive/notice checks, clean-install fixture, and
   exact hosted conformance on that same source commit.
6. Only after user authorization, create the SSH-signed annotated `v0.977.5`
   tag at that exact commit. Let `.github/workflows/release.yml` create the
   checksummed, SBOM-bearing, OIDC-attested draft.
7. Before publication, verify both draft manifests name the exact tag commit
   and tree; verify every archive, SBOM, notice, checksum, and attestation;
   sign the exact published manifests with the distribution identity; then
   publish through `scripts/sign-published-release.sh`.
8. Reinstall each supported signed bundle by exact version and digest. Keep
   downstream repinning separate and invoke the accepted binary by absolute
   path.

## Draft release-note content

### Vela 0.977.5 — fail-closed governed reads and qualified external examples

- Governed read surfaces now enforce the independently published sequence-one
  authority root from the operating-system account trust store. Missing,
  malformed, and mismatched pins fail closed, and `HOME` cannot supply or
  override the selected lineage.
- Authenticated Submission, Verification, and producer-withdrawal writes remain
  unprivileged and change no Standing. Only an authorized attributed Decision
  admits a state transition.
- Release-facing documentation now names the install, Git, Verification, and
  trust prerequisites, and two bounded examples reproduce failure, rejection,
  correction or complete review, Decision, and replay without a Core fork.
- The release archives retain deterministic project and third-party license
  material alongside the executable, checksums, SPDX inventory, signed
  provider-neutral manifests, and hosted build provenance.

Explicit limitations: cumulative workflow advantage is not established. This
release makes no autonomous-discovery or foundational-intelligence claim.
Replay validates and reconstructs retained governed state; it is not a physical
experiment rerun or a native model, proof, or scientific Method rerun. The
examples do not establish external adoption or scientific truth beyond their
exact retained claims. Disposable macOS candidate qualification and the blind
R7 external-user simulation remain separate gates unless later completed and
accepted.

## Disposition

R6 returns `HOLD — RELEASE INTEGRITY` solely because the distributable package
does not yet carry qualifiable license/notice material. Artifact-to-source
traceability, checksums, signatures, tag semantics, locked inputs, and
reproducibility machinery otherwise pass or have explicit release-time actions.
The required version recommendation is `PATCH BUMP`; do not perform it until
the packaging blocker is repaired, independently requalified, and a release is
separately authorized.
