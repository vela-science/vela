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
CA trust, and distinct held neutral-calibration permits. The shared neutral
content is one committed canonical JSON object plus one committed prompt; an
equivalence receipt binds the same semantic atoms and expected response
contract for both providers. The runner loads the packet only from the exact
read-only `/input/packet.json` mount using no-follow, single-link, pre/post-open
identity and exact byte-root checks. Its packet-only JSON decoder rejects
duplicate keys at every object depth, retains exact number lexemes, recursively
canonicalizes objects, arrays, and primitives, and requires byte equality with
the committed top-level object before binding the exact provider request bytes
in custody. Inline reconstruction is rejected.

The provider schema is materialized into each committed `run.json` by splicing
the exact mounted file bytes as a `json.RawMessage`, without parsing or
re-serialization. A retained receipt binds no-follow, single-link, pre/post
inode checks, source bytes/root, inserted byte range/root, run root, and exact
structured-output request schema bytes. Each exact run input passes the same
participant validation and request-construction path in a network-none
container with a fixed non-secret dummy credential descriptor. The prospective
controller derives provider calls only from sequential endpoint-attempt
receipts and requires exact agreement across bridge, runner, terminal, and
custody. The qualifier still consumes only its synthetic no-science fixture.

The OpenAI Responses bridge requires every `function_call.arguments` wire
value to be exactly one JSON string. It decodes that string exactly once,
rejects malformed, non-object, double-encoded, or non-closed tool arguments,
and binds the retained raw field to the exact decoded object bytes with a
dedicated custody receipt and digest. Anthropic `tool_use.input` remains the
unchanged object-shaped Messages contract.

This correction did not inspect credential presence or values. Independent
exact review is the current blocker; a separate execution authorization would
still be required afterward.

The two predecessor neutral permits whose packet-root preimages were plaintext
rather than runner-loadable JSON are retained as unconsumed,
retired-non-releasable records. Their two canonical-JSON successors are fresh,
distinct, held, and independently review-gated. The earlier Anthropic v2
canonical permit is separately and permanently retained as a consumed non-call
in the independently passed stopped-evidence artifact, whose authoritative
endpoint-contact receipt is zero. Its new v3 replacement is distinct,
offline-validated, held, and non-releasable. Consequently all twelve
participant permits and the OpenAI neutral permit remain held, and the ledgers remain at zero
participant calls, provider calls, calibrations, responses, scoring, protected
keys, and Stage B selection. The package has no Protocol, Core, authority,
Decision, or Standing effect.

The corrective ancestry is explicit: the reviewed predecessor `b333186c` has
direct parent `5be82cb3`; the invalid-permit origin `9da1c794` is an ancestor,
not that direct parent. The retirement records bind all three roles separately.

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
