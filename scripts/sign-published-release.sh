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
# The release is a DRAFT when this runs, and publishing it is the last thing
# this script does. A published release in this repository is immutable, which
# is correct for a scientific artifact and refuses new assets outright, so the
# signature has to go on before that door closes. `release.yml` therefore leaves
# the release unpublished and invisible, and a consumer never sees the unsigned
# intermediate state.
#
# It signs the bytes the release already carries. It does not rebuild them, and
# that distinction is the whole reason this script exists rather than
# `scripts/release.sh --sign-key`: a local rebuild produces a different archive —
# different source or toolchain inputs — so a locally minted manifest may
# describe an archive nobody can download. `release.sh` now proves two binary
# builds and two archive passes agree, but signing still covers only the exact
# bytes the draft already carries. Attaching a manifest from any other build
# makes `install.sh` refuse the install.
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
PYTHON="${VELA_PYTHON:-python3}"
command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required to validate release manifests"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALLOWED_SIGNERS="$ROOT/allowed_signers"
[ -f "$ALLOWED_SIGNERS" ] || die "no allowed_signers at $ALLOWED_SIGNERS"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Checked as a status first, then as a value. Reading the value inside an `if`
# condition suppresses errexit, so a `gh` that failed for any reason — an unknown
# tag, no network, an expired token — produced an empty string that compared
# unequal to "true" and was reported as "already published", sending the operator
# to cut a version they did not need.
if ! draft_state="$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq '.isDraft' 2>"$WORK/gh.err")"; then
  die "cannot read $TAG from $REPO: $(tr -d '\n' < "$WORK/gh.err")"
fi
if [ "$draft_state" != "true" ]; then
  die "$TAG is already published, and a published release here is immutable — it
       refuses new assets with 422. Signing has to happen while the release is
       still a draft. Cut the next patch version, which release.yml now leaves
       as a draft for exactly this."
fi

if ! tag_commit="$(gh api "repos/$REPO/commits/$TAG" --jq '.sha' 2>"$WORK/tag.err")"; then
  die "cannot resolve $TAG to a commit in $REPO: $(tr -d '\n' < "$WORK/tag.err")"
fi
[ -n "$tag_commit" ] || die "$TAG resolved to no commit"

# One manifest per built bundle, named after it — the shape `release.yml`
# publishes and the shape `install.sh` fetches. A bare `release-manifest.json`
# is what `scripts/release.sh` writes locally by default, and the installer
# never looks for that name.
MANIFESTS=()
while IFS= read -r manifest; do
  MANIFESTS+=("$manifest")
done < <(gh release view "$TAG" --repo "$REPO" --json assets \
  --jq '.assets[].name | select(endswith(".release-manifest.json"))')
[ "${#MANIFESTS[@]}" -gt 0 ] || die "$TAG publishes no <asset>.release-manifest.json.
       Releases cut before the manifest existed cannot be signed after the fact;
       the bytes to sign have to be the ones the release carries."

# Publishing is irreversible. Require the complete supported target set here,
# even though release.yml already checks it before creating the draft: an
# operator can otherwise sign and publish a manually altered or partial draft.
EXPECTED_MANIFESTS=(
  "vela-linux-x86_64.tar.gz.release-manifest.json"
  "vela-macos-aarch64.zip.release-manifest.json"
)
[ "${#MANIFESTS[@]}" -eq "${#EXPECTED_MANIFESTS[@]}" ] \
  || die "$TAG has ${#MANIFESTS[@]} release manifests; expected exactly ${#EXPECTED_MANIFESTS[@]}"
for expected in "${EXPECTED_MANIFESTS[@]}"; do
  found=""
  for manifest in "${MANIFESTS[@]}"; do
    [ "$manifest" = "$expected" ] && found="yes"
  done
  [ -n "$found" ] || die "$TAG is missing required manifest $expected"
done

for manifest in "${MANIFESTS[@]}"; do
  echo "== $manifest =="
  gh release download "$TAG" --repo "$REPO" --pattern "$manifest" --dir "$WORK" --clobber

  manifest_inventory="$WORK/${manifest}.inventory"
  "$PYTHON" - "$WORK/$manifest" "$TAG" "$manifest" > "$manifest_inventory" <<'PY'
import json
import re
import sys

path, tag, manifest_name = sys.argv[1:]
with open(path, encoding="utf-8") as source:
    manifest = json.load(source)
if manifest.get("schema") != "vela.release-bundle-manifest.v1":
    raise SystemExit("release manifest has the wrong schema")
release = manifest.get("release", {})
if release.get("tag") != tag or release.get("version") != tag.removeprefix("v"):
    raise SystemExit("release manifest tag/version does not match the draft")
source = manifest.get("source", {})
if source.get("available") is not True or source.get("dirty") is not False:
    raise SystemExit("release manifest does not bind a clean Git source")
