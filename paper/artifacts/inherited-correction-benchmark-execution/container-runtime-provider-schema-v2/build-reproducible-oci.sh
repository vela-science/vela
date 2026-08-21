#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: build-reproducible-oci.sh BUILDER ABSOLUTE_OUTPUT_TAR" >&2
  exit 2
fi

builder=$1
output_tar=$2
case "$output_tar" in
  /*) ;;
  *) echo "output tar must be an absolute path" >&2; exit 2 ;;
esac
test ! -e "$output_tar"

runtime_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
docker buildx inspect "$builder" >/dev/null
docker buildx build \
  --builder "$builder" \
  --platform linux/arm64 \
  --no-cache \
  --provenance=false \
  --pull=false \
  --build-arg SOURCE_DATE_EPOCH=1757289600 \
  --output "type=oci,dest=$output_tar,rewrite-timestamp=true" \
  "$runtime_dir"

manifest_digest=$(tar -xOf "$output_tar" index.json | jq -er '.manifests | if length == 1 then .[0].digest else error("manifest_count") end')
manifest_hex=${manifest_digest#sha256:}
config_digest=$(tar -xOf "$output_tar" "blobs/sha256/$manifest_hex" | jq -er '.config.digest')
tar_digest=$(shasum -a 256 "$output_tar" | awk '{print "sha256:" $1}')
jq -cn \
  --arg image_digest "$manifest_digest" \
  --arg config_digest "$config_digest" \
  --arg oci_tar_bytes "$tar_digest" \
  '{image_digest:$image_digest,config_digest:$config_digest,oci_tar_bytes:$oci_tar_bytes,platform:"linux/arm64",source_date_epoch:1757289600}'
