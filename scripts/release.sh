#!/usr/bin/env bash
# Build one Vela release bundle from a clean checkout, without a CI provider.
#
# Release semantics used to live only in `.github/workflows/release.yml`, which
# meant they could not be run, read, or reproduced anywhere except inside GitHub
# Actions. This script owns the order of operations; the workflow calls it. The
# workflow keeps only what is genuinely provider-bound:
#
#   * `actions/attest-build-provenance` signs with a GitHub OIDC identity. There
#     is no provider-neutral equivalent, so it stays in the workflow and a local
#     run simply has no provenance attestation. The signed release manifest this
#     script emits is the provider-neutral binding, not a replacement for it.
#   * checkout, artifact upload, and `gh release create` are distribution, not
#     build.
#
# Everything else — version/tag agreement, toolchain channel, the auditable
# build, staging, the SPDX SBOM, the SBOM content check, archiving, checksums,
# the bundle smoke test, and the signed manifest — runs here and runs the same
# way on a laptop with no network.
#
# usage:
#   scripts/release.sh [--tag vX.Y.Z] [--out DIR] [--manifest-name NAME]
#                      [--sign-key PATH] [--require-signature]
#   scripts/release.sh --print-version
#
# environment:
#   VELA_SYFT_BIN               path to a syft binary (default: `syft` on PATH)
#   VELA_RELEASE_SIGNING_KEY    public key file of the distribution signing
#                               identity; the private half stays in ssh-agent
#   VELA_PYTHON                 python interpreter (default: python3, then python)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Pinned build inputs. `SYFT_VERSION` is the same version `release.yml` used to
# request from the `anchore/sbom-action` marketplace Action; asserting it here is
# what let that Action be removed.
CARGO_AUDITABLE_VERSION="0.7.5"
SYFT_VERSION="1.50.0"
MANIFEST_SCHEMA="vela.release-bundle-manifest.v1"
SIGNATURE_NAMESPACE="vela-release"

TAG=""
OUT="$ROOT"
MANIFEST_NAME="release-manifest.json"
SIGN_KEY="${VELA_RELEASE_SIGNING_KEY:-}"
REQUIRE_SIGNATURE=false
PRINT_VERSION=false

die() { echo "release: $*" >&2; exit 1; }
step() { printf '\n== %s ==\n' "$1"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --tag) TAG="${2:?--tag needs a value}"; shift 2 ;;
    --out) OUT="${2:?--out needs a value}"; shift 2 ;;
    --manifest-name) MANIFEST_NAME="${2:?--manifest-name needs a value}"; shift 2 ;;
    --sign-key) SIGN_KEY="${2:?--sign-key needs a value}"; shift 2 ;;
    --require-signature) REQUIRE_SIGNATURE=true; shift ;;
    --print-version) PRINT_VERSION=true; shift ;;
    -h|--help) sed -n '1,30p' "$0"; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

# Signing-key checks run before anything is built. Both of these used to be
# discovered after the compile, the SBOM and the smoke test had already run,
# which is a long way to travel to learn that an argument was wrong.
#
# The distribution signing identity is not the repository-authority key.
# `docs/SIGNING.md` scopes the authority key to attesting that a principal,
# authorization, semantic action, read-set recheck and canonical write matched.
# Publishing a binary is none of those, and one key that signs both makes a
# release indistinguishable from a Decision to anyone checking signatures.
if [ -n "$SIGN_KEY" ]; then
  case "$SIGN_KEY" in
    *repository_authority*|*repository-authority*)
      die "refusing to sign a release with the repository-authority key ($SIGN_KEY); see docs/SIGNING.md"
      ;;
  esac
  [ -f "$SIGN_KEY" ] || die "signing key file not found: $SIGN_KEY"
elif [ "$REQUIRE_SIGNATURE" = true ]; then
  die "--require-signature was given but no signing key was selected"
fi

PYTHON="${VELA_PYTHON:-}"
if [ -z "$PYTHON" ]; then
  PYTHON=python3
  command -v "$PYTHON" >/dev/null 2>&1 || PYTHON=python
fi
command -v "$PYTHON" >/dev/null 2>&1 || die "no python interpreter found"

