# Vela Hub — the protocol node

**The hub is a PROTOCOL node: an INDEX over git-replayed state, not the
authority and not a website.** Every endpoint speaks JSON (or serves a
content-addressed artifact). The log proves itself; viewers are
replaceable.

The authoritative source of a frontier is its git repo (the committed
`.vela/events` log), reproducible from a clean clone. The hub is a
convenience layer: cross-frontier search, reverse-dependency lookup,
producer queries, projection and event-stream APIs, and the editorial
"live" filter. It does not own byte custody (git + LFS do), and it is
not the write authority for acceptance (a reviewer's signed
`review.accepted` event in a PR is). Its one write endpoint is the
owner-signed git-remote registration: an owner binds a repo once
(`vela hub register-git <vfr> --remote <url>`, or POST
`/entries/{vfr}/git-remote` with a signed `vela.frontier-git-remote.v0.1`
record), and the hub re-derives the index by fetching the repo and
holding it to the one strict bundle (`verify_frontier_strict`:
content-address validation, strict reducer replay — never the loader's
repair-lenient degrade — and error-severity signature signals; a
tampered signed event is refused with "id does not re-derive"), then
promoting with `authority_mode='git_ingested'`. `git push` is
publication.

Clients verify locally on read, so a compromised hub can withhold or
reorder, but cannot fabricate or tamper without breaking signatures and
hashes.

The public hub is **<https://hub.constellate.science>**.

## The viewer contract

The hub serves state; it does not present it. **Any client that speaks
the endpoints below is a viewer** — a browser app, a CLI, an agent over
`/mcp`, a script over `curl`. <https://app.constellate.science> is one
such viewer (the reference one); it holds the design system, the record
pages, and the human-facing 404/unavailable UX. The hub carries none of
that: presentation changes never touch the protocol node, and the node
can be mirrored by anyone without inheriting a frontend.

Concretely:

- Requests without `Accept: text/html` get protocol JSON. This is the
  default for `curl`, SDKs, and agents.
- Requests **with** `Accept: text/html` on a state surface get a
  `301 Moved Permanently` into the app (the table below), with a
  one-line `text/plain` body naming the target for curl users.
- `GET /` keeps a minimal self-describing HTML banner (system fonts, no
  assets): hub name, version, doctrine, endpoint list, and the doors to
  the app and these docs.

## The 301 map (hub URL → app URL)

`{site}` is `VELA_SITE_URL` (default `https://app.constellate.science`).

| Hub URL (with `Accept: text/html`) | 301 target |
|---|---|
| `/entries` | `{site}/frontiers` |
| `/entries/{vfr_id}` | `{site}/r/{vfr_id}` |
| `/entries/{vfr_id}/findings/{vf_id}` | `{site}/r/{vfr_id}/findings/{vf_id}` |
| `/entries/{vfr_id}/review` (also `?format=html`) | `{site}/r/{vfr_id}/review` |
| `/entries/{vfr_id}/packs/{pack_id}` | `{site}/r/{vfr_id}/packs/{pack_id}` |
| `/entries/{vfr_id}/reproduce` (redirect-only: any Accept) | `{site}/r/{vfr_id}/reproduce` |
| `/entries/{vfr_id}/proof` | `{site}/r/{vfr_id}/proof` |
| `/producers/{pubkey}` | `{site}/producer/{pubkey}` |
| `/search` | `{site}/search?q={q}` (query preserved) |

JSON responses on the same routes are unchanged from the pre-cut
contract (search and `/entries` gained ADDITIVE pagination fields only;
see below).

## Endpoints

