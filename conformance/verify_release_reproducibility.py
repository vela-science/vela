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
SELECTED_GENERATOR = ROOT / ".github/release/selected-release-packages.py"
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


def notice_fixture(target: str = "x86_64-unknown-linux-musl") -> bytes:
    version = (ROOT / ".github/release/cargo-about-version").read_text().strip()
    return (
        "Vela Third-Party License and Notice Material\n"
        "Format: vela.third-party-notices.v1\n"
        f"Generator: cargo-about {version} (--frozen --fail)\n"
        f"Cargo.lock sha256: {sha256(ROOT / 'Cargo.lock')}\n"
        f"about.toml sha256: {sha256(ROOT / '.github/release/about.toml')}\n"
        f"deny.toml sha256: {sha256(ROOT / 'deny.toml')}\n"
        f"Target: {target}\n"
        "Package count: 1\n"
        "License text count: 1\n"
        "LICENSE TEXTS (1)\n"
        "fixture license text\n"
        "ADDITIONAL PACKAGE NOTICES (0)\n"
    ).encode()


def stage(path: Path, target: str = "x86_64-unknown-linux-musl") -> Path:
    path.mkdir(parents=True)
    binary = path / "vela"
    binary.write_bytes(b"#!/bin/sh\necho vela-reproducibility-fixture\n")
    binary.chmod(0o755)
    for name in PROJECT_LICENSES:
        shutil.copyfile(ROOT / name, path / name)
    (path / NOTICE_NAME).write_bytes(notice_fixture(target))
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


