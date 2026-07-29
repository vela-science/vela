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

`result.v1.json` records the registered observation at raw byte root
`sha256:1ba33ce4387c624c7c0381091140db34bb7ff4bf933ce56d0abe5479cf495acd`.
The run began after Proposal `vpr_23f32f95d4f073e8` reached terminal Standing.
It retained seven warm-cache samples for each operation.

`reproduction.v1.json` records a second same-machine execution from detached
clean clones at raw byte root
`sha256:8ee2588e3745324555862a14a7559d2374984661aa5ce783d6ed7c400b02599b`.
The two executions match on the plan, binary, Frontier commits and trees,
repository roots, counts, tracked inventory, normalized operation outputs,
and limits. Their shared deterministic projection root is
`sha256:f30d4c3464618e0159603ae8adaf58eb7addd63a4ce00f7a1d3fec18d2f85bd3`.
The second execution tests local reproducibility. It provides no independent
or cross-machine performance evidence.

Reproduce the observation from the exact retained commits:

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
`test_measure.py` checks the registered result, all retained samples, summary
statistics, and deterministic equality with the second execution.
