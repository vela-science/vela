# Lean Correspondence Stage A open pilot prelaunch

Status: **artifact frozen at 0/12; launch blocked; independent prelaunch review required**.

This package fixes the 12-cell Stage A selection, assignment, arm bytes, prompts,
response schema, held permits, scoring semantics, and custody boundary defined by
the reviewed Lean Correspondence + Foundry method. It performs no participant,
provider, scoring, key, Repository-authority, Decision, or Standing action.

The three public calibration cases are permanently excluded from Stage B:

- the bounded Erdős 730 affirmative-RHS relation;
- the Formal Conjectures to LeanEval `OeisA303656` lineage/history relation; and
- a deterministic semantic-invalid relation between two compiling declarations
  that return distinct natural numerals and whose Lean witness fails.

The participant-visible identifier for the invalid fixture is neutral. Its
custody identifier remains explicit because Stage A is open, but neither prompt
nor packet contains a gold label or scoring key. The assisted arm receives only
the rooted relation, witness outcome, recheck state, and explicit impact output
that the mechanism under test derives from the same base semantic atoms available
to the raw-source arm.

## Exact blocked prerequisite

The reviewed method requires two cold, tool-using immutable model configurations
from different provider organizations under the same read-only offline shell/file
boundary. The independently qualified runtime inherited from the earlier Vela
study is not reusable: it is a single OpenAI/Codex configuration, disables tools,
and binds a different prompt and response schema. No exact two-provider Stage A
runtime bundle or maintained-qualifier receipt exists at this commit.

Consequently `runtime-binding.json` and `prelaunch-state.json` fail closed. Both
configuration slots are unbound, every one of the 12 permits is held and
non-releasable, and execution remains unauthorized. A later prospective amendment
must bind both exact model snapshots and a complete runtime bundle, invoke the
maintained qualifier at its bound Vela commit, regenerate all transitive roots and
permits, and obtain a fresh independent exact PASS before any call.

## Regenerate and verify

Use clean detached checkouts of the neutral implementation and candidate packets:

```bash
uv run --project conformance --locked python \
  paper/artifacts/lean-correspondence-stage-a-open-pilot/generate.py \
  --implementation /absolute/path/to/lean-correspondence-at-01d0b325 \
  --candidates /absolute/path/to/lean-proofs-at-148e18cc

uv run --project conformance --locked python \
  paper/artifacts/lean-correspondence-stage-a-open-pilot/verify.py \
  --implementation /absolute/path/to/lean-correspondence-at-01d0b325 \
  --candidates /absolute/path/to/lean-proofs-at-148e18cc \
  --check-lean

uv run --project conformance --locked python -m unittest discover \
  -s paper/artifacts/lean-correspondence-stage-a-open-pilot -p 'test_*.py' -v
```

The verifier requires the exact reviewed Vela method commit/tree, neutral
implementation commit/tree, reviewed import, candidate packet commit/tree,
maintained qualifier blob, fixed 2 × 3 × 2 denominator, exact prompt/schema
bytes, arm atom equivalence, unique assignments and held permits, semantic-invalid
fixture Git identities, compiling declarations, failing witness, zero-call state,
and absence of response, score, key, capture, Stage B selection, or authority
material. The adversarial suite rejects stale or missing roots, case substitution,
answer leakage, arm atom mismatch, duplicate/reused ids, denominator drift, early
qualification, and early permit release.

Passing these checks means only that the blocked prelaunch package is internally
deterministic. It is not launch authorization, a Stage A result, a Stage B family
selection, scientific acceptance, Vela authority, a Decision, or Standing.

Authority effect: **none**.
