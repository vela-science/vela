# Stage A held two-provider runtime candidate

This package prospectively binds the independently passed Stage A `0/12`
package to the maintained evidence qualifier merged on Vela main at
`cc3b88d8bfcfd7b4f720a023f049d5c365be9423`, tree
`341e0d22fa570b1b5e8dd9f70b219c11308ba45f`.  The exact qualifier executable
has SHA-256
`61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013`.

The exact OpenAI Responses and Anthropic Messages candidate configurations now
have distinct deterministic, launchable Linux ARM64 OCI identities. Each
provider runner is built twice from the committed no-dependency Go source with
fresh independent empty caches; binary, OCI archive, image, config, and layer
identities must match. The retained rootfs layers pass a network-none,
read-only, capability-free launch self-test. The participant process has no
network client: it communicates over inherited descriptor 3 with a host bridge
restricted to the single frozen provider endpoint. Credential material, if
separately authorized later, enters the host bridge on a distinct inherited
descriptor and is never retained by the participant image.

Both bundles use the maintained exact four-rule provider-schema registry for
the frozen Stage A response schema. Both pass the maintained qualifier fully
offline with the same read-only `git status` and regular-file information
boundary, raw event/tool/result custody, canonical read-only mounts, retained
CA trust, and distinct held neutral-calibration permits. The qualifier consumes
only its synthetic no-science self-test fixture; neither campaign neutral
permit is consumed and no provider is contacted.

The OpenAI Responses bridge requires every `function_call.arguments` wire
value to be exactly one JSON string. It decodes that string exactly once,
rejects malformed, non-object, double-encoded, or non-closed tool arguments,
and binds the retained raw field to the exact decoded object bytes with a
dedicated custody receipt and digest. Anthropic `tool_use.input` remains the
unchanged object-shaped Messages contract.

The candidate remains blocked only because both Platform API-key classes are
absent. Only their environment names and absence were checked; no credential
value or consumer OAuth surface was requested, used, or inspected. Independent
exact review and a separate execution authorization remain prerequisites even
after appropriate Platform credentials become available.

Consequently all twelve participant permits remain held and non-releasable,
both neutral-calibration permits remain held, and the ledgers remain at zero
participant calls, provider calls, calibrations, responses, scoring, protected
keys, and Stage B selection. The package has no Protocol, Core, authority,
Decision, or Standing effect.

Run the committed stopped-state verifier and regressions with:

```bash
uv run --project conformance --locked python \
  paper/artifacts/lean-correspondence-stage-a-runtime-qualification/verify.py

uv run --project conformance --locked python -m unittest \
  paper.artifacts.lean-correspondence-stage-a-runtime-qualification.test_verify -v
```

`offline_qualify.py` regenerates the two bundles at one caller-supplied,
explicitly scoped canonical workspace, builds each runner twice with fresh
empty caches, launches only the offline self-tests, invokes the exact qualifier
in a fixed locked environment, and retains the immutable images, runners,
source/build/launch receipts, provider contracts and schemas, tool boundaries,
held permits, hold states, and canonical qualification record. It requires an
explicit trust-bundle path and never reads credential values or opens provider
network access.
