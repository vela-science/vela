#!/usr/bin/env bash
#
# Sign the release manifest CI published, and attach the signature to it.
#
# `.github/workflows/release.yml` builds the manifest and deliberately does not
# sign it: putting the distribution identity into Actions would re-couple the
# provider-neutral artifact to the provider it exists to be independent of
# (`docs/SIGNING.md`, "Distribution identity"). So the signature is an operator
# step, and this is it.
#
# It signs the bytes the release already carries. It does not rebuild them, and
# that distinction is the whole reason this script exists rather than
# `scripts/release.sh --sign-key`: a local rebuild produces a different archive —
# different tree, different absolute paths in the debug info, no reproducible
# build claimed anywhere in this repository — so a locally minted manifest
# describes an archive nobody can download. Attaching one makes `install.sh`
# refuse every install, because it compares the published bytes to the digest in
# the manifest and finds they disagree.
#
# usage:
#   scripts/sign-published-release.sh vX.Y.Z ~/.ssh/vela_release_distribution_ed25519.pub
#
# The private half stays in ssh-agent: `ssh-keygen -Y sign -U` takes the public
# key and signs through the agent, so this script never reads private material.
set -euo pipefail

REPO="${VELA_RELEASE_REPOSITORY:-vela-science/vela}"
SIGNATURE_NAMESPACE="vela-release"
SIGNER_IDENTITY="release@vela.space"

die() { echo "sign-published-release: $*" >&2; exit 1; }

TAG="${1:-}"
SIGN_KEY="${2:-${VELA_RELEASE_SIGNING_KEY:-}}"
[ -n "$TAG" ] || die "usage: scripts/sign-published-release.sh <tag> <public-key>"
[ -n "$SIGN_KEY" ] || die "give the distribution identity's PUBLIC key, or set VELA_RELEASE_SIGNING_KEY"
[ -f "$SIGN_KEY" ] || die "no such key file: $SIGN_KEY"
case "$SIGN_KEY" in
  *repository_authority*|*repository-authority*)
    die "refusing to sign a release with the repository-authority key; see docs/SIGNING.md" ;;
esac
command -v gh >/dev/null 2>&1 || die "gh is required to download and upload release assets"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOWED_SIGNERS="$ROOT/allowed_signers"
[ -f "$ALLOWED_SIGNERS" ] || die "no allowed_signers at $ALLOWED_SIGNERS"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# One manifest per built bundle, named after it — the shape `release.yml`
# publishes and the shape `install.sh` fetches. A bare `release-manifest.json`
# is what `scripts/release.sh` writes locally by default, and the installer
# never looks for that name.
mapfile -t MANIFESTS < <(gh release view "$TAG" --repo "$REPO" --json assets \
  --jq '.assets[].name | select(endswith(".release-manifest.json"))')
[ "${#MANIFESTS[@]}" -gt 0 ] || die "$TAG publishes no <asset>.release-manifest.json.
       Releases cut before the manifest existed cannot be signed after the fact;
       the bytes to sign have to be the ones the release carries."

for manifest in "${MANIFESTS[@]}"; do
  echo "== $manifest =="
  gh release download "$TAG" --repo "$REPO" --pattern "$manifest" --dir "$WORK" --clobber

  # Refuse a manifest that already carries a good signature rather than minting
  # a second one: two signatures over the same bytes is not better evidence, and
  # replacing one silently is how a rotation gets lost.
  if gh release view "$TAG" --repo "$REPO" --json assets \
      --jq '.assets[].name' | grep -qxF "${manifest}.sig"; then
    echo "already signed; leaving it alone"
    continue
  fi

  ssh-keygen -Y sign -f "$SIGN_KEY" -U -n "$SIGNATURE_NAMESPACE" "$WORK/$manifest" >/dev/null
  ssh-keygen -Y verify -f "$ALLOWED_SIGNERS" -I "$SIGNER_IDENTITY" \
    -n "$SIGNATURE_NAMESPACE" -s "$WORK/${manifest}.sig" < "$WORK/$manifest" >/dev/null \
    || die "the signature did not verify against $ALLOWED_SIGNERS; is this the published identity?"

  # The tie-back, checked here rather than discovered by the first person to
  # install. A manifest whose digests do not match the assets beside it is worse
  # than no manifest, because `install.sh` trusts the signature and then refuses.
  asset="${manifest%.release-manifest.json}"
  gh release download "$TAG" --repo "$REPO" --pattern "$asset" --dir "$WORK" --clobber
  declared="$(grep -F -A3 "\"name\": \"${asset}\"" "$WORK/$manifest" \
    | grep '"sha256"' | head -1 | sed -E 's/.*"sha256": "(sha256:)?([0-9a-f]{64})".*/\2/')"
  observed="$(shasum -a 256 "$WORK/$asset" | awk '{print $1}')"
  [ "$declared" = "$observed" ] \
    || die "$manifest declares $declared for $asset and the published asset is $observed"
  echo "manifest agrees with the published $asset"

  gh release upload "$TAG" --repo "$REPO" "$WORK/${manifest}.sig"
  echo "uploaded ${manifest}.sig"
done

cat <<EOF

== done ==
Signed by $(ssh-keygen -lf "$SIGN_KEY" -E sha256 | awk '{print $2}')

Verify the way a consumer does, with no gh and no GitHub API:

  VELA_VERSION=$TAG VELA_REQUIRE_SIGNED_MANIFEST=1 sh install.sh
EOF
