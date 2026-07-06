#!/usr/bin/env python3
"""submit.py — one command to submit a Sidon witness to the Vela record.

You found a Sidon set in the n-dimensional 0/1 cube larger than the current
best bound (see bounds.json). This submits it as a signed state transition
with YOUR key on it. The flow is:

    1. re-verify your witness with the frozen verifier (vela reproduce)
    2. build the lower-bound claim from the witness
    3. write it as a signed proposal into your checkout of the frontier
       (vela land), which self-publishes a local commit; you then git push
       that commit (to your fork) and open a PR

Publication is git-native: the hub re-derives its index from the frontier's
committed event log on every ingest sweep, so pushing the commit IS the write.
Acceptance into verified state is a separate human review step; this is the
external WRITE — "someone other than the maintainer signed a transition into
the frontier."

Prerequisites (one-time):
    - the `vela` binary on PATH         (cargo install, or a release binary)
    - a keypair + identity:  vela id create --handle <you>
    - a checkout of the frontier repo   (this script ships inside it, at
      examples/sidon-a309370 two levels up; fork it to get push access)

Usage:
    python3 submit.py <witness.json>                # inside a checkout of the repo
    python3 submit.py <witness.json> --dry-run      # verify + preview, write nothing
    python3 submit.py <witness.json> --frontier <path-to/examples/sidon-a309370>

A witness file is:
    {"kind": "sidon", "n": 20, "points": [[0,1,0,...], ...], "claimed_size": 1990}
each point a 0/1 vector of length n; the set is Sidon iff all pairwise sums
(with repetition) are distinct.
"""
import argparse, hashlib, json, os, re, shlex, subprocess, sys, urllib.request, datetime

# The Sidon frontier's id (recorded in the receipt).
DEFAULT_VFR = "vfr_496956067dc5ad79"
# bounds.json lives next to this script; also published in the public repo.
HERE = os.path.dirname(os.path.abspath(__file__))
BOUNDS_LOCAL = os.path.join(HERE, "bounds.json")
# The sidon frontier repo lives at examples/sidon-a309370 in the same tree; a
# producer who cloned the repo to get this script already has it two levels up.
DEFAULT_FRONTIER = os.path.normpath(os.path.join(HERE, "..", "..", "examples", "sidon-a309370"))
BOUNDS_URL = (
    "https://raw.githubusercontent.com/constellate-science/vela/main/"
    "frontiers/sidon-sets/bounds.json"
)


def die(msg):
    print(f"submit: {msg}", file=sys.stderr)
    sys.exit(1)


def sha256_file(path):
    return "sha256:" + hashlib.sha256(open(path, "rb").read()).hexdigest()


def self_check_sidon(points):
    """A pure-Python mirror of the frozen verifier: every pairwise sum (with
    repetition) of the 0/1 vectors must be distinct. Lets you sanity-check
    before you even have `vela` installed; the authoritative check is
    `vela reproduce`."""
    seen = set()
    for i in range(len(points)):
        for j in range(i, len(points)):
            s = tuple(a + b for a, b in zip(points[i], points[j]))
            if s in seen:
                return False, f"collision at points {i},{j}"
            seen.add(s)
    return True, f"{len(seen)} pairwise sums all distinct"


def current_best(n):
    """The current accepted lower bound for a(n). Returns `(best, status)` where
    status is "ok" (best is the recorded bound), "untracked" (bounds.json was
    read but has no entry for n — a fresh cell), or "unreadable" (no local copy
    and the public URL could not be fetched)."""
    data = None
    if os.path.exists(BOUNDS_LOCAL):
        data = json.load(open(BOUNDS_LOCAL))
    else:
        try:
            with urllib.request.urlopen(BOUNDS_URL, timeout=10) as r:
                data = json.loads(r.read())
        except Exception:
            return None, "unreadable"
    for b in data.get("bounds", []):
        if b["n"] == n:
            return b["best_lower_bound"], "ok"
    return None, "untracked"


