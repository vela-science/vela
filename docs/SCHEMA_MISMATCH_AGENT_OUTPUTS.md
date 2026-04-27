# Schema mismatch: Notes Compiler vs strict validator

**Status:** Known issue, surfaced 2026-04-27 by the v0.30 Phase A
content expansion. Site rendering is unaffected; `vela check` (strict
mode) flags 152 errors across the 38 newly-accepted findings.

## What's happening

`vela compile-notes` emits findings with these enum values:

| Field                              | Agent value                       |
|------------------------------------|-----------------------------------|
| `assertion.type`                   | `tension`                         |
| `evidence.type`                    | `extracted_from_notes`            |
| `provenance.source_type`           | `researcher_notes`                |
| `provenance.extraction.method`     | `notes_compiler_via_claude_cli`   |

The strict validator (in `crates/vela-protocol/src/lint.rs` and `validate.rs`) only accepts:

| Field                              | Allowed values                                                                     |
|------------------------------------|------------------------------------------------------------------------------------|
| `assertion.type`                   | `mechanism`, `therapeutic`, `diagnostic`, `epidemiological`, `observational`, `review`, `methodological`, `computational`, `theoretical`, `negative`, `measurement`, `exclusion` |
| `evidence.type`                    | `experimental`, `observational`, `computational`, `theoretical`, `meta_analysis`, `systematic_review`, `case_report` |
| `provenance.source_type`           | `published_paper`, `preprint`, `clinical_trial`, `lab_notebook`, `model_output`, `expert_assertion`, `database_record`, `data_release` |
| `provenance.extraction.method`     | `llm_extraction`, `manual_curation`, `database_import`, `hybrid`                   |

So every notes-compiler-derived finding fails strict validation on
all four fields.

## Why it's not a data corruption

- The findings render correctly in the site (the site reads JSON
  directly without strict validation).
- The findings are content-addressed; the agent's enum values went
  into the SHA-256 preimage that produced each `vf_<hash>`.
- The accept events in `.vela/events/` reference those hashes. They
  represent a real, signed acceptance.
- `vela stats` reports them in the totals.

## Why the migration can't just rewrite values in place

The id formula:
```
vf_<id> = SHA256(normalize(assertion.text) + "|" + assertion.type + "|" + (DOI || PMID || title))[:16]
```

Changing `assertion.type` from `tension` to `theoretical` would
change every affected `vf_<id>`. That breaks:
- The 38 `accept_proposal` events that reference the old ids
- The reviewer notes that score the old ids
- Any links in the frontier that target them

A schema migration is a v-bump operation, not a hot-fix.

## Two clean fixes (pick one before publishing the frontier to the hub)

### Option A — relax the validator (preferred for v0.30)

Add the four agent-emitted enum values to the validator's allow-list
with semantic equivalence:

| Agent value | Semantic equivalent |
|---|---|
| `assertion.type: tension` | `theoretical` (a theoretical claim about a contradiction) |
| `evidence.type: extracted_from_notes` | `theoretical` (no primary data; expert framing) |
| `provenance.source_type: researcher_notes` | `expert_assertion` |
| `provenance.extraction.method: notes_compiler_via_claude_cli` | `llm_extraction` |

Either:
1. Extend the validator's allow-list to accept the agent values directly, OR
2. Add a `kind=note` flag to evidence and let the validator branch.

Implementation: `crates/vela-protocol/src/lint.rs` — find the four
enum allow-lists and add the agent values. Re-run `vela check` —
should pass strict.

### Option B — fix the agent to emit canonical values

Edit `crates/vela-scientist/src/notes.rs` so it emits canonical
schema values for new proposals. Existing 38 findings remain
"agent-shape" but new compile-notes runs produce canonical state.

Mapping:
- For tensions: `assertion.type = "theoretical"`, set `flags.contested = true`
- For other extractions: `assertion.type = "theoretical"` (or per-finding inferred)
- Always: `evidence.type = "theoretical"`, `source_type = "expert_assertion"`, `extraction.method = "llm_extraction"`

This breaks no existing data but creates a "two regimes" frontier
where pre-fix and post-fix findings differ. Probably not what we
want long-term.

## Recommended action

Pick Option A. It's a single-PR change, preserves all existing
content, and accepts the agent's semantics as valid. The agent
output IS theoretical-evidence expert-assertions extracted by an
LLM — the canonical schema just used different words for the same
thing.

After the fix lands, run `vela check projects/bbb-flagship` to
verify strict-validation passes.

## What it doesn't block

- The site (already rebuilt with all 86 findings rendering).
- The `vela frontier diff` CLI (works regardless).
- The weekly-diff machinery.
- Local `vela stats` and `vela tensions`.

## What it does block

- `vela registry publish` — should not publish to the hub a
  frontier that fails strict validation. Resolving this issue is a
  precondition to the v0.30 hub publish.
