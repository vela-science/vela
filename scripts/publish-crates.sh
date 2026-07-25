#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-check}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(python3 - <<'PY'
import tomllib
with open("Cargo.toml", "rb") as source:
    print(tomllib.load(source)["workspace"]["package"]["version"])
PY
)"
TAG="v$VERSION"
CRATES=(
  vela-protocol-core
  vela-verify
  vela-protocol
  vela-authority
  vela-signer
  vela-edge
  vela-cli
)
CRATES_API_USER_AGENT="vela-release-publisher/$VERSION (https://github.com/vela-science/vela)"

preflight_package_graph() {
  # Cargo 1.91+ assembles and verifies unpublished workspace dependencies in a
  # temporary local registry. This proves the complete seven-crate graph before
  # the first immutable upload.
  cargo package --workspace --locked
}

crate_exists() {
  local crate="$1"
  curl -A "$CRATES_API_USER_AGENT" -fsS \
    "https://crates.io/api/v1/crates/$crate/$VERSION" >/dev/null 2>&1
}

case "$MODE" in
  check)
    preflight_package_graph
    printf '%s\n' "Complete seven-crate package graph is publishable."
    ;;
  --execute)
    [[ "$VERSION" != *-* ]] || {
      printf '%s\n' "refusing to publish prerelease workspace version $VERSION" >&2
      exit 1
    }
    test -z "$(git status --short)" || {
      printf '%s\n' "refusing crates publication from a dirty worktree" >&2
      exit 1
    }
    test "$(git branch --show-current)" = main || {
      printf '%s\n' "refusing crates publication outside main" >&2
      exit 1
    }
    test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" || {
      printf '%s\n' "refusing crates publication before origin/main matches HEAD" >&2
      exit 1
    }
    test "$(git rev-parse "$TAG^{commit}")" = "$(git rev-parse HEAD)" || {
      printf '%s\n' "refusing crates publication unless local $TAG points to HEAD" >&2
      exit 1
    }

    preflight_package_graph

    for crate in "${CRATES[@]}"; do
      if crate_exists "$crate"; then
        printf '%s\n' "$crate $VERSION already exists; preserving immutable registry bytes"
        continue
      fi
      cargo publish --locked -p "$crate"
      # Dependent package publication needs the preceding immutable version in
      # the registry index. Bound the wait and never republish an existing crate.
      ready=false
      for _ in $(seq 1 24); do
        if crate_exists "$crate"; then
          ready=true
          break
        fi
        sleep 5
      done
      "$ready" || {
        printf '%s\n' "$crate $VERSION was uploaded but did not become readable within 120 seconds" >&2
        exit 1
      }
    done
    ;;
  *)
    printf '%s\n' "usage: scripts/publish-crates.sh [check|--execute]" >&2
    exit 2
    ;;
esac
