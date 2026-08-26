# T3 counterfactual branching and metering qualification

Recorded: 2026-08-26, America/Toronto.

```text
Lane: T3 Counterfactual Branching + Metering
Branch: campaign/compose1-counterfactual
Integrated Phase-1 parent: 9eff75e62319b766d33118cea71c1baa65e62d81
Production code changed: no
Protocol objects or schemas changed: none
New state engine, runner, database, or branch command: none
Campaign anomaly: none
Scientific experiments run: 0
```

## Result

`LOCALLY QUALIFIED` for supervisor review.

Git plus the current Vela CLI already supply the required controlled-branch
semantics. The missing apparatus was evidence, not a Core primitive: no single
same-lineage lifecycle froze task/evaluator bytes before the branch point,
proved both branches began from the same governed state, retained explicitly
scoped resource facts, rejected incomplete metering, checked isolation in both
directions, and produced a path-independent deterministic comparison.

The added integration lifecycle supplies that qualification without changing
runtime behavior. It creates one disposable Vela Repository, retains one exact
Submission and scoped passing Verification Record, and branches that identical
pending state. One sibling admits an authorized accept Decision and the other
admits an authorized reject Decision. Strict replay derives different terminal
Standing from the two branch-local authority histories.

This is Level-1 internal engineering evidence. It is not a scientific result,
an experiment cell, cumulative-science evidence, external validation, or an
anomaly.

## Existing-apparatus audit

| Requirement | Existing apparatus | T3 disposition |
| --- | --- | --- |
| Branch creation and lineage | Ordinary Git commits, trees, branches, merge-base, clone, and diff | Use directly; no `vela branch` command |
| Governed starting state | `vela replay --json`, Repository/origin roots, authority roots, Git commit/tree, deterministic Standing | Bind all of them at the branch point |
| Divergent state | Existing `review accept|reject` Decision path and strict replay | Exercise sibling Decisions over the same Proposal and Verification |
| Verification boundary | T1-qualified Verification invariance and Decision admission | Retain Verification before the fork; branch-local Decisions alone diverge Standing |
| Receipts | Git-retained source files plus existing canonical authority/Decision records | Add only a campaign-owned metering receipt; no generic receipt object |
| Sealing | Exact Git blobs and SHA-256 over source-owned bytes | Check branch-point, `HEAD`, and working-tree bytes |
| Comparison | Git diff plus stable Vela JSON and RFC 8785 canonicalization already used by the implementation | Build a test-only, source-owned comparison; no public CLI contract |

No reproduced contradiction, second maintained-consumer need, or missing Core
primitive was found. There is therefore no STOP condition to escalate.

## Branch-point identity and lineage

A branch name is not an identity. The fixture binds this exact tuple before
divergence:

```text
Git commit
Git tree
Repository ID
Repository root
origin ID and root
authority keyset root
authority model root
sequence-one authority-record root
test-only accepted-Claim-slice commitment
```

Both sibling checkouts must reproduce the whole tuple, identify the same Git
merge-base, and retain the same task, evaluation, and metering-plan blobs. The
accepted-Claim-slice commitment is only a test oracle, following the T1
clarification: Protocol 1 publishes no standalone `standing_root`.

The fork occurs after Submission and Verification. Both branches therefore
start with zero accepted Claims, one pending Claim, the same Proposal, the same
Verification Record, and the same full Repository root. This is stronger than
merely starting with equivalent prose or independently initialized
Repositories.

## Sealed source-owned inputs

The campaign fixtures are committed before the branch point and copied into
the disposable source Repository:

| Input | Exact file SHA-256 |
| --- | --- |
| `task.json` | `sha256:6e245d0641c1fd39ab2f59cf5373d5fc1ff43d9ff3a33388e1f480dc7cdfe9d2` |
| `evaluation.json` | `sha256:5f202f5a99fbb5b7974b73951a6caedc27b977eb6d21929d27607e9fdd73fb6b` |
| `metering-plan.json` | `sha256:1a9de4b9c253a0d83ffb287fbadc56fd9cfac18985b3765d53a8385f9f266bf6` |

