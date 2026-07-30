# Vela CLI

Vela is a Git-native scientific-state tool. Producers submit evidence,
verifiers report scoped results, authorized Decisions change Standing, and Git
preserves the exact repository history.

## Ordinary workflow

```bash
vela status . --json
vela next . --limit 1 --json
vela start <target> --frontier . --as agent:<name> --json

vela submit --frontier . \
  --attempt <vat_id> \
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
vela check . --strict --json
vela why . <claim_id> --json
```

`submit` retains one authenticated Submission, issues its Registration Record,
and creates a pending Proposal. It does not create a Verification Record,
Decision, Event, or accepted Standing.

## Daily commands

Default help exposes exactly:

```text
init status next start submit show why review check reproduce log doctor
```

| Command | Contract |
| --- | --- |
| `init` | Create a minimal Git-native Frontier from a name and bounded scope. |
| `status` | Report identity, roots, replay, blockers, counts, and one next action. |
| `next` | Return canonically ranked producer Targets. |
| `start` | Start one local bounded Attempt against an exact Target. |
| `submit` | Build or import one authenticated Submission and pending Proposal. |
| `show` | Inspect one exact typed object and its authority effect. |
| `why` | Explain one Claim's Standing from retained roots and history. |
| `review` | List, show, accept, or reject one exact Proposal. |
| `check` | Verify repository structure, roots, replay, authority, and strict signals. |
| `reproduce` | Run retained evidence through its frozen verifier. |
| `log` | Read admitted Event history. |
| `doctor` | Report blockers and one safe repair action. |

## Advanced commands

```text
claim id agents config verification authority target-index repository
```

- `claim` provides current Claim record, standing, evidence, and attribution
  views.
- `why` also resolves a retained superseded Claim through covered authority
  history and returns its exact predecessor, successor, Proposal, applied
  event, and terminal Decision bindings.
- `id` manages optional file-backed producer identities.
- `agents` regenerates agent adapters from `VELA.md`.
- `config` manages closed local and Frontier configuration.
- `verification import` retains a non-authorizing scoped Verification Record.
- `authority` initializes or inspects the repository writer and public trust
  roots.
- `target-index` inspects or seals derived producer Targets.
- `repository verify` verifies the signed current origin and active repository.

`vela help advanced` is the executable source for this grouping.

## Attempts and Submissions

`vela next` returns a ranked Target Offer. Review work never enters that
producer queue.

`vela start` creates a local ignored Attempt bound to:

- the exact target and packet;
- the repository origin and root;
- the Target Index root;
- the Git commit and tree;
- the completion contract;
- the current Vela controller binary and exact runner build root;
- a closed set of routine operations and Artifact classes;
- enforced Submission, Artifact, and retained-byte budgets;
- an `evidence_only` or `pending_review` consequence ceiling; and
- local expiry.

It appends no canonical Event and reads no authority key.

The default remains one short command:

```bash
vela start <target> --as agent:<name> --json
```

An external runner supplies `--runner-build sha256:<digest>`. Target packets
that declare typed outputs contribute those Artifact classes; otherwise the
private fallback is `other`. Repeat `--artifact-class <kind>` once for each
real producer output class, such as `text/plain`, `engine-manifest`, and
`verifier-manifest`. `--max-submissions`, `--max-artifacts`, and
`--max-artifact-bytes` narrow the fixed defaults. A successful Submission
increments the private counters but does not delete the Attempt. Every later
Submission revalidates the exact current Target read set. Expiry or `start
<target> --drop` stops future use without changing retained evidence.

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
New Claims still require an active Attempt. Missing, shortened, stale, or
mismatched targets fail before intake. Registration creates a pending Proposal
and cannot decide it.

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
remains the interoperability path for an already signed record.

Acceptance is eligible only when the current Proposal and Submission still
match and every declared verification requirement has a valid independent
passing Verification Record. A failure blocks. Missing, invalid, dependent,
or inconclusive records do not count.

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
accept or reject. `review list` remains the compact record queue; `review
show` remains the complete source packet.

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

Consumers install the returned sequence-one authority-record root through an
independent channel:

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
vela repository verify <frontier> --json
```

The command verifies the current manifest, native-genesis or signed-predecessor
origin, retained authority chain, exact object roots, and rejection of retired
active paths. The one-time migration writer is not part of the current binary.

## Machine contracts

- `status --json` returns compact identity, full roots, replay, blocker counts,
  object counts, readiness, and one next action.
- `next --json` returns ranked producer Targets only.
- `start --json` returns one exact Attempt contract.
- `review list --json` returns compact Proposal summaries.
- `review inbox --json` returns rooted consequence-only decision summaries.
- `review show --json` returns one pending Review Packet or terminal Decision.

Default JSON does not embed full packet bodies, review collections, private
coordination, test telemetry, or secret material.

## Fail-closed behavior

Non-strict checks may report defects for diagnosis. They never grant trust,
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
