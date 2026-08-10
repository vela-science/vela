# Vela CLI

Vela is a Git-native scientific-state tool. Producers submit evidence,
verifiers report scoped results, only an authorized human Decision changes
Standing, and Git preserves the exact repository history.

## Ordinary workflow

```bash
vela status . --json
vela next . --limit 1 --json

vela submit --repo . \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

vela verification import . verification.json \
  --as verifier:<name> \
  --json

vela claims . --json
vela review show . <vpr_id> --json
vela replay . --json
vela why . <claim_id> --json
```

`submit` retains one authenticated Submission and creates a pending Proposal.
It does not create a Verification Record,
Decision, Event, or accepted Standing.

## Daily commands

Default help exposes exactly:

```text
init status claims submit show why review replay reproduce log
```

| Command | Contract |
| --- | --- |
| `init` | Create a signed, replayable repository ready for scientific work. |
| `status` | Report identity, replay, Decision Inbox readiness, and one safe next action. |
| `claims` | Page the repository claim index: Claim ID, one-line assertion, Standing, origin era. |
| `submit` | Build or import one authenticated Submission and pending Proposal. |
| `show` | Inspect one exact typed object and its authority effect. |
| `why` | Explain one Claim's Standing from retained roots and history. |
| `review` | List, show, accept, or reject one exact Proposal. |
| `replay` | Verify repository structure, roots, replay, and authority. |
| `reproduce` | Run retained evidence through its frozen verifier. |
| `log` | Read admitted Event history. |

## Advanced commands

```text
verification correction authority
```

- `why` also resolves a retained superseded Claim through covered authority
  history and returns its exact predecessor, successor, Proposal, applied
  event, and terminal Decision bindings.

Advanced verification and integration:

