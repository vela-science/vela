#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: preflight-provider-schema.sh IMAGE_DIGEST INPUT_DIR OUTPUT_DIR" >&2
  exit 2
fi

image_digest=$1
input_dir=$2
output_dir=$3
test -d "$input_dir"
test -d "$output_dir"
test -z "$(find "$output_dir" -mindepth 1 -maxdepth 1 -print -quit)"
: > "$output_dir/provider-events.jsonl"
docker run --rm --network=none --read-only \
  --mount "type=bind,src=$input_dir,dst=/input,readonly" \
  --entrypoint node "$image_digest" /opt/vela-runner/provider-schema-preflight.mjs \
  > "$output_dir/receipt.json" 2> "$output_dir/stderr.txt"
test ! -s "$output_dir/provider-events.jsonl"
