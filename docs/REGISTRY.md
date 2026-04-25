# Vela Registry

The registry is Vela's verifiable-distribution primitive. A frontier
publisher signs a manifest of `(vfr_id, name, owner, snapshot_hash,
event_log_hash, locator, timestamp)`; a third party pulls and verifies.

This is the npm-tarball-with-a-signature shape. Use cases:

- Archival snapshots of a frontier at a specific moment.
- Reproducibility: a paper cites a frontier publication; readers `pull`
  and verify byte-identical reconstruction.
- Integrity-checked transfer between collaborating institutions.

Cross-frontier *links* (`vf_…@vfr_…` references) — composition — are
deferred to v0.6. v0.5's registry does *distribution* only.

## CLI

```bash
# Publish: sign and append to a registry
vela registry publish frontier.json \
  --owner reviewer:will-blair \
  --key ~/.vela/keys/private.key \
  --locator file:///abs/path/to/frontier.json \
  --to ~/.vela/registry/entries.json

# List entries
vela registry list --from ~/.vela/registry/entries.json

# Pull a frontier by vfr_id; verifies signature + snapshot + event_log
vela registry pull vfr_36aa055313a51e7e \
  --from ~/.vela/registry/entries.json \
  --out ./pulled.json
```

Defaults: when `--to`/`--from` is omitted, uses
`~/.vela/registry/entries.json`.

## Manifest format

```json
{
  "schema": "vela.registry-entry.v0.1",
  "vfr_id": "vfr_aaaaaaaaaaaaaaaa",
  "name": "BBB Flagship",
  "owner_actor_id": "reviewer:will-blair",
  "owner_pubkey": "<hex Ed25519 public key, 64 chars>",
  "latest_snapshot_hash": "<sha256 hex, 64 chars>",
  "latest_event_log_hash": "<sha256 hex, 64 chars>",
  "network_locator": "file:///abs/path/to/frontier.json",
  "signed_publish_at": "2026-04-25T00:00:00Z",
  "signature": "<hex Ed25519 signature, 128 chars>"
}
```

The `signature` is Ed25519 over the canonical preimage of every other
field. The same canonical-JSON discipline as `vev_…`/`vpr_…` derivation:
sorted keys at every depth, no whitespace, finite numbers, UTF-8 verbatim.
Two implementations agree byte-for-byte.

## Pull verification

`vela registry pull` performs a *total* verification:

1. The entry's `signature` verifies against `owner_pubkey`.
2. The pulled frontier's `snapshot_hash` matches
   `latest_snapshot_hash`.
3. The pulled frontier's `event_log_hash` matches
   `latest_event_log_hash`.

Any mismatch is rejection. The partial pulled file is removed. No partial
trust.

## Latest-publish-wins

Multiple publications of the same `vfr_id` are appended to the registry;
`vela registry pull` selects the entry with the latest `signed_publish_at`.

## Supported transports

Read-side (`--from`):

- Bare path: `/path/to/registry.json`
- `file://` URL
- Directory: appends `entries.json`
- `https://` (v0.7): fetched via blocking HTTP, parsed identically. URLs
  ending in `/` get `entries` appended; everything else is verbatim.

Write-side (`--to`):

- Bare path / `file://` / directory — local file append.
- `http(s)://` (v0.7): the entry is POSTed to `<hub>/entries`. The hub
  verifies the signature against the declared `owner_pubkey` and stores
  the canonical bytes verbatim. See [HUB.md](HUB.md) for hub semantics.

`git+...` is deferred to v0.8+.

## Doctrine

- **A registry is a fact archive, not a permission system.** It records
  who signed what, when, with what hashes. It does not gate who can
  pull or what they can do with it.
- **The signature binds the publisher.** The owner's pubkey is on the
  entry; the signature is over the entry's content. A registry entry
  whose signature does not verify is not an entry — it's noise.
- **Hash equality is the proof.** Verification is mechanical: did the
  publisher's hashes survive transport? If yes, it's the same frontier.
  If no, it isn't.
