#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path, PurePosixPath

PACKAGE = Path(__file__).resolve().parent
REPO = PACKAGE.parents[2]
RUNTIME = PACKAGE.parent / "lean-correspondence-stage-a-runtime-qualification"
STAGE_A = PACKAGE.parent / "lean-correspondence-stage-a-open-pilot"
PRODUCER = "404adad5f03ccf22f0bcf46770dec59b868acc64"
PRODUCER_TREE = "ec90f9b6d51bcd48d78be7657e47a99764ffd9af"
EVIDENCE_PREDECESSOR = "157393175a5ca1085a8a83470ded3c2431465388"
EVIDENCE_PREDECESSOR_TREE = "20439ce60be3a25c4666445d4aadf4014fa887f1"
QUALIFIER = "cc3b88d8bfcfd7b4f720a023f049d5c365be9423"
QUALIFIER_TREE = "341e0d22fa570b1b5e8dd9f70b219c11308ba45f"
QUALIFIER_SHA256 = (
    "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013"
)
SHA256 = re.compile(r"sha256:[0-9a-f]{64}")
EXPECTED_FILES = frozenset(
    {
        "README.md",
        "endpoint-contact-receipt.json",
        "execution-build.json",
        "execution-sources/controller.py",
        "execution-sources/orchestrator.go",
        "execution-sources/runner_relay.go",
        "inputs/packet.json",
        "inputs/provider-schema.json",
        "inputs/run.json",
        "permit/neutral-calibration-anthropic-json-v2.permit.consumed.json",
        "raw/bridge.stderr",
        "raw/bridge.stdout",
        "raw/container.stderr",
        "raw/controller-attempt-terminal.json",
        "raw/docker.stderr",
        "raw/docker.stdout",
        "raw/permit-release.json",
        "raw/process-teardown.json",
        "seal.py",
        "terminal-outcome.json",
        "test_verify.py",
        "verify.py",
    }
)
EXPECTED_DIRECTORIES = frozenset({"execution-sources", "inputs", "permit", "raw"})


class VerificationError(RuntimeError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise VerificationError(code)


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def load(path: Path) -> object:
    return json.loads(path.read_bytes())


def exact(value: object, keys: set[str], code: str) -> dict[str, object]:
    require(type(value) is dict and set(value) == keys, code)
    return value


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO, check=True, capture_output=True, text=True
    ).stdout.strip()


def filesystem_inventory(root: Path) -> tuple[set[str], set[str]]:
    root_metadata = os.lstat(root)
    require(
        stat.S_ISDIR(root_metadata.st_mode) and not stat.S_ISLNK(root_metadata.st_mode),
        "artifact_root_type",
    )
    files: set[str] = set()
    directories: set[str] = set()
    pending = [root]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                target = Path(entry.path)
                relative = target.relative_to(root).as_posix()
                metadata = entry.stat(follow_symlinks=False)
                require(not stat.S_ISLNK(metadata.st_mode), "undeclared_symbolic_path")
                if stat.S_ISDIR(metadata.st_mode):
                    directories.add(relative)
                    pending.append(target)
                elif stat.S_ISREG(metadata.st_mode):
                    require(metadata.st_nlink == 1, "artifact_file_link_count")
                    files.add(relative)
                else:
                    raise VerificationError("undeclared_special_path")
    return files, directories


def seal_manifest(root: Path) -> dict[str, object]:
    files, directories = filesystem_inventory(root)
    require(
        files == EXPECTED_FILES | {"artifact-root.json"},
        "artifact_file_set",
    )
    require(directories == EXPECTED_DIRECTORIES, "artifact_directory_set")
    entries = []
    for path_value in sorted(EXPECTED_FILES):
        raw = (root / path_value).read_bytes()
        entries.append({"path": path_value, "bytes": len(raw), "sha256": digest(raw)})
    return {
        "schema": "vela.stage-a-anthropic-neutral-terminal-artifact.v1",
        "files": entries,
        "artifact_root": digest(canonical(entries)),
    }


