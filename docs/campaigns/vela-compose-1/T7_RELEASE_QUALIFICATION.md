# T7 internal release-qualification matrix

Recorded: 2026-08-26, America/Toronto.

```text
Lane: T7 Product/Release Integration
Required supervisor commit: dc3d46a8c2ca7eeb03f22bfd1aee200f6cb11cb0
Required supervisor tree: 5bf9caf80db1fff4360e95ed3eff214053d34b49
Candidate status: INTEGRATED AFTER SUPERVISOR AUDIT
Integration status: INTERNALLY QUALIFIED
Release status: NOT RELEASED
```

## Scope and disposition vocabulary

VC1-D018 replaces the original T7 dependency on a positive handoff result with
authority to integrate only the behavior that qualified. This matrix
therefore covers the T7 contract while preserving T5 and T6 as terminal
negative gates. It does not reinterpret a missing result as a pass.

- `QUALIFIED` means current evidence and current-worktree tests support the
  bounded item.
- `QUALIFIED WITH LIMITATIONS` means the bounded behavior passed but the named
  adjacent claim did not.
- `UNQUALIFIED` means the campaign did not produce the evidence required for
  that claim.
- `NOT REQUIRED` means no qualifying change created the obligation.
- `NOT AUTHORIZED` means the campaign did not grant authority for the action.

## Acceptance matrix

