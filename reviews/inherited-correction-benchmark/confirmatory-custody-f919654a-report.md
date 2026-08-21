# Independent F04/G07/G08 re-review: confirmatory custody repair

## Verdict

**BLOCKED**, bound to producer commit
`f919654a7e55d336c5588719e962e2d67a699710`, tree
`352d7796e09bf4339c9296c44192c3a0d0e71331`, whose sole parent is the
previously blocked prelaunch commit
`b4bd2542b2fb71944a0d1e7e487b007392c008b6`.

The repair closes the original F04 bridge defect: an exact 16-cell capture now
requires bridge-generated v2 records and revalidates each consumed permit,
launch record, terminal receipt, raw provider event stream, stderr, runtime
response, condition configuration, authorized shared-to-condition mapping,
packet, prompt, image, trust bundle, runtime source, assignment, timing, usage,
attempt, and status. Synthetic legacy v1 records no longer produce a capture.

Prelaunch remains blocked by one narrow G07/G08 defect in the scoring entry
point. No participant may be launched under this verdict. This review does not
authorize a permit release, provider call, paid inference, merge, protected
adjudication access, scientific claim, or any Standing, authority, or Decision
effect. Status remains 0/16, `not_run`; both hold files remain `hold` and all
permits remain unconsumed.

## F05 — scoring can consume response bytes that are not in the frozen capture

`score_runs` verifies the committed capture at `benchmark.py:1019` and then
opens the protected adjudication gate at line 1022. Only afterward, at lines
1024–1030, it re-reads each mutable `run.json` and `response.json` from disk.
Those post-gate reads are not compared with the capture-manifest digests before
they influence the score.

Therefore a response changed after the pre-key verification can be scored even
though its bytes no longer equal the frozen capture. The scored result still
publishes the old `capture_root`, so the result can claim one capture while
being calculated from different response bytes. This violates
`G07_deterministic_scoring` and `G08_deterministic_custody`.

### Minimal deterministic reproduction

The review built all 16 runner-shaped bridge captures using the committed
offline control, ingested them through `confirmatory-custody.py`, wrote the
result of `benchmark.capture_manifest`, and patched the adjudication path to a
nonexistent temporary file. A test-only scoring-gate hook changed one valid,
closed `response.json` after `verify_capture_manifest` returned and supplied an
in-memory synthetic adjudication object. The real protected adjudication path
was never opened.

`score_runs` returned a scored result even though:

- frozen response digest:
  `sha256:c1ee1533b583f517e1b69f49dcdff15f283f463f4f64d12a5bfa0ad2de60ae12`;
- response digest actually read by scoring:
  `sha256:211cf71c89c6b265fe76fa702c54f228646647ca8669978f7e18272042cc2d5f`;
- the two digests were unequal; and
- the returned result remained bound to the previously computed capture root.

The reproduction changes only a closed action code, so the post-gate response
remains structurally valid and can alter component points or exact-success
status rather than merely causing a parser failure.

### Minimal prospective repair

Before opening protected adjudication, load and validate the exact run and
response bytes into an immutable in-memory snapshot whose digests equal the
capture manifest, then score only that snapshot. Add an adversarial test that
changes a response after the capture verification boundary and proves that the
changed bytes cannot affect or produce a scored result. A final recheck alone
is weaker than scoring the already-validated bytes because another mutation
window remains.

This is prospective: zero confirmatory calls or permit consumptions have
occurred, so no experimental result needs repair or exclusion.

## Evidence that passed

### Immutable subject and exact scope

- `origin/codex/inherited-correction-study` resolved exactly to the handed-off
  commit, tree, and parent.
- The diff contains exactly 37 paths, all under the inherited-correction
  benchmark and execution artifacts.
- Source hashes and executable modes match the handoff.
- Packet prompts, response schema, source/evidence fixture, protected scoring
  key, positive gate, runtime/container, pilot, canaries, Core, Protocol,
  Standing, authority, and Decision bytes are unchanged.

### Recomputed frozen identities

Independent recomputation matched:

- benchmark registration:
  `sha256:78cc6e0154ec1baaa4eb86a15131ad9910802a5a2aeae2b82779de3c3ba6e67b`;
- runtime registration:
  `sha256:aaa951dc6cb34c6b86e5a5a096e974ac51c56ba9c02be6690e5fa1bdc7c77a5e`;
- assignment:
  `sha256:3b4ad819d74e81408274d2e836f03008184028e1343cb2b4c7b1c863411c206a`;
- authorization:
  `sha256:0c1515b2f913429da3a902e9fe71645d6952125632aca601dfac4da0323719e2`;
- shared configuration:
  `sha256:c4447adbcd27442e80b97b1881c9cb9300e457a06d9ba787babfbe9a254278bf`;
- authorized mapping:
  `sha256:e5f4e11369661d5059ff2f69b099903ccd70dad1e6987d27b4b1088c0700453b`;
- permit set:
  `sha256:c59cdb037bbbe9d4258554d23a392607b2c27fbe74e007d141b7d58b58068801`;
- 32-entry artifact manifest:
  `sha256:301e5230d4706e600cca49a4ab514823f33ca7de94a274d3bb1b33daf9017b56`;
- prelaunch canonical root:
  `sha256:db5f37730f8f0d0291c9bab467acc58a1c3f58210d662fa748ad8990f7aba37b`;
  and bytes
  `sha256:4995f7497bba345606c54ffe61e206b726a3ff6f92595ef702ef08342e2b70c9`;
- scoring bindings:
  `sha256:c694880a9d25fe4b6fd7ae05468a3bf3a2b0f02fa80050536618e06c6778e213`;
  and all disclosed condition, packet, prompt, runtime, image, trust, and seed
  roots.

The replacement registration explicitly retains the blocked b4bd prelaunch,
review commit `23986b9b02ea5ba1324cd9aad91545f969db8a56`, and zero observed
confirmatory calls.

### Deterministic checks

The exact nominal command set passed independently from a fresh detached
checkout; the Python, event-contract, regeneration, benchmark, runtime, and
diff checks passed twice:

- Ruff 0.12.11 check and format check on all six changed Python files;
- locked event-contract wrapper;
- 11 container-runtime tests;
- 9 confirmatory-custody tests, including the exact 16-cell positive control
  and synthetic-record rejection;
- 6 confirmatory-prelaunch tests, including isolated byte-exact 32-file
  regeneration;
- benchmark verification;
- 16 benchmark tests; and
- `git diff --check`.

No participant, provider/model call, paid inference, OAuth access, permit
consumption, protected-adjudication read, canary, scoring of experimental data,
human study, merge, authority action, Standing mutation, or Decision was
performed during review.