def preflight_vela(vela):
    """Confirm `vela` resolves to the Rust CLI, not a shadowing binary (a conda
    `vela` Python package is a common one). Both can exit 0, so we check the
    OUTPUT shape: the real CLI prints `vela <version>`. Fail fast with an
    actionable message rather than a misleading "verifier rejected" later."""
    try:
        out = subprocess.run([vela, "--version"], capture_output=True, text=True, timeout=20)
    except FileNotFoundError:
        die(f"`{vela}` not found on your PATH. Install the Rust CLI\n"
            "    cargo install --git https://github.com/constellate-science/vela vela-cli\n"
            "or pass the full path with --vela /path/to/vela")
    first = (out.stdout + out.stderr).strip().splitlines()
    first = first[0] if first else ""
    parts = first.split()
    if not (len(parts) >= 2 and parts[0] == "vela" and parts[1][:1].isdigit()):
        die(f"the `{vela}` on your PATH is not the Vela CLI (it printed: {first!r}).\n"
            "You likely have a different `vela` shadowing it (e.g. a Python package).\n"
            "Install the Rust CLI and run it by full path, e.g.\n"
            "    cargo install --git https://github.com/constellate-science/vela vela-cli\n"
            "    python3 submit.py <witness.json> --vela ~/.cargo/bin/vela")


def build_finding(n, size, witness):
    """A minimal, faithful finding bundle: the lower-bound claim plus the
    witness as a sibling of `finding` in the payload (the verifier re-checks
    the witness; the reviewer accepts the transition)."""
    text = (
        f"OEIS A309370 a({n}) >= {size}: a Sidon set of {size} distinct binary "
        f"vectors in {{0,1}}^{n} under componentwise integer addition, all "
        f"pairwise sums distinct. Frozen-verified by vela-verify (sidon kind)."
    )
    finding = {
        "assertion": {"text": text, "type": "computational",
                      "direction": None, "relation": None, "entities": []},
        "confidence": {
            "kind": "frontier_epistemic", "method": "frozen_verifier",
            "score": 1.0, "extraction_confidence": 1.0,
            "basis": "deterministic re-check by vela-verify (sidon); review required for acceptance",
        },
        "flags": {"contested": False, "declining": False, "gap": False,
                  "gravity_well": False, "negative_space": False, "retracted": False},
        "evidence": {"evidence_type": "computational", "effect_size": None},
    }
    return {"finding": finding, "witness": witness}


