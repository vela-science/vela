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

Formal Conjectures retained the exact archive under corrected Proposal
`vpr_7aba66544ffefd99`. A credential-free import preflight then verified the
signed receiver Verification, its actor, Proposal, Submission, Claim,
Artifacts, current repository, authority history, trust anchor, Cedar
authorization, and canonical transaction sets. With `SSH_AUTH_SOCK`
deliberately absent, execution stopped when the already-authorized transaction
needed its signature and left Git, the worktree, and every operation journal
byte-identical. The rooted observation is
`erdos-424/receiver-preflight.v1.json`, byte root
`sha256:00b135a27088af1049ffe86cc329a5bec10fde098e32ac8342900c84a8a95c09`.
This proves readiness and fail-closed behavior, not receiver publication.

Receiver publication is recorded separately in
`erdos-424/receiver-publication.v1.json`, byte root
`sha256:a5867554d4dc9ea4dcd6d415a2be263c84dc0f6fbbe497fb86b427104368d75c`.
Formal Conjectures imported scoped Verification `vvr_ebc29eae4f5f4edf`,
pushed commit `3fe6bf62afd587b9cdeac39f5eb3c62a28fbc0aa`, and reproduced repository
root
`sha256:5e59e05a5639ac0ec4331ec40fec9f50229b795a1a08d983ba96834d4777b58a`
from a clean clone. The Proposal remains `pending_review`, no Decision exists,
and accepted-event delta is zero. This completes B8 without importing source
authority. It does not establish external independence or measured lift.

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
