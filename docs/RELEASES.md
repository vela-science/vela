# Release qualification

Vela has two related but distinct release surfaces:

- **Protocol 1** is the current release-candidate specification, schemas,
  conformance vectors, and independent readers.
- **Vela 0.973.0** is the pre-1.0 implementation identity prepared by this
  tree. The version and release notes do not themselves create a tag, signed
  bundle, or publication. A future `v1.0.0` publication requires a separate,
  explicit release authorization.

The latest signed published release remains `v0.972.1`. User-facing installer
examples stay pinned to that version until a later release is actually signed
and published; they do not follow the workspace candidate version.

Neither a protocol conformance result nor a signed software bundle is a
scientific Decision. Neither changes Standing or demonstrates external
adoption.

## Release-candidate gates

A candidate is locally qualified only when all of these gates pass:

1. The version, release tag, source commit, and source tree agree. Signing
   refuses a modified or untracked tree and requires the tag to name `HEAD`.
2. The Protocol 1 digest manifest, schemas, conformance vectors, independent
   Python and JavaScript implementations, and three reference flows pass the
   full core conformance union.
3. `scripts/release.sh` builds the binary twice in separate target directories
   with fixed path remapping and `SOURCE_DATE_EPOCH`; the two binaries must be
   byte-identical.
4. The staged tree is archived twice with sorted entries, fixed ownership,
   permissions, paths, and timestamps; the two archives must be byte-identical.
5. The entry point emits the SPDX SBOM, checksums, bundle manifest, and smoke
   test. The manifest records the source commit and the two comparisons.
6. GitHub Actions adds per-asset OIDC build provenance, runs the same bundle
   smoke test on both supported targets, and creates an unpublished draft.
7. An operator signs each exact published manifest with the distribution
   identity. Before publication, `scripts/sign-published-release.sh` checks the
   tag commit and downloads and verifies every archive and SBOM named by every
   manifest. Any mismatch leaves the release a draft.

The supported release targets remain Linux x86-64 and macOS Apple silicon.
Reproducibility here means the entry point compared two local builds for one
declared source and target. The OIDC attestation remains provider-bound, and
independent rebuild agreement remains useful external evidence rather than a
claim made by this repository.

## Operator commands

For a release candidate, run the full local union:

```bash
uv run --project conformance --locked ./conformance/check-core.sh
cargo clippy --locked --workspace --all-targets -- -D warnings
scripts/release.sh
```

`conformance/check-current-object-waist.sh` is intentionally CI-only because it
writes a trust pin under the real operating-system account home. The hosted
conformance workflow runs that additional gate on a disposable runner.

Tagging, uploading, signing, and publishing are separate operations. Do not run
them merely because the local candidate is green.
