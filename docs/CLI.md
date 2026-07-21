# Vela 0.9 command contract

Vela keeps scientific authority in signed frontier events. Agents inspect,
reproduce, claim work, and land evidence. A signed policy may admit a bounded
class. A human key holder decides deferred proposals. Git publishes the bytes.

## Daily path

Default help exposes eleven commands:

```text
init status next work land review check reproduce log doctor migrate
```

The ordinary producer loop is:

```bash
vela status . --json
vela next . --limit 1 --json
vela work <target> --as agent:<name> --json

# Run the verifier named by the work response.

vela land --work <target> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

vela check . --strict --json
```

`land` builds or imports Receipt v1 and routes the proposal through the active
signed policy. An agent cannot accept or reject the result.

An exact delegated producer may additionally pass all four full roots:

```bash
vela land --work <target> ... \
  --packet-root <sha256:...> \
  --profile-root <sha256:...> \
  --verifier-capsule-root <sha256:...> \
  --result-contract-root <sha256:...>
```

Vela validates the all-or-nothing set and authors the closed
`vela.execution-binding.v1` extension through the same Receipt builder. The
roots are evidence, not an authority request; only an already signed matching
AcceptancePolicy v0.2 can Permit the positive result.

For the v0.2 exact-witness floor, declare one retained public artifact as
`<path>:vela-witness`. The executable verifier can check the same bytes and
claim independently:

```bash
vela-verify --claim '<exact lower-bound claim>' <witness.json>
```

A valid construction with a substituted or inflated claim exits nonzero.

## Commands

| Command | Contract |
| --- | --- |
| `init` | Create a minimal Git frontier from a name and bounded scope. |
| `status` | Report Git identity, full roots, replay, blockers, counts, policy readiness, and one next action. |
| `next` | Rank producer targets. Review work never appears here. |
| `work` | Claim one target and write a private, typed session. |
| `land` | Build or import Receipt v1, run policy, and publish the resulting bytes when requested. |
| `review` | List proposal summaries or inspect one exact Decision Brief. |
| `check` | Verify schemas, replay, signatures, roots, policy, and strict signals. |
| `reproduce` | Run stored evidence through its frozen verifier. |
| `log` | Read accepted event history. |
| `doctor` | Report blockers and one repair action. `--all` adds diagnostics. |
| `migrate` | Preview or apply a root-preserving repository-format migration. |

Setup and noun-oriented commands remain available:

```text
finding artifact frontier policy actor id agents config
```

Advanced help contains:

```text
gate proof ci serve
```

Run `vela help advanced` for the complete grouped list.

## Compact JSON

### Status

`vela status . --json` emits `vela.status.v1`:

```json
{
  "schema": "vela.status.v1",
  "frontier": {"id": "vfr_...", "name": "..."},
  "git": {"commit": "...", "tree": "...", "clean": true},
  "roots": {
    "event_log": "sha256:...",
    "snapshot": "sha256:...",
    "proposals": "sha256:...",
    "actor_registry": "sha256:...",
    "artifacts": "sha256:..."
  },
  "integrity": {
    "replay": "reproduced",
    "strict": "blocked",
    "blocker_count": 3,
    "blockers_by_code": {"missing_conditions": 3}
  },
  "counts": {
    "events": 10,
    "findings": 4,
    "open_work": 2,
    "pending_review": 1
  },
  "policy": {"state": "active", "permit_readiness": "ready"},
  "next_action": "vela next . --json"
}
```

The command omits Decision Briefs, packet bodies, pressure metrics, and test
telemetry. Use `review show` for a full brief.

### Producer offer

`vela next . --limit 1 --json` emits `vela.offer.v1`. Each item contains its
rank, target ID, packet path and root, objective, verifier profile, lease state,
and next command.

### Work session

`vela work <target> --json` emits `vela.work.v1`. It binds the session, starting
roots, exact packet, completion contract, verifier profile, and landing
command. Repeating the same claim by the same actor while its lease remains
active returns the exact session with `idempotent: true`; it does not refresh
the lease or append another event. The full packet stays in the tracked packet
file.

### Review queue

`vela review list . --json` emits `vela.review.v1` with compact, paginated
proposal summaries ordered by `created_at` newest-first, then full proposal ID.
Every row carries `created_at`; `review show` and `review preview` return one
Decision Brief. `review decide` prepares or approves one exact protected decision.
`review withdraw` lets a Receipt-bound producer close its own pending proposal.
`review export` writes proposal records without deciding them.

## Initialization

JSON mode requires both inputs:

```bash
vela init ./frontier \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --json
```

The command creates the canonical skeleton, `README.md`, `SCOPE.md`, Git safety
files, and `VELA.md`. It does not install MCP, CI, proof packets, or editor
adapters. Add an optional integration through its named setup command after
the frontier exists.

