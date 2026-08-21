#!/usr/bin/env python3
"""Freeze the distinct prospective trust-store repair canary-03."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
RUNTIME = ROOT / "container-runtime"
SOURCE_CANARY = ROOT / "neutral-canary-02"
OUTPUT = ROOT / "neutral-canary-03"
IMAGE = "sha256:6274d83356076640d6e4bc810b97d37ac2d1b5ab02546dd7c2ebed16f915b547"
BASE_IMAGE = "sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85"
TRUST_BUNDLE_PATH = "/etc/ssl/certs/ca-certificates.crt"
TRUST_BUNDLE_BYTES = (
    "sha256:714d457d580922dbf1d0be8bd35ba236a842b50b0072ae791582a19adef772a5"
)
CA_PACKAGE_VERSION = "20250419~deb12u1"
CA_PACKAGE_BYTES = (
    "sha256:62b08a77d985d4253894b1f69aebda5925034ca4e294add364167fad8cb64a44"
)


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))


def tree_manifest(
    directory: Path, excluded: set[str] | None = None
) -> list[dict[str, Any]]:
    excluded = excluded or set()
    files = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        relative = path.relative_to(directory).as_posix()
        if relative in excluded or "/node_modules/" in f"/{relative}/":
            continue
        content = path.read_bytes()
        files.append(
            {"path": relative, "bytes": len(content), "sha256": digest(content)}
        )
    return files


def command_bytes(arguments: list[str]) -> bytes:
    result = subprocess.run(arguments, check=True, capture_output=True)
    if result.stderr:
        raise SystemExit(f"unexpected stderr from {arguments!r}")
    return result.stdout


def main() -> int:
    if OUTPUT.exists():
        raise SystemExit(f"refusing to overwrite existing {OUTPUT}")
    shutil.copytree(SOURCE_CANARY / "packet", OUTPUT / "packet")
    write(
        OUTPUT / "packet/records/items.json",
        [
            {"identifier": "garnet", "measurement": 15},
            {"identifier": "ember", "measurement": 4},
            {"identifier": "hollow", "measurement": 8},
            {"identifier": "fjord", "measurement": 12},
        ],
    )
    (OUTPUT / "packet/records/binding.txt").write_text(
        "sha256:87fd5a02862c171bc619a5ecd664113a29134928d1333d525361de0d343d07df\n"
    )
    schema = json.loads((OUTPUT / "packet/response-schema.json").read_text())
    schema["properties"]["identifiers"]["items"]["enum"] = [
        "ember",
        "fjord",
        "garnet",
        "hollow",
    ]
    schema["properties"]["sum"]["const"] = 39
    write(OUTPUT / "packet/response-schema.json", schema)
    (OUTPUT / "input").mkdir(parents=True)
    subprocess.run(
        [
            "python3",
            str(RUNTIME / "prepare-prompt.py"),
            "--packet",
            str(OUTPUT / "packet"),
            "--output",
            str(OUTPUT / "input/prompt.txt"),
            "--condition",
            "neutral-runtime-calibration",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )

    with tempfile.TemporaryDirectory() as temporary:
        temporary_path = Path(temporary)
        for mode in ("corrected", "legacy"):
            source = temporary_path / "strict" / mode
            source.mkdir(parents=True)
            subprocess.run(
                [str(RUNTIME / "preflight-config.sh"), mode, IMAGE, str(source)],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            shutil.copytree(source, OUTPUT / "offline-preflight" / "strict" / mode)
        for mode in ("positive", "missing", "corrupt"):
            source = temporary_path / "trust" / mode
            source.mkdir(parents=True)
            subprocess.run(
                [
                    str(RUNTIME / "preflight-trust.sh"),
                    mode,
                    IMAGE,
                    TRUST_BUNDLE_BYTES,
                    str(source),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            shutil.copytree(source, OUTPUT / "offline-preflight" / "trust" / mode)

    cli_evidence = OUTPUT / "cli-evidence"
    cli_evidence.mkdir()
    docker_prefix = [
        "docker",
        "run",
        "--rm",
        "--network=none",
        "--tmpfs",
        "/codex-home:rw,nosuid,size=16m,uid=10001,gid=10001",
    ]
    (cli_evidence / "version.txt").write_bytes(
        command_bytes([*docker_prefix, "--entrypoint", "codex", IMAGE, "--version"])
    )
    (cli_evidence / "features-list.txt").write_bytes(
        command_bytes(
            [*docker_prefix, "--entrypoint", "codex", IMAGE, "features", "list"]
        )
    )
    help_raw = command_bytes([*docker_prefix, "--entrypoint", "codex", IMAGE, "--help"])
    (cli_evidence / "help.txt").write_bytes(
        b"\n".join(line.rstrip() for line in help_raw.splitlines()) + b"\n"
    )
    (cli_evidence / "help-raw.sha256").write_text(digest(help_raw) + "\n")

    trust_provenance = {
        "schema": "vela.inherited-correction-ca-provenance.v1",
        "source_distribution": "Debian 12 bookworm-security",
        "source_url": "https://deb.debian.org/debian-security/pool/updates/main/c/ca-certificates/ca-certificates_20250419~deb12u1_all.deb",
        "package": "ca-certificates",
        "package_version": CA_PACKAGE_VERSION,
        "package_bytes": CA_PACKAGE_BYTES,
        "bundle_generation": "dpkg-deb extract; concatenate usr/share/ca-certificates/mozilla/*.crt in LC_ALL=C path order",
        "bundle_path": TRUST_BUNDLE_PATH,
        "bundle_bytes": TRUST_BUNDLE_BYTES,
        "certificate_count": 150,
        "base_image_digest": BASE_IMAGE,
        "repaired_image_digest": IMAGE,
        "tls_verification_disabled": False,
        "insecure_overrides": [],
        "oauth_boundary": "unchanged read-only auth.json mount into disposable CODEX_HOME",
    }
    write(OUTPUT / "trust-material/provenance.json", trust_provenance)
    trust_provenance_root = canonical_root(trust_provenance)
    diagnosis = {
        "schema": "vela.inherited-correction-trust-diagnosis.v1",
        "parent_canary_commit": "70be21e2404af68daf5673f8094c47563224a11e",
        "parent_canary_result_root": "sha256:f608681e98eb7e4a49c48c5948dd24e118e74eb7d03754f77307a441ac202349",
        "parent_canary_provider_events_bytes": "sha256:78d64f7f5d7a874e91c879e56f04eacb21797295e3ef7ca9b99a83ad9e873d91",
        "parent_canary_provider_stderr_bytes": "sha256:9262d314e1db0a6665ca2a6974721f2b059f0fba695e24b16ab6e17e08682adf",
        "parent_image_digest": "sha256:13b753749787d68d628cea899f6b9875c0fc51c43877599b9aabf2009fe83388",
        "parent_image_ca_bundle_present": False,
        "parent_image_ca_package_installed": False,
        "docker_https_proxy_configured": True,
        "observed_failure": "invalid peer certificate: UnknownIssuer followed by failed HTTPS fallback and exact 600-second timeout",
        "diagnosis": "the frozen parent image omitted the operating-system CA bundle required to authenticate the proxy-forwarded provider certificate chain",
    }
    write(OUTPUT / "trust-material/diagnosis.json", diagnosis)
    diagnosis_root = canonical_root(diagnosis)

    strict_overrides = json.loads(
        command_bytes(
            [
                "node",
                "--input-type=module",
                "-e",
                f"import {{STRICT_OVERRIDES}} from {json.dumps((RUNTIME / 'strict-config.mjs').as_uri())}; process.stdout.write(JSON.stringify(STRICT_OVERRIDES))",
            ]
        )
    )
    strict_overrides_root = canonical_root(strict_overrides)
    packet_manifest = tree_manifest(OUTPUT / "packet")
    packet_root = canonical_root(packet_manifest)
    prompt_root = digest((OUTPUT / "input/prompt.txt").read_bytes())
    schema_bytes = digest((OUTPUT / "packet/response-schema.json").read_bytes())
    expected_response = {
        "schema": "neutral.runtime-canary-response.v1",
        "identifiers": ["ember", "fjord", "garnet", "hollow"],
        "sum": 39,
        "binding": "sha256:87fd5a02862c171bc619a5ecd664113a29134928d1333d525361de0d343d07df",
    }
    expected_response_root = canonical_root(expected_response)
    preflight_root = canonical_root(tree_manifest(OUTPUT / "offline-preflight"))
    cli_evidence_root = canonical_root(tree_manifest(cli_evidence))
    runtime_source_root = canonical_root(tree_manifest(RUNTIME))
    amendment = {
        "schema": "vela.inherited-correction-neutral-canary-amendment.v2",
        "status": "prospective_before_canary_03",
        "parent_canary_commit": "70be21e2404af68daf5673f8094c47563224a11e",
        "parent_canary_result_root": "sha256:f608681e98eb7e4a49c48c5948dd24e118e74eb7d03754f77307a441ac202349",
        "parent_canary_disposition": "terminal calibration evidence; unchanged; no retry",
        "diagnosis_root": diagnosis_root,
        "defect": "the pinned canary-02 image omitted an operating-system CA bundle",
        "repair": "copy only the deterministic CA bundle extracted from the hash-pinned Debian ca-certificates package into the newly pinned image",
        "trust_provenance_root": trust_provenance_root,
        "local_cli_evidence_root": cli_evidence_root,
        "offline_preflight_root": preflight_root,
        "runtime_source_root": runtime_source_root,
        "confirmatory_effect": "none; replacement registration remains unfrozen and 0/16",
    }
    amendment_root = canonical_root(amendment)
    write(OUTPUT / "amendment.json", amendment)
    registration = {
        "schema": "vela.inherited-correction-neutral-canary-registration.v3",
        "status": "calibration_only_authorized",
        "amendment_root": amendment_root,
        "study_registration_root": "sha256:7391c3c6adb74633886fd9fb2d35a257e7501bd37153acfb3e19ac850d0e9157",
        "purpose": "qualify the exact trust-store repair and one-response runtime without study facts or scoring",
        "confirmatory_denominator_credit": False,
        "scientific_claim_credit": False,
        "provider": "openai-chatgpt-oauth-codex",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "image_digest": IMAGE,
        "base_image_digest": BASE_IMAGE,
        "codex_cli_version": "0.149.0",
        "trust_bundle_path": TRUST_BUNDLE_PATH,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "trust_provenance_root": trust_provenance_root,
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "offline_preflight_root": preflight_root,
        "attempt": 1,
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "tools": "none tolerated",
        "sessions": 1,
        "retries": 0,
    }
    registration_root = canonical_root(registration)
    write(OUTPUT / "registration.json", registration)
    configuration = {
        "schema": "vela.inherited-correction-oci-participant-configuration.v3",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "base_image_digest": BASE_IMAGE,
        "codex_cli_version": "0.149.0",
        "authentication": "read-only ChatGPT OAuth auth.json mount into disposable CODEX_HOME",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "trust_bundle_path": TRUST_BUNDLE_PATH,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "strict_overrides": strict_overrides,
        "one_prompt": True,
        "one_model_turn": True,
        "tools": "none",
        "tool_boundary": "supported disables plus immediate streaming abort and terminal failure on any tool event",
        "workdir": "empty read-only participant workdir",
        "store": "ephemeral",
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "provider_usage_disposition": "cost telemetry only; only genuine provider context/output-limit failure invalidates",
        "attempt": 1,
        "retries": 0,
    }
    configuration_root = canonical_root(configuration)
    write(OUTPUT / "input/participant-configuration.json", configuration)
    (OUTPUT / "input/response-schema.json").write_bytes(
        (OUTPUT / "packet/response-schema.json").read_bytes()
    )
    assignment = {
        "schema": "vela.inherited-correction-neutral-canary-assignment.v3",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "assignments": [
            {
                "run_id": "neutral-canary-03",
                "condition": "neutral-runtime-calibration",
                "participant_instance_id": "neutral-oci-03",
                "packet_root": packet_root,
            }
        ],
    }
    assignment_root = canonical_root(assignment)
    write(OUTPUT / "input/assignment.json", assignment)
    authorization = {
        "schema": "vela.inherited-correction-neutral-canary-authorization.v3",
        "status": "authorized_calibration_only",
        "authorization_source": "supervisor task 01a02473-9465-76e2-ac9b-ae41379a3aa6 explicit canary-03 authorization",
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "max_sessions": 1,
        "confirmatory_sessions_authorized": 0,
    }
    authorization_root = canonical_root(authorization)
    write(OUTPUT / "authorization.json", authorization)
    permit = {
        "schema": "vela.inherited-correction-launch-permit.v1",
        "status": "authorized",
        "expires_at": "2026-08-23T23:59:59Z",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "run_id": "neutral-canary-03",
        "condition": "neutral-runtime-calibration",
        "participant_instance_id": "neutral-oci-03",
        "prompt_root": prompt_root,
        "packet_root": packet_root,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "attempt": 1,
    }
    permit_root = canonical_root(permit)
    write(OUTPUT / "permit-template/neutral-canary-03.permit.json", permit)
    write(
        OUTPUT / "permit-template/hold-state.default.json",
        {
            "schema": "vela.inherited-correction-hold.v1",
            "status": "hold",
            "reason": "default; no launch without exact frozen release",
            "updated_at": "2026-08-21T17:28:13Z",
        },
    )
    write(
        OUTPUT / "permit-template/hold-state.json",
        {
            "schema": "vela.inherited-correction-hold.v1",
            "status": "release",
            "reason": "one distinct neutral calibration canary-03 only; confirmatory study remains held",
            "updated_at": "2026-08-21T17:28:13Z",
        },
    )
    freeze = {
        "schema": "vela.inherited-correction-neutral-canary-freeze.v3",
        "status": "frozen_prelaunch_0_of_1",
        "amendment_root": amendment_root,
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "authorization_root": authorization_root,
        "permit_root": permit_root,
        "image_digest": IMAGE,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "trust_provenance_root": trust_provenance_root,
        "diagnosis_root": diagnosis_root,
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "offline_preflight_root": preflight_root,
        "files": tree_manifest(OUTPUT, {"prelaunch-freeze.json"}),
        "canary_01_status": "terminal_failed_closed_unchanged",
        "canary_02_status": "terminal_timeout_unchanged",
        "confirmatory_status": "stopped_0_of_16_replacement_not_registered",
    }
    write(OUTPUT / "prelaunch-freeze.json", freeze)
    print(
        json.dumps(
            {
                key: freeze[key]
                for key in freeze
                if key.endswith("_root")
                or key in {"image_digest", "trust_bundle_bytes"}
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
