# Changelog

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
