# Foreign-transfer contract audit

Benchmark family B8 requires a second Frontier to retain and check exact
foreign state without importing the source Frontier's authority.

The frozen source audit in `plan.v1.json` and `result.v1.json` established that
the current protocol has no such transfer contract. The historical
`vela.foreign-reference.v1` experiment is retained only in this evidence
companion. It is not current runtime support, a protocol object, writer
command, resolver, Registry, or federation service.

`erdos-424/` is the single real qualification package. It binds:

- the current compacted Erdős repository and its predecessor transition;
- the repository origin connecting those states;
- the exact accepted Claim, Submission, Proposal, and Verification;
- the applied and Decision Events;
- the signed repository-authority record and exact keyset; and
- an explicit local Standing effect of `none`.

The Rust reader and independently implemented Python reader both rederive object
identities and verify the complete semantic and signature chain. Their shared
assessment is rooted at
`sha256:b7b330ae6ea4915d5bac218233f0a272ee961060682be6d22f6a8ea1b78c4ed6`.
The Rust reader was removed from the current runtime after this qualification;
the exact evidence and independent reader remain here for reproduction.
This qualifies the source package and the two implementations. It does not
complete B8 until a second Frontier retains and verifies the package through
its ordinary non-authoritative producer path.

Formal Conjectures retained the exact source package under corrected Proposal
`vpr_7aba66544ffefd99`. A credential-free import preflight then verified the
signed receiver Verification, its actor, Proposal, Submission, Claim,
Artifacts, current repository, authority history, trust anchor, Cedar
authorization, and canonical transaction sets. With `SSH_AUTH_SOCK`
deliberately absent, execution stopped when the already-authorized transaction
needed its signature and left Git, the worktree, and every operation journal
byte-identical. The rooted observation is
`erdos-424/receiver-preflight.v1.json`, byte root
`sha256:00b135a27088af1049ffe86cc329a5bec10fde098e32ac8342900c84a8a95c09`.
This proves readiness and fail-closed behavior, not receiver publication.

Receiver publication is recorded separately in
`erdos-424/receiver-publication.v1.json`, byte root
`sha256:a5867554d4dc9ea4dcd6d415a2be263c84dc0f6fbbe497fb86b427104368d75c`.
Formal Conjectures imported scoped Verification `vvr_ebc29eae4f5f4edf`,
pushed commit `3fe6bf62afd587b9cdeac39f5eb3c62a28fbc0aa`, and reproduced repository
root
`sha256:5e59e05a5639ac0ec4331ec40fec9f50229b795a1a08d983ba96834d4777b58a`
from a clean clone. That retained audit is the pre-Decision checkpoint. A
second scoped Verification `vvr_179fb049e70ff620` later satisfied the exact
registered requirement. Human Decision event `vev_798955d528dc3030` accepted
only the bounded retention Claim; applied event `vev_973ee78ab0fdfda4` and
strict replay produce current Formal repository root
`sha256:f652b5793e2bcccd2863f24adb7dda3ff3dd707ae64e2de8ee447b37fb1c85e7`.
This completes B8 without importing source authority. It does not establish
external independence, measured lift, or a supported shared adapter contract.

Rebuild and verify the source package:

```bash
uv run --project conformance --locked python \
  paper/artifacts/transfer/materialize_real_reference.py \
  --source ../erdos-frontier \
  --source-ref 81e79f008b4fc653888efda810dd8eb48e50cffa \
  --output /tmp/erdos-424
uv run --project conformance --locked python \
  paper/artifacts/transfer/verify_foreign_reference.py \
  --package-root /tmp/erdos-424
```

The earlier deterministic tar helper and Vela-specific archive verifier were
deleted after this qualification. Their exact archive digest remains in the
historical receiver evidence, but maintaining a second package format would
duplicate RO-Crate and standard archive tooling. Current transfer evidence is
the attached RO-Crate 1.3 view plus its closed `SHA256SUMS` fixity manifest;
ordinary ZIP, tar, OCI, repository, or deposit tooling may carry those exact
files without acquiring Vela semantics.

The landscape decision remains intentionally narrow: Vela is scientific state
and inheritance infrastructure. A reusable package, read-only index, Registry,
or Atlas must be earned in that order by recurring external use, net deletion,
correction-aware transfer, and measured cold-user lift.

## Scientific change package interoperability baseline

The same accepted Erdős 424 bytes now have two read-only representations in
the existing `erdos-424/` package:

