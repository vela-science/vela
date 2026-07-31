# Product-compression v2

This source-only study asks whether the released Vela read path helps a cold
researcher understand one real Agent campaign and pending Decision faster than
Git and files alone, without losing exact provenance or confusing Verification
with acceptance.

The stopped v1 study remains unchanged in
`paper/artifacts/product-compression-v1`. This directory is the reusable
benchmark source: the closed answer contract, exact fixture compiler,
frozen-plan/result validator, deterministic Vela-specific scorer, and tests.
It cannot mutate a Frontier or perform a Decision. Compact final result roots
belong under `paper/artifacts/`; generated tasks and raw execution evidence do
not.

## Execution boundary

Harbor `0.20.0` is the removable execution harness for the next study:

```text
PyPI wheel SHA-256  4b7e48223aea2384cdb8c9eff35eaebd482fc9b1ec09f8193a121c47356ff19a
source commit       459ff6ec99417589b7f679d14ddf3b3f0ae4f1dc
task schema         1.3
trajectory schema   ATIF-v1.7
```

Harbor owns container execution, network policy, agent adapters, resource
limits, artifact collection, raw job/trial output, and its local comparison
viewer. Vela owns the frozen experimental plan, AB/BA assignment, exact answer
contract, deterministic scorer, state-drift checks, canonical roots, and the
interpretation of scientific authority.

The split is intentional:

- Harbor is transport and retained execution evidence, never scientific
  Standing or authority.
- ATIF is the agent-trajectory interchange format. Native logs remain beside
  it and missing usage is never rewritten as zero.
- OpenTelemetry is an optional metadata-only export, not the canonical record.
- Braintrust may become a read-only analysis mirror if several reviewers need
  shared annotation. It is not required for execution, scoring, or replay.
- Inspect AI is the replacement candidate if a later non-terminal scientific
  evaluation cannot be expressed in Harbor. It is not stacked into this study.

The deleted custom supervisor tried to recreate shell execution, macOS
sandboxing, executable custody, event parsing, and token accounting. That
machinery measured a brittle shell dialect and failed inside its own runtime
boundary. Harbor already supplies the needed environment and agent adapters, so
Vela keeps only the small part that is specific to its scientific claim.

Relevant upstream contracts:

