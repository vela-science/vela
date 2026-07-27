# Authority and attribution

Vela separates evidence production, verification, authorization, and
scientific standing.

```text
producer evidence
  -> deterministic verification
  -> pending proposal
  -> authorized semantic decision
  -> repository-authority record
  -> append-only standing
```

A signature proves control of a key over exact bytes. It does not prove that a
claim is true. Verifier success does not imply acceptance, and Git publication
does not imply either.

> **Candidate behavior.** Vela `0.930.0-rc.13` implements Proposed
> [ADR 0020](adr/0020-attributed-repository-authority-and-standard-delegation.md).
> Vela `0.915.1` remains available for byte-identical Era-0 replay. New Era-0
> personal-signing and migration writers are retired.

## The current authority model

Every migrated Frontier has:

- an append-only authority history;
- a full-root authority keyset;
- a retained, restricted Cedar policy bundle;
- attributed principals and scoped capabilities;
- one DSSE authority record covering each Era-1 transaction; and
- independently installed public trust anchors for the repository boundary
  and the sequence-1 repository-authority record.

The repository authority is a service identity, not the scientific reviewer.
Its signature attests that the recorded authentication, authorization,
semantic action, read-set recheck, and canonical write all matched. The
authority record separately attributes the human, agent, or workload principal
responsible for the action.

The initial local provider is the standard OpenSSH agent. Vela asks the agent
to sign the exact authority record with the key selected by the Frontier
keyset. Vela does not create, store, reveal, or recover a human signing seed.

## Human decisions

Inspect one pending proposal:

```bash
vela review show . <vpr_id> --json
```

Then execute exactly one semantic action:

```bash
vela review reject . <vpr_id> \
  --reason "The retained evidence does not satisfy the stated acceptance conditions." \
  --json
```

or, when the Review Packet says acceptance is eligible:

```bash
vela review accept . <vpr_id> \
  --reason "The exact claim, evidence, verifier record, and conditions support acceptance." \
  --json
```

The command itself is the human semantic action. Vela:

1. derives the exact Review Packet and transaction plan;
2. binds the proposal, action, reason, principal, policy, authority head,
   binary identity, read set, and canonical delta;
3. authenticates the current local operating-system principal;
4. evaluates the retained Cedar policy;
5. rechecks the transaction inputs;
6. asks the OpenSSH agent to sign the covering authority record; and
7. installs the transaction through the recoverable Frontier journal.

There is no copied root or timestamp, custom helper, Vela human key, approval
session, batch mode, wildcard, `--yes`, or persistent semantic approval.

If eligibility, policy, key selection, read-set verification, or repository
signing fails, the transaction does not cross the commit marker. Rejection
changes proposal standing but not accepted scientific state. Acceptance writes
the exact scientific event and explicit `review.accepted` event in one covered
transaction.

Agents may prepare or explain this command. They may not invoke an accepting
or rejecting decision on the human's behalf, access the repository-authority
key, or claim that a proposal became accepted without verifying the resulting
authority record and event history.

## Producer attribution

An optional file-backed agent identity is only for bounded producer work:

```bash
vela id create --agent --handle canopus
vela id show --json
```

The producer key signs exact lease, Submission, and withdrawal records.
It cannot authorize review, acceptance, policy administration, membership,
recovery, or repository-key rotation.

Routine producer work uses already governed policy:

```bash
vela next . --json
vela start <target> --frontier . --as agent:<handle> --json
vela submit submission.json --frontier . --as agent:<handle> --json
```

Current Submission registration retains the exact producer package, issues a
Registration Record, and creates a pending Proposal. It creates no
Verification Record, Decision, Event, or accepted Standing.

Producers may withdraw their own pending Proposal:

```bash
vela proposal withdraw . <vpr_id> \
  --as agent:<handle> \
  --reason "superseded by a corrected submission" \
  --json
```

Withdrawal never deletes evidence or changes accepted Claim Standing.

Fresh `vela init` repositories have structural identity but no configured
repository authority. Establish the standard writer with one dedicated
Ed25519 key already loaded in the normal OpenSSH agent:

```bash
vela authority init . \
  --reason "Establish the repository writer for this bounded Frontier." \
  --json
```

Vela automatically selects the key only when exactly one plain Ed25519
identity is loaded. Otherwise `--key SHA256:<full-fingerprint>` selects it.
The command reads no private-key file and is valid only over the exact
one-event Profile v1 genesis with an empty actor registry and no authority
history. It writes one `authority.initialized` event and one covering
sequence-1 DSSE record, with the initial keyset, Cedar bundle, and exact policy
material. It changes no scientific standing. Established and historical
Frontiers cannot use this path. The JSON result includes the full sequence-1
authority-record root that must be distributed independently of the Frontier
checkout.

## Consumer trust

There are two different trust choices and Vela does not collapse them.

An administrator-bound Frontier uses the ADR 0016 repository-boundary pin:

```bash
vela frontier trust pin . --boundary-root sha256:... --json
```

Every Era-1 Frontier also distributes its full sequence-1 authority-record
root through an independent channel. A consumer installs that public root
directly:

```bash
vela authority trust pin . --record-root sha256:... --json
```

The authority pin is the closed
`vela.authority-trust-anchor.v1 {frontier_id,
first_authority_record_root}` record. The sequence-1 record already binds the
initial keyset, policy authorization, principal, events, and execution claim,
so duplicating those fields in the local anchor would add ambiguity rather
than security.

Both pin commands write only local consumer configuration under the
operating-system account home. They read no key, show no semantic approval,
write no Frontier byte, and grant no scientific or repository authority.

## Historical replay

Era-0 actor registrations, event signatures, policies, and migration
boundaries remain immutable and verifiable. They are not live authoring
surfaces in the current candidate.

Use the historical `v0.915.1` binary when an exact old-command replay is
required. Do not hand-edit old events, registrations, policies, receipts,
proposals, authority records, or derived roots.

## Fail-closed rules

Vela refuses an authority transaction when any of the following is missing,
stale, ambiguous, or invalid:

- the applicable repository-boundary and sequence-1 authority trust anchors,
  or the authority history;
- the exact principal attribution;
- the required scoped capability or Cedar authorization;
- a required semantic human action;
- the Proposal, Submission, Verification Record, or policy binding;
- the transaction read set or canonical delta;
- the selected repository-authority key or signature; or
- journal recovery and marker verification.

Cancellation and preflight failure produce no authority event, proposal
mutation, accepted-state change, or commit marker.
