#!/usr/bin/env python3
"""Hold the released Protocol 1 surface to exact published bytes."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFORMANCE = ROOT / "conformance"
MANIFEST = CONFORMANCE / "protocol-1.json"
sys.path.insert(0, str(CONFORMANCE / "readers/python"))
from canonical import canonical_bytes


def paths() -> list[tuple[str, str, bool]]:
    selected: list[tuple[str, str, bool]] = [
        ("docs/PROTOCOL.md", "specification", True),
        ("docs/interop/scientific-state-profile.md", "conformance-profile", True),
        ("conformance/canonical-hashing.json", "canonical-vector", True),
        ("conformance/jcs-shadow-audit.json", "canonical-negative-vector", True),
        ("conformance/fixtures/claim-relation-vocabulary.json", "vocabulary-vector", True),
        (
            "conformance/fixtures/read-surfaces/decision-inbox-v3.json",
            "cli-read-contract-vector",
            False,
        ),
        (
            "conformance/fixtures/read-surfaces/README.md",
            "cli-read-contract-documentation",
            False,
        ),
        ("conformance/verify.py", "conformance-entrypoint", True),
        ("conformance/check-core.sh", "core-certification-entrypoint", False),
        ("conformance/verify_protocol_1.py", "manifest-verifier", True),
        ("conformance/verify_canonical_hashing.py", "vector-verifier", True),
        ("conformance/verify_current_objects.py", "object-interoperability-verifier", True),
        ("conformance/verify_wire_schemas.py", "schema-verifier", True),
        ("conformance/verify_authority_chain.py", "authority-vector-verifier", True),
        ("conformance/verify_correction_impact.py", "correction-vector-verifier", True),
        ("conformance/verify_reference_flows.py", "reference-flow-verifier", False),
        ("conformance/verify_release_reproducibility.py", "release-reproducibility-verifier", False),
        (
            "conformance/check-release-notice-fresh-cache.sh",
            "release-notice-fresh-cache-verifier",
            False,
        ),
        ("conformance/test_release_install.py", "release-installation-verifier", False),
        ("docs/RELEASES.md", "release-qualification", False),
        ("LICENSE", "license-boundary", False),
        ("CITATION.cff", "citation-metadata", False),
        (".github/release/create-deterministic-archive.py", "release-archiver", False),
        (".github/release/about.toml", "release-notice-policy", False),
        (
            ".github/release/cargo-about-version",
            "release-notice-generator-pin",
            False,
        ),
        (
            ".github/release/check-notice-bundle.py",
            "release-notice-gate",
            False,
        ),
        (
            ".github/release/generate-third-party-notices.py",
            "release-notice-generator",
            False,
        ),
        (
            ".github/release/selected-release-packages.py",
            "release-selected-graph-generator",
            False,
        ),
        (".github/release/check-sbom.py", "release-sbom-gate", False),
        (".github/release/smoke-bundle.sh", "release-smoke-gate", False),
        (".github/workflows/release.yml", "hosted-release-gate", False),
        ("allowed_signers", "release-trust-root", False),
        ("install.sh", "release-installer", False),
        ("scripts/release.sh", "release-entrypoint", False),
        ("scripts/release_manifest.py", "release-manifest-generator", False),
        ("scripts/sign-published-release.sh", "release-signing-gate", False),
    ]

    for path in sorted((ROOT / "schemas").glob("*.schema.json")):
        if path.name == "repository-projection.schema.json":
            # This is a stable software read contract, not a new Protocol 1
            # object or authority-bearing wire format.
            selected.append(
                (str(path.relative_to(ROOT)), "software-read-schema", False)
            )
        else:
            selected.append((str(path.relative_to(ROOT)), "json-schema-2020-12", True))
    for pattern, role, normative in (
        ("conformance/current-objects/*.json", "current-object-vector", True),
        (
            "conformance/fixtures/authority/math-coh-00/**/*",
            "authority-chain-vector",
            True,
        ),
        ("conformance/fixtures/correction/*.json", "correction-impact-vector", True),
        ("conformance/emitters/*", "independent-emitter", True),
        ("conformance/readers/python/*.py", "independent-reader", True),
        ("conformance/readers/javascript/*.mjs", "independent-reader", True),
        ("examples/**/*", "reference-flow", False),
    ):
        for path in sorted(ROOT.glob(pattern)):
            if (
                path.is_file()
                and "__pycache__" not in path.parts
                and path.name not in {"producer.seed.hex", "verifier.seed.hex", ".DS_Store"}
            ):
                selected.append((str(path.relative_to(ROOT)), role, normative))

    unique = {(path, role, normative) for path, role, normative in selected}
    if len(unique) != len(selected):
        raise AssertionError("Protocol 1 selection contains duplicate entries")
    return sorted(unique)


def build() -> dict[str, object]:
    entries = []
    for relative, role, normative in paths():
        path = ROOT / relative
        payload = path.read_bytes()
        entries.append(
            {
                "path": relative,
                "role": role,
                "normative": normative,
                "bytes": len(payload),
                "sha256": "sha256:" + hashlib.sha256(payload).hexdigest(),
            }
        )
    manifest: dict[str, object] = {
        "schema": "vela.protocol-conformance-manifest.v1",
        "protocol": "Vela Protocol 1",
        "status": "released",
        "software_release": "0.977.6",
        "authority_effect": "none",
        "entries": entries,
        "root_rule": "sha256 of RFC 8785 canonical JSON after removing only manifest_root",
    }
    manifest["manifest_root"] = "sha256:" + hashlib.sha256(canonical_bytes(manifest)).hexdigest()
    return manifest


def render(value: dict[str, object]) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bless", action="store_true")
    arguments = parser.parse_args()
    expected = render(build())
    if arguments.bless:
        MANIFEST.write_text(expected, encoding="utf-8")
        print(f"protocol-1: wrote {MANIFEST.relative_to(ROOT)}")
        return 0
    observed = MANIFEST.read_text(encoding="utf-8")
    if observed != expected:
        print(
            "protocol-1: manifest drift; review the changed standards surface, then run "
            "`python conformance/verify_protocol_1.py --bless`",
            file=sys.stderr,
        )
        return 1
    manifest = json.loads(observed)
    normative = sum(entry["normative"] for entry in manifest["entries"])
    informative = len(manifest["entries"]) - normative
    print(
        f"protocol-1: {normative} normative and {informative} informative files; "
        f"root {manifest['manifest_root']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
