# Independent post-canary review: inherited correction runtime

## Verdict

**BLOCKED**, bound to producer result commit
`e5f11cc58b3675f171cab2fc57b5714b330152f3`, tree
`b6ccfe6ba47753d17e555bc41eb4e500aafe20a8`, and its prelaunch parent
`c7d9b96740d8d2aaa1f86728302aa47e5ccf370d`, tree
`8c69d357b315b4d7a7690d8255eab05dfd310e85`.

The runtime, CA, authentication, permit, and terminal-capture evidence is
internally consistent and independently reproducible. The commit nevertheless
cannot receive PASS under the frozen review protocol: gate
`G08_deterministic_custody` requires every documented deterministic check to
succeed from the exact clean checkout, and the claimed Ruff receipt does not.

This verdict does **not** authorize or freeze a replacement confirmatory
registration, any of the 16 sessions, another canary, paid inference, human or
scientific validation, a merge, or any Standing, authority, or Decision effect.
The confirmatory status remains 0/16, `not_registered` / `not_run`.

## F03 — deterministic check receipt does not reproduce

From a new detached checkout of the exact result commit:

```text
uv run --project conformance --locked --group dev ruff check \
  paper/artifacts/inherited-correction-benchmark-execution/freeze-canary-03.py \
  paper/artifacts/inherited-correction-benchmark-execution/test_container_runtime.py

EXE001 Shebang is present but file is not executable
--> .../freeze-canary-03.py:1:1
Found 1 error.
exit 1
```

Git confirms `freeze-canary-03.py` is mode `100644`. The matching locked format
check also exits 1 and reports both changed Python files would be reformatted:

```text
uv run --project conformance --locked --group dev ruff format --check \
  paper/artifacts/inherited-correction-benchmark-execution/freeze-canary-03.py \
  paper/artifacts/inherited-correction-benchmark-execution/test_container_runtime.py

2 files would be reformatted
exit 1
```

The standalone event command named in the handoff also needs its locked local
dependencies restored in a clean clone: bare `node .../test-events.mjs` fails
with `ERR_MODULE_NOT_FOUND: ajv`; after
`npm ci --omit=dev --ignore-scripts` in `container-runtime`, it passes. This is
a reproducibility-prerequisite omission, not a runtime-contract failure.

Minimal repair: commit Ruff-formatted versions of the two changed Python files,
make the freeze script executable (or remove its shebang), and state the locked
`npm ci` prerequisite before the standalone Node test. A new immutable commit
requires a narrow re-review; no canary rerun is implied or authorized.

## Evidence that otherwise passed

### Immutable lineage and scope

- `origin/codex/inherited-correction-study` resolved exactly to
  `e5f11cc58b3675f171cab2fc57b5714b330152f3` and the handed-off tree.
- The result commit's sole parent is the handed-off prelaunch commit; that
  commit's sole parent is result base
  `70be21e2404af68daf5673f8094c47563224a11e`.
- The prelaunch commit time is `2026-08-21T17:31:51Z`; permit consumption is
  `2026-08-21T17:32:36.575Z`; provider start is one millisecond later; provider
  completion precedes the result commit time `2026-08-21T17:34:09Z`.
- The two diffs touch only the handed-off paper-artifact paths. No Core,
  Protocol, Standing, authority, or Decision byte changed.
- Canary-01 and canary-02 tree identities are unchanged from the result base.

### CA provenance and exact image

- The pinned image exists locally as
  `sha256:6274d83356076640d6e4bc810b97d37ac2d1b5ab02546dd7c2ebed16f915b547`
  for Linux arm64, based on the exact handed-off base digest.
- Independent download of the provenance URL produced Debian package SHA-256
  `62b08a77d985d4253894b1f69aebda5925034ca4e294add364167fad8cb64a44`.
- Independent `dpkg-deb` extraction and ordered concatenation of only
  `usr/share/ca-certificates/mozilla/*.crt` produced 150 certificates and
  bundle SHA-256
  `714d457d580922dbf1d0be8bd35ba236a842b50b0072ae791582a19adef772a5`.
- The exact pinned image exposes that same bundle. `SSL_CERT_FILE` points to it;
  no TLS-disable or insecure override is present. Positive, missing, and
  corrupt trust preflights reran with `--network=none`, matched their expected
  pass/fail polarity, and emitted zero provider bytes.
- The prior canary-02 UnknownIssuer evidence and its referenced hashes match;
  canary-03's successful four-event exchange is consistent with the bounded
  trust-store diagnosis.

### Authentication and one-shot custody

- The image contains no `auth.json` or credential environment value. The
  launcher mounts the host OAuth file read-only into an ephemeral tmpfs-backed
  `CODEX_HOME`; input is read-only, work is read-only, and the named canary
  container no longer exists.
- Captured event, stderr, and response bytes contain no credential-shaped
  material. Provider stderr is empty and `credential_retained` is false.
- The permit is validated, then atomically renamed to its consumed path before
  the provider process is spawned. The capture contains only the consumed
  permit; a replay through the launcher fails before provider execution.
- Default-hold and binding-drift adversaries fail before consumption. The
  source contains one exact run argument and no assignment scheduler.

### Receipts and roots

Independent recomputation matched every prelaunch manifest entry and all
documented roots, including:

- prelaunch bytes/root:
  `sha256:1405e33a4c5627c510d85174f602b6951c7b2c17e3903a28708867299682e1c4` /
  `sha256:3a749cd027bb80569785f22501f0e397750e9396e301027d9a7b53d242dac759`;
- registration, configuration, assignment, authorization, and permit roots:
  `sha256:87c4a6e3e230fa719417a349a3ed0f1c1843ec87daa80f9e32927bd3ed03aed0`,
  `sha256:54251d2cc114b13db1c981ab08e8718023335f694c0fc02ca6eb92a9cf8291da`,
  `sha256:dfd634e93fc75d3a4cc1f1173a040961f36f0861c4c144fc50371cad2a306bb4`,
  `sha256:dc730ba60c820323c51781f1a95b6e9f50f40d0b0074ffb6415991a4c4b91991`,
  and `sha256:f4fb639bc50194e3c6690bc6518815316b7d0cca567615b50be1092b7077da05`;
- packet, prompt, expected response, offline preflight, provenance, and diagnosis
  roots exactly as handed off;
- result bytes/root:
  `sha256:ea6c2a8c3da7aa3b09f32f7eb23414316f6ee1df6158e0eba44d4ac9c54bc1d7` /
  `sha256:53d7b376ca90b0dc33db2c53703a63e0700068159b991c8250ce8f1f47fba018`;
  and
- capture-manifest root
  `sha256:063f596d77040db4cd7c075f382e6b598525ab199a9fd13612c09a1f358d6327`.

The prelaunch generator independently recreated all 36 prelaunch files
byte-for-byte. The terminal receipt contains exactly one thread, one turn, one
agent response, and one completed turn; no tool, continuation, compaction,
retry, or stderr event appears. The response equals the frozen expected root,
and the usage and 4.58358096-second monotonic duration match the capture.

The 11 runtime tests, standalone event contract test after locked dependency
restore, benchmark verification, 15 benchmark tests, and `git diff --check`
passed twice. No provider, paid-model, human-study, merge, authority, Standing,
or Decision action was performed during review.
