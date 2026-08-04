#!/bin/bash
set -euo pipefail

if (( $# == 0 )); then
  mapfile -t profiles < <(find ComparatorChallenges -maxdepth 1 -name '*.json' -print | sort)
else
  profiles=("$@")
fi

git diff --exit-code
test "$(git rev-parse HEAD)" = "29362184c2b698c1b279bc85b3957ee813646c63"
test "$(git rev-parse HEAD^{tree})" = "730bf2c6a13dbb96606024c5fd681a48633fb393"
test "$(id -u)" = "10001"

for profile in "${profiles[@]}"; do
  printf 'ASTRA_PROFILE_BEGIN %s\n' "$profile"
  lake exe comparator "$profile"
  printf 'ASTRA_PROFILE_PASS %s\n' "$profile"
done
