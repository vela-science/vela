# VELA-RC-1 R4 external-use fixtures qualification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
PASS WITH DOCUMENTED LIMITATIONS
```

Two independent, release-facing examples complete the documented Vela loop
with unmodified Core and public commands. Example A preserves a false Proposal,
failed Verification, fail-closed attempted acceptance, and attributed rejection
before a separate corrected Proposal passes exhaustive Verification and enters
Standing through an authorized Decision. Example B binds heterogeneous CSV,
code, JSON, and prose evidence; remains blocked after only exact recomputation;
and reaches Standing only after a separate evidence-scope review and qualified
Decision. Both terminal histories replay from complete Git bundles with exact
roots, and both missing-evidence branches fail closed.

The limitation is evidence breadth, not a domain-interface failure. Lean was
not selected because a four-assignment Python-standard-library verifier gives a
complete formal check without adding a clean-install toolchain. The candidate
was source-built and exercised on macOS Apple silicon; this lane did not create
or test signed candidate release bytes or a second supported operating system.

This is R4 gate evidence only. It is not a release, tag, push, publication,
scientific result beyond the exact retained fixture claims, external adoption,
or authorization to change Protocol or packaging.

## Exact binding

| Field | Exact value |
| --- | --- |
| Delegated supervisor commit | `431120a6995d1b24ae2d50d6889878ce43efcd97` |
| Delegated supervisor tree | `8fa55a2094569561d582629708f5588e4b5cc3ef` |
| Vela version | `0.977.4` |
| Local generation binary SHA-256 | `b23ffd6dd9f6d01235369386e4582b55350cd18af70a4129bd414b8b1e16803d` |
| Product / protocol / schema changes | none |
| New Vela object or field | none |
| Campaign-local runtime dependency | none |

The local binary digest identifies only this source build. It is not a signed
release digest and is not claimed to be reproducible across toolchains, paths,
or hosts.

## Example A — verifier-rich failed and corrected Proposal

The source-owned verifier accepts a closed Boolean AST with variables `p` and
`q`, enumerates all four assignments, and retains every counterexample. It
reported four counterexamples for the bad assertion and none for the corrected
De Morgan assertion.

The bad Proposal received failing Verification
`vvr_fa502ad11e56aabb`. The Decision Inbox reported both
`failing_verification` and `missing_independent_passing_verification` blockers.
An authorized `review accept` exited 1 with
`current acceptance is blocked by a failing Verification Record`; replay before
and after reproduced Repository root
`sha256:1f251f51035e656c5cb3afe39a6d6433b9ad9be70bb7f4125b81971c6b4c9598`,
so the refusal mutated nothing. The attributed rejection then retained the
Submission, Proposal, failed Verification, output Artifact, Decision, and
rejection Event while leaving the Claim `unassessed`.

Because the bad Claim never entered Standing, the corrected work is honestly a
new `claim.add` Proposal rather than a `--corrects` relation to a nonexistent
accepted predecessor. Its exhaustive Verification passed, the Decision Inbox
became protocol-satisfied, and an attributed authorized accept admitted it.

| Boundary | Exact result |
| --- | --- |
| Bundle SHA-256 | `c5e25d570c03b3c0638b29745088184b37d08f1a6fcc18b7efd335259aa83645` |
| Sequence-one root | `sha256:d0b5be6a10ba9feac7040ddf9e3a248f882197ac5db3a825ebd148bf5243a314` |
| Valid Git commit / tree | `840702a681adfcc47e0354b07e1cea154157da33` / `ca7f3a9c868d60f477c4b202c62e7ae871f3f60c` |
| Terminal Repository root | `sha256:792e6fe849303a4da0a7f6a14018b3da5884f1f41311d441215dadf93af31011` |
| Accepted-set fixture commitment | `sha256:2576f7a444fe2b5bbfb0d6a3531948749bc857ede30067600752059be814501f` |
| Bad Proposal / Claim | `vpr_5a3dadd961d0b9cc` / `vcl_991c14480535ef573491e7b8b43d626af5147bc0bcb305633e9e64f0f7005d8b` |
| Bad Verification / outcome | `vvr_fa502ad11e56aabb` / `fail` |
| Rejection authority record | `sha256:a46e398c9c4c4df0a523ef5187053241eb33cb16ab40a20fba05adea5053f26c` |
| Corrected Proposal / Claim | `vpr_50b918b048e8d45c` / `vcl_36fa33468804142cabd939251c1a328965018565411c2ef51c3ba1211cbb7e09` |
| Corrected Verification / outcome | `vvr_ff9f97310aa8dd09` / `pass` |
| Accept authority record | `sha256:cad741abec3e9ee64e6c69f493c019465ca06af6d66491b1b009ed1e4d16b6b4` |
| Terminal counts | 2 Submissions, 2 Proposals, 2 Verifications, 5 Artifacts, 1 accepted Claim, 0 pending |

The `failed-proposal` bundle branch freezes the blocked pre-Decision state. The
`missing-artifact` branch removes required corrected verifier-output object
`e414e339...`; replay exits 1 and returns no Repository root or partial
Standing.

## Example B — heterogeneous bounded evidence

The Submission binds four different evidence kinds: raw CSV observations, the
exact Python analysis, its JSON result, and a prose method note. Verification
`vvr_071cb93c5b5d49e8` independently recomputes the exact result. At that point
the `incomplete-review` branch remains blocked because
`evidence_scope_review` is missing. Verification
`vvr_94cd27b885c05cfc` separately compares the Claim with every evidence kind
and explicit nonclaim. Only the two-record set makes the Decision Inbox
protocol-satisfied.

The Decision reason is qualified to the exact retained rows: it does not turn
fixture data into an instrument-accuracy, measurement-provenance,
representativeness, causal, or external-validity claim.

| Boundary | Exact result |
| --- | --- |
| Bundle SHA-256 | `4178258e612cfbc0f7cb52e2ecd816c6e39b92f65c4ec5e17cb7135bb121915c` |
| Sequence-one root | `sha256:687ffcf1d67ef4da4d623f516c179f83484661ce071c7efc4e8b477ded5f799d` |
| Valid Git commit / tree | `659107e30ad21c2d1c41f423b043df6646fff399` / `871720fd884ff65047e0561129617a83fcbe81c7` |
| Terminal Repository root | `sha256:97e508de9e08b272eeb5b1d0fd0a581180adb829fa8c77b59e040bd40d759f58` |
| Accepted-set fixture commitment | `sha256:175f647bf32fcd0ab78124199cd7936f3aa58c1b548367f1fb59c0c71cde725e` |
| Proposal / Claim | `vpr_ca75cd93988142d0` / `vcl_e74720dcb9dc5a3925e29192a3e3a05ddc0dc62da1acc923f90f2042eaa43e50` |
| Exact recomputation Verification | `vvr_071cb93c5b5d49e8`, `pass` |
| Evidence-scope Verification | `vvr_94cd27b885c05cfc`, `pass` |
| Accept authority record | `sha256:98318c6bae27a410c236787bdb47651bbe8aaf66331531f9ff5f381beecb4378` |
| Terminal counts | 1 Submission, 1 Proposal, 2 Verifications, 5 Artifacts, 1 accepted Claim, 0 pending |

The `missing-artifact` branch removes required CSV object `8f127718...` while
retaining every reference. Replay exits 1 and returns no partial Standing.

## Clean-checkout and independent exercise

The checked-in shell verifiers and focused Rust integration target assert:

- each native method reproduces its retained output bytes;
- each bundle SHA-256, branch commit, tree, Repository root, object count, and
  fixture-local accepted-set commitment matches `expected.json`;
- the bad formal Proposal remains rejected and unassessed while the corrected
  Claim is accepted;
- the one-Verification heterogeneous branch remains protocol-blocked;
- every valid read uses the independent sequence-one pin; and
- each missing-Artifact replay exits 1 with exact `vela.error.v1` output and no
  partial Standing.

Exact clean-clone command:

```bash
cargo build --locked --release -p vela-cli
VELA_BIN="$PWD/target/release/vela" \
  examples/external-formal-verifier/check.sh
