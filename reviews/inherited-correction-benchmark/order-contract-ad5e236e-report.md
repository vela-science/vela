# Independent stopped-state and prospective order-contract review

## Verdict

**BLOCKED**, bound to prospective successor commit
`ad5e236e3f29c347c4e3510f4585ec865ae6ef3f`, tree
`c60c3bc47b4b1c4f3b020255fbfefeda9fbdcb33`, exact parent stopped-state
commit `f14616e341929e7ad74927a846cba12e5889154e`, parent tree
`e66de9ef11a17f13856323ae955289c452c37995`, and live remote branch
`refs/heads/codex/inherited-correction-study` at the successor commit.

The order-contract implementation, stopped custody, regenerated successor,
and held-state gates pass independently. The committed successor nevertheless
has one blocking reproducibility defect: its own deterministic-check commands
verify the stopped predecessor artifact, not the prospective successor.

This review made no provider call, released no permit, accessed no protected
adjudication plaintext or key, performed no scoring, and authorizes no
participant or calibration launch, merge, Core or Protocol change, Repository
authority action, Standing change, or Decision effect.

## Blocking finding

`paper/artifacts/inherited-correction-held-out-order-replacement/README.md`
line 135 points the offline preflight at
`inherited-correction-held-out-replacement/calibration/input`; lines 137-140
invoke the predecessor's benchmark, custody verifier, benchmark tests, and
runtime tests. All five paths omit `-order-`.

This is not cosmetic. Running the documented verifier and custody commands
returns the stopped predecessor roots
`sha256:820b725d04cd3780e4bbdb6a89f3ee980a5bf993259c1f089984a3e7f7407f2b`
and
`sha256:cf69793d6ed3489b17690088e8f004d95b04859ec60d5aa5cf7e558cbb012b80`,
and reports 36 held / zero consumed from the predecessor templates. It never
checks the successor registration
`sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`,
assignment
`sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`,
new IDs, new permits, or order-invariant validator. It can therefore print a
green held receipt while bypassing every prospective repair surface and while
the separately frozen predecessor stop correctly records one consumed permit
and 35 held/unissued permits.

The repair is narrow: bind the successor's documented preflight and all four
Python commands to
`paper/artifacts/inherited-correction-held-out-order-replacement`, regenerate
the artifact manifest/root, and repeat commit-bound review. No permit should be
released before that correction passes independently.

## Exact stopped-state custody

The predecessor stop is exact and immutable. Its seven runtime-capture entries
recompute to
`sha256:d3b484af62b5c3b5f69b2547fbf305d604d451ac2d5c96eeefeb8d7fbd62e230`.
Every recorded byte count and digest matches the committed file.

The sole retained participant response has exact byte digest
`sha256:3a3340575b3ca2a66535a5b24476c0e6e6ba7e29c5462a6b2c0d1d542a3bb457`.
The consumed permit identity matches the frozen `schemafix-run-01` permit after
excluding only mutable issuance fields; the launch binds its exact bytes and
consumption precedes provider start. The four-event stream binds the exact raw
response, stderr is empty, and the receipt records one turn, zero tools and
compactions, no timeout, and no retained credential.

The response passes the registered schema. Its observed packet order is
`north-estimate`, `regional-sensitivity`, `archive-accession`,
`combined-estimate`; the predecessor custody validator reproducibly rejects it
only with `response_claim_order_invalid`. The stopped record retains it as the
sole terminal non-result with no ingestion, retry, substitution, score, or
replacement credit. Exactly one predecessor permit is consumed, the remaining
35 assigned runs are held/unissued, and continuation through
`schemafix-run-02..schemafix-run-36` is explicitly forbidden.

## Prospective order contract

When invoked directly at the correct successor path, the repair behaves as
specified. Packet, reverse, and fixed-random consequence order all validate to
the same canonical derived record. The validator leaves the caller's response
object unchanged, and custody copies the exact raw response bytes into both the
runtime evidence and retained response record.

