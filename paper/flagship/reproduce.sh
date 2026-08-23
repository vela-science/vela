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
HELDOUT_CAPTURE_COMMIT="5694bebac03b062d6acdce5a2a900551850e6a1c"
HELDOUT_RESULT_COMMIT="4524c8f776943a267e04e03e9a237ecaed14bc2c"
HELDOUT_REVIEW_COMMIT="e6d8348bea3a57e88c5f9426d44a480b7a026fbd"

for commit in \
  "$MAIN_COMMIT" \
  "$RESULT_COMMIT" \
  "$AUDIT_COMMIT" \
  "$ERDOS_COMMIT" \
  "$HELDOUT_CAPTURE_COMMIT" \
  "$HELDOUT_RESULT_COMMIT" \
  "$HELDOUT_REVIEW_COMMIT"; do
  git -C "$ROOT" cat-file -e "${commit}^{commit}"
done

python3 - \
  "$ROOT" \
  "$MAIN_COMMIT" \
  "$RESULT_COMMIT" \
  "$AUDIT_COMMIT" \
  "$ERDOS_COMMIT" \
  "$HELDOUT_CAPTURE_COMMIT" \
  "$HELDOUT_RESULT_COMMIT" \
  "$HELDOUT_REVIEW_COMMIT" <<'PY'
from __future__ import annotations

from decimal import Decimal
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1])
(
    main_commit,
    result_commit,
    audit_commit,
    erdos_commit,
    heldout_capture_commit,
    heldout_result_commit,
    heldout_review_commit,
) = sys.argv[2:]


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
    (heldout_capture_commit, "paper/artifacts/inherited-correction-held-out-order-replacement-execution/capture-manifest.json"):
        "sha256:767a14d5a6c7b6d980348ca2c7b0ed2eb254f045ee9d4f271984f64312678fdf",
    (heldout_capture_commit, "paper/artifacts/inherited-correction-held-out-order-replacement-execution/capture-summary.json"):
        "sha256:1ed258dc57d6581b30bbeb073b22dc50612dc959a500d1d9e4dd72b3683f5dcd",
    (heldout_capture_commit, "paper/artifacts/inherited-correction-held-out-order-replacement-execution/complete-custody.json"):
        "sha256:b72431055872f713a82598f538946953525663bf3738dd597c1c461be2b8ad0a",
    (heldout_result_commit, "paper/artifacts/inherited-correction-held-out-order-replacement-result/scored-result.json"):
        "sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372",
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

capture_summary = json.loads(show(
    heldout_capture_commit,
    "paper/artifacts/inherited-correction-held-out-order-replacement-execution/capture-summary.json",
))
assert capture_summary["fixed_denominator"] == 36
assert capture_summary["terminal_runs"] == 36
assert capture_summary["condition_counts"] == {
    "git-documents": 12,
    "state-wrapper": 12,
    "vela": 12,
}
assert capture_summary["retries"] == 0
assert capture_summary["substitutions"] == 0
assert capture_summary["outcome_counts"] == {"completed": 36}
assert capture_summary["complete_custody_root"] == "sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef"

heldout = json.loads(
    show(
        heldout_result_commit,
        "paper/artifacts/inherited-correction-held-out-order-replacement-result/scored-result.json",
    ),
    parse_float=Decimal,
)
assert heldout["fixed_denominator"] == 36
assert heldout["capture_root"] == "sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98"
assert heldout["positive_gate"] == "not_supported"
assert heldout["authority_effect"] == "none"
assert heldout["gates"] == {
    "governance_inheritance": False,
    "structure": False,
    "total": False,
}
expected_heldout = {
    "git-documents": (12, 12, 0, Decimal("12.800895867")),
    "state-wrapper": (12, 12, 0, Decimal("13.98268798558333")),
    "vela": (11, 12, 1, Decimal("63.252235329")),
}
for condition, expected in expected_heldout.items():
    actual = heldout["aggregate"][condition]
    assert (
        actual["exact_successes"],
        actual["correction_impact_complete_sessions"],
        actual["authority_errors"],
        actual["restricted_mean_seconds"],
    ) == expected

review = json.loads(show(
    heldout_review_commit,
    "reviews/inherited-correction-benchmark/order-result-4524c8f7-verdict.json",
))
assert review["verdict"] == "PASS"
assert review["subject"]["result_commit"] == heldout_result_commit
assert review["subject"]["sealed_capture_parent"] == heldout_capture_commit
assert review["roots"]["result_canonical_root"] == "sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5"
assert review["gates"]["positive_gate"] == "not_supported"

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
    "held_out_result_commit": heldout_result_commit,
    "held_out_positive_gate": heldout["positive_gate"],
    "held_out_authority_effect": heldout["authority_effect"],
}, sort_keys=True, separators=(",", ":")))
PY

if [[ "$MODE" == "integrity" ]]; then
  exit 0
fi

TMP_ROOT="$(mktemp -d)"
cleanup() {
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/main" >/dev/null 2>&1 || true
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/result" >/dev/null 2>&1 || true
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/heldout" >/dev/null 2>&1 || true
  git -C "$ROOT" worktree remove --force "$TMP_ROOT/erdos" >/dev/null 2>&1 || true
  rmdir "$TMP_ROOT" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$ROOT" worktree add --detach "$TMP_ROOT/main" "$MAIN_COMMIT" >/dev/null
git -C "$ROOT" worktree add --detach "$TMP_ROOT/result" "$RESULT_COMMIT" >/dev/null
git -C "$ROOT" worktree add --detach "$TMP_ROOT/heldout" "$HELDOUT_RESULT_COMMIT" >/dev/null
git -C "$ROOT" worktree add --detach "$TMP_ROOT/erdos" "$ERDOS_COMMIT" >/dev/null

(
  cd "$TMP_ROOT/main"
  uv run --project conformance --locked python conformance/verify.py
  cargo test --locked -p vela-cli --features test-support --test portable_divergence
)

(
  cd "$TMP_ROOT/heldout"
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-held-out-order-replacement/benchmark.py verify
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-held-out-order-replacement/custody.py verify-prelaunch
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-held-out-order-replacement/test_benchmark.py
  PYTHONDONTWRITEBYTECODE=1 python3 paper/artifacts/inherited-correction-held-out-order-replacement/test_provider_schema_runtime.py
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

echo '{"schema":"vela.flagship-paper-reproduction-result.v1","status":"pass","provider_calls":0,"authority_effect":"none","held_out_positive_gate":"not_supported"}'
