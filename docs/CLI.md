# Vela command contract

Vela keeps scientific authority in signed frontier events. Agents inspect,
reproduce, claim work, and land evidence. A signed policy may admit a bounded
class. A human key holder decides deferred proposals. Git publishes the bytes.

## Daily path

Default help exposes twelve commands:

```text
init status next work land review check reproduce verify log doctor migrate
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

`land` builds or imports Receipt v1. On an Era-0 Frontier it routes through
the active signed AcceptancePolicy. On a repository-authority Frontier it
authenticates one exact signed activity record and retains an object-only
pending proposal under the active Cedar bundle. That transaction appends no
accepted scientific event. An agent cannot accept or reject the result.

On Profile v1, `vela check` validates the repository context as a read-side
gate. A valid event log is necessary but not sufficient: Vela checks the
closed profile and settings, replay/parity, signed boundary chain, Git anchor
and ancestry, retained canonical bytes, actor registry, and—when an
administrator boundary exists—the consumer's independent first-boundary pin.
A repository-context defect never becomes an identity or
historical-signature exemption.

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
| `land` | Build or import Receipt v1, retain the exact pending or policy-routed result, and publish its covered bytes when requested. |
| `review` | List proposal summaries or inspect one exact Decision Brief. |
| `check` | Verify schemas, replay, signatures, roots, policy, and strict signals. |
| `reproduce` | Run stored evidence through its frozen verifier. |
| `verify attach` | Retain signed verifier evidence against one exact pending proposal; never accept it. |
| `log` | Read accepted event history. |
| `doctor` | Report blockers and one repair action. `--all` adds diagnostics. |
| `migrate` | Preview or apply a root-preserving repository-format migration. |

Setup and noun-oriented commands remain available:

```text
finding artifact frontier policy actor id authority agents config
```

`authority migrate` is temporary `0.930.0-rc.7` migration scaffolding, not a
daily signing surface. Its key-free preview and exact protected apply contract
are documented in [SIGNING.md](SIGNING.md#authority-model-migration-candidate).

Advanced help contains:

```text
gate proof ci serve
```

Advanced setup also exposes `target-index` maintenance. Run
`vela help advanced` for the complete grouped list.

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
    "scientific_state_root": "sha256:...",
    "legacy_snapshot_root": "sha256:...",
    "proposals": "sha256:...",
    "actor_registry": "sha256:...",
    "artifacts": "sha256:..."
  },
  "integrity": {
    "replay": "reproduced",
    "strict": "blocked",
    "blocker_count": 3,
    "blockers_by_code": {"missing_conditions": 3},
    "repository_context": {
      "generation": "profile_v1",
      "valid": true,
      "identity_mode": "pinned_boundary",
      "trust_anchor_root": "sha256:..."
    }
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

For Profile v1, `scientific_state_root` is the closed scientific-state root and
`legacy_snapshot_root` names the historical snapshot algorithm explicitly.
For a read-only Profile v0.1 replay, `scientific_state_root` is `null`,
`legacy_snapshot_root` remains available, and
`frontier.profile_generation` is `legacy_v0_1`; the command never relabels the
legacy snapshot as the current scientific root.

Profile v1 lockfiles also pin the exact Vela and verifier versions that
produced their non-scientific derived views. A newer compatible reader
validates `frontier.json` and `legacy_snapshot_root` against those pinned
materializer identities; it does not substitute its own version and falsely
report an untouched checkout as stale. Running `frontier materialize`
explicitly advances the derived-view metadata to the current binary. Neither
path changes the closed scientific-state root or grants authority.

Compact status runs the same complete Profile v1 repository-context gate as a
canonical write: profile and settings, reducer and proposal parity, exact Git
boundary, retained bytes, and the independently installed consumer trust pin.
For a verified pinned legacy boundary, proposal logical-ID conflicts already
present at the exact anchor are reported as
`anchored_immutable_unauthenticated` audit debt rather than current tampering.
The boundary freezes every such proposal byte and grants it no authentication
or authority; any new conflict, changed byte, invalid anchor, or missing pin
still blocks strict checking and every canonical write.
The verified pin is also supplied to Target Index v2 assessment. A missing,
wrong, tampered, forked, or incomplete boundary/pin appears under
`integrity.repository_context`, contributes its exact code to
`blockers_by_code`, makes strict standing `blocked`, and grants no producer
offer.

The command omits Decision Briefs, packet bodies, pressure metrics, and test
telemetry. Use `review show` for a full brief.

### Producer offer

`vela next . --limit 1 --json` emits `vela.offer.v1`. Each item contains its
rank, target ID, packet path and root, objective, verifier profile, lease state,
and next command. The top-level `availability` object has the exact integer
fields `configured`, `stale`, `leased`, `available`, and `returned`, plus the
single `repair_command`. The object is closed and does not also emit
`configured_open`.

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

The command creates Profile v1, closed `.vela/settings.toml`, the canonical
empty stores, one unsigned structural `frontier.created` event, `README.md`,
`SCOPE.md`, Git safety files, and `VELA.md`. The event establishes structural
identity and the canonical empty dependency root. It does not authenticate an
administrator or grant scientific authority.

Initialization does not install MCP, CI, proof packets, target indexes, domain
directory matrices, or editor adapters. Add an optional integration through
its named setup command after the frontier exists.

### First administrator

A new Frontier's first human administrator is a rare protected repository
ceremony:

```bash
vela id create
vela id protect --json
vela actor add . --json