- `verification import` retains a non-authorizing scoped Verification Record.
- `correction impact` projects what one correction costs the Claims resting on
  it, through the correction-impact derivation in `vela-edge`. The argument is
  the successor — the Claim carrying `corrects` or `supersedes` — so the
  question can be asked of a correction still in the review queue as readily as
  of one already ruled on. It reads the two relation kinds that carry
  consequence (`depends` as `depends_on`, and `supports`) and reports every
  relation it excluded. See [Corrections](#corrections).

Advanced setup:

- `authority` manages independently distributed repository-authority trust
  roots through its single subcommand, `authority trust pin`. A consumer runs
  it at install time after obtaining the sequence-one authority-record root
  through a separate channel; it grants no authority and changes no repository
  byte. See [Repository setup](#repository-setup). Writer initialization is
  `vela init`, not `authority`.

Repository-owned domain adapters generate the optional tracked `targets.json`
catalogue. `replay`, `next`, and `start` validate it; Vela has no separate
Target Index maintenance command.

`vela help advanced` is the executable source for this grouping.

## Reading what a repository holds

`vela claims` pages the claim index in the verified repository manifest. It is
the only verb that produces Claim IDs; `show` and `why` consume them. Without
it the Claim surface is reachable only by ID, and the rest of the read surface
produces few: `review list` reaches the Claim of each retained Proposal, which
on a repository whose origin admitted a large initial object set is a small
fraction of what it holds.

```bash
vela claims                        # accepted Claims, first page
vela claims --status all --json    # accepted and unassessed together
vela claims --cursor <full_vcl_id> # resume after the last row of the last page
```

`--status` takes the Claim standings the rows report, `accepted` and
`unassessed`, or `all`. It defaults to `accepted`. A Claim the manifest holds
pending is `unassessed`: no ruling stands over it.

Rows answer the standing axis only. The Proposal axis is read from retained
Proposals, which this verb does not open — a Claim bound by the origin's
initial object set has no Proposal at all, so a row has nothing to report
there. `vela why` and `vela show` read both and return `standing` beside
`proposal_status`; `vela review list --status` filters the Proposal axis and
keeps the Proposal vocabulary. Every `review` view names that axis `status`,
including `review show`, which called it `standing` through `0.966.3`.

It takes `--limit`, `--cursor`, `--status`, and `--json`, and pages through the
same rule as `review list` — one implementation, not two: the cursor is the
last returned row's own ID, never an offset, `--limit` is clamped to 100, and a
cursor naming no row is refused rather than silently restarting at page one.

Rows come out in Claim ID order, which is the manifest's own order, so a cursor
names the same boundary on every call. Each row carries the Claim ID, its
one-line assertion and kind, its Standing, and its origin era:

- `origin` — the repository's origin already bound this Claim in its initial
  object set, so no Proposal or Decision in this repository admitted it.
- `post_origin` — repository authority admitted it after the origin.

`--json` returns `vela.claims.v1`. `total` is the number of indexed Claims
matching `--status`; `origin_claims` is the size of the origin set. The two do
not subtract: a repository can have retired an origin Claim, which is why
`quantum-codes-frontier` reports 5 accepted Claims over an origin that bound 6.

Retained bytes are read only for the rows a page returns, so `total` is an
index count and never a claim about bytes that were not read. A row whose
bytes cannot be read at their declared root comes back as itself with
`readable: false` and a reason, and is counted in `unreadable_returned` — the
row is neither dropped from the page nor allowed to fail it, so a partial page
is never presented as a whole one.

The subject is the claim index, which is where the repository binds Standing.
The Claim of a Proposal that was rejected, withdrawn, or is still pending is
retained under `records/claims/` but holds no Standing and is not repository
state; read those through `vela review list --status all` and
`vela review show`.

## Target briefing and Submissions

`vela next` returns a ranked Target Offer. `queue_position` is the Target's
current place among open work; `rank` is its stable configured priority and may
begin above one after earlier Targets close. Review work never enters that
producer queue.

`vela start` revalidates the selected Target and returns:

- the exact target and packet;
- the repository origin and root;
- the Target Index root;
- the source Git identity, explicitly labeled `target_index_source` so it is
  not confused with the current `repository_head` reported by `status`;
- the repository scope and declared verifier profile; and
- the explicit boundary that evidence may enter review but only a human
  Decision changes Standing.

It writes no file, lease, run record, counter, budget, Event, or canonical object
and reads no authority key.

The default remains one short command:

```bash
vela start <target> --json
```

Vela does not select, launch, wrap, meter, or schedule a runner. The producer
uses its native agent, workbench, notebook, proof assistant, or laboratory
system. Harbor owns benchmark execution. Those systems may retain their own
run or attempt identities as ordinary provenance, but Vela does not create or
authorize them.

When a producer supersedes or abandons its own still-pending Proposal, it can
remove that item from human review without invoking repository authority:

```bash
vela review withdraw . <vpr_id> \
  --as agent:<name> \
  --reason "Superseded by a corrected Submission." \
  --json
```

The command requires the exact key that signed the retained Submission. It
appends a lifecycle record, creates no scientific Event, and cannot change
accepted Standing.

`vela status` is the compact repository summary. Its `decision_inbox` projection
reports pending, ready, and blocked consequence counts plus rooted projection
identities. The suggested next action may inspect the Inbox or select and
brief the next Target; it never accepts or rejects Standing.

`vela review inbox --json` returns `vela.decision-inbox.v2`. Each rooted entry
contains one explicit `standing_delta`: the affected Claim IDs, accepted
Standing now, accepted Standing under accept or reject, the corresponding
repository roots, and counts of unchanged and global accepted Claims. These
are deterministic previews of the existing Decision semantics, not retained
objects or recommendations. Derivation fails closed if a hypothetical
Decision changes accepted Standing outside the declared Claim scope.

`vela submit` accepts either explicit flags or a portable Submission file. For
a file import, each declared Artifact travels beside the Submission at
`artifacts/sha256/<digest>`. Vela verifies those bytes before installing the
canonical content-addressed objects in the repository transaction.

Corrective Submissions bind the full historical Claim identity and exact Claim
root:

```bash
vela submit --repo . \
  --claim "<replacement bounded claim>" \
  --type theoretical \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --supersedes <full_vcl_id> \
  --target-root <full_sha256_root> \
  --as agent:<name> \
  --json
```

An observed correction or supersession does not need a synthetic work target.
New Claims are signed and submitted directly. Submission creates a pending
Proposal and cannot decide it.

## Verification

```bash
vela verification record . <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --independent-of agent:<producer> \
  --as verifier:independent-check \
  --json
```

A Verification Record binds the Submission, Proposal, Claim, Artifacts,
method-manifest bytes, environment, scope, outcome, and verifier identity. The
command resolves the exact current Proposal package, signs the scoped record,
and retains it atomically. It changes no Standing.

The method manifest must be repository-relative, tracked, clean, and retained in
the current Git commit before this command runs. If it is new or changed,
commit its exact bytes first, then rerun the same command.

The ordinary one-requirement case needs no copied property string: Vela uses
the Submission's sole exact verification requirement. With multiple
requirements, pass one exact requirement through `--property`. Use
`--property ... --complementary` only for an observation that does not satisfy
the registered gate.

`vela verification import . verification.json --as verifier:<name> --json`
remains the interoperability and clean-clone path for an already signed record.
The exact Proposal, Submission, Claim, Artifacts, method, and verifier signature
supply the durable lineage binding. Verifiers do not inherit the producer
identity or any Decision capability.

The protocol permits an acceptance action only when the current Proposal and
Submission still match and every declared verification requirement has a valid
independent passing Verification Record. A failure blocks. Missing, invalid,
dependent, or inconclusive records do not count. Passing this gate is not a
recommendation and does not satisfy an unregistered scientific, product-value,
or external-independence test.

## Decisions

Inspect before acting:

```bash
vela review inbox . --json
vela review list . --json
vela review show . <vpr_id> --json
```

`review inbox` derives a consequence-only queue from exact current repository
objects. Each entry binds the Proposal, Claim, Submission, Verification set,
policy, authority heads, hypothetical accept/reject repository roots, limits,
blockers, and one deterministic entry root. It writes nothing and cannot
accept or reject. It classifies exact-target Verification Records as
requirement-satisfying, complementary, or blocking using the same predicate as
the protocol gate. Complementary evidence stays visible and root-bound but
does not silently satisfy the registered requirement. `review list` remains
the compact record queue; `review show` remains the complete source packet.
Entries whose protocol checks are satisfied appear before blocked cleanup;
each group remains oldest-first. `status` reports `protocol_ready_count` and
`protocol_blocked_count`; neither field is a scientific recommendation.

An authorized human performs one semantic action:

```bash
vela review reject . <vpr_id> \
  --if-entry-root sha256:<entry-root-from-review-show> \
  --reason "The retained evidence does not satisfy the stated conditions." \
  --json
```

or, when eligible:

```bash
vela review accept . <vpr_id> \
  --if-entry-root sha256:<entry-root-from-review-show> \
  --reason "The exact claim, evidence, verification, and conditions support acceptance." \
  --json
```

`--if-entry-root` is an optional compare-and-swap guard over the exact Decision
Inbox packet the human inspected. Observatory and automated review surfaces
should always pass it. If the Proposal, evidence, policy, authority head, or
Standing changed, Vela refuses the command before requesting an authority
signature. A person working entirely in one terminal may omit it; the Decision
transaction still re-prepares and fail-closes against the current repository.

The action, reason, principal, Proposal, policy, authority head, read set, and
canonical delta are covered by one repository-authority transaction. There is
no batch mode, copied confirmation root, custom signer, or Vela-managed human
key. Load the dedicated authority key once for the current operating-system
session (`ssh-add --apple-use-keychain` on macOS or `ssh-add -t 8h` on Linux).
Vela does not require OpenSSH's per-signature `-c` confirmation.

A trusted native agent session may execute a Decision that the operator
explicitly authorized. The scope is the named Decision or campaign; each
transaction still requires its own current Inbox root, reason, policy Allow,
read-set recheck, and authority record. Never forward `SSH_AUTH_SOCK` to
remote, untrusted, or proposal-supplied code.

## Corrections

Accepting a Claim that carries `corrects` or `supersedes` retires its
predecessor. `vela correction impact` reports what that costs everything else
the repository holds.

```bash
vela correction impact vcl_6fa1… --json
```

The argument is the **successor** — the Claim carrying the correction — not the
Claim being corrected. Naming it that way is what lets the question be asked
before the Decision: a correction sitting in `vela review inbox` can be asked
its cost, which is the question a repository authority actually has. Ruling on
it does not move the answer; the projection is over the transition and the two
Claim roots, and none of those change when a Decision is recorded.

The projection reads the two relation kinds that carry consequence. `depends`
is a hard dependency — correcting its target puts the source under a repair
obligation. `supports` is a support route — a source that loses every route it
had is under a repair obligation too, and one that loses some but not all is
reported as `route_changed`. Every other relation kind is retained description
that moves no Standing, and the verb reports each one it excluded by kind and
count rather than dropping it silently.

A repair obligation needs a discharge condition. A Claim Record can declare one
under the `vela.correction` extension:

```json
{ "extensions": { "vela.correction": { "repair_condition": "Re-run the search at the corrected bound." } } }
```

Where none is declared the protocol's own default applies, and the verb reports
per obligation which of the two it used.

**Known limitation.** The write path authors correction relations only.
`depends` and `supports` claim-to-claim edges exist in retained epoch-1 records
but no current verb writes one, so a repository built with today's CLI has no
edge for this projection to traverse and correctly reports an empty cascade.
Closing that is a change to the signed Submission schema and is not made here.

## Repository setup

Create a signed, replayable repository:

```bash
vela init ./my-repository \
  --name "Bounded question" \
  --scope "Does X hold?"
```

Initialization writes Repository Profile v1, binds one Ed25519 identity from the
normal
OpenSSH agent, installs the repository origin and local trust anchor, and
commits the verified initial state. If signing fails, the Profile is retained;
load the key and rerun the same `vela init` command. Use `--key` when the agent
contains more than one Ed25519 identity.

Independent consumers install the returned sequence-one authority-record root after
obtaining it through a separate channel:

```bash
vela authority trust pin . --record-root sha256:... --json
```

The pin is local public trust configuration. It reads no key, grants no
authority, and changes no repository byte.

Pinning the already installed root is idempotent. After independently
verifying a repository-origin transition, advance an existing pin only by
supplying its exact current root:

```bash
vela authority trust pin . \
  --record-root sha256:<new-sequence-1-root> \
  --previous-record-root sha256:<exact-installed-root> \
  --json
```

Vela compares the installed preimage atomically and verifies the new root
against the current sequence-one authority record.

## Repository verification

```bash
vela replay <repo> --json
```

The command verifies the current manifest, native-genesis or signed-predecessor
origin, retained authority chain, exact object roots, and rejection of retired
active paths. The one-time migration writer is not part of the current binary.

## Machine contracts

- `status --json` returns compact identity, full roots, replay, blocker counts,
  object counts, readiness, and one next action.
- `claims --json` returns one page of the repository claim index, with the
  Standing and origin era of each row and an explicit count of rows whose
  retained bytes could not be read.
- `next --json` returns ranked producer Targets only.
- `start --json` returns one exact write-free Target briefing.
- `review list --json` returns compact Proposal summaries.
- `review inbox --json` returns rooted consequence-only decision summaries
  with an explicit target-scoped Standing delta.
- `review show --json` returns one pending Proposal or terminal Decision.

Default JSON does not embed full packet bodies, review collections, private
coordination, test telemetry, or secret material.

Every `--json` outcome, success or failure, is one object carrying `ok`,
`command`, and a versioned `schema`. A failure is `vela.error.v1` and carries an
`error` with a `kind`, a `code`, a message, and a hint naming the next command.

The failure fields have different contracts, and mixing them up is
how a caller ends up parsing English:

- `kind` is one of six classes and chooses the exit code. It is stable.
- `code` names *which* failure this is, when the binary knows something the
  class does not — that the file was oversized rather than absent, that the
  repository has an unfinished `vela init` rather than a missing file. It is
  stable, it is `null` when the class is the whole story, and the key is always
  present. Adding a code is additive; renaming one is a breaking change.
- The message and the hint are written for a person and **will be reworded**.
  Do not branch on them. `crates/vela-cli/tests/wording_contract.rs` says so as
  a test rather than as a promise.

The exit code says which kind of failure it was, so a caller can act on it
without parsing prose:

| Code | Kind | Means |
| --- | --- | --- |
| 0 | — | the command did what it says |
| 1 | `domain` | the domain said no: replay broken, a gate red, a verification failed |
| 2 | `usage` | the invocation was wrong: a missing or malformed argument |
| 3 | `not_found` | the object named does not exist in this repository |
| 4 | `custody_refused` | the custody engine or a permission profile refused |
| 5 | `already_exists` | an idempotent no-op; the thing was already there |

A failure whose cause is genuinely ambiguous reports `domain`, because a
confident wrong code is worse than an honest general one.

## Fail-closed behavior

Checks fail closed on defects. Diagnostic output never grants trust,
signature validity, authority, or historical exemption.

Canonical writes refuse:

- dirty or drifting inputs;
- missing or mismatched trust roots;
- broken authority continuity;
- stale Proposals, Claims, Targets, or Artifacts;
- insufficient verification;
- ambiguous principal or authorization;
- invalid signatures;
- active or corrupt recovery state; and
- a changed read set before commit.

See [Authority and attribution](SIGNING.md) and [Protocol](PROTOCOL.md).
