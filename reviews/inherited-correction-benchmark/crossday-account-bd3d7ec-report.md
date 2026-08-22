# Independent F08/G08 cross-day account repair review

## Verdict

**BLOCKED**, bound to producer commit
`bd3d7ec837490b67801848b3180e3d244290c9ad`, tree
`277aba87a753bee3864096f8297e65cec770b8cd`, parent
`51131fd07d44b52c0bd550110d39d06891adaa54`, and live remote branch
`refs/heads/codex/inherited-correction-study` at that exact commit.

Two required gates fail. First, two newly created isolated `docker-container`
builders with empty caches both fail the exact pinned build before any repaired
layer is produced: the pinned base's `apt-get update` rejects all three Debian
InRelease files with `At least one invalid signature was encountered`. The
complete OCI archive, manifest, and config identities therefore cannot be
independently reproduced from the exact source in fresh builders.

Second, the account normalizer does not validate the complete closed
participant shadow record. A participant last-password-change field changed
from the observed decimal day to `notaday` is accepted and rewritten to the
fixed day. That malformed account shape must fail closed.

This review made no provider call, reran no calibration, released no permit,
accessed no protected adjudication key or scorer, and performed no merge,
authority, Standing, Core, or Protocol action.

## Exact prior failure

Independent inspection of the retained reviewed and fresh image exports
confirms 13 layers in each image. The first 12 layer identities are identical.
The last layers have identical path sets, and recursive comparison finds only
`/etc/shadow` different. Those shadow files have identical length and mode and
differ at exactly byte 519: ASCII `6` versus ASCII `7`. The complete entries
are respectively:

```text
participant:!:20686:0:99999:7:::
participant:!:20687:0:99999:7:::
```

The committed cross-day fixtures independently encode the same one-byte day
difference and bind the recorded fixture roots.

## Repair behavior that passes

The normalizer sets the participant last-change field to
`SOURCE_DATE_EPOCH / 86400`, yielding
`participant:!:20339:0:99999:7:::`. Both cross-day fixtures converge to the
same fixed bytes. Duplicate participant records, an extra shadow field, and a
changed password marker fail closed without modifying the source file.

An isolated Linux container check confirms normalization preserves ownership,
group, and mode. The repaired retained image has participant UID/GID 10001,
home `/home/participant` owned 10001:10001 with mode 0755, and `/etc/shadow`
owned 0:42 with mode 0640. The participant runtime remains unprivileged.

The two producer-retained repaired OCI archives are byte-identical at
`sha256:deb413a3e695d6e3591a0429afd2883573a0d348ea5ef800755ef6a6cddd5f2d`.
They bind manifest
`sha256:71bceb9885958619b129d7567b56277422f4c1d17c85a7076fb0d60c07633dea`
and config
`sha256:a3af3c330d18683d7a6e9f183a0aac1b1fb579faec26b9cd091e58730dfb975e`.
Loading that image and running the provider-schema preflight with network
disabled passes all registered/provider schema checks and leaves both provider
events and stderr empty. This retained-output check does not substitute for
the failed fresh-builder gate.

## Invariants and held state

The held benchmark verifier and prelaunch custody verifier recompute the new
registration and all transitive roots. Study state remains `not_run` at 0/36,
with exactly 36 participant permits held, zero consumed, and no participant
run evidence. Participant provider calls, protected-key accesses, and scoring
runs remain zero.

All ten assignment files differ only in the image and registration roots. All
ten runtime configuration files differ only in those same transitive roots.
All 36 participant permits and the one neutral calibration permit differ only
in their assignment, image, configuration, and registration roots. Participant
packet, prompt, and registered/provider schema Git objects are unchanged.
Preregistered design, families, purpose, scoring gates, claim ceiling, model,
reasoning effort, retry/tool limits, and scientific facts are unchanged.

The stopped original evidence tree remains
`0652cc6e51281e628d09c869bc2ccc62037db728`; the neutral calibration evidence
tree remains `54ab3c669ac49ac9c8c9d10aa83dbb086174c0e2`; the original held artifact,
Rust crates, Protocol, and architecture objects also match the parent exactly.

## Focused checks

The held benchmark verifier, prelaunch custody verifier, 21 benchmark tests,
seven provider-runtime tests, JavaScript event-contract tests, Ruff, offline
network-none provider-schema preflight, root comparisons, and `git diff
--check` pass. Account adversaries pass except for the nonnumeric day mutation
described above. Both fresh exact builds fail at the same signed Debian-index
step.

## Required repair

Validate the existing participant last-change field as a decimal value before
rewriting it, and make the exact pinned build contract complete successfully in
two fresh empty-cache builders without weakening repository signature checks.
Then regenerate the transitive roots and obtain a new commit-bound independent
review. No participant permit is authorized by this verdict.