# Inspect and commit only the exact actor-bootstrap paths.
git status --short
git diff
git add <exact-paths-from-the-bootstrap-delta>
git commit -m "Bootstrap Frontier administrator"

vela frontier bind . \
  --reason "establish the first administrator" \
  --json
vela frontier bind . \
  --reason "establish the first administrator" \
  --confirm-root <sha256:...> \
  --confirm-at <RFC3339> \
  --json
```

`actor add` works only against the canonical empty registry and only for a
protected human identity whose key matches the candidate record. Its possession
proof and the resulting delta are exact; it is not a general actor-registration
writer. The subsequent repository boundary requires a `reviewer:` or
`steward:` identity.

`frontier bind` previews without reading the key. Matching execution asks the
protected OS signer for one exact approval, appends one signed,
non-scientific boundary event, and installs the local first-boundary pin. It
does not stage, commit, or push. Inspect and commit its exact delta separately.

`0.914.0` does not expose a writer for later dependency changes. The first
boundary's exact dependency set remains fixed until a separately reviewed
two-phase protected-update command is released; do not hand-author a
`previous_boundary` event.

Another consumer receives the full first-boundary content root through an
independent channel and installs it in two phases:

```bash
vela frontier trust pin . --boundary-root <sha256:...> --json
vela frontier trust pin . --boundary-root <sha256:...> \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

The pin is public local consumer configuration under
`~/.vela/trust/frontiers/`, not a key, policy, event, or source of scientific
authority. Vela never derives it automatically from repository bytes.

## Review and authority

Agents may run:

```bash
vela review list . --json
vela review show . <vpr_id> --json
vela review preview . <vpr_id> --json
vela review decide . <vpr_id> --reject --reason <text> --json
```

If a task already supplies the full `vpr_` ID, begin with `review show`.
It returns the pending Decision Brief or signed terminal decision record in one
read. Rejected proposal findings are intentionally absent from accepted
`finding show` and `log` views; that absence is not deletion.

On an Era-0 Frontier, the first `review decide` call is key-free and returns a
`vela.review-decision.v1` preview. Codex may invoke the second call with the
matching `--confirm-root` and `--confirm-at`; only the registered human's exact
decision-card action authorizes the helper to use the protected key.

On a verified repository-authority Frontier, `review decide --accept|--reject`
instead derives and locks one exact `vela.repository-review-decision.v1` plan,
asks the platform for fresh user presence, evaluates restricted Cedar, and
asks the repository authority to sign the covering transaction. It accepts no
copied confirmation root or timestamp and reads no human Vela key.
Cancellation, authentication failure, stale state, policy denial, or
repository-signing failure writes nothing. Accept remains unavailable whenever
the Decision Brief or strict aggregate Engine gate blocks it; a permitted
accept installs and replays both the scientific domain event and the explicit
review event through the dual log.

Neither path accepts a key path, `--yes`, batch, wildcard, or saved-session
input. `vela sign` remains in advanced help for historical batch sessions and
detached files.

AcceptancePolicy is now a frozen Era-0 compatibility format. Its retained
bytes, signatures, policy heads, and admissions remain inspectable:

```bash
vela policy show . --json
vela policy test . --json
vela policy evaluate-proposal . <vpr_id> --json
vela policy log . --json
```

`show`, `test`, `evaluate-proposal`, and `log` are read-only compatibility
operations. Policy suggestion, drafting, activation, rotation, revocation,
legacy-pair preparation, hidden signing aliases, and CI auto-merge verdicts
are retired. Existing Era-0 frontiers replay unchanged; new authority is
established through the attributed repository-authority migration and
restricted Cedar policy.

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

Profile v0.1 remains readable, replayable, reproducible, and migratable. It is
not writable in Vela 0.914. Migration requires an external candidate Profile
v1 file and a domain-owned Target Index v2 candidate; a Frontier with legacy
dependencies also requires a closed dependency-resolution input.

Preview the exact protected plan:

```bash
vela migrate . --to frontier-repo-v1 --check \
  --profile ../frontier-profile-v1.yaml \
  --target-candidate ../target-index-candidate.json \
  --as reviewer:<administrator> \
  --reason "Bind exact legacy repository" \
  --json
```

