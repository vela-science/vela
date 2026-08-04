#!/bin/sh
set -eu

if [ "$#" -lt 1 ]; then
  echo "lean4export wrapper: missing module" >&2
  exit 2
fi

module="$1"
shift
exec /work/.lake/packages/lean4export/.lake/build/bin/lean4export \
  "$module" -- "$@"