def verify() -> dict[str, object]:
    files, directories = filesystem_inventory(PACKAGE)
    require(
        files == EXPECTED_FILES | {"artifact-root.json"},
        "artifact_file_set",
    )
    require(directories == EXPECTED_DIRECTORIES, "artifact_directory_set")
    manifest = exact(
        load(PACKAGE / "artifact-root.json"),
        {"schema", "files", "artifact_root"},
        "manifest_shape",
    )
    require(
        manifest["schema"] == "vela.stage-a-anthropic-neutral-terminal-artifact.v1",
        "manifest_schema",
    )
    entries = manifest["files"]
    require(type(entries) is list and entries, "manifest_files")
    seen: set[str] = set()
    normalized = []
    for entry_value in entries:
        entry = exact(entry_value, {"path", "bytes", "sha256"}, "manifest_entry_shape")
        path_value = entry["path"]
        require(type(path_value) is str and path_value not in seen, "manifest_path")
        logical = PurePosixPath(path_value)
        require(
            not logical.is_absolute()
            and ".." not in logical.parts
            and logical.as_posix() == path_value,
            "manifest_path_escape",
        )
        target = PACKAGE / path_value
        require(target.is_file() and not target.is_symlink(), "manifest_file_absent")
        raw = target.read_bytes()
        require(
            type(entry["bytes"]) is int
            and not isinstance(entry["bytes"], bool)
            and entry["bytes"] == len(raw),
            "manifest_size",
        )
        require(entry["sha256"] == digest(raw), "manifest_digest")
        seen.add(path_value)
        normalized.append(entry)
    require([entry["path"] for entry in entries] == sorted(seen), "manifest_order")
    require(seen == EXPECTED_FILES, "manifest_file_set")
    require(manifest["artifact_root"] == digest(canonical(normalized)), "artifact_root")
    require(manifest == seal_manifest(PACKAGE), "manifest_reseal")

    outcome = exact(
        load(PACKAGE / "terminal-outcome.json"),
        {
            "schema",
            "status",
            "failure_code",
            "failure_explanation",
            "producer",
            "qualifier",
            "attempt",
            "bindings",
            "custody",
            "credential_teardown",
            "teardown",
            "stopped_state",
        },
        "outcome_shape",
    )
    require(
        outcome["schema"] == "vela.stage-a-anthropic-neutral-terminal-evidence.v1",
        "outcome_schema",
    )
    require(
        outcome["status"] == "failed_pre_provider_request_terminal"
        and outcome["failure_code"] == "provider_schema_mount_binding_invalid",
        "terminal_status",
    )
    producer = exact(
        outcome["producer"],
        {
            "commit",
            "tree",
            "branch",
            "runtime_artifact_root",
            "runtime_registration_root",
            "offline_qualification_root",
        },
        "producer_shape",
    )
    require(
        producer["commit"] == PRODUCER and producer["tree"] == PRODUCER_TREE,
        "producer_binding",
    )
    require(
        git("rev-parse", f"{PRODUCER}^{{tree}}") == PRODUCER_TREE, "producer_git_tree"
    )
    require(
        git("rev-parse", f"{EVIDENCE_PREDECESSOR}^{{tree}}")
        == EVIDENCE_PREDECESSOR_TREE,
        "evidence_predecessor_tree",
    )
    ancestry = subprocess.run(
        ["git", "merge-base", "--is-ancestor", EVIDENCE_PREDECESSOR, "HEAD"],
        cwd=REPO,
        check=False,
    )
    require(ancestry.returncode == 0, "evidence_predecessor_ancestry")
    qualifier = exact(
        outcome["qualifier"], {"commit", "tree", "sha256"}, "qualifier_shape"
    )
    require(
        qualifier
        == {"commit": QUALIFIER, "tree": QUALIFIER_TREE, "sha256": QUALIFIER_SHA256},
        "qualifier_binding",
    )
    require(
        git("rev-parse", f"{QUALIFIER}^{{tree}}") == QUALIFIER_TREE, "qualifier_tree"
    )
    qualifier_raw = subprocess.run(
        ["git", "show", f"{QUALIFIER}:tools/evidence_qualification/qualification.py"],
        cwd=REPO,
        check=True,
        capture_output=True,
    ).stdout
    require(digest(qualifier_raw) == QUALIFIER_SHA256, "qualifier_bytes")

    attempt = exact(
        outcome["attempt"],
        {
            "run_id",
            "attempt",
            "retries",
            "permit_root",
            "permit_consumed",
            "permit_consumed_path",
            "provider_calls",
            "provider_request_constructed",
            "provider_endpoint_contacted",
            "participant_calls",
            "openai_neutral_permit_released",
            "participant_permits_released",
        },
        "attempt_shape",
    )
    require(
        attempt
        == {
            "run_id": "neutral-calibration-anthropic-json-v2",
            "attempt": 1,
            "retries": 0,
            "permit_root": "sha256:b9ba39cf1c511043324ca8dfbc02b6c59d91f457a2e560a37d78d32a1b84cdbe",
            "permit_consumed": True,
            "permit_consumed_path": "permit/neutral-calibration-anthropic-json-v2.permit.consumed.json",
            "provider_calls": 0,
            "provider_request_constructed": False,
            "provider_endpoint_contacted": False,
            "participant_calls": 0,
            "openai_neutral_permit_released": False,
            "participant_permits_released": 0,
        },
        "attempt_state",
    )
    consumed = (PACKAGE / attempt["permit_consumed_path"]).read_bytes()
    held = (
        RUNTIME / "offline-qualification-assets/anthropic-held_permit.json"
    ).read_bytes()
    require(consumed == held, "consumed_permit_bytes")
    require(
        digest(canonical(json.loads(consumed))) == attempt["permit_root"],
        "consumed_permit_root",
    )
    release = exact(
        load(PACKAGE / "raw/permit-release.json"),
        {
            "schema",
            "attempt",
            "consumed_path",
            "permit_root",
            "released_at",
            "run_id",
            "source_state",
            "zero_retries",
        },
        "release_shape",
    )
    require(
        release["run_id"] == attempt["run_id"]
        and release["permit_root"] == attempt["permit_root"]
        and release["attempt"] == 1
        and release["zero_retries"] is True,
        "release_binding",
    )

    bindings = exact(
        outcome["bindings"],
        {
            "image_digest",
            "oci_archive_sha256",
            "runtime_source_root",
            "packet_root",
            "prompt_root",
            "content_equivalence_root",
            "mounted_provider_schema_sha256",
            "embedded_provider_schema_sha256",
            "provider_schema_semantically_equal",
            "provider_schema_byte_equal",
            "trust_bundle_sha256",
            "participant_network",
            "host_bridge_endpoint",
            "credential_fd",
        },
        "bindings_shape",
    )
    require(
        bindings["image_digest"]
        == "sha256:26fa80f822ebc0357670e03b4358d01d8c2190803696b7fd8aefec83e3e84fcf"
        and bindings["participant_network"] == "none"
        and bindings["credential_fd"] == 4,
        "runtime_binding",
    )
    require(
        digest(
            (RUNTIME / "offline-qualification-assets/anthropic-image.tar").read_bytes()
        )
        == bindings["oci_archive_sha256"],
        "oci_bytes",
    )
    packet = (PACKAGE / "inputs/packet.json").read_bytes()
    prompt = (RUNTIME / "neutral-calibration/prompt.txt").read_bytes()
    schema = (PACKAGE / "inputs/provider-schema.json").read_bytes()
    require(
        digest(packet) == bindings["packet_root"]
        and digest(prompt) == bindings["prompt_root"]
        and digest(schema) == bindings["mounted_provider_schema_sha256"],
        "neutral_input_roots",
    )
    run_raw = (PACKAGE / "inputs/run.json").read_bytes()
    run = json.loads(run_raw)
    require(
        run["prompt"] == prompt.decode()
        and run["packet_sha256"] == bindings["packet_root"]
        and run["packet_bytes"] == len(packet),
        "run_neutral_binding",
    )
    marker = b'  "provider_schema": '
    start = run_raw.index(marker) + len(marker)
    end = run_raw.index(b',\n  "run_id":', start)
    embedded = run_raw[start:end]
    require(
        json.loads(embedded) == json.loads(schema) and embedded != schema.strip(),
        "schema_failure_reproduction",
    )
    require(
        digest(embedded) == bindings["embedded_provider_schema_sha256"]
        and bindings["provider_schema_semantically_equal"] is True
        and bindings["provider_schema_byte_equal"] is False,
        "schema_failure_roots",
    )

    custody = exact(
        outcome["custody"],
        {
            "run_input_path",
            "run_input_sha256",
            "provider_request_path",
            "provider_events_path",
            "tool_requests",
            "tool_results",
            "response_path",
            "usage_path",
            "terminal_response_path",
            "container_stderr_path",
            "container_stderr_sha256",
            "bridge_stdout_sha256",
            "bridge_stderr_sha256",
            "docker_stdout_sha256",
            "docker_stderr_sha256",
            "endpoint_contact_receipt_path",
            "controller_raw_terminal_path",
            "controller_raw_terminal_authoritative",
            "controller_raw_terminal_defect",
            "controller_raw_claimed_provider_calls",
        },
        "custody_shape",
    )
    require(custody["run_input_sha256"] == digest(run_raw), "run_input_custody")
    require(
        all(
            custody[key] is None
            for key in (
                "provider_request_path",
                "provider_events_path",
                "response_path",
                "usage_path",
                "terminal_response_path",
            )
        ),
        "provider_custody_absence",
    )
    require(
        custody["tool_requests"] == 0 and custody["tool_results"] == 0, "tool_counters"
    )
    require(
        (PACKAGE / custody["container_stderr_path"]).read_bytes()
        == b"provider schema mount binding invalid\n",
        "failure_stderr",
    )
    require(
        custody["controller_raw_terminal_authoritative"] is False
        and custody["controller_raw_claimed_provider_calls"] == 1,
        "raw_controller_defect",
    )
    raw_controller = load(PACKAGE / custody["controller_raw_terminal_path"])
    require(
        raw_controller["provider_calls"] == 1
        and raw_controller["status"] == "failed_terminal",
        "raw_controller_retention",
    )
    endpoint = exact(
        load(PACKAGE / custody["endpoint_contact_receipt_path"]),
        {
            "schema",
            "run_id",
            "authoritative",
            "provider_request_constructed",
            "bridge_provider_request_frame_received",
            "endpoint_write_attempted",
            "endpoint_contacted",
            "provider_calls",
            "raw_controller_terminal_authoritative",
            "raw_controller_terminal_path",
            "derivation",
        },
        "endpoint_receipt_shape",
    )
    require(
        endpoint["schema"] == "vela.stage-a-anthropic-neutral-endpoint-contact.v1"
        and endpoint["run_id"] == attempt["run_id"]
        and endpoint["authoritative"] is True
        and endpoint["provider_request_constructed"] is False
        and endpoint["bridge_provider_request_frame_received"] is False
        and endpoint["endpoint_write_attempted"] is False
        and endpoint["endpoint_contacted"] is False
        and endpoint["provider_calls"] == 0
        and endpoint["raw_controller_terminal_authoritative"] is False
        and endpoint["raw_controller_terminal_path"]
        == custody["controller_raw_terminal_path"],
        "endpoint_receipt",
    )

    teardown = exact(
        outcome["teardown"],
        {
            "participant_container_active",
            "host_bridge_active",
            "controller_active",
            "process_receipt_path",
        },
        "teardown_shape",
    )
    require(
        teardown["participant_container_active"] is False
        and teardown["host_bridge_active"] is False
        and teardown["controller_active"] is False,
        "teardown_state",
    )
    process = load(PACKAGE / teardown["process_receipt_path"])
    require(
        process["credential_fd_closed"] is True
        and process["credential_retained"] is False
        and process["participant_network"] == "none",
        "process_teardown",
    )
    credential = exact(
        outcome["credential_teardown"],
        {
            "regular_file",
            "owner_current_user",
            "mode",
            "link_count",
            "all_path_components_non_symlink",
            "acl",
            "xattrs",
            "credential_fd_closed",
            "credential_retained",
            "credential_bytes_in_evidence",
        },
        "credential_shape",
    )
    require(
        credential["regular_file"] is True
        and credential["owner_current_user"] is True
        and credential["mode"] == "0600"
        and credential["link_count"] == 1
        and credential["all_path_components_non_symlink"] is True
        and credential["acl"] == "empty"
        and credential["xattrs"] in ([], ["com.apple.provenance"])
        and credential["credential_fd_closed"] is True
        and credential["credential_retained"] is False
        and credential["credential_bytes_in_evidence"] is False,
        "credential_teardown",
    )
    stopped = exact(
        outcome["stopped_state"],
        {
            "participant_permits_held",
            "participant_responses",
            "scoring_attempts",
            "protected_keys_opened",
            "stage_b_families_selected",
            "authority_effect",
            "decision",
            "standing",
        },
        "stopped_shape",
    )
    require(
        stopped
        == {
            "participant_permits_held": 12,
            "participant_responses": 0,
            "scoring_attempts": 0,
            "protected_keys_opened": 0,
            "stage_b_families_selected": 0,
            "authority_effect": "none",
            "decision": None,
            "standing": None,
        },
        "stopped_state",
    )
    participant_permits = list((STAGE_A / "permits").glob("*.permit.json"))
    require(
        len(participant_permits) == 12
        and all(load(path)["status"] == "held" for path in participant_permits),
        "participant_permits",
    )
    openai = load(RUNTIME / "offline-qualification-assets/openai-held_permit.json")
    require(
        openai["status"] == "held" and openai["consumed_at"] is None, "openai_permit"
    )

    forbidden = (
        b"sk-" + b"ant-",
        b"x-api" + b"-key",
        b"Authorization: " + b"Bearer",
        b"ANTHROPIC_API" + b"_KEY=",
    )
    for path in PACKAGE.rglob("*"):
        if path.is_file():
            raw = path.read_bytes()
            require(
                not any(token in raw for token in forbidden), "credential_shaped_bytes"
            )
    return {
        "status": "PASS_FAILED_PRE_PROVIDER_REQUEST_TERMINAL",
        "artifact_root": manifest["artifact_root"],
        "attempts": 1,
        "retries": 0,
        "provider_calls": 0,
        "anthropic_neutral_permit_consumed": True,
        "openai_neutral_permit_held": True,
        "participant_permits_held": 12,
        "authority_effect": "none",
    }


if __name__ == "__main__":
    try:
        print(json.dumps(verify(), indent=2, sort_keys=True))
    except (
        VerificationError,
        OSError,
        ValueError,
        KeyError,
        subprocess.CalledProcessError,
    ) as error:
        raise SystemExit(f"FAIL: {error}")
