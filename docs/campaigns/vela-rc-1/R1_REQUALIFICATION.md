# VELA-RC-1 R1 independent requalification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
PASS WITH DOC FIXES
```

The repaired shipped CLI now enforces the normative independent sequence-one
authority selection on every governed read. Missing, malformed, and mismatched
operating-system-account pins fail closed before Standing or authority history
is returned. The correct pin selects the retained lineage. A hostile `HOME`
cannot supply or override that selection.

Routine authenticated Submission, Verification, and producer withdrawal writes
remain possible without the pin and do not append authority Events or Authority
Records. Once the Proposal is complete, an attempted Decision without the pin
is refused; restoring the exact pin permits the ordinary authorized Decision.

No product semantic blocker reproduced. Three already-recorded documentation
errors remain in current text, so this report does not return an unqualified
`PASS`. This verdict is independent R1 gate evidence only. It is not a release,
publication, tag, push, or release authorization.

## Exact audit binding

| Field | Exact value |
| --- | --- |
| Starting HEAD / campaign freeze | `6750eb79fbe83ab106ad575357ea0f1775b38146` |
| Starting tree | `94f0fc73d98918cdddc9021d7df9d4b5c23e4d46` |
| Repaired product commit | `ad2a4516078525025d05bd461b550ed5b8e35971` |
| Repaired product tree | `e08112922efbe59ef3b042d0a8f6b0f9557761ea` |
| Freeze delta from product commit | campaign records only: `DECISIONS.md`, `FAILURES.md`, `QUALIFICATION.md`, and `STATE.md` |
| Initial checkout state | clean, detached at the exact freeze commit |
| Vela | `0.977.4` |
| Protocol | Vela Protocol 1 release candidate |
| Submission | `vela.submission.v3` |
| Verification Record | `vela.verification-record.v2` |
| Proposal | `vela.proposal.v1` |

The audit added only the test-only
`crates/vela-cli/tests/r1_requalification.rs`, this report, and the two index
links required to keep current-document discovery complete. It did not modify
product code, normative Protocol text, schemas, conformance fixtures, versions,
tags, publishing, or R3-R7.

## Normative-to-implementation audit

Normative `docs/PROTOCOL.md` requires strict replay to verify an independently
pinned sequence-one authority root, load the pin from the canonical
operating-system-account home, ignore `HOME`, and refuse missing, malformed, or
mismatched selection. It explicitly excludes routine producer Submission,
Verification, and withdrawal writes from that governed-read boundary.

The repaired implementation has one corresponding read boundary:

- `load_trusted_repository_at` and `verify_trusted_repository_at` first verify
  retained current state and then require the independently held pin;
- pin loading resolves the account home with `geteuid` / `getpwuid_r`,
  canonicalizes it, and never reads `HOME`;
- `replay`, `status`, `claims`, `show`, `why`, `log`, `review list`, `review
  show`, `review inbox`, `projection`, and `correction impact` all reach that
  boundary; and
- `submit`, `verification record|import`, and `review withdraw` retain the
  strict repository verifier and routine-evidence write barrier without using
  the trusted-read pin. Decision admission separately acquires repository
  authority and requires the exact pin.

Current `README.md`, `docs/PROTOCOL.md`, `docs/CLI.md`, `docs/QUICKSTART.md`,
`docs/CONTINUITY.md`, and `docs/interop/scientific-state-profile.md` agree with
that shipped behavior. The conformance vector still correctly describes its
own explicit external-anchor input rather than claiming that a vector alone
tests local CLI configuration.

## Direct shipped-CLI trust matrix

The audit-only integration target clones the frozen neutral-replay valid
history at commit/tree
`0bd019a846902c8e3e7802d6150063b475f144dc` /
`0983f52ac18e11897225087cf7aa919d459823cd` and uses fixture metadata, not
repository bytes, for sequence-one root
`sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753`.

It invokes all eleven shipped surfaces under each trust state:

| Trust state | Replay, status, claims, show, why, log, review list/show/inbox, projection, correction impact | Result |
| --- | --- | --- |
| Missing OS-account pin | Each command invoked directly | All 11 exited nonzero with the independent-pin refusal before governed output |
| Malformed OS-account pin | Exact pin path contains malformed JSON with private file mode | All 11 exited nonzero with the trust-anchor load refusal |
| Mismatched OS-account pin | Valid anchor document selects `sha256:` plus 64 zeroes | All 11 exited nonzero with the selection-mismatch refusal |
| Correct OS-account pin | Exact independently retained sequence-one root | Ten reads succeeded; `correction impact` crossed the trust boundary and returned its expected usage result because the neutral Claim has no correction relation |

The separate real correction lifecycle passed with the correct pin, including
positive `correction impact` projection over an accepted correction.

For hostile-`HOME` selection the audit installed a correct attacker-controlled
pin while the OS-account pin was absent: replay still refused. It then restored
the correct OS-account pin and installed a mismatched attacker-controlled pin:
all eleven commands still selected the OS-account lineage. The audit restored
or removed its temporary OS-account anchor after every run.

Refused reads did not mutate Repository state. Correctly pinned reads reproduced
Repository root
`sha256:6e7c2d797352a70b9d102f79baa9f3431631aa6ca240233f3dcd37d13f938e6a`.

## Routine writes and Decision boundary

The focused current-repository lifecycle removed the pin before importing an
authenticated Submission and two scoped Verification Records. Those writes
committed locally, changed no accepted Standing, and appended neither an
authority Event nor an Authority Record. With complete evidence present, an
attempted `review reject` without the pin failed with the independent-pin
diagnostic. Reinstalling the exact sequence-one pin permitted the authorized
rejection and left accepted Standing empty.

The separate shipped CLI wording/withdrawal lifecycle removed the pin, authored
a second Submission, and successfully recorded the producing actor's exact
Proposal withdrawal. Thus Submission, Verification, and withdrawal each have
direct unpinned CLI evidence rather than only a unit-policy assertion.

## Authoritative R1 conformance matrix

| Invariant | Executable evidence | Requalification result |
| --- | --- | --- |
| Submission | `review_acceptance`; current genesis lifecycle | `PASS`: authenticated input and pending Proposal retained; no Standing or authority-Event effect |
| Verification pass | `review_acceptance`; current genesis lifecycle | `PASS`: exact scoped records retained; no pre-Decision Standing effect |
| Verification fail / contradiction / incomplete evidence | `disposable_rejection_lifecycle` | `PASS`: evidence coexists and blocks acceptance without mutation |
| Unauthorized Decision | `review_acceptance`; current genesis lifecycle | `PASS`: signer or independent pin refusal leaves state unchanged |
| Authorized accept and reject | acceptance and rejection lifecycles | `PASS`: exact Events, attribution, roots, and Standing effects reproduced |
| Correction and supersession | `correction_impact`; authority interop; exact revise unit test | `PASS`: predecessor history retained and exact replacement semantics reproduced |
| Retraction | exact withdrawal and Event-link unit tests | `PASS` at current kernel/import boundary; direct retraction authoring remains an inherited ergonomic limitation |
| Proposal withdrawal | `wording_contract` shipped CLI lifecycle | `PASS`: producer withdrawal remains unprivileged and changes no Standing |
| Rejected-history preservation | rejection lifecycle | `PASS`: Submission, Verification, Proposal, Decision, and rejection Event remain addressable |
| Replay and deterministic Standing | acceptance, rejection, genesis, portable divergence, neutral replay | `PASS`: exact governed bytes reproduce Repository/Event roots and accepted sets |
| Missing/corrupt Artifact | genesis and neutral-replay negative history | `PASS`: strict replay refuses before reporting the prior root |
| Changed authority | independent R1 four-state CLI matrix and 13 authority falsifiers | `PASS`: external selection and every retained authority mutation fail closed |
| Hostile `HOME` | independent R1 two-direction environment test | `PASS`: environment cannot supply or override the OS-account selection |
| Changed Method reference | genesis and review-method checks | `PASS`: binding drift refuses; unavailable native rerun material is reported without changing governed replay |
| Canonical bytes, schemas, and roots | Rust/Python/JavaScript conformance and wire tests | `PASS`: all independent readers and negative vectors agree |

## Findings and classifications

| ID | Classification | Finding | Disposition |
| --- | --- | --- | --- |
| R1R-F001 | `RESOLVED RELEASE BLOCKER` | The prior shipped-read path did not enforce independent sequence-one selection. | Resolved on the exact repaired product tree. Direct four-state testing covers every named governed read and hostile `HOME`; no Protocol weakening or second mode appeared. |
| R1R-F002 | `DOC FIX REQUIRED` | `docs/ROOTS.md` still says the Proposal root covers the canonical record "and status". `vela.proposal.v1` has no status field; Proposal status is derived from withdrawals and Decisions/Events. | Correct the catalogue after this gate without changing object bytes or semantics. This is inherited R1-F002. |
| R1R-F003 | `DOC FIX REQUIRED` | `docs/campaigns/vela-rc-1/RELEASE_CHECKLIST.md` still asks for an expected "Standing digest", but Protocol 1 publishes no standalone `standing_root`. | Name the existing exact commitments: accepted set, Repository root, and authority Event-log root. This is inherited R1-F004. |
| R1R-F004 | `DOC FIX REQUIRED` | The exact semantic scenario matrix remains campaign evidence rather than a compact release-facing implementer index. | Publish or link an equivalent reviewed matrix during the later authorized documentation lane; do not make campaign prose normative. This is inherited R1-F003. |
| R1R-F005 | `ERGONOMIC LIMITATION` | Retraction remains authorable through imported signed Submission v3 and the Decision kernel, not direct `vela submit` flags. | Preserve as an explicit current limitation or separately scope a future CLI task. No Protocol change is required. |
| R1R-F006 | `NOT A PROBLEM` | The conformance README says its external-anchor vector does not itself claim that local CLI pin loading occurred. | This is an evidence-scope disclaimer, not a contradiction with the now-tested shipped CLI. |
| R1R-F007 | `NOT A PROBLEM` | The correct neutral fixture has no correction relation, so `correction impact` returns usage exit 2 after trust selection. | The trust boundary was selected correctly; the real correction lifecycle separately supplies the positive semantic case. |

No new release blocker, protocol question, wire change, product-code defect, or
security downgrade was found.

## Exact verification commands and results

Focused semantic matrix:

```text
cargo test --locked -p vela-cli --features test-support --test review_acceptance --test disposable_rejection_lifecycle --test correction_impact --test genesis --test review_method_check --test portable_divergence
```

Result: `PASS`, 10 passed and 0 failed across six integration targets.

```text
cargo test --locked -p vela-protocol --test authority_chain_interop --test canonical_hashing_conformance --test wire_schemas
```

Result: `PASS`, 9 passed and 0 failed, including all 13 authority falsifiers.

```text
cargo test --locked -p vela-cli --features test-support repository_decision::tests::withdrawal_accepts_only_the_exact_accepted_claim
cargo test --locked -p vela-cli --features test-support claim_standing::tests::an_accepted_withdrawal_retracts_rather_than_accepts
cargo test --locked -p vela-cli --features test-support repository_decision::tests::revise_replaces_exactly_one_predecessor
cargo test --locked -p vela-cli --features test-support repository_decision::tests::acceptance_links_one_domain_event_to_one_review_event
```

Result: `PASS`, four selected unit tests passed and 0 failed.

```text
cargo test --locked -p vela-cli --test r1_requalification -- --nocapture
```

Final result: `PASS`, 1 passed and 0 failed. The first audit-harness iteration
incorrectly expected the neutral non-correction Claim to produce a successful
correction projection; the product correctly returned usage exit 2. A second
harness iteration correctly recognized the semantic result but initially
expected generic exit 1 instead of the CLI's documented usage exit 2. Both were
audit-test expectation errors, not product failures; the final target encodes
the exact outcome and passed.

```text
cargo test --locked -p vela-cli --features test-support --test wording_contract the_cli_speaks_the_vocabulary_the_protocol_fixes -- --nocapture
```

Result: `PASS`, 1 passed and 0 failed; this directly includes unpinned
Submission and Proposal-withdrawal writes.

```text
uv run --project conformance --locked python conformance/verify.py
```

Result: `PASS`, 77 normative files, 44 informative files, 14 schemas, 18
positive objects, 37 negative schema cases, 179 portable patterns, four
reference flows, 13 authority falsifiers, and Protocol 1 root
`sha256:6a9d475c11db78faeb239a2f6c55b369b8b9a3f79c26c92cb59b7ae5eb2eb5d4`.

```text
uv run --project conformance --locked ./conformance/check-core.sh
```

Result: `PASS`. Ruff, the portable Protocol verifier, deterministic independent
readers and emitters, wire schemas, correction impact, authority-chain
falsifiers, reference flows, release-reproducibility fixtures, Decision Inbox
v3, the complete locked workspace all-target suite, the complete
`vela-cli/test-support` suite including crash recovery, and documentation tests
all passed. The documented external Lean suite was not selected.

```text
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Result: `PASS`, 0 warnings promoted to errors.

```text
cargo test --locked -p vela-protocol --test cli_release_contract
cargo fmt --all -- --check
git diff --check
```

Result: `PASS`, all 11 release-contract documentation tests passed, formatting
matched, and the final report/test-only diff had no whitespace errors.

No CI or hosted exact-tree acceptance is claimed by this local audit.
