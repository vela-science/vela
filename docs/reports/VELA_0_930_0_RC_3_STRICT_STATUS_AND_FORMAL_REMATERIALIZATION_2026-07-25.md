# Vela 0.930.0-rc.3 strict-status and Formal rematerialization

- Date: 2026-07-25
- Status: candidate qualification complete; active migration not started
- Released baseline: Vela `v0.915.1`
- Candidate version: `0.930.0-rc.3`
- Protocol effect: none
- Scientific-state effect: none
- Authority effect: none

## Decision

Keep `v0.915.1` as the released ecosystem component. Use
`v0.930.0-rc.3` as the next repository-authority migration candidate.

The candidate repairs two implementation defects found before the first
active migration:

1. compact `vela status` could report `strict: pass` while canonical
   `vela check --strict` rejected stale derived state; and
2. a completed no-event maintenance journal could collide with a later
   legitimate rematerialization because its operation identity omitted the
   requested derived delta.

The repair changes no protocol schema, event, authority record, proposal,
Receipt, actor record, evidence object, or accepted scientific state.

## Reproduced strict-status defect

The exact Formal Conjectures Frontier was inspected at:

```text
repository: /Users/williamblair/personal/formal-conjectures-frontier
commit:     478f8932699efcebde85f55c9b8b1a826eba1250
branch:     main
```

The development candidate initially produced contradictory read results:

```text
vela status . --json          -> strict: pass
vela check . --strict --json  -> state_integrity failed: 2
```

The two failures were derived-state mismatches:

- a stale legacy snapshot root retained in `vela.lock`; and
- a stale `frontier.json` relative to the exact Profile v1 projection.

This was a false strict pass. Migration qualification stopped before any
preview, signer access, authentication, or write.

Compact status now derives its strict result from the canonical strict-check
payload. A failing check:

- sets `ok` to false;
- reports `strict: blocked`;
- aggregates the failed check categories into blocker counts; and
- gives a repair-oriented next action.

The regression fixture intentionally retains stale derived state and now
requires status to fail closed.

## Completed-journal identity repair

Formal retained a valid ignored completed journal for an earlier
materialization. Its operation identity was derived from Frontier and event
state only. A later renderer requested a different legitimate derived delta
against the same canonical event state and collided with the completed plan.

No completed journal was deleted to bypass the recovery barrier.

No-event maintenance operations now bind:

```text
frontier id
+ request root
+ canonical delta root
+ fixed planning time
```

Active recovery and completed transaction verification retain their existing
fail-closed behavior. The change only prevents distinct maintenance plans
from sharing an operation identity.

## Byte-preserving Formal repair

The sanctioned repair used only:

```bash
vela frontier materialize .
```

It rematerialized derived Profile views and committed them through ordinary
Git history. The resulting exact Frontier is:

```text
commit:                8be46caa082c63374d1b208ccbd84c1f9c351a04
tree:                  81bdd3d7f91e5d51d2c6b80614ed4d59b6ec94fa
event count:           35
event-log root:        sha256:b9df87525e7f4313eedeb0b65ba29b21009e04e404aa25bcb5e29bfc9cd6d3f7
scientific-state root: sha256:4924adbbea6dfe288d14af03cf3d544f73c511df6b6ef8b938c8291685101444
legacy snapshot root:  sha256:02a1cedd97356943f02d68f241fc3f93c7acf52bcd8d8a7914c2fb417facacee
proposal root:         sha256:ba47ddf5c16ed567ddf835385066e3fc294b447bc0eabd3f9820f5e707efb39e
actor-registry root:   sha256:f52d59b1db885f467c66a29335ada68544a09da5f3869723461100eed0aac79e
artifact root:         sha256:fbd7e05b185cd06bc06484e8b0216c17c5263a71d8481ca38e574e9b2c5156d8
```

A manifest over all 76 canonical event, proposal, Receipt, actor, and evidence
files remained byte-identical before and after both materializations:

```text
sha256:2a7cd5c2be65b27be812e6cb7455f008a8228fb77c44a363aad663add1aa5241
```

The first repair commit was
`c45d4dabf80cded234a0bf29423c0542fce32753`; the final candidate-derived
views are at `8be46caa082c63374d1b208ccbd84c1f9c351a04`. Both are on published
`main`. No event or scientific object was rewritten.

## Exact current result

The candidate binary is:

```text
vela 0.930.0-rc.3
sha256:28ad73839d30d586669464187666b6d144102359fde21969212b9788194e402a
```

Against the exact Formal commit above:

```text
vela status . --json          -> ok: true, strict: pass, blockers: 0
vela check . --strict --json  -> ok: true, errors: 0, warnings: 0
```

Status reports 14 valid findings, 35 events, one available work item, and one
pending proposal. Policy remains absent and human-only. That is not a policy
or authority migration.

## Migration preview blocker

No exact migration preview was produced.

The standard OpenSSH agent currently exposes no identity:

```text
ssh-add -L
The agent has no identities.
```

The existing GitHub variable `VELA_REPOSITORY_TRUST_ANCHOR_B64` decodes to
the retained Profile v1 repository trust anchor. Its public key belongs to the
existing human administrator identity and authenticates the prior repository
boundary. It is not a dedicated Era-1 repository-authority service identity
and must not be silently reused as one.

The next gate therefore requires one human/operator action outside Codex:

1. provision a dedicated Ed25519 repository-authority identity;
2. load it into the standard OpenSSH agent; and
3. provide only its stable key ID and raw 32-byte public key as 64 lowercase
   hexadecimal characters.

Only then can Codex run the key-free, write-free preview. Apply, authentication,
signing, and semantic approval remain outside the agent boundary.

## Verification

Focused checks:

```bash
cargo test -p vela-cli compact_status --locked
cargo test -p vela-cli frontier_txn --locked
cargo test -p vela-cli --test task_first_workflows \
  decision_brief_read_surfaces_share_the_same_review_contract --locked
cargo clippy -p vela-cli --all-targets -- -D warnings
python3 conformance/verify.py
```

Integration checks:

```bash
./scripts/full-conformance.sh --suite core --mode=ci
./scripts/full-conformance.sh --suite frontier --mode=ci
```

The core suite passes with 30 gates passed, zero warnings, and zero failures.
The Frontier suite passes with nine gates passed, one pre-existing
external-custody/reconciliation warning, and zero failures.

## Authority and Atlas boundary

This qualification preserves the active Atlas/Tapestry decision:

- a Frontier remains the only authority unit;
- Vela remains the only standing and authority boundary;
- publication facts, Tapestry relations, lenses, Neon, and Observatory views
  remain disposable read projections; and
- no Atlas service, relay, second projector, global ontology, or public writer
  is introduced while the authority migration gate is open.
