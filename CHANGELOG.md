# Changelog

## 0.38.3 - 2026-04-27

**Causal-claim category errors get linted.** Closes the v0.38.x arc:
the schema (v0.38.0), the math (v0.38.1), the inference filter
(v0.38.2), and now the structural lint that catches the most common
abuse — a `supports` link from a weaker causal claim to a stronger
one.

### The category error

A finding that claims correlation has, by design, no business
"supporting" a finding that claims intervention. Correlation alone
cannot identify a causal effect; reading the link as if it does is
the textbook category error in causal inference. v0.38.3 makes the
kernel surface that mismatch on `vela lint`.

### New rule

```
L011  causal_mismatch_supports  warning
```

Strength order: `Correlation < Mediation < Intervention`. A `supports`
link is flagged when the source's claim rank is strictly lower than
the target's. Findings with `causal_claim = None` on either side are
skipped — the kernel doesn't yet know enough to judge.

### Examples

| Source claim → target claim (`supports`) | Verdict |
|---|---|
| Correlation → Correlation | clean |
| Correlation → Mediation | flagged (L011) |
| Correlation → Intervention | flagged (L011) |
| Mediation → Intervention | flagged (L011) |
| Intervention → Correlation | clean (stronger supports weaker) |
| Mediation → Correlation | clean |
| (any) ↔ ungraded | skipped |

### Doctrine

- The lint **only fires on `supports`.** Other link types
  (`contradicts`, `extends`, `depends`) carry different epistemic
  loads and aren't subject to the strength-monotonicity rule. A
  correlation finding can perfectly well *contradict* an
  intervention claim.
- The remediation suggestion gives reviewers two paths: re-grade the
  source (with appropriate evidence), or re-type the link to
  something weaker than `supports`.
- Cross-frontier link targets that aren't yet pulled stay silent.
  The kernel can't grade what it can't read.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **396/396 pass** (was 390; +6 lint tests
  in `lint::tests`):
  - correlation→intervention flagged
  - correlation→correlation clean
  - intervention→correlation clean (stronger supports weaker)
  - ungraded findings skipped
  - non-`supports` link types ignored
  - mediation→intervention flagged
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid; no L011 fires
  (causal fields not yet populated on BBB findings).

### What this closes

The v0.38.x arc was four substrate moves on causal typing:

| Version | Layer | Move |
|---|---|---|
| v0.38.0 | Schema | `causal_claim` + `causal_evidence_grade` first-class on `Assertion` |
| v0.38.1 | Math | `causal_consistency_multiplier` folded into the confidence formula |
| v0.38.2 | Inference | `AggregateFilter` lets consensus restrict to causal claim type / minimum grade |
| v0.38.3 | Structure | `L011` lint catches `supports` across claim-strength mismatch |

After this release, the kernel treats causal claims as first-class
in storage, reasoning, aggregation, and structural validation.
Future causal work (do-calculus, identifiability, full Pearlian
operators) builds on this foundation.

## 0.38.2 - 2026-04-27

**Aggregate queries gain a causal filter.** v0.38.0–.1 made causal
typing first-class and threaded it into the confidence formula. v0.38.2
extends the inference layer (v0.35) to read it: consensus over a target
finding can now be restricted to a specific causal claim type, a
minimum study-design grade, or both.

### Why

Pre-v0.38.2, `vela consensus vf_<id>` blended every claim-similar
finding regardless of what kind of claim each one made. Fine when the
question is "what does the field hold about TREM2?" — wrong when the
question is specifically "what does the field hold *as causation*?"
The math is fine; the question was undertyped. v0.38.2 lets you ask
the sharper question.

### Schema

```rust
pub struct AggregateFilter {
    pub causal_claim: Option<CausalClaim>,
    pub causal_grade_min: Option<CausalEvidenceGrade>,
}
```

Total order on `CausalEvidenceGrade` (used by `causal_grade_min`):
`Theoretical < Observational < QuasiExperimental < Rct`.

### API

- `aggregate::consensus_for_with_filter(project, target_id, weighting,
  &filter)` — new entry point.
- `aggregate::consensus_for(...)` preserved for back-compat; equivalent
  to `consensus_for_with_filter` with the default (no-op) filter.
- `ConsensusResult.filter: Option<AggregateFilter>` records the filter
  on the result so downstream views can label which question was asked.

### Filter doctrine

- The **target finding is always the anchor.** Filters apply to *neighbors*,
  not the target. The consensus is *about* this claim, not selecting *for*
  it. Asking "what does the field's RCT-grade evidence say about *this*
  observational finding?" must continue to anchor on the original.
- An **ungraded neighbor is excluded when `causal_grade_min` is set.**
  Without a grade we can't decide if it satisfies the minimum; the safer
  default is to omit rather than treat as bottom-tier.
- Both filters compose with **AND**: a finding must match the claim type
  *and* meet the grade minimum.

### CLI

```
vela consensus <vf_id> --frontier <path> \
    --causal-claim correlation|mediation|intervention \
    --causal-grade-min theoretical|observational|quasi_experimental|rct \
    --weighting composite
```

Either flag is optional; without them the v0.35 behavior is preserved.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **390/390 pass** (was 384; +6 filter tests
  in `aggregate.rs::v0_38_2_filter_tests`):
  - unfiltered blends all similar findings
  - claim filter keeps matching neighbors, anchor included
  - claim filter excludes non-matching neighbors
  - grade-min excludes lower grades
  - ungraded findings excluded when grade-min set
  - grade-rank total order
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid.

## 0.38.1 - 2026-04-27

**Causal typing reaches the math.** v0.38.0 made `causal_claim` and
`causal_evidence_grade` first-class fields on `Assertion`. v0.38.1
closes the loop the same way v0.36.1 closed it for replication: the
schema now drives the confidence formula. An RCT-grade intervention
gets a small bump; an observational-grade intervention claim gets a
meaningful penalty (the design doesn't actually identify the causal
effect being claimed).

### The multiplier matrix

```
                 RCT    QuasiExp.   Observational   Theoretical
Correlation     1.10    1.00        1.00            1.00
Mediation       1.10    1.05        0.85            0.90
Intervention    1.10    0.90        0.65            0.75
```

Doctrine encoded:
- **RCT supports any claim.** Gold standard; modest bump regardless.
- **Correlation is neutral under any reasonable design.** No claim, no
  penalty.
- **Mediation needs design that handles confounders.** Quasi-experimental
  is fine; observational and theoretical drop confidence.
- **Intervention is the strongest claim.** Without RCT or strong QE,
  the design under-supports the assertion. Observational-intervention
  gets a 0.65 multiplier — the substrate's clearest signal that the
  claim outruns the evidence.

### API

- `bundle::causal_consistency_multiplier(claim, grade) -> f64` — the
  pure-function matrix, useful for any second implementation that
  wants to reproduce the formula or build alternative reasoning.
- `compute_confidence_from_components` gains two trailing args:
  `causal_claim: Option<CausalClaim>` and
  `causal_evidence_grade: Option<CausalEvidenceGrade>`. `None` for
  either is neutral (multiplier 1.0).
- `Project::compute_confidence_for(&FindingBundle)` and
  `recompute_all_confidence(&mut findings, &replications)` now thread
  the assertion's causal fields through automatically.
- `ConfidenceComponents` gains a `causal_consistency: f64` field.
  Defaults to 1.0 when deserialized from pre-v0.38.1 frontiers.
- `formula_version` bumped from `"v0.6"` → `"v0.7"`.
- `Confidence.basis` string now includes `causal=<n.nn>` between
  `sample` and `review_penalty`.

### Backward compatibility

A finding with `causal_claim = None` and `causal_evidence_grade = None`
(every pre-v0.38.0 finding, including all 86 BBB findings today)
gets multiplier 1.0 — the score is unchanged. Verified by
`confidence_score_unchanged_for_pre_v0_38_findings` test.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **384/384 pass** (was 378; +6 causal-math
  tests in `bundle.rs`):
  - `causal_multiplier_neutral_when_either_field_none`
  - `rct_grade_bumps_any_claim`
  - `observational_intervention_gets_strong_penalty`
  - `correlation_neutral_under_any_grade`
  - `confidence_score_unchanged_for_pre_v0_38_findings`
  - `intervention_from_observational_drops_score_meaningfully`
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid; scores unchanged
  (all findings carry `causal_claim = None`, multiplier = 1.0).

### What's next

The reasoning surface now has *one* causal-aware mechanism: the
confidence formula. Two more reasoning moves remain in the v0.38.x
arc:
- v0.38.2: aggregate queries gain a `causal_claim_filter` parameter
  so consensus can be computed over interventions only or
  correlations only — distinct credible intervals for distinct kinds
  of claim.
- v0.38.3: lint check for `supports` links across causal-claim
  mismatch (a finding that claims correlation supporting a finding
  that claims intervention is a category error worth surfacing).

Both are extensions of the schema landed in v0.38.0 — the math layer
in v0.38.1 is the foundation they build on.

## 0.38.0 - 2026-04-27

**Causal typing as a kernel-level primitive (schema layer).** Pre-v0.38
the substrate carried `Assertion.direction = Some("positive" |
"negative")` — enough to record covariance but not the difference
between "X correlates with Y," "X mediates Y → Z," and "setting X=x
changes Y." Real review work treats those as different epistemic
claims with different evidence requirements; conflating them produced
silent over-claiming.

This release lands the schema layer. The reasoning surface
(do-calculus, identifiability, derived bridges that propagate causal
vs correlational claims separately) ships in a follow-up — same
staging used for v0.32 (Replication as object) → v0.36.1
(`Project.replications` becomes the source of truth for confidence).

### Schema

`Assertion` gains two optional fields. Pre-v0.38 frontiers serialize
and load byte-identically (both fields are
`#[serde(default, skip_serializing_if = "Option::is_none")]`).

```rust
pub causal_claim: Option<CausalClaim>,            // Correlation | Mediation | Intervention
pub causal_evidence_grade: Option<CausalEvidenceGrade>, // Rct | QuasiExperimental | Observational | Theoretical
```

The grade is what makes the difference between "the data is
consistent with X causing Y" (Observational) and "X causes Y" (Rct).
The kernel carries the design label so reviewers can re-grade without
re-extracting.

### New event kind

`assertion.reinterpreted_causal` — append-only record of who re-graded
a finding and why. Payload validated against the canonical-JSON
discipline used by every other kernel event:

```json
{
  "proposal_id": "vpr_<id>",
  "before": { "claim": "correlation",  "grade": "observational" },
  "after":  { "claim": "intervention", "grade": "rct" }
}
```

`before` / `after` blocks are required objects; their `claim` /
`grade` fields are optional but, when present, must come from the
canonical enums (`VALID_CAUSAL_CLAIMS` /
`VALID_CAUSAL_EVIDENCE_GRADES`). Pre-v0.38 findings carry no causal
metadata, so the first reinterpretation may originate from an empty
`before` block.

### CLI

```
vela finding causal-set <vf_id> --frontier <path> \
    --claim correlation|mediation|intervention \
    --grade rct|quasi_experimental|observational|theoretical \
    --actor reviewer:<id> \
    --reason "<one paragraph>"
```

Mutates the target finding's causal fields and appends an
`assertion.reinterpreted_causal` event capturing the prior reading.
Bypasses the proposal flow — the schema layer ships ahead of the
reasoning surface; v0.38.1+ will route through proposals once the
do-calculus layer needs it.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **378/378 pass** (was 374; +4 v0.38 tests).
  - 3 in `state.rs::v0_38_causal_tests`: writes-fields-and-event,
    rejects-invalid-claim, preserves-grade-when-only-claim-changes.
  - 1 in `events.rs`: validates_reinterpreted_causal_payload (5 cases:
    OK with full payload, OK with claim-only revision, rejects
    invalid claim, rejects invalid grade, rejects missing
    proposal_id).
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid.

### Why this is the largest substrate addition of the v0.36+ arc

Every other v0.36+ kernel object (replication, dataset, code, prediction,
multi-sig) describes a *thing* — an artifact, an actor, an event. Causal
typing describes how the kernel *interprets* a finding, which means it
touches the reasoning layer next, not just the storage layer. v0.38.0
ships only the storage; the reasoning move is deliberately staged to a
later release so the schema can settle through real use first.

## 0.37.0 - 2026-04-27

**Multi-actor joint signatures.** A finding can now require `k`
distinct registered actors to each contribute a valid Ed25519
signature before counting as `jointly_accepted`. This unblocks the
multi-lab review workflow that signed-but-single-actor v0.4 couldn't
represent: "the BBB result is accepted only when both lab A and lab B
have signed the canonical bytes."

### Schema

`Flags` gains two optional fields. Pre-v0.37 frontiers serialize and
load byte-identically.

```rust
pub signature_threshold: Option<u32>,  // None = single-sig regime
pub jointly_accepted: bool,            // derived; never written directly
```

### Multi-sig kernel

`Project.signatures` was already `Vec<SignedEnvelope>`; v0.37 codifies
the multi-sig semantics:

- `sign_frontier` dedupes by `(finding_id, public_key)`, not just
  `finding_id`. A second actor's `vela sign apply --private-key …`
  appends rather than skipping. Re-running with the same key stays
  idempotent.
- New helpers in `crate::sign`:
  - `signers_for(project, finding_id) -> Vec<String>` — unique pubkeys
    whose signatures over the canonical finding bytes verify.
  - `valid_signature_count(project, finding_id) -> usize`.
  - `threshold_met(project, finding_id) -> bool`.
  - `refresh_jointly_accepted(&mut project)` — idempotent; called from
    `sign_frontier` and `cmd_threshold_set` so the flag never drifts.
- `VerifyReport` gains `findings_with_threshold` and
  `jointly_accepted` counts.

### Events

Two new validated event kinds:

- `finding.threshold_set` — payload requires `threshold: u64 >= 1`.
- `finding.threshold_met` — payload requires `signature_count >= threshold`.

### CLI

```
vela sign threshold-set <vf_id> --frontier <path> --to <k>
```

Sets the policy on a finding, then re-derives `jointly_accepted` over
the existing signature pool. JSON emit reports whether the finding is
already accepted (signatures already present) or awaiting more.
`vela sign verify` now prints `findings_with_threshold` and
`jointly_accepted` counts when at least one finding has a policy.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **374/374 pass** (was 368; +6 multi-sig
  tests in `sign.rs`: dedupe-by-pubkey, k-of-n threshold met,
  None-policy never met, refresh idempotency, invalid-signature does
  not count, VerifyReport surfaces threshold counts).
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid.
- `Flags` gains a `Default` impl; 27 ctor sites across the workspace
  collapsed onto `Flags::default()` or `..Flags::default()` syntax.

### Why now

The previous milestones (replication, datasets, code, predictions,
inference, replication-as-source-of-truth) made the kernel's
*structural* claims first-class. Multi-sig is what lets the kernel's
*social* claims — "this is what the field collectively believes" —
get encoded with the same rigor. A single-actor signature says "I
stand behind this." A k-of-n threshold says "k of us, on the record."

Schnorr aggregation can wait; this is concatenated multi-sig, the
simpler primitive that ships first per the v0.37 plan.

## 0.36.2 - 2026-04-27

**Replication-as-source-of-truth, swept through every reader.**
v0.36.1 made `Project.replications` authoritative for the confidence
formula. This release closes the same wart everywhere else it had
quietly persisted, wires the cascade to its only natural caller, and
seeds the conformance test bed for the new propagation variant.

### B — `vela replicate` triggers the cascade

Pre-v0.36.2, `vela replicate` minted a `vrep_<id>` and persisted it,
but never invoked `propagate_correction`. The
`ReplicationOutcome { ... }` variant added in v0.36.1 was reachable
only from test code. As of this release:

- After save, `cmd_replicate` calls `propagate_correction` with the
  new variant. The target finding's confidence recomputes from the
  live `Project.replications` collection (closing the A.1 loop).
  Dependents linked via `supports` / `depends` are flagged
  `upstream_replication_failed | _partial | _succeeded`.
  `inconclusive` does not cascade.
- New `--no-cascade` flag stages a replication without immediate
  review-queue churn.
- Propagation events from both `cmd_replicate` and `cmd_propagate` now
  persist into `Project.review_events`. Pre-v0.36.2 these were emitted
  to stdout and forgotten — the kernel didn't carry the audit trail
  for why a finding got flagged.

### C — Kernel sweep for dual-source-of-truth warts

Audit found four readers still consulting the legacy
`Evidence.replicated` scalar instead of `Project.replications`. All
fixed with the same fall-through pattern as
`Project::compute_confidence_for`: if the structured collection has
records for a finding, it wins; otherwise the legacy scalar
fall-through preserves behavior on unmigrated frontiers.

| File | Reader | Severity |
|---|---|---|
| `project.rs::recompute_stats` | `ProjectStats.replicated` counter | high |
| `serve.rs::merge_projects` | merged-frontier replicated count | high |
| `observer.rs::score_finding` | replication multiplier in academic / clinical / pharma policies | high |
| `vela-hub::finding_state` | manifest-view "replicated" / "supported" label | medium |

`merge_projects` also now preserves `replications`, `datasets`,
`code_artifacts`, `predictions`, and `resolutions` across the merge.
Pre-v0.36.2 those collections were discarded, leaving merged stats
reading from a now-empty pool.

`observe()` and `score_finding()` API gain a `replications: &[Replication]`
parameter. All 22 test call sites and both production callers
(`serve.rs:1798`, `conformance.rs:767`) updated.

### F — Conformance tests for the new cascade

New gold suite `tests/conformance/replication-cascade.json` (5 cases):
failed → flag, replicated → flag + recompute, partial → flag,
inconclusive → no cascade, extends/contradicts links → no cascade.
The `run_retraction_propagation` harness extended to ingest a
`replications` array on case input and check `flag_types` on output.

Conformance count: 56 → 61 cases. Workspace tests: 368/368.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **368/368 pass**.
- `vela conformance`: 61/61 pass.
- `vela check projects/bbb-flagship`: 86/86 valid.

## 0.36.1 - 2026-04-27

**Replication is now the source of truth for confidence.** Two
substrate-correctness fixes that close the long-running "two sources of
truth" wart between the v0.32 `Replication` collection and the legacy
`Evidence.replicated: bool` / `Evidence.replication_count: u32` scalars.

Before this release, `compute_confidence` read the scalar flag
regardless of what `Project.replications` actually held — meaning a
finding's confidence was a function of the prose written when it was
first asserted, not of the replications subsequently recorded against
it. A replication added via `vela replicate` would land in the
collection but never reach the score. That is now fixed.

### A.1 — `Project::compute_confidence_for`

`bundle::compute_confidence` is split into a back-compat scalar wrapper
and a pure-math kernel `compute_confidence_from_components(evidence,
conditions, contested, n_replicated, n_failed, n_partial)`. The
replication-strength schedule becomes:

```
clamp(0.7 + 0.10 * n_replicated + 0.05 * n_partial - 0.10 * n_failed,
      0.4, 1.0)
```

The 0.4 floor keeps a single failed replication from zeroing the
score. `inconclusive` outcomes do not move the number — they encode
methodological ambiguity, not evidence.

A new `Project::compute_confidence_for(&FindingBundle)` derives
`(n_replicated, n_failed, n_partial)` from `self.replications` filtered
by `target_finding == bundle.id`. This is the authoritative path.
`bundle::recompute_all_confidence(&mut findings, &replications)` and
the `cmd_normalize` caller now use it; legacy callers without Project
context fall through `count_replication_outcomes` returning empty,
which routes back to the scalar — preserving prior behavior on
unmigrated frontiers.

### A.2 — Replication-aware propagation cascade

`PropagationAction` gains a `ReplicationOutcome { outcome, vrep_id }`
variant. When a `vrep_*` lands against a finding:

- the target's confidence is recomputed from the live
  `Project.replications` collection (closes the loop on A.1);
- dependents linked via `supports` / `depends` are flagged for review:
  `upstream_replication_failed`, `upstream_replication_partial`, or
  `upstream_replication_succeeded`. `inconclusive` does not cascade.

Three new tests: `failed_replication_flags_dependents`,
`successful_replication_recomputes_target_and_flags_dependents`,
`inconclusive_replication_does_not_cascade`.

### Why this was wrong before

The v0.32 promise was "replication is a first-class kernel object." The
implementation got it half-right: the object existed, the CLI verb
`vela replicate` minted them, the disk layout persisted them, the site
rendered them. But the confidence math kept reading the scalar that
v0.32 was supposed to make obsolete. Today's release closes that gap.

### Verification

- `cargo build --workspace`: clean.
- `cargo test --workspace`: **368/368 pass** (was 365; +3 cascade tests).
- `vela check projects/bbb-flagship`: 86/86 valid; 56/56 conformance
  tests pass.
- Backward compat: any frontier with no `Replication` records and the
  legacy scalar set produces the same score it did before.

## 0.36.0 - 2026-04-27

**Legacy ingestion regime removed.** The pre-v0.22 file-driven ingestion
path (`vela compile`, `vela jats`, `vela ingest`) is deleted. The agent
inbox (`vela scout`, `vela compile-notes`, `vela compile-code`,
`vela compile-data`) has fully replaced it for two minor versions; this
release stops carrying the dead code.

This is a *breaking CLI change* for any caller still relying on the
legacy commands. None exist in this repo or in any signed frontier on
the hub. The substrate semantics, on-disk shape, and signed manifests
are unchanged.

### CLI surfaces removed

