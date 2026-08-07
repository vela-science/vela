# Valid verifier corpus

The tiny synthetic witness in `smoke/` exercises directory discovery and the
`vela reproduce` CLI against known-valid data. It is a test fixture, not a
publication or a source of scientific Standing.

Where the canonical records are is two answers, not one, and the difference is
the epoch boundary.

`vela-science/math` is the one live mathematics authority. It declares the
sources and will hold the Claims, Decisions and witness collections that an
authority admits — and it holds none of them yet: it has accepted no Claim and
retains no artifact. Saying its records live there today would be as wrong as
the sentence this replaces.

`vela-science/erdos-frontier` and `vela-science/sidon-frontier` hold the
epoch-1 records. Both are archived and read-only, preserved exactly as their
signed history, and they are still where those witness collections are. They
are not a live home to send anyone to; re-admitting that state into `math`
through Submission, Verification and Decision is a cross-repository transfer
that has not been done.

Every verifier kind has focused positive unit coverage. CI additionally runs
`vela reproduce` over this smoke corpus. The sibling `../invalid` corpus proves
representative malformed, overstated, and adversarial inputs fail.