1. `reference.v1.json` is the minimal rooted native manifest. It retains the
   exact Claim, Submission, Proposal, Verification, Decision and applied
   Events, source repository identity, authority evidence, source Standing,
   and local Standing effect `none`.
2. `ro-crate-metadata.json` is an attached RO-Crate 1.3 metadata view over that
   same native manifest, the same 11 object files, and four exact
   source-transition evidence payloads. It introduces no copied Vela object
   tree, protocol object, import command, or authority.

The present artifact is a **Decision-chain transfer package**, not a complete
Vela Submission RO-Crate profile. The retained Submission references source
artifact
`sha256:d18024c4333f77144955adf0036ce831e71b331ea7d9cc9cb69958f960f56d6c`,
and the crate now includes that exact source-diff payload, predecessor and
successor `424.lean` bytes, and the exact full-index patch. The reader binds the
Submission digest to those bytes, recomputes both Git blob identities, and
applies the patch to reproduce the successor. These files are evidence for the
retained Decision chain; RO-Crate conformance does not make them accepted
Standing.

The `erdos-424/` directory is an evidence companion, not itself a distributable
crate directory: later assessment, materialization, receiver-preflight, and
receiver-publication results coexist there but are deliberately outside the
RO-Crate `hasPart` set. A real distributor can copy only the paths named by the
closed fixity manifest into a standard archive or repository deposit. Vela does
not define another archive format or archive runtime.

`vela-loss-report.v1.json` states the six Vela semantic planes that base
RO-Crate does not preserve losslessly. `reader-result.v1.json` records the
existing independent native verifier and a separate standard-library
RO-Crate reader. `result.v1.json` records two byte-identical clean rebuilds and
the four required fail-closed mutations. `SHA256SUMS` is the archive fixity
manifest: it covers the metadata, native manifest, loss report, every retained
native object, and all four source-transition evidence files, but excludes
itself and evidence-companion result files.

The plan was frozen before any derived output:

```text
plan root
sha256:72d84fd4ceeb69c170beaf2e63dc22a801db6e99b749123c87a2f42ebbf07e42

plan amendment root
sha256:38d3cd699bcc4540a01852460ae218d91eb476ad40e7e2cac8886c02ff248ad8

source-evidence amendment root
sha256:ff22ac6c97ded34f0049fa6f75b3f6aea6e88212bfe3ef71672051f5bf26271f
```

RO-Crate 1.3.0 is the current Recommendation, published 2026-06-22. The
amendment records an external-tool limitation rather than hiding it:
`roc-validator 0.11.2` provides RO-Crate 1.1 and 1.2 profiles but no 1.3
profile, and a current 0.11.3 installation exposes the same base-profile set.
The experiment therefore records `unsupported_profile` and refuses to
substitute the older standard. The clean-room reader checks the mandatory
RO-Crate 1.3 package shape directly; this is not a claim of off-the-shelf 1.3
validator passage.

Rebuild the sidecars without an external dependency. `--roc-validator` is an
optional diagnostic that rechecks the frozen unsupported-profile observation;
it is not a build prerequisite:

```bash
uv run --project conformance --locked python \
  paper/artifacts/transfer/build_scientific_change_package.py \
  --source-package paper/artifacts/transfer/erdos-424 \
  --publish-to paper/artifacts/transfer/erdos-424
```

The builder refuses a different publication directory and refuses to
overwrite stale generated bytes. Verify without any external dependency:

```bash
uv run --project conformance --locked python \
  paper/artifacts/transfer/read_scientific_change_package.py \
  --package-root paper/artifacts/transfer/erdos-424 \
  --plan paper/artifacts/transfer/scientific-change-package-plan.v1.json \
  --plan-amendment \
    paper/artifacts/transfer/scientific-change-package-plan-amendment-001.v1.json \
  --evidence-amendment \
    paper/artifacts/transfer/scientific-change-package-plan-amendment-002.v1.json

uv run --project conformance --locked python -m unittest discover \
  -s paper/artifacts/transfer -p 'test_*.py'
```

This completes a first-party base-RO-Crate packaging baseline only. It does not establish
external adoption, measured continuation lift, RO-Crate validation of Vela
semantics, or source authority in another Frontier.

These missing product and adoption results are promotion gates, not protocol
requirements for a human Decision on the receiver's bounded Claim. If the
receiver authority decides before those results exist, the Decision must state
the limitation and must not be presented as independent adoption or measured
product lift.
