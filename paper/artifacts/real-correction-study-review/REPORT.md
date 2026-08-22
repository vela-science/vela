# Independent review of the real-correction study packet

Verdict: **BLOCKED**

Reviewed at `2026-08-22T03:14:36Z` from a clean separate worktree after
refreshing `origin`.

## Exact review boundary

- Producer ref: `origin/codex/real-correction-fixtures`
- Producer commit: `4bebbe23f60fa2edd5cb683dc01e6344f9810878`
- Producer tree: `e98e984cd0dad06d577173afeedde9b5e7b5b50f`
- Packet tree: `9ee7a6ba4d30a0347cdaee9f408b42c8ed70346c`
- Exact base: `origin/main` at
  `4685462c44b1f073870f31025ae73d1d8770ce73`
- Base tree: `13c5e0cf2e64be907cee4c0fd740ab0027118e13`
- Qualification root:
  `sha256:4f2fee9dbb0f62550873daab4911564c4968ad9b84fe40ef3af83c6355b7832c`
- Qualification-result file SHA-256:
  `sha256:8bdb9ab292f4070809ff45c2300993e4a875f2318937e0dbd49a3f7f09ac5624`

The producer commit is the direct child of the exact base. The review did not
modify any producer byte.

## Blocking findings

### 1. The miss audit overstates cause

The immutable run evidence supports these facts exactly: `orderfix-run-25`
completed normally in `16.511676507` seconds, made zero tool calls, exited 0,
did not time out, emitted empty stderr, and had no schema-validation error. It
recovered `taxonomy-t7` -> `taxonomy-t8`, all four consequence
classification/action atoms, all four evidence bindings, and the observed
`authorized_status_change` effect. Its only scored mismatch was
`record_no_status_change` instead of the registered historical-action label
`accept_authorized_status_change`. The response schema allowed every
effect/action pairing and did not define a current-versus-historical temporal
axis. The 600-second value is the registered penalty for an inexact response,
not runtime.

The four recovered consequence atoms were exact:

- `cohort-count`: `affected` -> `recompute_with_successor`, bound to
  `sha256:fca4c6a82a5fded07b46a5b977a6e3609f2b66877e8e9a6c3471392c8ea4d81e`;
- `trend-model`: `must_reassess` -> `rerun_dependent_method`, bound to
  `sha256:899be5f752871d108a61739d8b3c64e4be0658582555e474cc9fe37668188b0b`;
- `freezer-commissioning`: `unaffected` ->
  `no_correction_reassessment`, bound to
  `sha256:a00e88127de3c207d720dc634928249ec47fb5e6b90fc2c957e928e87b539e39`;
- `combined-incidence`: `presently_unprovable` ->
  `retrieve_missing_premise`, bound to
  `sha256:c6f11fa287ee71eb03474ab33bfaa312810d4e717289a59b73f761161e0f7417`.

That establishes an ambiguous response contract and an output consistent with
one current-safe-action reading. It does not establish what caused this one
response. `failure-audit.json` calls the contract the
`primary_supported_cause`, and `claim-matrix.md` says the miss "was" a
temporal-contract conflict. Those formulations exceed the sealed evidence,
which cannot distinguish contract ambiguity from representation salience or
one-sample output variance. The supported classification is: contract defect
observed; response consistent with the ambiguity; cause unestablished; no
Protocol, Core, Repository-authority, or general model-capability defect
evidenced.

### 2. Erdős 264 authority and downstream provenance are not bound by this packet

The retained signature is cryptographically valid, and the two retained Event
files match the signed sequence-four authority record's object delta. However,
the packet does not name the source evidence repository, commit, or tree. It
does not retain or bind the independent trust anchor, sequences one through
three, the referenced previous authority-record root, the policy bundle and
authorization request/entity snapshot, the before/after Repository manifests,
or the exact Claim, Proposal, Submission, and Verification objects. The
verifier consequently trusts the keyset carried beside the record and checks a
signed `allow`; it does not verify an independently rooted Decision chain or
derive Standing.

The underlying first-party evidence does exist. Independent reconstruction
found byte-identical objects in the public archived
`https://github.com/vela-science/erdos-frontier.git` commit
`12fdb0ad09c710469e50a60e8a6e2c81c9d18c3f`, tree
`8b57c21c6c2a1ae279a3171cbad47291ab7af44c`. The selected roots are:

- keyset:
  `sha256:c4a88730dead6074cce49cce6649b23874dfed41c598815852f62ca65741f328`
- authority-record file:
  `sha256:e60bdf2c05f60deca8330f85bb4bd9aa41aced83d3de895709c8dfe13b8b1a7e`
- review Event:
  `sha256:2e4d2b9faf940b605dba9d0ebefaa08d93b214fc9b6fc4f8071aa5c1736833db`
- supersession Event:
  `sha256:caac7f2984f720cbc453acfa3e1e574b8aa4d218ede36195d4edc8c029e5febc`
- fidelity artifact:
  `sha256:4443284e9856a2df1902dd81fb443f4042fb28b510278bfa2fe23ef935be3173`
