# Changelog

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
