# Protocol cost observation

`plan.v1.json` freezes the first descriptive cost measurement before the
terminal Erdős 424 Decision. `measure.py` runs read-only commands with an empty
home and no credential environment, rejects dirty Frontiers, records every
wall-time sample, and binds each normalized output by SHA-256.

The measurement covers:

- `status` and strict repository verification on all four mathematical
  Frontiers;
- proposal inspection and frozen-witness replay on Erdős; and
- tracked working-tree file and byte counts.

It excludes clone, network, compilation, model execution, human review, Git
object compression, and hosted services. The result is a local cost
observation, not a performance claim.

Run only after Proposal `vpr_23f32f95d4f073e8` reaches terminal Standing:

```bash
python3 paper/artifacts/cost/measure.py \
  --vela <immutable-vela-binary> \
  --frontier erdos=<erdos-frontier> \
  --frontier formal=<formal-conjectures-frontier> \
  --frontier sidon=<sidon-frontier> \
  --frontier quantum=<quantum-codes-frontier> \
  --output <new-result.json>
```

The script performs no write, signing, Verification, or Decision operation.
