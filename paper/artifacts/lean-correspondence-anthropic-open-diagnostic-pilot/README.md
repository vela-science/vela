# Anthropic-only open diagnostic pilot

This is a separate, additive, non-confirmatory six-cell diagnostic. It does not
modify, reinterpret, release, or replace the frozen 12-cell, two-provider Stage
A package. It reuses byte-for-byte the three visible Stage A cases, the
Anthropic-side raw and correspondence-assisted prompts and packets, the closed
response schema, the scoring semantics, and the arm-information boundary.
The assignment ID printed inside each copied prompt and packet is therefore the
frozen `source_assignment_id`; the new `cell_id` and `participant_id` are the
distinct diagnostic execution and custody identities. The verifier binds this
one-to-one mapping and rejects substitution or cross-binding.

The design has one exact independently qualified Anthropic configuration, one
fresh one-shot participant per case/arm cell, a fixed denominator of six, zero
retries, and zero substitutions. Every new permit is held and non-releasable
until a separate independent exact prelaunch review passes and a later explicit
execution authorization exists. No credential contents are present here.

A future diagnostic PASS could qualify only the feasibility of this exact
Anthropic reviewer-agent configuration on these three open cases. It cannot
satisfy the frozen two-provider Stage A, the Living Frontier roadmap's G3
inheritance-advantage or Phase 0 gates, Stage B, cross-provider generality,
scientific lift, human benefit, Frontiers expansion, a breakthrough claim, or
any authority, Decision, or Standing effect.

The current roadmap headline remains the sealed 36-cell negative result: Git
and documents 12/12, neutral state wrapper 12/12, Vela 11/12 with one authority
error, and every preregistered positive gate false. The earlier 16-cell
directional observation is not current evidence and is not used here.

Generate and verify without creating bytecode:

```console
PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/generate.py
PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/verify.py
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/test_verify.py
```

Generation, verification, and tests perform no credential access, provider
call, permit release, response scoring, Stage B selection, Protocol/Core
change, or authority action.
