# Independent corrective re-review of the real-correction study packet

Verdict: **PASS**

Reviewed at `2026-08-22T03:54:44Z` from a fresh detached worktree after
refreshing `origin`. This review is a read-only evidentiary judgment. It creates
no authority, Decision, Event, or Standing effect and does not authorize a
pilot, provider call, score, protected key, merge, release, or outreach.

## Exact boundary

- Corrective producer ref: `origin/codex/real-correction-fixtures`
- Corrective producer commit:
  `9b7a3448d9b9925624a7f73fb0b712dfd14f8d34`
- Corrective producer tree:
  `efc28aec19efdccd4f436c3001f78378f5869450`
- Packet subtree: `cff690d5155092e54d1fbc0d6d32b8e0696fb5e4`
- Producer predecessor: `4bebbe23f60fa2edd5cb683dc01e6344f9810878`
- Predecessor review: `2ff39cea36311bab5a36d5c85350fed4d9da1361`
- Original exact base: `4685462c44b1f073870f31025ae73d1d8770ce73`
- Original base tree: `13c5e0cf2e64be907cee4c0fd740ab0027118e13`
- Source-manifest content root:
  `sha256:d8987513e4860eaf5bf65f8ec9bd2be9a75691dbcc35de2c05b01fe400c66a17`
- Source-manifest file SHA-256:
  `sha256:e9cf24a5330f6cdf8a01e8798ce50bb16596a1057d2427bea740784abcb1bef5`
- Qualification root:
  `sha256:b08e1872be57e37857c740f82ae96df56ad023d91a345a8b73ebf25cba854165`
- Qualification-result file SHA-256:
  `sha256:a0a3ce16135712c49e2d9886abeca713558ebb9a4bac22bfb593a8b7fa4384e5`
- Packet root:
  `sha256:d2b9f5bc3cf95005c1964643fcb870133cc9e952bedc2a0d9ad6fc909579d71e`
- Packet-manifest file SHA-256:
  `sha256:1e788494282093d99ede48cc6e2c6a62a0db763156e47e3c9eb61313b7a70ae5`

The corrective commit is the direct child of the reviewed predecessor
producer. Its 68-file diff is confined to
`paper/artifacts/real-correction-study`. The detached producer checkout
remained clean. No producer byte was modified by this review.

## Findings

### Sealed miss audit

All seven cited blobs at historical result commit
`4524c8f776943a267e04e03e9a237ecaed14bc2c` reproduced with their declared Git
blob IDs, byte counts, and SHA-256 roots. `orderfix-run-25` completed normally
in exactly `16.511676507` seconds, made zero tool calls, exited 0, did not time
out, emitted empty stderr, and had no validation error. The 600-second value is
the preregistered exact-failure penalty, not runtime.

The response recovered `taxonomy-t7` -> `taxonomy-t8`, all four consequence
classifications and safe actions, and all four evidence bindings. It correctly
reported the observed effect `authorized_status_change` but paired it with the
currently safe `record_no_status_change`, while the registered historical
action was `accept_authorized_status_change`.

The corrected audit is proportionate. It establishes a response-contract
current-versus-historical ambiguity and says the response is consistent with
one reading. It explicitly leaves cause unestablished. Representation
complexity and one-sample output variance remain secondary unproven
hypotheses. It claims no general authority misunderstanding, runtime failure,
or Protocol/Core/Repository-authority defect.

### Exact source corrections and bounded consequences

Fresh filtered clones of `google-deepmind/formal-conjectures`,
`vela-science/erdos-frontier`, and `plby/lean-proofs` reproduced every cited
commit, tree, blob, retained byte sequence, and full-index diff.

| Fixture | Exact predecessor -> successor | Full-index diff root | Complete bounded inventory |
| --- | --- | --- | ---: |
| Erdős 264 | `593e6b76702c5dbffaaa91b59f4faaed705d04ce` -> `0598b8f281060a18416d60753fd75621d659bb07` | `sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f` | 8 |
| Snake-in-the-Box | `5dd9f04d6c53be13cdec8ba8792e242582a7f5c7` -> `89091814b683af5e580251762b8c5633bf53c6f2` | `sha256:c85fc2e5a4e3f3044d4cf9c96e909d51ec97cc3ef99813dce80475e097962772` | 9 |
| Erdős 1055 | `38d5a036c5005ff3e6c5fd91a5bd0e472565ee61` -> `ab7989605ff82ae5680812b2a70bfbf52c33fa87` | `sha256:3c2c03eb84a3662379108418710dff0733b2c49000f7ed2fd584daca464d5c50` | 7 |

These are genuine source corrections: Erdős 264 changes the perturbation from
natural-valued to bounded integer-valued; Snake replaces vertex-set equality
with equality to the path subgraph and repairs the local zero-dimensional
proof; Erdős 1055 adds exclusion from all lower classes. The verifier closes
the declared file-local dependency scopes against the exact successor
declarations. Erdős 1055 makes no finite-change claim; both asymptotic actions
are the non-presuppositional `rederive_and_reverify_under_successor`.

