#!/usr/bin/env bash
#
# Build canonical release assets for a Vela v0 release.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/dist/release-assets}"
VELA="$ROOT/target/release/vela"

cd "$ROOT"

echo "== Build release binary =="
# The `vela` binary lives in crates/vela-cli (vela-protocol is the
# substrate library only). Build the bin explicitly so $VELA resolves.
cargo build --release --bin vela

rm -rf "$OUT"
mkdir -p "$OUT"

echo "== Canonical frontier =="
cp frontiers/bbb-alzheimer.json "$OUT/bbb-alzheimer.json"

echo "== Check report =="
"$VELA" check frontiers/bbb-alzheimer.json --json > "$OUT/check-bbb-alzheimer.json"

echo "== Benchmark report =="
"$VELA" bench frontiers/bbb-alzheimer.json \
  --gold benchmarks/gold-50.json \
  --json > "$OUT/bench-bbb-alzheimer.json"

echo "== Proof packet =="
"$VELA" proof "$OUT/bbb-alzheimer.json" \
  --out "$OUT/bbb-alzheimer-proof-packet" \
  --json > "$OUT/proof-bbb-alzheimer.json"
"$VELA" packet validate "$OUT/bbb-alzheimer-proof-packet" > "$OUT/packet-validate-bbb-alzheimer.txt"

tar -C "$OUT" -czf "$OUT/bbb-alzheimer-proof-packet.tar.gz" bbb-alzheimer-proof-packet

echo "== Release manifest =="
python3 - "$OUT" <<'PY'
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
files = []
for path in sorted(root.iterdir()):
    if path.name == "RELEASE_MANIFEST.json" or path.name == "SHA256SUMS":
        continue
    if path.is_dir():
        continue
    data = path.read_bytes()
    files.append({
        "path": path.name,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    })

manifest = {
    "release_artifact_format": "vela.v0.release-assets",
    "schema_version": "0.2.0",
    "canonical_frontier": "bbb-alzheimer.json",
    "canonical_proof_packet": "bbb-alzheimer-proof-packet.tar.gz",
    "benchmark_report": "bench-bbb-alzheimer.json",
    "check_report": "check-bbb-alzheimer.json",
    "files": files,
}
root.joinpath("RELEASE_MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n")
PY

(
  cd "$OUT"
  shasum -a 256 \
    bbb-alzheimer.json \
    check-bbb-alzheimer.json \
    bench-bbb-alzheimer.json \
    proof-bbb-alzheimer.json \
    packet-validate-bbb-alzheimer.txt \
    bbb-alzheimer-proof-packet.tar.gz \
    RELEASE_MANIFEST.json > SHA256SUMS
)

echo "Release assets written to: $OUT"
