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

The source packet manifests remain byte-for-byte unchanged. Because the
reviewed runner accepts only recursively canonical JSON packet bytes, the
package also freezes one deterministic execution-packet derivative per cell
and a duplicate-aware source-to-execution semantic-equivalence receipt. Each
maintained permit binds the exact execution bytes the runner loads; custody
binds both source and execution roots. The runner's canonical-byte requirement
is not weakened.

The design has one exact independently qualified Anthropic configuration, one
fresh one-shot participant per case/arm cell, a fixed denominator of six, zero
retries, and zero substitutions. Every new permit is held and non-releasable
until a separate independent exact prelaunch review passes and a later explicit
execution authorization exists. No credential contents are present here.
All six permits use the maintained closed-launch schema. Each is packaged with
its exact run input, raw-schema materialization receipt, request bytes,
lossless transport custody, network-none same-input validation, production
runtime/image/schema bindings, and maintained `qualified_hold` receipt.

Each cell also has a content-addressed read-only `/workspace` assignment tree.
It contains the exact packet-referenced repository atoms, bounded history
receipts, Lean files, and witnesses; raw cells receive only base atoms and
assisted cells receive those same atoms plus the registered correspondence
derivatives. A closed assignment manifest binds every logical path, mounted
path, byte count, and digest. The provider sees these bytes only through the
single maintained read/list/stat/literal-search tool—never by prompt
augmentation or a command surface—and an offline bridge receipt proves every
referenced path is reachable before a held permit can become eligible for later
independent release review.

The scorer accepts no correctness or safety booleans. It reads a complete
six-cell capture set, checks the raw response, terminal, usage, permit/run, and
custody roots, and derives every registered component against the frozen open
adjudication. Missing, failed, timed-out, and malformed outcomes stay in the
denominator. Restricted-time differences use canonical Decimal strings and
tool-count differences use exact integers.

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
PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/verify.py --maintained-qualifier
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/test_verify.py
PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/lean-correspondence-anthropic-open-diagnostic-pilot/verify_bundle_adversaries.py
```

Generation, verification, and tests perform no credential access, provider
call, permit release, response scoring, Stage B selection, Protocol/Core
change, or authority action.
