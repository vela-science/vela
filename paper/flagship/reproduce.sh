#!/usr/bin/env bash
set -euo pipefail

MODE="full"
if [[ "${1:-}" == "--integrity-only" ]]; then
  MODE="integrity"
elif [[ $# -ne 0 ]]; then
  echo "usage: $0 [--integrity-only]" >&2
  exit 2
fi

ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
MAIN_COMMIT="4685462c44b1f073870f31025ae73d1d8770ce73"
RESULT_COMMIT="7641d775911f6026a9c36649d6cf1354dd1f70c0"
AUDIT_COMMIT="de13073ff8f3a9f2958f8c93c848205c533ddb1e"
ERDOS_COMMIT="b6e554513346f515090e013a3484548261b7b93d"

for commit in "$MAIN_COMMIT" "$RESULT_COMMIT" "$AUDIT_COMMIT" "$ERDOS_COMMIT"; do
  git -C "$ROOT" cat-file -e "${commit}^{commit}"
done

python3 - "$ROOT" "$MAIN_COMMIT" "$RESULT_COMMIT" "$AUDIT_COMMIT" "$ERDOS_COMMIT" <<'PY'
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1])
main_commit, result_commit, audit_commit, erdos_commit = sys.argv[2:]


def show(commit: str, path: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), "show", f"{commit}:{path}"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


expected_files = {
    (main_commit, "examples/portable-divergence/expected.json"):
        "sha256:858019d298f55295fe92989bb23a343ce73b6976338f36c7c637c82272274041",
    (main_commit, "paper/artifacts/map-target-loop/pre-run.json"):
        "sha256:e0a517d543ce448917f6baa1a620727431caa53b0590f247bbf3fe9f5c3ed6d6",
    (main_commit, "paper/artifacts/map-target-loop/post-verification-map.json"):
        "sha256:439a804908890e4029922cc91cdd0a79122187d573530fc760a419d90786be21",
    (main_commit, "paper/artifacts/map-target-loop/post-decision.json"):
        "sha256:b29e8cbb50aff3cc81a4ac6f4cf261b9a3ca9d80dbe69614d9a771116d80151c",
    (result_commit, "paper/artifacts/inherited-correction-benchmark-execution/confirmatory-replacement-execution/runs/scored-result.json"):
        "sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f",
}
for key, expected in expected_files.items():
    actual = digest(show(*key))
    if actual != expected:
        raise SystemExit(f"root mismatch for {key[1]}: {actual}")

portable = json.loads(show(main_commit, "examples/portable-divergence/expected.json"))
assert portable["submission_root"] == "sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a"
assert portable["accept"]["standing"] == "accepted"
assert portable["reject"]["standing"] == "unassessed"
assert portable["accept"]["repository_root"] != portable["reject"]["repository_root"]

result = json.loads(show(result_commit, "paper/artifacts/inherited-correction-benchmark-execution/confirmatory-replacement-execution/runs/scored-result.json"))
assert result["fixed_denominator"] == 16
assert result["positive_gate"] == "not_supported"
assert result["authority_effect"] == "none"
assert result["conditions"]["git-documents"]["sessions"] == 8
assert result["conditions"]["vela"]["sessions"] == 8
assert result["conditions"]["git-documents"]["exact_successes"] == 0
assert result["conditions"]["vela"]["exact_successes"] == 5

manifest = json.loads(show(audit_commit, "paper/artifacts/inherited-correction-post-result-audit/manifest.json"))
for member in manifest["files"]:
    data = show(audit_commit, f"paper/artifacts/inherited-correction-post-result-audit/{member['path']}")
    assert digest(data) == member["sha256"]
files_bytes = json.dumps(
    {"files": manifest["files"]}, sort_keys=True, separators=(",", ":")
).encode()
assert digest(files_bytes) == manifest["artifact_root"] == "sha256:8463024ee31116c33cee9e43262286bb78855654ecc974e77818bf4dfac581af"

erdos = json.loads(show(erdos_commit, "paper/artifacts/erdos-264-proof-repair-2026-08-03/result.v1.json"))
assert erdos["result_root"] == "sha256:f9c009ec0e53cfd0362b924b440ba44cee243af5248906da1c82f516ec4c7585"
assert erdos["comparison"]["exact_pass_at_1"] == {"git_files": 0, "vela_guided": 0}
assert erdos["post_study_repair"]["clean_clone_replay"]["outcome"] == "pass"

print(json.dumps({
    "schema": "vela.flagship-paper-integrity-result.v1",
    "main_commit": main_commit,
    "result_commit": result_commit,
    "audit_commit": audit_commit,
    "erdos_commit": erdos_commit,
    "positive_gate": result["positive_gate"],
    "authority_effect": result["authority_effect"],
    "held_out_status": "not_run",
}, sort_keys=True, separators=(",", ":")))
PY

if [[ "$MODE" == "integrity" ]]; then
  exit 0
fi

TMP_ROOT="$(mktemp -d)"
cleanup() {
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/main" >/dev/null 2>&1 || true
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/result" >/dev/null 2>&1 || true
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/erdos" >/dev/null 2>&1 || true
  rmdir "$TMP_ROOT" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$ROOT" worktree add --detach "$TMP_ROOT/main" "$MAIN_COMMIT" >/dev/null
git -C "$ROOT" worktree add --detach "$TMP_ROOT/result" "$RESULT_COMMIT" >/dev/null
git -C "$ROOT" worktree add --detach "$TMP_ROOT/erdos" "$ERDOS_COMMIT" >/dev/null

(
  cd "$TMP_ROOT/main"
  uv run --project conformance --locked python conformance/verify.py
  cargo test --locked -p vela-cli --features test-support --test portable_divergence
)

(
  cd "$TMP_ROOT/result"
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-benchmark/benchmark.py verify
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-benchmark/test_benchmark.py
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-benchmark-execution/canonicalize-post-result.py --check-fixture
)

(
  cd "$TMP_ROOT/erdos"
  PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s paper/artifacts/erdos-264 -p 'test_*.py'
)

echo '{"schema":"vela.flagship-paper-reproduction-result.v1","status":"pass","provider_calls":0,"authority_effect":"none","held_out_status":"not_run"}'
