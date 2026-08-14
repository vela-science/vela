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
# two-build binary comparison, deterministic SBOM normalization, deterministic
# archiving, the bundle smoke test, and the signed manifest — runs here and
# runs the same way outside CI.
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
REMAP_SOURCE_PREFIX="/src/vela"
REMAP_TARGET_PREFIX="/build/target"
REMAP_CARGO_HOME_PREFIX="/build/cargo-home"
REMAP_ACCOUNT_HOME_PREFIX="/build/account-home"

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

# Rust retains source paths for panic locations and debug metadata. Resolve the
# two ambient homes Cargo can read before building so dependency paths receive
# the same stable public treatment as the repository and target directories.
ACCOUNT_HOME_RESOLVED="$("$PYTHON" - <<'PY'
import os

home = os.environ.get("HOME")
if not home:
    raise SystemExit("HOME is required to resolve the release builder account")
print(os.path.realpath(home))
PY
)"
CARGO_HOME_RESOLVED="$("$PYTHON" - <<'PY'
import os

home = os.environ.get("HOME")
if not home:
    raise SystemExit("HOME is required to resolve Cargo home")
cargo_home = os.environ.get("CARGO_HOME") or os.path.join(home, ".cargo")
print(os.path.realpath(cargo_home))
PY
)"
[ "$ACCOUNT_HOME_RESOLVED" != "/" ] || die "refusing release build with account home /"
[ "$CARGO_HOME_RESOLVED" != "/" ] || die "refusing release build with Cargo home /"

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

