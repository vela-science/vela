#!/usr/bin/env python3
"""Convert a Vela frontier manifest into a VelaBench gold-claims file.

Usage:
    scripts/gold-from-frontier.py <frontier.json> [<gold.json>]

Reads a published frontier (e.g. `frontiers/alzheimers-therapeutics.json`)
and emits the lighter `[GoldFinding]` shape that `vela bench --gold`
expects: assertion text + type, entity name list, and a confidence
range derived from the canonical score (±0.15, clamped to [0,1]).

The gold file is the ground truth for VelaBench leaderboard scoring;
it is rebuilt deterministically from the canonical frontier so
submitters can verify against the same hash.
"""

from __future__ import annotations

import json
import pathlib
import sys
from typing import Any


def gold_from_finding(f: dict[str, Any]) -> dict[str, Any]:
    a = f.get("assertion", {})
    c = f.get("confidence", {})
    score = float(c.get("score") or 0.5)
    low = max(0.0, score - 0.15)
    high = min(1.0, score + 0.15)
    entities = [
        e.get("name")
        for e in (a.get("entities") or [])
        if isinstance(e, dict) and e.get("name")
    ]
    return {
        "id": f.get("id"),
        "assertion_text": a.get("text", ""),
        "assertion_type": a.get("type", ""),
        "entities": entities,
        "confidence_range": {"low": round(low, 3), "high": round(high, 3)},
    }


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2

    in_path = pathlib.Path(argv[1])
    out_path = (
        pathlib.Path(argv[2])
        if len(argv) > 2
        else pathlib.Path("benchmarks/gold-alzheimers.json")
    )

    if not in_path.exists():
        print(f"input frontier not found: {in_path}", file=sys.stderr)
        return 1

    frontier = json.loads(in_path.read_text())
    findings = frontier.get("findings") or []
    if not findings:
        print(f"no findings in {in_path}", file=sys.stderr)
        return 1

    gold = [gold_from_finding(f) for f in findings]
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(gold, indent=2) + "\n")

    print(f"wrote {len(gold)} gold claims  {in_path.name} → {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
