# T1 kernel + conformance report

Date: 2026-08-26, America/Toronto.

## Disposition

`LOCALLY QUALIFIED` for supervisor review from Phase-0 root
`6a0f5adee55f9c50e7e154ac8d118662809d3323` on
`campaign/compose1-kernel`.

The current Protocol 1 governed-transition semantics are internally
consistent. No implementation or wire change was required. The concrete gap
was conformance coverage: the successful CLI lifecycle consumed only one
Verification Record, while contradictory pass/fail handling was proved only
below the product boundary. The existing lifecycle fixtures now prove both
cases through the shipped CLI. One existing Decision/Event unit was extended
to cover accepted retraction linkage.

This is local engineering qualification. It is not a campaign integration
Decision, release certification, scientific acceptance, or external
validation.

## Semantic matrix

| Kernel obligation | Direct evidence | Result |
|---|---|---|
| Proposed-transition identity | `ProposalV1::canonical_root` and `ProposalV1::id`; `proposal::tests::a_proposal_derives_its_handle_from_its_own_root`; `repository::tests::proposal_directly_binds_its_signed_submission` | Proposal identity commits the exact action, Claim subject, producer package, actor, time, reason, and caveats. |
| Submission authority boundary | `SubmissionRecordV3::seal/from_envelope`; `submission::tests::standing_and_event_fields_are_rejected_as_unknown`; `repository::tests::routine_evidence_overlay_is_append_only_and_cannot_change_standing` | Producer authentication can install a pending transition but cannot write Standing or Events. |
| Multiple scoped Verification Records | `review_accept_admits_the_event_that_moves_standing` now registers two requirements, retains two separate passing records, and proves both requirements are consumed by one exact `verification_set_root` | PASS. |
| Conflicting Verification Records | `empty_disposable_repository_rejects_failed_verification_without_standing` now retains pass and fail records for the same property; Decision Inbox classifies them `requirement_satisfying` and `blocking`; acceptance fails without mutation | PASS; contradiction remains evidence and cannot decide. |
| Verifier identity | `VerificationRecordEnvelopeV2::seal/from_envelope`; `verification_record::tests::the_verifier_is_read_from_the_signed_identity`; CLI fixtures use a verifier identity distinct from the producer | PASS; verifier identity comes from signed payload identity, not an unsigned duplicate field. |
| Verification has no Standing effect | Both modified CLI lifecycles compare a test-only canonical commitment over `.vela/repository.json.accepted_claims` before and after Verification; authority Event/Record bytes also remain identical in the acceptance fixture | PASS. Repository roots move for retained evidence; accepted Standing does not. |
| Dependency-incomplete acceptance | `genesis::current_submission_and_verification_replay_without_changing_accepted_state` and the rejection lifecycle attempt acceptance without an independent passing record and verify exact pre-Decision state is unchanged | PASS, fail closed before authority. |
| Failing evidence blocks acceptance | `require_acceptance_evidence`; `repository_decision::tests::acceptance_requires_exact_independent_passing_evidence`; contradictory lifecycle acceptance attempt | PASS, including a pass beside a fail. |
| Decision authority | Acceptance lifecycle first attempts the same attributed action without the repository signer and receives `authority_refused` with unchanged Git and authority chain; retry through the bound OpenSSH agent succeeds | PASS. Performer attribution does not select or grant authority. |
| Multi-Verification consumption | Decision Inbox and `review.accept` expose the same canonical `verification_set_root`; the accepted plan consumes the two-record rooted set reviewed by the performer | PASS. |
| Semantic Event linkage | Acceptance lifecycle checks distinct `claim.asserted` and `review.accepted` authority Events and the exact semantic `applied_event_id`; repository tests reject missing, duplicate, or cross-transaction links | PASS. |
| Standing derivation and current queries | `load_current_proposal_decisions`, `validate_current_proposal_standing`, `replay`, `status`, `claims`, `why`, and `projection`; acceptance fixture checks the pre-Decision historical checkout and post-Decision checkout | PASS; status tokens are derived from current manifest plus covered authority history. |
| Authorized acceptance changes Standing | Acceptance fixture proves accepted-Claim commitment changes only after the attributed authorized Decision admits its Events | PASS. |
| Rejection retention | Rejection lifecycle retains Submission, both Verification Records, Proposal, rejection Event, actor/session attribution, and `why`/`review show` addressability while accepted Standing remains empty | PASS. |
| Correction and supersession | `correction_impact::an_accepted_correction_leaves_the_repository_readable_and_projectable`; `repository_decision::tests::revise_replaces_exactly_one_predecessor` | PASS across two correction generations; predecessor and intermediate history remain projectable as superseded. |
| Invalidation/retraction | `repository_decision::tests::withdrawal_accepts_only_the_exact_accepted_claim`; `claim_standing::tests::an_accepted_withdrawal_retracts_rather_than_accepts`; extended Event-link test proves `claim.retracted` plus linked `review.accepted` | PASS at the kernel boundary. Producer-owned withdrawal of an undecided Proposal remains a separate no-Event routine action. |
| Identical history gives identical Standing | Acceptance lifecycle replays the same decided Git history in place and in a clean clone with identical repository root and accepted set; projection is identical across checkout paths | PASS. |
| Content-addressed integrity | Protocol object tests, `authority_chain_interop`, canonical hashing, wire-schema negatives, strict repository load, and conformance readers | PASS; tamper, shortened roots, missing objects, forks, rollback, and relation drift fail closed. |
| Semantic versus execution metadata | `AuthorityEventV1::semantic_state_event` excludes transaction identity while retaining exact semantic actor/time/reason/payload; `authority::tests::era_one_event_recovers_transaction_independent_reducer_identity`; Event linkage uses that semantic ID while authority records retain transaction execution | PASS without a second Event model. |

