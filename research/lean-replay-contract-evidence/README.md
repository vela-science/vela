# Lean replay package evidence

This directory retains qualification evidence for the source-local Level 0
Lean replay package experiment. It is outside the logical package root and is
not a package index, Registry, Vela object, Verification Record, or source of
scientific authority.

The package it qualified is no longer in the tree. The experiment failed its
own net-deletion gate, both named consumers were archived read-only by ADR
0039, and `docs/PORTABLE_WAIST_CAMPAIGN.md` recorded it as closed while the
implementation went on being built and tested. Git history retains that
implementation; what is kept here is the answer it produced, which is what
`docs/ECOSYSTEM.md` cites.

`qualification.json` records the package's current identity, both independent
root readers agreeing on it, and the binding stop condition that keeps it at
Level 0.

The record tracks the package. When the repository-wide rename to unversioned
filenames moved three of the package's ten paths, the logical root moved with
them — a package root is a function of exactly those paths and bytes — and the
record was recomputed rather than annotated, with the superseded root retained
under `predecessor` so the move is legible.

The two-consumer gate went to `false` in the same edit, and not as bookkeeping.
Formal and Erdős agreed on the predecessor root at the commits named under
`consumers`, and ADR 0039 has since archived both repositories read-only. Those
runs cannot be repeated and no live repository consumes this package, so the
gate is unrepeatable against them rather than merely unpassed. Level 1
promotion needs it earned again on a live repository under the current
contract, on a replay recurrence where shared mechanics replace duplicated
maintained code — plus real net deletion and the cross-platform reconstruction
gate.

