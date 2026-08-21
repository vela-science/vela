#!/bin/sh
set -eu

if [ "$#" -ne 4 ] || { [ "$1" != "positive" ] && [ "$1" != "missing" ] && [ "$1" != "corrupt" ]; }; then
  echo "usage: preflight-trust.sh positive|missing|corrupt IMAGE_DIGEST TRUST_BUNDLE_SHA256 EMPTY_OUTPUT_DIR" >&2
  exit 2
fi

mode=$1
image_digest=$2
trust_bundle_sha256=$3
output_dir=$4
test -d "$output_dir"
test -z "$(find "$output_dir" -mindepth 1 -maxdepth 1 -print -quit)"

docker run --rm --network=none --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=16m,uid=10001,gid=10001 \
  --mount "type=bind,src=$output_dir,dst=/evidence" \
  --env "EXPECTED_TRUST_BUNDLE_SHA256=$trust_bundle_sha256" \
  --entrypoint node "$image_digest" \
  /opt/vela-runner/trust-preflight.mjs "$mode" --output /evidence

test ! -s "$output_dir/provider-events.jsonl"
test "$(jq -r .trust_check_passed "$output_dir/receipt.json")" = "true"
test "$(jq -r .provider_contact_possible "$output_dir/receipt.json")" = "false"
