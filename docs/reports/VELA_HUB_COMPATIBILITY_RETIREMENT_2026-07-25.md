# Vela Hub compatibility retirement

- Date: 2026-07-25
- Status: completed source retirement; unreleased
- Last source commit before removal:
  `241dbedf4cd92f414f8abe1d07288e7eb21ec7b7`
- Protocol effect: none
- Authority effect: none
- Frontier mutation: none

## Decision

Remove the unpublished `vela-hub` crate from the breaking `0.930` source
train. Keep exact historical tags and the verified public-state sunset archive.
Use local `vela serve` or the optional Observatory for current read tasks.

Do not replace the crate with another service, projector, public API, or
database. The surviving invariant is smaller: any reader is a disposable
projection over exact Frontier Git/Vela state and has no authority.

## Consumer trace

| Surface | Evidence | Decision |
| --- | --- | --- |
| GitHub organization | Current code search found implementation references only in `vela-science/vela`, historical parent documents, and Vela Web sunset documentation | no current client |
| Fly.io | Current account inventory contains no `vela-hub` or Vela Hub application | no deployment |
| GitHub releases | `v0.915.1` ships only `vela` and `vela-signer` archives, SBOMs, checksums, and trust records | no released Hub binary |
| crates.io | `cargo search vela-hub` returns no package; the crate declared `publish = false` | no published crate |
| Vela Web | Current documentation states that Web does not depend on Fly or the legacy Hub | Observatory remains the sole production projector |
| Canopus and Frontiers | Current source search found no Hub dependency | no producer or canonical-state consumer |
| Service state | The former service exposed derived projections over canonical Git repositories | retain verified sunset archive; no canonical state to migrate |

The pre-removal workspace contained eight members and 481 resolved packages.
The crate occupied 332 KiB of source and brought the Postgres SQLx backend,
`async-stream`, and server-only transitive dependencies into the workspace
lock. After removal the workspace contains seven members and 449 resolved
packages.

The resulting package check also exposed and repaired a candidate-release
bookkeeping defect: the new `vela-authority` crate was packageable but absent
from the ordered crates.io publication list and release-version loop. The
retired workspace now verifies and publishes one explicit seven-crate graph.

## Preserved evidence

Historical source remains available through every prior Vela tag and Git
commit. The final public service projection and redacted operator inventory are
retained under:

```text
~/Desktop/Constellate/Archives/vela-hub-sunset-2026-07-19/
```

All 37 files pass the retained `SHA256SUMS`; its root is:

```text
sha256:ac819ac132641aca7fa2b5a60492d9779f85edf81614dfd7c1a0ca3351836a66
```

The archive contains public derived projections and redacted operator
metadata, not authority keys, credentials, or unique scientific history.

## Removed

- `crates/vela-hub/`, including SQLite/Postgres projection code, HTTP/MCP
  service, source scheduler, Dockerfile, and Fly configuration;
- active `docs/HUB.md`;
- workspace, release, package, and routine conformance bookkeeping;
- stale CLI guidance that pointed retired `vela hub` users to another binary;
- Hub-specific server dependencies that no surviving crate uses.

Current protocol and threat-model documentation now describes the general
derived-reader boundary instead of a product that no longer exists.

## Verification contract

Retirement must preserve:

- canonical event and object replay;
- strict signals and proposal standing;
- local `vela serve`;
- stable CLI JSON and retired-command guidance;
- cross-implementation fixtures;
- Vela Web's independent Git-to-Neon projection; and
- exact historical checkout and release reconstruction.

No release candidate is tagged by this source-only change.
