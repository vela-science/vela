# Vela CLI

Vela is a Git-native scientific-state tool. Producers submit evidence,
verifiers report scoped results, only an authorized human Decision changes
Standing, and Git preserves the exact repository history.

## Ordinary workflow

```bash
vela status . --json
vela next . --limit 1 --json

vela submit --frontier . \
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

vela review show . <vpr_id> --json
vela check . --json
vela why . <claim_id> --json
```

`submit` retains one authenticated Submission and creates a pending Proposal.
It does not create a Verification Record,
Decision, Event, or accepted Standing.

## Daily commands

Default help exposes exactly:

```text
init status next start submit show why review check reproduce log doctor
```

| Command | Contract |
| --- | --- |
| `init` | Create a minimal Git-native Frontier from a name and bounded scope. |
| `status` | Report identity, replay, Decision Inbox readiness, and one safe next action. |
| `next` | Return canonically ranked producer Targets. |
| `start` | Print a write-free briefing for one exact current Target. |
| `submit` | Build or import one authenticated Submission and pending Proposal. |
| `show` | Inspect one exact typed object and its authority effect. |
| `why` | Explain one Claim's Standing from retained roots and history. |
| `review` | List, show, accept, or reject one exact Proposal. |
| `check` | Verify repository structure, roots, replay, and authority. |
| `reproduce` | Run retained evidence through its frozen verifier. |
| `log` | Read admitted Event history. |
| `doctor` | Report blockers and one safe repair action. |

## Advanced commands

```text
verification authority
```

- `why` also resolves a retained superseded Claim through covered authority
  history and returns its exact predecessor, successor, Proposal, applied
  event, and terminal Decision bindings.
- `verification import` retains a non-authorizing scoped Verification Record.
- `authority` initializes the repository writer for a fresh Frontier. It is an
  exceptional setup surface rather than an ordinary workflow.

Frontier-owned domain adapters generate the optional tracked `targets.json`
catalogue. `check`, `next`, and `start` validate it; Vela has no separate
Target Index maintenance command.

`vela help advanced` is the executable source for this grouping.

## Target briefing and Submissions

`vela next` returns a ranked Target Offer. Review work never enters that
producer queue.

`vela start` revalidates the selected Target and returns:

- the exact target and packet;
- the repository origin and root;
- the Target Index root;
- the Git commit and tree;
- the completion contract;
- the producer identity used in the direct Submission template; and
- the explicit boundary that evidence may enter review but only a human
  Decision changes Standing.

It writes no file, lease, Attempt, counter, budget, Event, or canonical object
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

`vela status` is the compact Frontier summary. Its `decision_inbox` projection
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
vela submit --frontier . \
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
  --property "Replay the exact retained artifact." \
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
  --reason "The retained evidence does not satisfy the stated conditions." \
  --json
```

or, when eligible:

```bash
vela review accept . <vpr_id> \
  --reason "The exact claim, evidence, verification, and conditions support acceptance." \
  --json
```

The action, reason, principal, Proposal, policy, authority head, read set, and
canonical delta are covered by one repository-authority transaction. There is
no batch mode, copied confirmation root, custom signer, or Vela-managed human
key.

Agents may prepare or explain the command. They may not invoke it on a human's
behalf or access repository-authority credentials.

## Repository setup

Create a structural Frontier:

```bash
vela init ./my-frontier \
  --name "Bounded question" \
  --scope "Does X hold?"
```

Initialization writes Profile v2 and scaffolding only. `status` and `doctor`
report `authority_uninitialized`; strict repository verification remains
blocked. To establish the repository writer, load one dedicated Ed25519
identity in the normal OpenSSH agent and run:

```bash
vela authority init ./my-frontier \
  --reason "Establish the repository writer for this bounded Frontier." \
  --json
```

Initialization installs the creator's matching local trust anchor. Independent
consumers install the returned sequence-one authority-record root after
obtaining it through a separate channel:

```bash
vela authority trust pin . --record-root sha256:... --json
```

The pin is local public trust configuration. It reads no key, grants no
authority, and changes no Frontier byte.

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
vela check <frontier> --json
```

The command verifies the current manifest, native-genesis or signed-predecessor
origin, retained authority chain, exact object roots, and rejection of retired
active paths. The one-time migration writer is not part of the current binary.

## Machine contracts

- `status --json` returns compact identity, full roots, replay, blocker counts,
  object counts, readiness, and one next action.
- `next --json` returns ranked producer Targets only.
- `start --json` returns one exact write-free Target briefing.
- `review list --json` returns compact Proposal summaries.
- `review inbox --json` returns rooted consequence-only decision summaries
  with an explicit target-scoped Standing delta.
- `review show --json` returns one pending Review Packet or terminal Decision.

Default JSON does not embed full packet bodies, review collections, private
coordination, test telemetry, or secret material.

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
