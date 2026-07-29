# Vela paper and artifact

`vela.md` is a working systems-paper draft governed by
[`docs/WHITEPAPER_CONTRACT.md`](../docs/WHITEPAPER_CONTRACT.md). It is not a
protocol-breakthrough claim.

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
```

The build refuses a dirty Vela worktree or any external commit, tree, or
content-root mismatch. Tar order, ownership, permissions, and timestamps are
normalized so repeated builds from the same inputs produce the same archive
root.
