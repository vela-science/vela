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
from pathlib import Path
from typing import Any

QUALIFIER_COMMIT = "586c305915f9f192822a720df7fd5abf416d9439"
QUALIFIER_TREE = "59c1847e8b4a8f57ba515febc487b0ce0e68c37f"
QUALIFIER_SHA256 = "6db638f5cec4df9eac53fe8edc2376fcc4db89afe3f08b977d47873669c41ddc"
SOURCE_DATE_EPOCH = 1_757_289_600
ADAPTERS = ("openai-responses-v1", "anthropic-messages-v1")
PROVIDERS = {
    "openai-responses-v1": ("OpenAI", "gpt-5.6-sol", "neutral-calibration-openai"),
    "anthropic-messages-v1": (
        "Anthropic",
        "claude-opus-5",
        "neutral-calibration-anthropic",
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
    staged = workspace / "maintained-qualifier/tools/evidence_qualification"
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
    project = workspace / "conformance"
    project.mkdir(parents=True, exist_ok=True)
    for name in ("pyproject.toml", "uv.lock"):
        write(project / name, (repository / "conformance" / name).read_bytes())
    subprocess.run(
        ["uv", "sync", "--project", str(project), "--locked", "--offline"],
        check=True,
        env={**os.environ, "UV_NO_PROGRESS": "1"},
    )
    return project / ".venv/bin/python"


def canonical_tar(files: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, raw in sorted(files.items()):
            info = tarfile.TarInfo(name)
            info.size = len(raw)
            info.mtime = SOURCE_DATE_EPOCH
            info.mode = 0o644
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(raw))
    return buffer.getvalue()


def provider_image(bundle: Path, adapter: str, q: Any) -> None:
    archive_path = bundle / "runtime/a.oci.tar"
    with tarfile.open(archive_path, mode="r") as archive:
        files = {
            member.name: archive.extractfile(member).read()
            for member in archive.getmembers()
            if member.isfile()
        }
    index = json.loads(files["index.json"])
    old_manifest = index["manifests"][0]["digest"]
    manifest_name = "blobs/sha256/" + old_manifest.removeprefix("sha256:")
    manifest = json.loads(files.pop(manifest_name))
    old_config = manifest["config"]["digest"]
    config_name = "blobs/sha256/" + old_config.removeprefix("sha256:")
    config = json.loads(files.pop(config_name))
    config["config"] = {
        "Labels": {
            "org.vela.provider-adapter": adapter,
            "org.vela.runtime-mode": "offline-qualification-held",
        }
    }
    config["created"] = "2026-08-21T00:00:00Z"
    config_raw = q.canonical_json_bytes(config)
    config_digest = q.digest(config_raw)
    files["blobs/sha256/" + config_digest.removeprefix("sha256:")] = config_raw
    layer_raw = ("provider-adapter=" + adapter + "\nnetwork=none\n").encode()
    layer_digest = q.digest(layer_raw)
    files["blobs/sha256/" + layer_digest.removeprefix("sha256:")] = layer_raw
    manifest["config"] = {
        "mediaType": "application/vnd.oci.image.config.v1+json",
        "digest": config_digest,
        "size": len(config_raw),
    }
    manifest["layers"].append(
        {
            "mediaType": "application/vnd.oci.image.layer.v1.tar",
            "digest": layer_digest,
            "size": len(layer_raw),
        }
    )
    manifest_raw = q.canonical_json_bytes(manifest)
    manifest_digest = q.digest(manifest_raw)
    files["blobs/sha256/" + manifest_digest.removeprefix("sha256:")] = manifest_raw
    index["manifests"] = [
        {
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "digest": manifest_digest,
            "size": len(manifest_raw),
        }
    ]
    files["index.json"] = q.canonical_json_bytes(index)
    raw = canonical_tar(files)
    for name in ("a.oci.tar", "b.oci.tar"):
        write(bundle / "runtime" / name, raw)
    layers = [item["digest"] for item in manifest["layers"]]
    for name in ("independent-a.json", "independent-b.json"):
        receipt_path = bundle / "runtime" / name
        receipt = load(receipt_path)
        receipt.update(
            {
                "source_date_epoch": SOURCE_DATE_EPOCH,
                "image_digest": manifest_digest,
                "config_digest": config_digest,
                "layer_digests": layers,
                "oci_tar_bytes": q.digest(raw),
            }
        )
        write_json(receipt_path, receipt)
    compatibility_path = bundle / "config/compatibility.json"
    compatibility = load(compatibility_path)
    compatibility["image_digest"] = manifest_digest
    write_json(compatibility_path, compatibility)
    for relative in (
        "permit/participant-run-01.permit.json",
        "fixture/permit/neutral-qualification-01.permit.template.json",
        "fixture/permit/neutral-qualification-01.permit.consumed.json",
    ):
        value = load(bundle / relative)
        value["image_digest"] = manifest_digest
        write_json(bundle / relative, value)
    consumed = bundle / "fixture/permit/neutral-qualification-01.permit.consumed.json"
    launch_path = bundle / "fixture/evidence/launch.json"
    launch = load(launch_path)
    launch.update(
        {
            "image_digest": manifest_digest,
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
            "prompt_root": q.digest((run_id + " prompt\n").encode()),
            "packet_root": q.digest((run_id + " packet\n").encode()),
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
    replace_trust(bundle, trust_raw, q)
    provider_image(bundle, adapter, q)
    tests.upgrade_to_tool_bundle(fixture, adapter)
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
    bundle = workspace / "stage-a-runtime-qualification-bundle"
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
        for label, relative in (
            ("image", "runtime/a.oci.tar"),
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
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trust-bundle", type=Path, required=True)
    parser.add_argument("--inside-fixed-environment", action="store_true")
    args = parser.parse_args()
    if args.inside_fixed_environment:
        # Avoid recopying over the module currently executing from the fixed path.
        sys.path.insert(0, str(args.workspace / "maintained-qualifier"))
        q = importlib.import_module("tools.evidence_qualification.qualification")
        tests = importlib.import_module(
            "tools.evidence_qualification.test_qualification"
        )
        modules = (q, tests)
        trust_raw = args.trust_bundle.read_bytes()
        bundle = args.workspace / "stage-a-runtime-qualification-bundle"
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
            for label, relative in (
                ("image", "runtime/a.oci.tar"),
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
