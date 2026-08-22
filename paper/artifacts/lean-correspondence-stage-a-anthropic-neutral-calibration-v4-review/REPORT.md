# Independent review: Stage A Anthropic neutral calibration v4

Verdict: **PASS** for the exact Anthropic neutral runtime calibration evidence.

## Immutable subject

- Producer commit: `50ebc2280a1154243130c0520e633990b4d5feab`
- Producer tree: `4a6ff57fb3dea3af0ca8fa2e9c6ff0e89adda7d5`
- Parent lossless-runtime producer: `0aee2129f2f7824f328d9576f72e42f240e08932`
- Evidence artifact root: `sha256:58f2995e8993045d9f9653371c1b49a770e23c59ca85d79cc26ff536a1537a0a`

The producer branch, remote-tracking ref, and remote head were exact. A fresh
`--no-local` detached clone reproduced the commit, tree, sole parent, strict Git
object check, closed artifact inventory, manifest, and clean state.

## Findings

The v4 permit is distinct and was consumed exactly once. The retained evidence
contains one endpoint-attempt/provider call, one attempt, zero retries, and no
reuse. The OpenAI neutral permit and all twelve participant permits remain held.
The permanent v2 non-call and v3 failed-exact-request evidence directories are
byte-identical to their immutable source commits and independently verify in
those exact commit contexts.

The request custody is lossless. The committed pre-frame request, expected
request, strict canonical RFC 4648 base64 decode, retained network body, initial
bridge custody, and endpoint-write custody are the same 4,278 bytes at
`sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c`.
The exact 2,384-byte provider schema at
`sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7`
occurs once. The lossless provider-request frame is bound at
`sha256:6ba0a6cd840dc28d8dbba2f9b019d0e37cee109924593a328c173f38000cf074`.

The provider-event transcript, raw provider response, parsed text response,
usage, runner terminal, endpoint receipt, cross-layer call counts, process
teardown, and credential non-retention receipts close exactly. The raw provider
response is
`sha256:4b90af7d3453c95f8b59a49d6bf7761593cc2ef9ccbfee96fbff7abc8934c50e`;
the exact parsed response is
`sha256:95bf0c205c10167f57d769a0f77daef57ede2db3a6061464957c496e50eddc46`.
The response validates under both the provider-facing and registered Draft
2020-12 schemas. Both child processes exited zero and teardown closed all
credential and bridge descriptors. A scan of committed non-source evidence
found no credential-shaped bytes without reading the credential source.

The executed Darwin/arm64 bridge was independently rebuilt twice from an
isolated exact source copy with fresh caches, CGO disabled, trimpath, stripped
debug data, empty build ID, and
`-X main.providerAdapter=anthropic-messages-v1`. Both builds were byte-identical
to the recorded 5,918,210-byte binary at
`sha256:c6a8cb2f256eea8654a82333fd59c9b8aa0c086c0fec3629c56bd4bef2794eb4`.
Building inside a Git checkout embeds VCS metadata and is therefore outside the
recorded isolated-source build boundary.

The artifact verifier and all 34 committed adversaries passed. Proportionate
runtime, Stage A Lean, frozen method, v2/v3, Draft 2020-12 schema, Ruff,
conformance, JSON, reseal, and diff checks also passed.

## Decision and claim ceiling

This review qualifies the exact Anthropic neutral calibration evidence and the
exact Anthropic lossless runtime used for it. It permits the coordinator to
record this neutral calibration as independently PASSed.

It does **not** qualify OpenAI, release any participant permit, authorize the
participant study, score a result, select Stage B families, establish scientific
lift, mutate Protocol/Core, or create Repository authority, Decision, or
Standing. The producer's `positive_qualification: false` was correct while this
review was pending; any later status transition must bind this exact review and
must not rewrite the producer evidence.

No credential was accessed, no provider was called, no permit was released, and
no producer, scientific, or authority state was changed by this review.
