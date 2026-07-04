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

# Three cheap reads; each tolerated to fail (empty JSON -> section skipped).
status_json=$(cd "$root" && vela status --json 2>/dev/null) || status_json=""
sign_json=$(cd "$root" && vela sign --json 2>/dev/null) || sign_json=""
next_json=$(cd "$root" && vela next --json 2>/dev/null) || next_json=""

ROOT="$root" STATUS_JSON="$status_json" SIGN_JSON="$sign_json" \
  NEXT_JSON="$next_json" python3 - <<'EOF' || exit 0
import json, os

def load(name):
    try:
        return json.loads(os.environ.get(name, ""))
    except Exception:
        return {}

status = load("STATUS_JSON")
sign = load("SIGN_JSON")
nxt = load("NEXT_JSON")

root = os.environ.get("ROOT", "")
name = os.path.basename(root.rstrip("/")) or root
lines = [f"Vela frontier: {name}"]

if status.get("ok"):
    parts = []
    total = (status.get("findings") or {}).get("total")
    if total is not None:
        parts.append(f"{total} findings")
    replay = status.get("replay") or {}
    if "ok" in replay:
        parts.append("replay ok" if replay.get("ok") else "replay BROKEN")
    mode = (status.get("policy") or {}).get("mode")
    if mode:
        parts.append(f"policy {mode}")
    pending = (status.get("inbox") or {}).get("pending_total")
    if pending:
        parts.append(f"{pending} pending proposal(s)")
    if parts:
        lines.append("State: " + ", ".join(parts) + ".")

depth = sign.get("signable_total")
if isinstance(depth, int):
    if depth:
        lines.append(
            f"Sign queue: {depth} item(s) awaiting the human ceremony (vela sign)."
        )
    else:
        lines.append("Sign queue: clear.")

targets = nxt.get("targets") or []
if targets:
    top = targets[0] or {}
    tid = top.get("id") or ""
    title = (top.get("title") or "").strip()
    if tid:
        lines.append("Top next target: " + tid + (f" — {title}" if title else ""))

lines.append("Loop: next -> work -> land; humans sign. Full render: /vela:status.")
print("\n".join(lines))
EOF

exit 0
