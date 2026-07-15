# Changelog

## v0.800.8 — 2026-07-15 — Release lint closure

- Removed the needless test-only borrow caught by the hosted all-targets Clippy
  gate for `v0.800.7`. Runtime behavior, transaction bytes, schemas, and
  authority boundaries are unchanged.
- Retains the `v0.800.7` canonical proposal-ordering fix and its exact
  prepare-materialize-recovery regression as the release candidate used for
  frontier migration.

## v0.800.7 — 2026-07-15 — Canonical proposal transaction postimages

- Kept every in-memory pending-proposal insertion in the same proposal-ID
  order used by split-repository loading. A prepared transaction's visible
  postimages therefore remain byte-identical after the official materializer
  reloads the same proposal files.
- Added the exact regression found while migrating the Erdős frontier: prepare
  bounded legacy-policy retirement, materialize, compare `frontier.json`,
  `vela.lock`, and `proof/latest.json` byte for byte, then reacquire the
  completed-journal recovery barrier successfully.
- Added a direct protocol ordering regression. Accepted events, Receipt v1,
  scientific schemas, and authority rules are unchanged; this patch reads no
  key and performs no decision.

## v0.800.6 — 2026-07-15 — Bounded legacy-policy retirement

- Added one prepare-only recovery command for unsupported prelaunch policy
  bytes. `vela policy retire-legacy` records a closed, content-addressed
  governance proposal without reading a key, validating the legacy signature,
  or granting policy authority. The existing isolated `vela sign` Decision
  Plan remains the only acceptance path.
- Bound acceptance to the exact raw active pair, fixed internally derived
  paths, the optional byte-identical same-id snapshot pair, an intact replay,
  no current policy head, and no evidence that the legacy policy admitted
  state. Drift aborts before key access; rejection preserves every byte; an
  accepted review and the bounded deletions commit in one recoverable
  transaction.
- Narrowed strict-signal heuristics so typed, non-biomedical Erdős catalogue
  records do not inherit empirical missing-condition failures and a mathematical
  “translation property” is not treated as a missing biological translation
  condition. Empirical biomedical records retain the strict checks.
- Added the `vela.policy-legacy-retirement.v1` governance proposal shape and
  its review/audit regressions. Receipt v1, accepted event, policy-lane, and
  scientific finding schemas are unchanged.

## v0.800.5 — 2026-07-15 — One executable frontier scaffold

- Removed the unlaunched multi-template `vela init --template` branch and its
  orphaned adoption scaffold. New frontiers now have one task-first path and
  one generated command list: `agents sync`, `doctor`, `status`, `next`, and
  strict `check`.
- Replaced retired generated commands (`inbox`, `integrity`, `stats`,
  `source-inbox`, `task`, `claim diff`, and `gate .`) with commands the current
  binary actually exposes. The generated charter now teaches
  `next -> work -> land -> sign`, and a regression keeps first-run guidance on
  the release surface.
- Made a fresh frontier's MCP file byte-identical to `vela agents sync` and
  explicitly limited it to the nonfinalizing draft profile. Agent tooling can
  land a Receipt, but cannot sign or finalize a proposal.
- Made an empty frontier's first `next` useful: it offers one generic
  `seed:first` producer session, without inventing scientific content or
  restoring a template system. Init/doctor command hints now shell-quote the
  frontier path and the generated MCP adapter carries no dead environment.
- Treat a completely absent optional review-policy document set as an explicit
  conservative-default warning. Partially configured or malformed policy
  documents remain release-blocking, including explicitly declared files that
  are missing; declared paths never silently fall back. A fresh frontier no
  longer contradicts its own doctor guidance.
- Made `vela doctor` a local, offline diagnostic. It no longer probes a hosted
  hub, requires a Rust toolchain outside the substrate checkout, or treats an
  occupied optional Workbench port as failure.
- Added a tag-driven two-platform release workflow. Linux x86-64 and Apple
  Silicon binaries plus installer-compatible SHA-256 companions are now built
  from the exact tag and attached to its GitHub Release. Release jobs use fixed
  runner images, exact action commits, least-privilege job permissions, and
  repository-level immutable releases.

## v0.800.4 — 2026-07-15 — Trust-boundary parity hardening

- Consolidated policy-context derivation in the protocol. Landing, replay,
  review, policy testing, and policy suggestion now use one strict builder and
  one caller-supplied observation instant. Missing or incoherent retained
  material fails closed instead of being reconstructed optimistically by the
  CLI; legacy audit paths cannot manufacture credential or assurance facts.
- Added direct regressions for detached-HEAD refusal, publication to an
  un-checked-out branch without touching the caller index, linked-worktree
  rejection, and exact post-ref index-lock recovery. These tests exercise the
  existing Git transaction rather than adding a transport or authority layer.
