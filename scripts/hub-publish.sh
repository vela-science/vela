#!/usr/bin/env bash
#
# Publish a frontier through Postgres directly. Optional in v0.7+:
# the preferred path is `vela registry publish --to https://<hub>` which
# POSTs the signed manifest over HTTPS. This script remains for direct-DB
# operations (operator backfills, bulk imports, hubs that aren't reachable
# over HTTPS).
#
# Usage:
#   ./scripts/hub-publish.sh <frontier> <owner-actor-id> <key-path> <locator>
#
# Reads VELA_HUB_DATABASE_URL from ~/.vela/hub.env (or env). The frontier
# must already have <owner-actor-id> registered with the matching pubkey.
#
# Doctrine: this script is a transport adapter, not a substrate change.
# `vela registry publish` produces the canonical signed manifest; this
# script translates that manifest into a Postgres INSERT. Clients verify
# signatures, not server-side state.

set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <frontier.json> <owner-actor-id> <private-key-path> <locator-url>" >&2
  exit 2
fi

FRONTIER="$1"
OWNER="$2"
KEY="$3"
LOCATOR="$4"

# Load credentials from ~/.vela/hub.env if VELA_HUB_DATABASE_URL not in env.
if [[ -z "${VELA_HUB_DATABASE_URL:-}" && -f "$HOME/.vela/hub.env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.vela/hub.env"
fi
if [[ -z "${VELA_HUB_DATABASE_URL:-}" ]]; then
  echo "VELA_HUB_DATABASE_URL not set. Add it to ~/.vela/hub.env or export it." >&2
  exit 1
fi

VELA="${VELA:-./target/release/vela}"
if [[ ! -x "$VELA" ]]; then
  echo "vela binary not found at $VELA — run 'cargo build --release' first." >&2
  exit 1
fi

STAGE_DIR="$(mktemp -d -t vela-hub-stage.XXXXXX)"
STAGE="$STAGE_DIR/registry.json"
trap 'rm -rf "$STAGE_DIR"' EXIT

# 1. Build a signed manifest locally via the existing CLI.
"$VELA" registry publish "$FRONTIER" \
  --owner "$OWNER" \
  --key "$KEY" \
  --locator "$LOCATOR" \
  --to "$STAGE" \
  --json > /dev/null

# 2. Extract the entry as canonical JSON for Postgres insertion.
ENTRY=$(python3 -c "
import json, sys
r = json.load(open('$STAGE'))
print(json.dumps(r['entries'][0], sort_keys=True, ensure_ascii=False, separators=(',',':')))
")
ENTRY_ESC="${ENTRY//\'/\'\'}"

# 3. Insert into Postgres. Latest-publish-wins is at read time
#    (hub serves max(signed_publish_at) per vfr_id); we just append.
psql "$VELA_HUB_DATABASE_URL" -v ON_ERROR_STOP=1 <<SQL
INSERT INTO registry_entries (
  vfr_id, schema, name, owner_actor_id, owner_pubkey,
  latest_snapshot_hash, latest_event_log_hash, network_locator,
  signed_publish_at, signature, raw_json
)
SELECT
  raw->>'vfr_id', raw->>'schema', raw->>'name', raw->>'owner_actor_id', raw->>'owner_pubkey',
  raw->>'latest_snapshot_hash', raw->>'latest_event_log_hash', raw->>'network_locator',
  (raw->>'signed_publish_at')::timestamptz, raw->>'signature', raw
FROM (VALUES ('${ENTRY_ESC}'::jsonb)) AS t(raw);
SQL

VFR_ID=$(python3 -c "import json; print(json.load(open('$STAGE'))['entries'][0]['vfr_id'])")
echo "✓ published $VFR_ID to hub via Postgres"
