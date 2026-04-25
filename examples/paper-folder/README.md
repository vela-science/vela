# Paper folder example

This fixture is the smallest local-corpus path for new Vela users. It mixes
curated findings with text, JATS XML, and a PDF-like source so `vela compile`
can show source coverage, fallback extraction, checking, normalization, proof
export, and agent serving without requiring a large BBB frontier.

```bash
vela compile examples/paper-folder/papers --output /tmp/vela-example-frontier.json
vela check /tmp/vela-example-frontier.json --strict --json
vela normalize /tmp/vela-example-frontier.json --out /tmp/vela-example-frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' /tmp/vela-example-frontier.json)
vela review /tmp/vela-example-frontier.normalized.json "$FINDING_ID" --status contested --reason "Verify source span before reuse" --reviewer reviewer:demo --apply
vela caveat /tmp/vela-example-frontier.normalized.json "$FINDING_ID" --text "Tiny fixture corpus; do not generalize" --author reviewer:demo --apply
vela history /tmp/vela-example-frontier.normalized.json "$FINDING_ID"
vela proof /tmp/vela-example-frontier.normalized.json --out /tmp/vela-example-proof
vela serve /tmp/vela-example-frontier.normalized.json --check-tools
```

Or run the whole fixture path:

```bash
examples/paper-folder/run.sh
```

The expected generated sidecars are:

- `/tmp/compile-report.json`
- `/tmp/quality-table.json`
- `/tmp/frontier-quality.md`

## Send this to a user

Expected runtime is under a few minutes on a normal laptop after the release
binary exists. The useful outputs are the frontier, compile report, quality
table, human quality Markdown, review/caveat events, proof packet, and MCP tool
check.

Inspect first:

1. source accounting in `compile-report.json`
2. weak or low-span findings in `frontier-quality.md`
3. whether `check --strict --json` passes
4. whether `history` shows the review/correction events you added
5. whether agent answers cite `vf_*` IDs and preserve caveats

Known limitations:

- fallback extraction is conservative and incomplete without a model key
- low-text PDFs can produce weak review leads
- candidate gaps, bridges, and tensions are not conclusions
- the fixture is for onboarding, not proof of field-level scientific quality

Outputs are review artifacts. Candidate gaps, bridges, and tensions are leads
for inspection, not scientific conclusions.
