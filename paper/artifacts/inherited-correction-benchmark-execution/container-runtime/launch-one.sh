#!/bin/sh
set -eu

if [ "$#" -ne 6 ] || [ "$1" != "--run-id" ]; then
  echo "usage: launch-one.sh --run-id EXACT_RUN_ID IMAGE_DIGEST INPUT_DIR PERMIT_DIR EVIDENCE_DIR" >&2
  exit 2
fi

run_id=$2
image_digest=$3
input_dir=$4
permit_dir=$5
evidence_dir=$6

case "$run_id" in *[!a-z0-9-]*|'') echo "invalid run_id" >&2; exit 2;; esac
test -f "$permit_dir/$run_id.permit.json"
test ! -e "$permit_dir/$run_id.permit.consumed.json"
test -d "$input_dir"
test -d "$evidence_dir"
test -z "$(find "$evidence_dir" -mindepth 1 -maxdepth 1 -print -quit)"

auth_file=${CODEX_AUTH_FILE:-/Users/williamblair/.codex/auth.json}
test -f "$auth_file"

container_name="vela-inherited-$run_id"
if docker container inspect "$container_name" >/dev/null 2>&1; then
  echo "container name already exists" >&2
  exit 2
fi

set +e
docker run --rm --name "$container_name" --init --network=bridge --read-only --workdir /work \
  --tmpfs /tmp:rw,noexec,nosuid,size=32m \
  --tmpfs /work:rw,noexec,nosuid,size=16m,uid=10001,gid=10001 \
  --tmpfs /codex-home:rw,noexec,nosuid,size=16m,uid=10001,gid=10001 \
  --mount "type=bind,src=$auth_file,dst=/codex-home/auth.json,readonly" \
  --mount "type=bind,src=$input_dir,dst=/input,readonly" \
  --mount "type=bind,src=$permit_dir,dst=/permit" \
  --mount "type=bind,src=$evidence_dir,dst=/evidence" \
  --env "QUALIFIED_IMAGE_DIGEST=$image_digest" \
  "$image_digest" --run-id "$run_id"
status=$?
set -e
if docker container inspect "$container_name" >/dev/null 2>&1; then
  echo "container teardown failed" >&2
  exit 2
fi
exit "$status"