def main():
    ap = argparse.ArgumentParser(description="Submit a Sidon witness to the Vela record.")
    ap.add_argument("witness", help="path to the witness JSON")
    ap.add_argument("--vfr", default=DEFAULT_VFR, help="frontier id (vfr_…), recorded in the receipt")
    ap.add_argument("--frontier", default=DEFAULT_FRONTIER,
                    help="path to your checkout of the sidon frontier (examples/sidon-a309370). "
                         "`vela land` writes the signed proposal here; you then git push it and open a PR.")
    ap.add_argument("--actor", default=None,
                    help="your actor id, e.g. reviewer:alice (default: your configured `vela id`)")
    ap.add_argument("--vela", default="vela", help="path to the vela binary")
    ap.add_argument("--dry-run", action="store_true", help="verify + preview the write, but change nothing")
    args = ap.parse_args()

    # ── 0. confirm `vela` is the Rust CLI (fail fast, clear message) ─────
    preflight_vela(args.vela)

    # ── 1. read + sanity-check the witness ───────────────────────────────
    try:
        w = json.load(open(args.witness))
    except Exception as e:
        die(f"cannot read witness {args.witness}: {e}")
    if w.get("kind") != "sidon":
        die(f"not a sidon witness (kind={w.get('kind')!r})")
    n = w.get("n")
    pts = w.get("points") or []
    size = w.get("claimed_size") or len(pts)
    if not isinstance(n, int) or not pts:
        die("witness missing integer `n` or non-empty `points`")
    # Pure-Python self-check is O(size^2); skip it for large sets (the frozen
    # Rust verifier below is the authoritative, fast check either way).
    if len(pts) <= 1500:
        ok, detail = self_check_sidon(pts)
        if not ok:
            die(f"witness is NOT a Sidon set: {detail}")
        print(f"  self-check  ok   a({n}) >= {size}  ({detail})")
    else:
        print(f"  self-check  skipped (size {size} > 1500); relying on vela reproduce")

    # ── 2. frozen verifier (authoritative) ───────────────────────────────
    try:
        out = subprocess.run([args.vela, "reproduce", args.witness],
                             capture_output=True, text=True)
    except FileNotFoundError:
        die(f"`{args.vela}` not found. Install vela, or pass --vela <path>. "
            "(The self-check above already passed, but the record requires the frozen verifier.)")
    repro = (out.stdout + out.stderr).strip()
    if out.returncode != 0:
        die(f"frozen verifier rejected the witness:\n{repro}")
    print(f"  vela reproduce  ok   {repro.splitlines()[0].strip()}")

    # ── 3. is it actually a beat? ────────────────────────────────────────
    best, status = current_best(n)
    if status == "unreadable":
        verdict = "unknown (bounds.json not found locally and the public copy could not be fetched)"
    elif status == "untracked":
        verdict = f"NEW CELL — a(n={n}) is not yet on the record; this would be its first lower bound"
    elif size > best:
        verdict = f"BEATS the current best a({n}) >= {best} by {size - best}"
    elif size == best:
        verdict = f"TIES the current best a({n}) >= {best} (independent confirmation)"
    else:
        verdict = f"BELOW the current best a({n}) >= {best}; submitting anyway is allowed but will not improve the bound"
    print(f"  frontier    {verdict}")

    # ── 4. build the finding bundle payload ──────────────────────────────
    payload = build_finding(n, size, w)
    reason = f"Sidon lower bound a({n}) >= {size}, frozen-verified (sidon)."

    claim = payload["finding"]["assertion"]["text"]
    caveat = ("Admission to the log is not verification; this lower-bound claim "
              "must earn `verified` at the gate before it counts as held state.")
    frontier = os.path.abspath(args.frontier)
    land_cmd = [args.vela, "land", "--frontier", frontier,
                "--claim", claim, "--artifact", os.path.abspath(args.witness),
                "--caveat", caveat, "--json"]
    if args.actor:
        land_cmd += ["--as", args.actor]

    landable = os.path.isdir(os.path.join(frontier, ".vela"))

    if args.dry_run:
        print("\n  --dry-run: writing nothing. This would create a signed proposal via:")
        print("    " + " ".join(shlex.quote(c) for c in land_cmd[:-1]))  # hide --json in the preview
        if not landable:
            print(f"  note: {frontier} is not a writable frontier checkout (no .vela store) —")
            print("        pass --frontier <a clone of the sidon frontier's git repo> to write for real.")
        print("  then publish it: git push your commit (to your fork) and open a PR at")
        print("    https://github.com/constellate-science/vela")
        return

    # ── 5. write the signed proposal into the frontier repo (vela land) ──
    if not landable:
        die(f"no writable frontier checkout at {frontier} (no .vela store).\n"
            "This is the git-native write path: `vela land` records the signed proposal into a\n"
            "clone of the frontier's own git repo, which you then push. Point --frontier at such a\n"
            "checkout. (The examples/ dirs in the main repo are re-verifiable snapshots, not\n"
            "writable frontiers — see docs/HUB.md `register-git` for how a frontier is published.)")
    res = subprocess.run(land_cmd, capture_output=True, text=True)
    land_out = (res.stdout + res.stderr).strip()
    if res.returncode != 0:
        die(f"vela land failed:\n{land_out}")
    print("\n  " + land_out.replace("\n", "\n  "))

    # ── 6. emit a citable receipt ────────────────────────────────────────
    # `vela land` interleaves human lines with its JSON, so pull the proposal id
    # out of the combined output directly rather than json.loads()-ing all of it.
    m = re.search(r"vpr_[0-9a-f]+", land_out)
    proposal_id = m.group(0) if m else None
    receipt = {
        "ok": True,
        "frontier_id": args.vfr,
        "sequence": "oeis:A309370",
        "claim": claim,
        "n": n, "size": size,
        "beats": {"previous_best": best, "delta": (size - best) if best is not None else None},
        "witness_sha256": sha256_file(args.witness),
        "verifier": {"kind": "sidon", "crate": "vela-verify", "reproduce": repro.splitlines()[0].strip()},
        "proposal_id": proposal_id,
        "status": "recorded (git-local; push to publish)",
        "submitted_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "note": "The signed proposal is committed in your frontier checkout. Publish it: git push the "
                "commit to your fork and open a PR. The hub re-derives its index from the push.",
    }
    print("\n  receipt:")
    print(json.dumps(receipt, indent=2))
    print("\n  next: git push this commit (to your fork) and open a PR at")
    print("    https://github.com/constellate-science/vela")


if __name__ == "__main__":
    main()
