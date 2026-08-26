# VELA-RC-1 final release decision

Status: `RELEASE READY WITH EXPLICIT LIMITATIONS`.

This file is completed only after the mandatory gates. Its final verdict must
be exactly one of:

- `RELEASE READY`;
- `RELEASE READY WITH EXPLICIT LIMITATIONS`;
- `HOLD — FIXABLE RELEASE BLOCKERS`; or
- `DO NOT RELEASE CURRENT CANDIDATE`.

A ready verdict returns the candidate to the user as ready for authorization.
It does not tag, publish, push, sign, deploy, contact anyone, or release.

Foundational top-down search remains `CLOSED` regardless of the release
decision.

## Decision

```text
READY FOR USER AUTHORIZATION WITH LIMITATIONS
```

Vela Protocol 1 has passed the mandatory internal release-qualification gates:

- semantic integrity and hostile authority-boundary checks;
- independent clean installation and deterministic governed-state replay;
- release-facing documentation and two domain-independent examples;
- product-source semantic legibility;
- deterministic macOS and Linux packaging, dependency notices, SBOM identity,
  installer behavior, and hostile omission/corruption checks; and
- an uncoached blind external-user simulation.

The qualified public claim is narrow: Vela is a protocol and toolchain for
governed, replayable scientific-state transitions. It records what was
proposed, what verified it, what Decision was authorized, what changed, and
how current Standing can be reconstructed.

## Explicit limitations

- cumulative workflow advantage remains unestablished;
- no autonomous discovery, cumulative-intelligence, or foundational mechanism
  claim is made;
- replay reconstructs governed state and does not rerun physical experiments,
  stochastic models, or native verifier computation;
- the blind user used a locally cached source build, while cold clean-install
  evidence comes from R2 and R6;
- the deployed Problems projection is still legacy and unqualified for
  current strict-integrity claims until separately authorized release-record,
  reprojection, live-reader, build/runtime, and deployment checks pass; and
- no external human adoption or production workflow has yet been observed.

## Version and release-time actions

The current qualified source still reports `0.977.4`, but that public version
already identifies an immutable ancestor. The recorded version decision is:

```text
PATCH BUMP TO 0.977.5 AT AUTHORIZED RELEASE PREPARATION
```

No bump is performed by RC-1. After explicit user authorization, the release
lane must prepare one bounded `0.977.5` commit, rerun hosted conformance, both
supported artifact builds, clean installation, exact manifests/checksums,
provenance and supported signatures on that exact tree, and then return for
final publication authorization. The separately repaired Vela Web projection
must remain inactive until its own authorized reprojection and deployment
checks pass.

No tag, version change, signature, push, publication, deployment, activation,
or release has occurred.
