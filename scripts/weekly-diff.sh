#!/usr/bin/env bash
# weekly-diff.sh — emit a Monday rhythm marker for the
# Alzheimer's Therapeutics frontier (BBB Flagship on disk).
#
# What it does:
#   1. Resolves the current ISO week (or honours --week=YYYY-Www).
#   2. Reads .vela/findings/*.json and computes which findings were
#      created or updated within the week.
#   3. Writes a `weekly_diff` JSON to .vela/events/YYYY-Www-weekly-diff.json
#      with the structured payload.
#   4. Prints a short human summary so the rhythm of running this is
#      visible at the terminal.
#
# What it does NOT do (deliberate, v0.31):
#   - Does NOT call the Vela protocol's signing apparatus. The
#     resulting JSON is unsigned and lives outside the canonical
#     event log; v0.32 will replace this with a signed `weekly_diff`
#     event after `vela frontier diff` lands as a CLI subcommand.
#   - Does NOT publish to the hub.
#
# Usage:
#   scripts/weekly-diff.sh                    # current ISO week
#   scripts/weekly-diff.sh --week 2026-W18    # specific week
#   scripts/weekly-diff.sh --frontier projects/bbb-flagship  # alt project
#
# The script depends only on Python 3 (stdlib) and bash. No network.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTIER_DIR="${REPO_ROOT}/projects/bbb-flagship"
WEEK=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --week)
      WEEK="$2"; shift 2 ;;
    --week=*)
      WEEK="${1#*=}"; shift ;;
    --frontier)
      FRONTIER_DIR="$2"; shift 2 ;;
    --frontier=*)
      FRONTIER_DIR="${1#*=}"; shift ;;
    -h|--help)
      sed -n '1,40p' "$0"; exit 0 ;;
    *)
      echo "weekly-diff: unknown arg $1" >&2; exit 2 ;;
  esac
done

