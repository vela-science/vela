# Independent final F08/G08 offline-build and strict-shadow review

## Verdict

**PASS**, bound to producer commit
`04717caf38ca2581aca5c9905baf14ed9c2a21e0`, tree
`33457d30d827363efa58c2b6c8765d4ce924824d`, parent
`75ced5288f33913a27d3dbfe691fd5070a572b01`, and live remote branch
`refs/heads/codex/inherited-correction-study` at that exact commit.

The two findings in blocked review
`0c09f220f27ff77c644e5d778c448b7ef34de8c9` are independently cleared.
The Debian trust input is complete and locally bound without mutable repository
metadata, and the participant shadow record now rejects a nonnumeric existing
last-change day before normalization.

This review made no provider call, reran no calibration, released no permit,
accessed no protected adjudication key or scorer, and performed no merge,
authority, Standing, Core, or Protocol action.

## Offline Debian input

The committed Debian `ca-certificates` package is the exact official
`20250419~deb12u1` architecture-all package: 161,988 bytes at
`sha256:62b08a77d985d4253894b1f69aebda5925034ca4e294add364167fad8cb64a44`.
An independent download from the recorded official Debian URL is byte-identical
to the committed package.

The recorded official source descriptor is 1,769 bytes at
`sha256:72339e810ef8237a4c346540b52baf49607172cc849c2680328a608ce0f6a34b`.
It identifies source and version `ca-certificates 20250419~deb12u1` and binds
the 277,244-byte source archive at
`sha256:b2a431cbab9a0ece921cffacbe238dc27a3e382ad4a1806dc8968c5eff30471d`.
Independent official downloads match both recorded byte counts and hashes.

The committed 18,940-byte Debian copyright file at
`sha256:e85e1bcad3a915dc7e6f41412bc5bdeba275cadd817896ea0451f2140a93967c`
is byte-identical to the copyright file extracted from the official package.
The provenance record is bound at
`sha256:864c2c7e2a1184d896cdda3654d2e9cffe09793ba1fcc0b7113db8b64484af26`
and records the Debian packaging and Mozilla data licenses.

The Dockerfile contains no `apt-get`, package installation, repository
metadata, signature bypass, or maintainer-script execution. Its trust stage
copies the local package, verifies the exact SHA-256 digest, and runs only
`dpkg-deb --extract` under `RUN --network=none`. Independent network-disabled
extraction reproduces the runtime trust bundle exactly at
`sha256:714d457d580922dbf1d0be8bd35ba236a842b50b0072ae791582a19adef772a5`.
Although the package contains ordinary Debian maintainer scripts, neither the
Dockerfile nor the independent extraction executes them.

## Fresh-build reproduction and runtime

Two newly created independent `docker-container` builders began with empty
BuildKit caches and built the exact checkout with `--no-cache`, no provenance,
no pull, Linux arm64, `SOURCE_DATE_EPOCH=1757289600`, and OCI timestamp
rewriting. Their complete OCI archives are byte-for-byte identical and match
the frozen identities:

- OCI archive: `sha256:87a1b1d80a27dbc92a0fd5dd69543c4c55386d3cfef77e7c76dab37d2c905183`;
- image manifest: `sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1`;
- image config: `sha256:0b41c9eb78b4afcd34b8e6c8c3bf85d81eda431fa4f7f99445c6d951eaa49348`.

Loading the independently built image and running the provider-schema preflight
with container networking disabled passes every registered/provider schema
check. Provider events and stderr are both empty files with the standard empty
SHA-256 digest. The loaded image contains the exact trust bundle above.

## Closed account normalization

The normalizer now requires the existing participant last-password-change
field to match a nonempty decimal string before replacement. Independent
adversaries for `notaday`, a duplicate participant record, and an extra field
all fail before normalization and leave their source bytes unchanged.

Both valid cross-day fixtures converge to
`participant:!:20339:0:99999:7:::`. All nonparticipant lines remain byte-exact,
so only the participant last-change field is normalized. An isolated Linux
check confirms the replacement preserves shadow ownership 0:42 and mode 0640.
The loaded image preserves participant UID/GID 10001, home
`/home/participant` owned 10001:10001 with mode 0755, and the unprivileged
runtime behavior.

## Roots, invariants, and held state

The held verifier and custody verifier independently recompute the complete
artifact and transitive held roots:

- runtime source: `sha256:163f0bab3459e95f59ef503a4105600c9ee096dd16745c3187982a104e731971`;
- runtime: `sha256:3f7a753141306771b05c582d1c0ff30489cdb8a35c556e21ac5fdabb9a431ba8`;
- artifact: `sha256:86a00770c182b9dc8ed2267633cbc4425b0b85268bb0d17f69100905ebecb8cc`;
- registration: `sha256:820b725d04cd3780e4bbdb6a89f3ee980a5bf993259c1f089984a3e7f7407f2b`;
- assignment: `sha256:cf69793d6ed3489b17690088e8f004d95b04859ec60d5aa5cf7e558cbb012b80`;
- configuration mapping: `sha256:347707e1e1662144ec9c17124e6606694d1ce6f511f976135aae031906da12b3`;
- shared participant configuration: `sha256:8a90184da5c3d8632c725e1870c2df432764475aae02d0c9b2ed30fa9b8617d2`;
- permit set: `sha256:6310a342b13f445e8dfa7821e1cca187dc5ec4c6ad24b12e1be2092a6f19b009`;
- prelaunch: `sha256:41f3495a49af99e44f7fec02605c856a96a304f291d2b86ea58a96dbc1ce6032`.

All assignment, runtime-configuration, mapping, permit, and prelaunch changes
are transitive consequences of the repaired held runtime and final provenance
binding. Participant packet, prompt, registered/provider schema, study design,
model, reasoning effort, gates, scientific facts, assignment seed, and
protected adjudication commitment objects are unchanged.

The stopped original execution tree remains
`0652cc6e51281e628d09c869bc2ccc62037db728`; the independently PASSed neutral
calibration evidence tree remains
`54ab3c669ac49ac9c8c9d10aa83dbb086174c0e2`; the original held artifact,
Rust crates, Protocol, and architecture objects also match the blocked producer
exactly.

Study state remains `not_run` at 0/36. Exactly 36 participant permits are held,
zero are consumed, and no participant run evidence exists. Participant provider
calls, protected-key accesses, and scoring runs remain zero. Authority effect
remains `none`.

## Focused checks

The held benchmark verifier, prelaunch custody verifier, and all 30 focused
benchmark/runtime tests pass independently under CPython 3.10, 3.11, 3.12,
3.13, and 3.14. JavaScript event-contract tests, Ruff, `git diff --check`,
offline Debian extraction, both fresh builds, complete OCI comparison, loaded
network-none provider-schema preflight, object-identity comparisons, and the
strict shadow adversaries all pass.

## Boundary

This PASS confirms only the final F08/G08 runtime repair at the exact producer
commit above. It does not release or authorize a participant permit, provider
call, calibration rerun, protected-key access, scoring action, merge, scientific
claim, Repository authority, Standing, Core, or Protocol change.
