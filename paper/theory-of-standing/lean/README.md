# Mechanized minimal Standing model

This isolated Lean 4 package mechanizes the smallest abstract state model used
by the Phase III P1.1 proof artifact. It is explanatory research material. It
does not implement, extend, or replace the normative Vela Protocol 1 runtime,
schemas, canonical roots, authorization engine, or CLI.

The model retains only:

- authenticated Submission input and scoped Verification records, neither of
  which changes Standing;
- Repository-local authorization, performer attribution, expected-root and
  read-set freshness, and correction-reference validation for Decisions;
- canonical admitted Events, deterministic replay, authority-indexed Standing,
  and correction consequences; and
- one finite C-versus-D witness. C retains the same correction-aware semantic
  actions, Repository labels, and authority labels as D but omits Decision
  identity, performer, expected root, read set, and admission state. A fresh
  and stale correction therefore collapse under C while D distinguishes them.

The witness is constructed from small natural-number identifiers in
`TheoryOfStanding.lean`. It does not encode, load, translate, or inspect any of
the eight held-out Standing-minimality fixtures. D is an ordinary reducer under
the definitions in the file and is proved against explicit expected results;
it is not used as an answer oracle.

## Check

From this directory:

```bash
lake build
lake env lean TheoryOfStanding.lean
```

The package pins `leanprover/lean4:v4.19.0`, matching the existing isolated
Lean fixture convention in this repository, and has no external dependency.
Both commands check every theorem and execute the rejection and separation
examples.

For the focused placeholder scan from the Vela repository root:

```bash
rg -n '\b(sorry|axiom|unsafe|admit)\b|declaration uses' \
  paper/theory-of-standing/lean --glob '*.lean'
```

The scan must return no matches.

## Theorem inventory

- `replay_determinism`
- `standing_change_implies_admitted_authorized_decision`
- `unauthorized_decision_fails_closed`
- `stale_root_decision_fails_closed`
- `stale_read_set_decision_fails_closed`
- `misattributed_decision_fails_closed`
- `correction_reference_invalid_fails_closed`
- `decision_is_repository_local`
- `plural_authority_consistency`
- `correction_history_preserved`
- `correction_predecessor_is_superseded`
- `correction_replacement_is_accepted`
- `correction_consequence_updates_deterministically`
- `finite_c_versus_d_separation`

The model makes no statement about universal scientific truth, productivity,
adoption, or the suitability of Standing as an all-science substrate.

## Evidence boundary

The model follows the surviving narrow P0 distinction: the frozen semantic
candidate at `2dd76cb70f6e93fffb74c994afd3d8dedab4a460` and the independently
revealed held-out result at `3ce2c9643e98d07a105570acec0612b53dba95c9`
showed one D-only gain on a `superseded-panel-root` lifecycle-freshness branch.
Those commits motivate the abstract witness but are not imported as proof
premises, fixtures, or generated theorem data.
