#!/usr/bin/env bash
# Seed a witness mirror by replaying owner-signed registrations.
#
# The registration document (vela.frontier-git-remote.v0.1) is signed
# over its payload, not a session — so the SAME document the primary
# verified verifies on any hub. An empty mirror's owner check passes by
# the bootstrap doctrine (the signature IS the ownership claim). No new
# key ceremony: this script moves existing signatures, it mints nothing.
#
# Usage: scripts/seed-witness.sh [primary-url] [witness-url]
set -euo pipefail

PRIMARY="${1:-https://hub.constellate.science}"
WITNESS="${2:-https://vela-hub-witness.fly.dev}"

echo "seeding $WITNESS from $PRIMARY"
vfrs=$(curl -fsS "$PRIMARY/entries" | python3 -c "
import sys, json
for e in json.load(sys.stdin).get('entries', []):
    print(e.get('vfr_id') or e.get('frontier_id') or '')
" | grep -v '^$')

ok=0; skipped=0; failed=0
for vfr in $vfrs; do
  reg=$(curl -fsS "$PRIMARY/entries/$vfr/git-remote" 2>/dev/null \
    | python3 -c "
import sys, json
d = json.load(sys.stdin)
r = (d.get('git') or {}).get('registration')
print(json.dumps(r) if r else '')" || true)
  if [ -z "$reg" ] || [ "$reg" = "null" ]; then
    echo "  $vfr: no registration on the primary — skipped"
    skipped=$((skipped + 1))
    continue
  fi
  code=$(curl -s -o /tmp/seed-resp.json -w "%{http_code}" \
    -X POST -H "Content-Type: application/json" \
    -d "$reg" "$WITNESS/entries/$vfr/git-remote")
  if [ "$code" = "200" ] || [ "$code" = "201" ]; then
    echo "  $vfr: replayed"
    ok=$((ok + 1))
  else
    echo "  $vfr: REFUSED ($code): $(cat /tmp/seed-resp.json)"
    failed=$((failed + 1))
  fi
done
echo "seeded: $ok replayed, $skipped skipped, $failed refused"
[ "$failed" -eq 0 ]