The lifecycle compares each branch-point Git blob with both the terminal
`HEAD` blob and the actual working-tree bytes. Its negative case mutates the
evaluator only in a disposable terminal clone and proves the seal check refuses
the drift.

These `vela-compose-1.*` JSON documents are campaign-owned test inputs. They
are not Protocol 1 objects, portable schemas, authority records, or a proposed
general experiment format.

## Exact workflow and command surface

The qualification composes the shipped grammar rather than adding an API:

```bash
# Establish and inspect the exact common branch point.
vela replay <source-repository> --json
git -C <source-repository> rev-parse HEAD^{commit} HEAD^{tree}

# Create sibling source-owned histories from that exact commit.
git clone <source-repository> <accept-checkout>
git clone <source-repository> <reject-checkout>
git -C <accept-checkout> switch -c counterfactual/accept
git -C <reject-checkout> switch -c counterfactual/reject
git -C <accept-checkout> merge-base HEAD <branch-point-commit>
git -C <reject-checkout> merge-base HEAD <branch-point-commit>

# Read the exact rooted Decision input, then diverge only through authority.
vela review inbox <accept-checkout> --json
vela review accept <accept-checkout> <proposal-id> \
  --if-entry-root <entry-root> --reason <reason> \
  --as agent:<performer> --session-ref <source-session> --json
vela review inbox <reject-checkout> --json
vela review reject <reject-checkout> <proposal-id> \
  --if-entry-root <entry-root> --reason <reason> \
  --as agent:<performer> --session-ref <source-session> --json

# Reconstruct and compare retained terminal histories.
vela replay <accept-checkout> --json
vela replay <reject-checkout> --json
git -C <accept-checkout> diff --name-only --no-renames \
  <branch-point-commit>..HEAD --
git -C <reject-checkout> diff --name-only --no-renames \
  <branch-point-commit>..HEAD --
```

The test-only comparison orders fields and metrics deterministically, omits
checkout paths, canonicalizes the resulting JSON, and hashes the exact bytes.
It repeats comparison in place and across two fresh terminal clones. The bytes
and comparison root must match for the same retained histories and receipts.
The comparison root is informative campaign evidence, not a Vela Repository or
Standing root.

## Metering boundary and semantics

The frozen execution window begins with the branch-local Decision Inbox read
and ends when that branch's authorized Decision returns. Common Submission and
Verification setup, post-Decision replay/comparison, receipt persistence,
campaign authoring, and supervisor review are excluded and named as such.

| Required resource | Fixture status | Qualification boundary |
| --- | --- | --- |
| Model calls | `not_used = 0` | No model call occurs inside the synthetic branch action |
| Input/output tokens | `not_used = 0` | Follows the model-call boundary |
| Tools | `measured = 2 top_level_calls` | One Inbox read and one Decision call; Vela subprocesses are not inferred |
| Verifiers | `not_used = 0` branch-local | One exact Verification is common pre-branch state |
| Solvers/simulations | `not_used = 0` | None invoked |
| Wall time | `measured`, comparison `incomparable` | Sequential execution on one shared host cannot estimate a branch effect |
| CPU time | `unavailable` | The runner exposes no reliable per-branch child-process CPU counter |
| GPU time | `not_used = 0` | No GPU requested |
| External services | `not_used = 0` | Local Git, local binary, local disposable SSH agent; no network |
| Artifacts | measured count and exact canonical bytes | The retained Decision-command JSON |
| Persistent state | measured changed-file count and current bytes | Git-tracked Decision delta before evaluator receipt persistence |
| Human interventions | `not_used = 0` | Synthetic agent performer; author/reviewer work is explicitly out of window |

Every receipt must contain every frozen metric exactly once. A negative case
deletes `cpu_time_ms` from an in-memory receipt and proves comparison refuses
the incomplete inventory. `unavailable`, `incomparable`, and `not_used` remain
distinct; none is coerced to a zero-valued measurement.

## Proof matrix

