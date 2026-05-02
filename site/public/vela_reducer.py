#!/usr/bin/env python3
# Vela reducer — second implementation, stdlib-only.
#
# What this proves: the per-kind reducer mutation rules are protocol,
# not Rust artifact. Two implementations of the reducer (this Python
# one and the Rust one in `crates/vela-protocol/src/reducer.rs`) must
# produce byte-equivalent post-replay finding state from the same
# canonical event log on the same genesis findings. If they don't,
# one of them is wrong.
#
# Usage:
#   python3 vela_reducer.py /path/to/cascade-fixture-00.json
#   python3 vela_reducer.py /path/to/fixtures/dir/   # walks all *.json
#   python3 vela_reducer.py --json /path/to/fixture.json
#
# Exit codes:
#   0  — every fixture's expected_states matched after Python replay
#   1  — at least one fixture mismatched (cross-implementation drift)
#   2  — fixture directory empty, malformed, or unreadable
#
# This implementation deliberately uses only Python stdlib so a
# reviewer can read it end to end and reason about whether it's doing
# the same thing the Rust reducer does. The matching Rust source is
# documented inline next to each apply_* function.
#
# Doctrine reference (events.rs::validate_event_payload + reducer.rs):
#   "two implementations of the reducer must agree on the mutation
#    rules per kind" — this script is the second implementation.
#
# Fixture schema: vela.science/schema/cross-impl-reducer-fixture/v1
# Generator: crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs

from __future__ import annotations

import argparse
import json
import sys
from copy import deepcopy
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


# ── Per-kind reducer rules ─────────────────────────────────────────────
#
# Each function mirrors a `fn apply_finding_*` in the Rust source at
# crates/vela-protocol/src/reducer.rs. The mutation rules are kept in
# sync by the cross-impl fixture test:
#   crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs
# If the Rust source changes a mutation rule, the fixture's
# expected_states drifts; if Python doesn't drift the same way, this
# script returns exit 1.


# ReviewState → contested mapping. Mirrors `ReviewState::implies_contested`
# in bundle.rs:1278-1288.
_CONTESTED_REVIEW_STATES = {"contested", "needs_revision", "rejected"}


def _find_finding(state: list[dict], finding_id: str) -> dict | None:
    for f in state:
        if f.get("id") == finding_id:
            return f
    return None


def _ensure_flags(f: dict) -> dict:
    if "flags" not in f or not isinstance(f["flags"], dict):
        f["flags"] = {}
    return f["flags"]


def _ensure_annotations(f: dict) -> list:
    if "annotations" not in f or not isinstance(f["annotations"], list):
        f["annotations"] = []
    return f["annotations"]


def _ensure_confidence(f: dict) -> dict:
    if "confidence" not in f or not isinstance(f["confidence"], dict):
        f["confidence"] = {}
    return f["confidence"]