- Proved that flag authoring and file import retain byte-identical canonical
  Receipt v1 bytes and roots for the same facts. Landing-time activity,
  proposal, and commit identities remain separate provenance; exact retries
  on one frontier remain fully idempotent.
- Receipt-backed finding proposals now retain one typed evidence span per
  explicitly bound artifact, pointing into the canonical Receipt. The normal
  task-first result is therefore review-ready without inventing verifier,
  independence, or acceptance claims.
- No Receipt, event, policy-lane, or Decision Brief schema changed. Existing
  accepted-event bytes are not rewritten, and no human decision or key
  ceremony is part of this release.

## v0.800.3 — 2026-07-15 — Nested-workspace test portability

- Made the frontier-repository integration tests honor the explicit
  `VELA_BIN` contract, matching the release-contract tests. Parent workspaces
  can now reuse the Vela binary they already built instead of requiring a
  duplicate binary under the submodule's private `target/` directory.
- No runtime behavior, protocol schema, Receipt, verifier, accepted event, or
  materialized-frontier bytes changed.

## v0.800.2 — 2026-07-15 — External Lean boundary consolidation

- Removed the unlaunched replay-packet compatibility mode, its packet lineage
  and sealed-environment contract machinery, and the last producer-specific
  Lakefile helper from the installed external Lean verifier.
- Kept one generic external boundary: a full GitHub repository URL, commit,
  and Lean declaration are reconstructed in a Vela-controlled project and
  produce a typed draft result without gaining acceptance authority.
- Added a prelaunch regression guard so packet flags and producer-specific
  Diderot or Krafft assumptions cannot return to the installed verifier.
- Preserved historical Diderot corpus bytes as explicitly inert provenance.
  Diderot remains an early exploratory project, not a Vela partner,
  dependency, verifier, compatibility target, or release gate.
- No protocol schema, Receipt, accepted event, or materialized-frontier bytes
  changed.

## v0.800.1 — 2026-07-15 — Portable prelaunch maintenance

- Made the one-writer-path regression guard depend only on standard Unix
  tools and Git, so a clean GitHub runner checks the same surface as a local
  checkout without requiring ripgrep.
- Made agent-adapter generation use the tracked frontier manifest instead of
  ignored local state, and refreshed the generated task-first skills.
- Made cross-workspace CLI contract tests honor the suite's explicit
  `VELA_BIN`, so a clean parent checkout reuses the binary it already built.
- Moved the broad historical Lean model build to an explicit manual workflow,
  documented its custom assumptions honestly, and removed optional external
  Lean packaging assertions from routine core CI.
- Made the active packet test fixture derive its compiler version from the
  package instead of pinning the prior release.
- No protocol, schema, verifier, Receipt, or materialized-frontier bytes
  changed. Frontiers recorded with Vela `0.800.0` remain exact historical
  artifacts.

## v0.800.0 — 2026-07-14 — Task-first protocol hard cut

Vela's prelaunch protocol candidate is organized around one contribution path:
a producer emits Receipt v1, `vela land` records it, the signed policy routes
it, and a human key holder alone can make an uncovered truth-bearing decision
through `vela sign`.

- Removed unlaunched compatibility aliases and alternate writer paths,
  including direct proposal accept/reject, submit, attempt import, auto-admit,
  legacy finding apply, redundant clients, stale schemas, and obsolete
  examples.
- Removed the unlaunched acceptance-policy compatibility subsystem. Vela now
  accepts only current content-addressed policy IDs, signatures bound to
  `signed_at`, and `vela.policy-lane.v2` replay records.
- Consolidated the portable Python emitter, installed external-verifier core,
  canonical JSON reader, and conformance commands into one crate-owned resource
  bundle. Removed the duplicate checkout-only package and made the whole-body
  `vela:receipt_body` binding mandatory in the single Receipt v1 validator.
- Reduced publication to the exact reviewed Git delta.
- Rebuilt the Hub as a disposable read-only Git index over a versioned source
  catalog. It no longer registers or deprecates sources, signs records, stores
  witness objects, or writes canonical scientific state.
- Removed Carina from live code, schemas, manifests, locks, and documentation.
  Existing immutable event payloads remain readable as opaque historical data.
- Generalized the optional external Lean verifier and removed Diderot-specific
  compatibility and release checks. Diderot is an early exploratory project,
  not a Vela partner, protocol target, or release dependency.
- Added a prelaunch-surface regression gate so retired paths cannot quietly
  return before the protocol is published.
- Removed the duplicate automatic Receipt-draft workflow. Receipt conformance
  stays in the focused core gate; optional external verifier execution remains
  explicit.
