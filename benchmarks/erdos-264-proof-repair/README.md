# Erdős 264 proof-repair benchmark

This is one direct Harbor task, not a Vela runner. It asks a cold successor to
select the first current Erdős Frontier Target and complete the native Lean
repair opened by the accepted Erdős 264 statement correction.

The episode is intentionally more consequential than the existing JSON
comprehension fixtures:

- the source correction is independently authored and changes mathematical
  meaning from natural-valued to integer-valued perturbations;
- the theorem is tied to the published Kovač–Tao result
  [`arXiv:2406.17593`](https://arxiv.org/abs/2406.17593);
- the retained public proof is no longer compatible without a semantic repair;
- exact success is decided by native Lean in a separate network-denied verifier;
- one arm has ordinary Git/files only and one additionally has the read-only
  Vela CLI; all scientific bytes are identical.

One paired episode can demonstrate or falsify a useful correction-continuation
case. It cannot establish statistical agent lift, new theorem discovery,
external adoption, or a protocol breakthrough.

The frozen plan labels this evidence `real_correction_case`, binds one
scientific-episode root over the publication, independently authored source
transition, accepted correction, and native repair obligation, and reports
exact pass@1 for both matched arms. Repackaging this episode never increases
the sample size.

Materialization fails until the exact correction Claim has an attributed human
Decision and `vela next` exposes `erdos:264:parts-i-proof-repair` first. It does
not perform that Decision.

```bash
python3 benchmarks/erdos-264-proof-repair/materialize.py \
  --frontier ~/personal/erdos-frontier \
  --formal-conjectures /path/to/exact-formal-conjectures-checkout \
  --reference-proof /path/to/exact-lean-proofs-checkout \
  --vela target/debug/vela \
  --vela-linux /path/to/exact-linux-vela \
  --model openai/gpt-5.6 \
  --codex-version <exact-version> \
  --output "${XDG_CACHE_HOME:-$HOME/.cache}/vela/harbor/erdos-264-proof-repair"

harbor run \
  --config "${XDG_CACHE_HOME:-$HOME/.cache}/vela/harbor/erdos-264-proof-repair/harbor-job.json"
```

Do not repeat the same episode to inflate sample size. The matched pair is one
scientific episode and earns only bounded case-study credit.
