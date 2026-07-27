# Agent quickstart

Vela gives agents a producer boundary, not scientific authority. An agent may
inspect a frontier, run frozen verifiers, claim work, and land Receipt v1. A
signed policy may admit a bounded result. A human key holder decides deferred
proposals.

## Rules

Agents may:

- read `status`, `next`, `log`, `finding`, and `review`;
- run `check` and `reproduce`;
- start or abandon an Attempt through `start`;
- land a verifier-backed result through `land`; and
- regenerate derived views with `frontier materialize`.

Agents may not:

- approve a protected review or policy card, run legacy `sign`, or handle a
  human key (an agent may only prepare and invoke one exact protected request);
- read, copy, or invoke a human key;
- hand-edit accepted events or derived views; or
- describe verification or Git publication as scientific acceptance.

Use an agent identity on each write:

```bash
export VELA_ACTOR_ID=agent:demo
```

## One bounded loop

```bash
vela status . --json
vela next . --limit 1 --json
vela start <target> --as agent:demo --json
```

The Attempt response is `vela.attempt.v1`. It names the exact target, starting roots,
packet path and root, completion contract, verifier profile, and landing
command. Read the tracked packet and satisfy the first unresolved checkable
obligation.

Run the verifier named by the profile. Keep the artifact and replay command.
Then land one bounded claim:

```bash
vela land --work <target> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:demo \
  --json
```

A bounded negative search must record its algorithm, range, inputs, counts,
and replay command. It cannot support a universal nonexistence claim.

The landing response separates:

- verifier result;
- policy route;
- accepted-event delta;
- proposal ID; and
- Git publication state.

`Deferred` or `pending_review` means the result awaits a human decision.
`policy_admitted` means an earlier human-signed policy authorized that exact
class. Neither state lets the agent enter the key path.

## Stop or release work

Release an abandoned lease through Vela:

```bash
vela start <target> --drop \
  --reason "<why the attempt stopped>" \
  --as agent:demo \
  --json
```

Deleting `.vela/work/` does not release the frontier lease.

## Read review state

```bash
vela review list . --json
vela review show . <vpr_id> --json
```

Review reads do not resolve or read a key. The full Review Packet appears only
for one selected proposal. When a task supplies a full `vpr_` ID, make
`review show` the first read; it returns either the pending Review Packet or
the signed terminal decision record. A rejected proposal's candidate finding
is intentionally absent from accepted `finding show` and `log` views. That
absence is not deletion and does not require searching those surfaces first.

## Verification

```bash
vela check . --strict --json
vela reproduce .
```

`check` verifies frontier structure, event replay, signatures, roots, policy,
strict signals, and Profile v1 repository context. Repository context includes
the closed profile/settings, boundary chain, Git anchor and ancestry, retained
bytes, actor registry, and the consumer's independent first-boundary pin when
an administrator boundary exists. Non-strict inspection reports the same
defects but never grants an identity, dependency, signature, or historical
exemption. `reproduce` reruns stored scientific evidence. A frontier can replay
while strict scientific debt remains; report both states.

## Optional tool surface

`vela init` creates no MCP configuration. A project may opt into `vela serve`
after initialization. The draft profile can expose the nonfinalizing producer
loop. An agent may invoke one exact protected `review accept` or
`review reject` request, but only the registered human can authorize the exact
local decision card. MCP exposes no decision path.

`vela serve . --http 3741` is a loopback-only public reader. HTTP has no
authenticated actor identity and cannot expose restricted data, protected
decisions, or signing.

## Output contracts

- `vela.status.v1`: exact identity, roots, blockers, counts, policy, next action.
- `vela.offer.v1`: producer ranking only.
- `vela.attempt.v1`: one root-bound private Attempt.
- `vela.review.v1`: compact proposal summaries or one selected brief.

See [CLI.md](CLI.md) for command syntax and [PROTOCOL.md](PROTOCOL.md) for the
authority model.
