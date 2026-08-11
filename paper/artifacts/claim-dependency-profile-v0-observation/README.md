# Claim-dependency matched observation v0

This packet freezes an instrumentation pilot before any participant output
exists. It compares two presentations of the same synthetic, counterfactual
claim-dependency facts owned by ADR 0043. Historical ADR 0040 is superseded
and remains only the record of the rejected wire-first design. This packet does
not change the experiment
fixture, Vela protocol bytes, Math, Web, authority, or Standing.

[Harbor 0.20.0](https://www.harborframework.com/docs/core-concepts) remains the
only execution and custody substrate. Harbor owns the isolated Docker
environment, Codex execution, native trial records, trajectories, verifier
phase, telemetry, and artifacts. This packet contributes only:

- an exact common-scope amendment that makes the two arms information-equal;
- a participant instruction and closed response contract;
- four sequential, counterbalanced run registrations;
- a task materializer that copies only allowlisted inputs;
- one held-out, offline scorer; and
- result retention and root rules.

There is no Vela runner, session database, model API adapter, or authority
action. Generated Harbor tasks, jobs, images, and runs remain outside Git.

## Scientific boundary

The task is synthetic. Current Math Claims at
`a6a31a528ee86ab79c2aaf4e71e43fc63f4a4e98` have empty relation arrays. A0 was
rejected, A1 was later accepted, and B, D, and E are fixtures. This is not an
accepted-state Correction, a real rooted dependent, Class E evidence,
scientific truth, or a productivity result.

The baseline receives the exact raw facts, state, reducer semantics,
RO-Crate/review context, and `shared-scope.json`. The treatment receives those
same bytes plus `profile.json`. The shared-scope amendment states facts that
the profile previously made explicit only to treatment: one Repository context,
closed Claim/dependency sets, and the same 8/8 bounds. It was frozen before
the first observation and does not alter the ADR 0043 experiment package.

## Pilot boundary

The registered pilot is four fresh agent sessions: two per arm in two
counterbalanced blocks. It can validate isolation, scoring, telemetry, and
repeatability only. One synthetic task repeated four times is one experimental
unit, not four. It earns no causal, general performance, reviewer-productivity,
adoption, external-independence, scientific, or protocol claim.

Harbor has no audited monotonic, answer-blind checkpoint seam for the four
registered milestone timings. They therefore remain `not_measured`. Exact
projection-root agreement and both cold-successor fields also remain
`not_measured`; transitions per expert minute remains `not_computable`.

## Custody

The agent image contains only the arm input files, the shared response schema,
an empty task-data workspace, and the ordinary operating-system/Codex runtime.
It contains no Git repository or history, answer key, scorer, plan, fixture
README, expected projection, negative vectors, Vela source, Math checkout,
prior result, user configuration, memory, skill, MCP server, connector, Vela
authority key, or authority-agent socket. The held-out scorer and answer key
exist only in Harbor's separate, no-network verifier image.

Harbor necessarily injects an ephemeral Codex model-transport authentication
file for the agent phase and removes it best-effort afterward. That credential
is not scientific context or Vela authority and is not supplied as a task
input. It must not be retained or described as result evidence. Before result
import, custody review must secret-scan the participant artifact without
publishing any matched value; a suspected credential copy is quarantined as an
executor-contract failure, not interpreted or silently deleted.

`/workspace` is the only task-data work area and `/logs/artifacts` is the only
retained participant result path. Harbor and Codex also require transient
writes under the participant home, `/tmp/codex-home`, `/tmp/codex-secrets`, and
`/logs/agent`; none is scientific input or an additional retained answer path.

The task environment has no ambient network. During the agent phase Harbor
allows only the recorded OpenAI/ChatGPT transport hosts. The verifier has no
network. Automatic Harbor retry is disabled because Harbor may replace failed
trial custody during a queue retry. The one permitted infrastructure retry is
a new Harbor job, only before any participant-authored output; the failed job
must remain retained and rooted.

## Before running

No participant run may begin until all of these gates pass against the same
committed packet:

1. `test_observation.py` passes, including double materialization and scorer
   mutations.
2. both generated task contexts contain exactly their registered allowlists;
3. both Docker images build and an unscored shell probe proves that `/input`
   is read-only, `/workspace` is the only retained task-data work area,
   held-out names are absent, and direct network is unavailable outside
   Harbor's agent phase;
4. Harbor resolves four sequential jobs with Codex `0.145.0`, requested model
   `gpt-5.6-sol`, `high` reasoning, automatic reasoning summaries, disabled Web
   search, no skills/MCP, and zero automatic retries;
5. the built Linux Codex binary and image IDs are recorded in the generated
   study manifest before the first run; and
6. an independent source review confirms that the original experiment files
   are byte-identical.

The run custodian must prove that `OPENAI_BASE_URL`, `OPENAI_API_KEY`, and
`CODEX_AUTH_JSON_PATH` are unset and select `codex_auth_json_transport` with
`CODEX_FORCE_AUTH_JSON=1`. That mechanism must remain fixed across all four
jobs. The credential and its source path are transient custody data rather
than scientific input or publishable provenance.

Materialize without running a model:

```bash
uv run --project conformance --locked python \
  paper/artifacts/claim-dependency-profile-v0-observation/materialize.py \
  --output /exact/external/claim-dependency-observation-v0
```

The materializer creates two byte-stable task definitions, one for each arm,
and four separate `jobs/*.json` files that map the registered run IDs onto
those tasks. Run the jobs sequentially in the order in `plan.json`; never
replace that order with a concurrent Harbor dataset job. Result import is a
later, separately reviewed step.
