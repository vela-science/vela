#!/usr/bin/env python3
"""Recompute a repository's root from a clean clone, with no Vela code.

The claim this protocol makes to a stranger is that the state is bytes and
anyone can recompute it. That claim is only worth as much as the cheapest
independent check of it, so here is the cheapest one: read
`.vela/repository.json`, hold it to its own canonical form under RFC 8785, hash
it, and compare against a root the caller supplies out of band.

`canonical.py` next door is the JCS encoder and nothing more. This file is the
smallest thing that uses it to check a real repository, and it deliberately
imports nothing from Vela — not the CLI, not the Rust crates, not a schema
library. Its whole dependency set is `rfc8785` and `hashlib`.

**What it establishes.** Two things, and only when `--expect` is supplied.
First, that the manifest file's bytes are already canonical — re-encoding them
under RFC 8785 reproduces them exactly, which is the same refusal
`CurrentRepositoryV4::parse` makes. Second, that their SHA-256 equals the root
the caller supplied out of band.

Nothing in the clone names the root. `.vela/repository.json` has no
`repository_root` field, so there is no self-check available here: without
`--expect` this computes a number and compares it to nothing, and says so in
its own output rather than reporting a check it did not make.

**What it does not establish**, stated because a check that overstates itself
is worse than none:

- Not that each retained object hashes to the root the manifest binds it to.
  `vela replay` walks the whole object set; this reads one file.
- Not that the authority chain is intact, that signatures verify, or that any
  Decision was authorized.
- Not that the Claims are true. No verifier ran here.

Usage:

    python conformance/readers/python/repository_root.py <repository> [--expect sha256:...]

Exits non-zero if `--expect` is given and disagrees.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from canonical import canonical_bytes


def repository_root(repository: Path) -> tuple[str, dict]:
    manifest_path = repository / ".vela" / "repository.json"
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    canonical = canonical_bytes(manifest)
    # Hashing the re-encoded value while claiming to hash the file would pass a
    # reindented manifest that `vela` itself refuses
    # (repository.rs: "current repository bytes are not canonical
    # JSON"). Holding the file to its own canonical form first is what makes
    # the digest a statement about the bytes on disk.
    if raw != canonical:
        raise SystemExit(
            f"{manifest_path} is not canonical JSON; its bytes are not the bytes its root is over"
        )
    return f"sha256:{hashlib.sha256(canonical).hexdigest()}", manifest


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("repository", type=Path)
    parser.add_argument("--expect", default=None)
    options = parser.parse_args()

    root, manifest = repository_root(options.repository)
    expected_matched = options.expect is not None and options.expect == root
    report = {
        "schema": "vela.independent-repository-root.v1",
        "repository_id": manifest.get("repository_id"),
        "repository_root": root,
        "accepted_claims": len(manifest.get("accepted_claims", [])),
        "pending_claims": len(manifest.get("pending_claims", [])),
        "proposals": len(manifest.get("proposals", [])),
        "verifications": len(manifest.get("verifications", [])),
        # Named so nobody reads this as a full replay, and so the name is
        # earned on every path: with no expected root there is nothing to
        # compare against and this says that rather than implying a match.
        "checked": (
            "manifest bytes are canonical and hash to the supplied root"
            if expected_matched
            else "manifest bytes are canonical; no expected root was supplied, so nothing was compared"
        ),
        "not_checked": "retained object roots, signatures, authority chain",
    }
    sys.stdout.write(json.dumps(report, indent=2, sort_keys=True) + "\n")

    if options.expect is not None and options.expect != root:
        sys.stderr.write(f"expected {options.expect}, computed {root}\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
