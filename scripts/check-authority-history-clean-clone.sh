#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/vela-authority-history.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
git clone --quiet --no-local --no-hardlinks "$ROOT" "$TMP/vela"
git -C "$TMP/vela" checkout --quiet --detach "$COMMIT"

if [[ -n "$(git -C "$TMP/vela" status --short)" ]]; then
  printf 'clean-clone authority replay: clone is dirty\n' >&2
  exit 1
fi

authority_run=(env PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify.py --authority-history-only)
capability_run=(env PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify_principal_capability.py)
case "$(uname -s)" in
  Darwin)
    if ! command -v sandbox-exec >/dev/null 2>&1; then
      printf 'clean-clone authority replay: sandbox-exec is required on macOS\n' >&2
      exit 1
    fi
    (
      cd "$TMP/vela"
      sandbox-exec -p '(version 1) (allow default) (deny network*)' "${authority_run[@]}"
      sandbox-exec -p '(version 1) (allow default) (deny network*)' "${capability_run[@]}"
    )
    ;;
  Linux)
    if command -v bwrap >/dev/null 2>&1; then
      bwrap \
        --unshare-net \
        --ro-bind / / \
        --dev-bind /dev /dev \
        --proc /proc \
        --tmpfs /tmp \
        --chdir "$TMP/vela" \
        "${authority_run[@]}"
      bwrap \
        --unshare-net \
        --ro-bind / / \
        --dev-bind /dev /dev \
        --proc /proc \
        --tmpfs /tmp \
        --chdir "$TMP/vela" \
        "${capability_run[@]}"
    elif command -v unshare >/dev/null 2>&1 && unshare -n true 2>/dev/null; then
      (cd "$TMP/vela" && unshare -n "${authority_run[@]}")
      (cd "$TMP/vela" && unshare -n "${capability_run[@]}")
    else
      printf 'clean-clone authority replay: bwrap or usable unshare -n is required on Linux\n' >&2
      exit 1
    fi
    ;;
  *)
    printf 'clean-clone authority replay: unsupported platform %s\n' "$(uname -s)" >&2
    exit 1
    ;;
esac

printf 'clean-clone authority foundation: ok (%s, network denied)\n' "$COMMIT"
