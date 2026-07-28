# ADR 0010: Nonmutating Runs and explicit Submission

- Status: Accepted
- Product release: `product-v0.8.0` (Canopus `0.8.0`)
- Protocol effect: None
- Vela requirement: current `vela.submission.v1` and `vela submit`
- Supersedes for current writers: automatic landing portions of ADR 0004
- Acceptance gate: the corrected real Erdős Submission must register as
  pending review with accepted-event delta zero and reproduce from a clean
  clone; package, custody, provenance, and release checks must also pass.
- Accepted: 2026-07-28 after Submission `vsb_be4ef74c7c4857c9`
  registered with zero accepted-state delta, Verification
  `vvr_1974ed5d3e3a72c3` passed, the human Decision completed, and the exact
  Frontier replayed from a clean clone.

## Context

Canopus is a bounded research runner, not a scientific-state authority. Its
released product nevertheless combined model execution, independent verifier
replay, Receipt construction, Vela work mutation, Proposal registration, and
landing into one ordinary `run` command.

That composition made a successful computation hard to distinguish from a
registered scientific request. It also made safe inspection depend on terms
such as landing and Receipt that are no longer part of Vela's current producer
language.

The current product cycle is:

```text
doctor -> run -> show -> replay -> export -> submit
```

The boundary must remain useful when Canopus is deleted and interoperable with
any producer capable of emitting the same authenticated Vela Submission.

## Decision

### Run

`canopus run` is nonmutating by default and has no mutating mode.

It:

1. resolves one exact Vela offer;
2. prepares disposable, bounded worker and verifier environments;
3. freezes the candidate bytes;
4. executes the independent verifier;
5. reproduces the verifier result from a clean clone; and
6. writes `canopus.run.v2` with `effect: "none"`,
   `authority: "non_authoritative"`, and `submission: null`.

It does not claim Vela work, publish Artifacts into a Frontier, register a
Proposal, create a Verification Record, create a Decision or Event, or change
Standing.

There is no `--no-land` switch because the safe behavior is not optional.

### Show and replay

`canopus show` reads current Run v2 and retained failure records. Historical
diagnostic and landed Run formats remain readable through their exact tagged
Canopus releases; the current package does not ship their parsers or schemas
and never normalizes their bytes into the current schema.

`canopus replay` reruns the frozen verifier without a model call or Frontier
mutation. The current command accepts Run v2 only.

### Export

`canopus export <run.json>` converts only a successful, independently
verifier-passing current Run into a portable bundle:

```text
canopus.submission-bundle.v1
  submission.json
  manifest.json
  artifacts/sha256/<full-digest>
```

`submission.json` is a whole-body Ed25519-signed `vela.submission.v1`. The
identity is ephemeral and agent-class. Canopus retains no private key or
withdrawal capability after the authenticated Submission bytes exist.

Independent verifier output is named as a verification requirement. It is not
placed in `producer_checks`, called a Verification Record, or treated as
acceptance.

`vela.execution-binding.v1` remains optional on Submission v1. A review-only
Run without a positive result contract may export without it; the Submission's
verification requirement still binds the exact capsule root. Such a Submission
is not policy-eligible. Exact policy eligibility requires the full packet,
profile, verifier-capsule, and result-contract binding.

### Submit

`canopus submit <bundle> <frontier>` is the only current Canopus operation that
may change a Frontier.

Before mutation it verifies:

- the closed bundle manifest;
- the Submission identity, whole-body signature, and full root;
- exact agreement between bundled and declared Artifacts;
- safe relative paths, byte counts, and full digests;
- the exact retained source Git commit and tree;
- that the current clean Frontier is that commit or a descendant of it;
- a clean source worktree; and
- the current supported registration binary version and its observed SHA-256.

The Run's historical Vela version and SHA-256 remain immutable source
provenance. They are not incorrectly required to write a later current
repository epoch. The submit result records the exact current registration
binary identity.

It performs registration in a disposable exact-head clone, accepts only
`vela.submit-result.v1` with `pending_review` and accepted-event delta zero,
then fast-forwards the still-clean source checkout. A failure before the final
fast-forward leaves the source Frontier unchanged.

Canopus never emits a Verification Record, Decision, Event, or scientific
Standing.

## Compatibility and migration

Historical Run, Receipt, landing, and activity bytes remain immutable in Git
history, releases, and retained run evidence. Their exact releases remain the
corresponding readers. The current package contains neither their writers nor
their readers.

Mission v1 and profile v2 remain advanced, content-addressed input contracts.
Their historical `landing` field is interpreted only as a bounded legacy
result expectation; current Run does not execute a landing. A future input
schema may rename that field only with a demonstrated interoperation need.

Current source exposes exactly:

```text
doctor run show replay export submit
```

`inspect`, automatic landing, Receipt authoring, retained producer
capabilities, and Canopus-managed withdrawal are not compatibility aliases.

## Adversarial cases

- A dirty or root-drifted source Frontier blocks submit.
- A missing source commit, wrong retained tree, or non-descendant current
  Frontier blocks submit.
- The submit result discloses the exact registration-binary SHA-256; release
  qualification fails if it does not match the pinned platform artifact.
- A malformed, extra-field, path-traversing, duplicated, missing, truncated,
  or digest-mismatched Artifact blocks before Vela runs.
- A manifest that swaps producer, identity binding, Submission, source roots,
  or Artifact bindings blocks.
- A verifier pass cannot populate a Verification Record or accepted Standing.
- A failed clean-clone reproduction cannot be exported.
- A successful Run cannot mutate the Frontier, even when a historical Mission
  contains a landing expectation.
- Deleting Canopus and all private Run evidence cannot change Vela replay.

## Conformance contract

Focused tests must prove:

- current help exposes exactly the six-command cycle;
- Run v2 leaves source Git and Vela roots unchanged;
- a current Run reproduces from a clean clone;
- export emits an authenticated Vela Submission with no retained key;
- bundle and Submission Artifact sets agree exactly;
- submit rejects path traversal, bundle drift, unsupported registration
  binaries, missing/non-descendant source history, and authority-changing Vela
  results;
- submit fast-forwards only after verified pending registration;
- historical current-source readers still parse released Run formats; and
- released Vela composition retains zero mutation during Run.

Release checks:

```bash
bun install --frozen-lockfile
bun run check
bun run pack:check
git diff --check
```

## Consequences

The ordinary product now matches its actual responsibility: Canopus produces
bounded, replayable research evidence; Vela registers authenticated producer
input and owns review and Standing.

The extra explicit `export` and `submit` steps are deliberate. They make the
mutation boundary inspectable, scriptable, and replaceable without adding a
new service, protocol object, authority surface, or retained key lifecycle.
