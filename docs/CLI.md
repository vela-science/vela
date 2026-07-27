# Vela command contract

Vela is a Git-native scientific-state tool. Agents and people submit evidence,
Verification Records report scoped checks, governed Decisions change standing,
and Git publishes the exact bytes.

## Daily path

Default help exposes twelve commands:

```text
init status next start submit show why review check reproduce log doctor
```

The ordinary producer loop is:

```bash
vela status . --json
vela next . --limit 1 --json
vela start <target> --frontier . --as agent:<name> --json

# Run the verifier named by the Attempt response.

vela submit --frontier . \
  --attempt <vat_id> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

vela check . --strict --json
```

`submit` builds or imports Submission v1. Current Frontiers authenticate exact
producer input, retain a Registration Record, and create a pending Proposal.
The transaction appends no Verification Record, Decision, Event, or accepted
scientific-state change. An agent cannot accept or reject a Proposal.

## Commands

| Command | Contract |
| --- | --- |
| `init` | Create a minimal Git frontier from a name and bounded scope. |
| `status` | Report Git identity, full roots, replay, blockers, counts, policy readiness, and one next action. |
| `next` | Rank producer targets. Review work never appears here. |
| `start` | Start one bounded Attempt against an exact Target. |
| `submit` | Build or import Submission v1 and register a pending Proposal. |
| `show` | Inspect one exact current or historical typed object. |
| `why` | Explain a Claim's standing from exact evidence, verification, Decisions, Events, and corrections. |
| `review` | List, inspect, diff, accept, reject, or export Proposals. |
| `check` | Verify schemas, replay, signatures, roots, policy, and strict signals. |
| `reproduce` | Run retained evidence through its frozen verifier. |
| `verification import` | Retain one scoped Verification Record; never accept it. |
| `log` | Read accepted event history. |
| `doctor` | Report blockers and one repair action. `--all` adds diagnostics. |

Setup and read-oriented nouns:

```text
claim artifact attempt submission registration verification proposal frontier policy actor id agents config
```

Advanced verification and integration:

```text
gate proof serve target-index
```

Run `vela help advanced` for the grouped list. Historical personal signing,
repository migration, actor mutation, and policy authoring are not live
commands in the current candidate.

## Compact JSON

### Status

`vela status . --json` emits `vela.status.v2`. It includes:

- Frontier and Git identity;
- full event, scientific-state, proposal, actor-registry, and artifact roots;
- repository-context and replay validity;
- strict blocker totals by stable code;
- event, finding, work, and review counts;
- current policy readiness; and
- one next action.

It does not embed Review Packets, packet bodies, test telemetry, or private
coordination.

### Producer offer

`vela next . --limit 1 --json` emits `vela.offer.v1`. Each offer contains its
canonical rank, target ID, packet path and root, bounded objective, verifier
profile, lease state, and next command.

Review items never enter producer ranking. A stale or invalid Target Index
produces no offer.

### Attempt

`vela start <target> --frontier . --json` emits `vela.attempt.v1` with a
`vat_` identifier. It binds:

- the exact target and packet;
- starting roots;
- completion contract;
- verifier profile;
- lease state; and
- current producer submission command.

Repeating an active claim by the same actor is idempotent.

### Review

```bash
vela review list . --json
vela review show . <vpr_id> --json
vela review diff . <vpr_id> --json
```

`review list` emits compact paginated summaries. `review show` returns either
one pending Review Packet or one terminal Decision record.

An authorized person executes one exact decision:

```bash
vela review reject . <vpr_id> \
  --reason "insufficient independent evidence" \
  --json
```

The action and reason are bound into one repository-authority transaction.
There are no copied confirmation roots, timestamps, Vela human keys, batch
answers, or custom signer helpers. See [SIGNING.md](SIGNING.md).

A producer may withdraw only its own pending Proposal:

```bash
vela proposal withdraw . <vpr_id> \
  --as agent:<name> \
  --reason "superseded" \
  --json
```

## Initialization

```bash
vela init ./my-frontier \
  --name "Bounded question" \
  --scope "Does X hold?"
```

