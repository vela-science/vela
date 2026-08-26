#!/usr/bin/env python3
"""Normalize cargo-about output into Vela's distributable notice bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

import tomllib

FORMAT = "vela.third-party-notices.v1"
NOTICE_FILE = re.compile(
    r"^(?:NOTICE|COPYRIGHT|AUTHORS)(?:[._-].*)?$", re.IGNORECASE
)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def clean_text(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise SystemExit(f"third-party notices: missing {label}")
    return value.replace("\r\n", "\n").replace("\r", "\n").strip("\n")


def locked_packages(path: Path) -> dict[tuple[str, str, str], dict[str, object]]:
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    packages = document.get("package")
    if not isinstance(packages, list):
        raise SystemExit(f"third-party notices: {path} has no package list")
    result = {}
    for package in packages:
        if not isinstance(package, dict):
            raise SystemExit(f"third-party notices: malformed package in {path}")
        name = package.get("name")
        version = package.get("version")
        source = package.get("source")
        if all(isinstance(value, str) for value in (name, version, source)):
            result[(name, version, source)] = package
    return result


def render(arguments: argparse.Namespace) -> bytes:
    about_path = Path(arguments.about_json)
    lock_path = Path(arguments.cargo_lock)
    config_path = Path(arguments.config)
    deny_path = Path(arguments.deny_config)
    document = json.loads(about_path.read_text(encoding="utf-8"))
    raw_crates = document.get("crates")
    raw_licenses = document.get("licenses")
    if not isinstance(raw_crates, list) or not raw_crates:
        raise SystemExit("third-party notices: cargo-about returned no crates")
    if not isinstance(raw_licenses, list) or not raw_licenses:
        raise SystemExit("third-party notices: cargo-about returned no license texts")

    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    accepted = config.get("accepted")
    targets = config.get("targets")
    if not isinstance(accepted, list) or not all(isinstance(x, str) for x in accepted):
        raise SystemExit("third-party notices: about config has no accepted licenses")
    if not isinstance(targets, list) or not all(isinstance(x, str) for x in targets):
        raise SystemExit("third-party notices: about config has no release targets")
    deny = tomllib.loads(deny_path.read_text(encoding="utf-8"))
    denied_license_config = deny.get("licenses")
    denied_allow = (
        denied_license_config.get("allow")
        if isinstance(denied_license_config, dict)
        else None
    )
    if accepted != denied_allow:
        raise SystemExit(
            "third-party notices: about.toml accepted licenses differ from deny.toml"
        )

    locked = locked_packages(lock_path)
    packages: dict[str, dict[str, object]] = {}
    package_rows = []
    notices = []
    for item in raw_crates:
        if not isinstance(item, dict) or not isinstance(item.get("package"), dict):
            raise SystemExit("third-party notices: malformed cargo-about crate")
        package = item["package"]
        name = clean_text(package.get("name"), "crate name")
        version = clean_text(package.get("version"), f"version for {name}")
        source = clean_text(package.get("source"), f"source for {name} {version}")
        package_id = clean_text(package.get("id"), f"id for {name} {version}")
        declared = clean_text(
            package.get("license"), f"declared license for {name} {version}"
        )
        evaluated = clean_text(
            item.get("license"), f"evaluated license for {name} {version}"
        )
        lock_entry = locked.get((name, version, source))
        if lock_entry is None:
            raise SystemExit(
                f"third-party notices: {name} {version} is absent from Cargo.lock"
            )
        checksum = clean_text(
            lock_entry.get("checksum"), f"lock checksum for {name} {version}"
        )
        if not re.fullmatch(r"[0-9a-f]{64}", checksum):
            raise SystemExit(
                f"third-party notices: invalid lock checksum for {name} {version}"
            )
        if package_id in packages:
            raise SystemExit(f"third-party notices: duplicate package {package_id}")
        repository = package.get("repository")
        if repository is not None and not isinstance(repository, str):
            raise SystemExit(
                f"third-party notices: invalid repository for {name} {version}"
            )
        manifest_path = Path(
            clean_text(
                package.get("manifest_path"), f"manifest path for {name} {version}"
            )
        )
        if not manifest_path.is_file():
            raise SystemExit(
                f"third-party notices: missing package manifest for {name} {version}"
            )
        packages[package_id] = {
            "name": name,
            "version": version,
        }
        package_rows.append(
            {
                "checksum": checksum,
                "declared": declared,
                "evaluated": evaluated,
                "name": name,
                "repository": repository or "not declared",
                "source": source,
                "version": version,
            }
        )
        for path in sorted(manifest_path.parent.iterdir(), key=lambda value: value.name):
            if not path.is_file() or not NOTICE_FILE.fullmatch(path.name):
                continue
            payload = path.read_bytes()
            try:
                text = payload.decode("utf-8")
            except UnicodeDecodeError as error:
                raise SystemExit(
                    f"third-party notices: {name} {version}/{path.name} is not UTF-8"
                ) from error
            notices.append(
                {
                    "filename": path.name,
                    "name": name,
                    "sha256": sha256_bytes(payload),
                    "text": clean_text(text, f"{name} {version}/{path.name}"),
                    "version": version,
                }
            )

    covered: set[str] = set()
    grouped: dict[tuple[str, str], set[tuple[str, str]]] = {}
    for license_entry in raw_licenses:
        if not isinstance(license_entry, dict):
            raise SystemExit("third-party notices: malformed license entry")
        license_id = clean_text(license_entry.get("id"), "license identifier")
        if license_id not in accepted:
            raise SystemExit(
                f"third-party notices: cargo-about selected unaccepted {license_id}"
            )
        text = clean_text(license_entry.get("text"), f"text for {license_id}")
        used_by = license_entry.get("used_by")
        if not isinstance(used_by, list) or not used_by:
            raise SystemExit(f"third-party notices: {license_id} applies to no crate")
        users = grouped.setdefault((license_id, text), set())
        for use in used_by:
            crate = use.get("crate") if isinstance(use, dict) else None
            package_id = crate.get("id") if isinstance(crate, dict) else None
            if package_id not in packages:
                raise SystemExit(
                    f"third-party notices: {license_id} names an unknown crate"
                )
            covered.add(package_id)
            package = packages[package_id]
            users.add((str(package["name"]), str(package["version"])))
    missing = sorted(packages.keys() - covered)
    if missing:
        raise SystemExit(
            "third-party notices: crates without license text: " + ", ".join(missing)
        )

    package_rows.sort(key=lambda value: (value["name"], value["version"], value["source"]))
    graph_payload = json.dumps(
        package_rows, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()
    license_rows = sorted(
        (
            {
                "id": license_id,
                "sha256": sha256_bytes(text.encode()),
                "text": text,
                "users": sorted(users),
            }
            for (license_id, text), users in grouped.items()
        ),
        key=lambda value: (value["id"], value["sha256"]),
    )
    notices.sort(key=lambda value: (value["name"], value["version"], value["filename"]))

    lines = [
        "Vela Third-Party License and Notice Material",
        f"Format: {FORMAT}",
        f"Generator: cargo-about {arguments.cargo_about_version} (--frozen --fail)",
        "Normalizer: .github/release/generate-third-party-notices.py",
        f"Cargo.lock sha256: {sha256_file(lock_path)}",
        f"about.toml sha256: {sha256_file(config_path)}",
        f"deny.toml sha256: {sha256_file(deny_path)}",
        f"Dependency graph sha256: {sha256_bytes(graph_payload)}",
        f"Targets: {', '.join(sorted(targets))}",
        f"Package count: {len(package_rows)}",
        f"License text count: {len(license_rows)}",
        f"Additional notice count: {len(notices)}",
        "",
        "This material is generated from the exact locked normal-dependency union for",
        "Vela's two supported release targets under the repository's accepted-license",
        "policy. It is distributable notice material, not a legal conclusion.",
        "",
        f"PACKAGE INVENTORY ({len(package_rows)})",
        "=" * 80,
    ]
    for package in package_rows:
        lines.extend(
            [
                f"{package['name']} {package['version']}",
                f"  source: {package['source']}",
                f"  Cargo.lock checksum: sha256:{package['checksum']}",
                f"  declared license: {package['declared']}",
                f"  evaluated license: {package['evaluated']}",
                f"  repository: {package['repository']}",
            ]
        )

    lines.extend(["", f"LICENSE TEXTS ({len(license_rows)})", "=" * 80])
    for license_row in license_rows:
        lines.extend(
            [
                "",
                f"License: {license_row['id']}",
                f"Text sha256: {license_row['sha256']}",
                "Applies to:",
                *(f"  - {name} {version}" for name, version in license_row["users"]),
                "-" * 80,
                str(license_row["text"]),
            ]
        )

    lines.extend(["", f"ADDITIONAL PACKAGE NOTICES ({len(notices)})", "=" * 80])
    if not notices:
        lines.append("No package-root NOTICE, COPYRIGHT, or AUTHORS files were present.")
    for notice in notices:
        lines.extend(
            [
                "",
                f"Package: {notice['name']} {notice['version']}",
                f"File: {notice['filename']}",
                f"Text sha256: {notice['sha256']}",
                "-" * 80,
                str(notice["text"]),
            ]
        )
    return ("\n".join(lines).rstrip() + "\n").encode()


def main() -> int:
    parser = argparse.ArgumentParser(prog="generate-third-party-notices")
    parser.add_argument("--about-json", required=True)
    parser.add_argument("--cargo-lock", required=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--deny-config", required=True)
    parser.add_argument("--cargo-about-version", required=True)
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()
    output = Path(arguments.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(render(arguments))
    print(f"third-party notices: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
