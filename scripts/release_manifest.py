#!/usr/bin/env python3
"""Emit the manifest that binds one release bundle to the state it came from.

Per-asset provenance already exists (`actions/attest-build-provenance`), but it
is OIDC-bound to one CI provider and it attests each file separately. Nothing
tied the archive, the SBOM, the source commit and tree, the compiler, and the
target triple together in one document that a consumer could verify with
OpenSSH and `shasum` alone. This is that document.

It is not a protocol object. `docs/ROOTS.md` classes a release checksum as
"exact binary/archive bytes", explicitly not a substitute for a source commit or
a build attestation, and this manifest is the same kind of thing one level up:
distribution evidence, carrying no Standing and no authority. The schema id is
deliberately unrelated to `vela.observatory-release-manifest.vN`, which is a
vela-web read projection and shares nothing with this but the English word.

Called by `scripts/release.sh`. Every digest here is computed from bytes on
disk at the moment of the call; nothing is copied from a log or a build output.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import subprocess
import sys


ATTESTATION_NOTE = (
    "actions/attest-build-provenance is bound to a GitHub Actions OIDC identity "
    "and has no provider-neutral equivalent, so it stays in "
    ".github/workflows/release.yml and is absent from a local build. This "
    "manifest binds the same bytes without it; it does not replace it."
)

SIGNATURE_NOTE = (
    "Signed out of band as a detached OpenSSH signature over these exact bytes, "
    "namespace vela-release. The signing identity is a distribution identity. "
    "It is never the repository-authority key: publishing software is not a "
    "scientific Decision (docs/SIGNING.md)."
)


def digest(path: pathlib.Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(chunk)
    return "sha256:" + hasher.hexdigest()


def git(*arguments: str) -> str | None:
    try:
        completed = subprocess.run(
            ["git", *arguments],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout.strip() or None


def source_state() -> dict[str, object]:
    commit = git("rev-parse", "HEAD")
    if commit is None:
        # A source archive with no `.git` is a legitimate way to build. Say so
        # rather than inventing a commit.
        return {"available": False}
    status = git("status", "--porcelain")
    return {
        "available": True,
        "commit": commit,
        "dirty": bool(status),
        "remote": git("config", "--get", "remote.origin.url"),
        "tree": git("rev-parse", "HEAD^{tree}"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(prog="release_manifest")
    parser.add_argument("--out", required=True)
    parser.add_argument("--schema", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", default=None)
    parser.add_argument("--toolchain-channel", required=True)
    parser.add_argument("--rustc", required=True)
    parser.add_argument("--target-triple", required=True)
    parser.add_argument("--build-command", required=True)
    parser.add_argument("--cargo-auditable-version", required=True)
    parser.add_argument("--sbom-tool", required=True)
    parser.add_argument("--sbom-tool-version", required=True)
    parser.add_argument("--binary", required=True)
    parser.add_argument(
        "--asset",
        action="append",
        default=[],
        metavar="KIND=PATH",
        help="one released file; repeat per asset",
    )
    arguments = parser.parse_args()

    assets = []
    for entry in arguments.asset:
        kind, _, raw = entry.partition("=")
        if not kind or not raw:
            raise SystemExit(f"release_manifest: malformed --asset {entry!r}")
        path = pathlib.Path(raw)
        if not path.is_file():
            raise SystemExit(f"release_manifest: no such asset: {path}")
        assets.append(
            {
                "bytes": path.stat().st_size,
                "kind": kind,
                "name": path.name,
                "sha256": digest(path),
            }
        )
    assets.sort(key=lambda asset: (asset["kind"], asset["name"]))

    binary = pathlib.Path(arguments.binary)
    if not binary.is_file():
        raise SystemExit(f"release_manifest: no such binary: {binary}")

    architecture, _, _ = arguments.target_triple.partition("-")
    platform = "macos" if "apple-darwin" in arguments.target_triple else "linux"

    manifest = {
        "schema": arguments.schema,
        "assets": assets,
        "attestation": {
            "build_provenance": "actions/attest-build-provenance",
            "note": ATTESTATION_NOTE,
            "provider_neutral": False,
        },
        "binary": {
            "name": binary.name,
            "sha256": digest(binary),
        },
        "build": {
            "cargo_auditable_version": arguments.cargo_auditable_version,
            "command": arguments.build_command,
            "entry_point": "scripts/release.sh",
            "provider": "github-actions"
            if os.environ.get("GITHUB_ACTIONS") == "true"
            else "local",
            "sbom": {
                "format": "spdx-json",
                "tool": arguments.sbom_tool,
                "tool_version": arguments.sbom_tool_version,
            },
        },
        "generated_at": dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "release": {
            "tag": arguments.tag,
            "version": arguments.version,
        },
        "signature": {
            "namespace": "vela-release",
            "note": SIGNATURE_NOTE,
            "sidecar": pathlib.Path(arguments.out).name + ".sig",
        },
        "source": source_state(),
        "target": {
            "architecture": architecture,
            "platform": platform,
            "triple": arguments.target_triple,
        },
        "toolchain": {
            "channel": arguments.toolchain_channel,
            "rustc": arguments.rustc,
        },
    }

    out = pathlib.Path(arguments.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"release manifest: {out} ({len(assets)} assets)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
