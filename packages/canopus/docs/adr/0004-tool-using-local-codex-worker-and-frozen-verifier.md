# ADR 0004: Tool-using local Codex worker and frozen verifier

- Status: Accepted
- Release: Canopus `v0.2.0`
- Scope: one authority-free local research mission

## Context

Canopus Mission v0 deliberately gave Codex no tools. That contract remains
useful for reproducing frozen benchmarks, but it cannot test whether Canopus is
useful for ordinary research. The current Erdős frontier also has known strict
debt unrelated to a new producer task. Treating the frontier as strict-clean
would make the harness unusable; ignoring the debt would weaken Vela.

The useful next test is narrower than the deferred independent-participant
program: select the first ranked non-review attack target, let one local Codex
worker inspect an exact checkout and produce a bounded artifact, verify frozen
bytes in a separate process, and land only a pending Receipt. This is
first-party engineering and research evidence. It earns no independent,
scientific, human, or acceptance credit.

## Decision

Canopus introduces `canopus.mission.v1` and these primary commands:

```text
canopus mission prepare
canopus mission validate
canopus run
canopus inspect
```

Mission v0 and its tool-free execution remain supported for frozen benchmark
reproduction. Benchmark commands remain available but move out of default-help
prominence.

Mission v1 binds:

- exact Git commit and tree;
- Vela version, binary SHA-256, event root, and snapshot root;
- the first ranked attack target and exact target-packet path and digest;
- the complete strict blocker set by canonical root and per-rule counts;
- direct Codex version and native binary SHA-256, model, a bundled default-deny
  permission profile and its SHA-256, the target-packet-only workspace mode,
  bundled structured-output schema and its SHA-256, and resource ceilings;
- a separately stored verifier capsule, capsule and executable SHA-256, Docker
  image, argv, cwd, timeout, and output limit;
- allowed artifact paths, prompt and artifact budgets, and the exact
  scientific-chain assertion; and
- a single allowed landing outcome: `defer` with accepted-event delta zero.

`mission prepare` derives roots and identities from a clean source checkout,
the installed native Codex binary, and the locally present verifier image. It
copies the exact packet, permission profile, structured-output schema, and
executable capsule into a fresh portable bundle. It rejects a dirty checkout,
stale binary, changed packet, missing or non-executable capsule, unavailable
image, target override, unbounded objective, or malformed budget. `mission
validate` rechecks the closed contract and bundled bytes without making a model
call.

## Registered strict debt

Mission v1 does not turn strict failure into success. It registers one exact
baseline so an unrelated task can run on a debt-bearing frontier.

The canonical baseline is derived from the complete strict `signals.blockers`
array after every non-signal check passes and the summary reports zero
structural errors and invalid findings. Canopus sorts canonical blocker objects,
hashes the complete array, and records sorted per-rule counts.

Every control-lane inspection requires equality with that baseline. A missing,
new, or altered blocker; event failure; state-integrity failure; policy failure;
root change; malformed strict output; or exit/status disagreement fails closed.
Mission v0 retains the strict-clean requirement.

## Worker boundary

The worker runs the exact native Codex CLI on macOS with shell and patch actions
available under a bundled custom permission profile. The profile defaults to
deny, grants minimal operating-system reads, grants writes only beneath the
fresh workspace, denies command network, disables login shells, and inherits no
host environment. Canopus exposes only the hash-verified target packet in that
workspace. The full source checkout, landing clone, Vela home, host home,
authentication file, verifier capsule, and unrelated repositories remain
outside command-readable paths.

Codex itself retains provider transport and reads an ephemeral credential copy
for startup. The spawned shell cannot read that copy. Codex runs `--ephemeral`
with strict configuration and has browser, web search, MCP, apps, memories,
computer use, multi-agent work, plugins, hooks, goals, image tools, and other
ambient surfaces disabled. The prompt forbids signing, authority claims,
credential paths, process inspection, and use of Vela or the verifier as an
oracle.

Every run executes a deterministic sandbox preflight through the exact Codex
binary. It must prove the runtime authentication file, sealed source checkout,
host canary, and an outside-workspace path are respectively unreadable,
unreadable, unreadable, and unwritable. The live hostile model fixture also
requires a positive shell sentinel while proving source/runtime credentials,
the canary, unrelated repositories, process-environment authentication, outside
writes, and command network unavailable. A readable secret, secret-bearing
output, workspace escape, forbidden command, or unbounded action stops before
verification or landing.