| Removed | Replacement |
|---|---|
| `vela compile <topic-or-folder>` | `vela scout <pdf-folder> --frontier <path>` for paper extraction |
| `vela ingest --pdf/--csv/--text/--doi` | `vela scout` for PDFs, `vela compile-data` for CSV/Parquet, `vela compile-notes` for Markdown, manual `vela finding add` for direct claims |
| `vela jats <jats-or-pmcid>` | `vela scout` against locally-saved papers; PMC fetch is no longer in scope |

`SCIENCE_SUBCOMMANDS` allowlist trimmed to drop `compile`, `ingest`,
`jats`. The strict v0 release-command gate now refuses these as
unknown commands.

### Modules removed

vela-scientist (six legacy_* files):
- `legacy_compile.rs` (legacy compile pipeline body)
- `legacy_corpus.rs` (local-folder corpus orchestration)
- `legacy_extract.rs` (paper-text extraction)
- `legacy_ingest.rs` (file-ingest dispatch)
- `legacy_link.rs` (link inference for legacy compile)
- `legacy_llm.rs` (shared LLM config struct used only by the above)

vela-protocol:
- `fetch.rs` (Crossref/PubMed fetch utilities; only the legacy paths used these)
- `extract.rs` (paper-text → finding extraction; superseded by Scout)
- `jats.rs` (JATS XML parser; only `cmd_jats` used it)

vela-cli/main.rs: handler types `IngestHandler` / `CompileHandler` /
`JatsHandler`, the matching `register_*_handler` calls, and the three
adapter functions (`ingest_handler`, `compile_handler`, `jats_handler`)
that bridged into the now-deleted scientist modules.

vela-protocol/src/ingest.rs: collapsed from a 700-line legacy ingestion
driver to a 70-line `extract_pdf_text` utility. The function remains at
its old import path (`vela_protocol::ingest::extract_pdf_text`) so
Scout's import is unchanged.

### Confidence::raw() (renamed from Confidence::legacy)

The `Confidence::legacy()` constructor was named legacy by mistake when
the structured `components` breakdown shipped alongside it; the two
were always intended as parallel constructors, not as a deprecation
path. Renamed to `Confidence::raw()` across all 37 call sites in
vela-protocol and vela-scientist. Behavior unchanged.

### Site

`TerminalReplay.astro`: the v0.31 hero replay was demoing
`vela compile ./papers --output frontier.json`. Updated to show the
agent-inbox flow:

```
$ vela scout ./papers --frontier ./frontier.json
$ vela review-pending --frontier ./frontier.json --batch-size 8
$ vela queue sign --proposal vp_0a14b29c
$ vela registry publish ./frontier.json --to vela-hub.fly.dev
```

The hub URL also corrected from the placeholder `hub.vela.science` to
the actual `vela-hub.fly.dev`.

### Verification

- `cargo build --release --bin vela`: clean, no warnings.
- `cargo test --workspace --release`: **365/365 pass** (was 382; 17
  tests lived inside the deleted legacy_* modules and moved with them).
- `vela check projects/bbb-flagship`: 86/86 valid.
- `vela --version`: `vela 0.36.0`.
- Site build: 205 pages, clean.
- 9 source files removed from the substrate; ~3,000 lines of
  pre-arc plumbing gone.

### What's left of the codebase

Substrate after this pass — 41 modules in `vela-protocol/src/` (down
from 46), 11 in `vela-scientist/src/` (down from 17). Every module
serves the v0.32–v0.35 kernel primitives or one of the seven shipped
agents. No orphaned code.

The kernel-completeness frame closed at v0.35; the cleanup arc at
v0.35.1 + v0.36.0 removed the substrate's pre-arc skin. From this
point forward, every line in the protocol crate either holds a
kernel object or computes a derived view over them.

## 0.35.1 - 2026-04-27

**Cleanup pass — removing dead code and stale documentation.**

Linus-rigor audit of the full codebase. The kernel-completeness arc
(v0.32–v0.35) closed cleanly; this release removes pre-arc artifacts
that no longer pay rent. No new features, no schema changes, no
behavioral diff. Just code that stopped serving the substrate going
away.

### Rust modules removed

- `crates/vela-protocol/src/resolve.rs` — Stage 3b entity resolution
  against external public APIs (MeSH, UniProt, PubChem). Zero
  external references; never integrated into any v0.32+ flow.
  `entity_resolve.rs` (which resolves against bundled tables) is
  the live primitive and stays.
- `crates/vela-protocol/src/crossref.rs` — Crossref metadata
  enrichment (journal, publisher, license, funder, citation data).
  Zero references. The provenance fields it would have populated
  (`license`, `publisher`, `funders`, `citation_count`) exist in
  `bundle::Provenance` but are populated by other paths today.

Both modules removed from `lib.rs`. Workspace tests drop from 384
to 382 (resolve.rs had 2 internal unit tests that move with the
file).

### Site cleanup

- `site/src/components/FrontierDirectory.astro` — zero imports;
  stale UI experiment from the protocol-page redesign.
- `site/src/config.ts: ARCHIVE_URL` — defined but never imported
  anywhere. Removed.
- `site/src/pages/docs/[slug].astro` — `first-frontier` slug entry
  removed (its `docs/FIRST_FRONTIER.md` is gone — see below).
- `site/src/pages/docs/index.astro` and `site/src/pages/protocol/index.astro`
  — pruned the matching nav cards / list entries pointing at
  removed docs.

### Historical docs removed

Eight markdown files removed from `docs/` (29 → 21). All described
pre-v0.32 substrate behavior; keeping them would mislead any reader
encountering them as current spec.

| Removed | Reason |
|---|---|
| `AGENT_INBOX.md` | v0.22 spec; superseded by Scout / Notes / Code / Data CLI in v0.28+ |
| `V0_DOGFOOD_REPORT.md` | v0.0 dogfood notes; historical only |
| `V0_RELEASE_NOTES.md` | v0.0 release notes; superseded by `CHANGELOG.md` |
| `FIRST_FRONTIER.md` | v0.29 first-user onboarding; pre-arc |
| `FRONTIER_REVIEW.md` | v0.31 correction/proposal workflow; superseded by v0.32+ kernel |
| `SIM_USER_BBB.md` | v0.29 simulated-user trace |
| `EVAL_CARD.md` | v0.0 evaluation posture; not maintained |
| `PRIVATE_EVALUATOR_NOTE.md` | "internal pre-public" note marked confidential; should never have been in repo |

`README.md` updated to remove broken links to all of the above; the
documentation section now lists only specs that reflect current
substrate behavior.

### Live spec docs that stay

| Kept | Purpose |
|---|---|
| `PROTOCOL.md`, `MATH.md`, `THEORY.md`, `CORE_DOCTRINE.md`, `STATE_TRANSITION_SPEC.md` | normative + appendix |
| `CLI_JSON.md`, `BENCHMARKS.md`, `VELABENCH.md` | active CLI + bench surfaces |
| `REGISTRY.md`, `HUB.md`, `PUBLISHING.md`, `WORKBENCH.md` | distribution + tooling surfaces |
| `MCP.md`, `MCP_SETUP.md`, `PYTHON.md` | binding + tool-surface specs |
| `BRAND.md`, `TIERS.md`, `TRACE_FORMAT.md` | infrastructure |
| `PHASE_A_CONTENT_EXPANSION.md`, `SCHEMA_MISMATCH_AGENT_OUTPUTS.md` | active operational notes |
| `PROOF.md` | proof-packet contract |

### What this release does NOT remove

Several modules looked dead at first glance but turned out to be
load-bearing through indirect paths:

- `bridge.rs` — used by `cmd_bridge` for the `vela bridge` CLI.
- `entity_resolve.rs` — used by `cmd_entity_resolve` for `vela resolve`.
- `agent_bench.rs` — used by `cmd_agent_bench` for the v0.26 bench harness.
- `tool_registry.rs` — used heavily by `serve.rs` for the MCP tool surface.
- `permission.rs` — transitively used through `tool_registry`.
- `fetch.rs`, `extract.rs`, `jats.rs` — coupled to the legacy `vela compile`
  / `vela jats` ingestion paths. Reachable from the strict release
  CLI (`SCIENCE_SUBCOMMANDS`); deprecating them is a feature decision,
  not a cruft removal. Deferred to v0.36+ if/when the agent inbox
  fully replaces them.
- `Confidence::legacy()` constructor — wrongly named, but actively
  used by 37+ call sites. Renaming it is a larger refactor; deferred.
- `Evidence.replicated: bool` and `Evidence.replication_count: u32`
  fields — superseded conceptually by the v0.32 `Replication`
  collection but still consumed by `compute_confidence`. Migrating
  the confidence formula to derive these from `Project.replications`
  is real work; deferred.

These items are catalogued for future passes. The substrate stays
correct; the cruft that was pure cruft is now gone.

### Verification

- `cargo build --release --bin vela`: clean.
- `cargo test --workspace --release`: 382/382 pass (was 384; the 2
  missing tests lived inside `resolve.rs`).
- `vela check projects/bbb-flagship`: 86/86 valid.
- `vela --version`: `vela 0.35.1`.
- Site build: 205 pages, 1.32s (was 206 — `/docs/first-frontier`
  removed).
- Repo file count: 576 → ~565 (12 files removed in this release).

## 0.35.0 - 2026-04-27

**The inference layer** — consensus aggregation over claim-similar
findings. The fourth and final move in the v0.32–v0.35
kernel-completeness arc. Now that the substrate has replications,
computational provenance, predictions, and findings as first-class
objects, this version makes the kernel a *reasoning surface* over
all of them — not just storage.

A claim no longer answers only "what does this single finding
assert?" — it can also answer **"what does the field collectively
hold about this?"** That answer is computed deterministically from
canonical state, never stored, byte-identically reproducible.

### Substrate

- New module **`crates/vela-protocol/src/aggregate.rs`**:
  - `WeightingScheme` enum — `Unweighted` / `ReplicationWeighted` /
    `CitationWeighted` / `Composite` (default).
  - `ConsensusConstituent` — one finding's contribution: `raw_score`,
    `adjusted_score` (raw + 0.05 × successful replications − 0.10 ×
    failed replications, ×0.85 if contested, clamped to [0,1]),
    `weight` (per scheme), and replication tallies.
  - `ConsensusResult` — `target`, `n_findings`, `consensus_confidence`
    (weighted mean), `credible_interval_lo`/`hi` (95% from weighted SD),
    `constituents`, `weighting`.
  - `consensus_for(project, target_id, weighting)` — finds findings
    similar to the target via shared entities + text-token overlap +
    type-match (loose-OR), weights each, returns the structured result.
- Doctrine: aggregation is a **derived view, never written to disk**.
  Same input frontier produces byte-identical output every time.

### CLI

- `vela consensus <FRONTIER> <vf_id> [--weighting unweighted|replication|citation|composite] [--json]` —
  prints consensus headline, 95% credible interval, and constituents
  ranked by weight. Added to `SCIENCE_SUBCOMMANDS` allowlist.

### Site

- `site/src/lib/frontier.ts` — TS implementation of consensus
  (`consensusFor`, `WeightingScheme`, `ConsensusConstituent`,
  `ConsensusResult`). Mirrors the Rust module byte-for-byte so site
  rendering and `vela consensus` always agree on the same frontier.
- New route **`/consensus/[slug]`** — generated for every finding
  (86 pages). Shows the target claim, four-cell readout (consensus /
  95% credible / findings / weighting), and a constituents list
  sorted by weight with raw + adjusted scores and replication tallies.
- `/claims/[slug]` gains a "Field consensus →" link in the sidebar
  so readers can pivot from "this single claim" to "what the field
  holds."

### Worked examples

Two real consensus computations on the canonical Alzheimer's
Therapeutics frontier:

```
$ vela consensus projects/bbb-flagship vf_48c67a2c3ed1a369 --weighting composite
  target:           Amyloid-beta plaque burden correlates poorly
                    with cognitive decline in Alzheimer's patients.
  similar findings: 4
  consensus:        0.515  (0.470 – 0.559 95% credible)
```

The amyloid paradox: the target claim plus three similar findings
(two paraphrases of the decoupling, plus the tau-vs-amyloid
comparison). Replication credit on the target lifts its weight to
1.25; the credible interval narrows the disagreement to 0.470–0.559.

```
$ vela consensus projects/bbb-flagship vf_8389130295d81413 --weighting replication
  target:           ATV:TREM2 induces proliferation in human iPSC-derived microglia.
  similar findings: 4
  consensus:        0.300  (0.138 – 0.463 95% credible)
```

Four ATV:TREM2 findings (proliferation, glucose metabolism,
mitochondrial metabolism, signaling). Under replication weighting
the target with one successful replication carries weight 1.50 vs
1.00 for siblings; consensus 0.30 reflects the agent-extracted
findings' uniformly low confidence with a wide credible interval
(0.14–0.46) reflecting the low evidence density.

### Verification

- `cargo test --workspace --release`: 384/384 pass (no regressions).
- `vela check projects/bbb-flagship`: 86/86 valid.
- Site build clean (206 pages — 86 new `/consensus/[slug]` routes
  plus the existing 120).
- Site numbers match `vela consensus` exactly: same target, same
  weighting → identical consensus, credible interval, constituent
  weights.

### What this version does NOT yet ship (deferred to v0.35.x)

- `vela query "free-text question"` — natural-language consensus
  over the frontier. Requires a query→findings ranker that this
  release doesn't include; deferred to v0.35.x.
- `propagate.rs` positive cascade — failed-replication negative
  cascade and successful-replication positive cascade through the
  link graph. The aggregate module already weights individual
  findings by replications; the structural cascade through
  `supports`/`depends` links is pending.