The JSON preview is a compact reader projection. It reports the exact
candidate, index, packet, input, and Decision Plan roots plus target and packet
counts, without embedding the complete target collection a second time. The
`plan_root` still binds the full sealed target index and every packet byte.

Apply only the matching plan:

```bash
vela migrate . --to frontier-repo-v1 --apply \
  --profile ../frontier-profile-v1.yaml \
  --target-candidate ../target-index-candidate.json \
  --as reviewer:<administrator> \
  --reason "Bind exact legacy repository" \
  --confirm-root <sha256:...> \
  --confirm-at <RFC3339> \
  --json
```

When the legacy profile has dependencies, add
`--dependency-input ../dependency-migration.json` to either command.

Vela `0.914.0` requires each entry's exact state to equal the verified signed
dependency-boundary anchor. Accepted ADR 0018 keeps the same
closed input and exact dependency record but may rederive a named historical
commit as a retained ancestor of the independently pinned first
temporalization anchor. It never accepts a branch, tag, timestamp, short ID,
current-state substitution, or unverified Git ancestry. The pin remains
context rather than evidence or authority.

The preview is key-free. It binds the candidate files, signer, reason, Git and
Vela roots, exact touched paths, before/after root families, dependency
resolutions, sealed target index, and one planned non-scientific boundary
event. Matching apply revalidates those facts and expiry before the protected
OS card. Repository-boundary approval remains one-shot and root-bound, with a
ten-minute card window so a full retained-history revalidation does not turn
ordinary human review into a race.

Migration refuses dirty input, incomplete recovery, shallow or non-ancestor
history, replay failure, root drift, unknown legacy settings, ambiguous
dependencies, and unexpected paths. It preserves every pre-boundary event,
proposal, Receipt, registration, policy, finding, artifact, evidence object,
and signature byte. It appends one boundary event, so Git and event-log roots
change intentionally. Scientific debt remains visible. Apply leaves an exact
uncommitted delta for human inspection; it does not stage, commit, or push.

### Private recovery compaction

After an operation is settled and published, frontier transactions can discard
verified postimage byte copies while retaining the exact plan, commit marker,
file-state commitments, and event membership needed for idempotency and
fail-closed checks. Compact this ignored, non-scientific recovery data with:

```bash
vela frontier compact-recovery . --json
```

The command first verifies every completed marker, retained blob, event-set
root, and non-derived postimage. It refuses active or incomplete recovery and
never changes tracked frontier bytes.

## Target-index maintenance

The hidden advanced setup surface seals domain-owned candidates; it does not
author target meaning:

```bash
vela target-index repair . --json
vela target-index seal . --candidate <candidate.json> --check --json
vela target-index seal . --candidate <candidate.json> --apply --json
vela target-index inspect . [<full-target-id>] --json
```

`next` and `work` require a fresh, tracked Profile v1 Target Index v2 and exact
packet bytes. Stale indexes grant no work. A claimed target is retained as
`vela.target-task-binding.v1` in both the private session and the eventual
Receipt.

Native Windows is read/check/reproduce capable, but Profile v1 settings writes
and `target-index seal --apply` are deliberately unavailable in this release.
Those commands require a handle-relative atomic exchange that preserves and
checks the displaced exact preimage. `ReplaceFileW` is path-based, while the
documented Win32 `FileRenameInfoEx` operation replaces rather than exchanges
the destination. Vela therefore fails before creating a temporary or touching
the destination. Use WSL2 with the checkout on its Linux filesystem, or a
supported Unix host; a checkout under `/mnt/c` is not the supported WSL2
mutation path.

## Local serving

```bash
vela serve .                       # read-only MCP over stdio
vela serve . --profile draft       # adds only nonfinalizing work
vela serve . --http 3741           # same selected profile on loopback
```

The HTTP read path has no authenticated request identity. It ignores
caller-asserted actor names and returns only public-tier data even when the
nonfinalizing draft tool is selected. Neither profile offers signing or a
protected-decision operation. It cannot be turned into a network service by
supplying another bind address.

## Strict and non-strict checking

```bash
vela check . --json
vela check . --strict --json
```

Both forms report invalid Profile v1 repository context with the same typed
signal and `valid: false`. Non-strict mode is useful for diagnosis, but grants
no boundary, identity, dependency, signature, or historical exemption. Strict
mode also makes the signal fatal. Canonical writers use the strict
repository-context gate regardless of how a prior diagnostic check was run.

## Retired 0.8 surfaces

| Old command | Replacement |
| --- | --- |
| `proposals` | `review` |
| `diff vpr_*` | `review preview` |
| `diff <left> <right>` | `frontier diff` |
| `state` and `credit` | `finding show --view ...` |
| `publication` | `frontier recover-publication` |
| `hub` | `vela serve` for a local read surface, or the optional Observatory |
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