For Erdős 264, the retained downstream material is not a placeholder layer. It
is byte-identical to first-party evidence Repository commit
`12fdb0ad09c710469e50a60e8a6e2c81c9d18c3f`, tree
`8b57c21c6c2a1ae279a3171cbad47291ab7af44c`, Repository-manifest root
`sha256:f03be3a76ce43be0c2f9ca63ff731b9a5ff5c010b768e95b46a35f3a067eed96`.
The packet retains and roots the independent-problem Claim, superseded mutable
source-locator Claim, hosted-proof Claim, the historical hosted proof bytes,
the exact correction objects, and the accepted repaired proof objects. The
hosted predecessor proof is correctly classified for rebinding and
reverification; it is not presented as proof of the corrected theorem. The
accepted repair has its exact Formal Conjectures source definition and retained
proof bytes, but one repair does not establish complete propagation.

### Authority regimes

The three regimes are kept separate from Git state.

- Erdős 264: the packet-declared trust root
  `sha256:c4a88730dead6074cce49cce6649b23874dfed41c598815852f62ca65741f328`
  verifies five linked DSSE authority records. All five signatures, predecessor
  roots, event-log commitments, policy/material roots, entity snapshot,
  authorization request commitments, and Repository before/after roots
  reproduce. The simple Cedar policy permits the retained human principal's
  exact `authority_initialize` and `review_accept` actions. Sequence 4 admits
  the correction and supersedes the predecessor; sequence 5 admits the repaired
  proof Claim. The corresponding local Standing transitions reproduce.
- Snake-in-the-Box: no authorized acceptance action is retained or asserted;
  the only safe prospective action is submission preparation with no status
  change.
- Erdős 1055: a Decision is posited for the prospective scenario but no
  authorization chain is retained, so status change is presently unprovable
  and must be withheld pending the chain.

The trust root is declared by and bound into this packet, not obtained from a
new external trust ceremony. The supported claim is therefore exact historical
Repository-local authorization at the bound roots. This review neither imports
that authority nor changes present Standing. No authority or Standing is
inferred from a source merge, commit, fact, signature alone, or Verification
Record alone.

### Discrimination and design

The public deterministic discrimination check reproduces exactly:

- fact-only constant resolver: `1/3`;
- authority-aware resolver: `3/3`;
- source-atoms root:
  `sha256:7b9005dd10c3dc959be485a29a369cc41280d77069e91d3878275c7198f34297`;
- case-contract root:
  `sha256:e588db44ce10e6bc78ff2dfca1a013a1e7d09248c7ecc89fee9fcb838687437f`.

The committed mutations reject arm, authority-regime, action, source,
downstream, discrimination code/output, trust/signature/predecessor, and
generated-layer drift. This proves authority-input irreducibility for the
constructed three-case mapping only. It does not show that participant
Git/documents performance is non-ceiling or that Vela has an empirical
advantage.

The proposed three-arm design preserves identical semantic atoms, a fixed
36-cell denominator, zero retry/substitution, strict-increment estimands, and
family-visible outcomes. Equality is failure for governance lift. The public
fixtures are excluded from confirmatory scoring. `confirmatory_freeze_allowed`,
`positive_lift_claim_allowed`, `protected_final_key_created`, and
`open_pilot_authorized` all remain false. A separately authorized independent
open pilot must first show at least 2/12 Git/documents failures and at least
6/12 exact passes without a response/runtime defect; fresh held-out families
must then be selected and independently custody-reviewed before any
confirmatory freeze.

### Whole-packet integrity and checks

The source manifest commits every retained material byte other than the three
explicit generated layers. The packet manifest then commits that manifest and
the byte-exact qualification result. Regenerating all generated layers in a
separate copy produced no diff.

- PASS: README deterministic verifier.
- PASS: README unit discovery, 13 adversarial tests.
- PASS: README external Git reconstruction against three fresh clones.
- PASS: independent regeneration plus recursive byte diff.
- PASS: all retained JSON parses with duplicate-key rejection in the verifier.
- PASS: locked Ruff check and format check for both Python files.
- PASS: `git diff --check` for predecessor -> corrective producer.
- PASS: clean detached producer status after all checks.

No participant/provider call, new score, protected-key creation/opening,
authority or Standing action, Core/Protocol change, merge, release, pilot, or
positive-lift claim is present in the reviewed producer diff or was performed
by this review. The Git record cannot establish absence of off-repository
activity; the conclusion is bounded to the retained producer bytes and this
review execution.

## Claim ceiling

The packet now supports: three exact open real source-correction fixtures with
bounded consequence inventories; exact historical Repository-local authority
evidence for Erdős 264 at the cited roots; two explicitly prospective authority
regimes; a by-construction authority-irreducibility check; and a gated research
design suitable for a separately authorized open pilot.

It does **not** support: a causal account of the sealed miss; a Protocol/Core or
Repository-authority defect; complete global correction propagation; external
adoption; non-ceiling participant Git/documents performance; structure,
governance/inheritance, or total lift; confirmatory readiness; general
productivity; scientific truth beyond the bound records; or present authority
outside the cited historical Repository roots.

No corrective action is required for this packet at this claim ceiling. The
remaining open-pilot, held-out-family, custody, authorization, and freeze gates
are prerequisites for future work, not defects repaired by this PASS.
