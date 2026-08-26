# Phase 0 repository baseline

Recorded: 2026-08-26, America/Toronto.

## Exact repository state

```text
Repository: https://github.com/vela-science/vela.git
Checkout: /Users/williamblair/personal/vela
Baseline branch: main
Baseline HEAD: 23c2eb86b0deb1b155807fae16bcd7ba5bb707c0
Baseline tree: 2327520d9f807c5f00001fc27cded7de7f980434
Dirty state before campaign: clean
Supervisor branch: campaign/compose1-supervisor
```

No root `.vela/` directory exists and none may be created: the Vela source
repository is not itself a Vela Repository. No `vela.lock` is present. The
requested `docs/ROADMAP.md` and `docs/ECOSYSTEM_COMPLETION_2026-08-09.md` are
absent; current architecture and boundaries replace those historical planning
surfaces.

## Toolchain and host

```text
Vela workspace version: 0.977.4
Rust: rustc 1.97.1
Cargo: 1.97.1
uv: 0.11.6
Host: Apple arm64, Darwin 27.0.0
```

## Current architecture

The repository already implements the product loop:

```text
init -> submit -> verify -> decide -> replay
```

The crate boundary is `vela-protocol <- vela-repository <- vela-cli`, with
`vela-authority` providing restricted authorization. Protocol owns objects,
canonicalization, roots, semantic events, and replay contracts. Repository owns
policy-neutral transactions/recovery. CLI owns concrete write policy,
repository authority, decision admission, and read projections.

The current scientific-state objects are authenticated Submission v3, scoped
Verification Record v2, repository-minted Proposal v1, Claim Record v1,
authorized Decision represented through signed authority history, canonical
semantic Event `vela.event.v0.1`, and root-bound derived Standing/read
projections. One Submission may receive multiple scoped Verification Records;
conflict does not resolve Standing. A current Proposal may be accepted,
rejected, revision-requested, or producer-withdrawn under distinct authority
effects.

## Existing semantic guarantees

- routine Submission and Verification intake is self-authenticated evidence and
  cannot change accepted Standing;
- an attributed, authorized Decision is the only accepted-state boundary;
- semantic events and authority records are append-only and content-addressed;
- accepted Standing is replayed from authoritative history;
- correction and supersession retain exact predecessor roots and history;
- rejected and failed Verification paths remain addressable;
- independent repositories may admit divergent Decisions over identical
  Submission bytes;
- derived projections have no authority effect.

## Existing CLI

The ordinary surface is `init`, `status`, `claims`, `submit`, `show`, `why`,
`review`, `replay`, and `log`. Advanced help contains verification and
maintenance commands. There is no current first-class `vela branch`, `vela
diff`, or `vela compare` command; Git already supplies branch mechanics, and a
new Core command must be earned by maintained consumers rather than the
campaign prompt.

## Baseline verification

All checks below ran from the clean baseline checkout before substantive code:

- `cargo test --locked -p vela-protocol`: 166 tests passed across unit,
  interop, hashing, schema, release-contract, authority, and conformance suites;
- selected CLI lifecycle tests with `--features test-support`: four passed
  (`review_acceptance`, `disposable_rejection_lifecycle`, and both
  `portable_divergence` cases);
- `uv run --project conformance --locked python conformance/verify.py`: PASS;
  Protocol 1 root `sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`,
  77 normative files, 39 informative files, 14 generated schemas, independent
  JavaScript/Python Submission and Verification implementations, correction,
  authority-chain, reference-flow, decision-inbox, and release-reproducibility
  checks all passed.

## Gaps relative to this campaign

1. Existing tests establish many kernel invariants, but there is no single
   campaign conformance matrix proving the complete
   Submission/Verification/Decision/Event/Standing lifecycle and its negative
   edges as one audited surface.
2. Deterministic state replay exists. The campaign still needs an explicit,
   tested distinction between state replay and computational rerun, plus a
   typed inventory of which native receipts are required for each qualified
   operation without inventing a universal receipt ontology.
3. Git can branch identical roots, but the campaign lacks qualified
   source-owned apparatus for sealed branch comparisons and uniform resource
   accounting.
4. Formal-math examples and public Math evidence exist, but not the full
   campaign lifecycle ending in a clean downstream continuation.
5. No qualified Alzheimer’s real-science transition or cross-domain lifecycle
   currently exists in this repository; that work must remain source-owned.
6. The current three-case inheritance evidence is descriptive and internal. A
   new R/V/E handoff experiment must be separately frozen and must not reuse it
   as confirmatory evidence.

## Semantic conflicts resolved before Phase 1

- The prompt's generic `Decision` object is not a license to add a second wire
  object. Current Decisions are attributable authority transitions backed by
  repository Proposal/Verification inputs and semantic Events.
- “Typed receipts” means typed references to native, content-addressed
  artifacts and bounded execution facts. Core must not absorb workflow traces,
  optimizer state, instruments, or domain ontologies.
- Counterfactual execution, agent histories, Lean work, biological work, and
  T6 metascience are source-owned activity. Core may gain only protocol
  behavior proven by multiple maintained consumers.
- Standing is a projection, not a mutable database or transported global truth.

## Workstream dependencies

T1 and T2 share only the current protocol interfaces. T1 may not create replay
machinery; T2 may not create a second state engine or alter Decision authority.
T3 depends on their qualified interfaces. T4/T5 exercise them without changing
Core semantics. T6 depends on T3 plus a qualified vertical. T7 waits for all
relevant gates.

