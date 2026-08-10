# Claim dependency profile v0 experiment

This directory freezes a **noncanonical, experiment-only** `requires` profile.
It is not a Vela schema, Submission or Claim field, relation object, Standing
rule, or protocol byte. The profile is a post-hoc source-local artifact;
SubmissionV2 and the current schema waist remain unchanged by this experiment.

The graph is a synthetic counterfactual over two exact retained Math anchors:
A0 is the rejected predecessor
`vcl_24878e1c...` / `sha256:40dec807...`; A1 is the accepted successor
`vcl_3d4fd595...` / `sha256:d5d77e7d...`. B, D, and E are synthetic. Current
Math Claim relations are empty, so this fixture is not an accepted-state
Correction, a real rooted dependent, Class E evidence, or a scientific result.

`profile.json` closes both its Claim and dependency sets, binds every node and
`requires` endpoint to a full Claim root plus the same Math Repository UUID and
origin root, and contains explicit `does_not_establish` text. `state.json`
freezes the counterfactual observations. `expected.json` freezes the result:
B and E are `review_required`, D is unaffected, Verifications V_B and V_E are
stale in dependency context, and target-before-source repair batches are B then
E. `negative-vectors.json` freezes 22 missing, unavailable, unaccepted,
incomplete, root-mismatched, duplicate, cyclic, foreign-context,
unsupported-field, and bound cases. The verifier separately exercises carrier
path, symlink, nonregular, mode, size, and root refusals.

The comparison is preregistered before observations. Both arms receive the same
raw facts, state, task, reducer semantics, RO-Crate view, fixture-only signed
review record, environment, and success criteria. Only the treatment receives
`profile.json` as supplemental context. Every observation remains `null` with
status `not_measured`; the primary transitions-per-expert-minute metric is
`not_computable`. The fixture review signature authenticates frozen test bytes
only and has `authority_effect=none`.

Run:

```bash
uv run --project conformance --locked python conformance/verify_claim_dependency_profile.py
```

The Python reader owns full contract and bounded-carrier validation, verifies
the fixture signature and all negative vectors, and invokes the independent
JavaScript canonical/reducer reader. Python and JavaScript independently derive
the custom projection and repair ordering. The Rust integration reader
strict-parses and hashes the frozen bytes, then adapts `requires` into the
existing correction-impact reducer to confirm B/E/D impact. `manifest.json`
binds every frozen data file except itself; including its own digest would be
recursive. Reader source is bound by the Git commit rather than copied into
that data manifest.
The initial fixture generator was deliberately deleted after freezing:
conformance checks committed bytes and roots directly instead of retaining a
second writer.
