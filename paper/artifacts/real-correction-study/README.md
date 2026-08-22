# Real consequential-correction study design

This packet is the highest-leverage successor to the completed negative
synthetic study: it moves method development onto exact real source
corrections while refusing to preregister another ceiling task.

The packet contains:

- `failure-audit.{json,md}`: source-bound reconstruction of the single Vela
  miss without rerunning, rescoring, or opening protected material;
- `fixture-qualification.json` and `fixtures/`: three open real corrections,
  exact predecessor/successor bytes, bounded consequence scopes, and authority
  evidence or explicitly prospective local scenarios;
- `arm-contract.json`: the identical-atoms three-arm contract;
- `discrimination-cases.json`: a public non-confirmatory test proving that
  source fact extraction alone cannot choose the authority-sensitive action;
- `estimands-and-gates.md`, `falsification-conditions.md`, and
  `claim-matrix.md`: paper-ready causal and claim boundaries; and
- `roadmap.md`: the stop/go path to a fresh held-out confirmatory program.

Run the deterministic qualification:

```bash
uv run --project conformance --locked python \
  paper/artifacts/real-correction-study/verify.py

uv run --project conformance --locked python -m unittest discover \
  -s paper/artifacts/real-correction-study -p 'test_*.py'
```

The expected result is `qualification-result.json`, root
`sha256:4f2fee9dbb0f62550873daab4911564c4968ad9b84fe40ef3af83c6355b7832c`.
It explicitly sets `confirmatory_freeze_allowed` to `false` and
`authority_effect` to `none`.

The retained source files are Apache-2.0 Formal Conjectures bytes extracted
directly from the commits bound in `fixture-qualification.json`. The historical
Erdős 264 Vela records are first-party public evidence. The other two local
authority regimes are prospective evaluation contexts, not claims about the
upstream Git repository's scientific authority.
