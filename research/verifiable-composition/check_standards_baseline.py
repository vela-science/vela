#!/usr/bin/env python3
"""Focused offline checks for the ADR 0004 standards baseline."""

from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
REFERENCE = ROOT / "reference"
FIXTURE = ROOT / "fixtures/standards-baseline"
VECTORS = ROOT / "vectors/standards-baseline-cases.json"
sys.path.insert(0, str(REFERENCE))

from fact_manifest import canonical_bytes  # noqa: E402
from standards_baseline import (  # noqa: E402
    BaselineError,
    build_dsse_envelope,
    document_bytes,
    load_bundle,
    strict_json_bytes,
    validate_bundle_values,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def load_values() -> tuple[dict[str, dict[str, Any]], dict[str, bytes], bytes]:
    paths = {
        "fact_manifest": FIXTURE / "fact-manifest.json",
        "vela_profile": FIXTURE / "vela-profile.json",
        "statement": FIXTURE / "in-toto-statement.json",
        "lock": FIXTURE / "science.lock",
    }
    raw = {label: path.read_bytes() for label, path in paths.items()}
    values = {
        label: strict_json_bytes(content, label=label)
        for label, content in raw.items()
    }
    values["envelope"] = build_dsse_envelope(values["statement"])
    raw["envelope"] = document_bytes(values["envelope"])
    return values, raw, (FIXTURE / "semantics.md").read_bytes()


def replace_hex(value: str) -> str:
    replacement = "0" if value[-1] != "0" else "1"
    return value[:-1] + replacement


def mutate(
    values: dict[str, dict[str, Any]],
    raw: dict[str, bytes],
    operation: str,
) -> None:
    if operation == "none":
        return
    if operation == "statement_manifest_root":
        values["statement"]["predicate"]["fact_manifest_root"] = "sha256:" + "f" * 64
    elif operation == "statement_subject_root":
        values["statement"]["subject"][0]["digest"]["sha256"] = "f" * 64
    elif operation == "statement_representation_claim":
        values["statement"]["predicate"]["representation_claim"] = "authority"
    elif operation == "dsse_payload":
        values["envelope"]["payload"] = "e30="
    elif operation == "dsse_signature_injection":
        values["envelope"]["signatures"] = [
            {"keyid": "fixture:forged", "sig": "00"}
        ]
    elif operation == "lock_fact_root":
        values["lock"]["lock"]["fact_manifest_root"] = "sha256:" + "f" * 64
    elif operation == "lock_signature":
        signature = values["lock"]["signatures"][0]["signature_hex"]
        values["lock"]["signatures"][0]["signature_hex"] = replace_hex(signature)
    elif operation == "lock_public_key":
        values["lock"]["signatures"][0]["public_key_hex"] = "00" * 32
    elif operation == "lock_scope_human":
        values["lock"]["signatures"][0]["scope"] = "human_authority"
    elif operation == "lock_extra_signature":
        values["lock"]["signatures"].append(
            copy.deepcopy(values["lock"]["signatures"][0])
        )
    elif operation == "profile_manifest_root":
        values["vela_profile"]["fact_manifest_root"] = "sha256:" + "f" * 64
    elif operation == "duplicate_lock_name":
        lock_raw = raw["lock"].decode("utf-8")
        raw["lock"] = lock_raw.replace(
            '{"lock":{',
            '{"schema":"science.lock.v0","lock":{',
            1,
        ).encode("utf-8")
        return
    else:
        raise RuntimeError(f"unknown standards-baseline mutation {operation}")
    for label in values:
        raw[label] = document_bytes(values[label])


def run_vectors() -> int:
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    require(
        vectors.get("schema")
        == "vela.verifiable-composition.standards-baseline-vectors.v0",
        "standards vector schema drift",
    )
    cases = vectors.get("cases")
    require(isinstance(cases, list) and cases, "standards vectors missing")
    identifiers = [case.get("id") for case in cases]
    require(len(identifiers) == len(set(identifiers)), "duplicate vector id")
    checked = 0
    for case in cases:
        values, raw, semantics_raw = load_values()
        mutate(values, raw, case["mutation"])
        expected = case["expected"]
        try:
            values = {
                label: strict_json_bytes(content, label=label)
                for label, content in raw.items()
            }
            validate_bundle_values(
                values,
                raw=raw,
                semantics_raw=semantics_raw,
            )
            actual = "pass"
        except BaselineError as error:
            actual = error.code
        require(
            actual == expected,
            f"{case['id']}: expected {expected}, got {actual}",
        )
        checked += 1
    return checked


def event(identifier: str, label: str) -> dict[str, Any]:
    return {
        "actor": {"id": "fixture:bundle-drill", "type": "agent"},
        "after_hash": "sha256:null",
        "before_hash": "sha256:null",
        "caveats": [],
        "id": identifier,
        "kind": "finding.noted",
        "payload": {"label": label},
        "reason": label,
        "schema": "vela.event.v0.1",
        "signature": "fixture-only",
        "target": {"id": "vf_1111111111111111", "type": "finding"},
        "timestamp": "2026-07-16T00:00:00Z",
    }


def event_root(events: list[dict[str, Any]]) -> str:
    stripped = []
    for item in events:
        value = copy.deepcopy(item)
        value.pop("signature", None)
        stripped.append(value)
    stripped.sort(key=lambda item: item["id"])
    import hashlib

    return f"sha256:{hashlib.sha256(canonical_bytes(stripped)).hexdigest()}"


def git(
    arguments: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    expected: tuple[int, ...] = (0,),
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *arguments],
        cwd=cwd,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    require(
        result.returncode in expected,
        f"git {' '.join(arguments)} failed: {result.stderr[:500]}",
    )
    return result


def write_events(repo: Path, events: list[dict[str, Any]]) -> None:
    (repo / "events.json").write_bytes(canonical_bytes(events) + b"\n")


def commit_events(
    repo: Path,
    events: list[dict[str, Any]],
    message: str,
    environment: dict[str, str],
) -> str:
    write_events(repo, events)
    git(["add", "events.json"], cwd=repo, environment=environment)
    git(
        ["-c", "commit.gpgsign=false", "commit", "-m", message],
        cwd=repo,
        environment=environment,
    )
    return git(
        ["rev-parse", "HEAD"], cwd=repo, environment=environment
    ).stdout.strip()


def is_ancestor(
    repo: Path,
    older: str,
    newer: str,
    environment: dict[str, str],
) -> bool:
    result = git(
        ["merge-base", "--is-ancestor", older, newer],
        cwd=repo,
        environment=environment,
        expected=(0, 1),
    )
    return result.returncode == 0


def read_events(
    repo: Path, commit: str, environment: dict[str, str]
) -> list[dict[str, Any]]:
    raw = git(
        ["show", f"{commit}:events.json"],
        cwd=repo,
        environment=environment,
    ).stdout.encode("utf-8")
    value = json.loads(raw)
    require(isinstance(value, list), "bundle event document is not a list")
    return value


def classify(
    repo: Path,
    last_seen: str,
    delivered: str,
    environment: dict[str, str],
) -> str:
    if last_seen == delivered:
        return "same"
    older = is_ancestor(repo, last_seen, delivered, environment)
    rollback = is_ancestor(repo, delivered, last_seen, environment)
    last_events = read_events(repo, last_seen, environment)
    delivered_events = read_events(repo, delivered, environment)
    last_prefix = delivered_events[: len(last_events)] == last_events
    delivered_prefix = last_events[: len(delivered_events)] == delivered_events
    if older and last_prefix:
        return "descendant"
    if rollback and delivered_prefix:
        return "stale"
    if not older and not rollback:
        return "fork"
    return "unresolvable"


def run_bundle_drill() -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="vela-adr4-git-baseline-") as directory:
        root = Path(directory)
        home = root / "home"
        home.mkdir()
        environment = {
            "GIT_CONFIG_NOSYSTEM": "1",
            "HOME": str(home),
            "LC_ALL": "C",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        }
        producer = root / "producer"
        producer.mkdir()
        git(["init", "-b", "main"], cwd=producer, environment=environment)
        git(
            ["config", "user.name", "ADR 0004 fixture"],
            cwd=producer,
            environment=environment,
        )
        git(
            ["config", "user.email", "fixture@example.invalid"],
            cwd=producer,
            environment=environment,
        )
        base_events = [event("vev_1111111111111111", "base")]
        base = commit_events(producer, base_events, "base", environment)
        git(["branch", "base", base], cwd=producer, environment=environment)
        descendant_events = [
            *base_events,
            event("vev_2222222222222222", "descendant"),
        ]
        descendant = commit_events(
            producer, descendant_events, "descendant", environment
        )
        git(
            ["checkout", "-b", "fork", base],
            cwd=producer,
            environment=environment,
        )
        fork_events = [*base_events, event("vev_3333333333333333", "fork")]
        fork = commit_events(producer, fork_events, "fork", environment)

        bundle = root / "frontier.bundle"
        git(
            ["bundle", "create", str(bundle), "--all"],
            cwd=producer,
            environment=environment,
        )
        verify = git(
            ["bundle", "verify", str(bundle)],
            cwd=producer,
            environment=environment,
        )
        require(
            "is okay" in verify.stderr or "is okay" in verify.stdout,
            "git bundle did not report verification",
        )

        consumer = root / "consumer"
        consumer.mkdir()
        git(["init", "-b", "main"], cwd=consumer, environment=environment)
        git(
            [
                "fetch",
                str(bundle),
                "refs/heads/*:refs/remotes/bundle/*",
            ],
            cwd=consumer,
            environment=environment,
        )
        cases = {
            "same": (descendant, descendant),
            "descendant": (base, descendant),
            "stale": (descendant, base),
            "fork": (descendant, fork),
        }
        outcomes = {
            name: classify(consumer, previous, delivered, environment)
            for name, (previous, delivered) in cases.items()
        }
        require(outcomes == {name: name for name in cases}, f"bundle drill: {outcomes}")
        roots = {
            commit: event_root(read_events(consumer, commit, environment))
            for commit in {base, descendant, fork}
        }
        require(len(set(roots.values())) == 3, "event roots did not distinguish states")
        return {
            "bundle_verify": "pass",
            "classifications": outcomes,
            "event_roots": len(roots),
            "gpg_signing": "disabled",
            "network": "disabled",
        }


def main() -> int:
    baseline = load_bundle(ROOT)
    checked = run_vectors()
    drill = run_bundle_drill()
    print(
        "standards baseline: "
        f"{checked}/{checked} wrapper vectors; "
        "signed fixture lock verified; "
        f"bundle {','.join(drill['classifications'])}; "
        f"fact root {baseline['fact_manifest_root']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
