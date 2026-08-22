#!/usr/bin/env python3
"""Generate and qualify the two held provider-specific offline bundles.

This is deliberately a no-provider operation.  It uses the maintained
qualifier's executable BundleFixture template, replaces the template image and
trust identities with provider-specific retained bytes, and consumes only the
qualifier's synthetic self-test permit.  The two campaign neutral-calibration
permits remain held and their consumed paths remain absent.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from typing import Any

QUALIFIER_COMMIT = "cc3b88d8bfcfd7b4f720a023f049d5c365be9423"
QUALIFIER_TREE = "341e0d22fa570b1b5e8dd9f70b219c11308ba45f"
QUALIFIER_SHA256 = "61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013"
REVIEWED_PREDECESSOR_COMMIT = "b333186cae1274ebb48353ba72e1ab3be42adcc0"
REVIEWED_PREDECESSOR_PARENT_COMMIT = "5be82cb3ab1ef11e7e870675337ae3704118fd46"
INVALID_PERMIT_ORIGIN_COMMIT = "9da1c79425c79af632197a719ca45ca07ab22a6c"
SOURCE_DATE_EPOCH = 1_757_289_600
RUNNER_SOURCE = Path(__file__).resolve().parent / "runtime-runner"
NEUTRAL_INPUTS = Path(__file__).resolve().parent / "neutral-calibration"
CANONICAL_BUNDLE = Path("/private/tmp/vela-stage-a-runtime-qualification-bundle-v1")
CANONICAL_QUALIFIER = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-maintained-v1"
)
CANONICAL_ENVIRONMENT = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-python-v1"
)
ADAPTERS = ("openai-responses-v1", "anthropic-messages-v1")
PROVIDERS = {
    "openai-responses-v1": (
        "OpenAI",
        "gpt-5.6-sol",
        "neutral-calibration-openai-json-v2",
    ),
    "anthropic-messages-v1": (
        "Anthropic",
        "claude-opus-5",
        "neutral-calibration-anthropic-json-v2",
    ),
}
RETIRED_PERMITS = {
    "openai-responses-v1": (
        "neutral-calibration-openai",
        "sha256:96a9c8af3d079ab8c73dd8eaaca05d62eebde2c70efe97a192b462edf2f7ff03",
    ),
    "anthropic-messages-v1": (
        "neutral-calibration-anthropic",
        "sha256:4bed98283ffb3af24ed0c99d7d4e135276770fef8288c11fbe87e9c8b0d37b9f",
    ),
}


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def load(path: Path) -> Any:
    return json.loads(path.read_bytes())


def write(path: Path, raw: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(raw)


def write_json(path: Path, value: Any) -> None:
    write(path, encoded(value))


def checked_workspace(path: Path) -> Path:
    resolved = path.resolve()
    if not resolved.is_absolute() or resolved in {Path("/"), Path.home()}:
        raise ValueError("unsafe_workspace")
    if "stage-a-runtime-qualification" not in resolved.name:
        raise ValueError("workspace_name_not_scoped")
    return resolved


def exact_git_value(repository: Path, *arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def exact_git_bytes(repository: Path, object_name: str) -> bytes:
    return subprocess.run(
        ["git", "show", object_name],
        cwd=repository,
        check=True,
        capture_output=True,
    ).stdout


def stage_qualifier(repository: Path, workspace: Path) -> Path:
    if (
        exact_git_value(repository, "rev-parse", f"{QUALIFIER_COMMIT}^{{tree}}")
        != QUALIFIER_TREE
    ):
        raise ValueError("qualifier_tree_drift")
    qualifier_raw = exact_git_bytes(
        repository,
        f"{QUALIFIER_COMMIT}:tools/evidence_qualification/qualification.py",
    )
    if hashlib.sha256(qualifier_raw).hexdigest() != QUALIFIER_SHA256:
        raise ValueError("qualifier_sha256_drift")
    del workspace
    staged = CANONICAL_QUALIFIER / "tools/evidence_qualification"
    if staged.parent.parent.exists():
        shutil.rmtree(staged.parent.parent)
    staged.mkdir(parents=True)
    write(staged.parent / "__init__.py", b"")
    write(staged / "__init__.py", b"")
    for name in ("qualification.py", "test_qualification.py"):
        write(
            staged / name,
            exact_git_bytes(
                repository,
                f"{QUALIFIER_COMMIT}:tools/evidence_qualification/{name}",
            ),
        )
    return staged.parent.parent


def prepare_environment(repository: Path, workspace: Path) -> Path:
    del workspace
    project = CANONICAL_ENVIRONMENT
    if project.exists():
        shutil.rmtree(project)
    project.mkdir(parents=True, exist_ok=True)
    for name in ("pyproject.toml", "uv.lock"):
        write(project / name, (repository / "conformance" / name).read_bytes())
    subprocess.run(
        ["uv", "sync", "--project", str(project), "--locked", "--offline"],
        check=True,
        env={**os.environ, "UV_NO_PROGRESS": "1"},
    )
    return project / ".venv/bin/python"


def canonical_tar(
    files: dict[str, bytes], *, executable: frozenset[str] = frozenset()
) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, raw in sorted(files.items()):
            info = tarfile.TarInfo(name)
            info.size = len(raw)
            info.mtime = SOURCE_DATE_EPOCH
            info.mode = 0o755 if name in executable else 0o644
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(raw))
    return buffer.getvalue()


def provider_contract(adapter: str) -> dict[str, Any]:
    endpoint = {
        "openai-responses-v1": "https://api.openai.com/v1/responses",
        "anthropic-messages-v1": "https://api.anthropic.com/v1/messages",
    }[adapter]
    request = (
        {
            "api": "Responses",
            "background": False,
            "max_output_tokens": 32768,
            "parallel_tool_calls": False,
            "reasoning_effort": "high",
            "service_tier": "default",
            "store": False,
            "temperature": "omitted",
        }
        if adapter == "openai-responses-v1"
        else {
            "api": "Messages",
            "anthropic_version": "2023-06-01",
            "max_tokens": 32768,
            "output_config_effort": "high",
            "service_tier": "standard_only",
            "temperature": "omitted",
            "thinking": "adaptive",
        }
    )
    events = {
        "raw_bytes_retained_before_normalization": True,
        "tool_arguments_retained": True,
        "tool_results_retained": True,
        "usage_retained_as_telemetry_only": True,
        "terminal_and_teardown_receipts_required": True,
    }
    if adapter == "openai-responses-v1":
        events.update(
            {
                "function_call_arguments_wire_type": "json_string",
                "function_call_arguments_decode_count": 1,
                "decoded_argument_bytes_retained": True,
                "raw_to_decoded_argument_binding_required": True,
            }
        )
    return {
        "schema": "vela.stage-a-provider-runtime-contract.v1",
        "provider_adapter": adapter,
        "endpoint": endpoint,
        "request": request,
        "transport": {
            "participant_network": False,
            "host_bridge_fd": 3,
            "host_bridge_single_endpoint": endpoint,
            "redirects": "rejected",
            "proxy_environment": "ignored",
            "unrestricted_clients": False,
            "credential_fd": 4,
            "credential_retained": False,
        },
        "packet_input": {
            "mount_path": "/input/packet.json",
            "regular_file_only": True,
            "single_link_only": True,
            "no_follow": True,
            "canonical_json_object": True,
            "recursive_duplicate_keys_rejected": True,
            "recursive_objects_arrays_primitives_canonical": True,
            "number_lexemes_preserved": True,
            "inline_reconstruction": False,
            "permit_byte_root_required": True,
            "request_byte_root_receipt_required": True,
            "injection": (
                "input[0].content[1].text_exact_packet_bytes"
                if adapter == "openai-responses-v1"
                else "messages[0].content_exact_prompt_newline_packet_bytes"
            ),
        },
        "events": events,
        "tools": [
            {
                "name": "shell",
                "allowed_argv": ["git", "--no-optional-locks", "status", "--short"],
                "cwd": "/workspace",
                "read_only": True,
                "shell_interpolation": False,
            },
            {
                "name": "read_file",
                "workspace": "/workspace",
                "operations": ["read", "list", "stat"],
                "regular_files_only": True,
                "symlinks": False,
                "path_escape": False,
                "write": False,
            },
        ],
    }


def build_runner(adapter: str, target: Path, cache: Path) -> tuple[bytes, bytes]:
    shutil.copytree(
        RUNNER_SOURCE,
        target,
        ignore=shutil.ignore_patterns(".ruff_cache", "__pycache__", "*.pyc"),
    )
    env = {
        **os.environ,
        "CGO_ENABLED": "0",
        "GOOS": "linux",
        "GOARCH": "arm64",
        "GOCACHE": str(cache / "go-build"),
        "GOMODCACHE": str(cache / "go-mod"),
    }
    output = target / "runner"
    subprocess.run(
        [
            "go",
            "build",
            "-mod=readonly",
            "-trimpath",
            "-buildvcs=false",
            "-ldflags",
            f"-s -w -buildid= -X main.providerAdapter={adapter}",
            "-o",
            str(output),
            ".",
        ],
        cwd=target,
        env=env,
        check=True,
    )
    bridge = target / "bridge"
    subprocess.run(
        [
            "go",
            "build",
            "-mod=readonly",
            "-trimpath",
            "-buildvcs=false",
            "-ldflags",
            f"-s -w -buildid= -X main.providerAdapter={adapter}",
            "-o",
            str(bridge),
            "./cmd/bridge",
        ],
        cwd=target,
        env=env,
        check=True,
    )
    return output.read_bytes(), bridge.read_bytes()


def actual_oci(
    adapter: str,
    runner_raw: bytes,
    bridge_raw: bytes,
    contract_raw: bytes,
    trust_raw: bytes,
    q: Any,
) -> tuple[bytes, str, str, list[str], str]:
    rootfs = canonical_tar(
        {
            "opt/vela/runner": runner_raw,
            "opt/vela/bridge": bridge_raw,
            "opt/vela/provider-contract.json": contract_raw,
            "etc/ssl/certs/ca-certificates.crt": trust_raw,
        },
        executable=frozenset({"opt/vela/runner", "opt/vela/bridge"}),
    )
    layer_digest = q.digest(rootfs)
    config = {
        "architecture": "arm64",
        "os": "linux",
        "created": "2025-09-08T00:00:00Z",
        "config": {
            "User": "65532:65532",
            "Env": ["SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"],
            "Entrypoint": ["/opt/vela/runner"],
            "Cmd": ["--run"],
            "Labels": {
                "org.vela.provider-adapter": adapter,
                "org.vela.runtime-mode": "held-host-bridge-network-none",
                "org.vela.source-date-epoch": str(SOURCE_DATE_EPOCH),
            },
        },
        "rootfs": {"type": "layers", "diff_ids": [layer_digest]},
        "history": [
            {
                "created": "2025-09-08T00:00:00Z",
                "created_by": "vela-stage-a-independent-offline-builder",
            }
        ],
    }
    config_raw = q.canonical_json_bytes(config)
    config_digest = q.digest(config_raw)
    manifest = {
        "schemaVersion": 2,
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest,
            "size": len(config_raw),
        },
        "layers": [
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar",
                "digest": layer_digest,
                "size": len(rootfs),
            }
        ],
    }
    manifest_raw = q.canonical_json_bytes(manifest)
    manifest_digest = q.digest(manifest_raw)
    index = {
        "schemaVersion": 2,
        "manifests": [
            {
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": manifest_digest,
                "size": len(manifest_raw),
            }
        ],
    }
    layout_raw = b'{"imageLayoutVersion":"1.0.0"}\n'
    files = {
        "index.json": q.canonical_json_bytes(index),
        "oci-layout": layout_raw,
        "blobs/sha256/" + manifest_digest.removeprefix("sha256:"): manifest_raw,
        "blobs/sha256/" + config_digest.removeprefix("sha256:"): config_raw,
        "blobs/sha256/" + layer_digest.removeprefix("sha256:"): rootfs,
    }
    raw = canonical_tar(files)
    return raw, manifest_digest, config_digest, [layer_digest], q.digest(layout_raw)


def verify_launchable_image(image_raw: bytes, adapter: str, q: Any) -> dict[str, Any]:
    with tarfile.open(fileobj=io.BytesIO(image_raw), mode="r") as archive:
        members = {
            member.name: archive.extractfile(member).read()
            for member in archive.getmembers()
            if member.isfile()
        }
    index = json.loads(members["index.json"])
    manifest_digest = index["manifests"][0]["digest"]
    manifest = json.loads(
        members["blobs/sha256/" + manifest_digest.removeprefix("sha256:")]
    )
    layer_digest = manifest["layers"][0]["digest"]
    layer_raw = members["blobs/sha256/" + layer_digest.removeprefix("sha256:")]
    with tempfile.TemporaryDirectory() as temporary:
        layer_path = Path(temporary) / "rootfs.tar"
        write(layer_path, layer_raw)
        tag = "vela-stage-a-" + adapter.split("-", 1)[0] + ":held-self-test"
        subprocess.run(
            [
                "docker",
                "import",
                "--platform",
                "linux/arm64",
                "--change",
                "USER 65532:65532",
                "--change",
                'ENTRYPOINT ["/opt/vela/runner"]',
                str(layer_path),
                tag,
            ],
            check=True,
            capture_output=True,
        )
        controls = [
            "--rm",
            "--network",
            "none",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
        ]
        self_test = subprocess.run(
            ["docker", "run", *controls, tag, "--self-test"],
            check=True,
            capture_output=True,
        )
        version = subprocess.run(
            ["docker", "run", *controls, tag, "--version"],
            check=True,
            capture_output=True,
        )
        bridge_test = subprocess.run(
            [
                "docker",
                "run",
                *controls,
                "--entrypoint",
                "/opt/vela/bridge",
                tag,
                "--self-test",
            ],
            check=True,
            capture_output=True,
        )
    if (
        self_test.stdout
        or self_test.stderr
        or version.stdout != b"neutral-runner/1\n"
        or bridge_test.stdout
        or bridge_test.stderr
    ):
        raise ValueError("launchability_self_test_output_drift")
    return {
        "schema": "vela.stage-a-runtime-launchability.v1",
        "provider_adapter": adapter,
        "oci_archive_sha256": q.digest(image_raw),
        "image_digest": manifest_digest,
        "layer_digest": layer_digest,
        "platform": "linux/arm64",
        "entrypoint": ["/opt/vela/runner"],
        "self_test": {
            "network": "none",
            "root_filesystem": "read_only",
            "capabilities": "all_dropped",
            "no_new_privileges": True,
            "exit_code": 0,
            "stdout_sha256": q.digest(self_test.stdout),
            "stderr_sha256": q.digest(self_test.stderr),
            "runner_version": "neutral-runner/1",
            "host_bridge_self_test": True,
        },
        "provider_calls": 0,
        "credential_values_observed": False,
    }


def provider_image(bundle: Path, adapter: str, trust_raw: bytes, q: Any) -> None:
    runtime_source = bundle / "runtime/source"
    shutil.rmtree(runtime_source)
    with (
        tempfile.TemporaryDirectory() as first,
        tempfile.TemporaryDirectory() as second,
    ):
        first_root = Path(first)
        second_root = Path(second)
        runner_a, bridge_a = build_runner(
            adapter, first_root / "source", first_root / "cache"
        )
        runner_b, bridge_b = build_runner(
            adapter, second_root / "source", second_root / "cache"
        )
        if runner_a != runner_b or bridge_a != bridge_b:
            raise ValueError("independent_runner_binaries_not_byte_identical")
        shutil.copytree(first_root / "source", runtime_source)
    contract_raw = encoded(provider_contract(adapter))
    write(runtime_source / "provider-contract.json", contract_raw)
    write(runtime_source / "vendor/ca-certificates.crt", trust_raw)
    # The locally compiled binary is a generated source input to the OCI-only
    # build. Its exact no-dependency Go source remains beside it.
    goroot = Path(
        subprocess.run(
            ["go", "env", "GOROOT"], check=True, capture_output=True, text=True
        ).stdout.strip()
    )
    license_path = goroot / "LICENSE"
    if not license_path.is_file():
        license_path = goroot.parent / "LICENSE"
    license_raw = license_path.read_bytes()
    write(runtime_source / "vendor/go-LICENSE", license_raw)
    manifest = q.tree_manifest(runtime_source)
    write_json(bundle / "runtime/source-manifest.json", manifest)
    source_root = q.canonical_root(manifest)
    build_inputs = {
        "schema": "vela.tooling.vendored-build-inputs.v1",
        "inputs": [
            {
                "path": "runtime/source/vendor/go-LICENSE",
                "bytes": len(license_raw),
                "sha256": q.digest(license_raw),
                "source_url": "https://go.googlesource.com/go/+/refs/tags/go1.26.2/LICENSE",
                "source_sha256": q.digest(license_raw),
                "license_path": "runtime/source/LICENSE",
            }
        ],
    }
    write_json(bundle / "runtime/build-inputs.json", build_inputs)
    build_inputs_root = q.canonical_root(build_inputs)
    oci_a = actual_oci(adapter, runner_a, bridge_a, contract_raw, trust_raw, q)
    oci_b = actual_oci(adapter, runner_b, bridge_b, contract_raw, trust_raw, q)
    if oci_a != oci_b:
        raise ValueError("independent_oci_builds_not_byte_identical")
    raw, manifest_digest, config_digest, layers, layout_digest = oci_a
    for name in ("a.oci.tar", "b.oci.tar"):
        write(bundle / "runtime" / name, raw)
    for name in ("independent-a.json", "independent-b.json"):
        receipt_path = bundle / "runtime" / name
        receipt = load(receipt_path)
        receipt.update(
            {
                "source_date_epoch": SOURCE_DATE_EPOCH,
                "source_root": source_root,
                "build_inputs_root": build_inputs_root,
                "image_digest": manifest_digest,
                "config_digest": config_digest,
                "layer_digests": layers,
                "oci_layout_bytes": layout_digest,
                "oci_tar_bytes": q.digest(raw),
            }
        )
        write_json(receipt_path, receipt)
    compatibility_path = bundle / "config/compatibility.json"
    compatibility = load(compatibility_path)
    compatibility.update(
        {
            "image_digest": manifest_digest,
            "runtime_source_root": source_root,
            "dockerfile_bytes": q.digest((runtime_source / "Dockerfile").read_bytes()),
        }
    )
    write_json(compatibility_path, compatibility)
    configuration_path = bundle / "qualification.json"
    configuration = load(configuration_path)
    configuration["runtime"]["trust_bundle_sha256"] = q.digest(trust_raw)
    write_json(configuration_path, configuration)
    for relative in (
        "permit/participant-run-01.permit.json",
        "fixture/permit/neutral-qualification-01.permit.template.json",
        "fixture/permit/neutral-qualification-01.permit.consumed.json",
    ):
        value = load(bundle / relative)
        value.update(
            {"image_digest": manifest_digest, "runtime_source_root": source_root}
        )
        write_json(bundle / relative, value)
    consumed = bundle / "fixture/permit/neutral-qualification-01.permit.consumed.json"
    launch_path = bundle / "fixture/evidence/launch.json"
    launch = load(launch_path)
    launch.update(
        {
            "image_digest": manifest_digest,
            "runtime_source_root": source_root,
            "permit_bytes": q.digest(consumed.read_bytes()),
        }
    )
    write_json(launch_path, launch)
    teardown_path = bundle / "fixture/evidence/teardown.json"
    teardown = load(teardown_path)
    teardown.update(
        {
            "permit_bytes": q.digest(consumed.read_bytes()),
            "launch_bytes": q.digest(launch_path.read_bytes()),
        }
    )
    write_json(teardown_path, teardown)
    terminal_path = bundle / "fixture/evidence/terminal-receipt.json"
    terminal = load(terminal_path)
    terminal.update(
        {
            "image_digest": manifest_digest,
            "trust_bundle_sha256": q.digest(trust_raw),
            "runtime_source_root": source_root,
            "permit_bytes": q.digest(consumed.read_bytes()),
            "launch_bytes": q.digest(launch_path.read_bytes()),
            "teardown_receipt_bytes": q.digest(teardown_path.read_bytes()),
        }
    )
    write_json(terminal_path, terminal)


def replace_trust(bundle: Path, trust_raw: bytes, q: Any) -> None:
    trust_path = bundle / "runtime/source/vendor/ca-certificates.crt"
    write(trust_path, trust_raw)
    manifest = q.tree_manifest(bundle / "runtime/source")
    write_json(bundle / "runtime/source-manifest.json", manifest)
    source_root = q.canonical_root(manifest)
    trust_digest = q.digest(trust_raw)
    for name in ("independent-a.json", "independent-b.json"):
        receipt_path = bundle / "runtime" / name
        receipt = load(receipt_path)
        receipt["source_root"] = source_root
        write_json(receipt_path, receipt)
    compatibility_path = bundle / "config/compatibility.json"
    compatibility = load(compatibility_path)
    compatibility["runtime_source_root"] = source_root
    write_json(compatibility_path, compatibility)
    for relative in (
        "permit/participant-run-01.permit.json",
        "fixture/permit/neutral-qualification-01.permit.template.json",
        "fixture/permit/neutral-qualification-01.permit.consumed.json",
    ):
        value = load(bundle / relative)
        value["runtime_source_root"] = source_root
        write_json(bundle / relative, value)
    consumed = bundle / "fixture/permit/neutral-qualification-01.permit.consumed.json"
    launch_path = bundle / "fixture/evidence/launch.json"
    launch = load(launch_path)
    launch.update(
        {
            "runtime_source_root": source_root,
            "permit_bytes": q.digest(consumed.read_bytes()),
        }
    )
    write_json(launch_path, launch)
    config_path = bundle / "qualification.json"
    config = load(config_path)
    config["runtime"]["trust_bundle_sha256"] = trust_digest
    write_json(config_path, config)
    teardown_path = bundle / "fixture/evidence/teardown.json"
    teardown = load(teardown_path)
    teardown.update(
        {
            "permit_bytes": q.digest(consumed.read_bytes()),
            "launch_bytes": q.digest(launch_path.read_bytes()),
        }
    )
    write_json(teardown_path, teardown)
    terminal_path = bundle / "fixture/evidence/terminal-receipt.json"
    terminal = load(terminal_path)
    terminal.update(
        {
            "runtime_source_root": source_root,
            "trust_bundle_sha256": trust_digest,
            "permit_bytes": q.digest(consumed.read_bytes()),
            "launch_bytes": q.digest(launch_path.read_bytes()),
            "teardown_receipt_bytes": q.digest(teardown_path.read_bytes()),
        }
    )
    write_json(terminal_path, terminal)


def hold_neutral_permit(bundle: Path, adapter: str, q: Any) -> None:
    provider, _model, run_id = PROVIDERS[adapter]
    config_path = bundle / "qualification.json"
    config = load(config_path)
    participant = config["participant_permit"]
    identity = participant["identity"]
    identity.update(
        {
            "registration_id": "stage-a-runtime-registration",
            "assignment_id": "neutral-calibration-assignment-" + provider.lower(),
            "participant_id": "neutral-calibration-" + provider.lower(),
            "run_id": run_id,
            "condition": "neutral-no-science-held",
            "prompt_root": q.digest((NEUTRAL_INPUTS / "prompt.txt").read_bytes()),
            "packet_root": q.digest((NEUTRAL_INPUTS / "packet.json").read_bytes()),
        }
    )
    permit_path = bundle / participant["permit"]
    permit = load(permit_path)
    permit.update(identity)
    write_json(permit_path, permit)
    hold_path = bundle / participant["hold"]
    hold = load(hold_path)
    hold.update(
        {
            "registration_id": identity["registration_id"],
            "assignment_id": identity["assignment_id"],
        }
    )
    write_json(hold_path, hold)
    participant["consumed_permit"] = f"permit/{run_id}.permit.consumed.json"
    write_json(config_path, config)


def retired_permit_record(adapter: str, successor_root: str) -> dict[str, Any]:
    run_id, permit_root = RETIRED_PERMITS[adapter]
    return {
        "schema": "vela.stage-a-neutral-permit-retirement.v1",
        "provider_adapter": adapter,
        "run_id": run_id,
        "original_permit_root": permit_root,
        "invalid_permit_origin_commit": INVALID_PERMIT_ORIGIN_COMMIT,
        "invalid_permit_origin_relationship": "ancestor_of_reviewed_predecessor_not_direct_parent",
        "reviewed_predecessor_commit": REVIEWED_PREDECESSOR_COMMIT,
        "reviewed_predecessor_parent_commit": REVIEWED_PREDECESSOR_PARENT_COMMIT,
        "original_state": "held_unconsumed",
        "retirement_reason": "packet_root_preimage_is_plaintext_not_runner_loadable_canonical_json",
        "successor_permit_root": successor_root,
        "status": "retired_non_releasable",
        "consumed": False,
        "releasable": False,
        "authority_effect": "none",
    }


def refresh_capture(bundle: Path, fixture: Any) -> None:
    # The maintained helper rebuilds the tool-mode capture manifest after all
    # launch, teardown, terminal, raw-event, and tool-receipt bindings exist.
    fixture.refresh_capture_bindings()
    config = load(bundle / "qualification.json")
    extra = [
        bundle / "fixture/evidence/provider-events.raw.jsonl",
        bundle / "fixture/evidence/tool-receipts.json",
        bundle / "fixture/evidence/tool.stdout",
        bundle / "fixture/evidence/tool.stderr",
    ]
    capture_path = bundle / config["neutral_fixture"]["capture_manifest"]
    capture = load(capture_path)
    entries = {item["path"]: item for item in capture["entries"]}
    for path in extra:
        entries[path.relative_to(bundle / "fixture").as_posix()] = {
            "path": path.relative_to(bundle / "fixture").as_posix(),
            "bytes": path.stat().st_size,
            "sha256": digest(path.read_bytes()),
        }
    capture = {
        "schema": capture["schema"],
        "entries": sorted(entries.values(), key=lambda item: item["path"]),
    }
    capture["capture_root"] = digest(
        (json.dumps(capture, sort_keys=True, separators=(",", ":")) + "\n").encode()
    )
    write_json(capture_path, capture)


def snapshot(bundle: Path, adapter: str, q: Any) -> dict[str, Any]:
    config = load(bundle / "qualification.json")
    boundary_value = load(bundle / config["configuration"]["tool_boundary"])
    boundary = q.validate_tool_boundary(boundary_value)
    events_raw = (bundle / config["neutral_fixture"]["events"]).read_bytes()
    events = q.validate_tool_events(
        events_raw, config["configuration"]["output_token_ceiling"], boundary
    )
    tool_receipts = load(bundle / config["neutral_fixture"]["tool_receipts"])
    tool_root = q.validate_tool_receipts(bundle, tool_receipts, events, boundary)
    raw_path = bundle / config["neutral_fixture"]["raw_provider_events"]
    return {
        "provider_adapter": adapter,
        "provider_organization": boundary["provider_organization"],
        "tool_boundary_root": boundary["tool_boundary_root"],
        "tool_semantics_root": boundary["tool_semantics_root"],
        "participant_visible_atoms_root": config["neutral_fixture"]["identity"][
            "packet_root"
        ],
        "registered_schema_bytes": q.digest(
            (bundle / config["schemas"]["registered"]).read_bytes()
        ),
        "provider_schema_bytes": q.digest(
            (bundle / config["schemas"]["provider"]).read_bytes()
        ),
        "raw_provider_events_bytes": q.digest(raw_path.read_bytes()),
        "normalized_events_bytes": q.digest(events_raw),
        "normalized_tool_semantics_root": events["normalized_tool_semantics_root"],
        "tool_receipts_root": tool_root,
    }


def build_bundle(
    bundle: Path, adapter: str, trust_raw: bytes, modules: tuple[Any, Any]
) -> Any:
    q, tests = modules
    if bundle.exists():
        shutil.rmtree(bundle)
    fixture = tests.BundleFixture(bundle)
    provider_image(bundle, adapter, trust_raw, q)
    tests.upgrade_to_stage_a_tool_bundle(fixture, adapter)
    hold_neutral_permit(bundle, adapter, q)
    refresh_capture(bundle, fixture)
    return fixture


def run(repository: Path, workspace: Path, output: Path, trust_bundle: Path) -> None:
    workspace = checked_workspace(workspace)
    workspace.mkdir(parents=True, exist_ok=True)
    staged_root = stage_qualifier(repository, workspace)
    python = prepare_environment(repository, workspace)
    helper = workspace / "driver.py"
    source = Path(__file__).resolve()
    if source != helper:
        write(helper, source.read_bytes())
        os.execv(
            str(python),
            [
                str(python),
                str(helper),
                "--repository",
                str(repository),
                "--workspace",
                str(workspace),
                "--output",
                str(output),
                "--trust-bundle",
                str(trust_bundle),
                "--inside-fixed-environment",
            ],
        )
    sys.path.insert(0, str(staged_root))
    q = importlib.import_module("tools.evidence_qualification.qualification")
    tests = importlib.import_module("tools.evidence_qualification.test_qualification")
    modules = (q, tests)
    trust_raw = trust_bundle.read_bytes()
    if b"-----BEGIN CERTIFICATE-----" not in trust_raw:
        raise ValueError("trust_bundle_invalid")
    bundle = CANONICAL_BUNDLE
    observed: dict[str, dict[str, Any]] = {}
    for adapter in ADAPTERS:
        build_bundle(bundle, adapter, trust_raw, modules)
        observed[adapter] = snapshot(bundle, adapter, q)
    common = {
        key
        for key in observed[ADAPTERS[0]]
        if key
        not in {
            "provider_adapter",
            "provider_organization",
            "tool_boundary_root",
            "provider_schema_bytes",
            "raw_provider_events_bytes",
        }
    }
    for key in common:
        if observed[ADAPTERS[0]][key] != observed[ADAPTERS[1]][key]:
            raise ValueError(f"provider_equivalence_drift:{key}")
    records = []
    output_assets = output.parent / "offline-qualification-assets"
    if output_assets.exists():
        shutil.rmtree(output_assets)
    output_assets.mkdir(parents=True)
    for adapter in ADAPTERS:
        build_bundle(bundle, adapter, trust_raw, modules)
        equivalence = {
            "schema": q.PROVIDER_EQUIVALENCE_SCHEMA,
            "providers": [observed[item] for item in ADAPTERS],
        }
        write_json(bundle / "fixture/provider-equivalence.json", equivalence)
        receipt = q.qualify_bundle(bundle)
        provider, model, run_id = PROVIDERS[adapter]
        prefix = provider.lower()
        retained = {}
        launchability = verify_launchable_image(
            (bundle / "runtime/a.oci.tar").read_bytes(), adapter, q
        )
        write_json(bundle / "runtime/launchability.json", launchability)
        for label, relative in (
            ("image", "runtime/a.oci.tar"),
            ("launchability", "runtime/launchability.json"),
            ("source_manifest", "runtime/source-manifest.json"),
            ("build_a", "runtime/independent-a.json"),
            ("build_b", "runtime/independent-b.json"),
            ("runner", "runtime/source/runner"),
            ("bridge", "runtime/source/bridge"),
            ("provider_contract", "runtime/source/provider-contract.json"),
            ("provider_schema", "schemas/provider.json"),
            ("tool_boundary", "config/tool-boundary.json"),
            ("held_permit", "permit/participant-run-01.permit.json"),
            ("hold_state", "permit/hold-state.json"),
        ):
            raw = (bundle / relative).read_bytes()
            suffix = Path(relative).suffix or ".bin"
            target = output_assets / f"{prefix}-{label}{suffix}"
            write(target, raw)
            retained[label] = {
                "path": target.relative_to(output.parent).as_posix(),
                "bytes": len(raw),
                "sha256": q.digest(raw),
            }
        for label, source_name in (
            ("neutral_packet", "packet.json"),
            ("neutral_prompt", "prompt.txt"),
        ):
            raw = (NEUTRAL_INPUTS / source_name).read_bytes()
            target = output_assets / f"{prefix}-{label}{Path(source_name).suffix}"
            write(target, raw)
            retained[label] = {
                "path": target.relative_to(output.parent).as_posix(),
                "bytes": len(raw),
                "sha256": q.digest(raw),
            }
        retirement = retired_permit_record(adapter, receipt["participant_permit_root"])
        target = output_assets / f"{prefix}-retired_permit.json"
        write_json(target, retirement)
        raw = target.read_bytes()
        retained["retired_permit"] = {
            "path": target.relative_to(output.parent).as_posix(),
            "bytes": len(raw),
            "sha256": q.digest(raw),
        }
        records.append(
            {
                "provider_adapter": adapter,
                "provider_organization": provider,
                "model": model,
                "held_neutral_run_id": run_id,
                "qualification_receipt": receipt,
                "retained": retained,
                "consumed_neutral_permit_exists": False,
                "provider_calls": 0,
            }
        )
    value = {
        "schema": "vela.lean-correspondence-stage-a-offline-runtime-qualification.v1",
        "status": "offline_qualified_hold",
        "qualifier": {
            "commit": QUALIFIER_COMMIT,
            "tree": QUALIFIER_TREE,
            "sha256": "sha256:" + QUALIFIER_SHA256,
        },
        "trust_bundle_sha256": digest(trust_raw),
        "provider_records": records,
        "provider_calls": 0,
        "neutral_calibrations_run": 0,
        "participant_calls": 0,
        "authority_effect": "none",
    }
    value["record_root"] = q.canonical_root(value)
    write_json(output, value)


def main() -> int:
    global RUNNER_SOURCE, NEUTRAL_INPUTS
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trust-bundle", type=Path, required=True)
    parser.add_argument("--inside-fixed-environment", action="store_true")
    args = parser.parse_args()
    RUNNER_SOURCE = (
        args.repository.resolve()
        / "paper/artifacts/lean-correspondence-stage-a-runtime-qualification/runtime-runner"
    )
    NEUTRAL_INPUTS = (
        args.repository.resolve()
        / "paper/artifacts/lean-correspondence-stage-a-runtime-qualification/neutral-calibration"
    )
    if args.inside_fixed_environment:
        # Avoid recopying over the module currently executing from the fixed path.
        sys.path.insert(0, str(CANONICAL_QUALIFIER))
        q = importlib.import_module("tools.evidence_qualification.qualification")
        tests = importlib.import_module(
            "tools.evidence_qualification.test_qualification"
        )
        modules = (q, tests)
        trust_raw = args.trust_bundle.read_bytes()
        bundle = CANONICAL_BUNDLE
        observed = {}
        for adapter in ADAPTERS:
            build_bundle(bundle, adapter, trust_raw, modules)
            observed[adapter] = snapshot(bundle, adapter, q)
        common = set(observed[ADAPTERS[0]]) - {
            "provider_adapter",
            "provider_organization",
            "tool_boundary_root",
            "provider_schema_bytes",
            "raw_provider_events_bytes",
        }
        for key in common:
            if observed[ADAPTERS[0]][key] != observed[ADAPTERS[1]][key]:
                raise ValueError(f"provider_equivalence_drift:{key}")
        records = []
        output_assets = args.output.parent / "offline-qualification-assets"
        if output_assets.exists():
            shutil.rmtree(output_assets)
        output_assets.mkdir(parents=True)
        for adapter in ADAPTERS:
            build_bundle(bundle, adapter, trust_raw, modules)
            write_json(
                bundle / "fixture/provider-equivalence.json",
                {
                    "schema": q.PROVIDER_EQUIVALENCE_SCHEMA,
                    "providers": [observed[item] for item in ADAPTERS],
                },
            )
            receipt = q.qualify_bundle(bundle)
            provider, model, run_id = PROVIDERS[adapter]
            retained = {}
            launchability = verify_launchable_image(
                (bundle / "runtime/a.oci.tar").read_bytes(), adapter, q
            )
            write_json(bundle / "runtime/launchability.json", launchability)
            for label, relative in (
                ("image", "runtime/a.oci.tar"),
                ("launchability", "runtime/launchability.json"),
                ("source_manifest", "runtime/source-manifest.json"),
                ("build_a", "runtime/independent-a.json"),
                ("build_b", "runtime/independent-b.json"),
                ("runner", "runtime/source/runner"),
                ("bridge", "runtime/source/bridge"),
                ("provider_contract", "runtime/source/provider-contract.json"),
                ("provider_schema", "schemas/provider.json"),
                ("tool_boundary", "config/tool-boundary.json"),
                ("held_permit", "permit/participant-run-01.permit.json"),
                ("hold_state", "permit/hold-state.json"),
            ):
                raw = (bundle / relative).read_bytes()
                target = (
                    output_assets
                    / f"{provider.lower()}-{label}{Path(relative).suffix or '.bin'}"
                )
                write(target, raw)
                retained[label] = {
                    "path": target.relative_to(args.output.parent).as_posix(),
                    "bytes": len(raw),
                    "sha256": q.digest(raw),
                }
            for label, source_name in (
                ("neutral_packet", "packet.json"),
                ("neutral_prompt", "prompt.txt"),
            ):
                raw = (NEUTRAL_INPUTS / source_name).read_bytes()
                target = (
                    output_assets
                    / f"{provider.lower()}-{label}{Path(source_name).suffix}"
                )
                write(target, raw)
                retained[label] = {
                    "path": target.relative_to(args.output.parent).as_posix(),
                    "bytes": len(raw),
                    "sha256": q.digest(raw),
                }
            retirement = retired_permit_record(
                adapter, receipt["participant_permit_root"]
            )
            target = output_assets / f"{provider.lower()}-retired_permit.json"
            write_json(target, retirement)
            raw = target.read_bytes()
            retained["retired_permit"] = {
                "path": target.relative_to(args.output.parent).as_posix(),
                "bytes": len(raw),
                "sha256": q.digest(raw),
            }
            records.append(
                {
                    "provider_adapter": adapter,
                    "provider_organization": provider,
                    "model": model,
                    "held_neutral_run_id": run_id,
                    "qualification_receipt": receipt,
                    "retained": retained,
                    "consumed_neutral_permit_exists": False,
                    "provider_calls": 0,
                }
            )
        value = {
            "schema": "vela.lean-correspondence-stage-a-offline-runtime-qualification.v1",
            "status": "offline_qualified_hold",
            "qualifier": {
                "commit": QUALIFIER_COMMIT,
                "tree": QUALIFIER_TREE,
                "sha256": "sha256:" + QUALIFIER_SHA256,
            },
            "trust_bundle_sha256": digest(trust_raw),
            "provider_records": records,
            "provider_calls": 0,
            "neutral_calibrations_run": 0,
            "participant_calls": 0,
            "authority_effect": "none",
        }
        value["record_root"] = q.canonical_root(value)
        write_json(args.output, value)
        return 0
    run(
        args.repository.resolve(),
        args.workspace,
        args.output.resolve(),
        args.trust_bundle.resolve(),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
