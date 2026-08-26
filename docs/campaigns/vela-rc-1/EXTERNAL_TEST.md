# VELA-RC-1 blind external-user test

Status: `PASS WITH FIRST-USER LIMITATIONS`.

The participant receives only the release-facing repository and documentation.
It may not inspect VELA-COMPOSE-1 or RC-1 campaign records and receives no
semantic coaching or hidden implementation hints.

The frozen tasks are to explain Vela in one paragraph; install it; initialize
a Repository; submit a bounded result; preserve a failed Verification; make an
authorized Decision; inspect Standing and explain why it changed; replay; and
diagnose one corrupt or missing Artifact fixture.

A pass requires operational completion and correct understanding that
Verification does not change Standing, Decision is authority-bearing, Event
history is retained, Standing is derived, replay reconstructs governed state,
and rejected or failed work is not accepted state. Coaching invalidates the
blind result. R7 records mistakes and friction but does not modify the product.

## Exact blind candidate

| Field | Value |
| --- | --- |
| Candidate commit | `41ec11750daf8268eba61f9307fe0bcbbd6ca044` |
| Candidate tree | `233d4713bcc6112aa3a4b9fdf64cddd0a69d6e02` |
| Version | `0.977.4` |
| Binary SHA-256 | `b23ffd6dd9f6d01235369386e4582b55350cd18af70a4129bd414b8b1e16803d` |
| Host | macOS 27.0, arm64 |
| Rust | 1.97.1 |
| Git | 2.53.0 |
| Python | 3.13.9 |
| OpenSSH | 10.3p1 |

The participant used only the frozen release-facing paths, CLI help, and
ordinary source access. It did not inspect RC-1 records, receive coaching, use
external credentials, or modify the product. The source build completed in
15.62 seconds from locally cached dependencies; this is not a cold-download
measurement.

## Operational result

The participant independently:

1. explained Vela accurately before using it;
2. built `vela 0.977.4` with an external Cargo target directory;
3. initialized a disposable Repository and authority identity;
4. submitted an intentionally false finite-Boolean result;
5. recorded an exhaustive failing Verification;
6. observed `accepted_claims: 0`, and an attempted acceptance failed without
   changing `HEAD`;
7. rejected the false Proposal while preserving its history;
8. submitted and verified a corrected result;
9. observed that the passing Verification still left `accepted_claims: 0`;
10. made the separate attributed Decision, after which Standing changed;
11. inspected both the rejected Proposal and accepted corrected Proposal;
12. replayed the same Repository root, roots, and counts in the working
    Repository and a fresh clone; and
13. diagnosed a missing Artifact fixture as fail-closed with no partial
    Standing returned.

The final consumer Repository had commit/tree
`9bff099f07ab00f9cf2bbe162d03389cdee2657e` /
`9a776083b3f2a45974c71ada84ccc286505e5a77`, Repository root
`sha256:b399d362cedc836c47786e14bdf9b59bd205a64e691a6a1a3c8d4eda0f430cf6`,
one accepted Claim, and two Proposals, Submissions, and Verifications. Its
Event history retained `authority.initialized`, `review.rejected`,
`claim.asserted`, and `review.accepted`.

## Semantic score

All six required semantic propositions were understood and demonstrated:

- Verification alone cannot change Standing: `PASS`.
- Decision is authority-bearing: `PASS`.
- Event history is append-only: `PASS`.
- current Standing is derived: `PASS`.
- replay reconstructs governed state rather than native computation: `PASS`.
- failed work remains distinguishable from accepted state: `PASS`.

The participant identified a concrete value over ad hoc Git, JSON, and logs:
Vela makes checker success, authorized acceptance, and current accepted state
separately queryable, root-bound, attributable, and replayable.

## First-user limitations

- The participant initially initialized an empty arithmetic-scoped Repository
  before choosing the Boolean example. It caught the scope mismatch before any
  scientific submission, preserved the empty genesis, and initialized the
  correctly scoped Repository.
- The documented corrupt-fixture command order could not install the trust pin
  directly from the corrupt branch because the missing Artifact blocked that
  read. Installing the same published pin through the valid fixture clone
  worked, after which corrupt replay failed closed. This is documentation
  friction, not a semantic or integrity failure.
- `init` and trust pinning write account-scoped anchors under `~/.vela`. The
  three anchors created by the run were removed after qualification.
- The source build reused locally cached Cargo dependencies; clean-install
  cold-path evidence remains R2/R6 evidence rather than R7 evidence.

These limitations are explicit and non-blocking for the qualified Core. No
product repair was made during the blind test.