- proof-repair target:
  `sha256:112931d7959a3f9201ea4c8402daef3d91ae25410aba1c8fc6765ce69888e3de`

That independent lookup cannot repair the producer packet's missing binding.
The earlier source-first paper artifact at Vela commit
`c2f8f0eb47d62d232d437825f360c5d94f092c40` already records the evidence
repository commit/tree and exact Claim, Proposal, Verification, Decision, and
dependent-repair roots; the present packet neither imports nor cites that
binding.

The same problem affects the claimed bounded Erdős 264 consequences named
`independent-problem-claim`, `mutable-source-locator-claim`, and
`hosted-parts-i-proof`. They are labels and summaries here, not retained rooted
scientific records. The cited hosted proof is real and was independently found
at `plby/lean-proofs` commit
`68da20b96673899166e94638f5a7fffeb7231d35`, path
`src/v4.29.1/ErdosProblems/Erdos264.lean`, with the claimed root
`sha256:10c61b6082a51a85d7b0e41bffc7ee0799d46183b6a3848a9816cf9e943fedf2`;
its bytes and Git binding are not retained or verified by this packet.

### 3. The advertised qualification root does not bind material packet semantics

The exact README verifier output matches `qualification-result.json`, but its
root is a qualification-summary root, not an exact packet root. Independent
mutations showed:

- changing `discrimination-cases.json.source_atoms_root` still qualified with
  the identical advertised qualification root;
- changing material arm-contract semantics still qualified with the identical
  advertised qualification root;
- changing a Snake consequence classification or an Erdős 1055 fixture safe
  action still returned `qualified_for_open_method_development` (with a changed
  root) because the verifier checks declaration IDs and counts, not those
  classifications/actions;
- the README commands print the result but do not automatically compare it to
  the committed result/root.

The public discrimination mapping itself is exact by construction: one common
fact-only input and three distinct required actions make a deterministic
constant fact-only resolver `1/3`, while the registered authority-aware map is
`3/3`. Duplicate regimes, duplicate actions, and a distinct-action swap were
all rejected. This proves authority-input irreducibility only for the
constructed mapping. It is not evidence that participant Git/documents
performance is non-ceiling, and the unbound `source_atoms_root` must not be
described as a verified source equivalence commitment.

### 4. The focused formatting gate fails

`ruff check` passes, but `ruff format --check` reports that both `verify.py`
and `test_verify.py` would be reformatted. `git diff --check`, bytecode
compilation, the README verifier, and all four committed unit tests pass.

## Verified source fixtures

All three source transitions are genuine direct-parent corrections in a fresh
clean clone of `google-deepmind/formal-conjectures`. The retained bytes equal
the cited Git objects, and every full-index diff root reproduces:

| Fixture | Predecessor -> successor | Full-index diff root | Bounded count |
| --- | --- | --- | ---: |
| Erdős 264 | `593e6b76702c5dbffaaa91b59f4faaed705d04ce` -> `0598b8f281060a18416d60753fd75621d659bb07` | `sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f` | 8 (5 direct source consumers + 3 claimed local dispositions) |
| Snake-in-the-Box | `5dd9f04d6c53be13cdec8ba8792e242582a7f5c7` -> `89091814b683af5e580251762b8c5633bf53c6f2` | `sha256:c85fc2e5a4e3f3044d4cf9c96e909d51ec97cc3ef99813dce80475e097962772` | 9 file-local declarations |
| Erdős 1055 | `38d5a036c5005ff3e6c5fd91a5bd0e472565ee61` -> `ab7989605ff82ae5680812b2a70bfbf52c33fa87` | `sha256:3c2c03eb84a3662379108418710dff0733b2c49000f7ed2fd584daca464d5c50` | 7 atoms (6 declarations + unchanged class-one clause) |

The exact predecessor/successor object bindings are:

- Erdős 264: tree/blob/file-root
  `5e79f7198c3891bdbb3fc6ec10c2b2a804cc56cb` /
  `8490f7dc0575480c7729acd5713433fc0af9c71b` /
  `sha256:98386d8f28112c5e952ec40c4ee439c27f3ff7560a4e767b493ccebc628fb29f`
  -> `e040cfc1cd6e5d1a79cf156047f452c2268c1920` /
  `3ff5ce70001355549571a07eee77960939323b57` /
  `sha256:5a3a0fb7063ed77d644a5c1cab503851e68d87b02c0882db8fa52e801aba1166`.
- Snake-in-the-Box: tree/blob/file-root
  `d2f3e0b42fb191c12ea62ea7f402496d412a98f8` /
  `d1f8c21775ed62d222a0441a325b1b6209942d64` /
  `sha256:1f9d6e1cbd8a7d40717d97abfe3b618c88e32f6f4f2c2ad35fb17ee1069ab9fa`
  -> `2d573d1b3b082ea5de8b6f6a375d67788b019184` /
  `9682d8124dc617b7dc823afaf309ddd893279031` /
  `sha256:1781f117a71066c6d1e74c5fb04059889169ceb80d1e9d04cb0f48a706977bd3`.