Acceptance is limited to exactly four unique expected claim IDs. Consequence
objects and evidence-binding objects have exact closed fields, and the four
expected packet path/digest pairs must be present exactly once. Independent
mutations for a missing claim, duplicate claim, unknown claim, extra field,
wrong evidence digest, duplicate evidence binding, and retained raw-response
byte drift all fail closed.

The original stopped response is accepted by the successor validator without
changing its raw bytes. Only the returned derived consequence and evidence
arrays are ordered by claim ID for downstream deterministic validation and
scoring.

## Successor roots and deterministic regeneration

The committed generator and all generated entries verify at the correct path.
The requested roots recompute exactly:

- artifact: `sha256:feb41e79aafacdd6534cd8c8ec347420d47006c5a7a2d711f5e1daa7c9c73899`;
- registration: `sha256:60acdfa31d25f9df5f342b75caf8e65426c5b71fa320c36fe5568de9fbf13b10`;
- assignment: `sha256:64a356db4800b6fb04090ae81a6c2d33bf37ad8b71e92e01567edc5fa6362e72`;
- prelaunch: `sha256:22c33b1fdb1030774d09924cfd6d0236270f1536ad96769c4dc80937c55315d3`;
- permit set: `sha256:24909f426fcc7e917f6f45072f558806fe7a12d3853b1ef8e6a5c86ab6b50d45`.

Fresh isolated copies regenerated with CPython 3.10, 3.11, 3.12, 3.13, and
3.14 produce the same complete file-set digest
`1d46abe081fd64191cda950d78658822d045a9d210956437d5f7d9e6d51ac1b8`.
Each regenerated copy passes its own verifier.

The independently qualified runtime remains exact: image manifest
`sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1`,
image config
`sha256:0b41c9eb78b4afcd34b8e6c8c3bf85d81eda431fa4f7f99445c6d951eaa49348`,
complete OCI tar
`sha256:87a1b1d80a27dbc92a0fd5dd69543c4c55386d3cfef77e7c76dab37d2c905183`,
and runtime root
`sha256:3f7a753141306771b05c582d1c0ff30489cdb8a35c556e21ac5fdabb9a431ba8`.
A network-none, read-only preflight in that image passes both schema surfaces
with empty events and stderr.

## Fresh identities, held state, and unchanged bytes

The successor seed differs from the stopped registration. Its 36 run IDs and
36 participant-instance IDs are unique and disjoint from the predecessor.
Exactly 36 `orderfix` participant permits are held with
`expires_at=not_authorized`; none is consumed. One distinct
`neutral-orderfix-calibration-01` permit is separately held and unconsumed.
The successor is `not_run` at 0/36 with zero provider calls, protected-key
accesses, or scoring runs.

Packet trees, all participant prompts, registered and provider schemas,
families/source facts, task bytes, scoring gates, participant model/runtime
configuration, runtime binding, and protected adjudication commitment are
byte-identical to the predecessor. Existing stopped execution, prior
calibration evidence, original held study/evidence, and runtime source trees
have identical Git object IDs in predecessor and successor. The successor diff
adds only the prospective artifact directory; Core, Protocol, Standing,
authority, and scientific bytes outside it are unchanged.

## Focused checks

At the correct successor path, the benchmark verifier, custody prelaunch
verifier, all 33 focused Python tests, Ruff, network-none image preflight,
`git diff --check`, independent stopped-custody recomputation, order
permutations/adversaries, and five-version byte-stable regeneration pass.

These passing implementation gates do not cure the blocker: the immutable
producer artifact documents a different set of commands that successfully
checks the wrong registration.

## Residual boundary

This BLOCKED verdict is not a scientific result and does not invalidate or
resume the stopped predecessor. The predecessor remains permanently stopped,
and the prospective successor remains held. A corrected immutable successor
requires fresh root binding and independent review before calibration or
participant release.