VELA_BIN="$PWD/target/release/vela" \
  examples/external-heterogeneous-evidence/check.sh
cargo test --locked -p vela-cli --test external_use_fixtures
```

The already qualified neutral replay fixture was reused as the harness pattern
and rerun as a separate baseline. It is not counted as either Example A or B,
and its one arithmetic lifecycle is not counted twice.

## Domain-interface classification

`PASS`: no domain-specific Vela Core fork is needed. Both native systems touch
Vela only through retained Artifacts, canonical Review Methods, scoped
Verification Records, attributed Decisions, Standing, and replay. The R4 diff
adds informative examples and one integration target; it changes no crate
implementation, protocol object, field, schema, authority rule, or command.

The release-facing friction encountered while generating the histories was
bounded and documented: `verification record --output` must be run from the
Repository so the tracked output path resolves in the documented context, and
the Decision Inbox readiness value is `satisfied`, not the prose synonym
“ready”. Neither required a Core change or fixture-specific field.

## R3 integration assumptions and overlap

While R4 was running, S0 reported R3 integrated separately at
`1b44b6e72f85329d58de7c0f928dc345ff5b17d1`, with informative Protocol root
`sha256:5be464c8c5968c93f2cabf2e73290894f9120963d3966482b27e970798586d97`.
R4 remains based exactly on its delegated parent and does not copy, rewrite, or
weaken R3.

- Direct file overlap: `docs/campaigns/vela-rc-1/README.md` and
  `docs/README.md`. R3 adds its report and requalification links; R4 adds this
  report to both indexes. S0 must retain every line.
- Manifest overlap: `conformance/verify_protocol_1.py` discovers
  `examples/**/*` as informative files. R4 intentionally does not edit
  `conformance/protocol-1.json`, because R3 already regenerated that manifest
  on another parent. After merging both lanes, S0 must regenerate once and
  record the new informative root rather than choosing either worker manifest.
- No other R3-touched file is changed by R4. In particular R4 leaves
  `docs/QUICKSTART.md`, `docs/ROOTS.md`, `conformance/README.md`, R3's release
  contract test, and the edited existing example READMEs untouched.

## Verification and handoff

Passed on the isolated delegated base:

```text
VELA_BIN=$PWD/target/release/vela examples/external-formal-verifier/check.sh
external formal verifier fixture: ok