JSON mode requires both name and scope. Initialization creates the canonical
Frontier skeleton and Git safety files. It does not generate an agent runtime,
CI integration, proof packet, hosted service, or scientific decision.

## Repository context

Every canonical writer runs the same fail-closed Profile v1 gate as strict
checking:

- profile and settings;
- Git anchor and ancestry;
- repository trust anchor;
- authority-history continuity;
- retained canonical bytes;
- reducer and proposal parity;
- actor and artifact registries; and
- transaction recovery state.

Non-strict checks may report defects for diagnosis. They never grant a trust,
signature, authority, or historical exemption.

Consumer trust pins are local:

```bash
vela frontier trust pin . --boundary-root sha256:... --json
```

Pinning changes no Frontier history.

## Claims, Submissions, and Artifacts

Use the universal read path:

```bash
vela show . <typed_id> --json
vela why . <claim_id> --json
```

Historical `vf_` Finding bytes remain replayable and are projected as Claim
Records with their source era disclosed. Current producer writes use Submission
v1 plus `submit`.

Artifact retraction creates a pending proposal rather than deleting evidence:

```bash
vela artifact retract . <va_id> \
  --as agent:<name> \
  --reason "superseded evidence" \
  --json
```

## Policy and actors

The Era-0 policy and actor surfaces are replayable and read-only:

```bash
vela policy show .
vela policy test .
vela policy evaluate-proposal . <vpr_id>
vela policy log .
vela actor list .
```

Current authorization comes from retained Cedar bundles, attributed
principals, scoped capabilities, semantic human action where required, and
repository-authority records. Vela does not expose a general policy editor or
actor-registry mutation command.

## Target-index maintenance

The advanced target-index surface seals derived producer targets; it does not
invent their scientific meaning:

```bash
vela target-index repair . --json
vela target-index seal . --candidate <candidate.json> --check --json
vela target-index seal . --candidate <candidate.json> --apply --json
vela target-index inspect . [<full-target-id>] --json
```

`next` and `start` require a fresh tracked Target Index and exact packet bytes.

## Local serving

```bash
vela serve .                    # read-only MCP over stdio
vela serve . --profile draft    # adds nonfinalizing work only
vela serve . --http 3741        # same profile on loopback
```

The HTTP read path has no authenticated request identity. It ignores
caller-asserted actor names and returns public-tier data. Neither profile
offers review decisions, signing, policy administration, or accepted-state
mutation.

## Recovery data

Completed transaction journals are ignored operational data. Compact them only
after marker, blob, event-membership, and postimage verification:

```bash
vela frontier compact-recovery . --json
```

The command refuses active or incomplete recovery and never changes tracked
Frontier bytes.

## Retired surfaces

| Retired command | Current path |
| --- | --- |
| `proposals` | `review` |
| `diff vpr_*` | `review diff` |
| `diff <left> <right>` | `frontier diff` |
| `state` and `credit` | `show` and `why` |
| `publication` | `frontier recover-publication` |
| `hub` | `vela serve` or the read-only Observatory |
| `foundry`, `atlas`, `reproduce-external` | Canopus profiles or parent scripts |
| `work` | `start` |
| `land` | `submit` |
| `verify attach` | `verification import` |
| `review decide` | `review accept` or `review reject` |
| `sign`, `migrate` | historical replay only; no current writer |

Fresh Profile v1 repositories expose one advanced setup command:

```bash
vela authority init <frontier> \
  [--key SHA256:<full-openssh-fingerprint>] \
  --reason <bounded-reason> \
  --json
```

It selects a plain Ed25519 identity already loaded in the standard OpenSSH
agent and establishes sequence-1 repository authority. It is not a migration,
personal signer, policy editor, or scientific decision command.

Retired commands do not execute compatibility aliases.

## Exit behavior

- `0`: the requested read or write completed.
- `1`: verification, replay, or domain integrity failed.
- `2`: command usage or a retired surface.
- `3`: a referenced object does not exist.
- `4`: an agent attempted a human-only authority action.

JSON mode writes one object to stdout. Human diagnostics use stderr only for
errors and migration hints. `NO_COLOR=1` removes ANSI output.