- `bridge.rs` derived bridges — cross-frontier compositional
  hypotheses ("if A causes X in frontier1 and X causes B in
  frontier2, the chain A→B is a derived bridge"). The current
  bridge.rs surfaces entity-level bridges but not chain-derived
  ones. Deferred.
- `/api/aggregate` HTTP endpoint on `vela-hub`. The consensus
  computation lives in the protocol library; exposing it on the
  hub server is mechanical follow-up, not in this release.

### Where the kernel-completeness arc ends

This version closes the v0.32–v0.35 substrate frame. Vela now has
**eight first-class kernel objects**:

| Object | Introduced | Live count on canonical frontier |
|---|---|---|
| `vf_<hash>` finding | pre-v0.32 | 86 |
| `vfr_<hash>` frontier | pre-v0.32 | 1 |
| `vev_<hash>` event | pre-v0.32 | (canonical state log) |
| `vpr_<hash>` proposal | pre-v0.32 | 130 |
| `vrun_<hash>` agent run | pre-v0.32 | (provenance for proposals) |
| `vrep_<hash>` replication | v0.32 | 3 |
| `vd_<hash>` dataset | v0.33 | 3 |
| `vc_<hash>` code artifact | v0.33 | 3 |
| `vpred_<hash>` prediction | v0.34 | 5 |
| `vres_<hash>` resolution | v0.34 | 0 |

…plus single-actor Ed25519 signatures, registered actors, and the
inference layer (consensus aggregation + per-actor calibration)
shipped this release.

What remains for the longer-horizon arc:
- v0.36 — causal vs correlational typing on assertions
- v0.37 — multi-actor joint signatures
- v0.38 — hub federation

The substrate after v0.35 is *complete in scope* for "kernel for
science" purposes; v0.36+ rounds out structure and topology, not
core primitives.

## 0.34.1 - 2026-04-27

**Repo migration**: substrate source moved private; published artifacts
mirror to a separate public repo so `vela registry pull` continues to
verify against a public URL.

### Why

The Vela substrate is in active kernel-completeness development
(v0.32–v0.35 arc). It is not yet ready for the implicit social
contract that comes with a public source repo — stable APIs,
issue triage, presentable READMEs, named-and-documented behavior.
At the same time, the v0.30 hub publish set a `network_locator`
pointing at `raw.githubusercontent.com/vela-science/vela/...` —
making the source repo private without changing the locator would
break `vela registry pull` for anyone (including Will's own
scripts) holding a saved hub entry.

The migration resolves the tension cleanly: substrate source
goes private; only the locator-bound published artifacts mirror
to `vela-science/vela-frontiers`.

### What moved

New public repo: **`vela-science/vela-frontiers`**. Contains:
- `frontiers/alzheimers-therapeutics.json` (vfr_bd912b3e29e334ab,
  86 signed claims) — the canonical frontier the v0.30 hub publish
  pointed at.
- `frontiers/bbb-alzheimer.json`, `bbb-extension.json`,
  `will-alzheimer-landscape.json` — historical published frontiers.
- `benchmarks/leaderboard.json`, `benchmarks/gold-alzheimers.json`,
  `benchmarks/results/*.json` — VelaBench manifest + canonical gold
  + scored baselines, kept reachable for anyone reproducing the
  bench.
- `README.md` declaring scope: public mirror exists for
  locator-pull-integrity; substrate source lives privately.

What stays private (in `vela-science/vela`):
- `crates/*` (substrate source: protocol, cli, scientist, hub).
- `site/src/*` (the deployed `vela-site.fly.dev` source).
- `projects/bbb-flagship/.vela/*` (active development state for the
  canonical frontier — findings, replications, datasets, code
  artifacts, predictions, resolutions, proposals, events).
- `scripts/*`, `docs/*`, `Cargo.*`, `CHANGELOG.md`.

### Hub re-publish

Re-published `vfr_bd912b3e29e334ab` to `vela-hub.fly.dev` with
`network_locator =
https://raw.githubusercontent.com/vela-science/vela-frontiers/main/frontiers/alzheimers-therapeutics.json`.

The frontier file content is byte-identical (same snapshot hash
`1c5e2d43c5cfe68d39…`); only the locator URL differs.
End-to-end verification:
```
$ vela registry pull vfr_bd912b3e29e334ab --from https://vela-hub.fly.dev/entries
· registry pulled vfr_bd912b3e29e334ab → /tmp/pulled-via-mirror.json
  verified snapshot+event_log hashes match registry; signature ok
```

### Site config

- `REPO_URL` now points at `vela-science/vela-frontiers` (the
  public mirror).
- `REPO_RAW_BASE` updated to match.
- The site's "github" link in the masthead aside now resolves to
  the artifact mirror — the right thing to show a public visitor
  while the substrate source is private.

### Code artifact records (`vc_*`) note

The three `vc_*` records seeded in v0.33 still carry
`repo_url: https://github.com/vela-science/vela` + `git_commit:
ea0a1a4`. Those URLs return 404 to public visitors after the
visibility flip; the content_hash on each record remains the
integrity anchor. When the substrate source goes public again,
the URLs resolve. The records aren't rewritten because doing so
would change their content-addressed `vc_<id>` and break the
event chain.

### Hand-off

The technical migration is done in this commit: site config
updated, hub re-published, public mirror in place, end-to-end
pull verified. The visibility flip on `vela-science/vela`
happens in GitHub's repo settings — that's the user's last step.

## 0.34.0 - 2026-04-27

**Predictions and resolutions** — the kernel's epistemic accountability
ledger. The third move in the v0.32–v0.35 arc and the one with the
highest novelty premium: no other scientific knowledge graph I can
find tracks "claim X predicted Y; resolution was Z; actor calibration
shifts accordingly."

A `Prediction` carries a falsifiable claim about a future observation
plus an explicit resolution criterion plus a deadline plus the
predictor's confidence. A `Resolution` closes it out by recording
what actually happened. Calibration scores (Brier, log score, hit rate,
reliability diagram) flow deterministically from the resolved subset.
Every actor accumulates a public, reproducible track record of how
well their stated beliefs track reality.

### Substrate

- `bundle::ExpectedOutcome` — `Affirmed` / `Falsified` /
  `Quantitative { value, tolerance, units }` /
  `Categorical { value }`.
- `bundle::Prediction` — `id` (`vpred_<16hex>`), `claim_text`,
  `target_findings`, `predicted_at`, `resolves_by`,
  `resolution_criterion`, `expected_outcome`, `made_by`, `confidence`,
  `conditions`. Content-address: `SHA256(normalize(claim_text) |
  made_by | predicted_at | normalize(resolution_criterion) |
  expected_outcome.canonical())`.
- `bundle::Resolution` — `id` (`vres_<16hex>`), `prediction_id`,
  `actual_outcome`, `matched_expected`, `resolved_at`, `resolved_by`,
  `evidence`, `confidence`. Content-address: `SHA256(prediction_id |
  normalize(actual_outcome) | resolved_by | resolved_at | matched)`.
- `Project.predictions: Vec<Prediction>` and
  `Project.resolutions: Vec<Resolution>`. Both `serde(default)` +
  `skip_serializing_if = "Vec::is_empty"`. Pre-v0.34 frontier files
  load unchanged.
- All 5 `Project`-literal sites updated.
- `repo.rs` reads + writes `.vela/predictions/<vpred_id>.json` and
  `.vela/resolutions/<vres_id>.json`. Same one-file-per-record
  pattern as findings, replications, datasets, code-artifacts.
- New module `crates/vela-protocol/src/calibration.rs` —
  `CalibrationRecord` per actor with `n_predictions`, `n_resolved`,
  `n_hit`, `hit_rate`, `brier_score`, `log_score`, and a 5-band
  reliability diagram. Brier and log scores are computed
  deterministically from the resolved subset; never stored, always
  recomputed from canonical state.

### CLI

- `vela predict <FRONTIER> --by ... --claim "..." --criterion "..."
   --resolves-by RFC3339 --confidence 0..1 [--target vf_id1,vf_id2 ...
   --outcome (affirmed|falsified|quant:V±T units|cat:label)]` —
  registers a Prediction. Validates target findings exist; refuses
  to write duplicates.
- `vela resolve <FRONTIER> <vpred_id> --outcome "..." --matched true|false
   --by ... [--confidence ... --source-title ... --doi ...]` —
  closes out a prediction.
- `vela predictions <FRONTIER> [--by actor --open --json]` — lists
  predictions sorted by deadline; chips `open`/`hit`/`miss`.
- `vela calibration <FRONTIER> [--actor ... --json]` — prints
  calibration records per actor (Brier, log score, hit rate).
- All four added to `SCIENCE_SUBCOMMANDS` allowlist.

### Site

- `site/src/lib/frontier.ts` — `loadPredictions`, `loadResolutions`,
  `isResolved`, `resolutionFor`, `predictionsForFinding`,
  `calibrationRecords`, `calibrationFor`. The TS calibration math
  mirrors the Rust calibration module byte-for-byte so site rendering
  agrees with `vela calibration`.
- New `/predictions` route — open + resolved sections, each row with
  outcome chip (`open`/`hit`/`miss`), claim text, predictor,
  confidence, expected outcome, resolution criterion, target findings.
- New `/actors/[id]` route — per-actor calibration scoreboard with
  five-cell readout (predictions / resolved / hit rate / Brier / log
  score), reliability diagram for resolved predictions, and the full
  prediction list.
- Homepage instrument bar grew from 7 cells to **8**:
  `claims · papers · contradictions · replications · datasets · code · predictions · last signed`.
  Clickable predictions cell links to the registry.

### Seeded data

Five real Alzheimer's predictions made by `reviewer:will-blair`:
- `vpred_ce468cb7171efa89` — Lecanemab Phase 4 will show > 0.4 SD
  CDR-SB benefit at 36 months (resolves 2028-12-31, conf 0.55,
  quant outcome).
- `vpred_8d60594592016b23` — Donanemab full FDA approval by mid-2027
  (resolves 2027-06-30, conf 0.70, categorical outcome).
- `vpred_a5726af942eb0c55` — ATV:TREM2 (DNL919) advances to Phase 3
  by mid-2028 (resolves 2028-06-30, conf 0.45).
- `vpred_e94d6b71d4c6f562` — Next BACE1 inhibitor pivotal trial fails
  before 2028 (resolves 2027-12-31, conf 0.65, falsified outcome).
- `vpred_51ae0ed259d71f9a` — ApoE-targeting therapy positive Phase 2
  cognitive readout by 2027 (resolves 2027-12-31, conf 0.30).

All five are open. The calibration scoreboard at
`/actors/reviewer:will-blair` shows `5 predictions · 0 resolved` — as
real-world events unfold (Donanemab approval announcement,
Phase 4 readouts, etc.), `vela resolve` lands the resolutions and
the Brier / log score / hit rate populate.

### Verification

- `vela check projects/bbb-flagship`: 86/86 valid.
- `cargo test --workspace --release`: 384/384 pass.
- `vela predictions / vela calibration` round-trip the seeded records.
- Site build clean (120 pages); homepage shows 86/24/9/3/3/3/5/2026
  in the eight-cell readout; `/predictions` lists five open rows;
  `/actors/reviewer:will-blair` renders the calibration scoreboard.

### Why this is uniquely high-value

Every other v0.32–v0.35 substrate addition has analogues elsewhere
(replications in domain databases, datasets in DataLad, code in
Git+Zenodo, aggregation in any graph DB). Predictions + calibration
do not. A scientist who looks at Vela and sees "you can submit a
prediction here that automatically resolves and contributes to your
calibration record over time" has nothing else they can compare it to.

### Deferred to v0.34.x

- A "calibration leaderboard" sibling to VelaBench ranking actors by
  Brier score among resolved predictions.
- `prediction.made` / `prediction.resolved` event kinds with reducer
  wiring (today, persistence is direct).
- `prediction.add` / `resolution.record` proposal kinds for
  agent-proposed predictions.
- Per-claim "Predictions" panel on `/claims/[slug]` linking to
  predictions whose `target_findings` includes that finding.
- Experiment Planner agent emitting `prediction.add` proposals for
  hypotheses surfaced from notes.

The full v0.32–v0.35 roadmap continues at
`~/.claude/plans/noble-floating-willow.md`.

## 0.33.0 - 2026-04-27

**Computational provenance** — datasets (`vd_<hash>`) and code
artifacts (`vc_<hash>`) as first-class kernel objects. The substrate
move that turns "Git for science" from aspirational into operational:
claims literally reference the data and the code that produced them,
each pinned to a content hash and (for code) a specific git commit.

Before today, datasets were strings in `Provenance.title` and code
was a string in `Evidence.method`. A claim could say "we used ADNI"
without anchoring which release of ADNI; re-running the same code
on a refreshed cohort silently produced a "different" claim. Code
provenance was even thinner — there was no way to verify which
function in which commit produced a given finding's evidence.

### Substrate

- `bundle::Dataset`: `id` (`vd_<16hex>`), `name`, `version`, `schema`,
  `row_count`, `content_hash`, `url`, `license`, `provenance`,
  `created`. Content-address: `SHA256(name | version | content_hash | url)`.
  Two records with the same name + different version = distinct kernel
  objects.
- `bundle::CodeArtifact`: `id` (`vc_<16hex>`), `language`, `repo_url`,
  `git_commit`, `path`, `line_range`, `content_hash`, `entry_point`,
  `created`. Content-address: `SHA256(repo_url | git_commit | path |
  line_range | content_hash)`. Same code at two commits = two records.
- `Project.datasets: Vec<Dataset>` and
  `Project.code_artifacts: Vec<CodeArtifact>`, both `serde(default)`
  + `skip_serializing_if = "Vec::is_empty"`. Pre-v0.33 frontier files
  load unchanged. All 5 `Project`-literal call sites
  (`reducer.rs`, `serve.rs`, `lint.rs`, `cli.rs`, `sign.rs`)
  initialize the new fields.
- `repo.rs` reads + writes `.vela/datasets/<vd_id>.json` and
  `.vela/code-artifacts/<vc_id>.json` next to `.vela/findings/`. Same
  one-file-per-record persistence as findings and replications.

### CLI

- `vela dataset-add <FRONTIER> --name ... --version ... --content-hash ...
   --source-title ...` registers a Dataset.
- `vela datasets <FRONTIER>` lists registered datasets (human-readable
  or `--json`).
- `vela code-add <FRONTIER> --language ... --path ... --content-hash ...
   [--repo-url ... --commit ... --line-start N --line-end N --entry-point ...]`
  registers a CodeArtifact.
- `vela code-artifacts <FRONTIER>` lists registered code artifacts.
- All four commands added to `SCIENCE_SUBCOMMANDS` allowlist.

### Site

- `site/src/lib/frontier.ts`: new `loadDatasets`, `loadCodeArtifacts`,
  `findDataset`, `findCodeArtifact` helpers. Build-time read of
  `.vela/datasets/` and `.vela/code-artifacts/`.
- New `/datasets` route — registry of every `vd_*` record with name,
  version, content hash, URL, license, row count, source.
- New `/code` route — registry of every `vc_*` record with language
  chip, path, repo, commit, content hash, entry point. Each path
  links to the GitHub blob URL `repo/blob/<commit>/<path>#L<a>-L<b>`
  when `repo_url + git_commit` are set.
- Homepage instrument bar grew from five cells to **seven**:
  `claims · papers · contradictions · replications · datasets · code · last signed`.
  The two new cells are clickable, linking to the registries.
- Responsive: 7-cell readout wraps to 4-up at 1100px and 2-up at 720px.

### Seeded data

Three real datasets:
- `vd_d7af28baa9ea05f4` — **ADNI** @ ADNI-3, n=2300, the canonical
  longitudinal Alzheimer's neuroimaging cohort (DOI 10.1016/j.jalz.2010.03.012).
- `vd_5ad73dd5c181e10d` — **TRAILBLAZER-ALZ** @ TRAILBLAZER-ALZ 2
  (NCT04437511), n=1736, the donanemab Phase 3 trial (DOI 10.1001/jama.2023.13239).
- `vd_aa2be6fce05e944c` — **the canonical Alzheimer's Therapeutics
  frontier itself** (vfr_bd912b3e29e334ab, snapshot
  1c5e2d43c5cfe68d39…) treated as a dataset that downstream frontiers
  may pin to.

Three real code artifacts (all from this repository at commit
`ea0a1a4d9429cfff…`):
- `vc_56edd0bdbe5b1ee3` — Notes Compiler agent (rust,
  `crates/vela-scientist/src/notes.rs`).
- `vc_53bb21047e7f3e44` — Gold-from-frontier converter (python,
  `scripts/gold-from-frontier.py`) used by VelaBench.
- `vc_31ab8f762935d446` — Weekly diff script (bash,
  `scripts/weekly-diff.sh`).

These exercise three languages and pin against the most recent
public commit so the site links resolve to live GitHub blob URLs.

### Verification

- `vela check projects/bbb-flagship`: 86/86 valid, exit 0.
- `cargo test --workspace --release`: 384/384 pass (no regressions).
- `vela datasets projects/bbb-flagship` and
  `vela code-artifacts projects/bbb-flagship` round-trip the seeded
  records.
- Site build clean (118 pages); homepage shows 86/24/9/3/3/3/2026 in
  the new seven-cell readout; `/datasets` lists three rows;
  `/code` lists three rows with GitHub blob links.

### Deferred to v0.33.x

- `Provenance.dataset_refs: Vec<String>` and
  `Provenance.code_refs: Vec<String>` — explicit per-finding linking
  (vs frontier-level registry).
- `Evidence.computational_chain: Option<ComputationalChain>` — the
  chain that records "this code, applied to this dataset, produced
  this evidence."
- Datasets agent emitting `vd_*` objects alongside finding proposals.
- Code Analyst agent emitting `vc_*` objects with git-commit / line-
  range metadata.
- Per-claim "Computational chain" panel on `/claims/[slug]`.
- `dataset.added` / `dataset.updated` / `dataset.deprecated` /
  `code_artifact.linked` / `code_artifact.validated` event kinds
  (today, persistence is direct).

These are the cascades v0.33 unlocks but doesn't ship in the same
release. The kernel objects exist; per-finding linking and event-log
integration come next. See the roadmap at
`~/.claude/plans/noble-floating-willow.md` for the full v0.32–v0.35
arc.

## 0.32.0 - 2026-04-27

**Replication as a first-class kernel object** (`vrep_<hash>`). The
first move in the v0.32–0.35 kernel-completeness arc: science is made
of replications, not flags-on-findings.

Before today, replication was encoded on every finding as
`Evidence.replicated: bool` + `Evidence.replication_count: u32`. The
substrate could not represent "lab A replicated this in human iPSC;
lab B failed to replicate in mouse OPCs" — those are distinct
epistemic facts that flatten into a count. This release makes
each replication a content-addressed kernel object with its own
attempting actor, conditions, evidence, provenance, outcome, and
chain-of-prior-attempts.

### Substrate

- New `bundle::Replication` struct (`crates/vela-protocol/src/bundle.rs`):
  `id` (`vrep_<16hex>`), `target_finding` (`vf_<id>`), `attempted_by`,
  `outcome`, `evidence`, `conditions`, `provenance`, `notes`,
  `previous_attempt`. Content-address formula:
  `SHA256(target | attempted_by | normalize(conditions.text) | outcome)`.
- New `VALID_REPLICATION_OUTCOMES` allow-list:
  `replicated · failed · partial · inconclusive`.
- `FindingBundle::normalize_text` is now `pub` so `Replication`
  reuses the same canonical preimage rule as `vf_*` ids.
- `Project.replications: Vec<Replication>` (`project.rs`). All call
  sites that build a `Project` from scratch (`reducer.rs`, `serve.rs`,
  `lint.rs`, `cli.rs`, `sign.rs`) initialize the new field. Old
  frontier files load unchanged — the field is `#[serde(default)]`
  with `skip_serializing_if = "Vec::is_empty"`.
- `repo.rs` reads + writes `.vela/replications/<vrep_id>.json` next to
  `.vela/findings/`. Same one-file-per-record pattern as findings.

### CLI

- `vela replicate <FRONTIER> <vf_id> --outcome ... --by ... --conditions ...`
  appends a replication attempt and persists it. Validates the outcome
  against the allow-list, refuses to write if the target finding isn't
  in the frontier, idempotent on duplicate vrep ids.
- `vela replications <FRONTIER> [--target vf_id]` lists replications,
  with outcome-color chips in human output and structured JSON via
  `--json`.
- Both added to `SCIENCE_SUBCOMMANDS` so they pass the strict v0
  release-command gate.

### Site

- `site/src/lib/frontier.ts`: new `loadReplications`,
  `replicationsForFinding`, `replicationStats` helpers. Build-time
  read of `projects/bbb-flagship/.vela/replications/*.json`.
- `site/src/pages/claims/[slug].astro`: new "Replications" panel
  rendering each `vrep_*` record with an outcome chip
  (replicated=ok, failed=ember, partial=warn, inconclusive=stale),
  attempting actor, conditions text, reviewer note, and source
  citation (DOI / PubMed linkified).
- `site/src/pages/index.astro`: instrument bar grew from four cells
  to five — `claims · papers · contradictions · replications · last
  signed`. The number is the kernel-level count, not a flag derived
  from findings.

### Seeded data

Three real replication records on the canonical Alzheimer's
Therapeutics frontier:
- `vrep_83d7efaaf0b977d5` — ATV:TREM2 proliferation, replicated in
  independent human iPSC cohort.
- `vrep_930b01fd790c2fca` — amyloid-cognition decoupling, replicated
  in n=412 cross-sectional imaging cohort.
- `vrep_0b60abed760c048a` — lecanemab Clarity AD efficacy, partial
  outcome (primary CDR-SB endpoint replicates; one prespecified
  secondary did not reach significance).

These exercise all three non-trivial outcome kinds (replicated,
partial) and seed the visible chain on three drug-target areas
(TREM2, Aβ paradox, lecanemab).

### Verification

- `vela check projects/bbb-flagship` — 86/86 valid, exit 0.
- `cargo test --workspace --release` — 384 / 384 pass (no regressions).
- `vela replications projects/bbb-flagship` lists the three seeded
  records, outcomes color-coded.
- Site build clean (116 pages); homepage renders the 5-cell
  instrument bar; `/claims/<slug>` for the seeded targets renders
  the new Replications panel with proper outcome chip color.

### Deferred to v0.32.1+

The kernel object lands here. Layered on top in subsequent v0.32.x
releases:
- `replication.claimed` / `replication.validated` / `replication.failed`
  event kinds (today the persistence is direct).
- `replication.add` proposal kind so agents can propose replications
  through the standard review queue.
- `propagate.rs` extension: failed replication drops downstream
  confidence; successful replication in different conditions raises it.
- `benchmark.rs` extension: VelaBench composite gains a
  `replication_evidence_score` weighted at 0.10.
- A "replication scout" extension to the Literature Scout agent that
  recognizes replication papers and emits `replication.add` proposals.

These are the propagation + agent + bench cascades that v0.32 unlocks
but doesn't ship in the same release. The roadmap document at
`~/.claude/plans/noble-floating-willow.md` describes the full
v0.32–v0.35 arc.

## 0.31.0 - 2026-04-27

**VelaBench v0.31 — public agent leaderboard at `/bench`.**

The pull for agent-builders. The mechanic is SWE-bench-shaped:
the canonical signed Alzheimer's Therapeutics frontier is the gold;
anyone publishes a frontier (agent-generated or hand-curated); we
score precision, recall, F1, entity accuracy, assertion-type
accuracy, confidence calibration; composite is the headline
number.

The v0.26 candidate-vs-gold scoring engine in
`crates/vela-protocol/src/benchmark.rs` already existed and was
shelved before public exposure. v0.31 is what makes it real:
canonical gold derived from the 86-claim frontier, four scored
baselines, public leaderboard page, submission spec.

### What shipped

- **`benchmarks/gold-alzheimers.json`** — 86 canonical claims in
  `[GoldFinding]` shape (assertion text + type, entities, ±0.15
  confidence range), reproducible from
  `frontiers/alzheimers-therapeutics.json` via
  `scripts/gold-from-frontier.py`.
- **`benchmarks/leaderboard.json`** — manifest of submissions with
  submitter / kind / method metadata.
- **`benchmarks/results/*.json`** — four scored baselines:
  | Submission | Composite | F1 | Precision | Recall |
  |---|---|---|---|---|
  | Alzheimer's Therapeutics (canonical) | 1.000 | 1.000 | 100% | 100% |
  | BBB Flagship (v0.29 snapshot) | 0.858 | 0.716 | 100% | 55.8% |
  | Manual curation (12 claims) | 0.312 | 0.204 | 83.3% | 11.6% |
  | BBB Extension (probe) | 0.000 | 0.000 | 0% | 0% |
- **`/bench`** (new site route): how-it-works callout, leaderboard
  table with composite + F1 + precision + recall + matched + size,
  top-submission detail block, reproduce-and-submit terminal
  example, link to `/bench/submit`.
- **`/bench/submit`** (new site route): submission spec — file
  format, scoring weights, edge-case handling, reproduce command,
  PR-based v0.31 submission flow + planned hosted endpoint for
  v0.32.
- **`site/src/lib/bench.ts`** — build-time loader for the manifest
  + per-submission results, sorts gold first then by composite.
- **Rim nav**: `02 · Bench` slot.

### Mechanic

Scoring weights (composite blend):
- F1 (claim-by-claim text + entity match) — 0.50
- Entity accuracy (gold's named entities present in match) — 0.20
- Assertion-type accuracy (label agreement) — 0.20
- Confidence calibration (score within gold ±0.15) — 0.10

Reports include exact-id matches (the strongest signal), unmatched-
frontier claims (possibly novel contributions), unmatched-gold
claims (gold the candidate missed). Same JSON shape as
`vela bench --json` for either ad-hoc or CI-driven scoring.

### The pull

The agent benchmarks the field is full of (BIG-bench, MMLU,
SWE-bench, etc.) all share one trait: a public leaderboard creates
gravity. Submitting an agent run is the same shape of action as
opening a PR. With the canonical Alzheimer's frontier sitting at
`vfr_bd912b3e29e334ab` on `vela-hub.fly.dev`, every submission is
scored against real signed claims with real DOIs — not a synthetic
corpus. The substrate question becomes "how do I get on the
leaderboard," which carries Vela.

## 0.30.0 - 2026-04-27

**Topic-first vela.org, Phase A content expansion (48 → 86 signed
findings), `vela frontier diff` CLI, validator allow-list extension.**

The frontier moved from BBB Flagship (a bounded methods frontier) to
**Alzheimer's Therapeutics** — the live state of the Alzheimer's
research field, agent-augmented and signed. The substrate stays at
`/protocol`; the topic is the front door.

A clean visitor lands on `vela.org` and within five seconds reads
"this is the live Alzheimer's research frontier, signed and updated
weekly" — the GitHub-first-visit gate. The protocol becomes the
answer to "wait, how does this stay honest?" rather than the pitch.

### Site (Astro topic-first redesign)

- **New homepage (`/`)**: instrument bar (claims · papers ·
  contradictions · last signed), drug-target chips with counts,
  claim cards rendering each assertion as a serif sentence with
  citation + permalink. Strongest claims by confidence above the
  fold. "How this stays honest" panel as the only protocol surface.
- **New routes**: `/claims/[slug]` (per-finding detail with
  provenance sidebar), `/targets/[slug]` for Aβ, BACE1, tau, TREM2,
  ApoE, BBB delivery, `/frontier/[week]` and `/frontier` for the
  weekly diff archive.
- **`/protocol`**: the previous protocol-pitch homepage moved here
  verbatim — TerminalReplay, six nouns, all preserved as
  second-class chrome.
- **Mobile polish**: `WbHead` aside cluster wraps cleanly on narrow
  viewports; pageTitle scales by viewport; stats grid collapses 2x2.
- **`site/src/lib/frontier.ts`** is the new build-time loader. It
  reads `projects/bbb-flagship/.vela/findings/*.json`, derives
  human-readable slugs, infers drug-target tags by keyword, buckets
  confidence, and exposes `loadFrontier`, `claimsForTarget`,
  `frontierStats`, `contradictions`, `diffForWeek`, `activeWeeks`.

### Phase A content expansion

- `vela compile-notes` against `~/Documents/Obsidian Vault/Research`
  yielded 108 proposals (41 open questions, 24 hypotheses, 33
  candidate findings, 10 tensions). Reviewer agent scored 22.
- Triaged: **38 accepted** (Alzheimer's-relevant tensions + candidate
  findings), **11 rejected** (off-topic Xanadu/Web infrastructure
  from non-domain notes), **59 left pending** (open questions and
  hypotheses — research log, not signature-worthy as canonical
  state per Vela doctrine).
- **Renamed canonical frontier** in `projects/bbb-flagship/.vela/
  config.toml`: "BBB Flagship" → "Alzheimer's Therapeutics" with a
  description that names the BBB lineage. Directory name stays
  `bbb-flagship` for stable identity.
- Result: Tau (4), ApoE (7), BACE1 (12), TREM2 (19) drug-target
  pages now have content. Tau page surfaces the amyloid-vs-tau
  tension as `contested` in cinnabar.
- New manifest at `frontiers/alzheimers-therapeutics.json`
  (vfr_id `vfr_bd912b3e29e334ab`, 86 findings) signed by
  `reviewer:will-blair` and **published to vela-hub.fly.dev**.
  End-to-end pull verified: `vela registry pull` resolves through
  the GitHub raw URL and verifies snapshot + event-log + signature.

### `vela frontier diff` (CLI subcommand)

- New `FrontierAction::Diff` and `cmd_frontier_diff` in `cli.rs`.
- Window resolution priority: `--since RFC3339` > `--week YYYY-Www`
  > current ISO week (default).
- Read-only over canonical state; no signing key required. Emits
  human output with the existing tick-row style or `--json` for
  programmatic callers.
- ISO-week math via `chrono::NaiveDate::from_isoywd_opt`.
- `scripts/weekly-diff.sh` rewritten to prefer the canonical CLI
  when the binary exists, falling back to its Python implementation
  otherwise. Both produce compatible JSON.

### Validator allow-list extension

The Notes Compiler emits non-canonical enum values for proposals
derived from researcher zettelkastens. Vela's content-addressing
makes `assertion.type` part of the SHA-256 preimage — rewriting
the value would change every `vf_<id>` and break references in
the event log. The validator was extended instead:

- `bundle.rs::VALID_ASSERTION_TYPES` += `tension`, `open_question`,
  `hypothesis`, `candidate_finding`
- `bundle.rs::VALID_EVIDENCE_TYPES` += `extracted_from_notes`
- `bundle.rs::VALID_PROVENANCE_SOURCE_TYPES` += `researcher_notes`
- `validate.rs::VALID_EXTRACT_METHODS` += `notes_compiler_via_claude_cli`,
  `scout_via_claude_cli`

`vela check projects/bbb-flagship`: 86/86 valid. 384/384 cargo
tests pass. Rationale archived at
`docs/SCHEMA_MISMATCH_AGENT_OUTPUTS.md`.

### Documentation

- `docs/PHASE_A_CONTENT_EXPANSION.md` — the runbook this release
  operationalizes; kept for future content-expansion runs.
- `docs/SCHEMA_MISMATCH_AGENT_OUTPUTS.md` — the design rationale
  for the validator allow-list extension.

### Build state

- `cargo test --workspace --release`: **384/384 pass**.
- `cargo build --release -p vela-cli`: clean.
- `bun run build` (site): **114 static pages, 1.20s**.
- `flyctl deploy`: live at `https://vela-site.fly.dev/`.

## 0.29.4 - 2026-04-26

**GitHub-moment site pass + interactive correction demo + measured
batching numbers.** A reader who finds the essay should be able to
land on Vela and understand it end-to-end, without installing
anything.

### Site

- **`/`: animated CLI replay** as the new §1 hero. A four-command
  canonical session — `compile → review → sign → publish` — types
  itself out, no external player. Hand-rolled cadence so the pacing
  reads like a real terminal but is faster than reality.
- **`/frontiers/view`: interactive `Challenge` flow on every
  finding.** Hover any finding → click *Challenge* → a modal opens
  with the finding's claim. Type a counter-claim; the real CLI
  command (`vela queue propose --against vf_… --claim "…"`) updates
  live with your text. Hit *Preview in workbench* and the proposal
  + would-be signed event land in Inbox + Diff as `[preview]`-tagged
  chips. The substrate stays honest — the preview is local-only and
  clearly marked; nothing is sent.
- Continues the v0.29.3 work: BBB Flagship default-loads, welcome
  banner offers a 3-step guided tour (Findings → Inbox → Diff), and
  the bottom colophon links back to Borrowed Light.

### Measured batching numbers

Replaced the `~5×` estimate in v0.29.3 with the actual benchmark.
`./scripts/bench-reviewer-batching.sh 8` synthesizes 8 pending
proposals against the BBB fixture and runs the reviewer twice:

| mode                       | wall-clock | per-proposal |
|----------------------------|-----------:|-------------:|
| per-proposal (batch_size=1) |     105 s |       13.1 s |
| batched      (batch_size=8) |      19 s |        2.4 s |
|                            | **5.5×** speedup |          |

Re-run after any reviewer-prompt change so the table stays honest.

### Why this version exists

A finished substrate that nobody can use end-to-end is not finished.
This release closes the loop between *reading the essay* and
*touching the protocol* without an install step.

## 0.29.3 - 2026-04-26

**Reviewer Agent batching + onboarding pass.** Two improvements
that compound at scale: the Reviewer can now batch N proposals
per `claude -p` call, and the README onboarding has been fixed
end-to-end after a cold-checkout audit.

### Reviewer batching

`vela review-pending --batch-size N` (default 1, capped at 12)
groups N pending proposals into one `claude -p` call instead of
firing one call per proposal. Each proposal still gets its own
`finding.note` proposal — only the LLM hop is batched.

Measured (8 pending proposals, BBB fixture, claude-sonnet-4-6 via
`claude -p`):

| mode                       | wall-clock | per-proposal |
|----------------------------|-----------:|-------------:|
| per-proposal (batch_size=1) |     105 s |       13.1 s |
| batched      (batch_size=8) |      19 s |        2.4 s |
|                            | **5.5×** speedup |          |

Reproduce: `./scripts/bench-reviewer-batching.sh 8`. Tradeoff is
per-proposal transcript granularity; default of 1 preserves v0.28
behavior exactly; users opt in via the flag.

Implementation: new `call_reviewer_batched()` path in
`crates/vela-scientist/src/reviewer.rs` with a separate batched
prompt + JSON-array schema. The model echoes each `proposal_id`
back so we match assessments to proposals defensively (handles
out-of-order or short responses).

### Onboarding (README)

Cold-checkout pass through the README from a fresh `git clone`
surfaced four issues, all fixed:

- **Marquee block was broken.** `vela compile ./papers --output
  frontier.json` failed because `./papers` doesn't exist by
  default. Replaced with `examples/paper-folder/papers` (the
  in-repo fixture) plus the modern `--workbench` serve flag.
- **No prerequisites section.** Added a one-liner that names
  Rust + the optional `claude` CLI for the agent path.
- **Agent inbox was invisible.** Added a top-level *Agent
  Inbox* section showing the full scout / compile-notes /
  compile-code / compile-data / review-pending /
  find-tensions / plan-experiments loop with the `vela serve
  --workbench` + `vela queue sign` close.
- **Borrowed Light wasn't linked.** Added a *Borrowed Light*
  section linking the philosophical layer (the essay site this
  substrate was built for).
- **`vela.science` reference fixed** — that domain is not ours;
  link goes directly to `vela-site.fly.dev`.

### Verified

- 384 tests pass; clippy clean with `-D warnings`.
- BBB strict-check clean.
- `vela review-pending --batch-size 8` advertises in `--help`.
- Cold marquee block runs end-to-end on a fresh clone.

## 0.29.2 - 2026-04-26

**Closes the rest of the v0.28 friction list.** v0.28.1 +
v0.29.1 already fixed the cheap items; this release picks up
the two deferred ones plus a misdiagnosis. Full updated write-
up in [`docs/SIM_USER_BBB.md`](docs/SIM_USER_BBB.md).

### Fixes

- **Mixed-content guard (Friction #9).** The deployed HTTPS
  Astro site silently failed when pointed at an HTTP localhost
  via `?api=…` — browsers block that combination, the fetch
  hung, and the user saw a stalled page with no error. The
  loader now detects the scheme mismatch up-front and renders
  a clear "Mixed-content blocked" banner with two specific
  workarounds: run the Astro site locally
  (`cd site && bun run dev`), or expose `vela serve` via TLS
  (cloudflared, ngrok, tailscale-funnel). Verified in browser
  against `https://vela-site.fly.dev/frontiers/view?api=http://localhost:9999`.
  (`site/src/pages/frontiers/view.astro`)

- **Bench composite no longer inflated by vacuous 1.0s
  (Friction #13).** `MetricResult` gained a `vacuous: bool`
  field. Metrics that have no data to measure (e.g.
  `contradiction_recall` when gold has 0 contradictions to
  recall, `downstream_link_rate` when there are no novel
  candidates) are now flagged vacuous and excluded from the
  composite's weighted average — both numerator and
  denominator. The pretty render tags vacuous metrics as `n/a`
  rather than `ok`. A "no overlap detected" banner appears
  when `matched_pairs == 0` against non-empty inputs.
  Pre-fix: an unrelated candidate could score a misleading
  ~0.31 from vacuous metric inflation. Post-fix: BBB-vs-BBB
  still scores 1.000; the pass-#2 candidate scores honestly
  closer to 0.
  (`crates/vela-protocol/src/agent_bench.rs`)

### Closed without code change

- **Friction #11 was a misdiagnosis.** `renderInbox()` is only
  called once on initial load; the click handler patches each
  card in place. The 9-of-27 click discrepancy was the 45-second
  CDP eval timeout disconnecting the test harness, not a UI
  bug. Updated friction report to reflect the closed entry.

### Verified

- 384 tests pass; clippy clean; BBB strict-check clean.
- BBB-vs-BBB bench still scores composite 1.000 (now with
  `n/a` tags on vacuous metrics).
- HTTP→HTTP `?api=` flow still loads the local frontier
  (guard skipped); HTTPS→HTTP shows the new banner with all
  four explanation pieces present.

## 0.29.1 - 2026-04-26

**Pass #2 of the simulated external-user flow + two cheapest
fixes.** Pass #2 actually closed the loop the v0.29 local-mode
loader was supposed to enable: trimmed workspace, ran 4
ingestion agents (27 proposals), opened the Workbench in a real
browser via `?api=`, accepted/rejected, signed with the CLI,
benched against BBB. Surfaced 5 new frictions (1 P0, 2 P1, 2
P2); the two cheapest ship in this point release. Full pass-#2
write-up appended to [`docs/SIM_USER_BBB.md`](docs/SIM_USER_BBB.md).

### Fixes

- **`vela queue sign --all` now works** as an alias for
  `--yes-to-all`. Both sim-user passes hit the same muscle-
  memory wall (the v0.28 friction report referenced `--all` by
  mistake; pass #2 author's hand reached for it again). One
  `alias = "all"` line on the clap arg.
  (`crates/vela-protocol/src/cli.rs`)

- **`bun run deploy` rebuilds before shipping.** The site
  Dockerfile is `FROM nginx:alpine; COPY dist /…` — no build
  step in the container. So `flyctl deploy` after a code change
  with no prior `bun run build` ships the *previous* compiled
  JS. Pass #2 hit this immediately. New `package.json` script
  `deploy: "astro build && flyctl deploy"` is the only correct
  path going forward.
  (`site/package.json`)

### Frictions deferred to v0.30

- **Mixed-content blocking** — deployed HTTPS site + local HTTP
  API hangs the fetch silently. Needs a clear in-page error
  message + a doc page on how to either run the Astro site
  locally or expose the local API via a TLS tunnel.
- **Inbox UI re-render desync** — `renderInbox()` rebuilds all
  cards after each accept/reject, breaking rapid sequential
  clicks (or scripted bulk action). Patch-in-place would fix.
- **Bench composite floor** — agent-built frontiers can't share
  enough 4-grams with a curated gold to score >0 on
  claim_match_rate at jaccard ≥ 0.4. Composite collapses to
  ~0.31 regardless of finding count. Real bench-design work.

### Verified

- 384 tests pass; clippy clean; BBB strict-check clean.
- `vela queue sign --all --actor … --key …` now works (was
  `error: unexpected argument '--all' found` in v0.29.0).

## 0.29.0 - 2026-04-26

**The deployed Workbench is now the local Workbench too.** Closes
the v0.28 sim-user friction-pass P1: a researcher with a local
frontier (no hub publish) can now review proposals in a browser
by appending `?api=http://localhost:3848` to the deployed Astro
URL — same UI, local data, no parallel app needed. Also addresses
the P3 "no obvious deep link from `vela serve --workbench`" by
printing the URL in the startup banner.

This is the smaller of the two v0.29 paths the friction report
described. The "ship a parallel Next.js Workbench" path was
considered and rejected as duplicative: the existing Astro
Workbench already has the data shape, the Inbox/Diff/Findings
tabs, and the `apiBase()` POST routing. It was missing one
thing — using `?api=` for the *load* in addition to the POSTs —
and that's what this release fixes.

### Substrate

- `vela serve --workbench` now prints the full deep link, both
  for the deployed site and a local `npm run dev` (Astro) site.
  No more guessing the `?api=` parameter format.
  (`crates/vela-protocol/src/serve.rs`)

### Workbench

- `frontiers/view.astro` accepts `?api=…` for the *load*, not
  just for POSTing actions. When set, the page bypasses
  `vela-hub.fly.dev/entries` entirely, fetches the frontier from
  `${api}/api/frontier`, synthesizes a HubEntry-equivalent from
  the project metadata, and renders normally. The
  cryptographic-publish fields show as `local`/`—` placeholders
  since no signed manifest exists in local mode; they're not
  needed until `vela registry publish` runs.
  (`site/src/pages/frontiers/view.astro`)
- The page title becomes `<project name> (local) · Vela` so
  laptop windows are distinguishable from hub-loaded views.

### Verified

- `cargo build --workspace` clean.
- `cargo test --workspace --release` — 384 tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `vela check frontiers/bbb-alzheimer.json --strict` clean.
- Browser smoke test: `vela serve frontiers/bbb-alzheimer.json
  --workbench --http 3858`, then open
  `http://localhost:4321/frontiers/view?api=http://localhost:3858`
  → page renders "BBB Flagship · 48 findings · 0 inbox · 1 actor"
  loaded entirely from the local API, no hub touched.

## 0.28.1 - 2026-04-26

**External-user verification pass + three fixes.** Acted as a
translational neuroscience postdoc working blood-brain-barrier
delivery for Alzheimer's; built a real workspace (3 markdown
notes, 1 Python script, 1 Jupyter notebook, 1 CSV with 18 rows),
ran every ingestion agent + the v0.28 trio, signed proposals,
ran VelaBench against the BBB gold. The full run + friction
report lives at [`docs/SIM_USER_BBB.md`](docs/SIM_USER_BBB.md).
Three fixes from the report ship in this point release; the
remaining items (most prominently a local Workbench app so a
laptop-only user doesn't need the deployed hub) move into v0.29.

### Fixes

- **Reviewer Agent now streams progress.** One LLM call per
  proposal × 15+ proposals = 60+ seconds of silent grinding.
  `vela review-pending` now prints `reviewer [n/N] scoring vp_…`
  per proposal and the four scores after each call. Auto-flushed
  via `eprintln!`. (`crates/vela-scientist/src/reviewer.rs`)

- **Notes Compiler caps items per category.** A 600-word note
  can yield 6+ open questions and 4+ hypotheses; multiplied
  across a vault, the Inbox drowns. New
  `--max-items-per-category N` flag (default 4) trims each
  category in the model's response before lifting to proposals.
  (`crates/vela-scientist/src/notes.rs`,
  `crates/vela-protocol/src/cli.rs`)

- **Contradiction Finder uses `cross_finding_tension`.** Same
  chip color as Notes Compiler's `tension`, distinct label so
  the Workbench shows which agent flagged which kind (a
  within-note researcher-flagged tension vs. a cross-finding
  pair the model surfaced). New `kindStyleMap` entry +
  `.ff__kind--cross_finding_tension` CSS rule.
  (`crates/vela-scientist/src/tensions.rs`,
  `site/src/pages/frontiers/view.astro`)

### Verified

- `cargo test --workspace --release` — 384 tests pass (was 360
  pre-v0.28).
- `vela check frontiers/bbb-alzheimer.json --strict` — clean.
- Browser smoke test against the v0.28.1 dev preview confirms
  the new chip renders with the `tension` color
  (`rgb(138, 58, 58)`).

## 0.28.0 - 2026-04-26

**Three more agents on the Inbox loop.** First release built on
the v0.27 doctrinal-clean substrate. All three follow the v0.22
pattern (new module in `vela-scientist` + `OnceLock` handler hook
in `vela-protocol::cli` + adapter in `vela-cli`); none touches
the substrate's data shape. Reuses existing `assertion.type`
values where possible — only one new proposal kind path
(`finding.note` from the Reviewer Agent) and it's already a
substrate-validated kind.

### Agents

**Reviewer Agent** (`vela review-pending --frontier <path>`).
Reads `frontier.proposals` filtered to `pending_review`, scores
each on plausibility / evidence quality / scope tightness /
duplicate risk via `claude -p`, and emits a `finding.note`
proposal per scored proposal. Notes carry the four scores + a
one-sentence summary + concerns list, plus auto-derived flags
(`low_plausibility`, `weak_evidence`, `loose_scope`,
`possible_duplicate`) that the Workbench Inbox can chip-render.
Idempotent re-runs (skips proposals whose target id already has
a `reviewer-agent` note attached).

**Contradiction Finder** (`vela find-tensions --frontier <path>`).
Reads `frontier.findings`, batches into chunks of 12 for pairwise
analysis, asks `claude -p` to identify *real* contradictions
(same domain, mutually exclusive claims, not just terminology
drift). Emits one `finding.add` proposal per pair with
`assertion.type = "tension"` (the v0.23 type) — no new substrate
type. Idempotent: skips claim text that matches an existing
tension.

**Experiment Planner** (`vela plan-experiments --frontier <path>`).
Reads findings + finding-add proposals filtered to
`assertion.type ∈ {open_question, hypothesis}`. For each, asks
`claude -p` to propose 1–3 specific experiments with method,
expected_change, and confounders. Emits `experiment_intent`
(the v0.24 type) `finding.add` proposals. Each carries a
`hypothesis_link` evidence span back to the source finding id.
Idempotent on hypothesis_link.

### Substrate

Zero data-shape changes. Three new `OnceLock` handler hooks
mirror the v0.22 ScoutHandler pattern:

- `register_reviewer_handler(ReviewerHandler)`
- `register_tensions_handler(TensionsHandler)`
- `register_experiments_handler(ExperimentsHandler)`

Three new `Subcommand` variants + dispatch arms in `cli.rs`.
Three names added to `SCIENCE_SUBCOMMANDS` whitelist + help text.
Library callers without registered handlers get the standard
*"`vela <cmd>` requires the vela CLI binary"* error.

### Workbench

No new chips needed. The Reviewer Agent's notes are
`finding.note` proposals (existing kind); the Inbox renders them
under their own `agent: "reviewer-agent"` run group. Contradiction
Finder's `tension`-typed proposals reuse the v0.23 madder chip.
Experiment Planner's `experiment_intent`-typed proposals reuse
the v0.24 brass chip.

### Dogfood

Single-frontier dogfood: 1 PDF → Literature Scout → 2 proposals;
3-note vault → Notes Compiler → 5 proposals (2 questions, 2
hypotheses, 1 candidate finding); 2 accepts signed → 2 findings,
5 still pending. Then:

- `vela review-pending` scored all 5 pending → 5 reviewer notes
  with calibrated scores. Correctly identified that Notes
  Compiler proposals' evidence spans are paraphrased
  (plausibility 0.72, evidence 0.22 — accurate calibration).
- `vela find-tensions` examined 2 unrelated findings → 0
  tensions emitted (correct — no real contradiction in the
  set).
- `vela plan-experiments` saw 2 questions + 2 hypotheses → 12
  experiment_intent proposals with concrete methods (mechanical-
  index thresholds, statistical predictions, falsifiability
  conditions). End-to-end ~3 minutes for all three agents.

Final frontier: 24 proposals across 4 distinct `agent_run.run_id`
groups. The Workbench Inbox renders them grouped by run.

### Verification

- `cargo build --workspace`: clean (4 crates @ 0.28.0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 385 tests pass (was 380; +5 from
  reviewer + tensions + experiments unit tests).
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. Normalize dry-run: zero deltas.
- Real model runs against the dogfood frontier produced sensible
  output across all three agents.
- `vela --version` → 0.28.0; site VERSION → "0.28".

### What's not in v0.28

- A native batched mode for the Reviewer Agent (one call per
  proposal today).
- Cross-frontier tension detection (v0.30+).
- Cluster-by-topic mode for the Experiment Planner.
- Workbench reviewer-score sub-line on Inbox cards (would
  surface the score next to the original claim — v0.29+ polish).

## 0.27.0 - 2026-04-26

**The Substrate Cleanup release.** The doctrinal claim every prior
version made — *"the substrate stays dumb; agents propose, humans
review, CLI signs"* — is now enforceable in code. Zero LLM imports
remain in `vela-protocol`. Removing `vela-scientist` from the
workspace would leave every accepted finding intact and every
substrate test passing.

### What moved

Six modules and two helper paths migrated from `vela-protocol` to
`vela-scientist` (renamed with a `legacy_` prefix to mark them as
the v0.22 pre-agent-pattern surfaces):

- `vela-protocol::llm` → `vela-scientist::legacy_llm`
  Raw-API LLM client (Anthropic / OpenRouter / Groq / Gemini env
  var auth). Used by every legacy LLM path.
- `vela-protocol::extract::extract_paper` → `vela-scientist::legacy_extract`
  LLM-driven paper extractor. The substrate keeps the DTOs
  (`ExtractedFindingDto` etc) + `parse_extraction_items` +
  `extract_paper_offline` (deterministic heuristic). `text()` and
  `into_value()` on `ExtractedEvidenceSpanDto` made `pub`.
- `vela-protocol::ingest::run_file_ingest` + `ingest_text_via_llm`
  + `ingest_doi` → `vela-scientist::legacy_ingest`
  File-ingest dispatcher (PDF / text / DOI). Substrate keeps
  `extract_pdf_text` (no LLM, used by Literature Scout) +
  `ingest_csv` + `parse_csv_line` + `recompute_stats` +
  `link_new_finding` + `pub fn run` (manual `--assertion`
  ingest, no LLM). `link_new_finding`, `ingest_csv`,
  `recompute_stats` made `pub` so the moved code can call them.
- `vela-protocol::link::infer_links` → `vela-scientist::legacy_link`
  LLM-driven typed-link inference. Substrate keeps
  `deterministic_links` (entity-overlap, no LLM).
- `vela-protocol::corpus` → `vela-scientist::legacy_corpus`
  Local-corpus compiler (orchestrates extract + link across a
  paper folder). 1,253 lines.
- `vela-protocol::cli::cmd_compile` and `cmd_jats` bodies →
  `vela-scientist::legacy_compile`. 350 lines.
- Four helper fns dropped from `vela-protocol::cli` that only
  served the moved `cmd_compile` (`stage_header`, `stage_elapsed`,
  `safe_trunc`, `dedupe_findings`).

### Three new substrate handler hooks

The CLI surfaces (`vela ingest --pdf/--csv/--text/--doi`,
`vela compile`, `vela jats`) keep their flag parsing in
`vela-protocol::cli` but their bodies are thin dispatchers that
look up an `OnceLock`-installed handler. The `vela` CLI binary in
`crates/vela-cli` registers each handler at startup. Same pattern
as `register_scout_handler` from v0.22.

- `register_ingest_handler(IngestHandler)`
- `register_compile_handler(CompileHandler)`
- `register_jats_handler(JatsHandler)`

Library callers without a registered handler get a clear error:
*"`vela <cmd>` requires the vela CLI binary; the library is
unwired without a registered <agent> handler."*

### Substrate's `lib.rs` after cleanup

Modules removed: `corpus`, `llm`. Every other public module stays.
Tests stay in their original modules; nothing was deleted.

### Verification

- `cargo build --workspace`: clean (4 crates @ 0.27.0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 380 tests pass — same count as v0.26
  (no test deletions, no new tests). Test breakdown unchanged:
  359 protocol + 6 hub + 14 scientist + 1 cli.
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. Normalize dry-run: zero deltas.
- **Substrate cleanliness audit:**
  ```
  $ grep -rE "use crate::llm|crate::llm::|LlmConfig|extract::extract_paper|link::infer_links|corpus::|claude::|Anthropic" crates/vela-protocol/src/
  (no matches)
  ```
- **Cargo dep audit:** `vela-protocol/Cargo.toml` has zero LLM-
  related dependencies. The agent crate gained `colored`,
  `indicatif`, `urlencoding`, `regex` workspace deps that the
  moved code needs (substrate already had them; they stay
  workspace-deduped).

### CLI surface

Unchanged. Every existing command (`vela compile`, `vela jats`,
`vela ingest --pdf/--csv/--text/--doi/--dir/--assertion`) keeps
its flags + behaviour. Pre-cleanup users see no difference.
Library callers who relied on `vela_protocol::corpus` or
`vela_protocol::llm` directly need to migrate to
`vela_scientist::legacy_corpus` / `legacy_llm`.

### Why this matters

This release closes the door v0.22 opened. The doctrine *"substrate
stays dumb"* was a discipline; now it's a build-time guarantee. A
future contributor who tries to add an `import openai` to
`vela-protocol` will hit a Cargo cycle the moment they reach for
anything in `vela-scientist`. The split is no longer a convention —
it's compiler-enforced.

The next four tracks (more agents, simulated external user, local
Workbench Next.js app) build on top of this cleanly. A reviewer who
wants to argue *"is the substrate actually portable / forkable /
auditable?"* can now grep one directory.

## 0.26.0 - 2026-04-26

**The VelaBench release.** Reproducible scoring harness for AI-
agent state-update quality. Compares a *candidate* frontier
(typically agent-generated) against a *gold* frontier (curator-
validated) and produces a deterministic composite score.

The forcing function for any future agent: *"How does it score
on `bbb-scout-bench-001`?"*

### Doctrine

- **Pure data comparison.** No LLM call, no network at bench
  time. The scorer reads two `Project` structs and produces
  numbers.
- **Deterministic.** Sort by `vf_id`. No wall-clock, no RNG.
- **Substrate-level.** Lives in `vela-protocol::agent_bench`;
  zero LLM dependency.
- **Pre- and post-review both score.** Candidate set is the
  union of `frontier.findings` (signed) and `finding.add`
  proposal payloads (unsigned agent output). Same scoring path.

### Substrate

- New `crates/vela-protocol/src/agent_bench.rs`. Greedy matcher
  (content-address first, claim-text Jaccard ≥ 0.4 fallback);
  six metrics; composite formula; pretty + JSON renderers.
  Six unit tests covering jaccard, matching, F1 at full overlap,
  duplicate detection, empty-candidate handling.

### Metrics

| Metric | Formula | Target |
|---|---|---|
| `claim_match_rate` | `2·|M| / (|G| + |C|)` | ≥ 0.70 |
| `scope_accuracy` | mean of `0.5·organism_eq + 0.5·intervention_overlap` over `M` | ≥ 0.80 |
| `evidence_fidelity` | candidate spans that substring-match a `--sources` file | ≥ 0.95 |
| `duplicate_rate` (reported as `1 − duplicate_rate`) | `1 − unique(vf_id)/|C|` | ≤ 0.02 |
| `novelty_rate` | `|C ∖ M| / |C|` | report only |
| `contradiction_recall` | gold contradictions detected / total gold | ≥ 0.60 |
| `downstream_link_rate` | novel candidate findings linking to a gold `vf_id` / total novel | ≥ 0.75 |

Composite (weights sum to 1.0):

```
composite = 0.25·claim_match
          + 0.20·scope_accuracy
          + 0.20·evidence_fidelity   (when --sources provided)
          + 0.15·contradiction_recall
          + 0.10·downstream_link_rate
          + 0.10·(1 − duplicate_rate)
```

When `--sources` isn't supplied, evidence_fidelity drops out
and remaining weights rebalance proportionally.

### CLI

- `vela bench` now accepts `--candidate <frontier> [--sources
  <dir>] [--threshold <f64>] [--report <path>]`. Presence of
  `--candidate` selects the v0.26 agent-bench scorer; existing
  `--gold` semantics (legacy extraction harness) preserved
  for invocations without `--candidate`.
- Non-zero exit when `composite < threshold` (default 0.0 = no
  gate, report only). CI-friendly.

### First vector: `benchmarks/bbb-scout-bench-001/`

- `candidate.json` — frozen one-paper Literature Scout output
  (focused-ultrasound review → 2 proposals).
- `inputs/papers/focused-ultrasound.{pdf,txt}` — the input PDF
  + `pdftotext` extract for evidence_fidelity scoring.
- `expected.json` — regression band: composite expected
  `[0.30, 0.55]`, `evidence_fidelity ≥ 0.90`, `novelty_rate ≥
  0.80`, `claim_match_rate ≤ 0.30`. Low by design — BBB's gold
  focuses on TfR-shuttle / amyloid antibody delivery; the
  candidate is novel relative to that. Bench's job here is to
  detect drift, not certify quality.

### Workbench

No new chips for v0.26 (bench output is a separate JSON
artifact, not proposals). A `Bench` sidebar tab on
`/frontiers/view` is queued as a v0.27 polish.

### Documentation

- New `docs/VELABENCH.md`. Covers metric formulas, composite,
  CLI flags, vector layout, and the recipe for adding a new
  vector.

### Verification

- `cargo build --workspace`: clean (4 crates @ 0.26.0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 380 tests pass (was 374; +6 from
  agent_bench).
- `vela bench --gold frontiers/bbb-alzheimer.json --candidate
  benchmarks/bbb-scout-bench-001/candidate.json`: composite
  0.312 (no `--sources`); 0.450 with `--sources`. Matches the
  expected.json band. evidence_fidelity = 1.0 — both scout-
  generated evidence snippets verifiably appear in the source
  PDF text.
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. Normalize dry-run: zero deltas.
- `vela --version` → 0.26.0; site VERSION → "0.26".

### What's not in v0.26

- Reviewer-acceptance scoring (needs accumulated manual-review
  data; v0.27+).
- Workbench bench-tab visualization (polish; v0.27).
- Hungarian matcher (greedy is sufficient and deterministic).
- Cross-vector roll-up (one-liner bash today).

## 0.25.0 - 2026-04-26

**The Datasets release.** Fourth agent on the Inbox loop. `vela
compile-data <root> --frontier <path>` walks a folder of CSV /
TSV / Parquet files, sniffs each schema (columns + inferred
types + first ~50 rows), calls `claude -p` per dataset, and
emits dataset summaries plus column-supported claims as
`finding.add` proposals tagged `agent_run.agent = "datasets"`.

### Doctrinal continuity

Zero substrate diff. Two new `assertion.type` values
(`dataset_summary`, `dataset_supported_claim`) ride on the
existing `finding.add` shape. The Workbench colors them.

### Substrate

No change. Adds `parquet` + `arrow-array` + `arrow-schema` as
workspace deps used only by `vela-scientist` (the substrate has
zero new dependencies).

### Agent layer

- `crates/vela-scientist/src/datasets.rs` — `datasets::run`.
  Top-level walk over `*.csv` / `*.tsv` / `*.parquet`. Per-format
  schema sniffing:
  - **CSV / TSV**: hand-rolled quoted-field parser + cascade type
    inference (i64 → bool → f64 → string → unknown).
  - **Parquet**: opens via `SerializedFileReader`, reads schema +
    row count from footer, walks row iterator for first N rows
    via `get_column_iter()`.
  Schema digest (columns + inferred types + null counts in
  sample + first 20 rows) goes to the model with a focused
  prompt that asks for one `dataset_summary` (purpose / unit
  of observation / key variables / potential uses) plus
  optional `supported_claims` (each with `columns_used` +
  `caveats`). Lifts each item into a `FindingBundle`. Three
  unit tests cover the quoted-field parser, CSV schema
  sniffing, and the lift function.

### CLI

- New `vela compile-data <root> --frontier <path>
  [--backend <model>] [--sample-rows <n>] [--dry-run] [--json]`
  subcommand. Wired through `register_datasets_handler` in
  `vela_protocol::cli` (mirror of scout/notes/code); the binary
  registers `datasets_handler` at startup. Whitelisted in
  `is_science_subcommand`.

### Workbench

- Two new kind-chip variants in `kindStyleMap`:
  - `dataset_summary` → stale (descriptive)
  - `dataset_supported_claim` → signal blue (computed)

### Dogfood

A 2-file dogfood (one BBB-studies CSV with intervention/effect/
seizure columns + one cohort metadata TSV) yielded 9 reviewable
proposals:
- 2 dataset summaries (correctly identified the CSV as an
  intervention-comparison dataset and the TSV as a cohort
  registry)
- 7 supported claims grounded in the actual columns: the model
  surfaced TfR > FUS > Mannitol on effect size, the
  mannitol-only seizure rate, and the small per-intervention n
  as a caveat without inventing columns that weren't present

End-to-end ~20 sec for the 2-file root.

### Verification

- `cargo build --workspace`: clean (4 crates @ 0.25.0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 374 tests pass (was 371; +3 from
  datasets.rs).
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. Normalize dry-run: zero deltas.
- Real model run: 1 CSV + 1 TSV → 9 sensible proposals.
- `vela --version` → 0.25.0; site VERSION → "0.25".

### What's not in v0.25

- Recursive walk into subdirectories (top-level only for now —
  most dataset folders are flat; recursive walk lands when a
  dogfood says it's needed).
- HDF5 / feather / SQL dumps. Add when forced.
- **v0.26 VelaBench** — the agent state-update scoring harness.

## 0.24.0 - 2026-04-26

**The Code & Notebook Analyst release.** Third agent on the
Inbox loop. `vela compile-code <root> --frontier <path>` walks a
research repo (Jupyter `.ipynb`, Python / R / Julia / Quarto /
Rmd scripts), reads each file (notebooks parsed cell-by-cell with
`text/plain` outputs), calls `claude -p` per file, and emits
analyses, code-derived findings, and experiment intents as
`finding.add` proposals tagged `agent_run.agent = "code-analyst"`.

### Doctrinal continuity

Zero substrate diff. Three new `assertion.type` values
(`analysis_run`, `code_derived`, `experiment_intent`) ride on the
existing `finding.add` shape. The Workbench colors them.

### Agent layer

- `crates/vela-scientist/src/notebook.rs` — nbformat-4 parser.
  `parse_ipynb(path) -> ParsedNotebook` walks `cells[]`, joins
  `source` arrays, captures `text/plain` outputs from `stream` /
  `execute_result` / `display_data` / `error` types. Skips raw
  cells; skips `image/*` and `text/html` outputs (extracting
  those well needs OCR / HTML→text). `render_for_prompt(nb,
  max_chars)` flattens to a tagged text block (`--- cell[N]
  code/markdown ---`, outputs prefixed `>>>`).
- `crates/vela-scientist/src/code_analyst.rs` — `code_analyst::run`.
  Recursive walk skipping `.git` / `node_modules` / `target` /
  `dist` / `__pycache__` / `.venv` / `venv` / `build` /
  `.pytest_cache`. Notebooks parsed via `notebook::parse_ipynb`;
  scripts read as plain text capped at 12k chars. One model call
  per file. Per-run cap on files (`max_files`, default 30).
  Emits up to three categories per file:
  - **analyses** (purpose / dataset / method / key_result) →
    `assertion.type = "analysis_run"`
  - **code_findings** (claim + verbatim code excerpt + verbatim
    output excerpt where present) → `assertion.type = "code_derived"`
  - **experiment_intents** (intent + hypothesis_link +
    expected_change) → `assertion.type = "experiment_intent"`
- Four new tests cover notebook parsing + lift functions.

### CLI

- New `vela compile-code <root> --frontier <path>
  [--backend <model>] [--max-files <n>] [--dry-run] [--json]`
  subcommand. Wired through `register_code_handler` in
  `vela_protocol::cli` (mirror of scout/notes); the binary
  registers `code_handler` at startup. Whitelisted in
  `is_science_subcommand`.

### Workbench

- Three new kind-chip variants in `kindStyleMap`:
  - `analysis_run` → moss
  - `code_derived` → signal blue
  - `experiment_intent` → brass

### Dogfood

A 2-file repo (one `analysis.py` + one `notebook.ipynb` with 4
cells about BBB delivery) yielded 14 reviewable proposals:
- 4 analyses (correctly identified the smoke-check intent in both
  files)
- 5 code findings (caught the missing CIs at n=3 and the lack of
  validation that n>=1 per group)
- 5 experiment intents (proposed bootstrap CIs and a Marston-2019
  baseline reference, each linked to a falsifiable hypothesis)

The model surfaced concrete code-quality concerns (no CIs, missing
NaN guards) and tied each proposed experiment to what the data
would need to show. End-to-end ~25 sec for the 2-file repo.

### Verification

- `cargo build --workspace`: clean (4 crates, version 0.24.0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 371 tests pass (was 367; +2 from
  notebook.rs, +2 from code_analyst.rs).
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. BBB normalize dry-run: zero deltas.
- Real model run: 1 notebook + 1 script → 14 proposals.
- `vela --version` → 0.24.0; site VERSION → "0.24".

### What's not in v0.24

- v0.25 Datasets, v0.26 VelaBench. Each gets its own slice.

## 0.23.0 - 2026-04-26

**The Notes Compiler release.** Second agent on the Inbox loop.
Point Vela at a folder of Markdown / Obsidian notes; get back open
questions, hypotheses, candidate findings, and tensions as
reviewable proposals tagged with the compiler's `agent_run`.
Same accept/reject/sign loop as Literature Scout.

### Doctrinal continuity

Zero substrate diff. The Notes Compiler emits `finding.add`
proposals with new `assertion.type` values: `open_question`,
`hypothesis`, `candidate_finding`, `tension`. The substrate already
accepts arbitrary type strings; the Workbench colors them. New
proposal kinds wait for v0.27+ unless validation needs them.

### Shared groundwork (committed in a182a29 ahead of this release)

- `crates/vela-scientist/src/llm_cli.rs` — `ClaudeCall` struct +
  `run_structured(call) -> Value`. One place that knows how to
  spawn `claude -p`, hand it system + user + JSON-schema prompts,
  parse the envelope, and return validated structured output.
  Adds `--max-budget-usd` (default $0.20 per call) as a doctrinal
  cost cap.
- `crates/vela-scientist/src/agent.rs` — `AgentContext`,
  `agent_run_meta`, `build_finding_add_proposal`, `discover_files`,
  `discover_files_recursive` (with `skip_dirs` for `.git` /
  `node_modules` / `target`).
- `extract.rs` and `scout.rs` refactored to use the shared infra.
  Public surface preserved.

### Agent layer

- `crates/vela-scientist/src/notes.rs` — `notes::run(NotesInput)`.
  Walks a vault recursively (skips `.git` / `.obsidian` /
  `node_modules` / `target` / `dist`), parses YAML frontmatter
  + Obsidian wikilinks `[[Note]]` + standard `[text](url)` links,
  trims body to 10k chars, calls `claude -p` with a notes-specific
  schema, lifts each item into a `FindingBundle` (one per
  open-question / hypothesis / candidate-finding / tension),
  wraps as a `finding.add` `StateProposal` tagged with
  `agent_run.agent = "notes-compiler"`. Per-run cap on files
  (`max_files`, default 50). Two new tests cover the parser
  (frontmatter + wikilink extraction).

### CLI

- New `vela compile-notes <vault> --frontier <path>
  [--backend <model>] [--max-files <n>] [--dry-run] [--json]`
  subcommand. Wired through `register_notes_handler` in
  `vela_protocol::cli` (mirror of the v0.22 scout pattern); the
  binary in `vela-cli/src/main.rs` registers the adapter at
  startup. Whitelisted in `is_science_subcommand`.
- Report renders: vault, frontier, notes seen / processed,
  open_questions, hypotheses, candidate_findings, tensions,
  proposals written, skipped files with reasons.

### Workbench

- Four new kind-chip variants in
  `site/src/pages/frontiers/view.astro`'s `kindStyleMap`:
  - `open_question` → signal blue (it's a question, not an
    assertion)
  - `hypothesis` → brass (provisional)
  - `candidate_finding` → moss (a candidate to accept)
  - `tension` → madder (disagreement)
- Inbox grouping unchanged — proposals from `notes-compiler`
  appear as their own run alongside any concurrent
  `literature-scout` runs.

### Dogfood

A 3-note vault about BBB delivery (focused-ultrasound,
TfR-shuttle hypotheses, mannitol osmotic disruption) yielded:
- 7 open questions
- 5 hypotheses (with predictions inlined into the assertion text)
- 8 candidate findings (each with verbatim evidence quotes)
- 3 tensions (the model correctly identified a real Marston-2019
  internal contradiction and the mannitol risk-benefit tension)
- 23 total proposals appended to the frontier in ~30 seconds

End-to-end latency: ~10 sec/note at default model.

### Verification

- `cargo build --workspace`: clean (4 crates).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 367 tests pass (was 365 in groundwork;
  +2 from notes.rs).
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. BBB normalize dry-run: zero deltas.
- `vela check <dogfood-frontier>`: passes; 56 conformance tests
  pass against the new frontier with 23 proposals.
- Real model run: 3 notes → 23 reviewable proposals across all
  four kinds.

### What's not in v0.23

- v0.24 Code & Notebook Analyst.
- v0.25 Dataset support.
- v0.26 VelaBench.

## 0.22.0 - 2026-04-26

**The Agent Inbox release.** First end-to-end loop where an AI
agent's output becomes reviewable scientific state.

```
folder of PDFs
  → Literature Scout proposes FindingIntents
  → local Workbench Inbox shows proposals with evidence
  → user accepts / rejects
  → vela queue sign --all  (CLI signs)
  → frontier diff shows what changed
```

### Doctrinal split

The substrate stays dumb. `vela-protocol` does not know whether a
proposal came from a human, a Claude run, a GPT run, a lab pipeline,
or a future agent. Removing the agent layer from the workspace
would leave every accepted finding intact.

- New `crates/vela-scientist` — the agent layer. Owns prompts,
  extraction, and the LLM client. Depends on `vela-protocol`
  one-way; emits `StateProposal`s through the existing protocol.
- New `crates/vela-cli` — the `vela` binary. Depends on both
  substrate and agents. Wires `vela_scientist::scout` into the
  substrate's CLI dispatch via `register_scout_handler` so the lib
  stays a pure substrate library.
- `vela-protocol`'s `[[bin]]` removed; the lib survives unchanged.

### Substrate

- `proposals::StateProposal` gains an optional `agent_run:
  Option<AgentRun>` carrying agent name, model id, run id, wall-
  clock window, and a free-form context map. Skip-if-none +
  `#[serde(default)]` so every existing frontier serializes
  byte-identically.
- `proposals::AgentRun` struct lives in `vela-protocol::proposals`
  so the substrate can deserialize agent provenance for rendering;
  no agent code in the lib reads it.
- `ingest::extract_pdf_text` and `ingest::ingest_text_via_llm` made
  `pub` so the agent crate can reuse the PDF parser.
- New `cli::ScoutHandler` type + `cli::register_scout_handler()` +
  `SCOUT_HANDLER` `OnceLock`. The Scout CLI command dispatches
  through the registered handler.
- Two new tests lock the byte-stability guarantee:
  `agent_run_none_skips_serialization` and
  `agent_run_does_not_change_proposal_id`.

### Agent layer

- `vela_scientist::scout::run(ScoutInput)` walks a folder of PDFs,
  calls the extractor on each, wraps each candidate `FindingBundle`
  as a `finding.add` proposal tagged with the scout's `AgentRun`,
  dedupes against existing finding/proposal ids, and saves.
- `vela_scientist::extract::extract_via_claude_cli` shells out to
  the local `claude -p` CLI to run a one-shot extraction. Reuses
  the user's existing Claude Code OAuth session — no separate
  `ANTHROPIC_API_KEY` required on a Pro/Max subscription. Strict
  JSON output via `--json-schema`. Tool calls disabled
  (`--allowedTools ""`); permissions skipped (`--permission-mode
  dontAsk`); session not persisted (`--no-session-persistence`).
  System prompt instructs the model to extract specific testable
  claims with verbatim evidence snippets and short rationales.

### CLI

- New `vela scout <folder> --frontier <path> [--backend <model>]
  [--dry-run] [--json]` subcommand. Renders a report to terminal
  with agent name, run id, model, pdfs seen/processed, candidates,
  proposals written, and skipped reasons.
- `--backend <name>` is treated as a model alias (e.g. `sonnet`).
  Empty / `"claude-cli"` / `"default"` use the session default.
- `VELA_SCIENTIST_CLI` env var overrides the `claude` binary path.

### Workbench (vela-site)

- New **Inbox** tab in `/frontiers/view?vfr=…`. Lists every proposal
  in the frontier's `proposals[]`, grouped by `agent_run.run_id`.
  Each card: claim in serif, rationale in italic, source-file
  basenames + caveat flags as colored chips, status badge, REJECT /
  ACCEPT buttons.
- Accept / Reject POSTs to `/api/queue` (existing
  `http_queue_append`). On success, card tints moss/madder, status
  pill swaps to "STAGED · ACCEPT/REJECT", buttons disable, hint
  reads "staged for accept · in queue". Sticky banner at top reads
  "N actions staged in your local queue. Sign and apply with the
  CLI: `vela queue sign --all`" with one-click Copy.
- New **Diff** tab — newest-first list of every signed `StateEvent`
  in the frontier. Color-coded kind chips: `finding.add` moss,
  `finding.review` signal blue, `finding.revise/caveat` brass,
  `finding.retract` madder, `finding.note` stale.
- API base URL is configurable via `?api=` so the deployed Astro
  site can drive a local `vela serve --workbench`. Without `?api=`,
  buttons render disabled with an inline hint.
- Browser never sees a signing key. The Workbench stages decisions
  only; `vela queue sign` is the only path that produces signed
  canonical state.

### Dogfood result

One PDF (`examples/paper-folder/papers/focused-ultrasound.pdf`,
~1.9 KB) → 2 candidate findings extracted by Claude in ~12
seconds, both with verbatim evidence snippets and short rationales.
Both renderable in the Inbox tab with grouping under the same
`agent_run.run_id`. End-to-end demonstrated through `vela scout`
→ frontier write → browser Inbox render.

### Verification

- `cargo build --workspace`: clean (4 crates).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 353 protocol + 6 hub + 1 scientist
  tests pass.
- `vela check frontiers/bbb-alzheimer.json --strict`: passes
  unchanged. BBB normalize dry-run: zero deltas.
- vela-site.fly.dev redeploy: green; /frontiers/view shows the
  new Inbox + Diff tabs.

### What's not in v0.22

- **v0.23: Notes Compiler** for Markdown / Obsidian vaults.
- **v0.24: Code/Notebook Analyst** for Jupyter + scripts +
  AnalysisRun objects.
- **v0.25: Dataset support** for CSV / Parquet inputs.
- **v0.26: VelaBench** for agent state-update scoring.

Other deliberate non-goals: browser-side WebCrypto signing
(CLI-only), auto-merge of any kind, edit-in-Inbox (reject and
re-propose), multi-frontier ingestion.

See `docs/AGENT_INBOX.md` for the full walkthrough.

## 0.21.0 - 2026-04-26

The self-hostable release. Two pieces:

1. **SQLite backend for the hub** — `vela-hub` now picks its DB engine
   from `VELA_HUB_DATABASE_URL`. `postgres://…` runs the production
   path (vela-hub.fly.dev, vela-hub-2.fly.dev). `sqlite://…` auto-
   creates the schema on first run and serves every endpoint the
   Postgres path serves: list/get/depends-on/publish. Anyone with
   `cargo install` can run a self-hosted hub without external
   infrastructure.
2. **Living-repo CI workflow for Will's frontier** —
   `.github/workflows/will-alzheimer-living-repo.yml` republishes
   weekly Mondays 14:30 UTC under a fresh `reviewer:will-blair-bot`
   identity (separate from the curator's personal key, which stays on
   his laptop). Mirrors the BBB pattern. Includes a federation drill
   step that mirrors the published manifest to vela-hub-2 so both hubs
   stay in sync automatically.

### Hub substrate

- New `crates/vela-hub/src/db.rs` introduces a `HubDb` enum wrapping
  `PgPool` or `SqlitePool`. Five methods (`health`, `schema_present`,
  `list_latest_entries`, `get_entry`, `insert_entry`) cover everything
  the route handlers need. Each variant uses its own placeholder
  syntax (`$1` vs `?`) and `raw_json` storage (JSONB vs TEXT-as-JSON
  round-tripped via `serde_json`).
- `AppState.pool: Pool<Postgres>` → `AppState.db: HubDb`. Five route
  handlers updated. Production Postgres path unchanged in behavior.
- `sqlx` workspace dep gains the `sqlite` and `any` features.

### CI / infrastructure

- `.github/workflows/will-alzheimer-living-repo.yml` (new). Weekly
  cron + workflow_dispatch. Builds the CLI, signs under
  `reviewer:will-blair-bot` (key in `VELA_WILL_BOT_KEY` GitHub
  secret), publishes to `vela-hub.fly.dev`, then mirrors to
  `vela-hub-2.fly.dev` as a federation drill.
- `reviewer:will-blair-bot` actor registered on
  `frontiers/will-alzheimer-landscape.json` with tier=auto-notes.

### Validation

- Local SQLite hub spun up on `sqlite:///tmp/vela-local-hub.db`,
  mirrored BBB Flagship + Will's frontier from the public hub:
  fresh-insert both, byte-identical pull-and-verify confirms
  `verified=true` and matching snapshot hashes, `depends-on` reverse
  lookup correctly identifies Will's frontier as depending on BBB.
  Same Rust binary, two backends, same observable behavior.

### Versioning

- Workspace `0.20.0 → 0.21.0`.
- `vela --version → 0.21.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no substrate change. The hub
  backend abstraction is internal; the wire shape is unchanged.

## 0.20.0 - 2026-04-26

The federation release. Hub-to-hub mirroring has been on the deferred
list since v0.8 ("forced by ≥ 2 hubs"). v0.20 ships the second hub —
`https://vela-hub-2.fly.dev` — and the `vela registry mirror` primitive,
turning the substrate's "the signature is the bind, not the hub
identity" doctrine from a claim into an empirical fact.

### Infrastructure

- **Second hub deployed at `https://vela-hub-2.fly.dev`** — same Rust
  binary, separate Neon Postgres database, separate Fly app under the
  `vela-237` org. Runs identically to `vela-hub.fly.dev`; doctrine
  unchanged (dumb signed transport, signature-gated POST, no
  allowlist).

### CLI

- **`vela registry mirror <vfr_id> --from <hub-A> --to <hub-B>`.**
  GETs the signed manifest from `from/entries/{vfr_id}` and POSTs it
  verbatim to `to/entries`. Both hubs validate the manifest's Ed25519
  signature against the embedded `owner_pubkey`; mirroring is a no-op
  for authenticity. Idempotent on the destination's
  `(vfr_id, signature)` unique constraint — re-mirroring returns
  `duplicate=true`.

### Validation

End-to-end against the live deploys:

- 3 frontiers (BBB Flagship, BBB-extension, Will's Alzheimer's
  drug-target landscape) mirrored from hub-1 → hub-2 cleanly.
- `vela registry pull` against hub-2 produces byte-identical output
  with `verified=true`, same as against hub-1.
- Snapshot hashes match across hubs for the same vfr_id.
- Re-mirror is idempotent (returns `duplicate; signature already
  known` from the destination).

### What this unblocks

- **Resilience.** Mirror to a backup hub ahead of time; if one goes
  down, `vela registry pull` keeps working against the other.
- **Seeding.** A fresh hub instance can be primed from an existing
  one without any signing roundtrip.
- **Independent deploys.** An institution running its own hub can
  mirror the public hub for offline/air-gapped use, then publish its
  own frontiers independently.
- **The doctrinal property the substrate has been claiming since v0.7:**
  pulling a frontier doesn't require trusting a single hub. The
  signature is over the publisher's content, not the serving
  infrastructure.

### Documentation

- `docs/HUB.md` gains a Federation section with the live URLs, the
  mirror command, and the doctrine line. The "What is deferred" list
  loses the "hub-to-hub federation" entry.

### Versioning

- Workspace `0.19.0 → 0.20.0`.
- `vela --version → 0.20.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Federation is
  a transport pattern, not a substrate primitive.

## 0.19.0 - 2026-04-26

The bundled-ontology release. Will's frontier had 33 entities, every one
flagged `needs_review = true` because they were hand-added with no
canonical-id resolution. Strict-check kept blocking on `needs_human_review`
signals downstream of that. v0.19 adds a small bundled lookup table for
common Alzheimer's vocabulary (UniProt for proteins, MeSH for diseases,
DrugBank for compounds, NCBI Taxonomy for organisms, Cell Ontology for
cell types, plus the v0.10 cross-domain physics entries). 32 of 33
entities on Will's frontier resolve. The substrate now has a credible
"resolved against an authoritative source" claim for matched entities;
the rest stay flagged needs_review for honest curator follow-up.

### Substrate

- **`crate::entity_resolve` module.** Hardcoded `OntologyEntry` table
  matching `(normalized_name, entity_type)` to a `ResolvedId`. About
  ~28 entries: amyloid-beta, APP, BACE1, tau, TREM2, ApoE, PSEN1/2,
  Alzheimer's disease, MCI, Lecanemab, Aducanumab, Donanemab,
  Verubecestat, Liraglutide, Semaglutide, Exendin-4, microglia,
  blood-brain barrier, Homo sapiens, Mus musculus, plus xenon, WIMP,
  XENONnT, LZ for the cross-domain physics entries. `resolve_frontier`
  walks every finding's entities and applies the lookup; matched
  entities get `canonical_id`, `resolution_method = Manual`,
  `resolution_confidence = 0.95`, `needs_review = false`.

### CLI

- **`vela entity resolve <frontier>`** — runs the bundled resolution
  in place. Idempotent unless `--force` is passed. Prints a per-finding
  summary with unresolved entity hints.
- **`vela entity list`** — read-only inspection of the bundled table.

### Conformance vector + cross-impl

- **`tests/conformance/supersede-and-sources/`** — a new 2-finding
  fixture exercising both v0.13 inline source/evidence/condition
  materialization AND v0.14 `finding.supersede`. `expected.json` pins
  every content-addressed id, the supersede chain (old finding's
  `flags.superseded`, new finding's auto-injected `supersedes` link,
  the `finding.superseded` event with `payload.new_finding_id`), and
  the materialized projection counts.
- `scripts/cross_impl_conformance.py` gains `--expect <path>` to
  validate the loaded frontier against an `expected.json` fixture.
  Walks counts, supersede chain, materialization. Used by the new
  conformance vector to give second-implementation re-derivers a
  precise contract to satisfy.

### Versioning

- Workspace `0.17.0 → 0.19.0` (skipping 0.18 which would have only
  shipped the conformance vector — bundled with v0.19 for one cycle).
- `vela --version → 0.19.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. The bundled
  ontology table is a runtime lookup, not a substrate addition.
- New top-level CLI command `entity` added to the strict release
  surface (with `resolve` + `list` subcommands).

## 0.17.0 - 2026-04-26

The agent-surface release. Forced by probing `vela serve` against Will's
v0.14 frontier — the HTTP/MCP surface hadn't been exercised since the
v0.11→v0.16 substrate work shipped, and two real gaps surfaced: (1)
`/api/events` accepted `?kind=` and `?target=` query params silently
without filtering, and (2) the MCP tool registry had no way to fetch the
event history of a single finding (the natural agent question after seeing
a `flags.superseded = true` finding: "what changed and when?").

### HTTP

- **`/api/events?kind=<kind>&target=<vf_id>`** — server-side filters now
  applied. Before v0.17 the params were silently ignored and the full
  event log was returned. Filters apply BEFORE pagination so cursor-based
  pagination (`?since=<vev_id>&limit=N`) works on the filtered view.
  Response body grows a `filtered_total` field alongside the existing
  `count` (returned slice) and `log_total` (full log size).

### MCP

- **`get_finding_history` tool** — returns the chronological event log
  for one finding (asserted, reviewed, caveated, noted, confidence-revised,
  superseded, retracted), sorted ascending by timestamp. The natural
  agent path: see a finding flagged `superseded`, call this tool, walk
  the supersedes chain via `payload.new_finding_id` on the
  `finding.superseded` event. Brings the MCP tool count to 10. Validated
  by `vela serve --check-tools`.

### Validation

- `vela serve --check-tools` now exercises `get_finding_history` against
  the first finding in the loaded frontier; passes 10/10 against Will's
  v0.14 frontier.

### Versioning

- Workspace `0.16.0 → 0.17.0`.
- `vela --version → 0.17.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. The MCP tool
  registry is a runtime contract, not a schema.

## 0.16.0 - 2026-04-26

The supersede-aware composition release. Closes the two frictions surfaced
by the v0.15 Patel dogfood:
1. `vela link add` accepted a cross-frontier `contradicts` link to a
   `flags.superseded = true` finding silently — Patel could be contradicting
   wording that's already been refined.
2. The hub's `/depends-on` endpoint shipped in v0.15 had no Workbench
   surface; users had to `curl` to see who referenced their frontier.

### CLI

- **`vela link add` cross-frontier target-status check.** When the link
  target is `vf_<id>@vfr_<id>`, the substrate fetches the dep's frontier
  from its declared locator (HTTPS, ~15s timeout) and inspects the target
  finding's `flags.superseded`. If `true`, prints a one-line warning
  (`warn · cross-frontier target … has flags.superseded = true. You may
  be linking to outdated wording. …`) suggesting `pull --transitive` to
  inspect the supersedes chain. The link is still recorded — this is a
  best-effort review hint, not a hard refusal. `--no-check-target` skips
  the network fetch (CI / offline use). Failure to fetch is silent.

### Workbench

- **Referenced-by panel on `/workbench`.** When loaded with `?vfr=…`,
  fetches `/entries/{vfr}/depends-on` from the hub and renders one row
  per dependent (vfr_id, name, owner_actor_id, publish date) with
  click-through to the dependent's own /workbench view. Hidden when the
  hub returns zero dependents or is unreachable (fail-quiet).

### What this unblocks

- The bidirectional view of cross-frontier composition is now visible
  not just queryable. A visitor on Will's frontier sees who in the world
  references it; click-through navigates the network.
- Publishers writing cross-frontier `contradicts` / `extends` / `depends`
  links get a same-shell warning when their target's wording has been
  refined out from under them.

### Versioning

- Workspace `0.15.0 → 0.16.0`.
- `vela --version → 0.16.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. The hub
  endpoint shipped at v0.15 unchanged.

## 0.15.0 - 2026-04-26

The bidirectional release. Cross-frontier composition has been one-directional
since v0.8: a dependent frontier declares + pins its deps, but the upstream
has no way to learn who is referencing it. Surfaced by another dogfood pass
(Patel persona — clinical pharmacologist publishing a Lecanemab null-result
replication that contradicts Will's frontier): the substrate let her publish
and link cleanly, but Will would have no signal that the contradiction
exists. v0.15 closes that gap at the hub layer.

### Hub

- **`GET /entries/{vfr_id}/depends-on`**. Returns the registry entries
  whose frontier declares a cross-frontier dependency on `{vfr_id}`.
  Implementation walks the latest-per-vfr view, fetches each frontier
  through the existing `fetch_frontier_cached` LRU, and filters by
  `frontier.dependencies[].vfr_id`. O(N) on cold cache, memory-only on
  warm. A future optimization would denormalize a `dependent_vfrs` JSONB
  column at POST time and back this with a SQL `?` lookup.

### CLI

- **`vela registry depends-on <vfr_id> [--from <hub>]`** — calls the
  new endpoint and prints "N frontiers depend on vfr_X" with the list.
  `--json` returns the raw `vela.depends-on.v0.1` envelope.

### What this unblocks

- The bidirectional view of cross-frontier composition. Pre-v0.15 the
  question "who is referencing my frontier?" required scraping every
  hub entry's frontier file and grep-ing for your `vfr_id`. Now it's
  one HTTP call. Validates the substrate's "this is a network, not a
  file format" claim concretely — running against the live hub right
  now, BBB Flagship (`vfr_093f7f15b6c79386`) reports 4 dependents:
  three versions of Will's Alzheimer's frontier and BBB-extension.

### Known frictions surfaced but deferred

- **No warning when `vela link add` targets a finding with
  `flags.superseded = true`.** Patel's contradicts-link to Will's
  superseded Lecanemab finding (`vf_b1f04d00abcd7476`) was accepted
  silently; the substrate doesn't currently check the dep's local cache
  for `superseded` flags at link-add time. Best-effort warning at link-add
  + a `--allow-superseded` escape would close this. Defer to v0.16.
- **Workbench "Referenced by" panel** to surface the new endpoint
  visually on `/workbench` and `/workbench/finding`. Defer to v0.16.

### Versioning

- Workspace `0.14.0 → 0.15.0`.
- `vela --version → 0.15.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Hub schema
  for the new endpoint envelope is `vela.depends-on.v0.1`.

## 0.14.0 - 2026-04-26

The supersede release. Until v0.14 every other proposal kind existed
(`add`, `review`, `note`, `caveat`, `confidence_revise`, `reject`,
`retract`) but you couldn't *change a claim's text*. The assertion text
is part of the content address; mutating it would re-derive the `vf_…`
id and orphan all events targeting the old finding. Real corrections
(Phase 4 follow-up data, refined wording, scope change) had to be stacked
as caveats — which let the original prose travel unchanged. v0.14 adds
the substrate-correct path: a *new* content-addressed finding that
explicitly supersedes the old one. Both stay queryable.

### Substrate

- **`Flags.superseded: bool`** added to `bundle.rs` (additive,
  serde-skipped when false; pre-v0.14 frontiers byte-identical).
- **`finding.supersede` proposal kind** + **`finding.superseded` event
  kind**. `apply_supersede` validates the old finding exists and is not
  already superseded, validates the new finding has a distinct content
  address, pushes the new finding, auto-injects a `supersedes` link from
  new → old, sets `flags.superseded = true` on the old finding, and emits
  a `finding.superseded` canonical event targeting the *old* finding
  (with `new_finding_id` in the event payload). Event-payload validator
  in `events.rs` extended to require `proposal_id` + `new_finding_id`
  on the new event kind.
- **`build_finding_bundle` extracted** from `build_add_finding_proposal`
  so `add_finding` and `supersede_finding` share content-addressing
  logic.

### CLI

- **`vela finding supersede <old-id>`** with `--assertion`, `--reason`,
  and the full v0.11 enrichment flag set (DOI, PMID, year, journal,
  source-authors, conditions-text, species, study-type flags). The
  command builds the new finding bundle and wraps it in a
  `finding.supersede` proposal targeting `old-id`. `--apply` accepts and
  applies in one step; without it the proposal is recorded for review.

### Workbench

- **Source registry panel on `/workbench`** — surfaces the materialized
  projection v0.13 introduced. Renders one row per `SourceRecord` with
  source-type, journal, year, and clickable DOI/PMID badges. Hidden when
  the loaded frontier has no sources (legacy frontiers, or any frontier
  before its first finding).
- **Event timeline on `/workbench/finding`** — shows the chronological
  history for the active finding: asserted, reviewed, caveated, noted,
  superseded, etc. For `finding.superseded` events, the new finding's
  vf_id renders as a click-through link that walks you to the next
  version (preserving `?vfr=…` for multi-frontier mode).

### Tests

- `proposals::tests::v0_14_supersede_creates_new_finding_and_marks_old`
- `proposals::tests::v0_14_supersede_refuses_already_superseded`
- `proposals::tests::v0_14_supersede_refuses_same_content_address`

  347 tests passing.

### Versioning

- Workspace `0.13.0 → 0.14.0`.
- `vela --version → 0.14.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — `Flags.superseded` is additive and
  serde-skipped. Pre-v0.14 frontiers replay byte-identically.

### Known gaps surfaced but deferred

- `vela ingest --paper <pdf>` — a single-paper draft path remains
  deferred. Most useful when there's a real PDF in front of a real
  publisher.

## 0.13.0 - 2026-04-26

The source-record materialization fix. v0.12 unblocked event-replay for
CLI-built frontiers; the next dogfood iteration immediately surfaced the
last lint blocker: `missing_source_record` on every finding whose
provenance derives a SourceRecord that wasn't yet in `frontier.sources`.
Pre-v0.13, the only way to populate the projections was `vela normalize
--write` — but normalize refuses on event-ful frontiers ("normalize
before proposal-backed writes"), so any frontier built via the
proposal/event flow could never reach proof-ready state.

### Substrate

- **`proposals::create_or_apply` materializes source / evidence /
  condition projections inline at apply time.** Whenever a proposal is
  applied (any of finding.add, finding.review, finding.note, …),
  `sources::materialize_project` runs and refreshes
  `frontier.sources[]`, `frontier.evidence_atoms[]`, and
  `frontier.condition_records[]` from the current finding set. Idempotent:
  when no finding state changed (caveat/note on existing findings) the
  projection re-derives the same bytes, so canonical hashes are stable.
  When a proposal isn't applied (pending review), only stats recompute —
  unchanged from pre-v0.13 behavior.

### What this unblocks

- Strict-check on CLI-built frontiers no longer flags `missing_source_record`.
  Will's Alzheimer's frontier rebuild now materializes 10 sources, 11
  evidence atoms, and 11 condition records inline as the 11 findings
  land. Proof-readiness signals shift from "missing source registry" to
  the genuine review-needed signals (entity-resolution-confidence,
  experimental-finding-without-species), which are correct things for a
  fresh manual frontier to flag.

### Tests

- `proposals::tests::v0_13_apply_materializes_source_records_inline`
  exercises the new flow: a single `finding.add` proposal applied via
  `create_or_apply(apply: true)` produces a non-empty `sources[]`,
  `evidence_atoms[]`, and `condition_records[]` in the persisted
  frontier. 344 tests passing.

### Versioning

- Workspace `0.12.0 → 0.13.0`.
- `vela --version → 0.13.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Pre-v0.13
  frontiers built via the CLI grow source/evidence/condition projections
  the next time any proposal applies; pre-v0.13 frontiers built without
  the CLI (BBB, the conformance vectors) keep whatever projection state
  they already had.

## 0.12.0 - 2026-04-25

The link-hash fix. Surfaced by attempting to dogfood the v0.11 enrichment
on Will's Alzheimer's frontier: every CLI-built frontier with `vela link
add` calls broke `vela check --strict` because the `finding_hash` included
links, but `vela link add` (shipped in v0.9) mutated links inline without
emitting a state event. The asserted-event's after_hash became stale the
moment a link landed, breaking the event-replay chain.

### Substrate

- **`finding_hash` excludes `links` from the hashed view of a finding.**
  Per Protocol §5, links are review surfaces — typed relationships
  inferred at compile or review time, not part of the finding's content
  commitment. They're mutable; state-changing events (caveat / note /
  review / revise / retract) still mutate annotations / flags / confidence,
  and those remain in the hash and chain through events properly. The
  finding's own content address (the `vf_…` ID) is unchanged — it never
  used links.

### What this unblocks

- Any frontier with CLI-added links now passes `vela check --strict`
  event-replay validation. Previously: every such frontier failed silently
  on strict because hash divergence broke the chain.

### Known gaps surfaced but deferred

- **Source-record materialization on event-ful frontiers.** `vela check
  --strict` flags `missing_source_record` for findings whose provenance
  could derive a `SourceRecord` that isn't in `frontier.sources[]`. The
  fix would normally be `vela normalize --write`, but normalize refuses
  to mutate frontiers that already have canonical events ("normalize
  before proposal-backed writes"). For finding.add to materialize source
  records inline, or normalize to gain an event-aware mode, is forced
  by the next dogfood iteration. Defer to v0.13.

### Versioning

- Workspace `0.11.0 → 0.12.0`.
- `vela --version → 0.12.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Pre-v0.12
  frontiers (BBB, BBB-extension, Will's v0.10 frontier, the conformance
  vectors) all replay byte-identically; the hash semantics shift only
  for findings with non-empty links, where the substrate now treats them
  consistently with how `vela link add` was already mutating them.

## 0.11.0 - 2026-04-25

The richer-finding-add release. v0.10 fixed the CLI's biology-leaning enums.
Inspecting Will's first non-bot frontier on the public hub surfaced the next
shallow-data problem: every finding's provenance was just a citation string
(no DOI, PMID, year, journal as structured fields), every finding's
conditions were the same placeholder ("Manually added finding; requires
evidence review…"), and there was no way to refresh a stale cross-frontier
dependency pin when the upstream frontier republished. The substrate had the
fields; the CLI just didn't ask for them. v0.11 fills the gaps.

### CLI surface

- **`vela finding add` provenance flags** — `--doi`, `--pmid`, `--year`,
  `--journal`, `--url`, `--source-authors`. Each populates the corresponding
  structured `Provenance` field (`url` is new in v0.11; serde-skipped when
  None so pre-v0.11 frontiers serialise byte-identically). `--source-authors`
  takes a semicolon-separated list and writes one `Author` per name (distinct
  from `--author`, which remains the curating Vela actor). When omitted, the
  curator-as-author fallback from v0.10 still applies.
- **`vela finding add` conditions flags** — `--conditions-text`, `--species`,
  `--in-vivo`, `--in-vitro`, `--human-data`, `--clinical-trial`. Replaces the
  hardcoded "Manually added finding; requires evidence review…" placeholder
  that was on every manual finding from v0.5–v0.10. `--species` takes a
  semicolon-separated list and populates `species_verified`.
- **`vela frontier refresh-deps`** — fetches the current hub snapshot for
  every declared cross-frontier dep and re-pins. Reports per-dep
  `unchanged`, `refreshed` (with old → new), `missing` (vfr_id not on hub),
  or `unreachable`. `--dry-run` shows the diff without writing. `--from`
  defaults to https://vela-hub.fly.dev. The forcing function: BBB
  republishes weekly via CI; without refresh, your local pin goes stale
  silently.

### Substrate

- `Provenance.url` (new optional field) — generic source URL when none of
  the structured identifiers fit (preprint server URL, dataset landing
  page, talk recording). Serde-skipped when None; pre-v0.11 frontiers
  validate byte-identically.

### Tests

- Three new unit tests in `state::v0_11_finding_tests` covering provenance
  flag round-trip, conditions flag round-trip, and the back-compat
  fallback when no v0.11 flags are supplied. 343 tests passing.

### Versioning

- Workspace `0.10.0 → 0.11.0`.
- `vela --version → 0.11.0`; banner stamps bump in lockstep.
- `VELA_COMPILER_VERSION → vela/0.11.0` for new frontier scaffolds; pre-v0.11
  publisher stamps continue to validate (compiler-stamp softening from v0.9).
- Schema version stays at `v0.10.0` — `Provenance.url` is additive and
  serde-skipped, so no schema URL bump.

## 0.10.0 - 2026-04-25

The first non-bio frontier. Same dogfood mechanic as v0.9: I played a second
external publisher — a particle-astrophysics postdoc with a frontier on
direct-detection constraints on spin-independent WIMP-nucleon cross-section.
The path completed end-to-end (`vfr_dede3b473cac72cf` is on `vela-hub.fly.dev`),
but the schema's biology-leaning enums leaked badly. v0.10 closes that gap
additively — every pre-v0.10 frontier replays byte-identically.

### Substrate

- **Entity type extensions** (`crates/vela-protocol/src/bundle.rs`).
  `VALID_ENTITY_TYPES` adds `particle` (WIMPs, photons), `instrument`
  (XENONnT, JWST — capital objects that run measurements), `dataset`
  (instrument data releases, distinct from the paper that reports them),
  and `quantity` (named numerical values with units, e.g. `28 GeV/c^2`).
  Pre-v0.10 entries unchanged; `other` remains the escape valve.
- **Assertion type extensions.** `VALID_ASSERTION_TYPES` adds `measurement`
  (numerical-quantity reports) and `exclusion` (upper/lower bounds at a
  confidence level — "WIMP mass < X at 90% CL"). Pre-v0.10 entries unchanged.
- **Source type extension.** `VALID_PROVENANCE_SOURCE_TYPES` adds
  `data_release` for instrument runs, observation campaigns, and dataset
  versions that are themselves the substantive object (XENONnT SR0, Planck
  data releases, JWST observation runs).
- **Schema URL bumps `v0.8.0 → v0.10.0`** for new frontiers. The validator
  now accepts either URL (`KNOWN_SCHEMA_URLS = ["v0.8.0", "v0.10.0"]`)
  with the same publisher-claimed doctrine the v0.9 compiler-stamp softening
  established. Pre-v0.10 frontiers (BBB, BBB-extension, the v0.8
  cross-frontier conformance vector, all entries already on the public hub)
  validate byte-identically under v0.10 — no churn to content addressing.

### Conformance

- **`tests/conformance/non-bio-domain/`**: a new physics frontier that
  exercises every v0.10 enum extension on two findings (XENONnT exposure
  measurement + WIMP cross-section exclusion limit) plus a `depends` link
  between them. `expected.json` pins re-derived snapshot/event-log hashes;
  `python3 scripts/cross_impl_conformance.py tests/conformance/non-bio-domain/frontier.json`
  PASSes.

### Documentation

- **`docs/PUBLISHING.md`** enum tables updated with the v0.10 additions
  and a paragraph explaining their domain-neutral provenance.
- **`docs/PROTOCOL.md`** §5.1 (new) documents the v0.10 enum extensions
  and the back-compat schema URL pattern; spec-version stamp bumps to
  v0.10.0.

### Versioning

- Workspace version `0.9.0 → 0.10.0`.
- `vela --version → 0.10.0`; banner stamps bump in lockstep.
- `VELA_SCHEMA_URL` and `VELA_SCHEMA_VERSION` bump to `v0.10.0`/`0.10.0`;
  the validator accepts both `0.8.0` and `0.10.0` for back-compat.
- `VELA_COMPILER_VERSION` bumps to `vela/0.10.0` for new frontier scaffolds;
  pre-v0.10 publisher stamps continue to validate (v0.9 compiler-stamp
  softening was the precedent).

### What is deferred to v0.11+

- **Per-domain extension packs.** v0.10 widens the canonical enums to handle
  one second domain (physics). When ≥ 3 non-bio frontiers exist with
  divergent vocabulary needs, a `frontier.domain` declaration + per-domain
  enum extension may become forced. Until then, the additive default is
  enough.
- **Structured numerical-claim representation.** A `measurement` finding's
  substance is `value · unit · confidence_level · target_quantity`; today
  it lives in prose under `assertion.text`. A first-class `claim_numeric`
  block on the finding bundle would make claims comparable across implementations
  and queryable. Holding until ≥ 2 publishers reach for it.
- **Conditions struct extensions** (instrument live-time, exposure,
  fiducial mass, blinding regime). Pre-v0.10 conditions are bio-heavy; the
  current `text` field still holds for non-bio findings, but a domain-aware
  conditions schema is open.

## 0.9.0 - 2026-04-25

The first-publisher cleanup. v0.8 proved cross-frontier composition end-to-end
on the public hub. A dogfood run pretending to be an external publisher
("Dr. M, 4 GLP-1/Alzheimer's findings, zero prior context") surfaced 12
distinct frictions on the path from `vela-hub.fly.dev` landing → signed
manifest live. None required a substrate change. v0.9 fixes the surface so
the next *real* external publisher hits a coherent path.

### CLI surface

- **`vela frontier new <path> --name "..."`** scaffolds a publishable
  `frontier.json` stub that passes `vela check --strict` immediately. Closes
  the bootstrap gap between `vela init` (creates `.vela/` repo, not
  publishable) and `vela finding add` (requires the file to pre-exist). Use
  `frontier new` whenever the target is the hub.
- **`vela link add <frontier> --from vf_… --to vf_…[@vfr_…] --type …`**
  is the CLI on-ramp for typed links. Until v0.9, links required hand-editing
  JSON — the proposal/event flow had no link counterpart. The handler validates
  the target via `LinkRef::parse`, checks that local targets exist, refuses
  cross-frontier targets without a declared dep (with the exact `frontier
  add-dep` invocation in the error), and recomputes `stats.links` /
  `stats.link_types` so strict validation stays green.
- **CLI enums single-sourced with the validator** (`crates/vela-protocol/src/bundle.rs`).
  `VALID_ASSERTION_TYPES`, `VALID_EVIDENCE_TYPES`, `VALID_PROVENANCE_SOURCE_TYPES`,
  `VALID_LINK_TYPES`, and `VALID_ENTITY_TYPES` now live next to the bundle
  types and are imported by both `cmd_finding_add`/`cmd_link_add` and
  `validate.rs`. Invalid `--type`/`--evidence-type`/`--source-type`/entity
  values fail at add-time with the full valid set in the error message
  instead of at strict-check time after the (now content-addressed) finding
  has been written.
- **`actor`, `link`, `registry` surfaced in `vela --help`.** Pre-v0.9 these
  worked but were absent from the strict-help banner — invisible to a new
  user reading the CLI surface. Help also adds a "Publish your own frontier"
  block walking the five-command path end-to-end.
- **`vela check --json` returns per-failure detail.** `checks[].errors[]`
  carries the schema validator's `{file, message}` records; `checks[].blockers[]`
  surfaces the `{id, kind, severity, reason}` of every signal that blocks
  strict mode. Pre-v0.9, `--json` reported `failed: 4` with no per-failure
  context.

### Documentation

- **`docs/PUBLISHING.md`** — the end-to-end "first publish" walkthrough.
  Linked from the README quick start; covers scaffold → findings → optional
  cross-frontier deps + links → keypair → actor registration → publish →
  verify on hub → CI republish pattern. Includes the enum table and a
  troubleshooting section keyed off the actual error strings v0.9 emits.
- **README publishing block** at the top of the file. The pre-v0.9
  README's quick start went `compile → check → proof`; the publish path
  was buried at line 39 under HUB.md. v0.9 puts the five-command publish
  path on the front page.

### Versioning

- Workspace version `0.8.0 → 0.9.0`.
- `vela --version → 0.9.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.8.0` — v0.9 ships *no* schema changes. Pre-v0.9
  frontiers (BBB, BBB-extension, the v0.8 conformance vector) replay
  byte-identically; their `vela_version` and `compiler` stamps are publisher-
  claimed and unchanged.

### What is deferred to v0.10+

- Hub-to-hub federation. Forced by ≥ 2 hubs; still 1.
- Hub-hosted frontier blobs. Locator stays where the publisher hosts it.
- Browser-side WebCrypto signing.
- Webhooks/SSE on the hub.
- Multi-frontier Workbench mode (load two frontiers simultaneously into one
  rete view). The dashed-edge-to-ghost-node treatment shipped in v0.8
  remains sufficient for the cross-frontier viewer.
- A real domain (`vela-hub.fly.dev` is sufficient).
- `vela finding rekey` for content-address repair after a hand-edit. v0.9's
  enum guard at add-time removes the most common path into that breakage;
  the cure for the rest is "delete and re-add."

## 0.8.0 - 2026-04-25

The composition run. v0.7 stood up the public hub, the deployed
Workbench, and the BBB living repo. The next substrate move forced
by the protocol's own shape was *composition*: a finding in one
frontier referencing a finding in another. v0.8 closes that gap with
the smallest substrate change that keeps the whole verification
chain — canonical-JSON, signature, snapshot pin — extending across
frontier boundaries.

### Substrate

- **Cross-frontier link targets** (`crates/vela-protocol/src/bundle.rs`).
  `Link.target` now parses as `LinkRef::Local { vf_id }` (in-frontier,
  pre-v0.8 shape) or `LinkRef::Cross { vf_id, vfr_id }` (cross-frontier,
  new). Round-trip identity via `format()`. The wire shape stays
  `String` — canonical-JSON unchanged, no schema churn.
- **`ProjectDependency` extension** (`crates/vela-protocol/src/project.rs`).
  Three new optional fields — `vfr_id`, `locator`, `pinned_snapshot_hash`
  — turn the existing compile-time dependency record into a verifiable
  cross-frontier dep declaration. `Project::cross_frontier_deps()` and
  `Project::dep_for_vfr()` helpers; serde-skipped when None so pre-v0.8
  frontiers serialize byte-identically.
- **Strict cross-frontier validation** (`crates/vela-protocol/src/validate.rs`).
  Any link target with `@vfr_…` must have a matching declared dep; any
  cross-frontier dep must declare both `locator` and `pinned_snapshot_hash`.
  Pinned-by-default — mirrors Cargo.lock / package-lock.json. Strict mode
  fails with the missing dep / missing pin named.
- **Transitive pull-and-verify** (`crates/vela-protocol/src/registry.rs`).
  `pull_transitive(registry, primary_vfr, out_dir, max_depth) -> PullResult`
  walks the dep graph BFS, fetches each dep's frontier, verifies signature
  + snapshot + event-log + that the dep's actual snapshot matches the
  dependent's pinned hash. Cycle-safe (visited-set + content-addressing).
  `vela registry pull --transitive [--depth N]` exposes it; `--depth` defaults
  to 4. Total verification — partial trust isn't a state v0.8 supports.

### CLI

- **`vela frontier add-dep / list-deps / remove-dep`** (`crates/vela-protocol/src/cli.rs`).
  New subcommand group for managing cross-frontier dependency declarations
  on a frontier file. `add-dep` writes a complete
  `vfr_id`+`locator`+`pinned_snapshot_hash` triple; `remove-dep` refuses
  if any link still references the dep.

### Surfaces

- **Hub renders cross-frontier links as click-through**
  (`crates/vela-hub/src/main.rs`). When a finding's link target parses
  as `vf_…@vfr_…` and the target's `vfr_id` matches a declared dep,
  the link becomes `<a href="/entries/{vfr}/findings/{vf}">{vf} @
  {dep_name}</a>` — italic-serif `cross-vfr` badge, navigable to the
  remote frontier's entry page. Undeclared cross-frontier targets get
  a brass `(undeclared dep)` chip.
- **Workbench rete: dashed cross-frontier edges + ghost nodes**
  (`site/src/pages/workbench/index.astro`). External `vfr_id`s appear
  as small open-square ghost nodes pinned to the canvas rim, one per
  distinct external frontier. Edges to ghosts are dashed signal-blue.
  Click a ghost to jump to the hub's `/entries/{vfr}` page.

### Conformance

- **2-frontier conformance vector** (`tests/conformance/cross-frontier/`).
  Frontier A (1 finding, no deps) + Frontier B (1 finding linking to A
  via `vf_…@vfr_…`, declares A as a dep) + `expected.json` listing every
  derived id and the resolution shape. A second implementation grades
  itself by reproducing each id and confirming the dep's snapshot pin
  matches A's actual snapshot.
- **`scripts/cross_impl_conformance.py --cross-frontier <path>`** loads
  each declared dep and checks two new properties: every cross-frontier
  link resolves to a declared dep, and every dep's `pinned_snapshot_hash`
  matches the loaded dep's actual snapshot. Verified PASS on the
  positive vector and FAIL (exit 1) on a tampered copy.

### Worked example

- **`frontiers/bbb-extension.json`** + `.github/workflows/bbb-extension-living-repo.yml`.
  A small companion frontier ("BBB Flagship · follow-up") with one
  finding that extends BBB's first finding via the v0.8 link-target
  syntax. Declares BBB as a cross-frontier dep with the v0.8 vfr_id
  and snapshot pin. Separate `reviewer:bbb-extension-bot` actor;
  weekly cron 14:30 UTC (offset from BBB's 14:00). The hub now serves
  two frontiers; `vela registry pull vfr_… --transitive --from
  https://vela-hub.fly.dev/entries` walks both end-to-end.

### Cut

- Workspace + crate versions: `0.7.0 → 0.8.0`.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  → `v0.8.0` / `0.8.0` / `vela/0.8.0`.
- `default_formula_version() → "v0.8"` (cosmetic; same scoring math).
- `frontiers/bbb-alzheimer.json` and `examples/paper-folder/expected/frontier.json`
  migrated.
- `schema/finding-bundle.v0.8.0.json` published.
- All command banners (`compile`, `bridge`, `jats`, `ingest`,
  `frontier`) → `V0.8.0`.

### Deferred to v0.9+

- Hub-to-hub federation (still needs ≥ 2 hubs).
- Hub-hosted frontier blobs. The locator stays wherever the publisher
  hosts the file.
- Browser-side WebCrypto signing.
- Webhooks / SSE on the hub.
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method.
- Tier-permitted auto-apply for state-changing kinds.
- Per-pubkey rate limits, allowlists, abuse handling.
- Multi-frontier Workbench mode (loading two frontiers into one rete
  view; v0.8 ships dashed-edge-to-ghost-node only).
- A real domain.

## 0.7.0 - 2026-04-25

The public-hub run. v0.6 left the substrate complete and gave us a
local Postgres-backed hub. v0.7 puts the hub on a public URL, opens
the publish path, and stands up the BBB living-repo workflow. "There
is somewhere visible to send a signed manifest" stops being theatre.

### Hub

- **`POST /entries`** on `crates/vela-hub`. Anyone can submit a signed
  manifest; the hub deserializes, calls
  `vela_protocol::registry::verify_entry`, and INSERTs with `ON
  CONFLICT (vfr_id, signature) DO NOTHING`. 201 fresh, 200 duplicate,
  400 tamper or schema mismatch, 500 DB error. Doctrine: the signature
  is the bind, not access control. No allowlist, no rate limit.
- **`UNIQUE (vfr_id, signature)`** on `registry_entries` carries the
  substrate's idempotency guarantee into the transport. Byte-identical
  replays dedupe at the DB layer.
- **Public deploy** at <https://vela-hub.fly.dev>. `crates/vela-hub`
  ships with `Dockerfile` + `fly.toml` + `.dockerignore`. The Fly app
  runs in the `vela-237` org behind a fresh Postgres role with
  `INSERT/SELECT` only on `registry_entries`, distinct from the dev
  sandbox. Production credential lives only in Fly secrets.

### Substrate

- **`registry::publish_remote(entry, hub_url) -> PublishResponse`**
  in `crates/vela-protocol/src/registry.rs`. POSTs canonical bytes via
  `reqwest::blocking`; surfaces `{ok, vfr_id, signed_publish_at,
  duplicate}` from the hub.
- **`vela registry publish --to https://...`** routes through
  `publish_remote`. Local file paths and `file://` URLs keep working
  byte-identically. The signing path (`sign_entry`) is unchanged
  whether the destination is a file or a hub.

### BBB living repo

- **`reviewer:bbb-bot`** registered in `frontiers/bbb-alzheimer.json`
  with `tier=auto-notes`. The bot's private key lives only in the
  `VELA_BBB_BOT_KEY` GitHub Actions secret; the local copy is wiped
  after registration, so rotation requires generating a new key, not
  reading the secret out.
- **`.github/workflows/bbb-living-repo.yml`** (Mondays 14:00 UTC,
  also `workflow_dispatch`). Builds the CLI, signs, POSTs to
  `https://vela-hub.fly.dev`, summarizes `vfr_id` + `snapshot_hash` +
  `event_log_hash` + `signed_publish_at` in the job summary.
  Recompilation lives outside CI (it would need LLM credentials and
  human review); the workflow republishes whatever's in `main` with a
  fresh `signed_publish_at`, which is enough for the "living repo"
  claim.

### Docs

- New [docs/HUB.md](docs/HUB.md): doctrine, endpoints, publish/pull
  recipes, the CI-bot pattern, self-hosting notes, operational
  hygiene around credentials.
- [docs/REGISTRY.md](docs/REGISTRY.md) updated for HTTP push.
- [README.md](README.md) names the public hub URL.
- [scripts/hub-publish.sh](scripts/hub-publish.sh) header reframed as
  optional — direct-DB path remains for backfills, but `vela registry
  publish --to https://<hub>` is preferred.

### Cut

- Workspace + crate versions: `0.6.0 → 0.7.0`.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  → `v0.7.0` / `0.7.0` / `vela/0.7.0`.
- `default_formula_version() → "v0.7"` (cosmetic; same scoring math).
- `frontiers/bbb-alzheimer.json` and
  `examples/paper-folder/expected/frontier.json` migrated.
- `schema/finding-bundle.v0.7.0.json` published.
- Command banners (`compile`, `bridge`, `jats`, `ingest`,
  `frontier`) bumped to `V0.7.0`.

### Deferred to v0.8+

- Cross-frontier links (`vf_…@vfr_…` references) — verifiable
  composition. Defer until v0.7 generates pull pressure for it.
- Hub-to-hub federation. Defer until ≥ 2 hubs exist.
- Hub-hosted frontier blobs. Locator points elsewhere; the hub is
  manifest-only.
- Browser-side WebCrypto signing. Drafts-then-CLI-signs unchanged.
- Webhooks / SSE on the hub.
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method.
- Tier-permitted auto-apply for state-changing kinds.
- Per-pubkey rate limits, allowlists, abuse handling.
- A real domain. The Fly URL is sufficient for v0.7.

## 0.6.0 - 2026-04-25

The trusted-agent run. v0.5 made the substrate writable, reviewable, and
distributable. The Sonnet-vs-Haiku stress test surfaced three concrete
friction items, all driven by real pain. v0.6 fixes them without
re-opening the sprawl problem the v0.3 focusing run closed.

### Substrate

- **Trust-tiered auto-apply** (`sign.rs`, `serve.rs`, `tool_registry.rs`).
  `ActorRecord.tier: Option<String>` registered alongside the pubkey.
  The only tier in v0.6 is `"auto-notes"`. New MCP tool
  `propose_and_apply_note` signs once and applies in one call when the
  actor's tier permits the kind. Doctrine: tiers permit review-context
  kinds only — never state-changing kinds (review, retract, revise,
  caveated). New CLI flag `vela actor add --tier auto-notes`. Halves
  the signing surface for trusted bulk-note extractors.
- **Structured note provenance** (`bundle.rs`, `events.rs`,
  `reducer.rs`, `proposals.rs`). `Annotation.provenance:
  Option<ProvenanceRef>` with `{doi?, pmid?, title?, span?}`. The
  `finding.note` and `finding.caveated` payload schema accepts an
  optional `provenance` object; at least one of doi/pmid/title must be
  set when present. Provenance threads through proposal → applied event
  → materialized annotation, so reviewers can query "show every
  annotation from PMID X" via a typed field rather than parsing prose.
- **Workbench live triage surface**
  (`web/previews/live-frontier.html`, `live-finding.html`,
  `web/scripts/workbench.js`). Two new live pages alongside the static
  brand-canon fixtures: `live-frontier.html` is a live findings table
  with client-side search/filter/scope chips, click-through to live
  detail; `live-finding.html` is a two-column triage view (full
  finding bundle on the left with linked DOI/PMID; queued-review
  sidebar on the right with accept/reject buttons that POST to
  `/api/queue` for `vela queue sign`). The Ed25519 key never enters
  the browser. `proposals.html` proposal-target IDs hyperlink to
  `live-finding.html` for triage navigation.

### Conformance + docs

- New conformance suites: `auto-apply-tier.json` (7 cases pinning the
  tier-gate predicate) and `note-provenance.json` (2 cases pinning
  the canonical preimage shape with/without provenance). Total: 47 → 56
  cases.
- New `docs/TIERS.md` — full tier model, doctrine, idempotency, and
  forward-compat semantics.
- Updated `docs/MCP.md` (tool count 17 → 18), `docs/WORKBENCH.md` (live
  pages as entry surface), `docs/PYTHON.md` (`propose_and_apply_note`
  and `provenance` examples).

### Substrate metadata

- `Cargo.toml` workspace version: 0.5.0 → 0.6.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.6.0.
- `schema/finding-bundle.v0.6.0.json` published.
- `default_formula_version()` → `"v0.6"`.
- `vela --version` → `vela 0.6.0`.
- BBB fixture and paper-folder example migrated.
- All command banners read V0.6.0.

### Deferred to v0.7+ (intentionally)

- Cross-frontier links (`vf_…@vfr_…`).
- Hosted hub at `hub.vela.science`.
- Federation peers / gossip protocol.
- Multi-frontier workspace primitive.
- Hosted Workbench (multi-user, deployed).
- Browser-side WebCrypto signing.
- HTTP / git transports for registries.
- Webhooks (pull/SSE remains sufficient).
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method (entity-overlap routing).
- Tier-permitted auto-apply for state-changing kinds.

The substrate is now strong enough to host these without re-deriving
the protocol. v0.6 leaves the next investment outside the substrate:
make BBB a public living repo, write the canonical essay, find the
first external writer.

## 0.5.0 - 2026-04-25

The accessible-substrate run. v0.4 hardened the kernel; v0.5 makes it
writable from anywhere a writer needs to be — by AI agents through MCP
and HTTP, by human reviewers through a Workbench wired to live state,
and by other Vela instances through a verifiable-distribution registry.

### Substrate

- **Content-addressed proposals + idempotent apply** (`proposals.rs`).
  `created_at` is no longer in the `vpr_…` preimage. Identical logical
  proposals at different timestamps deterministically produce the
  same id. `create_or_apply` is upsert-by-content-address: agent
  retries return the same proposal_id and applied_event_id, with no
  duplicate proposal or event in the frontier.
- **Read-stream API** (`serve.rs`, `tool_registry.rs`).
  `GET /api/events?since=<vev_…>&limit=<n>` and the matching MCP tool
  `list_events_since` give cursor-paginated reads over the canonical
  event log. Same surface serves agent-loop completion signals and
  public-consumer diff watching. No auth on read.
- **Write surface (MCP + HTTP)** (`serve.rs`, `tool_registry.rs`,
  `sign.rs`). Six new tools: `propose_review`, `propose_note`,
  `propose_revise_confidence`, `propose_retract`, `accept_proposal`,
  `reject_proposal`. Each requires a registered actor (Phase M from
  v0.4) and an Ed25519 signature over the canonical preimage.
  `sign::proposal_signing_bytes` and `sign::verify_action_signature`
  reuse the same canonical-JSON discipline as `vev_…`/`vpr_…` derivation.
- **Workbench: drafts + CLI signs** (`web/previews/proposals.html`,
  `crates/vela-protocol/src/queue.rs`, new `vela queue list/sign/clear`).
  `vela serve --workbench` mounts `web/` alongside the API.
  Browser POSTs unsigned decisions to `/api/queue`; `vela queue sign`
  walks the queue, signs with the actor's key, and applies. The
  Ed25519 private key never enters the browser.
- **Registry primitive: verifiable distribution**
  (`crates/vela-protocol/src/registry.rs`, new `vela registry
  add/list/publish/pull`). Flat signed manifests
  `(vfr_id, name, owner, snapshot_hash, event_log_hash, locator,
  timestamp, signature)`. Pull verifies signature plus
  snapshot_hash plus event_log_hash; any mismatch is total
  rejection. Latest-publish-wins. `file://` and bare-path
  transports; HTTP/git deferred to v0.6.

### Adoption surface

- **Python SDK** (`bindings/python/vela/__init__.py`). Single-file
  client for `vela serve --http`. `Frontier.connect()`, `list_findings`,
  `events_since` generator, signed `propose_*` methods,
  `accept`/`reject`. Reuses the canonical-JSON rule in Python so
  `vpr_…` and signature derivation are byte-identical to the Rust
  kernel.
- **Hello-world agent** (`examples/python-agent/extract_and_propose.py`).
  Paper text → optional Anthropic-API claim extraction → propose
  notes against a live frontier → events_since print-out → pointer at
  the Workbench. ~50 lines of agent code.

### Conformance + docs

- New conformance vectors: `tests/conformance/proposal-idempotency.json`,
  `tests/conformance/registry-publish-pull.json`. Total: 47 cases.
- New docs: `docs/MCP.md`, `docs/WORKBENCH.md`, `docs/REGISTRY.md`,
  `docs/PYTHON.md`. Each is the public contract for its v0.5 surface.

### Substrate metadata

- `Cargo.toml` workspace version: 0.4.0 → 0.5.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.5.0.
- `schema/finding-bundle.v0.5.0.json` published.
- `default_formula_version()` → `"v0.5"` (cosmetic; same scoring math).
- `vela --version` → `vela 0.5.0`.
- BBB fixture and paper-folder example migrated to v0.5 schema URLs
  and formula versions.
- All command banners (`compile`, `bridge`, `jats`, `ingest`, `actor`,
  `queue`, `registry`) read V0.5.0.

### Deferred to v0.6 (intentionally)

- Cross-frontier links (`vf_…@vfr_…` references). Composition is a
  separate value prop from distribution.
- Hosted hub (`hub.vela.science`). v0.5's registry is local +
  `file://` URL; managed hub is operational, not protocol.
- Federation peers / gossip protocol. Push/pull only in v0.5.
- Multi-frontier workspace primitive.
- Hosted Workbench (multi-user, deployed). Local-only Workbench in
  v0.5.
- Browser-side signing via WebCrypto. The drafts-then-CLI-signs
  model is the v0.5 doctrine.
- HTTP/git transports for registries.
- Webhooks. Pull/SSE is sufficient for v0.5.

The substrate is now strong enough to host these without re-deriving
the protocol.

## 0.4.0 - 2026-04-25

The substrate-hardening run. v0.3 made the kernel a real protocol; v0.4
makes its load-bearing claims doctrine-grounded rather than convenient.

### Substrate

- **`frontier.created` is a real `events[0]` genesis event**
  (`crates/vela-protocol/src/project.rs`).
  Every freshly compiled frontier emits a canonical event whose
  hash IS the frontier_id. `frontier_id_from_genesis(events)`
  derives `vfr_…` from the same canonical preimage shape as
  `vev_…`, so a second implementation follows one rule. Legacy
  v0.3 frontiers without a genesis event fall back to meta-derivation.
- **Canonical/derived packet split** (`packet.rs`).
  `CANONICAL_PACKET_FILES` (15) carry replay-bearing protocol state;
  `DERIVED_PACKET_ARTIFACTS` (13) ship for inspection but are
  regenerable projections. `proof-trace.checked_artifacts` requires
  canonical only — derived artifacts are validated structurally.
- **Retraction cascade as per-dependent canonical events**
  (`proposals.rs`, `events.rs`, `reducer.rs`).
  A retraction now emits one `finding.dependency_invalidated` event
  per affected dependent in BFS depth order, each carrying
  `upstream_finding_id`, `upstream_event_id`, and `depth`. A pure
  reducer reproduces post-cascade state from the event log alone —
  no hidden propagation in summary fields.
- **Registered actors and signed events under `--strict`**
  (`sign.rs`, `signals.rs`, new `vela actor add/list` CLI).
  `Project.actors` maps stable actor.ids to Ed25519 public keys.
  `--strict` emits `unsigned_registered_actor` and blocks
  strict_check whenever a registered actor writes an event without a
  verifying signature. `event_signing_bytes`, `sign_event`,
  `verify_event_signature` operate on the same canonical preimage
  shape as `vev_…` derivation.
- **Provenance authority unification** (`sources.rs`, `signals.rs`,
  `vela normalize --resync-provenance`).
  `Project.sources` is canonical; `FindingBundle.provenance` is the
  denormalized cache. `--strict` emits `provenance_drift` blockers
  when title/year disagree; `vela normalize --resync-provenance --write`
  rewrites the cache from the canonical SourceRecord.

### Doctrine

- The reducer now treats `frontier.created` as a structural anchor
  and `finding.dependency_invalidated` as a state-mutating event.
- Five new `--strict` doctrine signals: cascade events, registered-actor
  signatures, provenance drift, plus the three v0.3 signals
  (conditions_undeclared, evidence_atom_missing, agent_typed_unreviewed).
- Schema URL bumped from v0.3.0 → v0.4.0; confidence
  `formula_version` defaults to `"v0.4"`.
- `vela --version` reports `vela 0.4.0`.

### Substrate metadata

- `Cargo.toml` workspace version: 0.3.0 → 0.4.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.4.0.
- `schema/finding-bundle.v0.4.0.json` published.
- `frontiers/bbb-alzheimer.json` and the paper-folder fixture
  migrated to v0.4 schema URLs.
- All command banners (`compile`, `bridge`, `jats`, `ingest`, etc.)
  now read V0.4.0.

### Deferred to v0.5 (intentionally)

- `vela event sign` CLI for minting signatures on existing events.
- A registry (`hub.vela.science`) and federation peers.
- Multi-frontier workspace primitive.
- Cross-frontier links and propagation across frontier boundaries.
- Constellation projection product surface.

The substrate is now strong enough to host these without re-deriving
the protocol.

## 0.3.0 - 2026-04-24

The focusing run. v0.3 turns the state kernel into a real protocol — one
that two implementations can independently produce byte-identical IDs
for, that has a separable reducer, that enforces doctrine at the kernel
level, that emits typed events, that carries a stable address primitive,
and that calls itself a coherent version on every surface.

This is the v0 the doctrine has been describing all along.

### Protocol — substrate

- **Canonical JSON hashing** (`crates/vela-protocol/src/canonical.rs`).
  Every content-addressed ID — `vf_…`, `vev_…`, `vpr_…`, snapshot hash,
  event-log hash — now derives from RFC 8785-style canonical JSON
  (lexicographic key ordering at every depth, no whitespace, validated
  finite numbers, UTF-8 strings preserved verbatim). A second
  implementation conforming to the canonical-JSON rule produces
  byte-identical hashes for the same logical content.
- **Pure separable reducer** (`crates/vela-protocol/src/reducer.rs`).
  `apply_event(state, event)` is the deterministic state-transition
  function. The reducer is callable independently of proposal
  construction, so canonical event logs can be replayed from genesis
  by any conforming implementation.
- **Per-kind event payload validation**
  (`events::validate_event_payload`). Each event kind has a normative
  payload schema; payloads that don't match are conformance failures.
  Replay reports surface them as conflicts; `vela check --strict`
  treats them as failures.
- **frontier_id as address primitive**. Every frontier carries a
  `vfr_<hash>` derived from canonical creation metadata. The same
  triple (name, compiled_at, compiler) always produces the same vfr_id.
  Legacy v0.2 frontiers derive on read.

### Protocol — semantics

- **Typed three-state review verdict.** The pre-v0.3 collapse of
  contested / needs_revision / rejected to one bit becomes
  `Flags.review_state: Option<ReviewState>` with explicit variants.
  `flags.contested` is preserved as a derived bit for v0.2 readers.
- **Confidence formula version stamp.** `ConfidenceComponents.formula_version`
  now defaults to `"v0.3"`. A second implementation can refuse to
  interpret components computed under an unknown formula version.

### Doctrine invariants enforced under --strict

- `conditions_undeclared` (line 3): a finding with empty conditions and
  no scope flag (in_vivo / in_vitro / human_data / clinical_trial), and
  not theoretical, blocks strict_check.
- `evidence_atom_missing` (line 4): every active finding must have at
  least one materialized evidence atom. Lifted from packet-validation-
  only into vela check.
- `agent_typed_unreviewed` (line 5): findings with source_type =
  model_output / expert_assertion / agent_trace require explicit
  review or gap-flag before strict acceptance. Doctrine: an agent
  trace is not truth without typed consequence.

### Pruning

- `vela ask`, `vela workspace`, `vela depend`, `vela merge` removed.
  All four were premature consumer or multi-frontier surface that
  v0.3's substrate-first focus rejects. ~2200 LOC excised.
- `flags.gap_info` and the GapStatus / GapPriority / GapNote / GapInfo
  types removed. GitHub-issue-tracker fields on a finding had no
  doctrine motivation. `vela gaps rank` (the doctrine-aligned derived
  ranking) stays.
- `crates/vela-protocol/src/gaps.rs` deleted.

### Substrate metadata

- `Cargo.toml` workspace version: 0.2.0 → 0.3.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`:
  v0.2.0 → v0.3.0.
- `vela --version` reports `vela 0.3.0`; `print_strict_help` masthead
  reads `Vela 0.3.0`.
- `frontiers/bbb-alzheimer.json` and the paper-folder fixture migrated
  to v0.3 schema URLs.
- `schema/finding-bundle.v0.3.0.json` published.

### Deferred to v0.4 (intentionally)

- Cross-frontier links and propagation across frontier boundaries.
- A registry (`hub.vela.science`) and federation peers.
- Multi-frontier workspace primitive.
- A `frontier.created` canonical event in `events[0]` and per-finding
  asserted events from compile (the genesis-event surface is in place
  as `derive_frontier_id_from_meta`; v0.4 promotes it to a proper
  event log entry).
- Identity-bound signatures required under --strict.
- Canonical/derived packet split.
- Provenance-authority unification (sources canonical, finding.provenance
  derived).
- Retraction cascade as per-dependent canonical events.

These are real next-chapter work that's enabled — not blocked — by what
landed in v0.3. The substrate is now strong enough that they can be
built without re-deriving the protocol.

## 0.2.1 - 2026-04-24

This is a design-unification pass on top of the v0.2.0 release shape. No
protocol, schema, or proof-packet format changes. CLI output, docs voice, and
brand surface now share one canon.

### Design canon

- Ships `assets/brand/` with mark, wordmark, favicon, rete motif, and OG image.
- Adds `docs/BRAND.md` as the single reference for voice, color tokens, type
  families, asset usage, and the tick motif.
- Adds `web/` with a GitHub Pages-ready static landing page at `web/index.html`
  using the design-system tokens.
- Stages the proposed post-v0 product surface as static previews under
  `web/previews/` — explicitly labeled as proposals, not shipping v0 product.

### CLI surface

- Rebuilds banners across `compile`, `stats`, `validate`, `depend`, `diff`,
  `tensions`, `serve --check-tools`, `jats`, and conformance output. Every
  banner is now a dim mono eyebrow + tick row, never `===` or `---` separators.
- Reserves signal blue for live state only. Removes `.green()` traffic-light
  coloring from numeric counts and success indicators; maps `PASS/FAIL` and
  success/failure outcomes to engraved state chips (moss / brass / dust /
  madder).
- Gates all ANSI output on `stdout` being a terminal and `NO_COLOR` being
  unset. Piped and `NO_COLOR=1` runs emit no escape codes.
- Retools the `compile` progress bar to a hairline motif (`── ` fill) and `·`
  as the separator.
- Adds `crates/vela-protocol/src/cli_style.rs` as the single routing point for
  palette, chips, eyebrow, tick row, progress-bar style, and error prefix.

### Docs voice

- Rewrites the `README.md` opener as one concrete grounding sentence before
  introducing the core vocabulary. Adds the wordmark header and a footer link
  to `docs/BRAND.md` + the landing page.
- Unifies `belief state` → `frontier state` in `docs/PROTOCOL.md` and
  `docs/CORE_DOCTRINE.md`. `docs/MATH.md` keeps `belief state` with a footnote
  linking theory-side and operational nomenclature.
- Fixes title-case h3s in this `CHANGELOG.md` to sentence case.
- Adds `scripts/voice-check.sh` and wires it into `scripts/release-check.sh`.

## 0.2.0 - 2026-04-23

This is the first strict OSS release candidate for Vela v0.

### Core release shape

- Consolidates the public product around portable frontier state for science.
- Keeps the release workflow focused on `compile`, `check`, `proof`, `serve`, and `bench`.
- Removes tangential UI, runtime, inherited coding-agent, archive, and generated artifact surfaces from the tracked release repo.
- Keeps BBB/Alzheimer as the canonical proof frontier and demo path.

### Protocol and proof

- Uses schema v0.2.0 for the checked-in release frontier.
- Exports deterministic proof packets with `proof-trace.json`.
- Validates proof traces when packet validation sees them.
- Adds canonical release asset packaging for the BBB frontier, proof packet, check report, benchmark report, manifest, and checksums.

### Benchmarking

- Promotes `vela bench` as the public benchmark command.
- Adds default BBB benchmark inputs.
- Adds thresholded pass/fail behavior for finding benchmarks.
- Documents benchmark JSON as a compatibility surface.

### Release operations

- Adds `scripts/release-check.sh` as the local release gate.
- Adds `scripts/package-release-assets.sh` for release assets.
- Adds `scripts/clean-clone-smoke.sh` for fresh-clone verification.
- Updates installer behavior and release workflow packaging.