| Endpoint | Behavior |
|---|---|
| `GET /` | JSON banner (endpoint list + version); minimal HTML banner for browsers. |
| `GET /healthz` | Liveness; reports DB reachability. Rate-limit exempt. |
| `GET /readyz` | Readiness (hosted-MCP projection built). Rate-limit exempt. |
| `GET /metrics` | Prometheus 0.0.4 text: request counters, latency histograms, db-cache series, `vela_hub_rate_limited_total`. Rate-limit exempt. |
| `GET /.well-known/vela` | Signed discovery manifest: endpoints, schemas, canonical-JSON rules, `peers`. |
| `GET /entries` | Live frontiers as `vela.registry-entry.v0.1` rows. Optional `limit` (1–500) + `offset`; when `limit` is present the response additively gains `{total, next_offset}`. Default (no params) is the full list, byte-compatible with existing consumers. |
| `GET /entries/{vfr_id}` | One live frontier entry (the signed manifest row). |
| `GET /entries/{vfr_id}/events?cursor=&limit=&kind=&target=` | Cursor-paginated canonical event log ordered by `seq`; `next_cursor` resumes the walk. Unknown cursors return 400. |
| `GET /entries/{vfr_id}/events/stream?cursor=` | Server-sent event stream: backlog, then heartbeat while idle. |
| `GET /frontier/{vfr_id}/inbox` | Agent-facing alias for the event stream. |
| `GET /entries/{vfr_id}/snapshot` | Derived materialized snapshot JSON. Content-addressed, so `latest_snapshot_hash` is the `ETag`: send `If-None-Match: "<hash>"` for a `304` when the frontier has not moved (before the hub materializes a multi-MB project). `?redirect=cdn` redirects to the immutable blob when available. |
| `GET /entries/{vfr_id}/summary` | Cheap projection-table counts. |
| `GET /entries/{vfr_id}/manifest` | Counts + log head + object-id index (the sparse-read primitive). |
| `GET /entries/{vfr_id}/status` | `live` / `deprecated`, with the signed deprecation receipt. |
| `GET /entries/{vfr_id}/objects/{type}` | One page of objects (`limit`, `offset`, `total`). |
| `GET /entries/{vfr_id}/objects/{type}/{id}` | Point lookup. |
| `GET /entries/{vfr_id}/findings/{vf_id}` | The finding bundle as-is. |
| `GET /entries/{vfr_id}/findings/{vf_id}/context` | Project-shaped slice scoped to one finding. |
| `GET /entries/{vfr_id}/findings/{vf_id}/gate-status` | Derived trust-gate status (never stored). |
| `GET /entries/{vfr_id}/gate-status` | Gate status for every finding, one request. |
| `GET /entries/{vfr_id}/review` | Review queue + autonomy ledger JSON (`vela.hub.review.v0.1`). |
| `GET /entries/{vfr_id}/proof` | The proof packet's three canonical files as JSON: `{manifest, proof_trace, lock}`. 404 (house envelope) when no packet exists. |
| `GET /entries/{vfr_id}/proof/download` | Proof packet as `.tar.gz` (binary artifact endpoint). |
| `GET /entries/{vfr_id}/log/sth` | Signed RFC 6962 tree head over the event log. |
| `GET /entries/{vfr_id}/log/proof/{event_id}` | Inclusion proof against the STH. |
| `GET /entries/{vfr_id}/log/consistency?first=&second=` | Append-only consistency proof. |
| `GET /entries/{vfr_id}/depends-on` | Reverse dependency lookup across live frontiers. |
| `GET /entries/{vfr_id}/proposals/{id}/evidence-diff` | Read-only Evidence Diff projection. |
| `GET /entries/{vfr_id}/git-remote` | The registered git remote (url, ref, subdir, ingest cursor). |
| `POST /entries/{vfr_id}/git-remote` | The one write: an owner-signed `vela.frontier-git-remote.v0.1` registration. |
| `GET /entries/{vfr_id}/sidon-frontier-map`, `…/sidon-observation` | Sidon-profile planning/read views (422 on non-Sidon frontiers). |
| `GET /producers/{pubkey}` | Cross-frontier objects signed by one key. |
| `GET /search?q=&type=&limit=&offset=` | Cross-frontier object search (below). |
| `GET /blobs/{hash}` | Content-addressed artifact blob: 302 to the immutable CDN object; the client re-hashes on receipt. |
| `GET /diff-packs/{pack_id}` | A registered `vsd_*` Scientific Diff Pack. |
| `POST /mcp` | Hosted MCP endpoint (streamable HTTP, stateless JSON, read-only profile). |
| `POST /webhook/github` | HMAC-authenticated push kick for ingest + MCP refresh. |

Historical note: `POST /entries` (the signed-manifest publish) was the
pre-git-native write path and is removed (ADR 0001 Phase 2). Index rows
are synthesized by the ingest loop from strictly replayed repos; the
`vela.registry-entry.v0.1` shape survives as the read-side row format.

### Errors

