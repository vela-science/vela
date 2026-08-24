# Prospective provider-schema compatibility assessment

Status: design assessment only. No runtime, schema, packet, prompt, permit, registration, or participant bytes are changed by this document. No new provider call is authorized.

## Observed blocker

The one authorized attempt for `heldout-run-01` reached the provider exactly once. The provider returned HTTP 400 with `invalid_json_schema` before generation because `uniqueItems` is not accepted at `properties.evidence_bindings`. The attempt is retained as the sole terminal non-result in the stopped 36-cell registration. Runs 02 through 36 are unissued and must not be released.

## Smallest semantics-preserving option

A prospective runtime adapter can derive a provider-only structured-output schema from the frozen Draft 2020-12 response schema by deleting exactly the single keyword at JSON Pointer `/properties/evidence_bindings/uniqueItems`. The registered response schema, participant-visible serialized packet, prompt, response contract, packets, facts, assignments, gates, and scorer remain unchanged. The provider-only derivative is used only as the `codex exec --output-schema` argument. After the single response returns, the existing locked Ajv Draft 2020-12 validator must validate the response against the unchanged registered schema, and `benchmark.validate_response` must continue to require exactly four distinct, complete path-and-digest bindings. Duplicate bindings therefore remain a terminal non-result and cannot be scored.

This option is semantics-preserving only if deterministic qualification proves all of the following:

- the derivative differs from the registered schema by exactly one deleted `uniqueItems: true` keyword;
- every other key, value, array order, enum, bound, pattern, and required field is byte-semantically identical;
- the registered schema and its roots remain the participant and scientific contract;
- duplicate bindings pass at most the provider surface but fail both local Ajv validation and the benchmark validator before custody marks a run completed;
- missing, extra, malformed, wrong-path, wrong-digest, and incomplete bindings still fail closed;
- the custody receipt binds both the registered schema root and the provider-derivative root;
- the one-turn, no-tool, fixed-timeout, output-bound, auth, image, trust, and append-only custody invariants remain unchanged.

## Required prospective treatment

Do not amend or continue the stopped registration. A replacement registration needs fresh run IDs, participant IDs, external seed and balanced assignments, unused permits, held state, updated runtime/image/configuration/custody roots, and a transparent amendment binding the stopped run and this compatibility repair. The scientific packets, participant prompts, closed registered response schema, scorer, gates, family facts, and three-arm design should remain exact.

Before any replacement participant call, qualification should include:

1. Offline exact-diff and adversarial tests for the provider-only derivative and local uniqueness enforcement.
2. The actual pinned container entrypoint and locked dependencies, including a fail-closed check that it never uses the derivative for final local validation.
3. If separately authorized, one neutral schema-surface calibration call containing no participant packet or study facts, proving the provider accepts the derivative. It is not a study session or retry and cannot enter a denominator.
4. Immutable commit and independent review of the amendment, image, roots, held permits, custody bridge, tests, and any calibration receipt.

No implementation, calibration call, replacement registration, permit release, protected-key access, or scoring is authorized by this assessment.
