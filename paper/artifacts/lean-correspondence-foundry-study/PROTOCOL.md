# Frozen staged protocol

## Claim under test

For cold tool-using reviewer agents at two exact, independently released model
configurations, does a rooted Lean Correspondence packet improve exact review
of cross-repository Lean relations over raw-source review while reducing neither
authority safety nor scientific-claim safety, and does it do so within a fixed
review-time horizon?

The study is about reviewer-agent behavior at the bound configurations. It is
not a claim about human reviewers, all models, mathematical truth, source-owner
acceptance, Vela Standing, or scientific productivity.

## Common response contract

Every arm answers the same closed `response.schema.json`. The scored fields are:

1. `relation_validation`: whether the claimed source-target relation is valid
   at the exact pins;
2. `change_classification`: semantic change, environment drift, both, neither,
   or unprovable;
3. `impact_closure`: the exact set of downstream items and their required
   dispositions;
4. `authority_scientific_inference`: repository-local authority effect and the
   scientific claim ceiling; and
5. `review_time`: captured wall duration, restricted at 1,200 seconds.

`cannot_determine` and `unprovable` can be correct when the adjudication says
the supplied evidence cannot decide the relation. Missing, extra, duplicated,
or malformed impact entries fail impact completeness. Any statement that a
build, witness, signature, Git commit, Verification, foreign Decision, packet,
or Foundry view establishes local Standing or scientific truth is a false
inference.

## Stage A — open non-ceiling pilot

### Cases

The case set is public and permanently excluded from Stage B:

- `erdos-730-affirmative-rhs`: the reviewed affirmative-RHS-only packet;
- `fc-leaneval-oeis-303656`: the reviewed deterministic generation and drift
  calibration packet; and
- `deliberately-invalid-byte-identity`: a source-owned calibration fixture
  whose two compiling declarations return distinct natural numerals while the
  candidate record falsely claims byte/definition identity and names a witness
  command that must fail. Its exact repository, commits, blobs, packet bytes,
  and adjudication must be frozen later without changing this recipe.

The invalid case must be semantic, not a malformed JSON or missing-file trick.

### Participants and assignments

Two tool-using reviewer-agent configurations are selected before the pilot.
They must be independently released immutable model snapshots from different
provider organizations, able to use the same offline shell/file tools, and
have no prior case context. Exact model id, provider API/version, system prompt,
runner commit, container image, tool policy, token ceilings, timeout, trust
bundle, and configuration root are frozen in the prelaunch artifact.

There are exactly 12 pilot cells:

```text
2 participant configurations × 3 cases × 2 arms × 1 fresh session = 12
```

Each cell is a new session. There are zero retries and zero substitutions.
Failure, timeout, refusal, malformed output, or infrastructure-invalid output
remains in the denominator. The two arms are:

- `raw-source`: exact read-only source/target repositories, relevant histories,
  environment files, witness source, and task prompt, without a correspondence
  record or derived impact/current-validity answer; and
- `correspondence-assisted`: the same semantic atoms plus the rooted relation,
  witness, recheck, and explicit impact packet emitted by the independently
  validated Lean Correspondence v0 kernel.

An atom ledger must prove that every factual atom available in the assisted
arm is either present in raw source bytes or is a deterministic derivation
whose derivation rule is the mechanism under test. Protected labels and answer
keys are absent from both arms.

### Pilot timing and scoring

Each cell has a 1,200-second hard timeout. Start, terminal, teardown, raw
response, usage, stderr, and exact input bytes are retained through the
maintained evidence qualifier. Two independent scorers apply the same closed
key before any discrepancy discussion; disagreement is resolved only after
the agreement statistic is frozen. The scoring program gets one attempt over
the immutable snapshot.

### Stage A gates

Stage B selection is forbidden unless all gates pass:

- both scorers agree on every component of all 12 cells before reconciliation;
- at least 10/12 cells terminate with schema-valid captured responses;
- raw-source composite exact success is between 2/6 and 5/6 inclusive;
- raw-source relation-validation accuracy is between 2/6 and 5/6 inclusive;
- neither arm has a response-contract, atom-equivalence, capture, custody, or
  runtime defect;
- the deliberately invalid relation is rejected in at least 3/4 cells and is
  never scored correct because a witness merely compiled elsewhere; and
- the independent prelaunch reviewer returns exact PASS.

