# Narrow F05/G07/G08 re-review: capture-bound scoring snapshot

## Verdict

**PASS**, scoped only to F05, `G07_deterministic_scoring`, and
`G08_deterministic_custody` at producer commit
`7596c12291c22a5b4b81a1ab1eb49189318f57de`, tree
`af67f32ebfb97990fe4faaac90176880f6cfb0b6`, whose sole parent is the
blocked producer commit `f919654a7e55d336c5588719e962e2d67a699710`.

This resolves F05 from independent review commit
`4dd3efc45d222e3d41defef38a5ce8306a1c6391`. The exact corrective commit
qualifies the prospective capture-to-score byte custody and retains the
previously passing F04 runtime-custody bridge.

This PASS does **not** release either hold file or authorize a permit
consumption, participant/provider/model call, paid inference, any of the 16
confirmatory sessions, protected-adjudication access outside an exact complete
capture, merge, positive-lift or scientific claim, or any Protocol, Core,
Standing, authority, or Decision effect. Confirmatory status remains 0/16,
`not_run`.

## F05 closure

The scoring entry point now:

1. recomputes and validates complete v2 runtime custody and the committed
   capture manifest;
2. takes the capture object returned by that validation rather than reopening
   the manifest;
3. opens every capture-listed `run.json` and registered `response.json` through
   a no-follow file descriptor, requires a regular file, and reads it once;
4. compares each immutable byte buffer with the exact digest in the capture;
5. parses and compares the capture-bound record fields, rejects unexpected
   responses, and independently derives the capture root from the buffered
   snapshot;
6. opens adjudication only after all 16 run/response buffers and the derived
   capture root pass; and
7. scores only JSON parsed from those buffered bytes, without reopening a live
   run or response path.

The snapshot iterates the capture's exact unique run identities, whose manifest
was already regenerated from the fixed 16-cell denominator and complete
runtime custody. A path mutation before buffering fails on its exact digest; a
path mutation after buffering cannot influence the score.

## Independent F05 adversaries

The prior exploit was rerun against 16 complete offline v2 captures with the
real adjudication path replaced by a nonexistent temporary path:

- a structurally valid post-verification response mutation failed with
  `capture_response_bytes_drift`;
- the protected-key tripwire was called zero times;
- the committed test also proves structurally valid post-verification
  `run.json` drift fails with `capture_run_bytes_drift` before the key.

The inverse boundary was tested independently. All 16 run files and all 16
responses were read exactly once (32 reads total) before the scoring-gate hook.
The hook then changed a live response path and returned an in-memory synthetic
adjudication object. The score completed from the original immutable buffers,
performed zero additional snapshot-file reads, retained the derived capture
root, and produced unchanged 128-point arm totals. No protected adjudication
was opened.

These controls establish that bytes not present in the validated capture can
neither open the key nor affect the scored result.

## Immutable subject and frozen identities

- The pushed remote ref resolved exactly to the handed-off commit, tree, and
  parent.
- The diff contains exactly 34 paths, all within the benchmark and prospective
  confirmatory prelaunch artifacts.
- Pilot, canary-01/02/03, container runtime, protected adjudication, packet,
  prompt, response schema, fixture/source evidence, Core, Protocol, Standing,
  authority, and Decision bytes are unchanged.
- `benchmark.py` bytes match
  `sha256:b14e955c9d77510f0c3b961c054440fbb23179137349e64ca9e7a301ac55071d`;
  unchanged benchmark tests match
  `sha256:2947908b9efdf3b6a1007ed5ae3c5941890ffa7a9d9a26845319ef92c7923e58`.

Independent recomputation matched all disclosed roots, including:

- benchmark registration:
  `sha256:a18f067652883680b39aea269e66fb4a833b8466790d7031abf2e4ab3748a3d2`;
- benchmark amendment:
  `sha256:2c3d82e5bfb56760c1d94ddc093732d18d5010774cbedbce2a369184396141c3`;
- runtime registration:
  `sha256:345d54e2bc8c76eb66a8938a03f26001ea4f953c04bf58d9c8cd7f8ad039d1b2`;
- prelaunch canonical root:
  `sha256:ccdf6ba7dacaaa326057f819cbfd3cfe03970916fe4ed9179c4b7915551592ca`;
  and bytes
  `sha256:9ec7fa75f0acdf091e332bf1d8dd9f97618cb96626e2927aba8b8891480abc66`;
- 32-entry artifact manifest:
  `sha256:1b1525fe67216463f0f99424a6432ebee64155084cf5fbe4d172cfea29ce42cc`;
- scoring bindings:
  `sha256:1f40fdd938161839a0fed6d8509a23d443e37140162fc1ce44c3cc0fa8826ad0`;
- assignment:
  `sha256:33ff47e1df10bca0ab7d0756b762108564138ae438142a3d00a8ffe9fca15368`;
- authorization:
  `sha256:468ffca20b33f489859b7173bc4e96e012c1385032abca0457e115b5675e4f2e`;
- shared participant configuration:
  `sha256:ed944c27285871bb39eb6ef80085824fa7dd1245d792a6a509b4723a882abaf4`;
- authorized mapping:
  `sha256:b061605a16c9c3c1dbb78f746c28dbd87d6b065ad5c5daa7f5b19ea6e81944e0`;
  and
- permit set:
  `sha256:31360933465b51bd7f39caca7889305234391958e652942cc0a9d7e69a7a1ecb`.

The registration retains blocked review
`4dd3efc45d222e3d41defef38a5ce8306a1c6391`, its producer, registration,
freeze root, and zero-call disposition. The seed, packet, prompt, image, trust,
and protected-adjudication roots remain unchanged.

## Deterministic checks

Each disclosed command produced independent passing receipts from a fresh
detached checkout:

- Ruff 0.12.11 check and format check on the six specified Python files;
- locked event-contract wrapper;
- 11 container-runtime tests;
- 10 confirmatory-custody tests, including both complete 16-cell gate-boundary
  mutation controls;
- 6 confirmatory-prelaunch tests, including isolated byte-exact regeneration;
- benchmark verification;
- 16 benchmark tests;
- confirmatory-custody prelaunch verification; and
- `git diff --check`.

A deliberately parallel review-harness rerun briefly overlapped `npm ci`
replacement of ignored `node_modules` with the custody test's runtime-source
traversal and produced a transient `FileNotFoundError`. Running the disclosed
event wrapper followed by the custody suite—the registered command order—passed
cleanly, as did a further standalone custody run. No frozen or tracked byte was
involved, and this does not affect the narrow F05 result.

Both holds are still `hold`; provider calls are zero, `permits_consumed` is
empty, and the scheduler is `none`. No participant, provider/model call, paid
inference, OAuth access, permit consumption, real scoring, protected-key read,
human study, merge, authority action, Standing mutation, or Decision was
performed during review.
