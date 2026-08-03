#!/usr/bin/env python3
"""Verify the exact merged Formal Conjectures correction for Erdős 264."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path


PREDECESSOR = "593e6b76702c5dbffaaa91b59f4faaed705d04ce"
PREDECESSOR_TREE = "5e79f7198c3891bdbb3fc6ec10c2b2a804cc56cb"
PREDECESSOR_BLOB = "8490f7dc0575480c7729acd5713433fc0af9c71b"
PREDECESSOR_FILE_ROOT = (
    "sha256:98386d8f28112c5e952ec40c4ee439c27f3ff7560a4e767b493ccebc628fb29f"
)
PREDECESSOR_DEFINITION_ROOT = (
    "sha256:c01f8742a00360a2a36cab0ce0c3be1e62d9539ca88df2d935607ea8492448cb"
)

SUCCESSOR = "0598b8f281060a18416d60753fd75621d659bb07"
SUCCESSOR_TREE = "e040cfc1cd6e5d1a79cf156047f452c2268c1920"
SUCCESSOR_BLOB = "3ff5ce70001355549571a07eee77960939323b57"
SUCCESSOR_FILE_ROOT = (
    "sha256:5a3a0fb7063ed77d644a5c1cab503851e68d87b02c0882db8fa52e801aba1166"
)
SUCCESSOR_DEFINITION_ROOT = (
    "sha256:6d8f5197e916b28724e586c8a79bd5e0607748a4bb9c50fccb2625bdc41ff986"
)

SOURCE_PATH = "FormalConjectures/ErdosProblems/264.lean"
DIFF_ROOT = "sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f"
ARTIFACT_ROOT = (
    "sha256:4443284e9856a2df1902dd81fb443f4042fb28b510278bfa2fe23ef935be3173"
)

PROBLEM_CLAIM = (
    "vcl_a9601802c65da247739eec2247cfcc5ae3016961673d346227af4e469f36fe82",
    "sha256:254621848ba12e76bff9970be183532499be6d86e4ee50e6bed341dc6739f00d",
)
LOCATOR_CLAIM = (
    "vcl_6b7736bc99918aee6ef5c3870861e3585cb1d07f4eaf199e4f4755b0375b9327",
    "sha256:7b12517e7ba9c077e00f18d8db3c0430950e1b05f1e2acf05593c2218a85d7be",
)
PARTIAL_PROOF_CLAIM = (
    "vcl_4386c93709bd09fc4c531108c633731e341862247309b5aa02aa1792111983f4",
    "sha256:62dd229a78539a2c74747b0b0ee859f326b18b9bfc13395eea0168a6c7d734ac",
)

CONSUMERS = (
    "Erdos264.erdos_264.parts.i",
    "Erdos264.erdos_264.parts.ii",
    "Erdos264.erdos_264.variants.example",
    "Erdos264.erdos_264.variants.ko_tao_neg",
    "Erdos264.erdos_264.variants.ko_tao_pos",
)


class VerificationError(ValueError):
    """Exact retained evidence does not match the registered correction."""


def sha256(payload: bytes) -> str:
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise VerificationError(message)


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def definition(source: bytes) -> bytes:
    start = source.index(b"def IsIrrationalitySequence")
    end = source.index(b"\n\n/--", start)
    return source[start:end] + b"\n"


def canonical_diff(repo: Path) -> bytes:
    return git(
        repo,
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--full-index",
        PREDECESSOR,
        SUCCESSOR,
        "--",
        SOURCE_PATH,
    )


def consumer_symbols(source: bytes) -> tuple[str, ...]:
    text = source.decode("utf-8")
    matches = re.finditer(
        r"theorem\s+([A-Za-z0-9_.]+)[\s\S]*?IsIrrationalitySequence", text
    )
    return tuple(f"Erdos264.{match.group(1)}" for match in matches)


def theorem_blocks(source: bytes) -> dict[str, str]:
    text = source.decode("utf-8")
    starts = list(re.finditer(r"(?m)^theorem\s+([A-Za-z0-9_.]+)", text))
    result = {}
    for index, match in enumerate(starts):
        end = (
            starts[index + 1].start()
            if index + 1 < len(starts)
            else text.index("end Erdos264")
        )
        result[f"Erdos264.{match.group(1)}"] = text[match.start() : end]
    return result


def load_artifact(path: Path) -> tuple[bytes, dict[str, object]]:
    payload = path.read_bytes()
    require(
        sha256(payload) == ARTIFACT_ROOT, "source-transition artifact root mismatch"
    )
    value = json.loads(payload)
    require(isinstance(value, dict), "source-transition artifact must be an object")
    return payload, value


def verify_claim(frontier: Path, claim_id: str, claim_root: str) -> None:
    path = frontier / "records" / "claims" / "sha256" / f"{claim_root[7:]}.json"
    payload = path.read_bytes()
    require(sha256(payload) == claim_root, f"Claim bytes disagree for {claim_id}")
    value = json.loads(payload)
    require(
        value.get("claim_id") == claim_id, f"Claim identity disagrees for {claim_id}"
    )


def verify(source_repo: Path, frontier: Path, artifact_path: Path) -> dict[str, object]:
    artifact_bytes, artifact = load_artifact(artifact_path)
    old = git(source_repo, "show", f"{PREDECESSOR}:{SOURCE_PATH}")
    new = git(source_repo, "show", f"{SUCCESSOR}:{SOURCE_PATH}")
    head = git(source_repo, "show", f"HEAD:{SOURCE_PATH}")

    require(
        git(source_repo, "rev-parse", f"{PREDECESSOR}^{{tree}}").decode().strip()
        == PREDECESSOR_TREE,
        "predecessor tree mismatch",
    )
    require(
        git(source_repo, "rev-parse", f"{SUCCESSOR}^{{tree}}").decode().strip()
        == SUCCESSOR_TREE,
        "successor tree mismatch",
    )
    require(
        git(source_repo, "rev-parse", f"{PREDECESSOR}:{SOURCE_PATH}").decode().strip()
        == PREDECESSOR_BLOB,
        "predecessor blob mismatch",
    )
    require(
        git(source_repo, "rev-parse", f"{SUCCESSOR}:{SOURCE_PATH}").decode().strip()
        == SUCCESSOR_BLOB,
        "successor blob mismatch",
    )
    require(sha256(old) == PREDECESSOR_FILE_ROOT, "predecessor file mismatch")
    require(sha256(new) == SUCCESSOR_FILE_ROOT, "successor file mismatch")
    require(
        sha256(definition(old)) == PREDECESSOR_DEFINITION_ROOT,
        "predecessor definition mismatch",
    )
    require(
        sha256(definition(new)) == SUCCESSOR_DEFINITION_ROOT,
        "successor definition mismatch",
    )
    require(
        sha256(definition(head)) == SUCCESSOR_DEFINITION_ROOT,
        "current source no longer retains the corrected definition",
    )
    require(
        sha256(canonical_diff(source_repo)) == DIFF_ROOT, "full-index diff mismatch"
    )

    found_consumers = consumer_symbols(new)
    require(found_consumers == CONSUMERS, "direct consumer inventory mismatch")
    blocks = theorem_blocks(new)
    require(
        all("sorry" in blocks[symbol] for symbol in CONSUMERS),
        "a direct consumer is no longer proof-unresolved",
    )

    for claim_id, claim_root in (PROBLEM_CLAIM, LOCATOR_CLAIM, PARTIAL_PROOF_CLAIM):
        verify_claim(frontier, claim_id, claim_root)

    subject = artifact.get("subject")
    transition = artifact.get("transition")
    scope = artifact.get("direct_consumer_scope")
    claims = artifact.get("frontier_claims")
    require(isinstance(subject, dict), "artifact subject missing")
    require(isinstance(transition, dict), "artifact transition missing")
    require(isinstance(scope, dict), "artifact consumer scope missing")
    require(isinstance(claims, dict), "artifact Claim classification missing")
    require(subject.get("path") == SOURCE_PATH, "artifact source path mismatch")
    require(
        transition.get("full_index_diff_sha256") == DIFF_ROOT,
        "artifact full-index diff mismatch",
    )
    artifact_consumers = scope.get("consumers")
    require(isinstance(artifact_consumers, list), "artifact consumers missing")
    require(
        tuple(
            item.get("symbol") for item in artifact_consumers if isinstance(item, dict)
        )
        == CONSUMERS,
        "artifact consumer order or identity mismatch",
    )
    require(
        all(
            item.get("semantic_impact") == "affected"
            and item.get("declaration_status") == "present"
            and item.get("proof_status") == "unresolved"
            for item in artifact_consumers
        ),
        "artifact consumer classification mismatch",
    )

    return {
        "schema": "vela.erdos-264-source-verification.v1",
        "outcome": "pass",
        "artifact_root": sha256(artifact_bytes),
        "source": {
            "repository": "https://github.com/google-deepmind/formal-conjectures",
            "path": SOURCE_PATH,
            "predecessor_commit": PREDECESSOR,
            "successor_commit": SUCCESSOR,
            "full_index_diff_root": DIFF_ROOT,
            "current_commit": git(source_repo, "rev-parse", "HEAD").decode().strip(),
            "current_definition_root": sha256(definition(head)),
        },
        "direct_consumers": [
            {
                "symbol": symbol,
                "semantic_impact": "affected",
                "declaration_status": "present",
                "proof_status": "unresolved",
            }
            for symbol in CONSUMERS
        ],
        "frontier_claims": {
            "survives": {"claim_id": PROBLEM_CLAIM[0], "claim_root": PROBLEM_CLAIM[1]},
            "supersede": {"claim_id": LOCATOR_CLAIM[0], "claim_root": LOCATOR_CLAIM[1]},
            "audit": {
                "claim_id": PARTIAL_PROOF_CLAIM[0],
                "claim_root": PARTIAL_PROOF_CLAIM[1],
            },
        },
        "checks": {
            "source_objects_exact": True,
            "full_index_diff_exact": True,
            "corrected_definition_retained_at_current_head": True,
            "direct_consumer_set_complete": True,
            "all_direct_consumers_proof_unresolved": True,
            "frontier_claim_bytes_exact": True,
        },
        "limits": [
            "This verifies exact source identity, semantic definition bytes, and the direct consumer inventory.",
            "It does not prove Erdos problem 264 or any direct consumer theorem.",
            "It does not establish compatibility of the retained hosted partial proof.",
            "It is scoped Verification evidence, not scientific acceptance or Standing.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-repo", type=Path, required=True)
    parser.add_argument("--frontier", type=Path, required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = verify(
            args.source_repo.resolve(),
            args.frontier.resolve(),
            args.artifact.resolve(),
        )
    except (
        OSError,
        ValueError,
        KeyError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
    ) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    encoded = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
