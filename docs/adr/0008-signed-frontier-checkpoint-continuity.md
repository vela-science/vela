# ADR 0008: Signed frontier checkpoint continuity

- Status: Proposed
- Candidate release: Vela `v0.802.0`
- Entry gate: ADR 0006 demonstrates a continuity gap after ADR 0007

## Context

An immutable Git root proves only its own bytes. It cannot reveal a future
correction. A consumer can validate a supplied descendant with Git ancestry and
Vela replay, but an untrusted mirror can present an older valid root or a
non-descendant history.

Vela needs a checkpoint only if the ADR 0006 experiment shows that explicit
root delivery plus Git ancestry and signed events cannot meet the rollback,
fork, and update requirements.

## Proposed decision

Define a detached, signed, non-scientific envelope:

```text
vela.frontier-checkpoint.v1 {
  frontier_id
  sequence
  git_commit
  git_tree
  event_log_root
  event_count
  actor_registry_root
  previous_checkpoint_root
  signer_actor
  issued_at
  signature
}
```

The checkpoint is portable JSON. It may travel beside a Git bundle, in an
archive, or through an untrusted mirror. It does not enter the scientific event
log and does not accept or revise a finding.

## Trust and signatures

- The first checkpoint requires an explicit pinned trust anchor consisting of
  frontier identity and checkpoint signer key fingerprint.
- A later checkpoint names the full root of its predecessor.
- The signer must be authorized by the previous trusted actor registry or by a
  governed registry change in the validated descendant history.
- The signature uses Vela's canonical Ed25519 input and existing actor keys.
- A checkpoint signature asserts publication continuity only. It carries no
  scientific acceptance.
- Models receive no checkpoint or human private key.

This ADR adds no signature algorithm, global registry, blockchain, consensus,
or hosted service.

## Validation

A reader validates:

1. checkpoint schema and full content root;
2. canonical signature input and signer;
3. prior checkpoint root and monotonic sequence;
4. Git commit and tree identity;
5. descendant relationship to the prior trusted commit;
6. exact event-prefix continuity;
7. event-log root and count;
8. actor-registry bytes and root; and
9. signer authorization across any registry rotation.

The reader stores its last trusted checkpoint root. A later presentation of an
older valid checkpoint reports `stale`.

Two valid non-descendant histories report `forked`. Two signed children with
the same parent and sequence report equivocation after the reader sees both.
The protocol cannot promise global detection without checkpoint exchange or
gossip.

Missing delivery reports unknown freshness. A reader never interprets silence
as proof that no correction exists.

## Migration and replay

Old frontiers and binaries replay unchanged. Checkpoints sit outside accepted
state and may be absent. A consumer that elects the checkpoint profile must pin
the first trust anchor explicitly.

Checkpoint deletion affects update availability, not historical scientific
replay. A retained checkpoint with missing Git objects is `unresolvable`.

## Adversarial cases

Conformance covers:

- invalid or unauthorized signature;
- wrong Git commit or tree;
- wrong event root or count;
- registry tampering;
- missing predecessor;
- sequence reuse or rollback;
- non-descendant update;
- event deletion, mutation, reorder, or insertion;
- conflicting signed children;
- shallow history;
- signer rotation without governed continuity;
- checkpoint copied to another frontier; and
- a checkpoint treated as scientific acceptance.

## Conformance

```bash
cargo test -p vela-protocol frontier_checkpoint
cargo test -p vela-edge checkpoint_continuity
cargo test -p vela-cli --test handoff_workflows checkpoint
python3 conformance/verify.py
```

The offline fixture must work from Git bundles and detached checkpoint files.

## Consequences

Consumers can distinguish descendant updates, stale roots, and visible forks
without trusting a host. Scientific authority remains inside signed Vela
events. Checkpoint delivery and monitoring remain replaceable services.
