# Vela Workbench (v0.5+)

The Workbench is a local browser-served review UI for Vela frontiers. It
mounts a set of live HTML pages alongside the API: the proposals queue
(Phase R, v0.5), the live frontier table (Phase γ, v0.6), and the live
finding detail view (Phase γ, v0.6). The static design fixtures
(`frontier.html`, `finding.html`) keep their preview-banner role as
brand-canon references.

## The drafts-then-CLI-signs model

The Ed25519 private key never enters the browser. Decisions made in the UI
queue locally as unsigned drafts; a separate CLI step (`vela queue sign`)
reads the actor's key and applies the queued actions.

```
browser (Workbench) ──POST /api/queue──▶ ~/.vela/queue.json ─┐
                                                             │
                          vela queue sign --actor <id> --key <path>
                                                             │
                                                             ▼
                                            applies via proposals::*_at_path
```

Doctrine: signing is a deliberate human act on a terminal that holds the
key. Browser-side signing would enlarge the trust surface to include
extensions, DNS rebinding, and any local process that can talk to
localhost. v0.5 rejects that.

## Running

The Workbench UI lives in the Astro site under `site/src/pages/workbench/`
and is served from [vela-site.fly.dev/workbench](https://vela-site.fly.dev/workbench).
It fetches against any `vela serve` instance over `/api/*`. Local
development:

```bash
# 1. Register your reviewer identity in the frontier
vela sign generate-keypair --out ~/.vela/keys
vela actor add frontier.json reviewer:will-blair --pubkey "$(cat ~/.vela/keys/public.key)"

# 2. Start the API server
vela serve --http 3848 frontier.json

# 3. Run the Astro site against it (proxies /api/* to localhost:3848 in dev)
cd site && npm run dev
# open http://localhost:4321/workbench           findings table
# open http://localhost:4321/workbench/finding   single finding view
# clicking a row navigates to /workbench/finding?id=vf_…
```

The single-finding page fetches `/api/findings/{id}` and `/api/frontier`,
renders the finding plus any pending proposals targeting it, and posts
accept/reject decisions to `/api/queue`. The page never touches the
private key.

## Walking the queue

```bash
# Inspect what you've queued
vela queue list

# Apply each queued action with explicit per-action confirmation
vela queue sign --actor reviewer:will-blair --key ~/.vela/keys/private.key

# Or sign-and-apply every queued draft (trusted-batch mode)
vela queue sign --actor reviewer:will-blair --key ~/.vela/keys/private.key --yes-to-all

# Drop the queue without signing
vela queue clear
```

`vela queue sign`:
1. Loads the queue file.
2. For each action, prompts unless `--yes-to-all`.
3. Reads the Ed25519 private key from `--key`.
4. Constructs the canonical preimage and signs.
5. Applies via the same `proposals::*_at_path` helpers the CLI uses.
6. Removes signed-and-applied actions; failed actions stay for retry.

## Queue file format

```json
{
  "schema": "vela.queue.v0.1",
  "actions": [
    {
      "kind": "accept_proposal",
      "frontier": "/path/to/frontier.json",
      "args": {
        "proposal_id": "vpr_...",
        "reviewer_id": "reviewer:will-blair",
        "reason": "reviewed via workbench",
        "timestamp": "2026-04-25T..."
      },
      "queued_at": "2026-04-25T..."
    }
  ]
}
```

Default location: `~/.vela/queue.json`. Override with `VELA_QUEUE_FILE`.

## What the Workbench is and isn't

**Is.** A reviewer's terminal-equivalent: a comfortable UI for walking a
proposal queue when terminal review is too dense. Local-only.

**Isn't.** A hosted SaaS, a multi-user surface, or a public adoption
funnel. Hosted Workbench (multi-user, deployed) is v0.6+.