## Standing-root clarification

Protocol 1 publishes the full Repository root, authority Event-log root, and
read-projection root. It deliberately does not publish a second standalone
`standing_root`: Standing is a derived projection, while the Repository root
also commits pending evidence and review history. The lifecycle tests therefore
hash the canonical accepted-Claim slice only as a test oracle. They prove that
Submission, Verification, failed acceptance, and rejection metadata can change
the full Repository root without changing accepted Standing, and that admitted
acceptance changes the accepted-Claim commitment. No new digest or wire object
was introduced.

## Files changed

- `crates/vela-cli/tests/review_acceptance.rs` — two-requirement/two-record
  acceptance, exact Verification-set consumption, and accepted-Standing
  commitment checks.
- `crates/vela-cli/tests/disposable_rejection_lifecycle.rs` — incomplete and
  contradictory acceptance refusals, pass/fail retention, Decision Inbox roles,
  and accepted-Standing invariance through rejection.
- `crates/vela-cli/src/repository_decision.rs` — test-only retraction Event and
  review-link coverage; production implementation unchanged.
- `docs/campaigns/vela-compose-1/T1_KERNEL_REPORT.md` — this audit matrix.
- `docs/campaigns/vela-compose-1/README.md` and `docs/README.md` — required
  current-document index links for the lane report.

## Verification

- Baseline reproduction before edits:
  `cargo test --locked -p vela-cli --features test-support --test review_acceptance --test disposable_rejection_lifecycle --test correction_impact --test portable_divergence`
  — 5 passed, 0 failed.
- Modified lifecycle focus:
  `cargo test --locked -p vela-cli --features test-support --test review_acceptance --test disposable_rejection_lifecycle`
  — 2 passed, 0 failed.
- `cargo test --locked -p vela-cli --features test-support` — 174 unit
  tests and 48 integration tests passed (222 total), 0 failed. This includes
  all 17 `bootstrap_cli_ux` cases; the required feature did not silently omit
  crash-recovery coverage.
- `cargo test --locked -p vela-protocol` — 166 passed, 0 failed across unit,
  authority-chain interop, canonical hashing, relation vocabulary, release
  contracts, JCS, object interop, and wire schemas.
- `uv run --project conformance --locked python conformance/verify.py` — PASS;
  77 normative files, 39 informative files, 14 schemas, 18 positive objects,
  37 negative schema cases, 179 portable patterns, four reference flows, the
  authority-chain falsifier inventory, correction impact, release
  reproducibility, and Decision Inbox v3 all passed. Protocol 1 root remained
  `sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`.
- `cargo clippy --locked -p vela-cli --all-targets --features test-support -- -D warnings`
  — PASS.
- `cargo fmt --all -- --check` and `git diff --check` — PASS.

## Unresolved issues and boundaries

- No kernel semantic conflict or implementation defect remains from this audit.
- There is intentionally no public standalone Standing digest; adding one would
  be a new protocol/read contract and was neither necessary nor authorized.
- Accepted retraction is proved across Decision reduction, Event construction,
  Standing classification, and Decision Inbox scope. There is no direct
  flag-authored CLI retraction lifecycle; current direct authoring exposes add,
  correction, and supersession, while a portable signed Submission may request
  `retract_claim`. T1 did not expand the CLI surface.
- Replay/rerun receipts, branching, verticals, experiments, UI, and release
  work remain outside T1 ownership.
