# Independent held-out stopped-state and prospective compatibility review

## Verdict

**PASS**, bound to producer commit
`d3bff9206609c53a0dc9b2ef7f85bbdc894a9904`, tree
`212c90ce47727bdabc027d727c2554dff51f5527`, whose sole parent is the
immutable run-01 freeze `3bce5d955ba69ae4cdb790422943b183a0e98058`.

This PASS qualifies only the truthfulness of the stopped-state record and the
methodological sufficiency of the proposed prospective boundary. It does not
qualify an implementation, provider-schema derivative, runtime image,
calibration, replacement registration, participant call, permit release,
scoring, merge, or any Core, Protocol, authority, Standing, or Decision action.

The original registration is stopped after one terminal non-result. Runs 02
through 36 must remain unissued. The compatibility document is a design
assessment, not an executable launch gate; present safety also depends on the
unchanged source hold and the absence of any released permit. Any repair must
be implemented and independently reviewed in a fresh replacement registration
before another study call.

## Immutable scope

The remote ref reconstructs exactly at the handed-off commit, tree, and parent
and is remote-equal. Live `origin/main` remains independently at
`4685462c44b1f073870f31025ae73d1d8770ce73`.

The diff adds exactly two files under the non-authoritative execution artifact:

- `stopped-after-run-01.json`, SHA-256
  `f6814ac506063e67777d81f5df11996c21e1c76fea06efbac7f93d2e4cf92d40`;
  and
- `prospective-schema-compatibility-assessment.md`, SHA-256
  `65e98a1f4be431502cf993bc2b58b81e90f087ecfebe70d26e871ec7cf7a40e4`.

No runtime, packet, prompt, response schema, permit, registration, scorer,
protected adjudication, Core, Protocol, authority, Standing, or Decision byte
changes in this commit.

## Stopped-state reconstruction

Independent validation of the committed capture and ingested run proves:

- `heldout-run-01` is the sole capture and sole ingested run among 36 assigned
  cells;
- it is Vela / method-version-correction / `heldout-sol-01`, attempt one;
- the exact consumed permit, launch, receipt, event stream, and empty stderr
  validate against the registered assignment, condition configuration, prompt,
  packet, image, trust bundle, and runtime roots;
- the terminal receipt is `non_result`, the bridged run is `failed`, and no
  response file or response digest exists;
- four provider events are retained: one thread start, one turn start, one
  provider error, and one failed turn; there is no agent message or completed
  turn;
- the provider diagnostic is HTTP 400 / `invalid_json_schema`, reporting that
  `uniqueItems` is not permitted in the `evidence_bindings` property;
- runtime custody recomputes exactly to
  `sha256:ab00eb1cb19474a3ed1306eaa20ceaa8ab784f62ffed268e1e97b4f6a9ea017f`;
- terminal-receipt and provider-event bytes recompute to
  `sha256:70c861d7161176f780bc3fd5df9eb98e43441e5da32bc0424e9a38f6add48df0`
  and
  `sha256:bad3ae5a2a6b8452da003a0c20d3a229f1a9ff85d04e90e3739a33ba4a263ad9`;
- credential retention is false, retries and substitutions are zero, and
  replacement credit is false; and
- runs 02–36 have no capture or run directory, no scoring or capture manifest
  exists, and the source hold remains `hold`.

The stopped counts are therefore accurate: 1/36 terminal, one consumed
execution permit, 35 unissued cells, 35 held/unissued execution permits, one
provider execution, zero retry/substitution, zero protected-key access, and
zero scoring runs. The immutable held templates remain source material rather
than evidence of another consumption.

## Prospective compatibility boundary

The assessment identifies the smallest plausible semantics-preserving adapter:
derive a provider-only schema by deleting exactly
`/properties/evidence_bindings/uniqueItems`, while retaining the registered
Draft 2020-12 response schema byte-for-byte as the participant-visible and
scientific contract.

That boundary is methodologically sufficient as a candidate because it
requires all constraints lost at the provider surface to remain fail-closed
before completed custody:

- the locked Ajv Draft-2020 validator must validate the returned response
  against the unchanged registered schema;
- `benchmark.validate_response` independently requires exactly four distinct,
  complete, packet-valid path/digest bindings;
- duplicates and missing, extra, malformed, wrong-path, wrong-digest, or
  incomplete bindings remain terminal non-results;
- custody must bind both the registered-schema root and the provider-derivative
  root;
- an exact one-keyword diff and actual entrypoint use must be tested offline;
  and
- the derivative must never become the final local validator or participant
  contract.

The stopped registration must not be amended or continued. A replacement
requires fresh run and participant IDs, a fresh external seed and balanced
assignments, unused permits, held state, updated runtime/image/configuration and
custody roots, an explicit stopped-run amendment, and another independent
prelaunch PASS. Any neutral provider-surface calibration is outside the study
denominator and separately requires authorization; it cannot be treated as a
retry or replacement credit.

This review makes no claim that the provider accepts the proposed derivative.
Only a separately authorized neutral calibration and subsequent immutable
qualification can establish that operational fact.

## Focused checks

From a fresh detached checkout, the following passed without a provider call
or protected-key access:

- exact remote commit/tree/parent and two-file scope;
- new-file SHA-256 recomputation;
- capture validation and full ingested-run custody reconstruction;
- stopped denominator, permit, response-absence, and scoring-absence checks;
- held-out benchmark verification and all 19 deterministic tests; and
- `git diff --check`.

This PASS is not a benchmark result, continuation approval, or scientific or
authority claim.
