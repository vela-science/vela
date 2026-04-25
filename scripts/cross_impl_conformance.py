#!/usr/bin/env python3
"""
Vela cross-implementation conformance validator.

A second, independent implementation of the v0.3 content-addressing rules.
It loads a Vela frontier JSON file and re-derives every content-addressed
ID from the canonical-JSON spec alone, then compares against the IDs
stored in the file. If any pair disagrees, the substrate's bit-stability
claim is broken.

Implements only what the protocol normatively specifies:
  - canonical JSON (sorted keys at every depth, no whitespace, finite
    numbers, UTF-8 verbatim)
  - vf_<16hex>  finding id      = sha256(normalize(text) "|" type "|" provid)[:16]
  - vev_<16hex> event id        = sha256(canonical(event-preimage))[:16]
  - vpr_<16hex> proposal id     = sha256(canonical(proposal-preimage))[:16]
  - vfr_<16hex> frontier id     = sha256(canonical({name,compiled_at,compiler}))[:16]
  - sha256:<64hex> finding hash = "sha256:" + sha256(canonical(finding))
  - <64hex>       event_log_hash= sha256(canonical(events))
  - <64hex>       snapshot_hash = sha256(canonical(frontier minus events,signatures,proof_state))

Usage:
    scripts/cross_impl_conformance.py frontiers/bbb-alzheimer.json
    scripts/cross_impl_conformance.py frontiers/bbb-alzheimer.json --json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from typing import Any


# --- canonical JSON ---------------------------------------------------------

def canonicalize(value: Any) -> Any:
    """Recursively sort object keys; reject non-finite numbers."""
    if isinstance(value, dict):
        return {k: canonicalize(value[k]) for k in sorted(value.keys())}
    if isinstance(value, list):
        return [canonicalize(v) for v in value]
    if isinstance(value, float):
        if value != value or value in (float("inf"), float("-inf")):
            raise ValueError("canonical: non-finite float")
    return value


def to_canonical_bytes(value: Any) -> bytes:
    canon = canonicalize(value)
    # `separators=(",", ":")` strips whitespace.
    # `ensure_ascii=False` preserves UTF-8 (matches Rust's serde_json default
    # which does not \u-escape non-ASCII).
    # `allow_nan=False` rejects NaN/Inf at encode time too.
    return json.dumps(
        canon,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_hex(value: Any) -> str:
    return hashlib.sha256(to_canonical_bytes(value)).hexdigest()


# --- ID derivations ---------------------------------------------------------

_TRAILING_PUNCT = re.compile(r"[.;:!?]+$")


def normalize_text(s: str) -> str:
    """Match `FindingBundle::normalize_text` in bundle.rs."""
    lower = s.lower()
    collapsed = " ".join(lower.split())
    return _TRAILING_PUNCT.sub("", collapsed)


def derive_finding_id(finding: dict) -> str:
    assertion = finding["assertion"]
    prov = finding["provenance"]
    prov_id = prov.get("doi") or prov.get("pmid") or prov.get("title", "")
    # Serde rename: Rust `assertion_type` is JSON key `type`.
    preimage = f"{normalize_text(assertion['text'])}|{assertion['type']}|{prov_id}"
    digest = hashlib.sha256(preimage.encode("utf-8")).hexdigest()
    return f"vf_{digest[:16]}"


def derive_event_id(event: dict) -> str:
    preimage = {
        "schema": event["schema"],
        "kind": event["kind"],
        "target": event["target"],
        "actor": event["actor"],
        "timestamp": event["timestamp"],
        "reason": event["reason"],
        "before_hash": event["before_hash"],
        "after_hash": event["after_hash"],
        "payload": event["payload"],
        "caveats": event.get("caveats", []),
    }
    return f"vev_{sha256_hex(preimage)[:16]}"


def derive_proposal_id(proposal: dict) -> str:
    """Phase P (v0.5): `created_at` is excluded from the canonical preimage so
    identical logical proposals at different times deterministically produce
    the same `vpr_…`. created_at remains on the proposal as non-canonical
    metadata."""
    preimage = {
        "schema": proposal["schema"],
        "kind": proposal["kind"],
        "target": proposal["target"],
        "actor": proposal["actor"],
        "reason": proposal["reason"],
        "payload": proposal["payload"],
        "source_refs": proposal.get("source_refs", []),
        "caveats": proposal.get("caveats", []),
    }
    return f"vpr_{sha256_hex(preimage)[:16]}"


def derive_frontier_id_from_meta(meta: dict) -> str:
    """Legacy v0.3 fallback when no `frontier.created` genesis event."""
    preimage = {
        "name": meta["name"],
        "compiled_at": meta["compiled_at"],
        "compiler": meta["compiler"],
    }
    return f"vfr_{sha256_hex(preimage)[:16]}"


def derive_frontier_id_from_genesis(events: list) -> str | None:
    """v0.4: vfr_… derives from canonical hash of `events[0]` when it's
    a `frontier.created` event. Same preimage shape as `vev_…`."""
    if not events:
        return None
    genesis = events[0]
    if genesis.get("kind") != "frontier.created":
        return None
    preimage = {
        "schema": genesis["schema"],
        "kind": genesis["kind"],
        "target": genesis["target"],
        "actor": genesis["actor"],
        "timestamp": genesis["timestamp"],
        "reason": genesis["reason"],
        "before_hash": genesis["before_hash"],
        "after_hash": genesis["after_hash"],
        "payload": genesis["payload"],
        "caveats": genesis.get("caveats", []),
    }
    return f"vfr_{sha256_hex(preimage)[:16]}"


def derive_frontier_id(frontier: dict) -> str:
    """Combined: prefer genesis-derived; fall back to meta-derivation."""
    derived = derive_frontier_id_from_genesis(frontier.get("events", []))
    if derived:
        return derived
    return derive_frontier_id_from_meta(frontier["frontier"])


def derive_finding_hash(finding: dict) -> str:
    return f"sha256:{sha256_hex(finding)}"


def derive_event_log_hash(events: list) -> str:
    return sha256_hex(events)


def derive_snapshot_hash(frontier: dict) -> str:
    snapshot = {k: v for k, v in frontier.items()
                if k not in ("events", "signatures", "proof_state")}
    return sha256_hex(snapshot)


# --- validation pass --------------------------------------------------------

class Result:
    def __init__(self) -> None:
        self.checks: list[dict] = []
        self.mismatches = 0
        self.computed: dict[str, Any] = {}

    def check(self, kind: str, label: str, expected: str, actual: str) -> None:
        ok = expected == actual
        if not ok:
            self.mismatches += 1
        self.checks.append({
            "kind": kind,
            "label": label,
            "expected": expected,
            "actual": actual,
            "ok": ok,
        })


def validate(frontier: dict) -> Result:
    result = Result()

    # findings: vf_<hash>
    for finding in frontier.get("findings", []):
        derived = derive_finding_id(finding)
        result.check("finding_id", finding["id"], finding["id"], derived)

    # events: vev_<hash>
    for event in frontier.get("events", []):
        derived = derive_event_id(event)
        result.check("event_id", event["id"], event["id"], derived)

    # proposals: vpr_<hash>
    for proposal in frontier.get("proposals", []):
        derived = derive_proposal_id(proposal)
        result.check("proposal_id", proposal["id"], proposal["id"], derived)

    # frontier_id: vfr_<hash> — derives from `frontier.created` genesis
    # event if present, else from frontier metadata (legacy v0.3).
    if frontier.get("frontier_id"):
        derived = derive_frontier_id(frontier)
        result.check("frontier_id", "frontier_id", frontier["frontier_id"], derived)

    # snapshot + event log hashes (computed, not stored — emit for CLI cross-check)
    result.computed["snapshot_hash"] = derive_snapshot_hash(frontier)
    result.computed["event_log_hash"] = derive_event_log_hash(frontier.get("events", []))
    result.computed["finding_count"] = len(frontier.get("findings", []))
    result.computed["event_count"] = len(frontier.get("events", []))
    result.computed["proposal_count"] = len(frontier.get("proposals", []))

    return result


# --- main -------------------------------------------------------------------

def parse_link_target(s: str) -> tuple[str, str | None]:
    """v0.8: parse `vf_<id>` (local) or `vf_<id>@vfr_<id>` (cross-frontier).

    Returns (vf_id, vfr_id_or_None). Mirrors the canonical Rust
    `LinkRef::parse` behaviour.
    """
    if not s:
        raise ValueError("empty link target")
    if s.count("@") > 1:
        raise ValueError(f"link target has more than one '@': {s!r}")
    if "@" in s:
        local, remote = s.split("@", 1)
        if not local.startswith("vf_") or len(local) <= 3:
            raise ValueError(f"link target's vf_ part is malformed: {s!r}")
        if not remote.startswith("vfr_") or len(remote) <= 4:
            raise ValueError(f"link target's vfr_ part is malformed: {s!r}")
        return local, remote
    if not s.startswith("vf_") or len(s) <= 3:
        raise ValueError(f"link target must start with 'vf_': {s!r}")
    return s, None


def validate_cross_frontier(
    primary: dict[str, Any],
    deps: dict[str, dict[str, Any]],
) -> tuple[bool, list[str]]:
    """v0.8: validate that every cross-frontier link in `primary` resolves
    to a declared dep whose `pinned_snapshot_hash` matches the actual
    snapshot of the loaded dep frontier (`deps[vfr_id]`).
    """
    errors: list[str] = []
    declared = {
        d.get("vfr_id"): d
        for d in primary.get("frontier", {}).get("dependencies", [])
        if d.get("vfr_id")
    }

    for f in primary.get("findings", []):
        for link in f.get("links", []):
            try:
                _, vfr_id = parse_link_target(link.get("target", ""))
            except ValueError as e:
                errors.append(f"link parse: {e}")
                continue
            if vfr_id is None:
                continue  # local link — not a cross-frontier check
            dep = declared.get(vfr_id)
            if dep is None:
                errors.append(
                    f"finding {f.get('id')} → {link.get('target')}: "
                    f"vfr_id {vfr_id!r} not declared in frontier.dependencies"
                )
                continue
            pinned = dep.get("pinned_snapshot_hash") or ""
            if not pinned:
                errors.append(
                    f"dep {vfr_id}: missing pinned_snapshot_hash"
                )
                continue
            actual_dep = deps.get(vfr_id)
            if actual_dep is None:
                errors.append(
                    f"dep {vfr_id}: not provided to validator (use --cross-frontier <path>)"
                )
                continue
            actual_snap = derive_snapshot_hash(actual_dep)
            if actual_snap != pinned:
                errors.append(
                    f"dep {vfr_id}: pinned snapshot {pinned} but actual is {actual_snap}"
                )
    return (not errors), errors


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("frontier", help="Path to a Vela frontier JSON file")
    ap.add_argument(
        "--cross-frontier",
        action="append",
        default=[],
        metavar="PATH",
        help="v0.8: also load this frontier as a declared dependency and "
        "verify the cross-frontier link resolution + snapshot pin. May be "
        "repeated for multiple deps.",
    )
    ap.add_argument("--json", action="store_true", help="Emit JSON result")
    args = ap.parse_args()

    with open(args.frontier, encoding="utf-8") as f:
        frontier = json.load(f)

    result = validate(frontier)

    # v0.8: cross-frontier resolution check.
    cross_ok = True
    cross_errors: list[str] = []
    if args.cross_frontier:
        deps: dict[str, dict[str, Any]] = {}
        for path in args.cross_frontier:
            with open(path, encoding="utf-8") as fh:
                dep_frontier = json.load(fh)
            dep_vfr = derive_frontier_id(dep_frontier)
            deps[dep_vfr] = dep_frontier
        cross_ok, cross_errors = validate_cross_frontier(frontier, deps)

    if args.json:
        print(json.dumps({
            "ok": result.mismatches == 0 and cross_ok,
            "mismatches": result.mismatches,
            "checks": result.checks,
            "computed": result.computed,
            "cross_frontier": {
                "ok": cross_ok,
                "errors": cross_errors,
                "deps_loaded": len(args.cross_frontier),
            },
        }, indent=2))
        return 0 if (result.mismatches == 0 and cross_ok) else 1

    by_kind: dict[str, tuple[int, int]] = {}
    for c in result.checks:
        ok, total = by_kind.get(c["kind"], (0, 0))
        by_kind[c["kind"]] = (ok + (1 if c["ok"] else 0), total + 1)

    print(f"Vela cross-implementation conformance · {args.frontier}")
    print(f"  schema:        {frontier.get('schema', '?')}")
    print(f"  vela_version:  {frontier.get('vela_version', '?')}")
    print()
    print("Re-derived IDs vs stored IDs:")
    for kind, (ok, total) in sorted(by_kind.items()):
        marker = "ok" if ok == total else "FAIL"
        print(f"  {kind:14} {ok}/{total:<4} {marker}")
    print()
    print("Computed (cross-check with `vela check --json`):")
    print(f"  snapshot_hash:  {result.computed['snapshot_hash']}")
    print(f"  event_log_hash: {result.computed['event_log_hash']}")
    print()
    print(f"Counts: {result.computed['finding_count']} findings, "
          f"{result.computed['event_count']} events, "
          f"{result.computed['proposal_count']} proposals")
    print()

    if result.mismatches:
        print(f"FAIL — {result.mismatches} mismatch(es):")
        for c in result.checks:
            if not c["ok"]:
                print(f"  [{c['kind']}] {c['label']}")
                print(f"    stored:  {c['expected']}")
                print(f"    derived: {c['actual']}")
        return 1

    if args.cross_frontier:
        print(f"Cross-frontier resolution ({len(args.cross_frontier)} dep(s) loaded):")
        if cross_ok:
            print("  ok all cross-frontier links resolve to declared deps")
            print("  ok every dep's pinned_snapshot_hash matches its actual snapshot")
        else:
            print(f"  FAIL — {len(cross_errors)} issue(s):")
            for e in cross_errors:
                print(f"    · {e}")
            return 1
        print()

    print("PASS — every content-addressed ID re-derives bit-identically from canonical JSON alone.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
