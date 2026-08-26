# VELA-RC-1 R6 bounded packaging repair

Recorded: 2026-08-26, America/Toronto.

## Disposition

```text
READY FOR INDEPENDENT R6 REQUALIFICATION
```

VRC1-F012 is repaired in the repository-controlled release path. Both archive
formats now require the exact Vela project license files and one deterministic,
reviewable third-party license and notice bundle. The generator is pinned,
forced offline, bound to `Cargo.lock`, the two supported targets, and the
accepted policy in `deny.toml`, and recorded with all input digests in the
provider-neutral release manifest. This is distributable notice-completeness
evidence under the repository's accepted dependency-license policy, not a
legal conclusion.

No Protocol object, schema, wire field, persisted data, CLI behavior, Standing,
version, Cargo package version, tag, public asset, signature, publication,
push, or release changed. Independent R6 requalification remains mandatory.

## Exact repair binding

| Field | Exact value |
| --- | --- |
| Delegated supervisor commit | `d7451b00d1616d862f1be01569677056b8efe854` |
| Packaging implementation commit | `0470d4781f5fd27d97baba414873a138fd1330d2` |
| Packaging implementation tree | `246ceae4c23a27dcaa9574b0db968b80ab579996` |
| Initial and implementation-final status | detached at the named commit; clean |
| Vela version | `0.977.4`, unchanged |
| Rust toolchain | `1.97.1`, unchanged |
| Notice generator | `cargo-about 0.8.4`, installed with `cargo install --locked` |
| Generator mode | `--frozen --fail --locked --workspace`; no network lookup during generation |
| Notice scope | union of normal dependencies for `aarch64-apple-darwin` and `x86_64-unknown-linux-musl`; private workspace, dev-only, and build-only crates excluded |
| Notice result | 87 packages, 25 distinct harvested license texts, 4 package-root copyright notices |
| Informative conformance root | `sha256:c3d9b95946d82c34c3ca01c911845df9598c74804f5b4c6def7a4b6c4181d2aa`; 77 normative and 71 informative files |

The four new informative conformance entries bind the notice policy, generator
pin, generator, and archive gate. The normative count remains 77; no normative
Protocol byte or schema changed.

## Exact changed files

- `.github/release/about.toml` — accepted-policy mirror, supported targets,
  dependency exclusions, and checksum-bound cargo-about workarounds.
- `.github/release/cargo-about-version` — single `0.8.4` generator pin used by
  the release and smoke paths.
- `.github/release/generate-third-party-notices.py` — validates every selected
  package against `Cargo.lock`, requires license coverage, collects package-root
  `NOTICE`, `COPYRIGHT`, and `AUTHORS` material, removes ambient paths, and emits
  deterministic `vela.third-party-notices.v1` text.
- `.github/release/check-notice-bundle.py` — requires exact project licenses,
  the generated notice file, current input hashes, generator identity, and
  nonempty package/license sections.
- `.github/release/smoke-bundle.sh` — checks unpacked notices before executing
  the binary and exposes a focused `--notices-only` negative-test path.
- `scripts/release.sh` — stages exact licenses, generates notices twice,
  compares bytes, refuses private paths, checks the staged/unpacked bundle, and
  passes generator/input identities into the release manifest.
- `scripts/release_manifest.py` — records the notice generator, version, notice
  digest, and five exact input digests; refuses missing or duplicate inputs.
- `conformance/verify_release_reproducibility.py` — proves deterministic ZIP
  and tar inventories, path-independent notice generation, locked-graph and
  coverage refusals, exact-license refusal, and four missing-file smoke cases.
- `conformance/test_release_install.py` — supplies the required notice-manifest
  inputs while retaining the five existing end-to-end installer cases.
- `conformance/verify_protocol_1.py` and `conformance/protocol-1.json` — bind
  the four release-notice files as informative release machinery.

No Cargo manifest, `Cargo.lock`, Rust crate source, schema, normative Protocol
file, installer, workflow, release note, or version file changed.

## Generator and locked inputs

The clean macOS manifest recorded:

| Input | SHA-256 |
| --- | --- |
| `Cargo.lock` | `d2817b798e56f0458d782f2d8878765bc4af61cefee45a5b04d68eab4cab98a6` |
| `.github/release/about.toml` | `cc4db808b781ba1e38b0b8307a75884ad6fcc43fbf962362ca66955ab889e881` |
| `.github/release/cargo-about-version` | `cf6a05db55c83ecdf0b3323be40d441e20f4bbd3860a3f4104a9dab70fbae161` |
| `deny.toml` | `9bf58cf01f4681b824013f1b5edff1bc95a78c2ba1179c0500379b8a02a093b5` |
| `.github/release/generate-third-party-notices.py` | `2f1ccf654298991500387476df4e74ba95e6de9579f0984f934737f77e06c33f` |
| Generated `THIRD-PARTY-LICENSES.txt` | `6f210c1acefd5993c0c07108482ef206a8996efeb7552bb7754cbe32842c7d96` |

`about.toml` must exactly match `deny.toml`'s accepted-license list or the
normalizer fails. Each registry package must match an exact name, version,
source, and checksum-bearing `Cargo.lock` entry, and every selected package
must be covered by nonempty harvested license text. The generator then retains
all package-root notice/copyright/author files from those exact package source
directories. Absolute registry, Cargo-home, account-home, source, and build
paths are excluded from the distributable output.

## Clean macOS archive evidence