- Erdős 1055: tree/blob/file-root
  `5b38f60870b297c7c17b057ebfd660114458bcd3` /
  `4835f12e96d618c7e9014f31a85f7549baf2ab79` /
  `sha256:ac988afdb5ab32d9dbfda77a94bc2859e0d873c9c09790a68acb85d4b1e959f9`
  -> `6a82a3ab09f5f7a1f729a2fdb8211b344cab0816` /
  `4348f2fe3b7f212801fb6f6be94400a29936c044` /
  `sha256:d3270825487fd25edb928565035455ca7ffa6b03a0dfaf44ba58d86abbd8e6d5`.

The Snake and Erdős 1055 inventories are bounded to real retained upstream
Lean declarations, not placeholder records. Their authority regimes are
explicit prospective evaluation contexts: Snake has no Decision or authorized
acceptance action by construction; Erdős 1055 declares a Decision present but
with no retained authorization evidence, so status change is presently
unprovable. Neither regime derives authority or Standing from the upstream Git
merge. For Erdős 264, the source-level five-consumer closure is exact, but the
three local scientific-record dispositions remain blocked as described above.

One Erdős 1055 action atom is also not justified by the retained source. Both
asymptotic consequences use `reverify_finite_change_invariance`, but the
successor adds lower-class exclusion at every recursive class, not only at
class 2. The upstream correction cites the illustrative change
`p 2 = 2` -> `p 2 = 13`; the packet does not verify that computation and does
not prove that the sequences differ at only finitely many indices.
`must_reassess` is proportionate, while the first safe action must remain a
generic rederivation/reverification under the successor unless a finite-change
lemma is retained and bound.

## Research-design assessment

The design correctly keeps three arms, identical semantic atoms, a fixed
36-cell denominator, component outcomes, family-visible results, explicit
authority errors, and strict-increment estimands. Equality cannot count as
governance/inheritance lift. `confirmatory_freeze_allowed` is false. The three
open fixtures are excluded from confirmatory scoring. Fresh held-out families,
an independently authorized open pilot establishing a non-ceiling but usable
Git/documents baseline, and independent methodological/custody review remain
mandatory before any freeze.

The producer diff contains no newly executed participant/provider artifact,
new score, protected adjudication key, Core/Protocol change, scientific
authority mutation, Standing action, positive lift claim, merge, or pilot
launch. It only audits the immutable earlier scored result. This review
performed none of those actions. The Git record cannot prove the absence of
off-repository activity; the supported statement is limited to the retained
branch and this review execution.

## Checks executed

- PASS: refreshed producer/base refs and clean-worktree bindings.
- PASS: README verifier output byte-equal to `qualification-result.json`.
- PASS: four committed unit tests.
- PASS: Python compile checks and `ruff check`.
- BLOCKED: `ruff format --check` on both Python files.
- PASS: `git diff --check` for base -> producer and the clean review tree.
- PASS: fresh-clone commit/tree/blob/byte/SHA-256 and full-index-diff
  reconstruction for all three source corrections.
- PASS: historical Erdős 264 source-first verifier and its five tests against
  the exact public Formal Conjectures and Erdős Repository commits.
- PASS: authority signature tamper, source-byte drift, and duplicate-regime
  committed mutations fail.
- PASS: discrimination duplicate-regime, duplicate-action, and swapped-action
  mutations fail.
- BLOCKED: unbound-root and unchecked semantic mutations described in finding
  3 pass qualification.

## Minimal corrective actions

1. Replace causal miss language with an observed-defect/consistent-explanation
   formulation; retain representation complexity and output variance as
   unresolved alternatives.
2. Bind the Erdős evidence repository URL, commit, tree, independent trust
   anchor, complete authority chain and referenced policy/authorization and
   Repository objects, or cite and cryptographically bind an immutable
   source-first artifact that does so. Verify Decision admission and Standing,
   not only an embedded-key signature and retained `allow` field.
3. Retain and root the exact three Erdős 264 local scientific records and the
   hosted proof bytes, then verify every consequence classification, action,
   and evidence binding for all fixtures. Replace the Erdős 1055
   `reverify_finite_change_invariance` actions with a non-presuppositional safe
   action unless finite-change evidence is added.
4. Publish an exact packet manifest/root covering the arm contract,
   discrimination source commitment, failure audit, fixtures, verifier, tests,
   and result. Make the README command fail unless the regenerated result and
   expected root are exact.
5. Add adversarial tests for every consequence/action atom, authority-scenario
   field, arm semantic, discrimination source root, external Git binding, and
   authority-chain predecessor/trust mutation; format the Python files.

Until these changes land on a new immutable producer commit, the claim ceiling
is limited to: three exact open real source corrections with bounded
source-level inventories; a by-construction authority-discrimination example;
and a design that correctly forbids confirmatory freeze and positive lift.
The packet is not yet qualified as exact authority-bound real-correction
research evidence.