| Obligation | Direct assertion | Result |
| --- | --- | --- |
| Identical starting governed state/root | Both clones reproduce the complete branch-point tuple, accepted-Standing commitment, one pending Claim, and exact merge-base | PASS |
| Divergent branch-local histories | Same Proposal and Inbox entry root receive sibling authorized accept/reject Decisions; authority-record and terminal Repository roots differ | PASS |
| Verification alone never changes Standing | Common passing Verification is retained before the fork while accepted Standing remains empty | PASS |
| Authorized Decision is the transition boundary | Accept yields one accepted Claim; reject yields zero and an addressable rejected Proposal | PASS |
| No cross-branch contamination | Reject root remains at the base after accept; accept root remains unchanged after reject; opposite result directories remain absent | PASS |
| Sealed task/evaluation inputs | Branch-point, terminal committed, and terminal working bytes match; a mutated evaluator is refused | PASS |
| Honest complete metering | All 15 frozen metrics occur once with typed availability/comparability; missing metrics fail | PASS |
| Deterministic comparison | Repeated in-place and fresh-clone comparisons have identical canonical bytes and SHA-256 root | PASS |
| Replayability | Each terminal history passes strict replay before and after source-owned receipt persistence | PASS |

## Files changed

- `crates/vela-cli/tests/counterfactual_branching.rs` — one test-only complete
  same-lineage lifecycle, metering validator, isolation checks, and
  deterministic comparison.
- `docs/campaigns/vela-compose-1/fixtures/t3/task.json` — sealed synthetic task.
- `docs/campaigns/vela-compose-1/fixtures/t3/evaluation.json` — sealed structural
  evaluator and explicit nonclaims.
- `docs/campaigns/vela-compose-1/fixtures/t3/metering-plan.json` — frozen
  branch-action boundary and required resource inventory.
- `docs/campaigns/vela-compose-1/T3_BRANCHING_REPORT.md` — this report.
- `docs/campaigns/vela-compose-1/README.md` and `docs/README.md` — current
  documentation index links.

## Verification

```text
cargo test --locked -p vela-cli --features test-support \
  --test counterfactual_branching --test portable_divergence \
  --test review_acceptance
  PASS: counterfactual_branching 1, portable_divergence 2,
        review_acceptance 1; 4 passed, 0 failed

cargo clippy --locked -p vela-cli --all-targets \
  --features test-support -- -D warnings
  PASS

cargo fmt --all -- --check
  PASS

jq -eS . docs/campaigns/vela-compose-1/fixtures/t3/{task,evaluation,metering-plan}.json
  PASS: 3/3

documentation index checks
  PASS: root and campaign indexes link this report

git diff --check
  PASS
```

The Protocol 1 interoperability waist did not change, so the broad conformance
runner was not rerun and this lane does not add a normative vector or claim a
new conformance root.

## Limitations

- The fixture is synthetic and local. It does not exercise a model, solver,
  simulation, GPU, external service, scientific vertical, or human Decision.
- The receipt counts only top-level harness calls. It does not infer Vela's
  internal Git subprocess count, filesystem write amplification, energy, or
  monetary cost.
- Persistent-state bytes are the current sizes of tracked files changed by the
  Decision, not I/O bytes written or allocated storage.
- Wall time is retained but intentionally incomparable; CPU time is explicitly
  unavailable. A future source runner must replace those statuses only when it
  can capture reliable branch-scoped measurements.
- Fresh executions create fresh authority keys, timestamps, Git commits, and
  elapsed times. Determinism is therefore asserted for repeated comparison of
  the same sealed terminal histories, not as one frozen root across new runs.
- Sibling authority histories are counterfactual alternatives. Combining both
  sequence-two records would be an authority fork, not a valid Git merge.
  Canonical publication must select a governed history or create a later valid
  transition; this worker does neither.
- The comparator remains test-only and campaign-owned. No second maintained
  consumer has established a public `vela compare`, metering schema, workflow
  runner, or Core extraction.
- No merge, push, publication, release, external contact, or experiment was
  performed.

## Blocked dependencies

None inside T3. T4–T7 remain supervisor-gated and were not started.
