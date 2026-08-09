# vela-source-manifest

One resolver and one pair of schemas for every Frontier's acquisition inventory.

A Frontier declares what it acquires in `sources.yaml` and records what it
actually got in `sources.lock.json`. This package reads the first and writes the
second, computing every content root from bytes it fetched or read.

## Why it sits beside `crates/`, not inside it

This is producer-side tooling. It describes how a Frontier acquired its inputs,
which is a fact about an acquisition run, not a fact the protocol adjudicates:
nothing here defines or depends on Vela authority semantics, and `vela replay`
must never need it. A lock is evidence a reader can check; it is not Standing. If
this package disappeared, every accepted record would still replay.

## Use

```sh
cd <a frontier root>
uvx --from vela-source-manifest vela-source-lock            # write the lock
uvx --from vela-source-manifest vela-source-lock --check    # verify it, offline
```

Until the first release there is no registry version to resolve that name from,
so run it from a checkout instead — `uvx --from ../vela/packages/vela-source-manifest
vela-source-lock`. The four Frontiers name the released form in their
`sources.yaml` headers with no pin, and say there that the pin arrives with the
release.

`--check` is what CI should run. It is offline by default, and that is
deliberate: a lock records what a Frontier acquired *at a moment*, so upstream
having moved since is not a defect in the lock. The Erdős Frontier's live-fetched
pins are stale on purpose — they record what was acquired when its Claims were
accepted. Add `--refetch` when you actually want to ask whether upstream still
serves the same bytes.

## The rule

**Every hash is computed from bytes this code actually fetched or read.** A
declared hash is never copied through from `sources.yaml`. Where a source
declares one, the declaration is an assertion to check, and a mismatch fails the
run rather than being retained under the same commit. Where no content hash can
be computed at all, the entry says so in `unlocked` and gives the reason. A hash
nobody computed is worse than no hash at all, and a source silently dropped is
worse still.

## The reader invariant

Every lock entry carries **exactly one** of:

| field | meaning |
| --- | --- |
| `sha256` | a content root computed from the bytes named by `url` or `path` |
| `exact_roots` | per-file content roots, for a repository pinned at a commit whose individual files are the retained evidence |
| `unlocked` | a sentence saying why no content hash exists for this entry |
| `error` | this should have been lockable and was not — written into the lock so the gap is on the record, after which the run exits non-zero |

An entry nobody could pin must never read as one nobody bothered to pin.

## One place `path` is refused rather than guessed at

`path` means two things across the Frontiers. On an entry with no `url` it names
bytes retained in the Frontier itself, hashed from disk. On a url-backed entry —
the four live Erdős registries — it names the file's location in the *upstream*
repository, and the url is the locator.

The two never collide today, because no declared upstream path also exists
locally. If one ever did, silently hashing it would switch the pin from upstream
bytes to local bytes under an entry that still names the url, and the lock would
read exactly as it does now. So the resolver fails that entry instead, and
`--check` reports the same ambiguity. Only the author knows which of the two
holds the acquired bytes.

## The schemas are the definition

`src/vela_source_manifest/schemas/sources.schema.json` and
`sources-lock.schema.json` are JSON Schema 2020-12, closed
(`additionalProperties: false`), and they ship as package data. The resolver
validates both its input and its own output against them.

A consumer in another language should read those files rather than restate the
shape. A restated schema is a second opinion, and second opinions drift — which
is the whole reason this package exists. Get the file with:

```sh
vela-source-lock --print-schema sources-lock.schema.json
```

or, from Python, `vela_source_manifest.schema_path(name)`.

## What is not here

Nothing writes to `records/`, `proof/`, `artifacts/`, `execution/`, `review/`,
`targets/`, `witnesses/` or `.vela/`. The resolver reads in-repository bytes when
a declaration names a path inside the Frontier, and reads nothing else locally.