commit = source.get("commit")
if not isinstance(commit, str) or re.fullmatch(r"[0-9a-f]{40}", commit) is None:
    raise SystemExit("release manifest has no exact source commit")
reproducibility = manifest.get("build", {}).get("reproducibility", {})
if reproducibility.get("binary_builds_compared", 0) < 2:
    raise SystemExit("release manifest lacks a two-build binary comparison")
if reproducibility.get("archive_builds_compared", 0) < 2:
    raise SystemExit("release manifest lacks a two-pass archive comparison")
if not isinstance(reproducibility.get("source_date_epoch"), int):
    raise SystemExit("release manifest has no SOURCE_DATE_EPOCH")
assets = manifest.get("assets")
if not isinstance(assets, list) or not assets:
    raise SystemExit("release manifest lists no assets")
bundle_name = manifest_name.removesuffix(".release-manifest.json")
expected_assets = {
    ("archive", bundle_name),
    ("sbom", bundle_name + ".spdx.json"),
}
observed_assets = {(asset.get("kind"), asset.get("name")) for asset in assets}
if observed_assets != expected_assets:
    raise SystemExit(
        f"release manifest assets {sorted(observed_assets)!r} do not match "
        f"the required bundle and SBOM {sorted(expected_assets)!r}"
    )
seen = set()
print("COMMIT", commit, sep="\t")
for asset in assets:
    name, digest = asset.get("name"), asset.get("sha256")
    if not isinstance(name, str) or "/" in name or name in seen:
        raise SystemExit("release manifest has an unsafe or duplicate asset name")
    if not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None:
        raise SystemExit(f"release manifest has an invalid digest for {name}")
    seen.add(name)
    print("ASSET", name, digest.removeprefix("sha256:"), sep="\t")
PY

  manifest_commit="$(awk -F '\t' '$1 == "COMMIT" {print $2}' "$manifest_inventory")"
  [ "$manifest_commit" = "$tag_commit" ] \
    || die "$manifest names source commit $manifest_commit but $TAG resolves to $tag_commit"

  # A manifest that already carries a signature is not re-signed — two
  # signatures over the same bytes is not better evidence, and replacing one
  # silently is how a rotation gets lost. But it is still VERIFIED. Skipping
  # outright trusted a sidecar on its filename alone and then published the
  # release irreversibly, so a `.sig` uploaded by hand, left from a failed run,
  # or made by the wrong key would have been sealed in unchecked.
  already_signed=""
  if gh release view "$TAG" --repo "$REPO" --json assets \
      --jq '.assets[].name' | grep -qxF "${manifest}.sig"; then
    already_signed="yes"
    gh release download "$TAG" --repo "$REPO" --pattern "${manifest}.sig" --dir "$WORK" --clobber
    echo "already signed; verifying the existing signature rather than replacing it"
  fi

  if [ -z "$already_signed" ]; then
    ssh-keygen -Y sign -f "$SIGN_KEY" -U -n "$SIGNATURE_NAMESPACE" "$WORK/$manifest" >/dev/null
  fi
  ssh-keygen -Y verify -f "$ALLOWED_SIGNERS" -I "$SIGNER_IDENTITY" \
    -n "$SIGNATURE_NAMESPACE" -s "$WORK/${manifest}.sig" < "$WORK/$manifest" >/dev/null \
    || die "the signature did not verify against $ALLOWED_SIGNERS; is this the published identity?"

  # Tie the signature to every archive and SBOM the manifest names, not only to
  # the archive the installer happens to select on this machine.
  while IFS=$'\t' read -r kind asset declared; do
    [ "$kind" = "ASSET" ] || continue
    gh release download "$TAG" --repo "$REPO" --pattern "$asset" --dir "$WORK" --clobber
    observed="$(shasum -a 256 "$WORK/$asset" | awk '{print $1}')"
    [ "$declared" = "$observed" ] \
      || die "$manifest declares $declared for $asset and the published asset is $observed"
    echo "manifest agrees with the published $asset"
  done < "$manifest_inventory"

  if [ -z "$already_signed" ]; then
    gh release upload "$TAG" --repo "$REPO" "$WORK/${manifest}.sig"
    echo "uploaded ${manifest}.sig"
  fi
done

# Publishing is what makes it immutable, and it happens only once every manifest
# is signed and every digest checked. A release that fails any check above stays
# a draft, which is the reversible state.
gh release edit "$TAG" --repo "$REPO" --draft=false
echo "published $TAG"

cat <<EOF

== done ==
Signed by $(ssh-keygen -lf "$SIGN_KEY" -E sha256 | awk '{print $2}')

Verify the way a consumer does, with no gh and no GitHub API:

  VELA_VERSION=$TAG VELA_REQUIRE_SIGNED_MANIFEST=1 bash install.sh
EOF