def check_selected_release_graph(root: Path) -> None:
    observed = {}
    forbidden = {
        "base16ct",
        "const-oid",
        "crypto-bigint",
        "der",
        "ecdsa",
        "elliptic-curve",
        "equivalent",
        "ff",
        "group",
        "hashbrown",
        "hmac",
        "indexmap",
        "lazy_static",
        "libm",
        "num-bigint-dig",
        "num-integer",
        "num-iter",
        "p256",
        "p384",
        "p521",
        "pkcs1",
        "pkcs8",
        "ppv-lite86",
        "primeorder",
        "rand",
        "rand_chacha",
        "rfc6979",
        "rsa",
        "sec1",
        "spin",
        "spki",
        "zerocopy",
    }
    for target in ("aarch64-apple-darwin", "x86_64-unknown-linux-musl"):
        output = root / f"selected-{target}.json"
        run(
            [
                sys.executable,
                str(SELECTED_GENERATOR),
                "--cargo-lock",
                str(ROOT / "Cargo.lock"),
                "--target",
                target,
                "--output",
                str(output),
            ]
        )
        document = json.loads(output.read_text(encoding="utf-8"))
        packages = document["packages"]
        third_party = [row for row in packages if not row["workspace"]]
        if len(third_party) != 85:
            raise AssertionError(f"{target} selected {len(third_party)} third parties")
        names = {row["name"] for row in third_party}
        if names & forbidden:
            raise AssertionError(f"{target} selected disabled packages {names & forbidden}")
        if not any(row["classification"] == "build-contributor" for row in third_party):
            raise AssertionError(f"{target} lost build-contributor classification")
        observed[target] = names
    if "core-foundation-sys" not in observed["aarch64-apple-darwin"]:
        raise AssertionError("macOS selected graph omitted core-foundation-sys")
    if "linux-raw-sys" not in observed["x86_64-unknown-linux-musl"]:
        raise AssertionError("Linux selected graph omitted linux-raw-sys")


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
    selected = directory / "selected.json"
    selected.write_text(
        json.dumps(
            {
                "format": "vela.release-selected-packages.v1",
                "cargo_lock_sha256": sha256(lock),
                "target": "x86_64-unknown-linux-musl",
                "packages": [
                    {
                        "checksum": "a" * 64,
                        "classification": "contained",
                        "name": "fixture-crate",
                        "source": source,
                        "version": "1.2.3",
                        "workspace": False,
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
        "--selected-graph",
        str(selected),
        "--cargo-about-version",
        "0.8.4",
        "--output",
        str(directory / NOTICE_NAME),
        "--inventory-output",
        str(directory / "notice-inventory.json"),
    ]


def check_notice_generation(root: Path) -> None:
    directories = [root / "left-notices", root / "right-notices"]
    outputs = []
    for directory in directories:
        run(notice_generator_arguments(directory))
        outputs.append(
            (
                (directory / NOTICE_NAME).read_bytes(),
                (directory / "notice-inventory.json").read_bytes(),
            )
        )
    if outputs[0] != outputs[1]:
        raise AssertionError("third-party notices depend on package source path")
    payload = outputs[0][0].decode()
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
    expect_failure(command, "invalid selected release graph")


def check_notice_gates(root: Path) -> None:
    version = (ROOT / ".github/release/cargo-about-version").read_text().strip()
    formats = (
        ("x86_64-unknown-linux-musl", "vela-linux-x86_64.tar.gz"),
        ("aarch64-apple-darwin", "vela-macos-aarch64.zip"),
    )
    for target, archive_name in formats:
        label = "linux" if target.startswith("x86_64") else "macos"
        valid = root / f"valid-{label}-notice-stage"
        stage(valid, target)
        checker = [
            sys.executable,
            str(NOTICE_CHECKER),
            "--bundle",
            str(valid),
            "--source-root",
            str(ROOT),
            "--cargo-about-version",
            version,
            "--target",
            target,
        ]
        run(checker)
        archive = root / archive_name
        run(
            [
                sys.executable,
                str(ARCHIVER),
                "--source", str(valid),
                "--output",
                str(archive),
                "--epoch",
                str(EPOCH),
            ]
        )
        run([str(SMOKE), "--notices-only", str(archive), "0.0.0"])

        cases = [(f"missing-{name.lower()}", name) for name in (*PROJECT_LICENSES, NOTICE_NAME)]
        cases.append(("altered-license-mit", None))
        for case_name, missing_name in cases:
            broken = root / f"{label}-{case_name}"
            shutil.copytree(valid, broken)
            if missing_name is None:
                (broken / "LICENSE-MIT").write_text("changed\n", encoding="utf-8")
                expected = "packaged LICENSE-MIT differs from source"
            else:
                (broken / missing_name).unlink()
                expected = f"missing required regular file {missing_name}"
            broken_checker = checker.copy()
            broken_checker[broken_checker.index(str(valid))] = str(broken)
            expect_failure(broken_checker, expected)
            broken_archive = root / f"{case_name}-{archive_name}"
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


def correspondence_inputs(root: Path) -> tuple[Path, Path]:
    selected = root / "selected-packages.json"
    notices = root / "notice-inventory.json"
    registry = "registry+https://github.com/rust-lang/crates.io-index"
    packages = [
        {
            "classification": "contained",
            "name": name,
            "version": "0.0.0",
            "workspace": True,
        }
        for name in sorted(
            ("vela-authority", "vela-cli", "vela-protocol", "vela-repository")
        )
    ]
    packages.extend(
        [
            {
                "checksum": "a" * 64,
                "classification": "contained",
                "name": "selected-runtime",
                "source": registry,
                "version": "1.2.3",
                "workspace": False,
            },
            {
                "checksum": "b" * 64,
                "classification": "build-contributor",
                "name": "selected-derive",
                "source": registry,
                "version": "2.0.0",
                "workspace": False,
            },
        ]
    )
    selected.write_text(
        json.dumps(
            {
                "format": "vela.release-selected-packages.v1",
                "cargo_lock_sha256": "c" * 64,
                "target": "x86_64-unknown-linux-musl",
                "packages": packages,
            }
        ),
        encoding="utf-8",
    )
    notices.write_text(
        json.dumps(
            {
                "format": "vela.third-party-notice-inventory.v1",
                "target": "x86_64-unknown-linux-musl",
                "packages": [
                    {
                        "classification": row["classification"],
                        "declared": "MIT",
                        "evaluated": "MIT",
                        "name": row["name"],
                        "source": row["source"],
                        "version": row["version"],
                    }
                    for row in packages
                    if not row["workspace"]
                ],
            }
        ),
        encoding="utf-8",
    )
    return selected, notices


def spdx_package(name: str, version: str) -> dict[str, object]:
    return {
        "SPDXID": f"SPDXRef-Package-{name}",
        "copyrightText": "NOASSERTION",
        "downloadLocation": "NOASSERTION",
        "filesAnalyzed": False,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": "NOASSERTION",
        "name": name,
        "versionInfo": version,
    }


def sbom_fixture(path: Path, stage: Path, created: str, nonce: str) -> None:
    root_id = "SPDXRef-DocumentRoot-Directory-" + str(stage).replace("/", "-")
    selected = [
        spdx_package(name, "0.0.0")
        for name in ("vela-authority", "vela-cli", "vela-protocol", "vela-repository")
    ]
    selected.append(spdx_package("selected-runtime", "1.2.3"))
    disabled = spdx_package("rsa", "0.9.10")
    document = {
        "SPDXID": "SPDXRef-DOCUMENT",
        "creationInfo": {"created": created, "creators": ["Tool: syft-1.50.0"]},
        "dataLicense": "CC0-1.0",
        "documentNamespace": f"https://anchore.example/{stage}-{nonce}",
        "name": str(stage),
        "packages": [{"SPDXID": root_id, "name": str(stage)}, *selected, disabled],
        "relationships": [
            *(
                {
                    "relatedSpdxElement": package["SPDXID"],
                    "relationshipType": "CONTAINS",
                    "spdxElementId": root_id,
                }
                for package in [*selected, disabled]
            ),
            {
                "relatedSpdxElement": root_id,
                "relationshipType": "DESCRIBES",
                "spdxElementId": "SPDXRef-DOCUMENT",
            },
            {
                "relatedSpdxElement": "SPDXRef-Package-selected-runtime",
                "relationshipType": "DEPENDS_ON",
                "spdxElementId": "SPDXRef-Package-rsa",
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
    selected, notices = correspondence_inputs(root)
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
        "--selected-graph",
        str(selected),
        "--notice-inventory",
        str(notices),
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
    names = {package["name"] for package in document["packages"]}
    if "rsa" in names or "selected-derive" not in names:
        raise AssertionError("canonical SBOM did not filter and complete the selected graph")
    if "SPDXRef-Package-rsa" in payload:
        raise AssertionError("canonical SBOM retains a disabled-package relationship")
    run(
        [
            sys.executable,
            str(SBOM_CANONICALIZER),
            str(outputs[0]),
            "--selected-graph",
            str(selected),
            "--notice-inventory",
            str(notices),
        ]
    )

    missing_notice = root / "missing-notice.json"
    notice_document = json.loads(notices.read_text(encoding="utf-8"))
    notice_document["packages"].pop()
    missing_notice.write_text(json.dumps(notice_document), encoding="utf-8")
    expect_failure(
        [
            sys.executable,
            str(SBOM_CANONICALIZER),
            str(outputs[0]),
            "--selected-graph",
            str(selected),
            "--notice-inventory",
            str(missing_notice),
        ],
        "notices differ from selected third-party graph",
    )

    extra_notice = root / "extra-notice.json"
    notice_document = json.loads(notices.read_text(encoding="utf-8"))
    notice_document["packages"].append(
        {
            "classification": "contained",
            "declared": "MIT",
            "evaluated": "MIT",
            "name": "rsa",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "version": "0.9.10",
        }
    )
    extra_notice.write_text(json.dumps(notice_document), encoding="utf-8")
    expect_failure(
        [
            sys.executable,
            str(SBOM_CANONICALIZER),
            str(outputs[0]),
            "--selected-graph",
            str(selected),
            "--notice-inventory",
            str(extra_notice),
        ],
        "notices differ from selected third-party graph",
    )

    missing_runtime = root / "missing-runtime.raw.spdx.json"
    missing_document = json.loads(raw[0].read_text(encoding="utf-8"))
    missing_document["packages"] = [
        package
        for package in missing_document["packages"]
        if package.get("name") != "selected-runtime"
    ]
    missing_runtime.write_text(json.dumps(missing_document), encoding="utf-8")
    command = [
        sys.executable,
        str(SBOM_CANONICALIZER),
        "--canonicalize",
        "--input",
        str(missing_runtime),
        "--output",
        str(root / "missing-runtime.spdx.json"),
        *common,
    ]
    expect_failure(command, "Syft omitted contained selected package")

    extra_spdx = root / "extra.spdx.json"
    extra_document = json.loads(outputs[0].read_text(encoding="utf-8"))
    extra_document["packages"].append(spdx_package("rsa", "0.9.10"))
    extra_spdx.write_text(json.dumps(extra_document), encoding="utf-8")
    expect_failure(
        [
            sys.executable,
            str(SBOM_CANONICALIZER),
            str(extra_spdx),
            "--selected-graph",
            str(selected),
            "--notice-inventory",
            str(notices),
        ],
        "SPDX differs from selected release graph",
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
        ("selected-graph-generator", b"fixture graph generator\n"),
        ("selected-release-graph", b"fixture selected graph\n"),
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
    if len(document["licenses"]["inputs"]) != 7:
        raise AssertionError("manifest omitted a notice-generation input")


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="vela-release-reproducibility-"
    ) as temporary:
        root = Path(temporary)
        check_archives(root)
        check_selected_release_graph(root)
        check_notice_generation(root)
        check_notice_gates(root)
        check_sbom_normalization(root)
        check_manifests(root)
    print(
        "release-reproducibility: deterministic tar.gz, zip, target notices, "
        "exact SBOM correspondence, ten archive negatives, and manifest"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
