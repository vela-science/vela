#!/usr/bin/env python3
"""Regenerate current signed objects with every independent client.

One independent emitter shows the specification is followable. Two show it is
followable the same way: independent Ed25519 stacks, independent argument
parsing, independent JSON handling, and both must reproduce the fixture byte
for byte.
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

    # The signer identity is the emitter's argument, not a draft field: a
    # producer must not be able to smuggle an actor or a key through the
    # scientific content it is asking someone to sign.
    signers = {
        "submission": ("agent:independent-js", "agent", "2026-07-27T12:00:00Z"),
        "verification": ("verifier:independent-js", "org", "2026-07-27T12:05:00Z"),
    }

    with tempfile.TemporaryDirectory(prefix="vela-current-objects-") as directory:
        work = Path(directory)
        for language, command in clients:
            for kind, stem in (("submission", "producer"), ("verification", "verifier")):
                actor, actor_class, declared_at = signers[kind]
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
                        "--actor",
                        actor,
                        "--actor-class",
                        actor_class,
                        "--declared-at",
                        declared_at,
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
