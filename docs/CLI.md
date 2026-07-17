# Vela 0.9 command contract

Vela keeps scientific authority in signed frontier events. Agents inspect,
reproduce, claim work, and land evidence. A signed policy may admit a bounded
class. A human key holder decides deferred proposals. Git publishes the bytes.

## Daily path

Default help exposes twelve commands:

```text
init status next work land review sign check reproduce log doctor migrate
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

## Commands

| Command | Contract |
| --- | --- |
| `init` | Create a minimal Git frontier from a name and bounded scope. |
| `status` | Report Git identity, full roots, replay, blockers, counts, policy readiness, and one next action. |
| `next` | Rank producer targets. Review work never appears here. |
| `work` | Claim one target and write a private, typed session. |
| `land` | Build or import Receipt v1, run policy, and publish the resulting bytes when requested. |
| `review` | List proposal summaries or inspect one exact Decision Brief. |
| `sign` | Run the human-only proposal decision ceremony. |
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
command. The full packet stays in the tracked packet file.

### Review queue

`vela review list . --json` emits `vela.review.v1` with compact, paginated
proposal summaries. `review show` and `review preview` return one Decision
Brief. `review export` writes proposal records without deciding them.

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
```

Only a human may run `vela sign`. Scripted decisions use a key-free preview,
then bind the exact Decision Plan root and observation time. Interactive and
scripted paths reject agent identities before key access.

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
