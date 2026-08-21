# Independent prelaunch review: replacement confirmatory study

## Verdict

**BLOCKED**, bound to producer commit
`b4bd2542b2fb71944a0d1e7e487b007392c008b6`, tree
`67924828f98dcd921a029549e0ba82a4db275d41`, whose sole parent is the passed
runtime/canary producer commit
`4c7bd6a811bbd0cf1ebd357d3ad72abb9127442a`.

The registration, assignment, arm inputs, runtime configuration, permits,
holds, manifests, and documented roots are internally consistent and
deterministically reproducible. Prelaunch is blocked by one custody defect:
the frozen package does not bind actual container terminal evidence into the
unchanged benchmark capture/scoring path.

No participant may be launched under this verdict. It does not authorize a
permit release, provider call, paid inference, merge, scoring access,
scientific claim, or any Standing, authority, or Decision effect. Status
remains 0/16, `not_run`; both hold files remain `hold` and all permits remain
unconsumed.

## F04 — runtime terminal evidence is not bound into capture or scoring

The study-level authorization fixes participant configuration root
`sha256:46b3325497d42a1d265890013fe310a474f1dc8c60315e8f10d575c0d6e0f9ec`.
The actual container runtime instead validates and emits one of two
condition-specific roots:

- Git/documents:
  `sha256:ba36d835b5e0d0b01bffe3f882e594a33d198f34e58670e988f7f41f342359f1`;
  or
- Vela:
  `sha256:7baec83c5d644bc4472e8348023be04c26e378b798fb992455bf832dc60c48f0`.

Neither condition root equals the study root. `run-once.mjs` writes the
permit's condition root into `terminal-receipt.json`, while the unchanged
benchmark `start` and frozen-run validator require every run record's
`participant_configuration_root` to equal the single authorization study root.
The capture manifest additionally rejects non-fixed roots.

Therefore no benchmark run record can both carry the actual terminal root and
pass the benchmark validator. Recording the study root makes the benchmark
capture pass, but drops the binding to the runtime configuration actually used.

More importantly, no new confirmatory file ingests or validates a terminal
receipt, provider-event bytes, consumed permit, launch receipt, runtime response
bytes, or their relationship to a benchmark run record. Independent
reproduction used the committed benchmark test helper to create 16 valid run
records, confirmed that zero `terminal-receipt.json` files existed, and then
successfully produced `benchmark.capture_manifest(runs)`. Thus the unchanged
scoring gate can be opened without any of the 16 terminal captures that the new
registration says are required.

This blocks `G07_deterministic_scoring` and `G08_deterministic_custody` for the
confirmatory launch package. The nominal prelaunch tests check the two halves
separately but do not test or enforce this bridge.

### Minimal prospective repair

Before any permit release, freeze and independently test a deterministic
runtime-to-benchmark capture bridge. It must require exactly one terminal
receipt and consumed permit per assigned run and validate at least:

- run, participant, condition, attempt, assignment, packet, prompt, image,
  trust, runtime-registration, and condition-configuration roots;
- provider start/completion, duration, terminal status, response bytes,
  provider-event bytes, tool/turn/compaction counts, and credential-retention
  state;
- the condition root's membership under the authorized fixed study root; and
- exact hashes of all runtime evidence in a pre-scoring capture root.

The scoring entry point must fail closed unless that execution capture root is
present and byte-matches the benchmark run/response capture. Tests must show
that missing, substituted, cross-condition, or root-drifted terminal evidence
cannot reach scoring. This repair is prospective because 0/16 calls have
occurred; it does not require or authorize another calibration canary.

## Evidence that otherwise passed

### Immutable scope and prior lineage

- `origin/codex/inherited-correction-study` resolved exactly to the handed-off
  commit and tree.
- The diff adds exactly 34 files: the generator, its test, and the 32-file
  `confirmatory-study/` tree.
- No pilot, canary, runtime, benchmark, Core, Protocol, Standing, authority, or
  Decision byte changed.
- Generator and test hashes match the handoff.

### Registration, fairness, and hold state

- The independently recomputed schedule exactly follows the disclosed seed
  algorithm, has 16 unique run IDs, 16 unique participant IDs, and eight fixed
  cells per arm.
- Both prompts are exact path-preserving serializations of their unchanged
  registered packet plus the same response schema. Each contains 16 virtual
  files; protected adjudication and Claim-to-action mappings are absent.
- Both runtime configurations are byte-identical except for the registered
  prompt root. Model, provider, reasoning, service tier, image, trust,
  overrides, timeout, output ceiling, attempt, retry, tool, and one-turn
  controls are fixed.
- All 16 permits bind the exact run, participant, condition, assignment,
  condition configuration, prompt, packet, image, trust, and attempt.
- Both hold files are `hold`; there is no capture directory or consumed permit.
  An exact held permit failed with `launch_on_hold`, and a released copy paired
  with the wrong condition input failed with `permit_configuration_root` before
  consumption or provider evidence.

### Roots and deterministic reproduction

Independent recomputation matched all handed-off roots, including the
benchmark and runtime registrations, prelaunch bytes/canonical root, artifact
manifest, seed, assignment, study and condition configurations, authorization,
permit set, packets, prompts, scoring bindings, runtime source, image, and
trust bundle.

Two isolated generator runs reproduced all 31 manifest entries plus
`prelaunch-freeze.json` byte-for-byte, 32 files total. The exact nominal command
set was run twice from a new detached checkout and passed both times:

- Ruff 0.12.11 check and format check;
- locked event-contract wrapper;
- 11 runtime tests;
- 6 confirmatory-prelaunch tests;
- benchmark verification;
- 15 benchmark tests; and
- `git diff --check`.

No provider, model, participant process, paid inference, OAuth secret,
experimental or candidate scoring operation, human study, merge, authority
action, Standing mutation, or Decision was performed during review.
