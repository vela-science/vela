# Valid verifier corpus

The tiny synthetic witness in `smoke/` exercises directory discovery and the
`vela reproduce` CLI against known-valid data. It is a test fixture, not a
Frontier, publication, or source of scientific Standing.

The canonical scientific records and complete witness collections live in
their independently governed Frontier repositories:

- `vela-science/erdos-frontier`
- `vela-science/sidon-frontier`

Every verifier kind has focused positive unit coverage. CI additionally runs
`vela reproduce` over this smoke corpus. The sibling `../invalid` corpus proves
representative malformed, overstated, and adversarial inputs fail.