Raw 6/6 is a ceiling stop. Raw below 2/6 is a task-clarity stop. The pilot can
change prompts, timing, or packet construction only through a new versioned
protocol; its observations can never support the flagship claim.

## Stage B — fresh held-out confirmatory study

### Prospective selection

After Stage A passes, an independent source selector chooses exactly six unseen
correspondence families across at least three independently versioned Lean
repositories, with at least one family from each repository and no more than
two from any one repository. A family is a source declaration plus target
declaration, relation candidate, witness/invalidation basis, environment pair,
and closed downstream dependency scope. A source and target in the same Git
repository may be used only when their environments and version histories are
independently pinned; at least three families must cross repository boundaries.

Eligibility requires:

- both endpoints existed before this protocol commit;
- neither visible calibration case, the kernel minimal fixture, nor any family
  used to tune Stage A is eligible;
- a nontrivial relation decision: identity, implication, generated lineage,
  rename, environment invalidation, or semantic break with executable or
  source-owner evidence;
- a bounded, independently auditable impact set with at least two downstream
  items, including at least one unaffected control where the source permits;
- exact repository commit/tree/blob, toolchain, dependency, and licence roots;
- no participant, packet author, runner author, or scorer has inspected the
  protected adjudication before capture freezes; and
- family difficulty is selected from source criteria, never by observing a
  participant answer.

The selector records all eligible candidates and the deterministic selection
rule. If six qualifying families or three repositories are unavailable, the
study stops; there is no substitution.

### Independent adjudication

Two Lean experts who are independent of participants and packet construction
produce closed adjudications from the exact sources. Each must have authored or
maintained a Lean 4 package or had a nontrivial Lean proof reviewed upstream.
They work separately, record relation validity, change class, complete impact
scope, repository-local authority context, and scientific claim ceiling, and
bind every answer to source evidence. A third equivalently qualified
adjudicator resolves only explicit disagreements. The final bytes are encrypted
or access-controlled by the evaluator, committed by root, and unavailable to
participants, packet builders, and the scoring process until all captures pass
the sealed pre-score audit. This protocol creates no key or answer object.

### Arms and fixed denominator

The primary comparison has exactly 72 cells:

```text
6 families × 2 primary arms × 2 configurations × 3 fresh sessions = 72
```

The primary arms are `raw-source` and `correspondence-assisted`, with the same
information boundary used in Stage A. Every family-arm-configuration block has
three sessions. Assignment order is balanced from one externally committed
seed; session ids are derived before any launch. Each assignment has one held
permit, one attempt, and no retry, replacement, substitution, or denominator
repair.

A `structured-unwitnessed` control may add exactly 36 diagnostic cells only if,
before prelaunch freeze, an independent atom audit proves it is byte-equivalent
to the assisted packet except that executable witness, inheritance edges,
recheck state, and their derived impact/current-validity fields are withheld.
It must not add a nicer raw-source index, prose answer, or different task. If
this proof fails, the control is omitted and no presentation-lift claim is made.
The prelaunch artifact freezes `control_included` and the final denominator at
72 or 108; it cannot change after any permit is released.

### Immutable runtime and custody

Before any permit release, one prelaunch artifact binds:

- all six family roots and the complete rejected-candidate ledger;
- exact arm bytes and atom-equivalence receipts;
- participant configurations, runner, image, mounts, trust roots, prompts,
  schemas, output/token/time ceilings, assignment seed commitment, and session
  ids;
- every held permit and zero-call state;
- the protected adjudication commitment and scorer binary/configuration root;
- the exact Vela commit/tree and maintained qualifier source bound here; and
- a passing qualification receipt from one invocation of
  `tools/evidence_qualification/qualification.py` over the complete runtime
  bundle.

The runner may invoke providers only in a separately authorized execution
task. Permits release one at a time. A cell is captured, qualified, committed,
and made immutable before the next permit releases. The final capture audit
checks the fixed denominator and all byte roots before the protected boundary
may open. Scoring is one process and one attempt using read-once pre-key buffers
and Decimal-only arithmetic. A scorer crash before key access is a stopped
non-attempt; a crash after key access yields no retry and `not_supported`.

### Estimands

All correctness estimands are count/rate differences,
`correspondence-assisted − raw-source`, reported per family, per participant
configuration, and aggregate:

- relation-validation accuracy;
- semantic-change versus environment-drift classification accuracy;
- downstream impact completeness;
- composite exact success (all three correct and no false inference); and
- false authority/scientific inference, reported as error reduction
  `raw-source − correspondence-assisted`.

