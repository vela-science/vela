# Independent schema-compatible held-out replacement prelaunch review

## Verdict

**BLOCKED**, bound to producer commit
`3f4d61c248a43a94d09e0d848693101dd1841aa0`, tree
`3553ff7270b479e93a99515aec7cfc54cf30f15f`, parent
`d3bff9206609c53a0dc9b2ef7f85bbdc894a9904`.

Finding **F08**: a clean rebuild does not reproduce the registered OCI image
digest. The frozen runtime and every launch permit bind
`sha256:bc37c2759cb75acc54d998c22c17b542097019e892262be9b30a8d6c68396efe`,
but this review's uncached build from the exact producer source produced
`sha256:c50b70faf41008ed3519a5d87baded3bbbd806462ef5514a47b2c7ad171317a1`.
Because the image digest is an exact custody field, the registered runtime
identity is not independently reconstructible from the frozen source.

This BLOCKED verdict does not release the neutral calibration permit or any of
the 36 participant permits, and it authorizes no provider call, scoring,
protected-adjudication access, merge, or Core, Protocol, authority, Standing,
or Decision action.

## Reproduction

From a fresh detached checkout, the two ordinary README builds returned the
frozen digest only because the producer-host BuildKit layers were already
cached. The clean reconstruction was:

```text
docker build --no-cache --provenance=false --pull=false \
  -t vela-schemafix-review-nocache \
  paper/artifacts/inherited-correction-benchmark-execution/container-runtime-provider-schema-v2
docker image inspect vela-schemafix-review-nocache --format '{{.Id}}'
```

Observed: `sha256:c50b70faf41008ed3519a5d87baded3bbbd806462ef5514a47b2c7ad171317a1`.
Expected: `sha256:bc37c2759cb75acc54d998c22c17b542097019e892262be9b30a8d6c68396efe`.

The pinned base layers match, but all eight custom rootfs layers differ,
beginning with the trust-bundle copy and continuing through the global Codex
installation, locked local dependencies, runtime source copies, fixtures, and
participant-user setup. Cache-backed repeat builds therefore do not establish
clean image determinism.

## Otherwise passing evidence

All other requested boundaries passed independently:

- the remote ref was re-fetched and remained exact at the commit/tree above;
- the 258-path producer diff is additive only, so the stopped registration and
  run-01 evidence are byte-identical to the parent;
- the stopped record remains exactly 1/36, run 01 is a retained non-result,
  runs 02-36 are forbidden/unissued, and its two public files retain SHA-256
  `f6814ac5...` and `65e98a1f...`;
- the registered Draft 2020-12 schema remains byte-identical to the stopped
  study at
  `sha256:ac96be686e749792956dfa1dfe9560f85c53d55c27fe2e8fd32bcc2a96a634ba`;
- the provider schema at
  `sha256:896f242086805d3b51e81ed04e6d50f33eb2b7deb71b7a1689e9abeba3b67eaf`
  differs only by deleting
  `/properties/evidence_bindings/uniqueItems`;
- network-none preflight showed that the derivative accepts the neutral valid
  response and a duplicate-binding response, while locked Ajv against the
  registered schema rejects the duplicate; an independent benchmark call also
  rejected it as `response_consequence_bindings_incomplete`;
- independent terminal-receipt mutations of each schema digest failed as
  `receipt_drift:registered_response_schema_bytes` and
  `receipt_drift:provider_response_schema_bytes`;
- every participant packet, prompt, registered response-schema input, family
  source, task, and input-equivalence byte matches the stopped study;
- model `gpt-5.6-sol`, high reasoning, default service tier, 600-second timeout,
  8,192-token ceiling, one turn, no tools, attempt one, zero retries, and the
  preregistered gates are unchanged;
- the fresh schedule has 36 unique, non-reused run/participant IDs, balanced
  4 per family/arm and 12 per arm; its seed commitment recomputes exactly;
- all 36 participant permits are held/unconsumed, and the distinct neutral
  calibration permit is held/unconsumed with no denominator credit;
- registration, assignment, prelaunch, and artifact roots independently
  recompute to `sha256:7b6a8675...`, `sha256:f436ab1b...`,
  `sha256:1c069f0e...`, and `sha256:b992c8b7...`;
- isolated CPython 3.14 regeneration was Git-byte-clean, and verify, prelaunch,
  and all 21 tests passed under CPython 3.10, 3.11, 3.13, and 3.14;
- all 4 provider-runtime tests, the prior 16-test benchmark suite, Ruff 0.12.11,
  event-contract tests, and `git diff --check` passed; and
- replacement status remains 0/36, `not_run`, with zero permit consumption,
  provider calls, protected-key access, and scoring.

The registered roots independently recomputed as:

- registration: `sha256:7b6a8675c81431d19c690d8a16efacb0502116d3dc912514027d3d7096933d09`;
- assignment: `sha256:f436ab1b9f621fe83a7c0f10f0bee9b3cd9910c1ca563ad99f97064ab7be37a8`;
- runtime: `sha256:fa0e74191ce9ef7d4b82cadc77f57437c3b1ab62be4acd29e93d967aee12c2e0`;
- participant configuration: `sha256:37f7a1c0a374ed1fa076d1e9d16819494e72ff7975d47da8791ef7c9a75e2ffb`;
- permit set: `sha256:df4763f7ad7a944ad5a4bbaa35e055596fe583cb6a26d578a146a85a3ba44f19`;
- prelaunch: `sha256:1c069f0ef56308dd9a540451a4194410c4f15cc16704de520f0147a4118da513`;
- artifact: `sha256:b992c8b7f47bf0debbb3a3f5a9cffdd4f89023713145d5a135a16cceaaeef586`.

## Minimal repair

Produce a runtime image whose digest is reproducible from a clean build (or
replace source-rebuild determinism with an independently retrievable,
digest-verified immutable OCI artifact if the registration explicitly adopts
that different qualification boundary). Then bind the corrected image and
runtime roots, regenerate the fresh registration, configuration, permits,
prelaunch freeze, and manifest while preserving 0/36 and both holds, and submit
those immutable bytes for another independent review.
