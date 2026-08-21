#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: verify-reproducible-oci.sh FIRST_TAR SECOND_TAR EXPECTED_IMAGE_DIGEST" >&2
  exit 2
fi

first=$1
second=$2
expected=$3
test -f "$first"
test -f "$second"
expected_hex=${expected#sha256:}
if [ "$expected_hex" = "$expected" ] || [ "${#expected_hex}" -ne 64 ]; then
  echo "invalid expected digest" >&2
  exit 2
fi
case "$expected_hex" in *[!0-9a-f]*) echo "invalid expected digest" >&2; exit 2;; esac

cmp "$first" "$second"
for image_tar in "$first" "$second"; do
  manifest_digest=$(tar -xOf "$image_tar" index.json | jq -er '.manifests | if length == 1 then .[0].digest else error("manifest_count") end')
  test "$manifest_digest" = "$expected"
done

tar_digest=$(shasum -a 256 "$first" | awk '{print "sha256:" $1}')
jq -cn \
  --arg image_digest "$expected" \
  --arg oci_tar_bytes "$tar_digest" \
  '{cache_independent:true,byte_identical:true,image_digest:$image_digest,oci_tar_bytes:$oci_tar_bytes}'
