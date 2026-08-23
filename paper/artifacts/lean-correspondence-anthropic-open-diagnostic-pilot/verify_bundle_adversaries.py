"""Run fail-closed mutations against one fully materialized held bundle.

Run this after ``build_execution_bundles.py``.  It mutates only the generated
``/private/tmp`` bundle, restores every touched byte, and never releases a
permit or contacts a provider.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Callable


BUNDLE_ROOT = Path(
    "/private/tmp/vela-anthropic-open-diagnostic-held-v2/"
    "anthropic-diag-2f719af4c655ef77"
)


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def qualifier_command() -> list[str]:
    value = load(BUNDLE_ROOT / "qualification.json")
    command = value["self_verification"]["command"]
    if command[-1] != str(BUNDLE_ROOT):
        raise RuntimeError("held bundle self-verification target drift")
    return command


def run_qualifier() -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        qualifier_command(),
        check=False,
        capture_output=True,
        env={
            "PATH": str(Path(qualifier_command()[0]).parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONNOUSERSITE": "1",
        },
    )


def expect_blocked(name: str, mutate: Callable[[], Callable[[], None]]) -> None:
    restore = mutate()
    try:
        result = run_qualifier()
        if result.returncode == 0:
            raise AssertionError(f"adversary unexpectedly passed: {name}")
    finally:
        restore()


def byte_mutation(path: Path, replacement: bytes) -> Callable[[], Callable[[], None]]:
    def mutate() -> Callable[[], None]:
        original = path.read_bytes()
        path.write_bytes(replacement)
        return lambda: path.write_bytes(original)

    return mutate


def json_mutation(path: Path, field: str, replacement: object):
    def mutate() -> Callable[[], None]:
        original = path.read_bytes()
        value = load(path)
        value[field] = replacement
        write_json(path, value)
        return lambda: path.write_bytes(original)

    return mutate


def missing_file(path: Path):
    def mutate() -> Callable[[], None]:
        original = path.read_bytes()
        mode = path.stat().st_mode & 0o777
        path.unlink()

        def restore() -> None:
            path.write_bytes(original)
            path.chmod(mode)

        return restore

    return mutate


def symlink_file(path: Path):
    def mutate() -> Callable[[], None]:
        original = path.read_bytes()
        mode = path.stat().st_mode & 0o777
        with tempfile.NamedTemporaryFile(delete=False) as handle:
            handle.write(original)
            external = Path(handle.name)
        path.unlink()
        path.symlink_to(external)

        def restore() -> None:
            path.unlink()
            path.write_bytes(original)
            path.chmod(mode)
            external.unlink()

        return restore

    return mutate


def external_hardlink(path: Path):
    def mutate() -> Callable[[], None]:
        directory = Path(tempfile.mkdtemp(prefix="vela-evidence-hardlink-"))
        alias = directory / "alias"
        os.link(path, alias)

        def restore() -> None:
            alias.unlink()
            directory.rmdir()

        return restore

    return mutate


def main() -> int:
    if not BUNDLE_ROOT.is_dir():
        raise RuntimeError("run build_execution_bundles.py first")
    valid = run_qualifier()
    if valid.returncode != 0:
        raise RuntimeError(valid.stderr.decode(errors="replace"))

    manifest = load(BUNDLE_ROOT / "workspace/assignment-manifest.json")
    evidence_path = (
        BUNDLE_ROOT
        / "workspace"
        / manifest["bindings"][0]["mounted_path"].removeprefix("/workspace/")
    )
    boundary_path = BUNDLE_ROOT / "config/tool-boundary.json"
    preflight_path = (
        BUNDLE_ROOT / "execution/offline-evidence/workspace-bridge-preflight.json"
    )
    stdout_path = BUNDLE_ROOT / "fixture/evidence/tool.stdout"

    expect_blocked("missing evidence", missing_file(evidence_path))
    expect_blocked("substituted evidence", byte_mutation(evidence_path, b"drift\n"))
    expect_blocked("symlink evidence", symlink_file(evidence_path))
    expect_blocked("external hardlink evidence", external_hardlink(evidence_path))
    expect_blocked(
        "unmounted evidence",
        json_mutation(boundary_path, "mounts", []),
    )
    expect_blocked(
        "stale preflight receipt",
        json_mutation(preflight_path, "status", "stale"),
    )
    expect_blocked(
        "forged tool result",
        byte_mutation(stdout_path, b"forged tool result\n"),
    )

    final = run_qualifier()
    if final.returncode != 0:
        raise RuntimeError("bundle restoration failed")
    print("PASS 7 fail-closed held-bundle adversaries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
