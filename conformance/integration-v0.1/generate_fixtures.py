#!/usr/bin/env python3
"""Regenerate the integration corpus from its compact semantic source."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import sys
from pathlib import Path

import rfc8785

HERE = Path(__file__).resolve().parent
SCHEMAS = {
    "manifest": "vela.integration-manifest.v0.1",
    "profile": "vela.integration-profile.v0.1",
    "binding": "vela.integration-binding.v0.1",
    "method": "vela.integration-method.v0.1",
}
ROOT_FIELDS = {
    "manifest": "manifest_root",
    "profile": "profile_root",
    "binding": "binding_root",
    "method": "method_root",
}


def root(kind: str, value: dict[str, object], domain: bytes | None = None) -> str:
    normalized = copy.deepcopy(value)
    normalized[ROOT_FIELDS[kind]] = ""
    framing = domain if domain is not None else SCHEMAS[kind].encode() + b"\0"
    return "sha256:" + hashlib.sha256(framing + rfc8785.dumps(normalized)).hexdigest()


def packet() -> dict[str, object]:
    data = json.loads((HERE / "fixtures.source.json").read_text())
    docs = data["packet"]
    docs["profile"]["profile_root"] = root("profile", docs["profile"])
    docs["method"]["method_root"] = root("method", docs["method"])
    docs["binding"]["profile"]["root"] = docs["profile"]["profile_root"]
    docs["binding"]["methods"][0]["root"] = docs["method"]["method_root"]
    docs["binding"]["binding_root"] = root("binding", docs["binding"])
    manifest = docs["manifest"]
    manifest["profiles"][0]["root"] = docs["profile"]["profile_root"]
    manifest["bindings"][0]["root"] = docs["binding"]["binding_root"]
    manifest["methods"][0]["root"] = docs["method"]["method_root"]
    manifest["manifest_root"] = root("manifest", manifest)
    docs["check_output"]["method_root"] = docs["method"]["method_root"]
    for case in data["hostile"]:
        if case[0] == "wrong_root_domain":
            case[3] = root("profile", docs["profile"], b"wrong-domain\0")
    names = {case[0] for case in data["hostile"]}
    additions = [
        [
            "binding_method_unknown_field",
            "binding",
            "methods.0.unexpected",
            "shared-envelope-open",
        ]
    ]
    data["hostile"].extend(case for case in additions if case[0] not in names)
    return data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    encoded = json.dumps(packet(), indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    path = HERE / "fixtures.json"
    if args.check:
        if not path.exists() or path.read_text() != encoded:
            print("integration-v0.1 fixtures drift; regenerate", file=sys.stderr)
            return 1
        print("integration-v0.1 fixture regeneration: ok")
        return 0
    path.write_text(encoded)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