VELA_BIN=$PWD/target/release/vela examples/external-heterogeneous-evidence/check.sh
external heterogeneous evidence fixture: ok

VELA_BIN=$PWD/target/release/vela examples/neutral-replay/check.sh
neutral replay fixture: ok

cargo test --locked -p vela-cli --test external_use_fixtures --test neutral_replay_fixture
3 passed; 0 failed

cargo clippy --locked -p vela-cli --test external_use_fixtures -- -D warnings
PASS

cargo fmt --all -- --check
git diff --check
PASS
```

Both Git bundles passed `git bundle verify` as complete histories, and the R4
example directories contain no OpenSSH private-key material.

Two integration-only checks are intentionally left for S0 after merging R3:

- `cargo test --locked -p vela-protocol --test cli_release_contract` passed 10
  of 11 tests on this delegated base. Its documentation-index test also
  requires the R2 requalification link that R3 adds; R4 adds its own required
  link. The merged indexes satisfy both inputs.
- `uv run --project conformance --locked python conformance/verify.py` stopped
  at `protocol-1: manifest drift`, as expected because R4 adds informative
  `examples/**/*` after R3's separate manifest regeneration. Per S0 direction,
  R4 did not bless a competing manifest root.

The exact committed fixture tree and independent clean-clone receipt are
appended in the handoff-only follow-up commit. No tag, push, publication,
version bump, signing, packaging, or release is authorized.
