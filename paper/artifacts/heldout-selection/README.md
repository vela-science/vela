# Held-out correction selection

`plan.v1.json` freezes the selection rule after both correction-impact readers
exist and before a held-out scientific case is known.

Canonical plan root:
`sha256:b9dbf4b86b841b7b09a79e865ae0187a3ed6dcead896cc2446edcacb836af6a8`.

The rule scans accepted correction transitions after four exact Frontier
baselines and chooses the first candidate in canonical Decision order that
satisfies every scientific, topology, identity, and removability condition.
If no candidate qualifies, the benchmark records a failed held-out entry gate.
It does not substitute a synthetic or preferred case.

After selection, a separate root-linked amendment must freeze the expected
projection before either reader runs on the case.

`audit.py` executes that rule against clean, exact local clones. It scans
first-parent accepted-state changes from each frozen baseline through the
current head, records any compaction predecessor, and checks every accepted
correction candidate for the required hard dependent, support diamond, and
non-consequential relation.

```bash
python3 paper/artifacts/heldout-selection/audit.py \
  --repos-root ~/personal
```

The first execution found one accepted correction transition: the already
declared Erdős 424 writer-qualification case. It is ineligible because it
overlaps that fixture and has no hard dependent, support diamond, or
non-consequential incoming relation. The other three Frontiers contain no
accepted correction transition after their frozen baselines.

The canonical result is `result.v1.json`, byte root
`sha256:f80cf6b81c9b056535ccf17a24b1631d8f3e57d3bc3ecea65d7516c1b831be5b`.
Its outcome is `no_qualifying_candidate`, which is the preregistered failed
held-out entry gate. No synthetic or preferred case replaces it.

The 2026-08-01 current-head rerun is retained separately as
`result.2026-08-01.json`, byte root
`sha256:c6462c22e049e2fc392ec129769bb8230f95b14f74eabf4714c3654366e555a9`.
It adds repository-v4 reader support and scans Erdős `da791f88`, Formal
`6cbc2cb5`, Sidon `e07b6317`, and Quantum `29202cfb`. It reaches the same
`no_qualifying_candidate` outcome: the only candidate remains the excluded
Erdős 424 qualification case and still lacks the required consequence
topology. The original frozen result remains unchanged.
