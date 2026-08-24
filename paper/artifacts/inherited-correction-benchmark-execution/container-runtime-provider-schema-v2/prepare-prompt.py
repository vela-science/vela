#!/usr/bin/env python3
"""Serialize one packet into an immutable, path-preserving prompt."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--packet", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--condition", required=True)
    args = parser.parse_args()
    packet = args.packet.resolve()
    files = []
    for path in sorted(item for item in packet.rglob("*") if item.is_file()):
        relative = path.relative_to(packet).as_posix()
        data = path.read_bytes()
        files.append(
            {
                "path": relative,
                "bytes": len(data),
                "sha256": digest(data),
                "content_base64": base64.b64encode(data).decode("ascii"),
            }
        )
    envelope = {
        "schema": "neutral.immutable-virtual-filesystem-prompt.v1",
        "condition": args.condition,
        "ordering": "lexicographic POSIX relative path",
        "encoding": "base64 of exact file bytes",
        "virtual_files": files,
    }
    prompt = (
        "You are one context-isolated participant. The complete immutable virtual "
        "filesystem follows. Decode it mentally; do not call or request any tool, "
        "file, shell, network, continuation, or compaction. Follow TASK.md and return "
        "only one JSON object matching response-schema.json.\n\n"
        + json.dumps(envelope, ensure_ascii=False, indent=2, sort_keys=True)
        + "\n"
    ).encode("utf-8")
    args.output.write_bytes(prompt)
    print(json.dumps({"prompt_bytes": digest(prompt), "files": files}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