| ID | Required T7 item | Direct campaign evidence | Current-worktree coverage | Disposition |
| --- | --- | --- | --- | --- |
| T7-A01 | Exact integration lineage and Git integrity | VC1-D018 and State 0016 authorize T7 from supervisor commit `dc3d46a8c2ca7eeb03f22bfd1aee200f6cb11cb0` | Required commit is the candidate parent; ancestry, `git fsck`, clean-start, final tree, and clean-end checks are part of the T7 receipt | `QUALIFIED` |
| T7-A02 | Kernel and Protocol 1 conformance | VC1-R002 and `T1_KERNEL_REPORT.md` cover multi-Verification admission, contradictory evidence, authority refusal, Event linkage, Standing invariance, rejection, correction, supersession, and retraction | Focused `review_acceptance` and `disposable_rejection_lifecycle` tests plus the Protocol 1 verifier exercise the claimed live surfaces | `QUALIFIED` |
| T7-A03 | Replay and receipt boundary | VC1-R001 and `T2_REPLAY_REPORT.md` cover same-checkout and clean-clone replay, retained Artifacts, Review Method drift, correction, rejection, divergent authority histories, and fail-closed integrity | Focused `genesis`, `correction_impact`, and `portable_divergence` tests plus Protocol 1 authority/correction vectors cover exact state reconstruction | `QUALIFIED WITH LIMITATIONS`: native computation, proof execution, instruments, and physical replication remain source-owned reruns |
| T7-A04 | Controlled branch and receipt integrity | VC1-R003 and `T3_BRANCHING_REPORT.md` bind identical branch-point state, divergent authorized Decisions, sealed task/evaluation/metering inputs, branch isolation, complete typed availability, and deterministic comparison | `counterfactual_branching` parses the three committed fixtures, binds their digests to execution, tests tamper/inventory refusals, and replays fresh terminal clones | `QUALIFIED WITH LIMITATIONS`: synthetic, test-only apparatus; no public `vela branch`, `vela diff`, or `vela compare` contract |
| T7-A05 | Verifier-rich Lean vertical | VC1-D009, VC1-E001/E002, and VC1-R004 bind source commit/tree `05b6e36fb46b840eeac533658faf6f71ad99dc06` / `4b491446071efc4d6cd306397fa33e8b008e2f29` and terminal Repository root `sha256:1f18d90faec38dfb602d1f6bfa51c0f7eb69373698baeb4e8f73cbf5dba5c82c` | Current Core lifecycle tests cover the general Submission, Verification, withdrawal, Decision, Event, Standing, and replay paths used; S0 audited the source-owned terminal evidence | `QUALIFIED WITH LIMITATIONS`: one real lifecycle, not theorem discovery or general prover evidence; producer authoring lacks the downstream canonical `depends` edge; zero-byte verifier streams need nonempty hash-receipt indirection |
| T7-A06 | Governed biological real-science vertical | VC1-D013/D015, VC1-E004, and VC1-R009 bind lifecycle commit/tree `363d1210e33f951739b6281097054f179ee04123` / `ba7c3381899c76ae972a71ed7355c3e1fcfc087c` and terminal Repository root `sha256:785fa897ac8ffa9e8dd92756090923ee9e8ce3ec593ed07bb73baa63aa58a79a` | Current general lifecycle/replay tests cover the Core behavior; S0 audited source custody, role separation, zero-Standing-delta Verifications, two fresh-root Decisions, correction lineage, strict replay, and fresh-clone reconstruction | `QUALIFIED WITH LIMITATIONS`: one governed literature-report lifecycle only; no biological discovery, effect estimate, clinical conclusion, or medical relevance |
| T7-A07 | Clean downstream continuation | VC1-D017, the VC1-E004 terminal addendum, and VC1-R011 preserve the one valid bounded attempt | The unchanged terminal root, empty Inbox, five Events, and zero new protocol objects were audited by S0 | `UNQUALIFIED`: the run exceeded the 15,000 observable-token cap and created zero pending Proposals; no retry is authorized |
| T7-A08 | R/E/V cumulative-handoff result | VC1-D014/D016/D018, the VC1-E003 terminal addenda, and VC1-R008/R010/R012 preserve both Stage-0 versions and every exclusion | The terminal records bind the independent hidden-evaluator and apparatus receipts; no participant cell exists for Core tests to replay | `UNQUALIFIED`: v1 and v1.1 produced 0 participant starts, 0 model starts, 0 scientific cells, and scientific denominator 0; hidden authorability failed and exact runtime/tokenizer qualification remained blocked; no R/E/V result exists |
| T7-A09 | CLI and documentation contract | Current CLI and Protocol 1 distinguish Submission, Verification Record, Proposal, Decision, Event history, and Standing; `CONTINUITY.md` distinguishes state replay from rerun | `cli_release_contract::the_documentation_index_lists_every_current_document` covers both T7 artifacts through `docs/README.md`; focused lifecycle tests cover the named CLI surfaces | `QUALIFIED WITH LIMITATIONS`: no product or Web surface changed; public branch comparison and producer-authored `depends` remain absent by design |
| T7-A10 | Protocol and software version declaration | `docs/PROTOCOL.md` declares Protocol 1 release-candidate status and Submission v3; workspace packages remain Vela `0.977.4` | Protocol 1 manifest verification recomputes the normative/informative inventory and root; Cargo metadata and the diff confirm no version edit | `QUALIFIED`: Protocol 1 and Vela `0.977.4` remain unchanged; this is not a Protocol 1.0 or software release |
| T7-A11 | Migration notes | T1-T5 required no Core semantic, schema, or wire change; T6 produced no qualified change | The candidate diff is documentation-only and Protocol 1 conformance remains on the existing surface | `NOT REQUIRED`: no schema, wire, canonical-byte, root, or persisted-data change; migration notes: **none** |
| T7-A12 | Release, publication, and external validation | VC1-D018 forbids release bump, tag, push, publication, and external validation | No release metadata, version, tag, publication, deployment, or provider state is changed by T7 | `NOT AUTHORIZED`: this is an internal candidate for independent supervisor audit, not a release |

## Coverage audit

The focused tests support the acceptance items as follows:

- `review_acceptance` and `disposable_rejection_lifecycle` exercise the T1
  authority and Standing claims through the shipped CLI.
- `genesis`, `correction_impact`, and `portable_divergence` exercise the T2
  reconstruction, retained-evidence, correction, and repository-local Standing
  claims. They do not execute native methods, which is the documented boundary.
