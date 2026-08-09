# Removability and hosted-service-loss check

This experiment asks whether canonical Erdős replay and proposal inspection
still work when optional producers, websites, read databases, hosted APIs, the
original agent session, and repository-authority credentials are absent.

The plan is frozen before execution in `plan.json`. The test runs the pinned
Vela binary against a fresh exact clone with an empty home directory and
network access denied. It is first-party removability evidence for benchmark
families B5 and B6 only.

After acquiring the exact Git clone, run:

```bash
python3 paper/artifacts/removability/run.py \
  --clone /path/to/erdos-frontier-at-the-frozen-commit \
  --vela /path/to/the-pinned-vela-binary \
  --output paper/artifacts/removability/observed
```

The harness parses the exact JSON output of `check`, `status`, and
`review show`, replaces only the machine-specific absolute clone path with
`<frontier>`, canonically encodes that projection, and roots the retained
bytes in `observed/result.json`. Scientific identifiers, counts, and roots are
not normalized.
