# Release qualification

Vela has two related but distinct release surfaces:

- **Protocol 1** is the current release-candidate specification, schemas,
  conformance vectors, and independent readers.
- **Vela 0.977.2** is the current signed pre-1.0 implementation release. It
  retains the final planned pre-1.0 wire cut from 0.977.0: Submission v3 is the
  only current Submission shape, and the v2/execution-binding runtime is absent.
  It also retains the stable, deterministic, authority-neutral Repository
  projection and the static musl Linux bundle.

The latest signed published release is `v0.977.2`. User-facing installer
examples pin that exact tag rather than following a moving branch.

`v0.977.3` is the compatible exact restart-recovery inspection candidate. It
preserves the Protocol 1 object selection, exact roots, replay, and authority
semantics while adding one command-specific, read-only CLI projection of the
private recovery barrier. It becomes current only after both platform bundles
and manifests pass the gates below, the exact draft manifests are signed, and
the draft is published.

`v0.977.2` is the compatible historical-correction projection patch. It
preserves the Protocol 1 object selection, exact roots, replay, and authority
semantics while carrying each admitted correction transition's exact Claim
roots into the derived correction-impact view. Both platform bundles and
manifests passed the gates below; the exact manifests were signed before the
draft became public and immutable.

`v0.977.1` is the compatible agent-operator coherence patch. It preserves
the v3 Protocol selection, exact roots, replay, and authority semantics while
making performer-versus-authority output explicit, adding stable bounded
failure codes, and removing ambient Git identity from mechanical publication.
Both platform bundles and manifests passed the gates below; the exact manifests
were signed before the draft became public and immutable.

`vela integration check` validates the shared Manifest, Profile, Binding,
Method, and Exact Reference contract. `vela integration inspect` renders its
rooted inventory. Neither command executes a native Method, initializes
Repository authority, makes a Decision, creates an Event, or changes Standing.
Source-specific scientific, build, proof, and review semantics remain owned by
the source repository. This patch changes no Protocol 1 object or selection and
does not claim external adoption or scientific lift.

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