- `counterfactual_branching` is the sole direct T3 apparatus test. It parses
  the committed task, evaluation, and metering fixtures and binds them to the
  execution path. The test provides no evidence for a public product command
  or a scientific treatment effect.
- Protocol 1 conformance covers canonical bytes, closed schemas, independent
  Submission/Verification implementations, authority-chain refusals,
  correction, Decision Inbox, reference flows, and release reproducibility. It
  establishes implementation agreement, not scientific acceptance or a
  release.
- T4 and T5 scientific bytes remain in their source-owning repositories. T7
  relies on the exact commits, trees, roots, hashes, and S0 audits in the
  campaign record. It neither copies nor regenerates that evidence.
- T5 continuation and T6 R/E/V have no positive test mapping because the frozen
  gates failed. Their correct dispositions are the preserved failures above.

No visual evidence is required: T7 changes no CLI rendering, Web page, or other
interactive surface.

## T7 current-state verification receipts

All commands ran in the generated T7 worktree from the required supervisor
parent. The final Git identity and cleanliness receipt is recorded after the
task commit.

```text
git merge-base --is-ancestor dc3d46a8c2ca7eeb03f22bfd1aee200f6cb11cb0 HEAD
PASS

git fsck --full --no-dangling
PASS

cargo test --locked -p vela-cli --features test-support \
  --test review_acceptance --test disposable_rejection_lifecycle \
  --test genesis --test correction_impact --test portable_divergence \
  --test counterfactual_branching
PASS: 10 tests, 0 failed

cargo test --locked -p vela-protocol --test cli_release_contract
PASS: 11 tests, 0 failed

uv run --project conformance --locked python conformance/verify.py
PASS: Protocol 1 root
sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd

uv run --project conformance --locked ./conformance/check-core.sh
PASS: ruff, Protocol 1 conformance, workspace all-target tests,
workspace all-target tests with vela-cli/test-support, and workspace doc tests
external Lean: not selected by the Core union

cargo fmt --all -- --check
PASS

cargo clippy --locked --workspace --all-targets -- -D warnings
PASS

terminal synthesis and cross-record identity checks
PASS: A/B/C/D/E structure, explicit cumulative-evidence and anomaly answers,
three next actions, VC1-D018 status note, and exact baseline/T4/T5 identities

git diff --check
PASS
```

## Internal candidate boundary

The candidate supports this statement:

> VELA-COMPOSE-1 is a Level-1 internal protocol proof across conformance,
> deterministic replay, controlled branch isolation, one verifier-rich Lean
> lifecycle, and one governed Alzheimer literature lifecycle. Cumulative
> workflow advantage remains weak and inconclusive because the T5 blind
> successor failed its frozen gate and the T6 R/E/V experiment produced no
> participant data.

It may not support Level 2, cumulative intelligence, productivity gain,
external validation, adoption, theorem discovery, biological discovery,
medical relevance, or a release claim.

## Supervisor integration receipt

S0 independently audited the task diff, preserved its documentation-only
scope, and integrated it with disposition `MERGE`.

```text
T7 task commit/tree: f124f9b8c533ab9890eb0329eb0115793c799cd2 / fd2662f0dd825e92f878dc5c45687851728cde9f
Integrated commit/tree: d0410cc0204e12b5f4167ebfee79bb0359389c66 / fd2662f0dd825e92f878dc5c45687851728cde9f
Direct supervisor parent: dc3d46a8c2ca7eeb03f22bfd1aee200f6cb11cb0
Independent fresh-clone focused CLI suite: PASS, 10 tests
Independent fresh-clone documentation contract: PASS, 11 tests
Independent fresh-clone Protocol 1 conformance: PASS
Protocol 1 root: sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd
Independent full Core union on supervisor parent: PASS
Independent T4/T5 strict replay roots: PASS
Release, tag, push, publication, deployment: NONE
```

The integration changes campaign documentation only. It does not promote an
unqualified item, alter an experiment, or authorize a release.