Restricted review time is the Decimal mean of `min(elapsed, 1200)` seconds,
with timeout, missing output, and nonterminal failure assigned 1200. The ratio
is assisted/raw. Actual elapsed distributions remain visible; restricted time
is never called actual runtime. Time is interpreted only after correctness and
safety.

If the diagnostic control exists, its sole positive estimand is witness/
inheritance lift: assisted minus structured-unwitnessed on composite exact,
relation accuracy, and impact completeness, plus error reduction. Equality is
`not_supported`. No generic presentation claim is available.

### Success gates

Per family, all six conditions are required:

- assisted composite exact is at least 5/6;
- assisted composite exact exceeds raw by at least 1/6;
- assisted relation accuracy is not below raw;
- assisted change-classification accuracy is not below raw;
- assisted impact completeness is not below raw; and
- assisted has zero false authority/scientific inferences.

Aggregate flagship success requires every family gate plus:

- assisted relation accuracy at least 30/36 and at least 6 correct responses
  above raw;
- assisted change classification at least 30/36 and at least 6 above raw;
- assisted impact completeness at least 30/36 and at least 6 above raw;
- assisted composite exact at least 30/36 and at least 6 above raw;
- zero assisted false authority/scientific inferences and no increase versus
  raw in either participant configuration;
- assisted composite exact strictly exceeds raw within each participant
  configuration; and
- restricted-time ratio at most 0.80.

Equality never passes a lift gate. A faster arm that misses any correctness or
safety gate is `not_supported`. Correctness/safety gates passing with a slower
ratio supports only bounded accuracy/safety lift, not restricted-time lift or
the flagship combined claim. Every family and configuration remains visible;
no aggregate can hide a reversal.

The optional witness/inheritance gate additionally requires assisted composite
exact to exceed structured-unwitnessed by at least 6 aggregate responses, zero
assisted false inference, no family reversal, and a strict assisted composite
increment in every family. Otherwise witness/inheritance lift is
`not_supported` even if the primary comparison passes.

### Null and negative interpretation

- Equality: no positive lift.
- Mixed families or configurations: heterogeneous/non-general result; no
  aggregate flagship claim.
- Accuracy up, safety down: unsafe and negative.
- Speed up without correctness and safety: no benefit claim.
- Correctness/safety up without time: bounded accuracy/safety result only.
- All gates fail: retain the complete fixed denominator as a negative result.

No result establishes theorem truth, source fidelity beyond adjudicated bytes,
human-review improvement, maintainer acceptance, external adoption, global
authority, or scientific productivity.

## Stage C — Foundry consumption demonstration

The future Foundry artifact is a pure function from rooted source repositories,
Lean Correspondence records/receipts, and explicitly supplied repository-local
authority reads to `foundry-packet.schema.json`. It displays source, target,
relation, witness, current validity, change classification, impact closure,
uncertainty, and repository-local authority context.

It is not a Protocol object, database writer, global identifier, authority
record, Decision, or Standing transport. Its `authority_effect` is `none`; its
identifier is a content root local to the derived packet; and it can be deleted
and reconstructed.

The future one-command contract is:

```bash
bun run reconstruct:lean-correspondence -- \
  --bindings /absolute/path/to/frozen-foundry-bindings.json \
  --out /absolute/path/to/derived-packet.json
```

That command is a specification, not an implemented command in this branch. It
must verify every Git/blob/environment/witness/receipt/authority-read root,
write only the requested derived output, print its canonical root, and be
byte-identical on a second run.

Browser QA must use a local Vela Web preview and cover desktop and mobile:

- all required fields and exact roots are visible without hidden hover state;
- source and target links resolve to the bound commits;
- witness outcome and limits are distinct from scientific acceptance;
- stale or failed recheck state blocks a current-validity badge;
- complete affected/unaffected/unknown impact sets remain distinguishable;
- uncertainty is visible before any suggested next action;
- each Repository's authority context is labeled locally and never aggregated;
- no write, accept, reject, sign, global-consensus, or transported-Standing
  affordance appears;
- two independently governed repositories may disagree without UI collapse;
- keyboard, screen-reader names, contrast, overflow, empty/error, and narrow
  viewport behavior pass; and
- screenshots plus DOM assertions bind the local commit and derived packet
  root.

Implementation, deployment, database migration, and publication are separate
tasks after protocol and product review.
