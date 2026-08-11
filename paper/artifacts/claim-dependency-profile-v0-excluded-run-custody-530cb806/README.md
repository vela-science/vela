# Excluded claim-dependency observation custody

This packet records the custody disposition of the four Harbor sessions run
from Vela commit `530cb806ad9d219341cf3e5ec168e9683136a427`.
All four sessions are excluded under the preregistered
`executor_contract_drift` code. They are retained as invalid execution
evidence only; they are not observations from either registered presentation
and are not eligible for result, timing, or lift interpretation.

The frozen observation packet at
`paper/artifacts/claim-dependency-profile-v0-observation` remains
byte-identical. This sibling packet does not amend its plan, prompts, response
contract, scorer, input manifests, experiment bytes, run order, or exclusion
rules. No retry or rerun is authorized under that packet.

## Why the sessions are excluded

Each session received two user messages instead of the frozen single Harbor
participant instruction. Before the 3,092-byte registered instruction, Codex
injected the same 1,897-byte `recommended_plugins` user message with raw root
`sha256:1bc125e27ebf6edfc3e9f8993f8ce768b29df05175099cc692986b90573508f1`.
The session world state also exposed five built-in skill descriptions
(`imagegen`, `openai-docs`, `plugin-creator`, `skill-creator`, and
`skill-installer`) with instruction inclusion enabled. That violates the
frozen context-isolation contract even though no skill, MCP, or participant
network tool call occurred and the review found no held-out scientific fact in
the exposed context.

Eight retained Harbor log files—the job and trial log for each session—also
name the host authentication source path
`/Users/williamblair/.codex/auth.json`. They expose the path only, not a secret
value. A post-run scan found no exact authentication leaf and no
high-confidence credential pattern in the retained run tree. No matched value
or authentication material is copied into this packet.

Harbor logged two skipped Docker operating-system validations per run, one
for the agent image and one for the verifier image. The ephemeral images were
deleted and are no longer inspectable. The pre-run execution attestation image
IDs remain bound as declared inputs, but there is no retained proof that those
IDs were the images actually used by these sessions. This independent custody
gap is recorded without treating it as a scientific result.

## Retention boundary

`custody.json` binds the source repository commit and tree, frozen packet tree
and manifests, experiment and preregistration roots, generated study and
attestation roots, task and declared image roots, exact run order, Harbor job,
trial, session, config, result, answer, and verifier identities, and every file
in the 64-file `runs/` subtree. JSON ledger rows include the RFC 8785 canonical
root produced by the frozen materializer; other rows include only their raw
byte root. It also binds the complete 124-file external study by one ledger
root and byte count.

Raw logs, session JSONL, trajectories, participant answers, verifier records,
and Harbor result files remain outside Git at:

```text
/Users/williamblair/personal/vela-observation-runs/claim-dependency-profile-v0-530cb806
```

Their contents are not reproduced here. The external tree is required to
reconstruct or inspect any bound file. The source-local packet is a hash
inventory, not a substitute for retained external custody.

## Reconstruction

From this Vela worktree, first prove that the frozen packet still matches the
source commit, then recompute both external ledger roots with the exact
materializer implementation that defined their row shape:

```bash
git diff --exit-code 530cb806ad9d219341cf3e5ec168e9683136a427 -- \
  paper/artifacts/claim-dependency-profile-v0-observation

PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python - <<'PY'
import importlib.util
from pathlib import Path

packet = Path("paper/artifacts/claim-dependency-profile-v0-observation")
spec = importlib.util.spec_from_file_location("materialize", packet / "materialize.py")
materialize = importlib.util.module_from_spec(spec)
spec.loader.exec_module(materialize)

source = Path(
    "/Users/williamblair/personal/vela-observation-runs/"
    "claim-dependency-profile-v0-530cb806"
)
runs = materialize.file_ledger(source / "runs", set())
study = materialize.file_ledger(source, set())

assert len(runs) == 64
assert sum(row["bytes"] for row in runs) == 1_146_904
assert materialize.root(materialize.rfc8785.dumps(runs)) == (
    "sha256:a404a0ed4487e3e6ba3640b841ec16d3199addaa02fbb708ab3deba704779438"
)
assert len(study) == 124
assert sum(row["bytes"] for row in study) == 1_311_849
assert materialize.root(materialize.rfc8785.dumps(study)) == (
    "sha256:305a0aef055668dc6bc0916b63059704373aea247807470ded5f53745aef90b6"
)
print("excluded-run custody roots verified")
PY
```

The root calculation is `sha256(rfc8785(file_ledger))`. Paths in the run
ledger are relative to `runs/`; paths in per-run bindings are relative to the
external study root.

## Nonclaims

- Authority effect is `none`; this packet makes no Decision or Standing
  change.
- Claim credit is `false`; all registered milestones remain `not_measured`
  and the primary metric remains `not_computable`.
- The recorded answers, verifier outputs, timings, tokens, and costs support no
  comparison between presentations.
- Absence of a skill/MCP/participant-network tool call, held-out fact, or
  detected credential does not cure the executor-context exposure.
- This packet does not authorize a retry, rerun, protocol change, experiment
  amendment, or scientific claim.
