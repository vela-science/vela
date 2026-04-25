#!/usr/bin/env python3
"""Validate benchmark fixture references against a frontier.

The validator is intentionally structural. It checks that gold fixtures are
tied to real frontier finding IDs, entities, links, and workflow minimums. It
does not claim scientific truth.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def load_json(path: Path):
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def resolve(base: Path, value: str) -> Path:
    path = Path(value)
    if path.is_absolute():
        return path
    candidate = base / path
    if candidate.exists():
        return candidate
    return Path.cwd() / path


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def validate_finding_gold(frontier_by_id: dict, gold_path: Path, errors: list[str]) -> int:
    items = load_json(gold_path)
    seen: set[str] = set()
    for index, item in enumerate(items):
        gid = item.get("id")
        if not gid:
            fail(errors, f"{gold_path}: finding item {index} is missing id")
            continue
        if gid in seen:
            fail(errors, f"{gold_path}: duplicate finding id {gid}")
        seen.add(gid)
        finding = frontier_by_id.get(gid)
        if not finding:
            fail(errors, f"{gold_path}: finding id {gid} is not in frontier")
            continue
        assertion = finding.get("assertion", {})
        if assertion.get("text") != item.get("assertion_text"):
            fail(errors, f"{gold_path}: assertion_text drift for {gid}")
        if assertion.get("type") != item.get("assertion_type"):
            fail(errors, f"{gold_path}: assertion_type drift for {gid}")
        frontier_entities = {e.get("name") for e in assertion.get("entities", [])}
        for entity in item.get("entities", []):
            if entity not in frontier_entities:
                fail(errors, f"{gold_path}: entity {entity!r} missing from {gid}")
    return len(items)


def validate_entity_gold(frontier: dict, gold_path: Path, errors: list[str]) -> int:
    items = load_json(gold_path)
    entity_index: dict[tuple[str, str], list[dict]] = {}
    for finding in frontier.get("findings", []):
        for entity in finding.get("assertion", {}).get("entities", []):
            entity_index.setdefault((entity.get("name"), entity.get("type")), []).append(entity)

    seen: set[tuple[str, str]] = set()
    for item in items:
        key = (item.get("name"), item.get("type"))
        if key in seen:
            fail(errors, f"{gold_path}: duplicate entity fixture {key[0]}:{key[1]}")
        seen.add(key)
        matches = entity_index.get(key, [])
        if not matches:
            fail(errors, f"{gold_path}: entity {key[0]}:{key[1]} is not in frontier")
            continue
        expected_source = item.get("expected_source", "")
        expected_id = item.get("expected_id", "")
        if expected_source or expected_id:
            if not any(
                (entity.get("canonical_id") or {}).get("source") == expected_source
                and (entity.get("canonical_id") or {}).get("id") == expected_id
                for entity in matches
            ):
                fail(errors, f"{gold_path}: canonical id mismatch for {key[0]}:{key[1]}")
    return len(items)


def validate_link_gold(frontier_by_id: dict, gold_path: Path, errors: list[str]) -> int:
    items = load_json(gold_path)
    seen: set[tuple[str, str, str]] = set()
    for item in items:
        key = (item.get("source_id"), item.get("target_id"), item.get("link_type"))
        if key in seen:
            fail(errors, f"{gold_path}: duplicate link fixture {key}")
        seen.add(key)
        source = frontier_by_id.get(key[0])
        if not source:
            fail(errors, f"{gold_path}: source finding {key[0]} is not in frontier")
            continue
        if key[1] not in frontier_by_id:
            fail(errors, f"{gold_path}: target finding {key[1]} is not in frontier")
        links = source.get("links", [])
        if not any(link.get("target") == key[1] and link.get("type") == key[2] for link in links):
            fail(errors, f"{gold_path}: link {key[0]} -> {key[1]}:{key[2]} is not in frontier")
    return len(items)


def validate_workflow(frontier: dict, task: dict, errors: list[str]) -> int:
    workflow = task.get("workflow") or {}
    findings = frontier.get("findings", [])
    metrics = {
        "min_findings": len(findings),
        "min_links": sum(len(finding.get("links", [])) for finding in findings),
        "min_entity_mentions": sum(
            len(finding.get("assertion", {}).get("entities", [])) for finding in findings
        ),
        "min_evidence_spans": sum(
            len(finding.get("evidence", {}).get("evidence_spans", [])) for finding in findings
        ),
        "min_provenance_complete": sum(
            1
            for finding in findings
            if finding.get("provenance", {}).get("doi")
            or finding.get("provenance", {}).get("pmid")
            or finding.get("provenance", {}).get("title")
        ),
        "min_assertion_types": len(
            {finding.get("assertion", {}).get("type") for finding in findings}
        ),
        "min_gap_flags": sum(1 for finding in findings if finding.get("flags", {}).get("gap")),
        "min_contested_flags": sum(
            1 for finding in findings if finding.get("flags", {}).get("contested")
        ),
    }
    checked = 0
    for key, expected_min in workflow.items():
        checked += 1
        if metrics.get(key, 0) < expected_min:
            fail(errors, f"workflow {task.get('id')}: {key} {metrics.get(key, 0)} < {expected_min}")
    return checked


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", default="benchmarks/suites/bbb-core.json")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    suite_path = Path(args.suite)
    suite = load_json(suite_path)
    suite_base = suite_path.parent
    frontier_path = resolve(suite_base, suite["frontier"])
    frontier = load_json(frontier_path)
    frontier_by_id = {finding.get("id"): finding for finding in frontier.get("findings", [])}

    errors: list[str] = []
    counts = {"finding": 0, "entity": 0, "link": 0, "workflow": 0}
    seen_tasks: set[str] = set()
    for task in suite.get("tasks", []):
        task_id = task.get("id")
        if task_id in seen_tasks:
            fail(errors, f"duplicate task id {task_id}")
        seen_tasks.add(task_id)
        mode = task.get("mode")
        if mode in {"finding", "entity", "link"}:
            gold = task.get("gold")
            if not gold:
                fail(errors, f"task {task_id} is missing gold path")
                continue
            gold_path = resolve(suite_base, gold)
            if not gold_path.exists():
                fail(errors, f"task {task_id} gold path does not exist: {gold_path}")
                continue
            if mode == "finding":
                counts[mode] += validate_finding_gold(frontier_by_id, gold_path, errors)
            elif mode == "entity":
                counts[mode] += validate_entity_gold(frontier, gold_path, errors)
            elif mode == "link":
                counts[mode] += validate_link_gold(frontier_by_id, gold_path, errors)
        elif mode == "workflow":
            counts[mode] += validate_workflow(frontier, task, errors)
        else:
            fail(errors, f"task {task_id} has unsupported mode {mode!r}")

    payload = {
        "ok": not errors,
        "suite": str(suite_path),
        "frontier": str(frontier_path),
        "tasks": len(suite.get("tasks", [])),
        "counts": counts,
        "failures": errors,
    }
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        status = "PASS" if payload["ok"] else "FAIL"
        print(f"{status} benchmark fixtures: {suite_path}")
        for key, value in counts.items():
            print(f"  {key}: {value}")
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
