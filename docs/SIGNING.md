# Authority and attribution

Vela separates evidence production, verification, authorization, and
scientific Standing.

```text
producer evidence
  -> scoped verification
  -> pending Proposal
  -> authorized semantic Decision
  -> repository-authority record
  -> Standing
```

A signature proves control of a key over exact bytes. It does not prove a
Claim true. Verification does not imply acceptance, and Git publication
implies neither.

## Repository authority

Every current Frontier has:

- an append-only repository-authority history;
- a full-root Ed25519 authority keyset;
- a retained restricted Cedar authorization bundle;
- attributed principals and scoped capabilities;
- one DSSE authority record covering each canonical transaction; and
- an independently distributed sequence-one authority-record root.

Repository authority is a service identity, not the scientific reviewer. Its
signature attests that the principal, authorization, semantic action, read-set
recheck, and canonical write matched. The record attributes the human, agent,
or workload responsible for the action.

The initial local provider is the normal OpenSSH agent. Vela asks it to sign
the exact authority record with the key selected by the Frontier keyset. Vela
does not create, store, reveal, or recover a human signing seed.

## Human Decisions

Inspect one pending Proposal:

```bash
vela review show . <vpr_id> --json
```

Then perform exactly one action:

```bash
vela review reject . <vpr_id> \
  --reason "The retained evidence does not satisfy the stated conditions." \
  --json
```

or, when the Review Packet says acceptance is eligible:

```bash
vela review accept . <vpr_id> \
  --reason "The exact claim, evidence, verification, and conditions support acceptance." \
  --json
```

The command is the semantic action. Vela:

1. derives the exact Review Packet and transaction plan;
2. binds the Proposal, action, reason, principal, policy, authority head,
   binary identity, read set, and canonical delta;
3. authenticates the local operating-system principal;
4. evaluates the retained policy;
5. rechecks every transaction input;
6. asks the OpenSSH agent to sign the covering authority record; and
7. installs the transaction through the recoverable journal.

There is no copied root or timestamp, custom signer helper, Vela human key,
approval session, batch mode, wildcard, `--yes`, or persistent semantic
approval.

An agent may prepare or explain this command. It may not invoke acceptance or
rejection on the human's behalf, access the authority key, or infer a Decision
from a prompt or signature request.

## Producer identity

An optional file-backed agent identity authenticates bounded producer work:

```bash
vela id create --agent --handle canopus
vela id show --json
```

It may sign an Attempt or Submission. It cannot authorize review, acceptance,
policy administration, recovery, membership, or repository-key changes.

```bash
vela next . --json
vela start <target> --frontier . --as agent:<handle> --json
vela submit submission.json --frontier . --as agent:<handle> --json
```

Submission intake creates no Verification Record, Decision, Event, or accepted
Standing.

## Initialize a new Frontier

Load one dedicated Ed25519 repository-authority identity into the normal
OpenSSH agent, then run:

```bash
vela authority init . \
  --reason "Establish the repository writer for this bounded Frontier." \
  --json
```

Vela automatically selects the key only when exactly one plain Ed25519
identity is loaded. Otherwise, select the full OpenSSH fingerprint with
`--key SHA256:<fingerprint>`.

Initialization is valid only for an untouched structural Frontier with no
authority history. It writes the initial keyset, Cedar bundle, exact policy
material, initialization Event, and covering sequence-one DSSE record. It
changes no scientific Standing.

Distribute the returned full sequence-one authority-record root independently
of the Frontier checkout.

## Consumer trust

Install the independently obtained public root:

```bash
vela authority trust pin . --record-root sha256:... --json
```

The anchor is a closed record binding the Frontier and first authority-record
root. That record already commits to the initial keyset, policy, principal,
Events, and execution claim.

Pinning writes only local consumer configuration. It reads no private key,
grants no authority, and changes no Frontier byte.

After a separately verified repository-epoch transition establishes a new
sequence-one authority record for the same Frontier, advance the local pin
with an exact compare-and-swap:

```bash
vela authority trust pin . \
  --record-root sha256:<new-sequence-1-root> \
  --previous-record-root sha256:<exact-installed-root> \
  --json
```

The new root must match the current repository's sequence-one record, and the
previous root must match the installed local pin. Repeating an already applied
pin is idempotent. This operation still reads no authority key, grants no
authority, and changes no Frontier byte.

## Failure behavior

Vela refuses a repository-authority transaction when any required input is
missing, stale, ambiguous, or invalid, including:

- the sequence-one trust anchor or authority history;
- exact principal attribution;
- required scoped authorization or human action;
- the Proposal, Submission, Verification Record, Claim, or Artifact binding;
- the transaction read set or canonical delta;
- the selected repository-authority key or signature; or
- recovery-journal and commit-marker integrity.

Preflight, authentication, authorization, signing, or cancellation failure
creates no committed authority record, Event, Proposal mutation, or Standing
change.

## Historical verification

Predecessor tags and source archives preserve earlier repository contracts.
Use their pinned binaries to verify their bytes. The current binary does not
keep historical writer commands, personal key products, or compatibility
aliases alive.
