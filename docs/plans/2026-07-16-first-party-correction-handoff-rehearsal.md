# First-Party Correction Handoff Rehearsal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete an authority-free, first-party rehearsal of the ADR 0006 graph handoff and matched standards baseline on released Vela `v0.800.22`, then fix only defects reproduced by the rehearsal.

**Architecture:** Add a separate internal-fixture registration and a dependency-free controller under `research/verifiable-composition/`. The controller generates and verifies the registered Grötzsch parent and Mycielski child, records two verifier paths, exercises pending and fixture-authorized handoff states, replays corrections and forks through the existing readers, and emits one canonical result document. It never mutates the independent-participant registration, reads a key, signs, or promotes an ADR primitive.

**Tech Stack:** Python 3 standard library, released Vela `v0.800.22`, Git, the existing ADR 0004 reference readers and standards-baseline fixtures, and optional checksum-pinned SAT/LRAT executables when present.

---

The user requires work directly on `main`; do not create a worktree or branch.
All commits are unsigned agent commits.

### Task 1: Freeze the first-party registration

**Files:**

- Create: `research/verifiable-composition/registration/first-party-handoff-rehearsal-v1.json`
- Create: `research/verifiable-composition/check_first_party_handoff_registration.py`
- Modify: `research/verifiable-composition/README.md`

**Steps:**

1. Write the registration checker first. Require the schema, run class
   `first_party_internal_fixture`, zero authority and independence credit,
   released Vela coordinates, graph-case root, exact input-file roots, fixed
   case order, tool custody, stop rules, and an empty result root.
2. Run
   `PYTHONDONTWRITEBYTECODE=1 python3 research/verifiable-composition/check_first_party_handoff_registration.py`
   and confirm it fails because the registration is absent.
3. Add the registration with these phases in order:
   `parent_generate`, `verifier_v1`, `verifier_v2`, `pending_handoff`,
   `fixture_authorized_child`, `correction_replay`, `reader_parity`,
   `standards_baseline`.
4. Bind the released binary hashes, current source commit, graph registration,
   reference modules, vector files, command caps, and no-key environment.
5. Run the checker and require one printed canonical registration root.
6. Commit the frozen registration before any rehearsal execution.

### Task 2: Generate exact parent and child artifacts

**Files:**

- Create: `research/verifiable-composition/reference/graph_handoff.py`
- Create: `research/verifiable-composition/check_graph_handoff.py`
- Create: `research/verifiable-composition/vectors/graph-handoff-cases.json`

**Steps:**

1. Write failing vectors for graph-root drift, noncanonical edges, a triangle,
   invalid colouring, parent substitution, wrong Mycielski labels, and a
   child not derived from the supplied parent bytes.
2. Implement strict bounded JSON loading, RFC-8785-compatible canonical bytes
   for the registered integer/string subset, adjacency construction,
   triangle checks, deterministic colouring search, deterministic DIMACS
   generation, and the exact Mycielski transformation.
3. Emit canonical parent bytes, a four-colouring, three-colourability DIMACS,
   child bytes, transformation witness, and five-colouring.
4. Require the child checker to hash and consume the delivered parent bytes.
5. Run
   `PYTHONDONTWRITEBYTECODE=1 python3 research/verifiable-composition/check_graph_handoff.py`
   and require all registered vectors to pass.
6. Commit the artifact and checker slice.

### Task 3: Record two verifier paths

**Files:**

- Modify: `research/verifiable-composition/reference/graph_handoff.py`
- Modify: `research/verifiable-composition/check_graph_handoff.py`
- Create: `research/verifiable-composition/reference/graph_handoff_v2.py`

**Steps:**

1. Add a failing parity test that feeds canonical and hostile cases to both
   verifier implementations.
2. Keep V1 adjacency/set based. Implement V2 independently with edge-bitsets
   and a different colouring search order; V2 may import only strict canonical
   JSON helpers, not V1 graph or colouring functions.