## Review and authority

Agents may run:

```bash
vela review list . --json
vela review show . <vpr_id> --json
vela review preview . <vpr_id> --json
vela review decide . <vpr_id> --reject --reason <text> --json
```

The first `review decide` call is key-free and returns a
`vela.review-decision.v1` preview. Codex may invoke the second call with the
matching `--confirm-root` and `--confirm-at`; only the registered human's exact
decision-card action authorizes the helper to use the protected key. The command accepts no key
path, `--yes`, batch, wildcard, or saved-session input. `vela sign` remains in
advanced help for historical batch sessions and detached files.

Policy administration uses the same exact-request flow:

```bash
vela policy draft search-witness . \
  --from-proposal <vpr_id> \
  --replace \
  --json
# Run the returned policy-only Git commit command.
vela policy decide . --activate <vap_id> --reason <text> --json
vela policy decide . --rotate <vap_id> --reason <text> --json
vela policy decide . --revoke --reason <text> --json
```

`--from-proposal` derives the packet, profile, verifier-capsule,
result-contract, and producer-credential roots from one retained pending
Receipt. It seals the narrower AcceptancePolicy v0.3 contract: even a globally
registered producer must match the exact credential root named by the rule.
The four execution-root flags plus `--producer-credential-root` remain an
advanced, all-or-nothing authoring path. Omitting the producer credential seals
the registry-backed v0.2 contract. The draft remains unsigned and carries no
authority until protected activation or rotation. Draft output separates the
required policy-only Git commit from the subsequent Decision Plan request so
the protected path never encounters an unexpectedly dirty checkout. Negative
results and mismatched roots Defer.

The first call returns a key-free `vela.policy-decision.v1` plan. A second call
with its exact `--confirm-root` and `--confirm-at` rechecks Git and frontier
identity, event and actor-registry roots, the full policy bytes, rule summary,
reviewer authority, current policy head, binary pin, and transaction read set
before showing one protected policy card. Cancellation writes no policy
signature, event, journal commit marker, or Git commit. Historical `policy
sign`, `policy revoke --key`, and `--yes` remain advanced compatibility paths;
ordinary documentation does not teach them.

A human identity moves its seed into the local OS credential store through the
one-shot helper. Enrollment authenticates once before reading the source:

```bash
vela id protect --json
vela id show --json
vela id lock --json
```

Protection uses safe defaults: it authenticates the human, verifies the public
key, installs identity v2 atomically, removes the plaintext source, and pins the
exact Vela binary. The 0.901 safety flags remain accepted but are not needed.
Every request binds and self-verifies the current sibling helper. An
interrupted cleanup leaves protected decisions disabled until the same command
safely resumes. The default session has 15-minute inactivity and one-hour
overall limits. `--mode always` additionally requires LocalAuthentication,
Windows Hello, or non-cached polkit authentication for every decision-signing
operation. `id lock` closes only the bounded local session; it does not alter
the protected identity or a frontier. Agent identities continue to use file
keys. Provider and binary details are diagnostics, not the human identity.

A producer may close only its own Receipt-bound pending proposal:

```bash
vela review withdraw . <vpr_id> --as agent:<producer> --reason <text> --json
```

Withdrawal retains every proposal, Receipt, record, and artifact byte. It
changes no accepted scientific state.

Git publication cannot accept a claim. It publishes the proposal, verifier
evidence, or signed decision bytes that already exist.

## Migration

Preview first:

```bash
vela migrate . --to 0.900 --check --json
```

Apply the exact preview:

```bash
vela migrate . --to 0.900 --apply --json
```

Migration refuses dirty input, an incomplete journal, non-ancestor state,
replay failure, root drift, and an unexpected output path. A clean migration
keeps canonical event, proposal, Receipt, registration, artifact, and signed
policy bytes unchanged. Scientific debt remains visible.

## Retired 0.8 surfaces

| Old command | Replacement |
| --- | --- |
| `proposals` | `review` |
| `diff vpr_*` | `review preview` |
| `diff <left> <right>` | `frontier diff` |
| `state` and `credit` | `finding show --view ...` |
| `publication` | `frontier recover-publication` |
| `hub` | the separate `vela-hub` binary |
| `foundry`, `atlas`, `reproduce-external` | Canopus profiles or parent campaign scripts |

Retired commands exit with one migration hint. Vela does not execute aliases.

## Exit behavior

- `0`: the requested read or write completed.
- `1`: verification, replay, migration, or domain integrity failed.
- `2`: command usage or a retired surface.
- `3`: a referenced object does not exist.
- `4`: an agent attempted a human-only authority action.

JSON mode writes one object to stdout. Human diagnostics use stderr only for
errors and migration hints. `NO_COLOR=1` removes ANSI output.
