# Independent sealed-capture pre-score audit

## Verdict

**PASS** for producer commit
`5694bebac03b062d6acdce5a2a900551850e6a1c`, tree
`feec0ff21b9b13be8cbb97083f441ef66bdd48f2`.

This verdict confirms only that the complete 36-cell participant capture is
sealed, internally complete, bound to the independently qualified successor,
and suitable to proceed to a separately authorized protected-key/scoring
stage. This review did not access the protected adjudication key, score any
response, release a permit, make a provider call, merge, or exercise Vela
authority or Standing.

## Immutable subject and lineage

- Live `origin/codex/inherited-correction-study` resolved exactly to the
  reviewed producer commit and the requested tree.
- The calibration evidence head
  `2c24b9f59c191972d31c142f65c7a905b9cae7ac`, tree
  `66f554175f6031aced579506277d09ff05eca59e`, is the exact launch-base
  ancestor. Its direct successor is the run-01 retention commit; 36 one-run
  retention commits and one final freeze commit lead to the reviewed head.
- The reviewed commit's immediate parent is the expected run-36 retention
  commit `ed52ac1c683ce77e5b0153ee1c67ea6fb8ce417d`.
- Relative to the calibration head, all 363 added paths are confined to
  `paper/artifacts/inherited-correction-held-out-order-replacement-execution/`.
  No registered packet, prompt, schema, benchmark, custody implementation,
  assignment, runtime, image, gate, commitment, Core, Protocol, Standing, or
  authority byte changed.
- The independently pushed PASS reviews remain remote-reachable and correctly
  bind the successor repair (`0093c6bf7c530009d72a69cfde3586c45ab24072`),
  its neutral calibration (`815ebb0364135da603659b877938d519ebb7d95c`),
  and the final runtime/image repair
  (`4a06ac8aa9a5f07abd019a375d755bfe5f0031aa`).

## Complete capture and custody

Independent recomputation from the 36 ingested run directories produced the
checked-in complete-custody object byte-for-byte and confirmed:

- complete capture root:
  `sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`;
- complete custody root:
  `sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`;
- registration root:
  `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`;
- assignment root:
  `sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`;
- runtime root:
  `sha256:3f7a753141306771b05c582d1c0ff30489cdb8a35c556e21ac5fdabb9a431ba8`;
- image manifest digest:
  `sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1`.

The outer capture manifest has 362 unique entries and exactly covers every
capture file except itself. Every recorded byte length and SHA-256 digest
matches the repository bytes. Removing only the manifest's self-root and
canonicalizing the remainder reproduces the registered complete capture root.

All 36 expected run IDs and 36 participant instance IDs are unique. All 36
runtime custody roots and consumed-permit byte digests are unique. Each run is
attempt 1, terminal `completed`, and has exactly one authorized consumed permit
whose frozen identity matches its assigned held template. The unconsumed
counterpart is absent from the captured launch directory, the launch receipt
binds the consumed bytes, and permit consumption precedes provider start.

The denominator and balance independently recompute as:

- 36/36 terminal completed runs;
- 12 `git-documents`, 12 `state-wrapper`, and 12 `vela`;
- 12 `provenance-revocation`, 12 `taxonomy-remap`, and 12
  `method-version-correction`;
- four cells for every family/condition pair;
- zero missing or duplicate run identities, retries, or substitutions.

For every run, the retained raw response exactly equals `response.json` and its
receipt-bound byte digest. Each provider event file contains one thread start,
one turn start, one agent message, and one turn completion; the terminal
receipt records one response, one turn, zero tools, and zero compactions.
Provider stderr is empty for all 36 runs. Every process exits zero without a
timeout or validation error. Every teardown record reports no remaining
container and no retained credential.

The independently summed duration is `496.941506686` seconds. Usage sums to
326648 input, 39168 cached input, zero cache-write input, 21637 output, and
7766 reasoning-output tokens, exactly matching the sealed summary.

The sealed summary and unchanged result record establish protected-key access
0, adjudication access false, scoring runs 0, and `positive_gate=not_evaluated`.
No protected plaintext or scoring output is present in the reviewed delta.

## Fail-closed pre-score boundary

The complete-custody verifier was rerun directly on the sealed run set. Twelve
independent mutations of run 01 were then tested without invoking the scoring
entry point or opening an adjudication file:

- missing, byte-drifted, and duplicated terminal receipts were rejected;
- missing, byte-drifted, and duplicated consumed permits were rejected;
- missing, byte-drifted, and duplicated retained responses were rejected;
- missing, byte-drifted, and duplicated provider event streams were rejected.

The failures occurred at the evidence-set, launch binding, byte-root, response
snapshot, or provider-event binding gates. Thus these mutations cannot reach
the protected-key/scoring boundary.

## Focused deterministic checks

- `benchmark.py verify`: PASS, held registration and roots recomputed.
- `custody.py verify-prelaunch`: PASS for the immutable registered templates.
- `test_benchmark.py`: PASS, 24 tests.
- `test_provider_schema_runtime.py`: PASS, 9 tests.
- Complete custody recomputation: byte-equal under CPython 3.10, 3.11, 3.13,
  and 3.14 available in the isolated checkout.
- Exact loaded image offline provider-schema preflight: PASS with container
  network disabled and zero-byte provider events and stderr.
- Capture file-set, byte-length, digest, duration, usage, balance, identity,
  response, event, receipt, permit, and teardown assertions: PASS.
- `git diff --check`: PASS.

## Boundary

This PASS is a pre-score custody verdict only. It does not itself authorize or
perform protected-key access, scoring, a provider call, permit release, merge,
scientific acceptance, a Vela Decision, Repository authority, or Standing.
