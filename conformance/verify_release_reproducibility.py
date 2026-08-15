#!/usr/bin/env python3
"""Prove deterministic archives and manifests from equivalent input trees."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARCHIVER = ROOT / ".github/release/create-deterministic-archive.py"
SBOM_CANONICALIZER = ROOT / ".github/release/check-sbom.py"
MANIFEST = ROOT / "scripts/release_manifest.py"
EPOCH = 1_786_406_400


def run(command: list[str], cwd: Path = ROOT) -> None:
    result = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(result.stderr)


def stage(path: Path) -> Path:
    path.mkdir(parents=True)
    binary = path / "vela"
    binary.write_bytes(b"#!/bin/sh\necho vela-reproducibility-fixture\n")
    binary.chmod(0o755)
    os.utime(binary, (EPOCH + len(str(path)), EPOCH + len(str(path))))
    return binary


def check_archives(root: Path) -> None:
    left, right = root / "left-stage", root / "right-stage"
    stage(left)
    stage(right)
    for suffix in ("tar.gz", "zip"):
        first, second = root / f"first.{suffix}", root / f"second.{suffix}"
        for source, output in ((left, first), (right, second)):
            run(
                [
                    sys.executable,
                    str(ARCHIVER),
                    "--source",
                    str(source),
                    "--output",
                    str(output),
                    "--epoch",
                    str(EPOCH),
                ]
            )
        if first.read_bytes() != second.read_bytes():
            raise AssertionError(f"{suffix} archive depends on source path or mtime")

        if suffix == "tar.gz":
            with tarfile.open(first, "r:gz") as archive:
                members = archive.getmembers()
                if [
                    (item.name, item.mtime, item.uid, item.gid) for item in members
                ] != [("vela", EPOCH, 0, 0)]:
                    raise AssertionError("tar metadata is not deterministic")
        else:
            with zipfile.ZipFile(first) as archive:
                entries = archive.infolist()
                if len(entries) != 1 or entries[0].filename != "vela":
                    raise AssertionError("zip inventory is not deterministic")


def sbom_fixture(path: Path, stage: Path, created: str, nonce: str) -> None:
    root_id = "SPDXRef-DocumentRoot-Directory-" + str(stage).replace("/", "-")
    document = {
        "SPDXID": "SPDXRef-DOCUMENT",
        "creationInfo": {"created": created, "creators": ["Tool: syft-1.50.0"]},
        "dataLicense": "CC0-1.0",
        "documentNamespace": f"https://anchore.example/{stage}-{nonce}",
        "name": str(stage),
        "packages": [
            {"SPDXID": root_id, "name": str(stage)},
            {"SPDXID": "SPDXRef-Package-vela-cli", "name": "vela-cli"},
        ],
        "relationships": [
            {
                "relatedSpdxElement": "SPDXRef-Package-vela-cli",
                "relationshipType": "CONTAINS",
                "spdxElementId": root_id,
            },
            {
                "relatedSpdxElement": root_id,
                "relationshipType": "DESCRIBES",
                "spdxElementId": "SPDXRef-DOCUMENT",
            },
        ],
        "spdxVersion": "SPDX-2.3",
    }
    path.write_text(json.dumps(document), encoding="utf-8")


def check_sbom_normalization(root: Path) -> None:
    raw = [root / "left.raw.spdx.json", root / "right.raw.spdx.json"]
    outputs = [root / "left.spdx.json", root / "right.spdx.json"]
    stages = [root / "private-left/stage", root / "private-right/stage"]
    sbom_fixture(raw[0], stages[0], "2026-08-14T12:00:01Z", "random-one")
    sbom_fixture(raw[1], stages[1], "2026-08-14T12:00:02Z", "random-two")
    common = [
        "--name",
        "Vela 0.0.0 x86_64-unknown-linux-musl release bundle",
        "--namespace",
        "https://vela.science/spdx/vela/0.0.0/x86_64-unknown-linux-musl",
        "--created",
        "2026-08-11T00:00:00Z",
        "--root-name",
        "vela-0.0.0-x86_64-unknown-linux-musl",
        "--root-id",
        "SPDXRef-DocumentRoot-Vela-0-0-0-x86_64-unknown-linux-musl",
    ]
    for source, output in zip(raw, outputs, strict=True):
        run(
            [
                sys.executable,
                str(SBOM_CANONICALIZER),
                "--canonicalize",
                "--input",
                str(source),
                "--output",
                str(output),
                *common,
            ]
        )
    if outputs[0].read_bytes() != outputs[1].read_bytes():
        raise AssertionError(
            "canonical SBOM depends on source path, wall clock, or UUID"
        )
    payload = outputs[0].read_text(encoding="utf-8")
    for private in (*map(str, stages), "random-one", "random-two"):
        if private in payload:
            raise AssertionError(f"canonical SBOM retains ambient input {private}")
    document = json.loads(payload)
    if document["creationInfo"]["created"] != "2026-08-11T00:00:00Z":
        raise AssertionError(
            "canonical SBOM timestamp does not come from SOURCE_DATE_EPOCH"
        )


def manifest_arguments(directory: Path, binary: Path) -> list[str]:
    archive = directory / "vela-linux-x86_64.tar.gz"
    archive.write_bytes(b"archive\n")
    sbom = directory / "vela-linux-x86_64.tar.gz.spdx.json"
    sbom.write_bytes(b'{"spdxVersion":"SPDX-2.3"}\n')
    return [
        sys.executable,
        str(MANIFEST),
        "--out",
        str(directory / "release-manifest.json"),
        "--schema",
        "vela.release-bundle-manifest.v1",
        "--version",
        "0.0.0",
        "--tag",
        "v0.0.0",
        "--toolchain-channel",
        "1.97.1",
        "--rustc",
        "rustc 1.97.1",
        "--target-triple",
        "x86_64-unknown-linux-musl",
        "--build-command",
        "cargo auditable build --locked --release -p vela-cli --bin vela --target x86_64-unknown-linux-musl",
        "--source-date-epoch",
        str(EPOCH),
        "--binary-build-count",
        "2",
        "--archive-build-count",
        "2",
        "--cargo-auditable-version",
        "0.7.5",
        "--sbom-tool",
        "syft",
        "--sbom-tool-version",
        "1.50.0",
        "--binary",
        str(binary),
        "--asset",
        f"archive={archive}",
        "--asset",
        f"sbom={sbom}",
    ]


def check_manifests(root: Path) -> None:
    directories = [root / "left-manifest", root / "right-manifest"]
    manifests = []
    for directory in directories:
        binary = stage(directory / "stage")
        directory.mkdir(exist_ok=True)
        run(manifest_arguments(directory, binary))
        manifests.append((directory / "release-manifest.json").read_bytes())
    if manifests[0] != manifests[1]:
        raise AssertionError("release manifest depends on build path or wall clock")
    document = json.loads(manifests[0])
    if document["generated_at"] != "2026-08-11T00:00:00Z":
        raise AssertionError("manifest timestamp does not come from SOURCE_DATE_EPOCH")
    if document["build"]["reproducibility"]["binary_builds_compared"] != 2:
        raise AssertionError("manifest omitted the two-build gate")


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="vela-release-reproducibility-"
    ) as temporary:
        root = Path(temporary)
        check_archives(root)
        check_sbom_normalization(root)
        check_manifests(root)
    print("release-reproducibility: deterministic tar.gz, zip, SBOM, and manifest")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
