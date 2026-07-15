# Vela Hub

The Hub is a disposable, read-only index over strictly replayed frontier Git
repositories. It improves discovery and cross-frontier queries. It does not own
frontier bytes, accept scientific state, register sources through an API, sign
tree heads, serve proof authority, or participate in a peer-consensus protocol.

The public deployment is <https://hub.constellate.science>.

## Authority boundary

For each published frontier:

```text
standalone Git repository
    -> exact commit and .vela/events bytes
    -> strict Vela validation and replay
    -> disposable SQL projections
    -> read-only HTTP and MCP queries
```

Git is the byte-custody and publication mechanism. Signed frontier events and
policy certificates carry authority. Frozen verifiers establish their declared
mechanical properties. A Hub row is only a derived observation of those inputs.

A compromised Hub can omit a frontier, lag behind Git, or answer incorrectly.
It cannot create a valid accepted history without the required signatures and
bytes. Consumers settle a discrepancy by cloning the configured repository and
running `vela check . --strict` and the frontier's frozen reproduction command.

## Source catalog

Operators select sources in a versioned JSON file:

```json
{
  "schema": "vela.hub-source-catalog.v1",
  "sources": [
    {
      "vfr_id": "vfr_001f148c07eebecb",
      "git_remote": "https://github.com/vela-science/example-frontier",
      "git_ref": "main"
    }
  ]
}
```

The bundled catalog is `crates/vela-hub/sources.json`. Set
`VELA_HUB_SOURCES_FILE` to use another checked-in catalog. There is no source
registration, deprecation, or mutation endpoint. Catalog selection is operator
configuration, not scientific authority.

The ingest loop fetches each source, requires the replayed `vfr_id` to match the
catalog, validates content addresses and signatures, and runs strict replay
before replacing projections. A later indexed tip must descend from the last
indexed commit. Sources removed from the active catalog and their projections
are pruned. Prelaunch databases from earlier schemas should be recreated, not
migrated through compatibility shims.

## Read surface

The HTTP API uses JSON and the common error envelope
`{"error":{"kind":"...","message":"..."}}`. The live route families are:

| Route | Purpose |
| --- | --- |
| `GET /healthz`, `/readyz`, `/metrics` | Liveness, readiness, and Prometheus metrics. |
| `GET /.well-known/vela` | Unsigned discovery metadata; never a trust root. |
| `GET /entries` and `/entries/{vfr_id}` | Configured frontier index rows. |
| `GET /entries/{vfr_id}/git-remote` | The configured Git source and ingest status. |
| `GET /entries/{vfr_id}/events` and `/events/stream` | Canonical replayed event projections and an SSE feed. |
| `GET /entries/{vfr_id}/snapshot`, `/summary`, `/manifest` | Materialized read models. |
| `GET /entries/{vfr_id}/objects/{type}[/{id}]` | Paged object reads and point lookup. |
| `GET /entries/{vfr_id}/findings/{vf_id}` and `/context` | Finding and bounded context projections. |
| `GET /entries/{vfr_id}/graph`, `/frontier`, `/boundary` | Derived graph and frontier views. |
| `GET /entries/{vfr_id}/review`, `/gate-status`, `/depends-on` | Derived review and dependency views. |
| `GET /search`, `/producers/{pubkey}`, `/diff-packs/{id}` | Cross-frontier queries. |
| `POST /mcp` | Stateless, read-only hosted MCP transport. |
| `POST /webhook/github` | HMAC-authenticated ingest wake-up; it carries no frontier state. |

Pagination, cursors, filtering, and response fields are discoverable from the
endpoint schema and stable CLI JSON contracts. This document intentionally does
not duplicate every projection field.

The review JSON contract is `vela.hub.review.v0.2`. It defaults to 25 rows and
accepts `limit=1..100` plus a non-negative `offset`. Every returned ledger has
its own `returned`, `total`, and optional `next_offset`; `stats` carries the
full totals, while `policy_admitted.by_policy` is explicitly scoped to the
current page. A continuation (`offset > 0`) must echo the first page's
`snapshot_hash`; a changed snapshot fails with `409 STALE_PAGE` so pages from
different replayed frontier states are never silently combined.

The Hub has negative route tests for former state-write, source-registration,
object-store, deprecation, status-authority, transparency-proof, and peer
surfaces. Adding any such route is a trust-boundary change, not routine API
work.

## Hosted MCP

`POST /mcp` uses the same read dispatcher as local `vela serve`, populated from
strictly promoted Hub projections. It exposes read tools only. It has no
filesystem-path tools, producer writes, decision verbs, key resolution, or
signing capability.

Local and hosted MCP are convenience query surfaces. A consumer that needs an
authority verdict or exact reproduction must use the standalone repository.

## Webhook

`POST /webhook/github` verifies `X-Hub-Signature-256` with
`VELA_HUB_WEBHOOK_SECRET` and schedules an ingest sweep. Without a configured
secret it returns unavailable and periodic sweeps continue. The webhook proves
only that GitHub sent the notification; strict replay still decides whether the
new commit may be indexed.

## Run locally

SQLite is sufficient for a disposable local index:

```bash
cargo build --release -p vela-hub
VELA_HUB_DATABASE_URL=sqlite:///data/hub.db \
  ./target/release/vela-hub
```

To use another catalog:

```bash
VELA_HUB_DATABASE_URL=sqlite:///data/hub.db \
VELA_HUB_SOURCES_FILE=/path/to/sources.json \
  ./target/release/vela-hub
```

Postgres uses the same derived schema. The executable schema definition is in
`crates/vela-hub/src/db.rs`. Back up the catalog and Git source identities, not
the disposable projection database.

## Operational checks

```bash
curl -fsS https://hub.constellate.science/healthz | jq
curl -fsS https://hub.constellate.science/entries | jq
```

If ingest fails, inspect the source's `ingest_error`, reproduce the failure from
a clean clone, and fix the frontier repository or catalog. Do not patch SQL
rows. A healthy database containing stale or unverified science is not a
successful Hub.
