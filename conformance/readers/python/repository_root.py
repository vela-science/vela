#!/usr/bin/env python3
"""Recompute a repository's root from a clean clone, with no Vela code.

The claim this protocol makes to a stranger is that the state is bytes and
anyone can recompute it. That claim is only worth as much as the cheapest
independent check of it, so here is the cheapest one: read
`.vela/repository.json`, canonicalize it under RFC 8785, hash it, and compare
against the root the repository is published under.

`canonical.py` next door is the JCS encoder and nothing more. This file is the
smallest thing that uses it to check a real repository, and it deliberately
imports nothing from Vela — not the CLI, not the Rust crates, not a schema
library. Its whole dependency set is `rfc8785` and `hashlib`.

**What it establishes.** That the manifest bytes in this clone hash to the root
they are named by. That is the anchor every other root in the repository hangs
from, and it is the number a reader would otherwise have to take from `vela
replay` — the tool whose output is what is being checked.

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
    manifest = json.loads(manifest_path.read_bytes())
    digest = hashlib.sha256(canonical_bytes(manifest)).hexdigest()
    return f"sha256:{digest}", manifest


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("repository", type=Path)
    parser.add_argument("--expect", default=None)
    options = parser.parse_args()

    root, manifest = repository_root(options.repository)
    report = {
        "schema": "vela.independent-repository-root.v1",
        "repository_id": manifest.get("repository_id"),
        "repository_root": root,
        "accepted_claims": len(manifest.get("accepted_claims", [])),
        "pending_claims": len(manifest.get("pending_claims", [])),
        "proposals": len(manifest.get("proposals", [])),
        "verifications": len(manifest.get("verifications", [])),
        # Named so nobody reads this as a full replay. See the module docstring.
        "checked": "manifest bytes hash to the root they are named by",
        "not_checked": "retained object roots, signatures, authority chain",
    }
    sys.stdout.write(json.dumps(report, indent=2, sort_keys=True) + "\n")

    if options.expect is not None and options.expect != root:
        sys.stderr.write(f"expected {options.expect}, computed {root}\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