# 1. Release identity: the version is read from the workspace manifest and the
#    tag has to agree with it. Neither is restated anywhere else.
VERSION="$("$PYTHON" - <<'PY'
import tomllib
with open("Cargo.toml", "rb") as source:
    print(tomllib.load(source)["workspace"]["package"]["version"])
PY
)"
[ -n "$VERSION" ] || die "Cargo.toml declares no workspace version"

if [ "$PRINT_VERSION" = true ]; then
  printf '%s\n' "$VERSION"
  exit 0
fi

if [ -n "$TAG" ] && [ "v$VERSION" != "$TAG" ]; then
  die "tag $TAG does not match the workspace version v$VERSION"
fi

step "release identity"
echo "version: $VERSION${TAG:+ (tag $TAG)}"

# 2. Toolchain channel, read from the pin rather than restated. The published
#    binary and the conformance run must come from one compiler.
CHANNEL="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
[ -n "$CHANNEL" ] || die "rust-toolchain.toml declares no channel"
RUSTC_VERSION="$(rustc --version)"
case "$CHANNEL" in
  [0-9]*)
    observed="$(printf '%s' "$RUSTC_VERSION" | awk '{print $2}')"
    [ "$observed" = "$CHANNEL" ] || die "rustc $observed is not the pinned channel $CHANNEL"
    ;;
esac
echo "toolchain: $CHANNEL ($RUSTC_VERSION)"

TARGET_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
[ -n "$TARGET_TRIPLE" ] || die "rustc reported no host triple"

case "$TARGET_TRIPLE" in
  x86_64-unknown-linux-gnu) ASSET="vela-linux-x86_64.tar.gz" ;;
  aarch64-apple-darwin)     ASSET="vela-macos-aarch64.zip" ;;
  *) die "no release bundle is defined for $TARGET_TRIPLE" ;;
esac
echo "target: $TARGET_TRIPLE -> $ASSET"

# 3. Auditable build. `cargo auditable` embeds the dependency graph in the
#    binary; the SBOM step below recovers it, and `check-sbom.py` fails the
#    release if that recovery came back empty.
step "build"
installed_auditable="$(cargo install --list 2>/dev/null | sed -n 's/^cargo-auditable v\([0-9][^:]*\):$/\1/p' | head -n 1)"
if [ "$installed_auditable" != "$CARGO_AUDITABLE_VERSION" ]; then
  echo "installing cargo-auditable $CARGO_AUDITABLE_VERSION (found: ${installed_auditable:-none})"
  cargo install cargo-auditable --version "$CARGO_AUDITABLE_VERSION" --locked
fi
BUILD_COMMAND="cargo auditable build --locked --release -p vela-cli --bin vela"
$BUILD_COMMAND

# 4. Stage exactly the product bytes. The archive is built from this directory
#    and the SBOM is scanned over it, so the two describe the same tree.
step "stage"
DIST="$OUT/dist"
# Build scratch, so it lands under the already-ignored `target/` rather than
# leaving an untracked directory beside the release assets.
STAGE="$ROOT/target/release-stage"
rm -rf "$STAGE"
mkdir -p "$DIST" "$STAGE"
cp target/release/vela "$STAGE/"
test -x "$STAGE/vela"
echo "staged: $STAGE/vela"

# 5. SPDX SBOM, from syft invoked directly.
#
#    This step was the last marketplace dependency in the release path
#    (`anchore/sbom-action`, which is itself a wrapper that downloads syft).
#    Calling syft directly removes the wrapper and makes the version an
#    assertion rather than an input to somebody else's action.
#
#    The script refuses to substitute a different syft. It will not download one
#    either: a release path that fetches an unpinned binary at build time is the
#    supply-chain dependency `docs/THREAT_MODEL.md` names under "Build
#    compromise", and pinning it would mean carrying a digest this repository
#    cannot compute for itself.
step "sbom"
SYFT="${VELA_SYFT_BIN:-syft}"
if ! command -v "$SYFT" >/dev/null 2>&1; then
  cat >&2 <<EOF
release: syft $SYFT_VERSION is required and was not found.

Install it once, out of band, and pin the version:

  # macOS
  brew install syft && syft --version    # must report $SYFT_VERSION

  # any platform: download the tagged release for your target from
  # https://github.com/anchore/syft/releases/tag/v$SYFT_VERSION
  # and verify it against the checksums published with that tag.