# Resolve frontier dir relative to repo root if not absolute.
if [[ "${FRONTIER_DIR}" != /* ]]; then
  FRONTIER_DIR="${REPO_ROOT}/${FRONTIER_DIR}"
fi

if [[ ! -d "${FRONTIER_DIR}/.vela/findings" ]]; then
  echo "weekly-diff: missing ${FRONTIER_DIR}/.vela/findings" >&2
  exit 1
fi

# Side-directory for unsigned weekly-diff markers. Kept out of
# `.vela/events/` because that directory holds canonical signed
# StateEvents; vela's loader rejects anything else there. The v0.32
# replacement (`vela frontier diff`) emits a real signed event into
# `.vela/events/` instead.
mkdir -p "${FRONTIER_DIR}/.vela/weekly-diffs"

# ── Canonical path: prefer `vela frontier diff` when available ──
#
# v0.32 ships the diff as a Rust subcommand. If a built binary
# exists, use it as the source of truth. The Python fallback below
# stays for environments without a built binary (CI, fresh clones)
# but produces the same payload shape.
VELA_BIN="${REPO_ROOT}/target/release/vela"
if [[ -x "${VELA_BIN}" ]]; then
  WEEK_ARG=()
  if [[ -n "${WEEK}" ]]; then
    WEEK_ARG=(--week "${WEEK}")
  fi
  if "${VELA_BIN}" frontier diff "${FRONTIER_DIR}" --json "${WEEK_ARG[@]}" \
       > "${FRONTIER_DIR}/.vela/weekly-diffs/_tmp.json" 2> /tmp/weekly-diff.err; then
    # Resolve the week key from the diff payload so the filename
    # matches the canonical CLI's understanding of "this week."
    RESOLVED_WEEK=$(python3 -c '
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
print(d.get("window", {}).get("iso_week") or "current")
' "${FRONTIER_DIR}/.vela/weekly-diffs/_tmp.json")
    OUT="${FRONTIER_DIR}/.vela/weekly-diffs/${RESOLVED_WEEK}-weekly-diff.json"
    mv "${FRONTIER_DIR}/.vela/weekly-diffs/_tmp.json" "${OUT}"
    python3 - "${OUT}" "${RESOLVED_WEEK}" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    d = json.load(f)
key = sys.argv[2]
t = d["totals"]
w = d["window"]
print(f"weekly-diff · {key}  (via `vela frontier diff`)")
print(f"  range:           {w['start']} → {w['end']}")
print(f"  added:           {t['added']}")
print(f"  updated:         {t['updated']}")
print(f"  contradictions:  {t['new_contradictions']}")
print(f"  cumulative:      {t['cumulative_claims']}")
print(f"  written:         .vela/weekly-diffs/{key}-weekly-diff.json")
PY
    exit 0
  else
    echo "weekly-diff: vela CLI failed; falling back to Python fallback." >&2
    echo "  see /tmp/weekly-diff.err for details" >&2
    rm -f "${FRONTIER_DIR}/.vela/weekly-diffs/_tmp.json"
  fi
fi

# ── Fallback: pure-Python computation ──
#
# Used when the vela binary isn't built (cold CI, fresh clones).
# Produces a compatible (but slightly older) payload shape. Build
# the binary with `cargo build --release -p vela-cli` to switch to
# the canonical path automatically.

python3 - "$FRONTIER_DIR" "$WEEK" <<'PY'
import datetime as dt
import json
import os
import pathlib
import sys

frontier_dir = pathlib.Path(sys.argv[1])
explicit_week = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else None

findings_dir = frontier_dir / ".vela" / "findings"
events_dir = frontier_dir / ".vela" / "weekly-diffs"


def iso_week_key(d: dt.date) -> str:
    iso_year, iso_week, _ = d.isocalendar()
    return f"{iso_year}-W{iso_week:02d}"


def iso_week_bounds(week_key: str) -> tuple[dt.datetime, dt.datetime]:
    # Monday 00:00 UTC inclusive → next Monday 00:00 UTC exclusive.
    year_str, w_str = week_key.split("-W")
    year, week = int(year_str), int(w_str)
    jan4 = dt.date(year, 1, 4)
    jan4_dow = jan4.isoweekday()  # Mon=1
    week1_mon = jan4 - dt.timedelta(days=jan4_dow - 1)
    start_date = week1_mon + dt.timedelta(weeks=week - 1)
    end_date = start_date + dt.timedelta(days=7)
    start = dt.datetime.combine(start_date, dt.time(0, 0), tzinfo=dt.timezone.utc)
    end = dt.datetime.combine(end_date, dt.time(0, 0), tzinfo=dt.timezone.utc)
    return start, end


def parse_iso(ts: str) -> dt.datetime | None:
    if not ts:
        return None
    try:
        # Python 3.11+ accepts most ISO formats; normalise trailing Z.
        return dt.datetime.fromisoformat(ts.replace("Z", "+00:00"))
    except ValueError:
        return None


now = dt.datetime.now(dt.timezone.utc)
week_key = explicit_week or iso_week_key(now.date())
start, end = iso_week_bounds(week_key)

added = []
updated = []
new_contradictions = []
cumulative = 0

for path in sorted(findings_dir.glob("*.json")):
    try:
        f = json.loads(path.read_text())
    except json.JSONDecodeError:
        print(f"weekly-diff: skipping malformed {path.name}", file=sys.stderr)
        continue

    created = parse_iso(f.get("created", ""))
    updated_ts = parse_iso(f.get("updated", ""))

    if created and created < end:
        cumulative += 1

    summary = {
        "id": f.get("id"),
        "assertion": (f.get("assertion") or {}).get("text", ""),
        "evidence_type": (f.get("evidence") or {}).get("type"),
        "confidence": (f.get("confidence") or {}).get("score"),
        "doi": (f.get("provenance") or {}).get("doi"),
        "pmid": (f.get("provenance") or {}).get("pmid"),
    }

    if created and start <= created < end:
        added.append(summary)
        flags = f.get("flags") or {}
        atype = (f.get("assertion") or {}).get("type", "")
        if flags.get("contested") or atype == "tension":
            new_contradictions.append(summary)
        continue

    if updated_ts and start <= updated_ts < end:
        updated.append(summary)


payload = {
    "schema": "vela.weekly_diff/v0.1",
    "kind": "weekly_diff.unsigned",
    "frontier_dir": str(frontier_dir.relative_to(frontier_dir.parent.parent)),
    "week": {
        "key": week_key,
        "start": start.isoformat(),
        "end": end.isoformat(),
    },
    "generated_at": now.isoformat(),
    "totals": {
        "added": len(added),
        "updated": len(updated),
        "new_contradictions": len(new_contradictions),
        "cumulative_claims": cumulative,
    },
    "added": added,
    "updated": updated,
    "new_contradictions": new_contradictions,
    "next": {
        "v0.32_action": "Replace this unsigned marker with a `weekly_diff` event signed via `vela frontier diff --since <date>` once the CLI subcommand lands.",
    },
}

out = events_dir / f"{week_key}-weekly-diff.json"
out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")

print(f"weekly-diff · {week_key}")
print(f"  range:           {start.date()} → {end.date()}")
print(f"  added:           {len(added)}")
print(f"  updated:         {len(updated)}")
print(f"  contradictions:  {len(new_contradictions)}")
print(f"  cumulative:      {cumulative}")
print(f"  written:         {out.relative_to(frontier_dir.parent.parent)}")
print()
print("Next: open /frontier/{key} on the local site to verify the page renders.".format(key=week_key))
print("Future (v0.32): the canonical replacement is a signed event via")
print("  vela frontier diff --since <last-monday>")
print("which becomes a real `weekly_diff` event in .vela/events/.")
PY
