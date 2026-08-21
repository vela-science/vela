# Independent held-out F08/G08 runtime reproducibility repair review

## Verdict

**PASS**, bound to producer commit
`bccae8ff047d20797347361725b8510c08e56960`, tree
`84124655704f474f199c16794ae005d85b84cbe8`, parent
`3f4d61c248a43a94d09e0d848693101dd1841aa0`, and remote branch
`refs/heads/codex/inherited-correction-study` at that exact commit.

Finding **F08** is closed and gate **G08 deterministic runtime custody** passes.
Two newly created independent `docker-container` builders with empty caches
performed Linux arm64 builds from the exact detached producer source, with
`--no-cache`, `--provenance=false`, `--pull=false`,
`SOURCE_DATE_EPOCH=1757289600`, and OCI `rewrite-timestamp=true`. The complete
OCI archives were byte-identical and matched every frozen runtime identity:

- image and manifest: `sha256:4799bee82c708fb68006b9c558a0fc345a0e7d1f2936fcad298a3d775e0d08bb`;
- image config: `sha256:c0868b563acbd5dd6c80d9512cc1bf019598d3579026fc7fed465cef2d60e56c`;
- complete OCI tar: `sha256:b48f80b60ad1407824e8cd9e1fd3c60abcfd486ed672f8d31ddc9918a28b0b74`.

This review qualifies the repaired held bytes only. It releases no permit,
makes no provider call, accesses no protected adjudication plaintext or key,
performs no experimental scoring, and authorizes no participant launch,
merge, Core or Protocol change, Repository authority action, Standing change,
or Decision effect.

## Independent reproduction

The review used a fresh clone of `https://github.com/vela-science/vela.git`,
checked out detached at the producer commit. The producer commit, tree, parent,
remote ref, and clean checkout all matched exactly. The original producer
worktree remained detached, clean, and unmodified.

Both clean builders ran BuildKit `v0.32.2`. Each independently fetched the
pinned base image and built without cache. The two resulting OCI tar files
matched under `cmp`, and their SHA-256 digests matched the frozen amendment.
Independent archive inspection also verified:

- the index contains exactly one manifest;
- the manifest bytes hash to the pinned image identity;
- the config descriptor bytes and every layer descriptor hash and size match;
- both archives contain the same complete sorted entry set;
- rewritten build-created timestamps are capped at the frozen source epoch;
  and
- mutation, wrong-identity, malformed-identity, relative-output, and overwrite
  adversaries fail closed.

The first archive was loaded locally by immutable image identity. The
provider-schema preflight then ran with container network `none`. It accepted
the neutral valid response and the provider-compatible duplicate response,
while the unchanged registered schema rejected the duplicate. Both stderr and
`provider-events.jsonl` remained empty, so no provider contact occurred.

## Scope and immutable scientific bytes

The 72-path producer diff is confined to the provider-schema-v2 runtime repair,
its reproducibility contracts and tests, the transparent F08 amendment, and
the held roots and permits transitively derived from the repaired image. The
runtime behavior source, pinned packages, model, one-turn semantics, timeout,
token ceiling, tool boundary, attempt, retry policy, and trust bundle are
unchanged.

Independent byte comparisons established that:

- both stopped-study subtrees are identical to the blocked parent; the stopped
  record remains exactly 1/36, with run 01 retained as a provider-schema
  non-result and runs 02-36 unissued;
- all replacement packet files, participant prompts, and registered response
  schemas match the stopped study byte-for-byte;
- `DESIGN.md`, `TASK.md`, `families-source.json`, the assignment seed,
  adjudication commitment, and launch authorization amendment are unchanged;
- the registered Draft 2020-12 schema remains
  `sha256:ac96be686e749792956dfa1dfe9560f85c53d55c27fe2e8fd32bcc2a96a634ba`;
- the provider derivative remains
  `sha256:896f242086805d3b51e81ed04e6d50f33eb2b7deb71b7a1689e9abeba3b67eaf`
  and differs only by deleting
  `/properties/evidence_bindings/uniqueItems`; and
- the full scoring design, thresholds, family/arm balance, participant IDs,
  packet roots, prompt roots, and scientific source/evidence bytes are
  unchanged.

## Recomputed bindings and held state

All roots were independently recomputed from committed bytes:

- registration: `sha256:4d078776f41ffd1df18768791de58014117319f19727c3b9671b606e527c1276`;
- assignment: `sha256:1e9da0f496480ece506e283a11ecdef991a17af7ecacc2d16d4cd164b8cdb27b`;
- runtime: `sha256:4ec67f446c98848eed59f3a3597a395962c14f3fe2fadcda70a367e9738772ab`;
- runtime source: `sha256:57ef9fda64b3f9cc0fd253d3ea807075e1adb466c42621cd4109bed6e774eafb`;
- participant configuration: `sha256:1ea86cdbf2c9c02154c0ba7dde449b0050fefb51492898b09c1c9b2a28b1f777`;
- configuration mapping: `sha256:4633cc802750ade0a0ae483fa5c747bdad7990df9d7f18c6e202bf2ec7e30920`;
- permit set: `sha256:652942dad383cfcd2434c47bc33218fa6a9676474a84b3c7e5da9025695d17c1`;
- prelaunch: `sha256:a65e92ba45c9d3bd66c673bc1f980bc6f3769760cee4fdaaf24d598af164b8f6`;
- artifact: `sha256:1e49c6461a9a43359be65772579e4b9678d68235ba9288b7218e2bd64cb628c9`.

The complete 237-entry manifest is unique and byte-exact. All nine packet
roots and all nine prompt roots recompute. The replacement remains `not_run`
at 0/36. All 36 participant permits and the distinct neutral calibration
permit remain held, unexpired, and unconsumed. Recorded provider calls,
protected-key accesses, and scoring runs all remain zero.

## Focused checks and adversaries

The committed verifier, prelaunch custody verifier, 21 benchmark tests, five
provider-runtime tests, Ruff, the JavaScript event-contract tests, and
`git diff --check` passed. The Python verification and both focused suites
passed independently under CPython 3.10, 3.11, 3.13, and 3.14. A fresh
CPython 3.14 regeneration was Git-byte-clean.

The focused adversaries cover exact path/digest binding, duplicate schema
bindings, runtime-configuration drift, missing terminal receipts, unfrozen
adjudication, score-snapshot mutation, answer and cross-family leakage,
neutral-wrapper vocabulary, held permit identity, governance equality, OCI
archive mutation, wrong or malformed image identity, and unsafe output paths.
All failed closed as registered.

## Residual boundary

This PASS closes only the independently reproduced F08/G08 runtime custody
blocker for the immutable producer commit above. The experiment remains held
at 0/36. Any later neutral calibration release, participant permit release,
provider call, protected-key access, scoring action, merge, or scientific
claim remains a separate authorized action outside this review.
