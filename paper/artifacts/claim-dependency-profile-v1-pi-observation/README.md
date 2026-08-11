# Claim-dependency matched observation v1: pinned Pi SDK

This packet is a fresh preregistration for the same synthetic, counterfactual
claim-dependency task frozen by ADR 0043. It replaces only the invalid executor
layer. It does not amend, rerun, or interpret the v0 Harbor/Codex sessions.
Those four outputs remain externally retained under their exclusion custody
packet; their shape was seen during custody review, but no bytes, answers, or
metrics enter this packet or any v1 participant message.

The scientific waist is unchanged. The two input manifests, answer schema,
held-out answer key, scorer, shared scope, and task prompt are byte-identical to
the packet committed at `530cb806ad9d219341cf3e5ec168e9683136a427`.
The baseline contains seven exact scientific files. Treatment contains those
same seven plus exact `profile.json`. Both receive the same answer schema in
the one rendered user message. Evidence paths remain virtual `/input`-relative
paths so the unchanged held-out scorer can resolve them against its separate
exact input copy.

## Executor boundary

The stock Pi CLI/RPC path is a NO-GO: that entrypoint owns ambient resource and
configuration discovery, session creation, and RPC framing, while this study
must itself construct the exact disabled resource loader, in-memory session,
and single prompt call. No compatibility shim or historical executor is
retained. The packet instead pins
`@earendil-works/pi-coding-agent@0.84.1` and constructs one SDK session with:

- a `DefaultResourceLoader` explicitly disabling extensions, skills, prompt
  templates, themes, and context files, with no extension factories;
- in-memory settings and session managers;
- no tools, SSE, high reasoning, automatic reasoning summary, no retry, and no
  compaction;
- one custom provider `instructions` field (not a system message), equal to
  `prompts/system.txt` plus Pi's deterministic
  `\nCurrent working directory: /workspace` suffix;
- exactly one user message and one `session.prompt(...,
  {expandPromptTemplates:false})` call; and
- raw last-assistant text on stdout, with deterministic nonsecret custody JSONL
  on stderr.

The model has no filesystem or tool path. The harness reads two files: a
mode-0444 compact request and a derived mode-0400 OAuth credential. The derived
credential copies only the current access JWT and its exact account/expiry
bindings. Its refresh field is the fixed public
`vela-nonrefreshable-sentinel-v1`; the real Codex refresh token is never copied.
A six-hour minimum validity makes refresh unnecessary, and the exact-one-entry
credential store throws on `modify` or `delete` before Pi can refresh.

## Exact egress boundary

The participant container runs `--network none`. Its only transport is a
private host bind-mounted Unix socket. A separate packet-local broker container
has network access, accepts exactly one socket request, and validates the exact
URL, method, header set, derived bearer/account identity, zstd-decoded request
shape, effective instructions, one user message, no tools, model, reasoning,
and session key against the frozen request. It then makes one literal HTTPS
request to `https://chatgpt.com/backend-api/codex/responses`, does not follow a
redirect, streams the response, closes, and refuses any second request.

The ephemeral bearer necessarily passes through the broker in memory. It is
exact-compared with the derived credential and forwarded, but never logged,
rooted, or persisted. Broker custody records contain only fixed target text,
header names, counts, status, byte counts, and content roots.

`run-participant.sh` runs both containers as the exact nonroot host UID:GID,
with read-only roots, dropped capabilities, and `no-new-privileges`; derives
the credential only in a private external temporary directory; mounts request
and auth read-only; retains stdout and closed audit separately; and must prove
the credential, socket, and temporary directories are gone before success.
No authority-agent socket, Vela authority credential, source repository, Git
history, prior output, answer key, or scorer enters the participant or broker.

## Pinned supply chain

The Node base is concrete `node:24.12.0-bookworm-slim` for `linux/amd64`, pinned
to platform manifest
`sha256:6d8047885b91084ceff824c02950be237dafcbfd3d1b6e69d49c919868e806be`.
`package-lock.json` pins all 144 registry rows with integrity, including the six
nested Pi packages whose published shrinkwrap omitted SRI. The exact tagged
MIT license is retained as base64 in the packet because the 1,069-byte upstream
file has no terminal newline; materialization and the image decode it and
verify root
`sha256:0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48`.

## No-model gates

Materialize a development copy outside every Git directory:

```bash
uv run --project conformance --locked python \
  paper/artifacts/claim-dependency-profile-v1-pi-observation/materialize.py \
  --development-worktree \
  --output /absolute/external/claim-dependency-profile-v1-pi-observation
```

Run static, adversarial, double-materialization, unchanged-scorer, and auth
custody tests:

```bash
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python \
  paper/artifacts/claim-dependency-profile-v1-pi-observation/test_observation.py
```

Build the exact linux/amd64 image without invoking a model:

```bash
docker build --platform linux/amd64 \
  -t vela-claim-dependency-pi-v1:capture \
  paper/artifacts/claim-dependency-profile-v1-pi-observation
```

The test suite runs the synthetic-OAuth request capture with participant
network disabled, decodes Pi's zstd request, and proves exact instructions,
one user input, no hidden resources/tools/continuation/retry/compaction, and no
external fetch. It also probes nonroot mode-0400 reads, write refusal, Unix
socket sharing between two containers, and post-exit cleanup. Synthetic capture
OAuth is not a usable credential.

No real participant run is authorized until the packet is committed and clean,
all no-model evidence is independently rooted in an execution attestation, and
the materialized study says `ready_for_participant_runs: true`. Run the four
fresh sessions sequentially in the fixed `plan.json` order. Never place the
derived credential, its path, any auth-derived identifier, or its bytes in a
manifest, log, result, or Git.

## Scientific limits

This remains one synthetic experimental unit repeated four times. All four
milestones remain `not_measured`; transitions per expert minute remains
`not_computable`. No causal, productivity, adoption, external-independence,
scientific, protocol, authority, Standing, accepted-state Correction,
rooted-dependent, or Class E claim is permitted. A Verification string,
fixture signature, profile, reducer status, model answer, or green scorer is
not a Decision or acceptance.