Then re-run, or point VELA_SYFT_BIN at the binary.
EOF
  exit 1
fi
observed_syft="$("$SYFT" --version 2>/dev/null | awk '{print $NF}' | sed 's/^v//')"
[ "$observed_syft" = "$SYFT_VERSION" ] ||
  die "syft $observed_syft is not the pinned $SYFT_VERSION"
SBOM="$DIST/$ASSET.spdx.json"
"$SYFT" scan "dir:$STAGE" -o "spdx-json=$SBOM" --quiet
"$PYTHON" .github/release/check-sbom.py "$SBOM"

# 6. Archive. tar on Linux, ditto on macOS, matching what `install.sh` unpacks.
step "archive"
ARCHIVE="$DIST/$ASSET"
rm -f "$ARCHIVE"
case "$ASSET" in
  *.tar.gz) tar -C "$STAGE" -czf "$ARCHIVE" . ;;
  *.zip)    ditto -c -k --norsrc "$STAGE/" "$ARCHIVE" ;;
  *) die "no archiver for $ASSET" ;;
esac

# 7. Checksums, written the way `install.sh` and `smoke-bundle.sh` read them:
#    relative names, so `shasum -c` works from inside `dist`.
step "checksums"
(
  cd "$DIST"
  shasum -a 256 "$ASSET" > "$ASSET.sha256"
  shasum -a 256 "$ASSET.spdx.json" > "$ASSET.spdx.json.sha256"
  cat "$ASSET.sha256" "$ASSET.spdx.json.sha256"
)

# 8. Bundle smoke test. Unchanged, and deliberately not reimplemented here: it
#    unpacks the archive, runs the binary, initializes a throwaway repository
#    and asserts the current public profile contract.
step "smoke"
.github/release/smoke-bundle.sh "$ARCHIVE" "$VERSION"

# 9. Signed release manifest.
step "manifest"
MANIFEST="$DIST/$MANIFEST_NAME"
manifest_arguments=(
  --out "$MANIFEST"
  --schema "$MANIFEST_SCHEMA"
  --version "$VERSION"
  --toolchain-channel "$CHANNEL"
  --rustc "$RUSTC_VERSION"
  --target-triple "$TARGET_TRIPLE"
  --build-command "$BUILD_COMMAND"
  --cargo-auditable-version "$CARGO_AUDITABLE_VERSION"
  --sbom-tool syft
  --sbom-tool-version "$SYFT_VERSION"
  --binary "$STAGE/vela"
  --asset "archive=$ARCHIVE"
  --asset "sbom=$SBOM"
)
if [ -n "$TAG" ]; then
  manifest_arguments+=(--tag "$TAG")
fi
"$PYTHON" scripts/release_manifest.py "${manifest_arguments[@]}"

if [ -n "$SIGN_KEY" ]; then
  # `-U` signs through ssh-agent and takes the *public* key, so this script
  # never reads private key material — the same custody rule `vela` itself uses.
  ssh-keygen -Y sign -f "$SIGN_KEY" -U -n "$SIGNATURE_NAMESPACE" "$MANIFEST" >/dev/null
  echo "signed: $MANIFEST.sig (namespace $SIGNATURE_NAMESPACE)"
  echo "signer: $(ssh-keygen -lf "$SIGN_KEY" -E sha256 | awk '{print $2}')"
  ( cd "$DIST" && shasum -a 256 "$MANIFEST_NAME" > "$MANIFEST_NAME.sha256" )
else
  cat <<EOF
manifest is unsigned. To sign it, load the distribution identity into ssh-agent
and re-run with its public key:

  ssh-add ~/.ssh/vela_release_distribution_ed25519
  scripts/release.sh --sign-key ~/.ssh/vela_release_distribution_ed25519.pub

Verify a signed manifest with OpenSSH alone:

  ssh-keygen -Y verify -f allowed_signers -I <signer> \\
    -n $SIGNATURE_NAMESPACE -s $MANIFEST_NAME.sig < $MANIFEST_NAME
EOF
fi

step "done"
ls -1 "$DIST"
