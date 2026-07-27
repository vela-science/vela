#!/usr/bin/env python3
"""Regenerate current signed objects with the independent JavaScript client."""

from __future__ import annotations

import filecmp
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    fixtures = root / "conformance" / "current-objects"
    emitter = root / "clients" / "javascript" / "vela_emit.mjs"
    node = shutil.which("node")
    if node is None:
        print("node is required for current-object interoperability", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="vela-current-objects-") as directory:
        work = Path(directory)
        for kind, stem in (("submission", "producer"), ("verification", "verifier")):
            seed = work / f"{stem}.seed.hex"
            shutil.copyfile(fixtures / f"{stem}.seed.hex", seed)
            seed.chmod(0o600)
            output = work / f"{kind}.json"
            result = subprocess.run(
                [
                    node,
                    str(emitter),
                    kind,
                    "--draft",
                    str(fixtures / f"{kind}-draft.json"),
                    "--seed-file",
                    str(seed),
                    "--output",
                    str(output),
                ],
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode != 0:
                print(result.stderr, file=sys.stderr)
                return 1
            if not filecmp.cmp(output, fixtures / f"{kind}.json", shallow=False):
                print(f"{kind} fixture differs from independent emitter output", file=sys.stderr)
                return 1
            print(f"  ok: independent JavaScript {kind}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
