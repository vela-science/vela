# Foreign-transfer contract audit

Benchmark family B8 requires a second Frontier to retain and check exact
foreign state without importing the source Frontier's authority.

The frozen source audit in `plan.v1.json` and `result.v1.json` established that
the current protocol has no such transfer contract. The derived
`vela.foreign-reference.v1` experiment is deliberately confined to
`vela-edge`; it is not a protocol object, writer command, resolver, Registry,
or federation service.

`erdos-424/` is the single real qualification package. It binds:

- the current compacted Erdős repository and its predecessor transition;
- the repository origin connecting those states;
- the exact accepted Claim, Submission, Proposal, and Verification;
- the applied and Decision Events;
- the signed repository-authority record and exact keyset; and
- an explicit local Standing effect of `none`.

The Rust reader and dependency-free Python reader both rederive object
identities and verify the complete semantic and signature chain. Their shared
assessment is rooted at
`sha256:b7b330ae6ea4915d5bac218233f0a272ee961060682be6d22f6a8ea1b78c4ed6`.
This qualifies the source package and the two implementations. It does not
complete B8 until a second Frontier retains and verifies the package through
its ordinary non-authoritative producer path.

Rebuild and verify the source package:

```bash
python3 paper/artifacts/transfer/materialize_real_reference.py \
  --source ../erdos-frontier \
  --output /tmp/erdos-424
python3 conformance/verify_foreign_reference.py \
  --package-root /tmp/erdos-424
```

Create and verify a deterministic portable archive:

```bash
python3 paper/artifacts/transfer/pack_reference.py \
  --package-root paper/artifacts/transfer/erdos-424 \
  --output /tmp/erdos-424-reference.tar.gz
python3 paper/artifacts/transfer/verify_archive.py \
  --archive /tmp/erdos-424-reference.tar.gz \
  --expected-root \
  sha256:b7b330ae6ea4915d5bac218233f0a272ee961060682be6d22f6a8ea1b78c4ed6
```

The landscape decision remains intentionally narrow: Vela is scientific state
and inheritance infrastructure. A reusable package, read-only index, Registry,
or Atlas must be earned in that order by recurring external use, net deletion,
correction-aware transfer, and measured cold-user lift.
