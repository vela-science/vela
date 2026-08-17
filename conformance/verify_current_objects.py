#!/usr/bin/env python3
"""Regenerate current signed objects with every independent client.

One independent emitter shows the specification is followable. Two show it is
followable the same way: independent Ed25519 stacks, independent argument
parsing, independent JSON handling, and both must reproduce the fixture byte
for byte.
"""

from __future__ import annotations

import base64
import filecmp
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "readers" / "python"))
from canonical import canonical_bytes


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
    readers = (
        (
            "JavaScript",
            [node, str(root / "conformance/readers/javascript/object.mjs")],
        ),
        (
            "Python",
            [sys.executable, str(root / "conformance/readers/python/object.py")],
        ),
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
            for kind, stem in (
                ("submission", "producer"),
                ("verification", "verifier"),
            ):
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
                    check=False,
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

        for kind in ("submission", "verification"):
            results = []
            for language, command in readers:
                result = subprocess.run(
                    [*command, str(fixtures / f"{kind}.json")],
                    capture_output=True,
                    text=True,
                    timeout=60,
                    check=False,
                )
                if result.returncode != 0:
                    print(result.stderr, file=sys.stderr)
                    return 1
                results.append(json.loads(result.stdout))
                print(f"  ok: independent {language} {kind} reader")
            if results[0] != results[1]:
                print(f"{kind} reader summaries disagree", file=sys.stderr)
                return 1

        current = json.loads((fixtures / "submission.json").read_text(encoding="utf-8"))
        payload = json.loads(base64.b64decode(current["payload"], validate=True))
        negatives = []

        retired_type = json.loads(json.dumps(current))
        retired_type["payloadType"] = "application/vnd.vela.submission.v2+json"
        negatives.append(("retired v2 payload type", retired_type))

        retired_schema = json.loads(json.dumps(current))
        payload_v2 = json.loads(json.dumps(payload))
        payload_v2["schema"] = "vela.submission.v2"
        retired_schema["payload"] = base64.b64encode(
            canonical_bytes(payload_v2)
        ).decode("ascii")
        negatives.append(("retired v2 schema", retired_schema))

        execution_binding = json.loads(json.dumps(current))
        payload_binding = json.loads(json.dumps(payload))
        payload_binding["execution_binding"] = {
            "schema": "vela.execution-binding.v1",
            "packet_root": "sha256:" + "a" * 64,
            "profile_root": "sha256:" + "b" * 64,
            "verifier_capsule_root": "sha256:" + "c" * 64,
            "result_contract_root": "sha256:" + "d" * 64,
        }
        execution_binding["payload"] = base64.b64encode(
            canonical_bytes(payload_binding)
        ).decode("ascii")
        negatives.append(("retired execution_binding field", execution_binding))

        for label, envelope in negatives:
            path = work / f"negative-{label.replace(' ', '-')}.json"
            path.write_bytes(canonical_bytes(envelope))
            for language, command in readers:
                result = subprocess.run(
                    [*command, str(path)],
                    capture_output=True,
                    text=True,
                    timeout=60,
                    check=False,
                )
                if result.returncode == 0:
                    print(f"{language} reader accepted {label}", file=sys.stderr)
                    return 1
                print(f"  ok: independent {language} reader refused {label}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
