#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: preflight-schema.sh IMAGE_DIGEST OUTPUT_DIR" >&2
  exit 2
fi

image_digest=$1
output_dir=$2
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
schema_dir=$(dirname -- "$script_dir")

test -d "$output_dir"
test -z "$(find "$output_dir" -mindepth 1 -maxdepth 1 -print -quit)"
: > "$output_dir/provider-events.jsonl"
docker run --rm --network=none --read-only \
  --mount "type=bind,src=$schema_dir/response-schema.json,dst=/input/response-schema.json,readonly" \
  --entrypoint node "$image_digest" /opt/vela-runner/schema-preflight.mjs \
  > "$output_dir/receipt.json" 2> "$output_dir/stderr.txt"
test ! -s "$output_dir/provider-events.jsonl"
