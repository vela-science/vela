# Vela Hub

The hub is HTTP transport over the registry primitive. Read endpoints
serve signed manifests; the write endpoint accepts any signed manifest
whose signature verifies against its declared `owner_pubkey`. Clients
verify locally on read, so a compromised hub can withhold or reorder,
but cannot fabricate or tamper without breaking signatures.

The public hub for v0.7 is **<https://vela-hub.fly.dev>**.

## Doctrine

- **Dumb transport.** The hub stores canonical bytes verbatim. It does
  not re-canonicalize, does not interpret findings, does not score.
- **The signature is the bind, not access control.** Anyone with an
  Ed25519 key can publish their own `vfr_id`. There is no allowlist of
  pubkeys; there is no rate limit in v0.7. Abuse mitigations land when
  abuse appears.
- **Manifests, not blobs.** The hub stores the signed entry. The
  frontier file lives wherever the publisher put it (`network_locator`
  on the entry — typically a raw GitHub URL or S3). `vela registry
  pull` fetches the frontier from its locator and verifies hashes.

## Endpoints

| Endpoint | Behavior |
|---|---|
| `GET /` | Banner + endpoint list + version. |
| `GET /healthz` | Liveness; reports DB reachability. |
| `GET /entries` | Full registry, latest-publish-wins per `vfr_id`. |
| `GET /entries/{vfr_id}` | Latest entry for one `vfr_id`. v0.8: HTML view fetches the frontier from its locator and renders findings inline; cross-frontier link targets render as click-through to the other entry. |
| `GET /entries/{vfr_id}/findings/{vf_id}` | Single-finding view: claim, conditions, evidence, links. v0.8: cross-frontier links navigate. |
| `POST /entries` | Publish a signed manifest. 201 fresh, 200 duplicate, 400 tamper or schema mismatch, 500 DB error. |

`POST /entries` body shape: a single registry entry matching
`vela.registry-entry.v0.1`. See [REGISTRY.md](REGISTRY.md#manifest-format).

### v0.8: cross-frontier composition

A frontier published to the hub may declare cross-frontier dependencies
in its `frontier.dependencies` array, pinning each remote frontier by
`vfr_id` and `pinned_snapshot_hash`. Findings in the dependent frontier
can then link to findings in the dep via `vf_<id>@vfr_<id>` link
targets. The hub renders such links as clickable navigation between
entries; clients use `vela registry pull <vfr> --transitive --from
https://vela-hub.fly.dev/entries` to fetch and verify the whole chain.

The hub itself remains dumb transport — it does not resolve cross-
frontier references at storage time. Resolution happens client-side at
pull time, where the canonical-JSON snapshot pin is the integrity
guarantee.

Idempotency: `(vfr_id, signature)` is unique. Re-POSTing identical
canonical bytes returns 200 with `duplicate=true`; the row is not
duplicated. Two CLI runs spaced apart produce *different* manifests
(each gets a fresh `signed_publish_at`), so both rows persist and the
latest-publish-wins read returns the newer.

## Publishing

```bash
vela registry publish frontier.json \
  --owner reviewer:my-id \
  --key ~/.vela/keys/private.key \
  --locator https://raw.githubusercontent.com/me/repo/main/frontier.json \
  --to https://vela-hub.fly.dev
```

The CLI signs locally, POSTs the entry, and surfaces the hub's
`{ok, vfr_id, signed_publish_at, duplicate}` response. The owner must
already be registered as an actor in the frontier with a matching pubkey.

## Pulling

```bash
vela registry list --from https://vela-hub.fly.dev/entries
vela registry pull vfr_… --from https://vela-hub.fly.dev/entries --out ./pulled.json
```

`pull` fetches the frontier from the entry's `network_locator` and
verifies signature + snapshot + event-log hashes. Any mismatch deletes
the partial file and rejects.

## CI bot actors (the BBB pattern)

A bot is just an actor whose private key lives in a CI secret. The
substrate already treats signing identity as portable — there is no
distinction between "human signs" and "bot signs."

```bash
# 1. Generate a keypair locally.
vela sign generate-keypair --out ~/.vela/keys/my-bot

# 2. Register the pubkey in the frontier with a tier.
vela actor add path/to/frontier.json reviewer:my-bot \
  --pubkey "$(cat ~/.vela/keys/my-bot/public.key)" \
  --tier auto-notes

# 3. Push the private key into a GitHub Actions secret.
gh secret set MY_BOT_KEY --repo me/repo < ~/.vela/keys/my-bot/private.key

# 4. Wipe the local copy. The secret is now the only authoritative
#    custodian. Rotation = generate a new key, update the frontier,
#    re-push the secret, commit. There is no "read out" of the secret.
rm ~/.vela/keys/my-bot/private.key
```

The BBB living-repo workflow at
[`.github/workflows/bbb-living-repo.yml`](../.github/workflows/bbb-living-repo.yml)
is a worked example.

## Self-hosting

The hub is one Rust binary plus a SQL backend — Postgres for production
or SQLite for low-volume self-hosted use (v0.21).

### SQLite (self-hosted, zero infrastructure)

```bash
cargo build --release -p vela-hub
VELA_HUB_DATABASE_URL="sqlite:///path/to/your-hub.db" \
  ./target/release/vela-hub
```

That's the whole setup. The hub auto-creates the schema on first
startup (`CREATE TABLE IF NOT EXISTS …`); subsequent runs reuse the
file. Ideal for a laptop hub mirroring the public one for offline use,
a small institution publishing one corpus, or anyone running the
federation pattern from §Federation without a Postgres dependency.

The SQLite backend serves every endpoint the Postgres one does:
`GET /entries`, `GET /entries/{vfr_id}`, `GET /entries/{vfr_id}/depends-on`,
`POST /entries`, `vela registry pull --from`, `vela registry mirror --to`.
Verified end-to-end against the public hub: pulling a BBB-mirrored
frontier from a SQLite hub returns byte-identical bytes with
`verified=true`, same as pulling from `vela-hub.fly.dev`.

### Postgres (production)

For higher concurrency / larger data, point at a Postgres URL instead
and apply this schema once:

```sql
CREATE TABLE registry_entries (
  id BIGSERIAL PRIMARY KEY,
  vfr_id TEXT NOT NULL,
  schema TEXT NOT NULL,
  name TEXT NOT NULL,
  owner_actor_id TEXT NOT NULL,
  owner_pubkey TEXT NOT NULL,
  latest_snapshot_hash TEXT NOT NULL,
  latest_event_log_hash TEXT NOT NULL,
  network_locator TEXT NOT NULL,
  signed_publish_at TIMESTAMPTZ NOT NULL,
  signature TEXT NOT NULL,
  raw_json JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_entries_vfr_id ON registry_entries (vfr_id);
CREATE INDEX idx_entries_signed_publish_at ON registry_entries (signed_publish_at DESC);
CREATE UNIQUE INDEX uq_entries_vfr_signature ON registry_entries (vfr_id, signature);
```

Deploy:

```bash
flyctl launch --no-deploy --config crates/vela-hub/fly.toml \
  --dockerfile crates/vela-hub/Dockerfile --copy-config \
  --name <your-hub-name> --org <your-org> --region <region>
flyctl secrets import --config crates/vela-hub/fly.toml < /path/to/prod.env
flyctl deploy --config crates/vela-hub/fly.toml \
  --dockerfile crates/vela-hub/Dockerfile .
```

The runtime needs only `VELA_HUB_DATABASE_URL`. Local dev can fall
back to `~/.vela/hub.env`. Other platforms work identically — the hub
is platform-agnostic; swap the runtime.

## Operational notes

- **Production credentials are not dev credentials.** The Fly secret
  is a fresh Postgres role with INSERT/SELECT (and sequence USAGE) on
  `registry_entries` only — no DROP, no ALTER, no DELETE. The dev
  sandbox URL in `~/.vela/hub.env` is for local testing.
- **Never paste connection strings into chat or commits.** If the URL
  ever appears in conversation, rotate the role's password.
- **Bot key rotation.** Generate a new keypair, run `vela actor add`
  to register the new pubkey in the frontier (replacing the old
  entry — `actor add` overwrites by id), commit, then update the CI
  secret. The old key stops being trusted as soon as the frontier
  re-publishes.
- **Hub compromise.** Anyone consuming the hub verifies the manifest's
  signature against `owner_pubkey` and the frontier's hashes against
  the manifest. The hub controls *availability*, not *authenticity*.

## Federation (v0.20)

Hub-to-hub federation is no longer hypothetical. A second hub instance
runs at <https://vela-hub-2.fly.dev> (same Rust binary, separate Neon
Postgres, separate Fly app under `vela-237`). The doctrinal claim —
*the signature is the bind, not the hub identity* — is now empirically
validated end-to-end.

