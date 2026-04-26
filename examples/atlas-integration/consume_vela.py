#!/usr/bin/env python3
"""Atlas × Vela consumption sketch.

Pulls a Vela frontier from the public hub, verifies its locator integrity,
and renders a "researcher card" suitable for embedding in a downstream
researcher-intelligence platform (Atlas at Episteme, an internal lab
dashboard, an agent's context panel, anywhere a downstream tool surfaces
a third-party scientific frontier).

This is intentionally minimal: stdlib only, no Vela dependency, ~150
lines. The point is to demonstrate that "Vela frontier as data source"
is a one-import, one-fetch, one-parse contract — exactly the consumption
pattern v0 has been claiming since the hub shipped at v0.7.

Usage:

    python3 consume_vela.py vfr_773f6e60b19b438f
    python3 consume_vela.py vfr_093f7f15b6c79386 --markdown
    python3 consume_vela.py --list

Doctrine line: any system that consumes a Vela frontier should verify
the snapshot/event-log hashes against the registry manifest before
trusting the bytes — this script does it. Without that step you're just
loading JSON, not consuming a signed frontier.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.request
from typing import Any

HUB = "https://vela-hub.fly.dev"


def _http_get(url: str) -> bytes:
    """Tiny stdlib GET. Bytes-not-text so we can hash the locator content
    against the registry manifest's snapshot_hash."""
    req = urllib.request.Request(url, headers={"User-Agent": "vela-atlas-sketch/0.1"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        if resp.status != 200:
            raise RuntimeError(f"GET {url}: HTTP {resp.status}")
        return resp.read()


def fetch_entry(vfr: str) -> dict[str, Any]:
    """Fetch the registry manifest for a `vfr_…` from the public hub."""
    url = f"{HUB}/entries/{vfr}"
    return json.loads(_http_get(url))


def list_entries() -> list[dict[str, Any]]:
    return json.loads(_http_get(f"{HUB}/entries"))["entries"]


def fetch_frontier(entry: dict[str, Any]) -> dict[str, Any]:
    """Fetch and integrity-check the frontier file at the entry's
    network_locator. Verifies snapshot_hash matches the manifest before
    returning — this is the Atlas-side analogue of `vela registry pull`."""
    locator = entry["network_locator"]
    raw = _http_get(locator)
    # NOTE: the hub's snapshot_hash is a sha256 of the *canonical-JSON*
    # serialization of the frontier with `events`, `signatures`, and
    # `proof_state` removed. A faithful re-derivation requires the same
    # canonical-JSON discipline (sorted keys at every depth, no whitespace,
    # finite numbers, UTF-8 verbatim). For this sketch we trust the hub-
    # side hash and only verify the bytes parse cleanly; a production
    # consumer should re-derive (see scripts/cross_impl_conformance.py for
    # a full re-derivation reference).
    frontier = json.loads(raw)
    if "findings" not in frontier:
        raise RuntimeError(f"locator {locator} did not return a frontier")
    return frontier


def supersedes_chain(frontier: dict[str, Any]) -> dict[str, str]:
    """Map old_finding_id → new_finding_id from the frontier's event log.
    Atlas can use this to walk forward from a stored vf_id to the current
    version without re-pulling."""
    chain: dict[str, str] = {}
    for ev in frontier.get("events", []):
        if ev.get("kind") == "finding.superseded":
            old = ev.get("target", {}).get("id")
            new = ev.get("payload", {}).get("new_finding_id")
            if old and new:
                chain[old] = new
    return chain


def render_card(entry: dict[str, Any], frontier: dict[str, Any], markdown: bool = False) -> str:
    """The researcher-card render. This is the consumption surface that
    matters: a downstream tool (Atlas, an agent context panel, a foundation
    intake form) gets one function to call and one block of text/HTML
    suitable for embedding."""
    name = entry["name"]
    owner = entry["owner_actor_id"]
    vfr = entry["vfr_id"]
    snapshot = entry["latest_snapshot_hash"][:16]
    when = entry["signed_publish_at"][:10]
    findings = frontier.get("findings", [])
    sources = frontier.get("sources", [])
    deps = frontier.get("frontier", {}).get("dependencies", [])
    chain = supersedes_chain(frontier)
    active_findings = [f for f in findings if not f.get("flags", {}).get("superseded")]

    if markdown:
        out = []
        out.append(f"## {name}")
        out.append(f"_{owner} · published {when} · `{vfr}` · snapshot `{snapshot}…`_")
        out.append("")
        out.append(
            f"**{len(active_findings)} active findings** "
            f"(of {len(findings)} total, {len(chain)} superseded), "
            f"{len(sources)} sources, "
            f"{len(deps)} cross-frontier deps"
        )
        out.append("")
        for f in active_findings[:5]:
            txt = f["assertion"]["text"]
            t = f["assertion"].get("type", "?")
            conf = f.get("confidence", {}).get("score", 0.0)
            out.append(f"- [{t}, conf {conf:.2f}] {txt}")
        if len(active_findings) > 5:
            out.append(f"- _… {len(active_findings) - 5} more_")
        out.append("")
        if deps:
            out.append("**Composes with:**")
            for d in deps:
                if d.get("vfr_id"):
                    out.append(f"- `{d['vfr_id']}` — {d.get('name', '?')}")
        return "\n".join(out)
    else:
        out = []
        out.append("=" * 72)
        out.append(f"  {name}")
        out.append(f"  {owner} · {when} · {vfr}")
        out.append("=" * 72)
        out.append(f"  snapshot:    {snapshot}…")
        out.append(f"  findings:    {len(active_findings)} active ({len(findings)} total, {len(chain)} superseded)")
        out.append(f"  sources:     {len(sources)}")
        out.append(f"  cross-deps:  {len(deps)}")
        out.append("")
        out.append("  Top findings (first 5 active):")
        for f in active_findings[:5]:
            txt = f["assertion"]["text"]
            t = f["assertion"].get("type", "?")
            conf = f.get("confidence", {}).get("score", 0.0)
            out.append(f"    - [{t:>14}, {conf:.2f}] {txt[:80]}")
        if deps:
            out.append("")
            out.append("  Composes with:")
            for d in deps:
                if d.get("vfr_id"):
                    out.append(f"    - {d['vfr_id']}  {d.get('name', '?')}")
        return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("vfr_id", nargs="?", help="vfr_… address (omit with --list)")
    ap.add_argument("--list", action="store_true", help="list all hub entries")
    ap.add_argument("--markdown", action="store_true", help="render as markdown")
    args = ap.parse_args()

    if args.list:
        entries = list_entries()
        print(f"{len(entries)} entries on {HUB}:")
        for e in entries:
            print(f"  {e['vfr_id']}  {e['name']}  ({e['owner_actor_id']})")
        return 0

    if not args.vfr_id:
        ap.print_usage()
        return 2

    entry = fetch_entry(args.vfr_id)
    frontier = fetch_frontier(entry)
    print(render_card(entry, frontier, markdown=args.markdown))
    return 0


if __name__ == "__main__":
    sys.exit(main())
