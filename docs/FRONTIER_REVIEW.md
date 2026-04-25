# Frontier review

Generated frontier state should be reviewed before scientific use. `compile`
produces candidate state; accepted proposals and canonical events are the v0
trust boundary.

## Review order

1. Open `compile-report.json` and confirm source coverage. Treat compile output
   as candidate state until reviewed.
2. Open `quality-table.json` and sort by low confidence, missing spans, and
   unresolved entities.
3. Confirm source records, evidence atoms, and condition records exist:

```bash
jq '.sources | length, .evidence_atoms | length, .condition_records | length' frontier.json
```

Every active finding should have at least one evidence atom. A weak atom with a
missing locator is acceptable only as a review lead.
Every active finding should also have a condition record so reviewers can see
whether the claim is bounded to exposure, efficacy, species, assay, comparator,
endpoint, and translation scope.
4. Inspect important findings with `vela search` and `get_finding` through MCP.
5. Normalize deterministic source/evidence/condition projections before
   durable writes if the frontier still needs normalization.
6. Record review state with `vela review`, `vela note`, `vela reject`,
   `vela caveat`, or `vela revise`. These create proposal records by default.
7. Add caveats when evidence is abstract-only, model-only, indirect, or weak.
8. Rerun `vela check`, `vela proof`, and any benchmark suite.

## What to fix

- assertion text that overstates the source
- entities with wrong type or spelling
- evidence spans that do not appear in the source
- missing source records or evidence atoms
- evidence atoms without locators when the source has inspectable text, rows, or pages
- missing method, model, species, comparator, translation scope, or condition details
- exposure/efficacy or mouse/human language that overgeneralizes beyond source conditions
- confidence scores unsupported by evidence components
- links that imply stronger dependency than the evidence supports

## Concrete repairs

Use state-transition commands when you want the correction to live with the
frontier history. These commands create `vela.proposal.v0.1` records first.
Use `--apply` when you want to accept and apply the proposal locally in one
step:

```bash
FINDING_ID=$(jq -r '.findings[0].id' frontier.json)

vela review frontier.json "$FINDING_ID" \
  --status contested \
  --reason "Only mouse-model evidence; do not generalize to humans yet." \
  --reviewer reviewer:demo \
  --apply

vela caveat frontier.json "$FINDING_ID" \
  --text "Evidence is indirect and should be treated as a review lead." \
  --author reviewer:demo \
  --apply

vela revise frontier.json "$FINDING_ID" \
  --confidence 0.42 \
  --reason "Confidence lowered after manual review of source scope." \
  --reviewer reviewer:demo \
  --apply

vela history frontier.json "$FINDING_ID"
```

`vela history --json` returns the current finding snapshot plus canonical
finding-local events, proposal records, and compatibility review/confidence
projections. If
`vela check --strict --json` reports replay conflicts, fix the event log before
using the frontier as proof-ready state.

Use CSV curation or source edits when you want to replace or add curated
candidate findings before recompiling. The fixture
`examples/paper-folder/reviewed-findings.csv` shows the expected shape.

Reject unclear finding:

```csv
assertion,type,evidence,confidence,entities,source,span
"Original claim was too vague; keep only the bounded LRP1/RAGE transport claim.",mechanism,curated_review,0.65,"LRP1:protein; RAGE:protein","review override","bounded LRP1/RAGE transport claim"
```

Add caveat:

```csv
"Focused ultrasound BBB opening claims should retain caveats about small local evidence scope.",methodological,curated_review,0.65,"focused ultrasound:method; blood-brain barrier:anatomy","review override","small local evidence scope"
```

Correct entity:

```csv
"LRP1-mediated amyloid-beta clearance should not be collapsed into generic LDL receptor activity.",mechanism,curated_review,0.70,"LRP1:protein; amyloid-beta:protein","review override","LRP1-mediated amyloid-beta clearance"
```

After repair:

```bash
vela compile ./papers --output frontier.json
vela check frontier.json --strict --json
vela normalize frontier.json --out frontier.normalized.json
vela proof frontier.normalized.json --out proof-packet
```

`vela proof` is non-mutating by default. Use `--record-proof-state` only when a
local frontier should remember the latest exported packet state.

Do not normalize after accepted events in v0. `normalize --out` and
`normalize --write` refuse eventful frontiers because normalization is not yet a
canonical event type and can otherwise invalidate replay hashes.

Recompiling can refresh candidate state, but it is not the proof. The proof is
whether accepted corrections are durable, replayable, inspectable, and able to
invalidate stale proof.

## Review standard

The goal is not to make the frontier look clean. The goal is to make uncertainty
visible. Candidate contradictions, gaps, and bridges should stay candidate
surfaces until a human reviewer accepts or corrects them.
