# Vela paper and artifact

`vela.md` is a working systems-paper draft governed by
[`docs/WHITEPAPER_CONTRACT.md`](../docs/WHITEPAPER_CONTRACT.md). It is not a
protocol-breakthrough claim.

Render a working PDF with the exact local Pandoc and pdfLaTeX versions checked
by the renderer:

```bash
python3 paper/render.py
```

The renderer refuses a dirty worktree, sets `SOURCE_DATE_EPOCH` from the
current commit, and lets long roots wrap without changing their source bytes.
Generated PDFs live under ignored `output/`; a release artifact packages the
qualified PDF and its render result separately from the source commit.

The deterministic source-only artifact includes the exact tracked Vela source,
the retained Erdős 424 source-diff Artifact, and both exact Formal Conjectures
source versions:

```bash
python3 paper/artifact.py build \
  --vela . \
  --erdos-frontier ../erdos-frontier \
  --formal-conjectures ../formal-conjectures \
  --output /tmp/vela-paper-artifact.tar.gz

python3 paper/artifact.py verify /tmp/vela-paper-artifact.tar.gz

python3 -m unittest discover \
  -s paper/artifacts/erdos-424 -p 'test_*.py'
python3 -m unittest discover \
  -s paper/artifacts/formal-505 -p 'test_*.py'
```

The build refuses a dirty Vela worktree or any external commit, tree, or
content-root mismatch. Tar order, ownership, permissions, and timestamps are
normalized so repeated builds from the same inputs produce the same archive
root. The compact state-lift result preserves the historical negative result
and its exact input and output roots; its retired custom scorer is available in
Git history rather than maintained as a second harness. Prospective execution
evaluation uses native Harbor through
[`benchmarks/product-compression`](https://github.com/vela-science/vela/blob/e68590415a0cc40ee489801f1f281dc8c5996337/benchmarks/product-compression/README.md),
retired from the tree and read at `e6859041`, the last commit that carried it.
