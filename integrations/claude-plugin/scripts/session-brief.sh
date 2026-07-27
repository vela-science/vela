#!/bin/bash
# SessionStart brief: a compact frontier orientation emitted as session
# context. Silent (exit 0, no output) anywhere that is not a Vela frontier,
# when `vela` is missing, or on ANY error — a broken hook must never break a
# session. Plain text only: this is context, not a terminal.
set -eu

# Walk up from $PWD looking for a .vela/ directory, the way git finds .git.
dir=$PWD
root=""
while :; do
  if [ -d "$dir/.vela" ]; then
    root=$dir
    break
  fi
  [ "$dir" = "/" ] && break
  dir=$(dirname "$dir")
done
[ -n "$root" ] || exit 0

command -v vela >/dev/null 2>&1 || exit 0
command -v python3 >/dev/null 2>&1 || exit 0

# Two cheap reads; each tolerated to fail (empty JSON -> section skipped).
status_json=$(cd "$root" && { vela status --json 2>/dev/null || true; })
next_json=$(cd "$root" && { vela next --json 2>/dev/null || true; })

ROOT="$root" STATUS_JSON="$status_json" NEXT_JSON="$next_json" \
  python3 - <<'EOF' || exit 0
import json, os

def load(name):
    try:
        return json.loads(os.environ.get(name, ""))
    except Exception:
        return {}

status = load("STATUS_JSON")
nxt = load("NEXT_JSON")

root = os.environ.get("ROOT", "")
name = os.path.basename(root.rstrip("/")) or root
lines = [f"Vela frontier: {name}"]

if status:
    parts = []
    counts = status.get("counts") or {}
    total = counts.get("findings")
    if total is not None:
        parts.append(f"{total} findings")
    integrity = status.get("integrity") or {}
    replay = integrity.get("replay")
    if replay:
        parts.append(f"replay {replay}")
    strict = integrity.get("strict")
    if strict:
        parts.append(f"strict {strict}")
    mode = (status.get("policy") or {}).get("state")
    if mode:
        parts.append(f"policy {mode}")
    pending = counts.get("pending_review")
    if pending:
        parts.append(f"{pending} pending proposal(s)")
    if parts:
        lines.append("State: " + ", ".join(parts) + ".")

targets = nxt.get("targets") or []
if targets:
    top = targets[0] or {}
    tid = top.get("id") or ""
    title = (top.get("title") or "").strip()
    if tid:
        lines.append("Top next target: " + tid + (f" — {title}" if title else ""))

lines.append(
    "Loop: next -> start -> land; accountable principals authorize. "
    "Full render: /vela:status."
)
print("\n".join(lines))
EOF

exit 0
