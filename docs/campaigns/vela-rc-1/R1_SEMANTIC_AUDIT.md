# R1 protocol and semantic audit

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
HOLD — SEMANTIC BLOCKER
```

Protocol 1 requires strict replay to verify an independently selected
sequence-one authority root. The shipped `vela replay` path verifies the
repository-retained authority chain but does not load or compare the local
trust pin. The current integration test explicitly removes that pin and then
expects both replay and status to report strict success. This is an externally
documented semantic mismatch at the authority boundary, not a missing prose
example.

R1 did not weaken the normative trust requirement, add a second replay mode,
or redesign authority. The candidate remains on hold until implementation,
normative documentation, conformance evidence, and the release-facing read
path agree on one fail-closed rule.

This is a worker verdict for supervisor review. It is not a release Decision,
publication, or authorization to change Protocol 1.

## Candidate identity

| Field | Audited value |
| --- | --- |
| Parent control commit | `6d680eebb4a17813e72b55685aa2eec6b34e5fae` |
| Parent control tree | `9273cdaea323859fcd26beebeccc3f7b7fb1acfe` |
| Parent branch | `campaign/vela-rc-1-supervisor` |
| Worker branch | `campaign/vela-rc-1-r1-semantic-audit` |
| Initial status | clean, detached at the exact parent control commit |
| Vela | `0.977.4` |
| Protocol | Vela Protocol 1 release candidate |
| Submission | `vela.submission.v3` |
| Verification Record | `vela.verification-record.v2` |
| Proposal | `vela.proposal.v1` |

## Release-blocking contradiction

Normative Protocol 1 says:

- consumers obtain the full sequence-one authority-record root through an
  independent channel and store it as `vela.authority-trust-anchor.v1`;
- repository bytes may not choose their own trust anchor; and
- strict replay verifies the independently pinned sequence-one authority root.

The qualified implementation instead has two different boundaries:

- `vela review accept|reject` loads the local trust pin and refuses a missing
  or mismatched pin before an authority write; but
- `vela replay`, `status`, and read projections call the repository authority
  loader without consulting the local pin. They accept whichever valid
  sequence-one chain is retained by the checkout.

The mismatch is already disclosed in
`docs/interop/scientific-state-profile.md`, which says current CLI read and
replay paths do not consult the local trust pin. That informative limitation
directly conflicts with the normative replay list in `docs/PROTOCOL.md` and
with the CLI's `strict: pass` result.

Direct reproduction is
`current_submission_and_verification_replay_without_changing_accepted_state`
in `crates/vela-cli/tests/genesis.rs`: it removes the pin, retains routine
evidence, successfully runs `vela replay` and `vela status`, and only later
proves that an attributed Decision is refused until the pin is reinstalled.
The test passed on the audited candidate. The independent authority-chain
vector also passed and correctly refused `wrong-trust-anchor`, proving that the
required behavior exists in conformance but is not enforced by the shipped
read path.

## Object and semantic audit

| Surface | Qualified meaning and observed behavior | Result |
| --- | --- | --- |
| Submission | Signed portable producer input; installs exact Artifacts, a derived Claim, and a pending Proposal without authority Events or accepted Standing. | `PASS` |
| Verification Record | Signed scoped observation over exact Claim, Submission, Proposal, Artifact, method, identity, outcome, and nonclaims; pass and fail can coexist; import changes no Standing. | `PASS` |
| Proposal | Distinct repository-minted, unsigned candidate transition. It binds action, subject, producer package, actor, reason, time, and caveats; status is derived and is not in the object. | `PASS` |
| Decision | Attributed accept or reject operation admitted only through repository write authority; the full plan binds the current read set and ordered Verification set. | `PASS` for writes; read-side authority selection is blocked by R1-F001 |
| Event | Canonical semantic transition covered by an authority record. Acceptance links one Claim Event to one review Event; rejection retains a review Event without accepted Standing. | `PASS` |
| Standing | Derived accepted, superseded, retracted, or unassessed result reconstructed from governed history and checked against the repository manifest. | `PASS` only after the intended authority lineage is independently selected; shipped replay does not enforce that selection |
| Artifact | Exact retained bytes addressed by full SHA-256. Missing or corrupt bytes fail strict replay before the repository root can be reported. | `PASS` |
| Authority | Explicit keyset, authorization model, authenticated principal, retained authorization request, signed record chain, and independent sequence-one trust selection. | `FAIL` in the shipped read/replay path |
| Rejection | Removes the pending transition, retains Submission, Proposal, Verification Records, Decision attribution, and rejection Event, and leaves the Claim unassessed rather than accepted. | `PASS` |
| Correction | A new signed request and `corrects` relation targeting one exact accepted predecessor; authorized acceptance retires the predecessor without rewriting it. | `PASS` |
| Supersession | A distinct signed request and `supersedes` relation with the same exact `claim.revise` state rule as correction; the label remains in retained bytes. | `PASS` |
| Retraction | A signed `retract_claim` request becomes `claim.withdraw`; authorized acceptance emits `claim.retracted` and removes the exact accepted Claim. | `PASS` at the kernel boundary; direct CLI authoring is not exposed |
| Replay | Reconstructs and validates governed state and exact retained objects; it does not rerun native computation or scientific Methods. | `FAIL` only at independent authority-root selection |
| Canonical bytes and digests | RFC 8785 JCS for protocol JSON and SHA-256 over the declared exact domain; canonical, schema, cross-reader, and authority vectors agree. | `PASS` |

`corrects` and `supersedes` are distinct authenticated intent and relation
labels, but Protocol 1 intentionally gives them the same Standing transition:
each replaces one exact accepted predecessor and produces a
`claim.superseded` Event. No policy or downstream consequence is inferred from
the choice of label.

## Ten required questions

| # | Answer | Classification |
| --- | --- | --- |
| 1 | There is one reducer meaning for current Standing, but the shipped read path does not independently select which otherwise-valid authority lineage governs that reducer. | `RELEASE BLOCKER` |
| 2 | Yes. Contradictory pass and fail Verification Records coexist, remain addressable, and block acceptance without resolving each other. | `NOT A PROBLEM` |
| 3 | No. Missing, dependent, failed, inconclusive, error, unavailable, or not-run evidence cannot satisfy acceptance. | `NOT A PROBLEM` |
| 4 | Decision writes explicitly bind principal, performer, keyset, model, current roots, read set, and signature. Third-party read verification is not complete until the shipped read path enforces an external sequence-one anchor. | `RELEASE BLOCKER` |
| 5 | Yes at the wire and history boundary: `correct_claim`/`corrects` and `supersede_claim`/`supersedes` remain distinct exact bytes while sharing one replacement transition. Any richer policy distinction is not Protocol 1 behavior. | `NOT A PROBLEM` |
| 6 | Yes. Rejection closes the Proposal, preserves all evidence and Decision history, and leaves accepted Standing unchanged. | `NOT A PROBLEM` |
| 7 | Replay is normatively specified as governed-state reconstruction, not native rerun, but its independent trust-anchor requirement is not implemented by `vela replay`. | `RELEASE BLOCKER` |
| 8 | Artifacts are protocol semantics: their exact paths, bytes, SHA-256 identities, and object references are replay inputs. Their scientific interpretation remains source-owned. | `NOT A PROBLEM` |
| 9 | Yes. Canonical bytes and typed digests are documented and independently reproduced. Protocol 1 publishes no standalone `standing_root`; deterministic Standing is covered by the accepted set, Repository root, authority Event-log root, and root-bound projections. | `NOT A PROBLEM` |
| 10 | The trust-pin split is visible only by reconciling the normative protocol, an informative interoperability caveat, implementation, and tests. A release-facing reader could reasonably believe `strict: pass` included the independent trust selection. | `RELEASE BLOCKER` |

## Authoritative R1 conformance matrix

This is the single R1 matrix for the audited candidate. `PASS` means the
expected Protocol 1 state and authority effect was reproduced. It does not
mean scientific acceptance or release readiness.

| Invariant | Test | Fixture | Expected result | Status |
| --- | --- | --- | --- | --- |
| Submit | `review_accept_admits_the_event_that_moves_standing`; `current_submission_and_verification_replay_without_changing_accepted_state` | CLI-created signed Submission v3 and retained Artifact | Submission, Claim, Artifact, and pending Proposal are retained; accepted Standing and authority Events do not change. | `PASS` |
| Verify pass | `review_accept_admits_the_event_that_moves_standing` | Two independent passing records satisfying two declared properties | Both signed records are retained and consumed by one ordered `verification_set_root`; Standing does not change before Decision. | `PASS` |
| Verify fail | `empty_disposable_repository_rejects_failed_verification_without_standing` | Signed failing Verification Record | Fail remains evidence, satisfies no requirement, blocks acceptance, and changes no Standing. | `PASS` |
| Contradictory verification | `empty_disposable_repository_rejects_failed_verification_without_standing` | Passing and failing records for the same property | Both coexist; Inbox marks one satisfying and one blocking; acceptance refuses without mutation. | `PASS` |
| Incomplete verification | Same rejection lifecycle | Proposal before its independent passing requirement exists | Acceptance refuses with the exact repository and accepted-Standing commitment unchanged. | `PASS` |
| Unauthorized Decision | `review_accept_admits_the_event_that_moves_standing` | Same complete Decision attempted without repository signer | `authority_refused`; no Git, authority-chain, Event, or Standing mutation. | `PASS` |
| Authorized accept | Same acceptance lifecycle | Current Inbox root, independent passing set, authorized principal, OpenSSH agent | Linked Claim and review Events are admitted; pending Claim becomes accepted; replay and clean clone agree. | `PASS` |
| Authorized reject | Rejection lifecycle | Contradictory evidence followed by authorized reject | One rejection Event and attribution are retained; pending entry closes; accepted Standing stays empty. | `PASS` |
| Correction | `an_accepted_correction_leaves_the_repository_readable_and_projectable`; authority-chain interop | `conformance/fixtures/authority/math-coh-00/` and correction lifecycle | Exact accepted predecessor is retired, successor accepted, predecessor bytes and Decision remain replayable. | `PASS` |
| Supersession | `exact_supersession_authoring_does_not_require_a_source_run`; authority-chain interop | Submission v3 supersession plus two retained `claim.superseded` transitions | Signed request and `supersedes` label remain distinct; exact predecessor is replaced without history rewrite. | `PASS` |
| Retraction | `withdrawal_accepts_only_the_exact_accepted_claim`; `an_accepted_withdrawal_retracts_rather_than_accepts`; Event-link unit | Kernel `retract_claim` / `claim.withdraw` plan | Only the exact accepted Claim can be removed; accepted Decision emits linked `claim.retracted` and review Events; old Claim reads retracted. | `PASS` with CLI-authoring limitation |
| Rejected-history preservation | Rejection lifecycle | Retained Submission, pass/fail Verifications, Proposal, and reject Event | `review show` exposes rejected Decision; `why` leaves Claim unassessed; every retained object remains addressable. | `PASS` |
| Replay | Acceptance, rejection, portable-divergence, and genesis lifecycles | Original checkout and complete clean clones | Same governed bytes reproduce repository root, Event roots, object counts, and Standing; native computation is not executed. | `PASS` except authority selection below |
| Missing Artifact | `current_submission_and_verification_replay_without_changing_accepted_state` | Retained Artifact deleted in clean clone | Replay exits nonzero before reporting the prior root. | `PASS` |
| Corrupt Artifact | Same genesis lifecycle | Retained Artifact bytes substituted | Digest/root mismatch; replay exits nonzero; restoring exact bytes restores the prior root. | `PASS` |
| Changed authority | `retained_math_authority_falsifiers_fail_closed_without_resigning`; unpinned genesis lifecycle | `wrong-trust-anchor`, changed key/model/request/Event vectors, and removed local pin | Every authority mutation or wrong external anchor must fail strict replay. Independent vector refuses; shipped `vela replay` succeeds without consulting the pin. | `FAIL — R1-F001` |
| Changed Method reference | Genesis lifecycle; `canonical_review_method_check_is_non_mutating_and_binding_exact` | Missing or substituted source-owned Review Method; changed profile/property/actor/nonclaim | Signed Verification binding drift refuses. Missing local Method is `unavailable`; substituted bytes fail projection root resolution; governed-state replay remains unchanged because native rerun bytes are not reducer input. | `PASS` |
| Deterministic Standing digest | Acceptance/rejection accepted-set commitments; authority-chain terminal state | `math-coh-00/expected.json` terminal accepted set and manifest root | Identical governed history and exact objects yield the same accepted set and Repository/Event roots. No standalone `standing_root` is part of Protocol 1. | `PASS`; release-checklist wording is a doc blocker |

## Findings and dispositions

| ID | Classification | Finding | Required disposition |
| --- | --- | --- | --- |
| R1-F001 | `RELEASE BLOCKER` | `vela replay` and read surfaces do not enforce the independently pinned sequence-one authority root required by normative Protocol 1, yet can report `strict: pass`. | Keep release on HOLD. Resolve one exact read contract and add a shipped-CLI negative test against a missing/mismatched external anchor; do not weaken the trust model as a documentation shortcut. |
| R1-F002 | `DOC BLOCKER` | `docs/ROOTS.md` says the Proposal root covers “record and status,” while `vela.proposal.v1` deliberately has no status field and derives status from Events. | Change the roots catalogue to say the root covers the complete Proposal record and that status is separately derived. |
| R1-F003 | `DOC BLOCKER` | Before this audit there was no single release-facing matrix connecting the required semantic scenarios to executable fixtures and expected state effects. This campaign matrix records the gap but is not a substitute for an integrated public implementer surface. | After R1-F001 is fixed and requalified, integrate a reviewed release-facing matrix or equivalent fixture index without making campaign notes normative. |
| R1-F004 | `DOC BLOCKER` | The RC-1 release checklist asks for an expected “Standing digest,” but Protocol 1 deliberately publishes no standalone `standing_root`. | Name the exact existing commitment the gate requires: terminal accepted set plus Repository root and authority Event-log root, or explicitly authorize a future schema change separately. |
| R1-F005 | `ERGONOMIC ISSUE` | Retraction is supported by imported signed Submission v3 and the Decision kernel, but direct `vela submit` authoring exposes add, correction, and supersession only. | Document the import-only authoring path for this release or add direct authoring only as a separately scoped CLI task; no Protocol change is needed. |
| R1-F006 | `FUTURE PROTOCOL QUESTION` | Whether `corrects` and `supersedes` should acquire different policy or consequence semantics, or whether Protocol 1 should publish a standalone Standing digest. | Do not add either to RC-1 scope. Current exact labels and rooted state remain sufficient once R1-F001 is resolved. |
| R1-F007 | `NOT A PROBLEM` | Verification/Decision separation, contradictory and incomplete evidence refusal, rejection retention, append-only correction history, Artifact integrity, canonical bytes, and governed-state replay all matched current implementation and focused conformance. | Preserve the current semantics; do not redesign them in response to the authority-read blocker. |

## Exact verification

Passed:

```text
cargo test --locked -p vela-cli --features test-support --test review_acceptance --test disposable_rejection_lifecycle --test correction_impact --test genesis --test review_method_check --test portable_divergence
```

Result: 10 passed, 0 failed across the six selected integration targets. The
passing genesis test reproduces R1-F001 by accepting unpinned replay and strict
status before refusing the later Decision.

```text
cargo test --locked -p vela-protocol --test authority_chain_interop --test canonical_hashing_conformance --test wire_schemas
```

Result: 9 passed, 0 failed. This includes external-anchor selection and all
thirteen authority falsifiers.

```text
cargo test --locked -p vela-cli --features test-support repository_decision::tests::withdrawal_accepts_only_the_exact_accepted_claim
cargo test --locked -p vela-cli --features test-support claim_standing::tests::an_accepted_withdrawal_retracts_rather_than_accepts
cargo test --locked -p vela-cli --features test-support repository_decision::tests::revise_replaces_exactly_one_predecessor
cargo test --locked -p vela-cli --features test-support repository_decision::tests::acceptance_links_one_domain_event_to_one_review_event
```

Result: 4 selected unit tests passed, 0 failed.

```text
uv run --project conformance --locked python conformance/verify.py
```

Result: PASS. It reproduced 77 normative files, 39 informative files, 14
schemas, 18 positive objects, 37 negative schema cases, 179 portable patterns,
four reference flows, the 13-vector authority falsifier inventory, and Protocol
1 root
`sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`.

Separately, the supervisor reported that the full locked Core union and
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed from
the same control commit and recorded that evidence at
`339db00cb93001440f1768e4e1d56d6dd0b2dc98`. R1 treats this as inherited
regression evidence only; it does not qualify the trust-selection mismatch or
change this independent gate.

`cargo fmt --all -- --check` and `git diff --check` also passed.

No tag, push, publication, version bump, release, deployment, signing,
outreach, or Protocol redesign was performed.
