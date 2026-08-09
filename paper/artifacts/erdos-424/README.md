# Erdős 424 exact-source verifier

This capsule verifies the exact Formal Conjectures source transition used by
the prospective Erdős 424 correction fixture. It has no authority and makes no
claim about the truth of Erdős problem 424.

Run it against a Git checkout that contains both retained commits:

```bash
python3 paper/artifacts/erdos-424/verify_source_transition.py \
  --source-repo /path/to/formal-conjectures \
  --source-diff /path/to/erdos-frontier/.vela/work/correction-erdos-424/source-diff.json
```

The deterministic JSON report recomputes the two commit trees, two source-file
SHA-256 digests, theorem-line transition, source-diff artifact digest, and
exact Git diff digest. The diff forces full blob identities and disables
external and text-conversion drivers so its bytes do not depend on Git's
repository-specific abbreviation length. A passing report is scoped
mechanical evidence. It is not organizational independence, a proof of the
theorem, or a scientific Decision.

`diff-abbreviation-defect.json` records the first prepared verifier's
pre-import failure and the exact replacement binding. The failed record is
evidence of a verifier defect, not a Verification result.
