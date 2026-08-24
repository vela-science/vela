# P1.3 coverage matrix

The two layers below are deliberately separate. “Model-proven” means checked
against the accepted Lean definitions as well as differentially. “Model
differential” means exact agreement and declarative mutation sentinels across
the independent Rust, Python, and JavaScript reducers. “Current-Core-tested”
means the existing Vela implementation remains authoritative. No Core or wire
definition was added or changed for P1.3.

## Model layer

| Distinction | Status | Evidence |
| --- | --- | --- |
| authenticated Submission; unauthenticated no-op | model-proven | `unauthenticated_submission_noop`, `unauthenticated_submission_is_noop` |
| scoped matching and unmatched Verification; no Standing effect | model-proven | `unmatched_verification_noop`, `matching_verification_no_standing`, `unmatched_verification_is_noop` |
| every authorized/unauthorized performer position | model-proven | five performer-position histories; `second_authorized_performer_is_admitted`, `unauthorized_continues` |
| Repository and performer attribution | model-proven | `wrong_repository_suffix`, `misattributed_suffix`; corresponding continuation theorems |
| stale/current root and read-set entries | model-proven | lower/future roots, first/second read-set entry, current multi-entry; continuation theorems |
| ineligible accept/reject/correct and missing pass | model-proven | six ineligible histories; `ineligible_continues` |
| valid/invalid correction references and retry | model-proven | `valid_correction`, two invalid-reference variants, retry; fresh/reference/continuation theorems |
| zero, one, multiple ordered rejections and continuation | model-proven | 14 / 19 / 1 histories; `multiple_rejections_are_noops_before_retry` |
| same external records under plural authorities | model-proven | two histories and `plural_authority_sample` |
| descriptive dependency independence | model-proven | present/absent/unrelated graph mutations and `descriptive_projection_sample` |
| canonical output and exact Event attribution | model differential | declarative output checks plus exact source-Decision/Event comparison |
| duplicate Decision id structural failure | model differential, frozen P1.2 | `corpus/cases/duplicate-decision-id.json`; intentionally not regenerated |

## Current Core/wire layer

These concepts are not fields or predicates in the proof-history format.

| Core-owned class | Status | Existing authoritative owner |
| --- | --- | --- |
| incomplete retained slices | current-Core-tested | `crates/vela-protocol/tests/authority_chain_interop.rs::retained_math_authority_falsifiers_fail_closed_without_resigning`; `conformance/verify_authority_chain.py` vectors `missing-sequence-two` and `missing-accepted-review-event` |
| duplicate admitted event identity | current-Core-tested | `crates/vela-cli/src/authority_transaction.rs::authority_history_rejects_duplicate_semantic_event_identity`; authority-history duplicate coverage validation |
| authority record/event order and terminality | current-Core-tested | `kernel::authority_history::tests::retained_authority_transitions_replay_and_close_is_terminal`; signed sequence, previous-record root, before/after event roots, and exact transaction coverage are verified by `verify_authority_history` |
| Repository identity/locality | current-Core-tested | `kernel::authorization::tests::closed_profile_denies_every_boundary_mismatch_with_stable_reasons`; `portable_divergence` binds divergent local repositories |
| execution environment identity | current-Core-tested | `verification::tests::method_manifest_bytes_bind_the_environment_and_fail_closed`; integration hostile cases `revision_drift`, `selector_drift`, and `binding_method_identity_drift` |
| canonical object roots and strict JSON | current-Core-tested | `canonical_hashing_conformance::{canonical_hashing_vectors_match_rust, non_finite_floats_cannot_reach_a_content_address}`; `object_interop` |
| DSSE signatures and authority chains | current-Core-tested | `authority_chain_interop`; `conformance/verify_authority_chain.py` positive chain and 13 ordered negative vectors |
| current correction projection | current-Core-tested | `crates/vela-cli/tests/correction_impact.rs::an_accepted_correction_leaves_the_repository_readable_and_projectable`; `conformance/verify_correction_impact.py` |
| replay fail-closed and authority-local divergence | current-Core-tested | authority-chain falsifiers; `portable_divergence::{frozen_distinct_principal_histories_match_every_bound_root, one_portable_submission_replays_under_divergent_local_decisions}` |

The focused commands used to recheck these owners are:

```bash
cargo test --locked -p vela-protocol --test authority_chain_interop
cargo test --locked -p vela-protocol --test canonical_hashing_conformance
cargo test --locked -p vela-protocol --test object_interop
cargo test --locked -p vela-protocol retained_authority_transitions_replay_and_close_is_terminal --lib
cargo test --locked -p vela-protocol closed_profile_denies_every_boundary_mismatch_with_stable_reasons --lib
cargo test --locked -p vela-cli authority_history_rejects_duplicate_semantic_event_identity --lib
cargo test --locked -p vela-cli method_manifest_bytes_bind_the_environment_and_fail_closed --lib
cargo test --locked -p vela-cli --features test-support --test portable_divergence
cargo test --locked -p vela-cli --test correction_impact
uv run --project conformance --locked python conformance/verify_authority_chain.py
uv run --project conformance --locked python conformance/verify_correction_impact.py
```

Observed pass evidence on 2026-08-24 was 2 authority-chain interop tests, 2
canonical-hashing tests, 4 object-interop tests, one authority-transition unit,
one authorization-boundary unit, one duplicate-semantic-event unit, one
environment-binding unit, 2 portable-divergence tests, and one correction
integration test. The independent authority-chain verifier accepted 6 records
and 11 authority events and rejected all 13 named negative vectors. The
correction verifier reported
`sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6`.
Portable-divergence bundle verification now runs against an explicitly
initialized empty bare repository in both its Rust integration test and the
matching Python reference-flow check, so neither check can borrow objects from
an ambient source checkout.

## Boundaries and transfer

| Area | Classification | Reason / next boundary |
| --- | --- | --- |
| source-specific proofs, computations, campaign scheduling, and scientific dossiers | out-of-scope / domain-owned | belongs in each source-owning Repository, not this artifact or Vela Core |
| hosted views and product presentation | out-of-scope / domain-owned | belongs in `vela-web` |
| transfer the seven valid-suffix rejection patterns to current Core tests where a concrete missing regression is later demonstrated | future P1.5 transfer | use Core objects and roots there; never add them to proof history |
| shrink or promote a model mutation based on an observed Core defect | future P1.5 transfer | requires a concrete current failure and an owning Core boundary |

The inventory found no concrete uncovered current failure requiring a new Core
vector in P1.3. In particular, the model’s numeric root and Repository labels
are abstractions, not substitutes for canonical SHA-256 roots, UUID Repository
identity, DSSE, trust anchors, transaction read-set roots, or retained slices.
