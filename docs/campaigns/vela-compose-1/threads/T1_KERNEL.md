# T1 — Kernel + Conformance worker contract

Read root `AGENTS.md`, `README.md`, current protocol/architecture/boundaries,
and all VELA-COMPOSE-1 controls before work.

## Objective

Audit and prove the minimal current governed transition kernel. Treat the live
Protocol 1 model as presumptively correct; reproduce a concrete gap before
changing semantics.

## Ownership

- `crates/vela-protocol` current objects/events only where a proven invariant is
  missing;
- focused kernel lifecycle/conformance tests in `crates/vela-cli/tests` and
  `conformance/`;
- a lane report under this campaign directory.

Coordinate before touching files also needed by T2. Do not modify replay/read
implementation, product UI, domain adapters, scientific experiments, or release
surfaces.

## Required audit

Settle with direct code/test evidence: proposed-transition identity; multiple
and conflicting Verification Records; verifier identity; Decision authority;
multi-verification consumption; semantic Event linkage; Standing derivation;
correction, supersession, invalidation, and rejection retention; current-status
queries; content-addressed integrity; semantic versus execution metadata.

At minimum prove that Verification cannot alter Standing, unauthorized Decision
cannot admit a transition, authorized Decision changes Standing through the
expected Event, rejection remains addressable without accepted Standing,
correction/supersession preserve history, identical accepted Event sequences
produce identical Standing, invalid or dependency-incomplete transitions fail
closed, Decision attribution survives replay, contradictory Verification
Records do not decide, and canonical Standing digest changes only with semantic
Standing.

Prefer a compact conformance matrix over broad test-count growth. Do not create
a new wire object merely to match campaign vocabulary.

## Finish

Commit on `campaign/compose1-kernel`. Report exact files, semantic matrix,
tests/commands/results, gaps found, unresolved issues, and commit. Do not merge.

