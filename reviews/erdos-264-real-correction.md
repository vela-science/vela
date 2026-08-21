# Independent review: Erdős 264 real-correction artifact

Verdict: **PASS**, limited to producer commit
`c2f8f0eb47d62d232d437825f360c5d94f092c40` and tree
`d8fdb38ad192752a691310ffd13bdaccf2dbfd36` over base
`4685462c44b1f073870f31025ae73d1d8770ce73`.

This review was performed from a fresh isolated clone. The producer ref was
remote-equal to the reviewed commit, the declared base was the merge base, and
the exact range added only these five paths:

- `paper/artifacts/erdos-264-real-correction/README.md`
- `paper/artifacts/erdos-264-real-correction/case.json`
- `paper/artifacts/erdos-264-real-correction/manifest.json`
- `paper/artifacts/erdos-264-real-correction/test_verify.py`
- `paper/artifacts/erdos-264-real-correction/verify.py`

Producer bytes were not modified.

## Artifact identity

The manifest covers every artifact file other than the manifest itself. The
declared byte counts and SHA-256 values reproduced exactly. Canonicalizing the
manifest's `files` object with sorted compact JSON produced artifact root
`sha256:3701349159f61b7bad887e6b066195d5f0a5ad20634a43c9445cf9a7a218a4bf`.
The case root is
`sha256:06236c11c3d26cdd548a67ae58968b97066e554940837fd512f9b1348899a4f3`.

The source-first verifier passed against fresh clones of
`google-deepmind/formal-conjectures` and `vela-science/erdos-frontier`. Its
canonical output SHA-256 was
`196eae9adc20e5a705ca6e55beeb9b1559492947f471a9a932a5c94b8dcc390a`.

## Source reconstruction

The Formal Conjectures objects and ancestry reproduce:

| State | Commit | Tree | Blob | File SHA-256 | Definition SHA-256 |
| --- | --- | --- | --- | --- | --- |
| predecessor | `593e6b76702c5dbffaaa91b59f4faaed705d04ce` | `5e79f7198c3891bdbb3fc6ec10c2b2a804cc56cb` | `8490f7dc0575480c7729acd5713433fc0af9c71b` | `98386d8f28112c5e952ec40c4ee439c27f3ff7560a4e767b493ccebc628fb29f` | `c01f8742a00360a2a36cab0ce0c3be1e62d9539ca88df2d935607ea8492448cb` |
| successor | `0598b8f281060a18416d60753fd75621d659bb07` | `e040cfc1cd6e5d1a79cf156047f452c2268c1920` | `3ff5ce70001355549571a07eee77960939323b57` | `5a3a0fb7063ed77d644a5c1cab503851e68d87b02c0882db8fa52e801aba1166` | `6d8f5197e916b28724e586c8a79bd5e0607748a4bb9c50fccb2625bdc41ff986` |
| repair source | `e6d6b867dc85eec2f88bc47496b4314c623f9f92` | `1e24e996a9fee330dc885ec2b314f60bfd508985` | `2f3a187a7f1a429b78c888eefb86548d80edecc3` | `c59caaa2524e3edd52944e63f5d9bb0614f1bc36d7fb8a0fec7029c14c266b46` | `6d8f5197e916b28724e586c8a79bd5e0607748a4bb9c50fccb2625bdc41ff986` |

The predecessor is an ancestor of the merged successor, and the successor is
an ancestor of the repair source. The exact full-index diff root is
`sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f`.
It changes the perturbation from `Nat -> Nat` to `Nat -> Int`, adds the lower
bound, and applies the required integer coercions.

Independent signature inspection finds exactly five theorem declarations in
the successor file that directly reference `IsIrrationalitySequence`:

1. `Erdos264.erdos_264.parts.i`
2. `Erdos264.erdos_264.parts.ii`
3. `Erdos264.erdos_264.variants.example`
4. `Erdos264.erdos_264.variants.ko_tao_neg`
5. `Erdos264.erdos_264.variants.ko_tao_pos`

The retained repair artifact has root
`sha256:9ba4b0c8aa144985aac8df40ee070c0ffe4ab7b59915d9b44eb90b42f96935e8`.
Against the exact `e6d6b867` source file, only the body of
`Erdos264.erdos_264.parts.i` changes; its signature and every unrelated byte
remain fixed. The corrected definition root remains the successor definition
root. This review did not rerun Lean and makes no new theorem-validity claim;
it verifies the exact retained artifact, capsule, scoped Verification, and
accepted state record.

## Retained Vela evidence and replay

All 16 bound retained paths independently reproduced their declared SHA-256:
the correction artifact, Submission, Verification, Claim, Proposal, two
Decision events, authority envelope; and the repair artifact, verifier capsule,
Verification, Claim, Proposal, two Decision events, and authority envelope.

The correction Decision is authority sequence 4. Its event-log transition is
`sha256:a8cb60fc4a61c0df2b8deee193ce9ae4f2125a106379ca8026ac75ca432d2a78`
to
`sha256:f7e132f4316b01d6abcf5ef30efee72a4a423718c1630853cf00f38fa82d56f2`.
The repair Decision is sequence 5 and starts at that exact event-log root,
ending at
`sha256:efdfcf8388c3a541c2f7227d2b4a9de5ec7e87df39e05ddb69dc4c116d9b1e77`.
The scoped Verification outcomes are both `pass`; the Decision event bindings,
Proposal and Claim identifiers, and repository before/after roots match their
authority records.

The evidence repository declares Vela 0.966.4 as its last compatible reader.
The published macOS archive checksum passed, and its binary SHA-256 was
`b2bcea661adba5b800006a31c2e554b5efc15cffbd96b5dddf6320bd72c58327`.
That exact binary replayed evidence commit
`12fdb0ad09c710469e50a60e8a6e2c81c9d18c3f` / tree
`8b57c21c6c2a1ae279a3171cbad47291ab7af44c` successfully, producing current
archive Repository root
`sha256:f03be3a76ce43be0c2f9ca63ff731b9a5ff5c010b768e95b46a35f3a067eed96`.
`vela why` reports both exact Claims accepted. The repair Claim's Vela
`relations` array is empty.

## Claim ceiling

The evidence supports only the following bounded statement: one exact merged
source-definition correction has five direct same-file theorem consumers; the
retained Vela history contains a scoped Verification and attributed accepted
correction Decision; and the next authority sequence accepts one exact
`parts.i` proof-repair artifact bound to the corrected definition.

It does **not** establish five Vela `depends` edges, a support diamond, complete
propagation, general scientific lift, lower continuation cost, proof of all of
Erdős 264 or the other four consumers, external independence, adoption, or
scientific truth beyond the exact retained Claim. The paper artifact itself is
non-authoritative, changes no Standing, and performs no Decision.

## Checks

- exact remote commit/tree/base/range reconciliation: PASS
- manifest byte counts, file hashes, artifact root, and case root: PASS
- `git diff --check` for the reviewed range: PASS
- five verifier unit tests: PASS
- Ruff 0.16.2 locked check and format check for both Python files: PASS
- full source-first verifier against fresh external clones: PASS
- independent source ancestry/tree/blob/file/definition/diff reconstruction: PASS
- all 16 retained external byte bindings: PASS
- archive-compatible Vela replay and both Standing explanations: PASS
- authority effect: none
- Standing effect: none

No producer byte, source repository, Vela authority, Standing, or scientific
state was modified. No inference, outreach, merge, or external authority action
occurred.
