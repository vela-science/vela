#!/usr/bin/env python3
"""Regenerate current signed objects with every independent client.

One independent emitter shows the specification is followable. Two show it is
followable the same way — and the two here differ where it matters:
`javascript.mjs` hand-rolls canonicalization and sorts keys by UTF-16 code
unit, while `python.py` calls `rfc8785`, which sorts by code point as JCS
specifies. Both must reproduce the fixture byte for byte.
"""

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
    emitters = root / "conformance" / "emitters"
    node = shutil.which("node")
    if node is None:
        print("node is required for current-object interoperability", file=sys.stderr)
        return 2
    clients = (
        ("JavaScript", [node, str(emitters / "javascript.mjs")]),
        ("Python", [sys.executable, str(emitters / "python.py")]),
    )

    with tempfile.TemporaryDirectory(prefix="vela-current-objects-") as directory:
        work = Path(directory)
        for language, command in clients:
            for kind, stem in (("submission", "producer"), ("verification", "verifier")):
                seed = work / f"{language}-{stem}.seed.hex"
                shutil.copyfile(fixtures / f"{stem}.seed.hex", seed)
                seed.chmod(0o600)
                output = work / f"{language}-{kind}.json"
                result = subprocess.run(
                    [
                        *command,
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
                    timeout=60,
                )
                if result.returncode != 0:
                    print(result.stderr, file=sys.stderr)
                    return 1
                if not filecmp.cmp(output, fixtures / f"{kind}.json", shallow=False):
                    print(
                        f"{kind} fixture differs from independent {language} emitter output",
                        file=sys.stderr,
                    )
                    return 1
                print(f"  ok: independent {language} {kind}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
