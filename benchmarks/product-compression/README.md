# Product-compression benchmark

This source-only study asks whether the released Vela read path helps a cold
researcher understand one real Agent campaign and pending Decision faster than
Git and files alone, without losing exact provenance or confusing Verification
with acceptance.

The stopped v1 study remains unchanged in
`paper/artifacts/product-compression-v1`. This directory is the reusable
benchmark source: the closed answer contract, exact fixture compiler,
short frozen study manifest, Harbor task materializer, and offline semantic
verifier.
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
limits, offline verification, multi-metric rewards, artifact collection, raw
job/trial output, token/cache/cost telemetry, and its local comparison viewer.
Vela owns only the frozen scientific inputs and comparison rule, exact answer
contract, state-drift checks, canonical roots, and interpretation of scientific
authority.

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

The deleted custom supervisor and the former Vela session/score/report schemas
tried to recreate shell execution, macOS
sandboxing, executable custody, event parsing, and token accounting. That
machinery measured a brittle shell dialect and failed inside its own runtime
boundary. Harbor already supplies those facilities, so Vela keeps only the
small part specific to its scientific claim. Terminal-Bench is a Harbor dataset,
not a second harness; the Codex SDK is an agent interface, not an eval runner.

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
recursively closed. `study.py` independently validates the semantic links
rather than treating JSON Schema as a second protocol implementation.

## Freeze and custody gates

No participant run may start until a frozen plan binds:

- one exact fixture and supervisor-only answer-key root;
- the Harbor release, source commit, task schema, package digest, and ATIF
  version above;
- exact model, agent, tool, Vela binary, Git source, and environment image
  identities;
- shared elapsed/output safety limits, retry rules, stopping rules, and AB/BA
  assignment;
- both arm-specific tool surfaces; and
- publication of every retained success, failure, and infrastructure stop.

The answer key is uploaded only for Harbor's verifier phase after agent
execution; it is absent during the agent phase. The verifier has no network, authority key,
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

Harbor writes two offline verifier rewards for every trial: `eligible` and
`exact`. Eligibility covers the matched assignment and toolchain, completed
execution, exact roots, no intervention or state drift, and bounded elapsed and
retained output sizes. Exactness covers the frozen answer. A valid but wrong
baseline remains eligible; otherwise a product can never demonstrate a
correctness advantage over its control.

Harbor then reports:

- elapsed time;
- validated ATIF tool calls and token usage;
- reported output volume;
- semantic interventions;
- Git, repository, and Standing drift; and
- exact answer and authority-boundary correctness.

The four first-party sessions are counterbalanced across two AB/BA pairs. The
prospective comparison rule is:

1. all four sessions must be eligible;
2. Vela-guided must be exact in both repetitions;
3. if the baseline is not exact in both repetitions, report a bounded
   task-specific exactness advantage only when median cost does not regress;
4. if both arms are exact twice, require at least 20 percent median elapsed-time
   improvement and no median cost regression; and
5. otherwise report no demonstrated advantage.

Tool calls, provider input/cache/output tokens, and the uncached-token proxy are
comparative telemetry, never arbitrary hard gates. The only hard limits are
elapsed execution and retained answer/tool/trajectory/verifier sizes. This
four-session first-party pilot cannot prove external adoption or general
scientific productivity.

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
study. The executed corrected v3 plan is
`sha256:55ed5aedac69fc9f2f48d7cee794fc2ca8301567883863d3f7bedff273eac252`;
it binds the corrected fixture, answer key, exact candidate Linux Vela binary,
model, Codex version, and limits. Its stopped predecessor bound an inexact CLI
version string; that failure remains retained under its distinct plan and job
roots rather than being rewritten.

That corrected run completed all four trials. Vela-guided was exact 2/2 versus
Git/files 0/2, while reducing median elapsed time by 51.61 percent, median cost
by 60.37 percent, uncached-token proxy by 58.84 percent, and tool calls by 28.57
percent. The frozen registered composite still records failure because it used
the old all-sessions-exact and 24,000-token rules. The result is not rewritten
or rerun. It is retained as strong task-specific product-comprehension evidence
and an explicit methodology correction for future tasks.

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

python3 benchmarks/product-compression/study.py validate \
  --kind answer --input answer.json

python3 benchmarks/product-compression/materialize.py \
  --frontier /exact/frontier --vela /exact/vela \
  --attempt /exact/private/attempt.json --proposal vpr_<id> \
  --output jobs/product-compression-v2/materials

python3 benchmarks/product-compression/study.py freeze-plan \
  --materials /exact/private/materials --model gpt-5.6-terra \
  --codex-version 0.145.0 --vela-linux /exact/static-linux-vela \
  --vela-version 'vela 0.950.1' --output /exact/plan.json

python3 benchmarks/product-compression/study.py prepare-harbor \
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
receives the exact read-only Linux Vela binary. The answer key appears only in
the offline verifier phase.

The ignored `jobs/product-compression*/` tree is the local execution area.
Harbor's job directory is the retained raw evidence. Harbor's own
result, lock, ATIF trajectory, verifier reward, artifact manifest, timing,
usage, cost, and viewer remain the inspection surface; Vela does not wrap them
in another session, score, or report format. The small Vela layer only
materializes exact tasks and verifies Vela-specific semantics. Harbor remains
an external tool, not a Vela runtime dependency.