- [Harbor tasks and separate verifier environments](https://www.harborframework.com/docs/tasks)
- [Harbor local jobs and comparison viewer](https://www.harborframework.com/docs/run-jobs/run-evals)
- [Harbor ATIF trajectories](https://www.harborframework.com/docs/agents/trajectory-format)
- [Inspect Agent Bridge](https://inspect.aisi.org.uk/agent-bridge.html)
- [Braintrust immutable experiments and local runs](https://www.braintrust.dev/docs/evaluate/run-evaluations)

## Exact study object

The fixture contains one completed Erdős Attempt, two root-linked Runs, one
registered Submission with one Verification and pending Proposal, one retained
corroborating Run, and the exact successor Target. Run order and registration
order are independent.

A participant reports:

1. the current Target and exact Attempt boundary;
2. both Runs and the receipt chain, including which Run was registered;
3. the pending Proposal, Verification scope, and conditional Standing change;
4. that inspection exercised no authority and changed no accepted state; and
5. the exact successor Target that can begin under a new bounded Attempt.

`answer.schema.json` is the participant-facing output contract and is
recursively closed. `harness.py` independently validates the semantic links
rather than treating JSON Schema as a second protocol implementation.

## Freeze and custody gates

No participant run may start until a frozen plan binds:

- one exact fixture and supervisor-only answer-key root;
- the Harbor release, source commit, task schema, package digest, and ATIF
  version above;
- exact model, agent, tool, Vela binary, Git source, and environment image
  identities;
- shared budgets, retry rules, stopping rules, and AB/BA assignment;
- both arm-specific tool surfaces; and
- publication of every retained success, failure, and infrastructure stop.

The answer key belongs only in Harbor's separate verifier image and is never
copied into the agent container. The verifier has no network, authority key,
authentication store, mutation route, or access to the source Frontier. The
baseline receives an isolated copy with ordinary repository tools. The guided
arm receives the same copy plus the exact read-only Vela interface.

The study uses Harbor's native Codex OAuth path by passing the host-only
`CODEX_AUTH_JSON_PATH` when Harbor starts. Authentication is deliberately absent
from `agents[].env`: Harbor 0.20 treats values in that field as secrets and may
redact matching low-entropy JSON literals in retained output. No Harbor source
patch or Vela auth adapter is used. Harbor's agent and task commands share one
container, so this is execution custody rather than credential isolation. The
normal OAuth session is used only with these trusted, Vela-owned tasks. Agent
egress is limited to the required provider hosts and the verifier remains fully
offline.

`materialize.py` reconstructs the fixture and answer key from one clean exact
Frontier, one exact Vela binary, a completed private Attempt record, and one
pending Proposal. It calls only `status`, `next`, and `review inbox`, verifies
that Git and repository state do not change, checks every retained Run,
Submission, Verification, Decision Inbox, and successor-packet binding, and
sanitizes private absolute paths.

## Measurement

The primary outcome is exact correctness under fixed budgets. The paired report
then measures:

- elapsed time;
- validated ATIF tool calls and token usage;
- reported output volume;
- semantic interventions;
- Git, repository, and Standing drift; and
- exact answer and authority-boundary correctness.

The four first-party sessions are counterbalanced across two AB/BA pairs. A
prospective result passes only if all exact answers pass, the guided arm is
faster in both pairs, median elapsed time improves by at least 20 percent, and
median tool calls and observed tokens do not regress. This pilot cannot prove
external adoption.

## First complete native Harbor result

The frozen plan
`sha256:3ec7c05814add867dc096887f6d8af5deca16fe70eb883e86c76747ba10f8598`
completed all four counterbalanced sessions on 2026-07-31 with no execution
exception, intervention, authority access, or Frontier mutation. The compact
result is retained at
`paper/artifacts/product-compression-v2/result.native-harbor.v1.json`.

The registered gate failed. Vela-guided was faster in both pairs and improved
median elapsed time by 29.78 percent, with lower median tool calls and observed
tokens. Only one session matched the frozen answer exactly, however; all four
exceeded the 24,000-token budget and three exceeded the 12-tool budget. This is
directional first-party evidence, not product-lift credit.

The raw run also exposed two defects in the frozen study. Its task verifier
compared byte-valued `git status` output with a string, unconditionally
reporting drift, while the trajectories show clean final checkouts. The answer
contract used `accepted_before` and `accepted_if_*` for a target-scoped delta
without naming that scope, so two agents returned global accepted Standing.
Because outputs already existed, v2 remains frozen. Any v3 task must use one
explicit target-scoped Standing delta, a corrected clean-worktree verifier, and
a new plan root. Do not rerun unchanged v2 or infer a product claim from its
directional timing signal.

The reusable source contract now consumes the explicit
`vela.decision-inbox.v2` Standing delta and validates its scope, hypothetical
repository roots, and global counts. This prepares a future v3 confirmation
study. The first corrected v3 plan is frozen locally at
`sha256:9894601c5234dfac451d4b045410af32b3660e399bf63916972f45b83bd3f0cb`;
it binds the corrected fixture, answer key, exact candidate Linux Vela binary,
model, Codex version, and budgets. No v3 participant output existed when that
root was created.

## Retained calibration evidence

Two custom-runner calibrations and their stopped sessions remain retained by
root outside this source tree. They prove that failures were preserved and
state did not drift. They do **not** establish product lift or product failure:
the custom macOS sandbox blocked legitimate Git/Vela runtime dependencies,
policed exact command spellings, and later failed with an internal operation
error before retaining a session. Those are execution-harness failures.

The next matched study receives a new plan root and session IDs. No output from
the stopped custom supervisor is silently reclassified as Harbor evidence.

## Local verification

```bash
python3 -m unittest discover \
  -s benchmarks/product-compression -p 'test_*.py'

python3 benchmarks/product-compression/harness.py validate \
  --kind answer --input answer.json

python3 benchmarks/product-compression/harness.py score \
  --plan plan.json --answer-key answer-key.json \
  --session session.json --output score.json

python3 benchmarks/product-compression/harness.py report \
  --plan plan.json --answer-key answer-key.json \
  --sessions session-*.json --output result.json

python3 benchmarks/product-compression/materialize.py \
  --frontier /exact/frontier --vela /exact/vela \
  --attempt /exact/private/attempt.json --proposal vpr_<id> \
  --output jobs/product-compression-v2/materials

python3 benchmarks/product-compression/harness.py freeze-plan \
  --materials /exact/private/materials --model gpt-5.6-terra \
  --codex-version 0.145.0 --vela-linux /exact/static-linux-vela \
  --vela-version 'vela 0.950.1' --output /exact/plan.json

python3 benchmarks/product-compression/harness.py prepare-harbor \
  --plan /exact/plan.json --materials /exact/private/materials \
  --frontier /exact/clean/frontier --vela-linux /exact/static-linux-vela \
  --job-name vela-product-compression-v3-native \
  --output jobs/product-compression-v3/harbor-native

cd jobs/product-compression-v3/harbor-native
env -u OPENAI_API_KEY CODEX_AUTH_JSON_PATH="$HOME/.codex/auth.json" \
  harbor run --config harbor-job.json --max-retries 0 \
  --jobs-dir ../runs
```

`prepare-harbor` emits four standard Harbor 1.3 task directories plus one job
configuration in the frozen AB/BA order. It is a materializer, not a runner or
adapter. Harbor itself owns task digests, container setup, execution, retries,
ATIF, artifact collection, job locks, and raw results. Each task receives the
same exact Frontier checkout and participant evidence; only the guided task
receives the exact read-only Linux Vela binary. The answer key exists only in
the separate offline verifier image.

The ignored `jobs/product-compression-v2/` tree is the local execution area.
Harbor's job directory is the retained raw evidence. Harbor's own
result, lock, ATIF trajectory, verifier reward, artifact manifest, timing,
usage, and viewer remain the inspection surface; Vela does not wrap them in
another execution format. The small Vela harness owns only the frozen plan,
answer semantics, and registered comparison gates. Harbor remains an external
tool, not a Vela runtime dependency.