The federation primitive:

```bash
vela registry mirror <vfr_id> \
  --from https://vela-hub.fly.dev \
  --to https://vela-hub-2.fly.dev
```

Mechanism: GET the signed manifest from `from/entries/{vfr_id}`, POST
the same bytes to `to/entries`. Both hubs validate the manifest's
Ed25519 signature against the embedded `owner_pubkey`. Mirroring is a
no-op for authenticity — neither hub gains any signing role.

Validated against the live deploy:

- 3 frontiers (BBB Flagship, BBB-extension, Will's Alzheimer's drug-
  target landscape) mirrored from hub-1 → hub-2 cleanly.
- Re-mirroring the same vfr returns `duplicate=true` from the destination
  (idempotent on the `(vfr_id, signature)` unique constraint).
- `vela registry pull` against hub-2 produces byte-identical bytes and
  same `verified=true` as against hub-1.
- Snapshot hashes match across hubs for the same vfr_id.

What this unblocks:

- **Resilience:** if one hub goes down, mirror to a backup ahead of
  time and keep `vela registry pull` working.
- **Seeding:** a fresh hub instance can be primed from an existing
  one without any signing roundtrip.
- **Independent deploys:** an institution running its own hub can
  mirror the public hub's content for offline / air-gapped use, then
  publish its own frontiers independently.
- **The right substrate property:** pulling a frontier doesn't require
  trusting a single hub. Any hub serving the bytes is sufficient
  because the signature is over the publisher's content, not the
  serving infrastructure.

What v0.20 still doesn't ship:

- Automatic mirror (cron-style "keep hub-2 in sync with hub-1"). The
  `mirror` primitive is the building block; an automated mirror is one
  bash loop or one CI job around it. Defer until someone runs into the
  manual-replay friction.
- Hub-A-discovers-hub-B / cross-hub queries (e.g. "which hubs have a
  copy of this vfr_id"). Each hub stays autonomous; clients pick which
  hub to talk to.

## What is deferred

Each of these is enabled by what's shipped, but not in scope:

- Hub-hosted frontier blobs. The locator is wherever the publisher
  hosts the file.
- Webhooks / SSE on the hub.
- Per-pubkey rate limits, allowlists, abuse handling. Add when abuse
  exists.
- A real domain (e.g. `hub.vela.science`). The Fly URLs are sufficient.