# A signature says "these bytes are the release". Refuse to make that claim from
# a tree that is not the release.
#
# The version comes from Cargo.toml and the bytes come from the working tree,
# and nothing tied the two together. So running this on a checkout 42 commits
# past `v0.967.0`, with the version not yet bumped, produced a correctly signed
# manifest for an archive whose digest matches no published release — and
# attaching it to `v0.967.0` would have made `install.sh` refuse every install,
# because the digest tie-back would compare the published archive against the
# locally built one and find they differ. That is the tie-back working, arriving
# far too late to be useful.
#
# Only checked when signing. An unsigned build from a dirty tree is an ordinary
# thing to do while working.
if [ -n "$SIGN_KEY" ] && git rev-parse --git-dir >/dev/null 2>&1; then
  if [ -n "$(git status --porcelain --untracked-files=normal)" ]; then
    die "refusing to sign a release built from a modified or untracked working tree; commit or stash first"
  fi
  release_tag="${TAG:-v$VERSION}"
  tagged="$(git rev-parse --verify -q "${release_tag}^{commit}" 2>/dev/null)" \
    || die "refusing to sign before ${release_tag} exists and names the release commit"
  head="$(git rev-parse HEAD)"
  if [ "$tagged" != "$head" ]; then
    behind="$(git rev-list --count "${release_tag}..HEAD" 2>/dev/null || echo '?')"
    die "HEAD is ${behind} commits from ${release_tag}, so these bytes are not that release.
       Bump the version and tag this commit, or check out ${release_tag} to rebuild it.
       Signing here would attest to an archive no release carries; \`install.sh\` compares
       the published bytes to the manifest and would refuse every install."
  fi
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

# A release timestamp is source state, not wall-clock state. It controls archive
# metadata and the manifest timestamp. Clean Git checkouts derive it from the
# release commit; source archives must supply the retained value explicitly.
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-}"
if [ -z "$SOURCE_DATE_EPOCH" ]; then
  SOURCE_DATE_EPOCH="$(git log -1 --format=%ct HEAD 2>/dev/null || true)"
fi
case "$SOURCE_DATE_EPOCH" in
  ''|*[!0-9]*) die "SOURCE_DATE_EPOCH must be a nonnegative integer (or build from a Git checkout)" ;;
esac
export SOURCE_DATE_EPOCH
echo "source date epoch: $SOURCE_DATE_EPOCH"

# 3. Auditable build. `cargo auditable` embeds the dependency graph in the
#    binary; the SBOM step below recovers it, and `check-sbom.py` fails the
#    release if that recovery came back empty.
step "build twice"
installed_auditable="$(cargo install --list 2>/dev/null | sed -n 's/^cargo-auditable v\([0-9][^:]*\):$/\1/p' | head -n 1)"
if [ "$installed_auditable" != "$CARGO_AUDITABLE_VERSION" ]; then
  echo "installing cargo-auditable $CARGO_AUDITABLE_VERSION (found: ${installed_auditable:-none})"
  cargo install cargo-auditable --version "$CARGO_AUDITABLE_VERSION" --locked
fi
BUILD_COMMAND="cargo auditable build --locked --release -p vela-cli --bin vela"
BUILD_ONE="$ROOT/target/release-build/one"
BUILD_TWO="$ROOT/target/release-build/two"
rm -rf "$BUILD_ONE" "$BUILD_TWO"

build_release() {
  local target_dir="$1"
  # rustc gives a later matching remap precedence, so put the account home
  # first and its specific Cargo, source, and target descendants after it. Both
  # target directories map to one retained path, and dependency panic locations
  # retain only the stable public Cargo prefix.
  local remap_flags="--remap-path-prefix=$ACCOUNT_HOME_RESOLVED=$REMAP_ACCOUNT_HOME_PREFIX --remap-path-prefix=$CARGO_HOME_RESOLVED=$REMAP_CARGO_HOME_PREFIX --remap-path-prefix=$ROOT=$REMAP_SOURCE_PREFIX --remap-path-prefix=$target_dir=$REMAP_TARGET_PREFIX"
  CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$target_dir" \
    RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }$remap_flags" \
    cargo auditable build --locked --release -p vela-cli --bin vela
}

build_release "$BUILD_ONE"
build_release "$BUILD_TWO"
cmp "$BUILD_ONE/release/vela" "$BUILD_TWO/release/vela" \
  || die "two clean release builds produced different vela binaries"
echo "binary reproducibility: two independent target directories agree"

# 4. Stage exactly the product bytes. The archive is built from this directory
#    and the SBOM is scanned over it, so the two describe the same tree.
step "stage"
DIST="$OUT/dist"
# Build scratch, so it lands under the already-ignored `target/` rather than
# leaving an untracked directory beside the release assets.
STAGE="$ROOT/target/release-stage"
rm -rf "$STAGE"
mkdir -p "$DIST" "$STAGE"
cp "$BUILD_ONE/release/vela" "$STAGE/"
test -x "$STAGE/vela"

refuse_private_path_bytes() {
  local artifact="$1"
  local path="$2"
  local label="$3"
  if LC_ALL=C grep -aFq -- "$path" "$artifact"; then
    die "release artifact retains the private $label path"
  fi
}

refuse_private_path_bytes "$STAGE/vela" "$ROOT" "source root"
refuse_private_path_bytes "$STAGE/vela" "$BUILD_ONE" "first target directory"
refuse_private_path_bytes "$STAGE/vela" "$BUILD_TWO" "second target directory"
refuse_private_path_bytes "$STAGE/vela" "$CARGO_HOME_RESOLVED" "Cargo home"
refuse_private_path_bytes "$STAGE/vela" "$ACCOUNT_HOME_RESOLVED" "account home"
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

Install it once, out of band, at that exact version:

  # any platform, and the only instruction that stays correct: download the
  # tagged release for your target from
  # https://github.com/anchore/syft/releases/tag/v$SYFT_VERSION
  # and verify it against the checksums published with that tag.

  # macOS, only while Homebrew's stable happens to be v$SYFT_VERSION:
  brew install syft && syft --version    # must report $SYFT_VERSION

Homebrew tracks whatever syft released most recently, so the second recipe
stops working the day Anchore ships a newer version, and a package manager
that installs "the current one" cannot satisfy a pin by construction. Use the
tagged download unless brew already agrees.

Then re-run, or point VELA_SYFT_BIN at the binary.
EOF
  exit 1
fi
observed_syft="$("$SYFT" --version 2>/dev/null | awk '{print $NF}' | sed 's/^v//')"
[ "$observed_syft" = "$SYFT_VERSION" ] ||
  die "syft $observed_syft is not the pinned $SYFT_VERSION"
SBOM="$DIST/$ASSET.spdx.json"
SBOM_SCRATCH="$ROOT/target/release-sbom-check"
SBOM_RAW_ONE="$SBOM_SCRATCH/one.raw.spdx.json"
SBOM_RAW_TWO="$SBOM_SCRATCH/two.raw.spdx.json"
SBOM_CHECK="$SBOM_SCRATCH/canonical.spdx.json"
rm -rf "$SBOM_SCRATCH"
mkdir -p "$SBOM_SCRATCH"
"$SYFT" scan "dir:$STAGE" -o "spdx-json=$SBOM_RAW_ONE" --quiet
"$SYFT" scan "dir:$STAGE" -o "spdx-json=$SBOM_RAW_TWO" --quiet
SBOM_CREATED="$("$PYTHON" - "$SOURCE_DATE_EPOCH" <<'PY'
import datetime as dt
import sys

print(dt.datetime.fromtimestamp(int(sys.argv[1]), dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"))
PY
)"
SBOM_NAME="Vela $VERSION $TARGET_TRIPLE release bundle"
SBOM_NAMESPACE="https://vela.science/spdx/vela/$VERSION/$TARGET_TRIPLE"
SBOM_ROOT_NAME="vela-$VERSION-$TARGET_TRIPLE"
SBOM_ROOT_ID="SPDXRef-DocumentRoot-Vela-${VERSION//./-}-$TARGET_TRIPLE"
canonicalize_sbom() {
  "$PYTHON" .github/release/check-sbom.py --canonicalize \
    --input "$1" --output "$2" \
    --name "$SBOM_NAME" --namespace "$SBOM_NAMESPACE" \
    --created "$SBOM_CREATED" --root-name "$SBOM_ROOT_NAME" \
    --root-id "$SBOM_ROOT_ID"
}
canonicalize_sbom "$SBOM_RAW_ONE" "$SBOM"
canonicalize_sbom "$SBOM_RAW_TWO" "$SBOM_CHECK"
cmp "$SBOM" "$SBOM_CHECK" \
  || die "two independently generated SBOMs produced different canonical bytes"
"$PYTHON" .github/release/check-sbom.py "$SBOM"
refuse_private_path_bytes "$SBOM" "$STAGE" "SBOM stage directory"
refuse_private_path_bytes "$SBOM" "$ROOT" "SBOM source root"
refuse_private_path_bytes "$SBOM" "$BUILD_ONE" "SBOM first target directory"
refuse_private_path_bytes "$SBOM" "$BUILD_TWO" "SBOM second target directory"
refuse_private_path_bytes "$SBOM" "$CARGO_HOME_RESOLVED" "SBOM Cargo home"
refuse_private_path_bytes "$SBOM" "$ACCOUNT_HOME_RESOLVED" "SBOM account home"
echo "SBOM reproducibility: two independent Syft scans agree after deterministic normalization"

# 6. Deterministic archive, built twice from the same staged bytes. The helper
#    fixes path order, ownership, modes, and timestamps for both published
#    formats; `cmp` makes reproducibility an actual release gate.
step "archive"
ARCHIVE="$DIST/$ASSET"
ARCHIVE_CHECK="$ROOT/target/release-archive-check/$ASSET"
rm -f "$ARCHIVE" "$ARCHIVE_CHECK"
"$PYTHON" .github/release/create-deterministic-archive.py \
  --source "$STAGE" --output "$ARCHIVE" --epoch "$SOURCE_DATE_EPOCH"
"$PYTHON" .github/release/create-deterministic-archive.py \
  --source "$STAGE" --output "$ARCHIVE_CHECK" --epoch "$SOURCE_DATE_EPOCH"
cmp "$ARCHIVE" "$ARCHIVE_CHECK" \
  || die "two deterministic archive passes produced different bytes"
echo "archive reproducibility: two independent archive passes agree"

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
  --source-date-epoch "$SOURCE_DATE_EPOCH"
  --binary-build-count 2
  --archive-build-count 2
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
  # An unsigned run must not leave a previous run's signature beside the
  # manifest it no longer covers. `dist/` survives between runs, so a signed
  # build followed by an unsigned one left a stale `.sig` sitting next to fresh
  # bytes: `ssh-keygen -Y verify` would have rejected it, but only if someone
  # ran it, and nothing in this pipeline does. The archive checksums are
  # rewritten every run and re-checked by the smoke test, so they cannot rot
  # this way; the manifest sidecars are the pair that can.
  rm -f "$MANIFEST.sig" "$MANIFEST.sha256"
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