## Verifier boundary

The producer cannot invoke the registered verifier. After the worker exits,
Canopus freezes artifact bytes and launches a second pinned image with:

- network mode `none`;
- a read-only root;
- all capabilities dropped and no-new-privileges;
- exact input, capsule, and artifact bytes mounted read-only;
- no host home, credentials, Docker socket, Vela key directory, or landing
  checkout; and
- bounded processes, memory, CPU, time, and output.

The capsule SHA-256 must equal both its registered capsule root and executable
digest. Artifacts are rehashed before and after verification. A nonzero verifier
result is an honest failure, not a success candidate.

The worker's `success` status means only that it produced the complete candidate
bytes required by the output contract. It must not claim verifier or scientific
success. `null` means no candidate was produced; `failed` means the producer
could not produce a contract-complete candidate or observed disqualifying
evidence. This distinction lets an isolated producer hand uncertain candidate
bytes to the separate verifier without collapsing the two trust roles.

## Landing and authority

Only a verifier-passing candidate reaches `vela land`, using an `agent:` actor.
Mission v1 permits only `Deferred` or its exact-retry equivalent, with accepted
event delta zero. Canopus first commits exactly the frozen artifact sources in
one unsigned, explicitly non-authoritative Git commit. This keeps `vela.lock`
and the Git tree self-contained without changing accepted scientific state.
Canopus then verifies the retained Receipt and artifact bindings and reproduces
roots and verifier digests in a clean clone.

Canopus exposes no `sign` or `accept` method. It does not read or trigger a
human scientific key. Git publication records bytes; it does not create
scientific acceptance.

## Stop and release gates

Stop on any:

- authentication, canary, or human-key exposure;
- workspace or mount escape;
- verifier network or persistent-write access;
- changed packet, capsule, image, binary, event, snapshot, or strict baseline;
- historical rewrite;
- false strict pass or false authority claim;
- accepted-state change; or
- mismatch in clean-clone reproduction.

Canopus `v0.2.0` may release only after focused tests, live hostile isolation,
and one real first-ranked mission complete safely. A safe null result is
preserved as run evidence but does not satisfy the release gate.

## Conformance

```bash
pnpm check
pnpm build
node --test dist/tests/mission-v1.test.js
node --test dist/tests/engine-tools.test.js
node --test dist/tests/verifier.test.js
node --test dist/tests/integration/released-vela.test.js
pnpm pack --dry-run
git diff --check
```

Live tests additionally prove the hostile custody boundary and a real
network-denied, write-denied verifier run. They never call `vela sign`.

## Consequences

Canopus becomes useful for one ordinary bounded task without adding an
authority surface or Vela primitive. The exact strict-debt registration keeps
the current frontier usable without laundering known failures. If the
experiment fails, Mission v0 and released Vela remain intact and the failure
identifies an orchestration gap rather than a protocol mandate.

## Implementation evidence

The Docker worker design was rejected. Codex could not create its nested Linux
Bubblewrap namespace while Docker dropped all capabilities, and relaxing the
outer seccomp boundary made credentials and the canary readable. Those failures
remain non-authoritative evidence and caused no verifier or landing effect.

The native default-deny profile passed the corrected live custody fixture: the
shell executed, while source and runtime authentication, unrelated repositories,
the host canary, outside-workspace writes, command network, and auth-bearing
process environment remained unavailable. The separate verifier fixture denied
network, root/input/artifact/capsule writes, and host-home visibility.

The first-ranked mission `erdos:1056` then completed on Vela `v0.800.23`. The
worker used 187,013 reported tokens and produced a 278-byte bounded-negative
artifact for every prime in `10428008..10428200`. The independently frozen
capsule exited zero, Vela retained Receipt root
`sha256:be2b34b57eac8a41d689f411d9dc1c97328a7901f943bb1cc023c843adc672bf`
as `defer`, accepted-event delta was zero, and a clean clone reproduced the
same verifier output and final roots. This satisfies the engineering release
gate. It does not authenticate the model, accept the scientific claim, or earn
independent-replication credit.