def apply_finding_asserted(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_asserted.
    For v0.3+ frontiers a genesis event may carry the finding inline at
    payload.finding; for legacy frontiers the finding is already in
    state from genesis and this is a no-op.
    """
    payload = event.get("payload") or {}
    finding = payload.get("finding")
    if not finding:
        return
    if any(f.get("id") == finding.get("id") for f in state):
        return
    state.append(deepcopy(finding))


def apply_finding_reviewed(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_reviewed.
    Sets flags.review_state from the snake_case status; sets
    flags.contested per ReviewState::implies_contested.
    Accepts both 'accepted' and 'approved' (Rust accepts both).
    """
    payload = event.get("payload") or {}
    status = payload.get("status")
    if not isinstance(status, str):
        raise ValueError("finding.reviewed missing payload.status")
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(f"finding.reviewed targets unknown finding {finding_id}")
    flags = _ensure_flags(f)
    if status in ("accepted", "approved"):
        flags["review_state"] = "accepted"
        flags["contested"] = False
    elif status == "contested":
        flags["review_state"] = "contested"
        flags["contested"] = True
    elif status == "needs_revision":
        flags["review_state"] = "needs_revision"
        flags["contested"] = True
    elif status == "rejected":
        flags["review_state"] = "rejected"
        flags["contested"] = True
    else:
        raise ValueError(f"unsupported review status {status!r}")


def apply_finding_annotation(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_annotation.
    Idempotent on annotation_id. Adds an Annotation with id, text,
    author=event.actor.id, timestamp=event.timestamp.
    """
    payload = event.get("payload") or {}
    text = payload.get("text")
    annotation_id = payload.get("annotation_id")
    if not isinstance(text, str) or not isinstance(annotation_id, str):
        raise ValueError("annotation event missing text or annotation_id")
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(f"annotation event targets unknown finding {finding_id}")
    annotations = _ensure_annotations(f)
    if any(a.get("id") == annotation_id for a in annotations):
        return
    annotations.append(
        {
            "id": annotation_id,
            "text": text,
            "author": (event.get("actor") or {}).get("id", ""),
            "timestamp": event.get("timestamp", ""),
            "provenance": payload.get("provenance"),
        }
    )


def apply_finding_confidence_revised(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_confidence_revised.
    Sets confidence.score, basis, method=expert_judgment.
    """
    payload = event.get("payload") or {}
    new_score = payload.get("new_score")
    previous = payload.get("previous_score", 0.0)
    if not isinstance(new_score, (int, float)):
        raise ValueError("finding.confidence_revised missing payload.new_score")
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(f"confidence_revised targets unknown finding {finding_id}")
    conf = _ensure_confidence(f)
    conf["score"] = float(new_score)
    conf["basis"] = (
        f"expert revision from {float(previous):.3f} to {float(new_score):.3f}: "
        f"{event.get('reason', '')}"
    )
    conf["method"] = "expert_judgment"


def apply_finding_rejected(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_rejected. Sets contested=true."""
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(f"finding.rejected targets unknown finding {finding_id}")
    _ensure_flags(f)["contested"] = True


def apply_finding_retracted(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_retracted. Sets retracted=true."""
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(f"finding.retracted targets unknown finding {finding_id}")
    _ensure_flags(f)["retracted"] = True


def apply_finding_dependency_invalidated(state: list[dict], event: dict) -> None:
    """Mirror of reducer.rs::apply_finding_dependency_invalidated.
    Sets contested=true and appends a deterministic annotation whose
    id encodes the upstream cascade event and the depth.

    Rust shape:
      annotation_id = format!("ann_dep_{}_{}", &event.id[4..], depth);
    The "vev_" prefix on event.id is stripped by [4..] — Python does
    the same with [4:].
    """
    payload = event.get("payload") or {}
    upstream = payload.get("upstream_finding_id", "?")
    depth = payload.get("depth", 1)
    finding_id = event.get("target", {}).get("id")
    f = _find_finding(state, finding_id)
    if f is None:
        raise ValueError(
            f"finding.dependency_invalidated targets unknown finding {finding_id}"
        )
    _ensure_flags(f)["contested"] = True
    event_id = event.get("id", "")
    if event_id.startswith("vev_"):
        event_tail = event_id[4:]
    else:
        event_tail = event_id
    annotation_id = f"ann_dep_{event_tail}_{depth}"
    annotations = _ensure_annotations(f)
    if any(a.get("id") == annotation_id for a in annotations):
        return
    annotations.append(
        {
            "id": annotation_id,
            "text": f"Upstream {upstream} retracted (cascade depth {depth}).",
            "author": (event.get("actor") or {}).get("id", ""),
            "timestamp": event.get("timestamp", ""),
            "provenance": None,
        }
    )


def apply_event(state: list[dict], event: dict) -> None:
    kind = event.get("kind", "")
    if kind == "frontier.created":
        return  # structural anchor, no mutation
    elif kind == "finding.asserted":
        apply_finding_asserted(state, event)
    elif kind == "finding.reviewed":
        apply_finding_reviewed(state, event)
    elif kind in ("finding.noted", "finding.caveated"):
        apply_finding_annotation(state, event)
    elif kind == "finding.confidence_revised":
        apply_finding_confidence_revised(state, event)
    elif kind == "finding.rejected":
        apply_finding_rejected(state, event)
    elif kind == "finding.retracted":
        apply_finding_retracted(state, event)
    elif kind == "finding.dependency_invalidated":
        apply_finding_dependency_invalidated(state, event)
    else:
        raise ValueError(f"reducer: unsupported event kind {kind!r}")


# ── Reducer-effects digest ─────────────────────────────────────────────
#
# Mirror of `finding_state` in
# crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs.
# Captures only the fields the reducer mutates so cross-impl agreement
# is testable without serializing the full Project struct.


def reducer_effects(state: list[dict]) -> list[dict]:
    sorted_state = sorted(state, key=lambda f: f.get("id", ""))
    out = []
    for f in sorted_state:
        flags = f.get("flags") or {}
        review_state = flags.get("review_state") or "none"
        confidence = f.get("confidence") or {}
        annotations = f.get("annotations") or []
        annotation_ids = sorted(a.get("id", "") for a in annotations)
        # Format score to 6 decimals so f64 noise can't cross the
        # implementation boundary. Rust uses `format!("{:.6}", score)`,
        # Python matches with `f"{score:.6f}"`.
        score = float(confidence.get("score", 0.0))
        out.append(
            {
                "id": f.get("id", ""),
                "retracted": bool(flags.get("retracted", False)),
                "contested": bool(flags.get("contested", False)),
                "review_state": review_state,
                "confidence_score": f"{score:.6f}",
                "annotation_ids": annotation_ids,
            }
        )
    return out


# ── Fixture verification ───────────────────────────────────────────────


@dataclass
class FixtureResult:
    path: str
    frontier_idx: int
    findings: int = 0
    events: int = 0
    cascade_depth: int = 0
    matched: int = 0
    diffs: list[dict] = field(default_factory=list)
    ok: bool = False
    error: str | None = None


def verify_fixture(path: Path) -> FixtureResult:
    result = FixtureResult(path=str(path), frontier_idx=-1)
    try:
        fx = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as e:
        result.error = f"unreadable fixture: {e}"
        return result
    if fx.get("fixture_version") != "1":
        result.error = (
            f"unsupported fixture_version {fx.get('fixture_version')!r}; expected '1'"
        )
        return result
    result.frontier_idx = int(fx.get("frontier_idx", -1))
    stats = fx.get("stats") or {}
    result.findings = int(stats.get("findings", 0))
    result.events = int(stats.get("events", 0))
    result.cascade_depth = int(stats.get("cascade_depth", 0))

    genesis = deepcopy(fx.get("genesis_findings") or [])
    event_log = fx.get("event_log") or []
    expected = fx.get("expected_states") or []

    state = genesis
    for event in event_log:
        try:
            apply_event(state, event)
        except ValueError as e:
            result.error = (
                f"reducer error on event {event.get('id', '?')} "
                f"({event.get('kind', '?')}): {e}"
            )
            return result

    actual = reducer_effects(state)

    # Deep-equal compare. Build a diff list of mismatches.
    actual_by_id = {f["id"]: f for f in actual}
    expected_by_id = {f["id"]: f for f in expected}
    all_ids = sorted(set(actual_by_id) | set(expected_by_id))
    for fid in all_ids:
        a = actual_by_id.get(fid)
        e = expected_by_id.get(fid)
        if a is None:
            result.diffs.append({"id": fid, "issue": "missing in python output", "expected": e})
        elif e is None:
            result.diffs.append({"id": fid, "issue": "extra in python output", "actual": a})
        elif a != e:
            result.diffs.append(
                {
                    "id": fid,
                    "issue": "mismatch",
                    "expected": e,
                    "actual": a,
                }
            )
        else:
            result.matched += 1

    result.ok = not result.diffs and result.matched == len(expected)
    return result


def render_text(results: list[FixtureResult]) -> str:
    lines: list[str] = []
    lines.append("vela reducer (python · stdlib · second implementation)")
    for r in results:
        status = "ok" if r.ok else "FAIL"
        head = (
            f"  {status:<4} · frontier {r.frontier_idx:02} · "
            f"{r.matched}/{r.findings} findings · "
            f"{r.events} events · cascade depth {r.cascade_depth}"
        )
        lines.append(head)
        if r.error:
            lines.append(f"          error: {r.error}")
        for d in r.diffs[:5]:
            lines.append(f"          · {d.get('id', '?')}: {d.get('issue')}")
            if d.get("expected") and d.get("actual"):
                exp = d["expected"]
                act = d["actual"]
                for k in sorted(set(exp) | set(act)):
                    if exp.get(k) != act.get(k):
                        lines.append(
                            f"              {k}: expected={exp.get(k)!r} actual={act.get(k)!r}"
                        )
        if len(r.diffs) > 5:
            lines.append(f"          (… {len(r.diffs) - 5} more)")
    if all(r.ok for r in results):
        lines.append("")
        lines.append("reducer: ok")
        lines.append(
            "  every event-log replay through the python reducer produced"
        )
        lines.append(
            "  the same per-finding state the rust reducer produced. the"
        )
        lines.append(
            "  per-kind mutation rules are now confirmed across two"
        )
        lines.append(
            "  independent implementations."
        )
    return "\n".join(lines)


def collect_fixtures(target: Path) -> list[Path]:
    if target.is_file():
        return [target]
    if target.is_dir():
        return sorted(target.glob("cascade-fixture-*.json"))
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Vela cross-implementation reducer — applies a canonical event log "
            "to genesis findings and asserts the post-replay state matches the "
            "Rust reducer's expected_states byte-for-byte."
        )
    )
    parser.add_argument(
        "target",
        type=Path,
        help="Path to a fixture .json or a directory containing cascade-fixture-*.json",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit a structured JSON report instead of human-readable output",
    )
    args = parser.parse_args(argv)

    fixtures = collect_fixtures(args.target)
    if not fixtures:
        print(f"error: no cascade-fixture-*.json found at {args.target}", file=sys.stderr)
        return 2

    results = [verify_fixture(p) for p in fixtures]

    if args.json:
        print(
            json.dumps(
                {
                    "ok": all(r.ok for r in results),
                    "fixtures": [
                        {
                            "path": r.path,
                            "frontier_idx": r.frontier_idx,
                            "ok": r.ok,
                            "findings": r.findings,
                            "events": r.events,
                            "cascade_depth": r.cascade_depth,
                            "matched": r.matched,
                            "diffs": r.diffs,
                            "error": r.error,
                        }
                        for r in results
                    ],
                    "verifier": "vela_reducer.py · python3 stdlib · second implementation",
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(render_text(results))

    return 0 if all(r.ok for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