From clean implementation commit `0470d478...`, the unmodified release entry
point built `aarch64-apple-darwin` twice, compared the binaries, generated the
notice material twice, compared it, ran two canonical Syft scans, created the
ZIP twice, compared it, verified both sidecar checksums, and completed the full
bundle smoke/operator path.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `vela-macos-aarch64.zip` | 2,743,979 | `dd695cbb0f4c10b9fdcb9b57b6a9e48feec8279295f7aff1f982fbfbb66b5aa0` |
| `vela-macos-aarch64.zip.spdx.json` | 194,324 | `8639b15f5c6f598cb547443357d5b264ac7dacf9a10d46eeedb7744745e97015` |
| `release-manifest.json` | 3,375 | `eed7b1cdde1c474ab802e60c4faed5b53787904c38656b2266eab5b4bbd68056` |
| staged `vela` | 7,380,240 | `aa7e47802b7ed7b0d9cdfcbe14a5c2162d841e7e87c5be36da947376e0f3b3a7` |

The manifest records source commit/tree `0470d478...` / `246ceae4...` with
`dirty: false`, two binary builds, two archive passes, cargo-about `0.8.4`, and
the exact notice/input digests above. Syft recovered 110 package names. These
are temporary local qualification bytes, not signed, published, accepted, or
release-authorized assets.

### Exact archive inventory

| Path | Mode | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `LICENSE` | `0644` | 643 | `dcfa071fa15664e4f14251a964838811ecc7d0ef07f1982e9aae50330472fbea` |
| `LICENSE-APACHE` | `0644` | 11,358 | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `LICENSE-MIT` | `0644` | 1,089 | `df1a810625de2c1ef2c49ce68b6a20b9e730f03b8e490e091df853c848e13aae` |
| `THIRD-PARTY-LICENSES.txt` | `0644` | 200,364 | `6f210c1acefd5993c0c07108482ef206a8996efeb7552bb7754cbe32842c7d96` |
| `vela` | `0755` | 7,380,240 | `aa7e47802b7ed7b0d9cdfcbe14a5c2162d841e7e87c5be36da947376e0f3b3a7` |

## Linux archive-shape evidence and limitation

The deterministic `tar.gz` path was run twice from the same clean staged tree,
matched byte-for-byte at
`sha256:2c9ded07d4d20eec7a9a0f460aa7c54b56873af2d0ccd44f616a9c1f222d647b`,
and passed `smoke-bundle.sh --notices-only`. Its five paths, modes, sizes, and
payload hashes exactly matched the table above; tar ownership was fixed to
UID/GID 0 and all timestamps to the commit-derived epoch.

This tar receipt proves the Linux archive shape and notice gate, not a Linux
binary. A clean Docker amd64 attempt used the previously qualified pinned
`rust:1.97.1-bookworm` image, musl target/linker, and checksum-verified Syft
1.50.0. Both Linux binary builds matched, but the bounded execution window
ended after installing cargo-about 0.8.4 and before notice staging/SBOM/archive
output. The first transport attempt had also failed before build because the
host linked-worktree `.git` file named an unavailable host path; retry through
a temporary bare ref fixed that harness error. Independent R6 must complete the
real Linux archive build and compare its notice/SBOM correspondence. No Linux
artifact hash is claimed here.

## Exact verification

Passed on the repair, unless the platform limitation above says otherwise:

```text
scripts/release.sh --out <temporary-clean-output>
PASS: clean macOS build; two binaries, two notice generations, two SBOM scans,
two ZIP passes, checksums, full bundle smoke, and manifest

uv run --project conformance --locked \
  python conformance/verify_release_reproducibility.py
PASS: deterministic tar.gz, ZIP, notices, SBOM, manifest; locked-graph,
coverage, altered-license, and four missing-file negative cases

uv run --project conformance --locked python conformance/test_release_install.py
PASS: 5 tests; valid signed install, wrong signer refusal, unsigned refusal,
asset mismatch refusal, and generator/installer digest agreement

cargo deny --locked check
PASS: advisories, bans, licenses, sources

cargo build --locked -p vela-cli
cargo test --locked -p vela-protocol --test cli_release_contract
PASS: build and 14 release-contract tests

uv run --project conformance --locked python conformance/verify.py
PASS: 77 normative, 71 informative; root sha256:c3d9b959...4181d2aa

uv run --project conformance --locked ruff check <changed Python files>
cargo fmt --all -- --check
bash -n scripts/release.sh .github/release/smoke-bundle.sh
git diff --check
PASS
```

The first direct `cli_release_contract` invocation found no `target/debug/vela`:
its ten file-only tests passed and four executable-help cases failed with
`ENOENT`. After the required locked CLI build, the same unchanged target passed
14 of 14. This was an audit setup error, not a product failure. A repository-wide
Ruff invocation also reported 84 pre-existing findings under `paper/`; focused
Ruff over every changed Python file passed, and the selected conformance entry
point passed its own Python gates.

## Remaining limits

- A real Linux x86-64 release archive still requires completion and independent
  inspection; only the binary reproducibility stage and exact tar notice shape
  were completed locally.
- The temporary macOS and tar bytes are local qualification artifacts. They
  are unsigned, unattested by hosted OIDC, unpublished, and not release bytes.
- `cargo-about` is an automated evidence generator. Frozen locked inputs,
  complete selected-package coverage, accepted-policy agreement, source notice
  retention, and reviewable exact output do not constitute legal advice.
- The final report commit adds only this evidence record after the clean
  implementation commit named above. Fresh independent R6 must bind the exact
  integrated candidate before R7 or release can open.

Subject to those explicit limits, the bounded lane returns:

```text
READY FOR INDEPENDENT R6 REQUALIFICATION
```
