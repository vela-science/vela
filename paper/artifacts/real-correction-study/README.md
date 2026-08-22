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
- `study-contract.json`: separate historical-effect and prospective-action
  semantics, fixed candidate denominator, gates, and stop lines;
- `discrimination-cases.json`: a public non-confirmatory test proving that
  source fact extraction alone cannot choose the authority-sensitive action;
- `source-manifest.json`, `qualification-result.json`, and
  `packet-manifest.json`: the complete material-byte commitment, deterministic
  result, and whole-packet root;
- `review-response.json`: an exact finding-by-finding response to independent
  review commit `2ff39cea36311bab5a36d5c85350fed4d9da1361`;
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

The first command fails unless the regenerated public-discrimination output,
source manifest, qualification result, and packet manifest are byte-exact.
The current immutable roots are read from `qualification-result.json` and
`packet-manifest.json`; they are not duplicated in this README. The result
explicitly sets `confirmatory_freeze_allowed` and
`positive_lift_claim_allowed` to `false`, and `authority_effect` to `none`.

For an external Git reconstruction, supply clean checkouts containing the
bound commits:

```bash
uv run --project conformance --locked python \
  paper/artifacts/real-correction-study/verify.py \
  --source-repo /path/to/formal-conjectures \
  --evidence-repo /path/to/erdos-frontier \
  --hosted-proof-repo /path/to/lean-proofs
```

The retained source files are Apache-2.0 Formal Conjectures bytes extracted
directly from the commits bound in `fixture-qualification.json`. The historical
Erdős 264 packet retains the public evidence-repository binding, declared trust
root, authority sequences 1–5, policy material, exact authorization
commitments, before/after Repository manifests, correction and repair objects,
three local downstream records, and hosted/repaired proof bytes. The verifier
reconstructs signatures, authorization, event-log roots, Repository changes,
and local Standing. This creates no new Decision or authority. The other two
local authority regimes are prospective evaluation contexts, not claims about
the upstream Git repository's scientific authority.