3. Record executable source roots, argv, exit code, stdout root, stderr root,
   and duration for each verifier.
4. Require exact agreement on every registered case and zero network or write
   attempts outside the run directory.
5. Run the focused graph check and commit.

### Task 4: Add the authority-free rehearsal controller

**Files:**

- Create: `research/verifiable-composition/run_first_party_handoff_rehearsal.py`
- Create: `research/verifiable-composition/check_first_party_handoff_rehearsal.py`
- Create at run time: `research/verifiable-composition/results/first-party-handoff-rehearsal-2026-07-16.json`

**Steps:**

1. Write the result checker first and confirm it fails with no result.
2. Make the controller refuse a dirty substrate worktree, wrong registration
   root, wrong Vela binary hash, any key-related environment variable, or an
   unregistered phase.
3. Generate and verify Producer A artifacts. Record all roots, commands,
   timings, and verifier parity.
4. Build a pending handoff package from released Vela output or a read-only
   pending fixture. Assert that it is not accepted and cannot satisfy a hard
   dependency.
5. Use the existing explicitly internal, fixture-authorized ADR 0004 profile
   only for the child mechanics. Mark this as simulated authority and grant
   no human or scientific credit.
6. Generate and verify the substantive Mycielski child while proving exact
   parent consumption and parent-substitution refusal.
7. Emit canonical results with `authority_attempts: 0`,
   `human_key_access: false`, `independent_credit: false`, and
   `protocol_promotion: false`.
8. Run the result checker and commit the controller slice.

### Task 5: Replay corrections, forks, and reader parity

**Files:**

- Modify: `research/verifiable-composition/run_first_party_handoff_rehearsal.py`
- Modify: `research/verifiable-composition/check_first_party_handoff_rehearsal.py`
- Modify: `research/verifiable-composition/results/first-party-handoff-rehearsal-2026-07-16.json`

**Steps:**

1. Run the registered same, descendant, scoped correction, verifier
   withdrawal, stale-root, and non-descendant-fork cases.
2. Require deterministic dependency status/code parity between the existing
   reference reader and the separately implemented Reader C path.
3. Require `child_truth` to remain `not_assessed` in every case.
4. Run all hostile fact-manifest and standards-baseline vectors.
5. Record the exact status distribution, commands, wall time, repair count,
   and intervention count.
6. Run:

   ```bash
   PYTHONDONTWRITEBYTECODE=1 python3 research/verifiable-composition/check_first_party_handoff_rehearsal.py
   PYTHONDONTWRITEBYTECODE=1 python3 research/verifiable-composition/check_fact_manifest_projections.py
   PYTHONDONTWRITEBYTECODE=1 python3 research/verifiable-composition/check_standards_baseline.py
   ```

7. Commit the replay and reader slice.

### Task 6: Decide whether any candidate ADR is evidenced

**Files:**

- Create: `research/verifiable-composition/results/first-party-handoff-gap-report-2026-07-16.md`
- Modify: `docs/adr/0006-independent-correction-aware-handoff-and-standards-baseline.md`
- Modify only if evidence requires clarification:
  `docs/adr/0007-full-digest-claim-revision-references.md`
- Modify only if evidence requires clarification:
  `docs/adr/0008-signed-frontier-checkpoint-continuity.md`
- Modify only if evidence requires clarification:
  `docs/adr/0009-exact-dependency-pins-and-standing.md`

**Steps:**

1. Compare the Vela-profile and standards-profile command count, context
   bytes, wall time, repairs, and semantic outcomes.
2. Classify each candidate gap as `not_reproduced`, `reproduced_existing_profile`,
   or `reproduced_missing_invariant`.
3. Do not implement ADR 0007, 0008, or 0009 unless the result contains a
   registered `reproduced_missing_invariant`.
4. State that first-party success cannot satisfy the independent or human
   gates in ADR 0006.
5. Run `git diff --check` and the focused research checks.
6. Commit and push the evidence on `main`.
