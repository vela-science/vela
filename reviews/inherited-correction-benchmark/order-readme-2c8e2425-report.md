# Independent order-successor self-verification re-review

## Verdict

**PASS**, bound to producer repair commit
`2c8e2425f17aa6488f59d23c48da5f852cab34b4`, tree
`0bb4b9d349e4d6c6fcf79c9a81fc25b9e289b769`, exact parent successor
`ad5e236e3f29c347c4e3510f4585ec865ae6ef3f`, and live remote branch
`refs/heads/codex/inherited-correction-study` at the repair commit.

The sole blocker recorded by independent review
`7a17e333b1a45965183a94a9b534f07c9d5949b7` is repaired. The successor's
documented preflight, benchmark verifier, custody verifier, benchmark tests,
and runtime tests now all target
`paper/artifacts/inherited-correction-held-out-order-replacement` rather than
the stopped predecessor artifact.

This review made no provider call, released no participant or calibration
permit, accessed no protected adjudication plaintext or key, performed no
scoring, and authorizes no participant or calibration launch, merge, Core or
Protocol change, Repository authority action, Standing change, or Decision
effect.

## Commit and semantic scope

The review used an isolated clean clone and refreshed the live remote. Commit,
tree, parent, and remote ref match exactly.

The producer changes only the successor `README.md` and generated
`manifest.json`. The authored semantic diff is exactly five executable path
substitutions:

1. the network-none provider-schema preflight input;
2. `benchmark.py verify`;
3. `custody.py verify-prelaunch`;
4. `test_benchmark.py`;
5. `test_provider_schema_runtime.py`.

Each substitution adds only `-order-` to select the prospective successor.
The manifest changes only its artifact root and the README entry's byte count
and digest. Every other manifest entry is identical to the parent.

## Stale-path search

A full search of the successor artifact finds no remaining executable path to
`paper/artifacts/inherited-correction-held-out-replacement`. The retained
`schemafix-run-01`, stopped-registration, and stop-evidence references occur
only in prospective history and custody bindings. They intentionally identify
the immutable predecessor failure and are not executable predecessor paths.

## Exact documented commands

Every corrected README command was run literally from the fresh checkout.

The network-none, read-only preflight used the successor calibration input and
reported both valid-response checks true, provider-only duplicate acceptance,
registered-schema duplicate rejection, and no provider-contact possibility.
Its provider-events and stderr files were both empty.

The corrected successor benchmark verifier reported verified and held. The
corrected custody verifier reported:

- registration:
  `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`;
- assignment:
  `sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`;
- participant permits: 36;
- participant permits consumed: zero;
- state: `verified_hold`.

The corrected benchmark test command ran 24 tests, including the order-contract
packet/reverse/fixed-random equivalence and closed-set failure adversaries. The
corrected runtime test command ran nine tests. Both pass. The documented
`git diff --check` also passes.

These outputs bind the successor rather than reproducing the predecessor roots
that caused the blocked verdict.

## Deterministic manifest and regeneration

All 238 manifest entry byte counts and digests recompute. Canonicalizing the
manifest without its self-field gives the requested repaired artifact root:

`sha256:78a107fa77819075467790fae87870acfba1b04dd2fed5b889f4d304c13a0c9a`.

Fresh isolated copies regenerated with CPython 3.10, 3.11, 3.12, 3.13, and
3.14 produce that same artifact root and the same complete file-set digest:

`sha256:f9f65762471f102fc6bc3aefce86ad582abe2103c7be69dee8659872a9792405`.

The successor registration, assignment, prelaunch, permit-set, runtime, image,
packets, prompts, schemas, order contract, stopped evidence, calibration
evidence, scientific bytes, protected commitment, and scoring gates are
byte-identical to the parent. Only the README and its deterministic manifest
binding changed.

## Held successor state

The successor remains `not_run` at 0/36. Exactly 36 fresh participant permits
are held with `expires_at=not_authorized`; none is consumed. The distinct
`neutral-orderfix-calibration-01` permit is also held and unconsumed. The
permit-set root remains
`sha256:24909f426fcc7e917f6f45072f558806fe7a12d3853b1ef8e6a5c86ab6b50d45`.
Recorded provider calls, protected-key accesses, and scoring runs are zero.

## Focused checks

The five exact corrected README commands, all 33 successor tests, two named
order-contract tests, Ruff, network-none schema preflight, manifest/root
recomputation, five-version regeneration, stale executable-path search, and
`git diff --check` pass.

## Residual boundary

This PASS confirms only the README/self-verification repair at the exact
producer commit. The successor remains held. It does not release or authorize
the distinct neutral calibration permit or any participant permit.
