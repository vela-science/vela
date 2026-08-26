#!/usr/bin/env python3
"""Prove deterministic archives and manifests from equivalent input trees."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARCHIVER = ROOT / ".github/release/create-deterministic-archive.py"
NOTICE_CHECKER = ROOT / ".github/release/check-notice-bundle.py"
NOTICE_GENERATOR = ROOT / ".github/release/generate-third-party-notices.py"
SMOKE = ROOT / ".github/release/smoke-bundle.sh"
SBOM_CANONICALIZER = ROOT / ".github/release/check-sbom.py"
MANIFEST = ROOT / "scripts/release_manifest.py"
EPOCH = 1_786_406_400
PROJECT_LICENSES = ("LICENSE", "LICENSE-APACHE", "LICENSE-MIT")
NOTICE_NAME = "THIRD-PARTY-LICENSES.txt"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


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


def expect_failure(command: list[str], expected: str, cwd: Path = ROOT) -> None:
    result = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if result.returncode == 0 or expected not in result.stderr:
        raise AssertionError(
            f"expected failure containing {expected!r}:\n{result.stdout}\n{result.stderr}"
        )


def notice_fixture() -> bytes:
    version = (ROOT / ".github/release/cargo-about-version").read_text().strip()
    return (
        "Vela Third-Party License and Notice Material\n"
        "Format: vela.third-party-notices.v1\n"
        f"Generator: cargo-about {version} (--frozen --fail)\n"
        f"Cargo.lock sha256: {sha256(ROOT / 'Cargo.lock')}\n"
        f"about.toml sha256: {sha256(ROOT / '.github/release/about.toml')}\n"
        f"deny.toml sha256: {sha256(ROOT / 'deny.toml')}\n"
        "Package count: 1\n"
        "License text count: 1\n"
        "LICENSE TEXTS (1)\n"
        "fixture license text\n"
        "ADDITIONAL PACKAGE NOTICES (0)\n"
    ).encode()


def stage(path: Path) -> Path:
    path.mkdir(parents=True)
    binary = path / "vela"
    binary.write_bytes(b"#!/bin/sh\necho vela-reproducibility-fixture\n")
    binary.chmod(0o755)
    for name in PROJECT_LICENSES:
        shutil.copyfile(ROOT / name, path / name)
    (path / NOTICE_NAME).write_bytes(notice_fixture())
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
                observed = [
                    (item.name, item.mtime, item.uid, item.gid) for item in members
                ]
                expected = [
                    (name, EPOCH, 0, 0)
                    for name in (*PROJECT_LICENSES, NOTICE_NAME, "vela")
                ]
                if observed != expected:
                    raise AssertionError("tar metadata is not deterministic")
        else:
            with zipfile.ZipFile(first) as archive:
                entries = archive.infolist()
                if [entry.filename for entry in entries] != [
                    *PROJECT_LICENSES,
                    NOTICE_NAME,
                    "vela",
                ]:
                    raise AssertionError("zip inventory is not deterministic")


def notice_generator_arguments(directory: Path, *, covered: bool = True) -> list[str]:
    source = "registry+https://github.com/rust-lang/crates.io-index"
    package_id = f"{source}#fixture-crate@1.2.3"
    package_root = directory / "registry/fixture-crate-1.2.3"
    package_root.mkdir(parents=True)
    (package_root / "Cargo.toml").write_text(
        '[package]\nname = "fixture-crate"\nversion = "1.2.3"\n',
        encoding="utf-8",
    )
    (package_root / "COPYRIGHT").write_text(
        "Copyright 2026 Fixture Authors\n", encoding="utf-8"
    )
    lock = directory / "Cargo.lock"
    lock.write_text(
        "version = 4\n\n"
        "[[package]]\n"
        'name = "fixture-crate"\n'
        'version = "1.2.3"\n'
        f'source = "{source}"\n'
        f'checksum = "{"a" * 64}"\n',
        encoding="utf-8",
    )
    config = directory / "about.toml"
    config.write_text(
        'accepted = ["MIT"]\n'
        'targets = ["aarch64-apple-darwin", "x86_64-unknown-linux-musl"]\n',
        encoding="utf-8",
    )
    deny = directory / "deny.toml"
    deny.write_text(
        '[licenses]\nallow = ["MIT"]\n',
        encoding="utf-8",
    )
    package = {
        "id": package_id,
        "license": "MIT",
        "manifest_path": str(package_root / "Cargo.toml"),
        "name": "fixture-crate",
        "repository": "https://example.invalid/fixture-crate",
        "source": source,
        "version": "1.2.3",
    }
    about = directory / "cargo-about.json"
    about.write_text(
        json.dumps(
            {
                "crates": [{"license": "MIT", "package": package}],
                "licenses": [
                    {
                        "id": "MIT",
                        "text": "Fixture MIT license text\n",
                        "used_by": [{"crate": package, "path": None}]
                        if covered
                        else [],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    return [
        sys.executable,
        str(NOTICE_GENERATOR),
        "--about-json",
        str(about),
        "--cargo-lock",
        str(lock),
        "--config",
        str(config),
        "--deny-config",
        str(deny),
        "--cargo-about-version",
        "0.8.4",
        "--output",
        str(directory / NOTICE_NAME),
    ]


def check_notice_generation(root: Path) -> None:
    directories = [root / "left-notices", root / "right-notices"]
    outputs = []
    for directory in directories:
        run(notice_generator_arguments(directory))
        outputs.append((directory / NOTICE_NAME).read_bytes())
    if outputs[0] != outputs[1]:
        raise AssertionError("third-party notices depend on package source path")
    payload = outputs[0].decode()
    for expected in (
        "Package count: 1",
        "License text count: 1",
        "Additional notice count: 1",
        "Copyright 2026 Fixture Authors",
    ):
        if expected not in payload:
            raise AssertionError(f"third-party notices omitted {expected!r}")

    uncovered = root / "uncovered-notices"
    expect_failure(
        notice_generator_arguments(uncovered, covered=False),
        "MIT applies to no crate",
    )
    absent = root / "unlocked-notices"
    command = notice_generator_arguments(absent)
    (absent / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    expect_failure(command, "Cargo.lock has no package list")


def check_notice_gates(root: Path) -> None:
    version = (ROOT / ".github/release/cargo-about-version").read_text().strip()
    valid = root / "valid-notice-stage"
    stage(valid)
    checker = [
        sys.executable,
        str(NOTICE_CHECKER),
        "--bundle",
        str(valid),
        "--source-root",
        str(ROOT),
        "--cargo-about-version",
        version,
    ]
    run(checker)
    archive = root / "valid-notices.tar.gz"
    run(
        [
            sys.executable,
            str(ARCHIVER),
            "--source",
            str(valid),
            "--output",
            str(archive),
            "--epoch",
            str(EPOCH),
        ]
    )
    run([str(SMOKE), "--notices-only", str(archive), "0.0.0"])

    for name in (*PROJECT_LICENSES, NOTICE_NAME):
        broken = root / f"missing-{name.lower()}"
        shutil.copytree(valid, broken)
        (broken / name).unlink()
        expected = f"missing required regular file {name}"
        broken_checker = checker.copy()
        broken_checker[broken_checker.index(str(valid))] = str(broken)
        expect_failure(broken_checker, expected)
        broken_archive = root / f"missing-{name.lower()}.tar.gz"
        run(
            [
                sys.executable,
                str(ARCHIVER),
                "--source",
                str(broken),
                "--output",
                str(broken_archive),
                "--epoch",
                str(EPOCH),
            ]
        )
        expect_failure(
            [str(SMOKE), "--notices-only", str(broken_archive), "0.0.0"],
            expected,
        )

    changed = root / "changed-project-license"
    shutil.copytree(valid, changed)
    (changed / "LICENSE-MIT").write_text("changed\n", encoding="utf-8")
    changed_checker = checker.copy()
    changed_checker[changed_checker.index(str(valid))] = str(changed)
    expect_failure(changed_checker, "packaged LICENSE-MIT differs from source")


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
    notices = directory / NOTICE_NAME
    notices.write_bytes(notice_fixture())
    license_inputs = []
    for name, content in (
        ("Cargo.lock", b"fixture lock\n"),
        ("about.toml", b"accepted = [\"MIT\"]\n"),
        ("cargo-about-version", b"0.8.4\n"),
        ("deny.toml", b'[licenses]\nallow = ["MIT"]\n'),
        ("normalizer", b"fixture normalizer\n"),
    ):
        path = directory / f"input-{name.replace('.', '-')}"
        path.write_bytes(content)
        license_inputs.extend(["--license-input", f"{name}={path}"])
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
        "--license-generator",
        "cargo-about",
        "--license-generator-version",
        "0.8.4",
        "--license-notices",
        str(notices),
        *license_inputs,
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
    if document["licenses"]["generator_version"] != "0.8.4":
        raise AssertionError("manifest omitted the pinned notice generator")
    if len(document["licenses"]["inputs"]) != 5:
        raise AssertionError("manifest omitted a notice-generation input")


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="vela-release-reproducibility-"
    ) as temporary:
        root = Path(temporary)
        check_archives(root)
        check_notice_generation(root)
        check_notice_gates(root)
        check_sbom_normalization(root)
        check_manifests(root)
    print(
        "release-reproducibility: deterministic tar.gz, zip, notices, SBOM, and manifest"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
