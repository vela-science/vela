# Expected output

Running the example corpus should produce:

- a frontier with at least 6 findings
- a `compile-report.json` with accepted CSV, text, JATS, and PDF sources
- a `quality-table.json` with one row per finding
- a `frontier-quality.md` human review queue
- a proof packet that passes `vela packet validate`
- a `serve --check-tools` report with all checked tools passing

Exact finding IDs may change if fixture text is intentionally edited. Benchmark
fixtures should be updated only when the scientific assertion text changes.
