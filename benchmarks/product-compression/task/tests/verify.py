#!/usr/bin/env python3
"""Harbor-native exact scorer for one product-compression task."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def record_root(value: dict[str, Any], field: str) -> str:
    payload = canonical_bytes({key: item for key, item in value.items() if key != field})
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def read(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def git(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd="/workspace/frontier", check=True,
        capture_output=True, text=True,
    ).stdout.strip()


def outcome(answer: Any, key: Any, binding: Any, head: str, status: str) -> dict[str, Any]:
    eligibility: list[str] = []
    correctness: list[str] = []
    if not isinstance(key, dict) or key.get("answer_key_root") != record_root(key, "answer_key_root"):
        eligibility.append("answer_key_invalid")
    if not isinstance(binding, dict) or binding.get("binding_root") != record_root(binding, "binding_root"):
        eligibility.append("task_binding_invalid")
    expected_head = binding.get("frontier", {}).get("git_commit") if isinstance(binding, dict) else None
    if head != expected_head:
        eligibility.append("frontier_head_drift")
    if status:
        eligibility.append("frontier_worktree_drift")
    if not isinstance(key, dict) or answer != key.get("expected"):
        correctness.append("answer_mismatch")
    return {
        "eligible": not eligibility,
        "exact": not correctness,
        "eligibility_failure_codes": eligibility,
        "correctness_failure_codes": correctness,
    }


def main() -> None:
    answer_path = Path("/logs/artifacts/answer.json")
    answer = read(answer_path) if answer_path.is_file() else None
    key = read(Path("/tests/answer-key.json"))
    binding = read(Path("/tests/task-binding.json"))
    result = outcome(answer, key, binding, git("rev-parse", "HEAD"), git("status", "--porcelain"))
    verification = {
        "schema": "vela.harbor-offline-verification.v2",
        "binding_root": binding.get("binding_root"),
        "answer_root": (
            f"sha256:{hashlib.sha256(canonical_bytes(answer)).hexdigest()}"
            if answer is not None else None
        ),
        **result,
        "network": "none",
        "authority_available": False,
    }
    logs = Path("/logs/verifier")
    logs.mkdir(parents=True, exist_ok=True)
    (logs / "verification.json").write_bytes(canonical_bytes(verification))
    (logs / "reward.json").write_bytes(canonical_bytes({
        "eligible": int(result["eligible"]),
        "exact": int(result["exact"]),
    }))


if __name__ == "__main__":
    main()
