# Narrow F03/G08 re-review: inherited correction runtime

## Verdict

**PASS**, scoped only to F03 and `G08_deterministic_custody` at corrective
producer commit `4c7bd6a811bbd0cf1ebd357d3ad72abb9127442a`, tree
`006eec100cff769b2b378d96ea4b9ad4c2530191`, whose sole parent is the blocked
result commit `e5f11cc58b3675f171cab2fc57b5714b330152f3`.

This resolves F03 from independent review commit
`a80d72bc5286fe7b0fd4df99570544165aec75b6`. The exact corrected commit now
qualifies the previously substantively passing container runtime and canary-03
calibration evidence. No canary, provider, or experimental session was rerun.

This PASS does **not** authorize or freeze a replacement confirmatory
registration, any of the 16 sessions, another canary, paid inference, human or
scientific validation, a merge, or any Standing, authority, or Decision effect.
Confirmatory status remains 0/16, `not_registered` / `not_run`.

## Corrective diff

The pushed remote ref resolves exactly to the handed-off commit and tree. The
diff from its blocked parent contains exactly three paths:

- `freeze-canary-03.py`: Ruff 0.12.11 formatting plus Git mode `100644` to
  `100755`, SHA-256
  `142ae62774111dcff38d47e47ec975659c92379e0d6cb52dd491b7e3a178ddc7`;
- `test_container_runtime.py`: Ruff 0.12.11 formatting, SHA-256
  `d465668e49347b364456873b82612c425cc4f9b25681d1b89b5082eb2a1f1ab3`;
  and
- executable `check-event-contract.sh`: locked dependency restore followed by
  the event test, SHA-256
  `571e7304508a1e34782507e35cec45a75be9bf8df9f1a12deaa05300f8720ed3`.

Independent Python AST comparison against the blocked parent is identical for
both formatted files. The wrapper is valid POSIX shell, changes to its own
directory, runs `npm ci --ignore-scripts` from the committed lockfile, and then
runs `node test-events.mjs`.

## Unchanged experimental bytes

The canary and runtime tree IDs are identical between the blocked parent and
the corrective commit:

- canary-01: `6ac6d05b4a71761758dafe5395c3095e12c746f5`;
- canary-02: `4685b2985884d5676bc9d3c6b99964a7cfc1bbe2`;
- canary-03: `1dd121c9c1d843576a2fbc98091270edfa5b0bfe`;
  and
- `container-runtime`: `b5df0ebb3127874857d5a3bd6e305db183be13c2`.

Thus all frozen canary inputs, raw receipts, provider events, runtime/image
source, permit evidence, and documented roots remain unchanged. The corrective
commit adds no Core, Protocol, Standing, authority, or Decision byte.

## Independent deterministic reproduction

All commands were run twice from a new detached checkout of the exact
corrective commit and passed both times:

- Ruff 0.12.11 check: `All checks passed!`;
- Ruff 0.12.11 format check: `2 files already formatted`;
- executable event wrapper: five locked packages restored and
  `event contract tests passed`;
- runtime suite: 11 tests passed;
- benchmark verification: `inherited-correction benchmark: verified`;
- benchmark suite: 15 tests passed; and
- `git diff --check`: passed.

Two independent isolated invocations of the formatted prelaunch generator each
recreated all 35 manifest entries plus `prelaunch-freeze.json` byte-for-byte,
36 files total. The prelaunch SHA-256 remains
`1405e33a4c5627c510d85174f602b6951c7b2c17e3903a28708867299682e1c4`, and
the complete previously reviewed root set is unchanged.

No provider, paid model, human study, canary, merge, authority operation,
Standing mutation, or Decision action was performed during re-review.