One envelope everywhere: `{"error": {"kind": "...", "message": "..."}}`
with kinds `INVALID_ARG`, `NOT_FOUND`, `PERMISSION_DENIED`,
`UNAVAILABLE`, `INTERNAL`, `RATE_LIMITED`.

## Search

`GET /search?q=<query>&type=<object_type>&limit=<n>&offset=<n>` — one
query over `frontier_objects` across live frontiers, restricted to one
`object_type` (default `finding`). `limit` defaults to 24, max 200;
`offset` defaults to 0. Response:

```json
{"q": "...", "type": "finding", "results": [{"vfr_id": "...", "object": {…}}],
 "total": 123, "next_offset": 24}
```

`total` counts every match under the same predicate; `next_offset`
appears only while more rows remain. Both fields are additive on the
original `{q, type, results}` shape. Empty `q` returns the empty result
set unchanged.

Backends: Postgres runs real full-text search — a stored generated
`tsvector` over the raw object JSON, GIN-indexed, queried with
`websearch_to_tsquery('english', q)` (total on any user syntax — it
never errors) and ranked by `ts_rank`. The SQLite backend (self-hosted)
keeps exact-substring LIKE with the same response shape.

## Rate limits

Per-client-IP sliding window, 60-second span:

| Class | Budget / min |
|---|---|
| default (all GETs and unlisted routes) | 120 (env `VELA_HUB_RATE_LIMIT_PER_MIN`; `0` disables the limiter) |
| `POST /mcp` | 20 |
| `POST /webhook/github` | 60 (HMAC'd, still bounded) |
| `/healthz`, `/readyz`, `/metrics` | exempt |

Over budget ⇒ `429` with `Retry-After: <seconds>` and the house error
envelope (`kind: "RATE_LIMITED"`). Refusals are counted at `/metrics`
as `vela_hub_rate_limited_total`. Client IP resolution order:
`Fly-Client-IP`, first `X-Forwarded-For` hop, socket peer address.
Budgets are per (IP, class), so a GET flood cannot starve the POST
lanes.

## Peers discovery

`GET /.well-known/vela` carries `manifest.peers`: other hubs known to
serve the same protocol surface, from env `VELA_HUB_PEERS`
(comma-separated URLs; absent ⇒ `[]`). The field lives inside the
manifest, so the detached Ed25519 signature over the manifest's
canonical bytes covers it. Peers are DISCOVERY, never trust: every
frontier read from any hub is still verified against the publisher's
signatures and hashes, and a witness check across hubs
(`scripts/test-multi-hub-witness.sh`) proves byte agreement without
making either hub authoritative.

## Publishing (git push)

Publication is `git push` to the frontier's repo; the hub's ingest loop
re-derives the index from the committed event log. Bind the repo once:

```bash
vela hub register-git <vfr_id> --remote <repo-url>
```

## Reading

```bash
git clone <repo-url> && vela check <dir> --strict   # the authority path
curl https://hub.constellate.science/entries        # the index view
```

## Hosted MCP (`/mcp`)

Remote agents connect to the hub as an MCP server — no clone, no local
binary. The endpoint is streamable HTTP with stateless JSON responses
(POST a JSON-RPC message; no server-initiated SSE stream), backed by the
same dispatcher, profile gate, and tool registry as `vela serve`. The
projection is hydrated from the hub's own database — every live
frontier's materialized snapshot — and rebuilt when any promoted
snapshot hash moves: on an interval (`VELA_HUB_MCP_REFRESH_SECS`,
default 300 s) or immediately after a webhook push's ingest sweep lands.

`/mcp` serves only strictly-verified promoted frontiers. Add it to any
MCP client as `https://hub.constellate.science/mcp` (transport:
streamable HTTP, no auth). The tool surface is the read-only profile
minus the filesystem-path tools — five tools: `orient`, `finding`,
`search`, `graph`, `external`. Every write verb and every decision verb
is absent by construction.

`vela serve <frontier> --http <port>` exposes the identical `/mcp`
route locally, so the remote and local contracts are the same surface.

## Webhook (`POST /webhook/github`)

The ingest and MCP refresh loops sweep on an interval; the webhook is
the latency lane that makes `git push` reflect in seconds. Configure a
GitHub webhook on the frontier repo (push events, content type JSON,
secret = the hub's `VELA_HUB_WEBHOOK_SECRET`); the hub verifies
`X-Hub-Signature-256` over the raw body and then kicks a targeted
sweep. Authenticity of state never rests on this header: whatever
arrives is still held to the strict replay bundle like every sweep. No
secret configured ⇒ the route answers 503 and the interval sweeps
remain the only refresh path.

## Run your own hub / mirror

The hub is one Rust binary plus a SQL backend. Because the git repos
are the store of record and the hub is a re-derivable projection, a
mirror needs no data handoff from the public hub — it re-derives the
same index from the same repos.

### SQLite one-liner (laptop mirror, small institution, air-gap prep)

```bash
cargo build --release -p vela-hub
VELA_HUB_DATABASE_URL=sqlite:///data/hub.db ./target/release/vela-hub
```

The schema auto-creates on first start. Then register the same git
remotes the public hub carries (each frontier's is readable at
`GET /entries/{vfr}/git-remote`):

```bash
vela hub register-git <vfr_id> --remote <repo-url>   # against YOUR hub
```

The ingest loop fetches, strictly replays, and promotes — your index
converges on the same state because it is derived from the same signed
logs, not copied from the public hub. To prove agreement, run the
witness check across both:

```bash
VELA_HUB_URLS=https://hub.constellate.science,https://your-hub.example \
  VELA_WITNESS_PACK_ID=<vsd_id> ./scripts/test-multi-hub-witness.sh
```

Agreement means byte-equivalent signed material. It does not make
either hub authoritative — the repos and the signatures already were.

Optionally announce the relationship: set
`VELA_HUB_PEERS=https://hub.constellate.science` on your hub (and ask
the public hub's operator to list yours) so clients can discover the
mirror from `/.well-known/vela`.

### Postgres (production)

Point `VELA_HUB_DATABASE_URL` at Postgres. The executable schema source
is [`crates/vela-hub/src/db.rs`](../crates/vela-hub/src/db.rs)
(`POSTGRES_EVENT_FIRST_SCHEMA`) — applied opportunistically at startup
when the role has DDL privileges, otherwise by a privileged migration
job (the production pattern; this includes the `/search` FTS column +
GIN index). The event-first backfill binary
(`vela-hub-backfill-event-first`, `--dry-run` first) promotes verified
historical rows.

Deploy (Fly):

```bash
flyctl deploy --config crates/vela-hub/fly.toml \
  --dockerfile crates/vela-hub/Dockerfile --depot=true --wait-timeout 600 .
```

Always pass `--wait-timeout 600` (readiness waits on the first MCP
projection build) and `--depot=true` (the default remote builder routes
to a dead org). Rollback via `flyctl releases` + `flyctl deploy
--image <previous>`.

Runbook checks before and after any deploy:

```bash
curl -fsS https://hub.constellate.science/healthz | jq
curl -fsS https://hub.constellate.science/entries/vfr_496956067dc5ad79 | jq '.vfr_id'
curl -fsS 'https://hub.constellate.science/entries/vfr_496956067dc5ad79/events?limit=1' | jq '.events[0].id'
```

The Sidon-sets frontier (`vfr_496956067dc5ad79`) is the public canary.
If a latest registry row fails verification, do not patch around it in
the database: fix the frontier repo and push (the ingest loop retries
every sweep). The failed row belongs in `frontier_publish_audit`.

## Release verification boundary

Two senses of "source of truth", never conflated:

- **Byte custody.** The frontier git repo is canonical for event-log
  bytes and witnesses; the hub's copy is an ingest-derived projection.
- **Scientific verdict.** The hub is signed transport, not the arbiter
  of truth. Correctness comes from re-deriving frontier state, the
  signed events, and frozen-verifier proof-packet validation on a clean
  clone. A hub serving tampered bytes simply fails to reproduce. The
  hub stores; the verifiers judge.

```bash
VELA_HUB_RELEASE_CHECK=1 ./tests/test-hub-release-boundary.sh
```

## Operational notes

- **Production credentials are not dev credentials.** The Fly secret is
  a Postgres role scoped to the hub schema. Never paste connection
  strings into chat or commits; rotate on exposure.
- **Bot keys** are actors whose private key lives in a CI secret
  (`vela id keygen` → `vela actor add … --tier auto-notes` → repo
  secret). Rotation: new keypair, re-register, re-push the secret.
- **Hub compromise** affects *availability*, not *authenticity*:
  clients verify the manifest signature against `owner_pubkey` and the
  frontier's hashes against the manifest.
