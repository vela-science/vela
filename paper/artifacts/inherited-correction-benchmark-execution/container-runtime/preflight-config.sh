#!/bin/sh
set -eu

if [ "$#" -ne 3 ] || { [ "$1" != "corrected" ] && [ "$1" != "legacy" ]; }; then
  echo "usage: preflight-config.sh corrected|legacy IMAGE_DIGEST EMPTY_OUTPUT_DIR" >&2
  exit 2
fi

mode=$1
image_digest=$2
output_dir=$3
test -d "$output_dir"
test -z "$(find "$output_dir" -mindepth 1 -maxdepth 1 -print -quit)"

legacy_arg=""
if [ "$mode" = "legacy" ]; then
  legacy_arg="--legacy-unsupported"
fi

docker run --rm --network=none --read-only \
  --tmpfs /codex-home:rw,nosuid,size=16m,uid=10001,gid=10001 \
  --mount "type=bind,src=$output_dir,dst=/evidence" \
  --entrypoint node "$image_digest" \
  /opt/vela-runner/strict-preflight.mjs --output /evidence $legacy_arg

test ! -s "$output_dir/provider-events.jsonl"
test "$(jq -r .strict_parse_passed "$output_dir/receipt.json")" = "true"
test "$(jq -r .provider_contact_possible "$output_dir/receipt.json")" = "false"
