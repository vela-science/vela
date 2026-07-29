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
