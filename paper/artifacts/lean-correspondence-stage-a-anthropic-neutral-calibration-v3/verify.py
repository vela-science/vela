#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
from pathlib import Path, PurePosixPath

PACKAGE = Path(__file__).resolve().parent
REPO = PACKAGE.parents[2]
RUNTIME = PACKAGE.parent / "lean-correspondence-stage-a-runtime-qualification"
STAGE_A = PACKAGE.parent / "lean-correspondence-stage-a-open-pilot"
STOPPED = PACKAGE.parent / "lean-correspondence-stage-a-anthropic-neutral-calibration"
PRODUCER = "2f99d225a5a3e675e32264b4398fa346d9c3bf97"
PRODUCER_TREE = "63c18f5812d9c4add3590ac798064ac207ac9d93"
RUN_ID = "neutral-calibration-anthropic-json-v3-replacement"
PERMIT_ROOT = "sha256:7ddf24c9dbeac2cdce1a4ca1972a0984287dbcf528881ae01cbfe297217e2f32"
PACKET_ROOT = "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
SCHEMA_ROOT = "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
RUN_ROOT = "sha256:efade9842484fe6e96a7e6fe4ced922b1b4da497351237208d6d45927861fc3d"
REQUEST_ROOT = "sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c"
RESPONSE_ROOT = (
    "sha256:00e3175172e7d4659400a599d686ae22c2ff0d50ba179516268557b18fb5abc6"
)
PROVIDER_RESPONSE_ROOT = (
    "sha256:0b5eafca1b2572c606bb462a57da53a94882eda6b2a62cc13b021afeb52faa44"
)
EXPECTED_FILES = frozenset(
    {
        "README.md",
        "execution-build.json",
        "seal.py",
        "test_verify.py",
        "verify.py",
        "execution-sources/controller.py",
        "execution-sources/orchestrator.go",
        "execution-sources/runner_relay.go",
        "inputs/expected-request.json",
        "inputs/materialization-receipt.json",
        "inputs/offline-validation-receipt.json",
        "inputs/packet.json",
        "inputs/provider-schema.json",
        "inputs/run.json",
        "permit/neutral-calibration-anthropic-json-v3-replacement.permit.consumed.json",
        "raw/attempt-terminal.json",
        "raw/bridge-to-runner.raw.jsonl",
        "raw/bridge.stderr",
        "raw/bridge.stdout",
        "raw/container.stderr",
        "raw/credential-nonretention.json",
        "raw/endpoint-contact-receipt.json",
        "raw/orchestrator.stderr",
        "raw/orchestrator.stdout",
        "raw/packet-custody.json",
        "raw/permit-release.json",
        "raw/process-teardown.json",
        "raw/provider-events.raw.jsonl",
        "raw/provider-response-0001.raw.json",
        "raw/provider-usage-0001.json",
        "raw/request.raw.json",
        "raw/response.raw.json",
        "raw/runner-to-bridge.raw.jsonl",
        "raw/terminal.json",
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


def exact_int(value: object, expected: int, code: str) -> None:
    require(type(value) is int and value == expected, code)


def inventory(root: Path) -> tuple[set[str], set[str]]:
    meta = os.lstat(root)
    require(
        stat.S_ISDIR(meta.st_mode) and not stat.S_ISLNK(meta.st_mode),
        "artifact_root_type",
    )
    files: set[str] = set()
    directories: set[str] = set()
    pending = [root]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                relative = Path(entry.path).relative_to(root).as_posix()
                meta = entry.stat(follow_symlinks=False)
                require(not stat.S_ISLNK(meta.st_mode), "symbolic_path")
                if stat.S_ISDIR(meta.st_mode):
                    directories.add(relative)
                    pending.append(Path(entry.path))
                elif stat.S_ISREG(meta.st_mode):
                    require(meta.st_nlink == 1, "file_link_count")
                    files.add(relative)
                else:
                    raise VerificationError("special_path")
    return files, directories


def seal_manifest(root: Path) -> dict[str, object]:
    files, directories = inventory(root)
    require(files == EXPECTED_FILES | {"artifact-root.json"}, "artifact_file_set")
    require(directories == EXPECTED_DIRECTORIES, "artifact_directory_set")
    entries = []
    for path in sorted(EXPECTED_FILES):
        raw = (root / path).read_bytes()
        entries.append({"path": path, "bytes": len(raw), "sha256": digest(raw)})
    return {
        "schema": "vela.stage-a-anthropic-neutral-v3-artifact.v1",
        "files": entries,
        "artifact_root": digest(canonical(entries)),
    }


def verify_response(response: object) -> None:
    value = exact(
        response,
        {
            "schema",
            "assignment_id",
            "relation_validation",
            "change_classification",
            "impact_closure",
            "authority_scientific_inference",
            "uncertainty",
        },
        "response_shape",
    )
    require(
        value["schema"] == "lean-correspondence.review-response.v1", "response_schema"
    )
    require(value["assignment_id"] == "lc-neutral-calibration", "response_assignment")
    require(
        value["relation_validation"] in {"valid", "invalid", "cannot_determine"},
        "response_relation",
    )
    require(
        value["change_classification"]
        in {"semantic_change", "environment_drift", "both", "neither", "unprovable"},
        "response_change",
    )
    impact = value["impact_closure"]
    require(type(impact) is list, "response_impact")
    for item_value in impact:
        item = exact(
            item_value,
            {"item_id", "disposition", "evidence_ids"},
            "response_impact_shape",
        )
        require(
            type(item["item_id"]) is str and bool(item["item_id"]), "response_impact_id"
        )
        require(
            item["disposition"]
            in {
                "recheck",
                "invalidate_relation",
                "remains_valid",
                "blocked_unprovable",
            },
            "response_disposition",
        )
        require(
            type(item["evidence_ids"]) is list
            and all(type(x) is str and x for x in item["evidence_ids"]),
            "response_evidence_ids",
        )
    authority = exact(
        value["authority_scientific_inference"],
        {"repository_authority_effect", "scientific_status"},
        "response_authority_shape",
    )
    require(authority["repository_authority_effect"] == "none", "response_authority")
    require(authority["scientific_status"] == "not_established", "response_scientific")
    require(
        type(value["uncertainty"]) is list
        and all(type(x) is str and x for x in value["uncertainty"]),
        "response_uncertainty",
    )


def verify() -> dict[str, object]:
    files, directories = inventory(PACKAGE)
    require(files == EXPECTED_FILES | {"artifact-root.json"}, "artifact_file_set")
    require(directories == EXPECTED_DIRECTORIES, "artifact_directory_set")
    manifest = exact(
        load(PACKAGE / "artifact-root.json"),
        {"schema", "files", "artifact_root"},
        "manifest_shape",
    )
    require(manifest == seal_manifest(PACKAGE), "manifest_reseal")
    require(
        manifest["schema"] == "vela.stage-a-anthropic-neutral-v3-artifact.v1",
        "manifest_schema",
    )
    entries = manifest["files"]
    require(
        type(entries) is list
        and [x["path"] for x in entries] == sorted(EXPECTED_FILES),
        "manifest_paths",
    )
    for entry_value in entries:
        entry = exact(entry_value, {"path", "bytes", "sha256"}, "manifest_entry")
        path = entry["path"]
        require(
            type(path) is str
            and PurePosixPath(path).as_posix() == path
            and ".." not in PurePosixPath(path).parts,
            "manifest_path",
        )
        raw = (PACKAGE / path).read_bytes()
        exact_int(entry["bytes"], len(raw), "manifest_bytes")
        require(entry["sha256"] == digest(raw), "manifest_digest")

    build = exact(
        load(PACKAGE / "execution-build.json"),
        {
            "schema",
            "producer",
            "runtime",
            "sources",
            "binaries",
            "stopped_v2",
            "stopped_state",
        },
        "build_shape",
    )
    require(
        build["schema"] == "vela.stage-a-anthropic-neutral-v3-execution-build.v1",
        "build_schema",
    )
    producer = exact(
        build["producer"],
        {
            "commit",
            "tree",
            "branch",
            "artifact_root",
            "registration_root",
            "offline_qualification_root",
        },
        "producer_shape",
    )
    require(
        producer["commit"] == PRODUCER and producer["tree"] == PRODUCER_TREE,
        "producer_binding",
    )
    require(
        subprocess.run(
            ["git", "rev-parse", f"{PRODUCER}^{{tree}}"],
            cwd=REPO,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        == PRODUCER_TREE,
        "producer_tree",
    )
    require(
        load(RUNTIME / "artifact-root.json")["artifact_root"]
        == producer["artifact_root"],
        "runtime_artifact_root",
    )
    require(
        load(RUNTIME / "offline-qualification.json")["record_root"]
        == producer["offline_qualification_root"],
        "offline_root",
    )
    registration = load(RUNTIME / "registration.json")
    require(
        digest(canonical(registration)) == producer["registration_root"],
        "registration_root",
    )
    require(
        load(STAGE_A / "artifact-manifest.json")["artifact_root"]
        == "sha256:f89d335912adbbd0e3b3f1cb98ec3f4fa78a27f3742652ac7244eaa86ed6aca8",
        "stage_a_root",
    )
    require(
        load(STOPPED / "artifact-root.json")["artifact_root"]
        == build["stopped_v2"]["artifact_root"],
        "stopped_v2_root",
    )
    require(
        build["stopped_v2"]
        == {
            "artifact_root": "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b",
            "permanent_consumed_non_call": True,
            "provider_calls": 0,
        },
        "stopped_v2",
    )

    for name, expected in build["sources"].items():
        require(
            digest((PACKAGE / "execution-sources" / name).read_bytes()) == expected,
            "execution_source",
        )
    runtime = build["runtime"]
    require(
        runtime["image_digest"]
        == "sha256:d314adbd8b3765d9aada03bd5bd87ec77826cd81fdc8c8aab3982dce3165385d",
        "image_digest",
    )
    require(
        runtime["qualifier_sha256"]
        == "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013",
        "qualifier",
    )

    permit_path = (
        PACKAGE
        / "permit/neutral-calibration-anthropic-json-v3-replacement.permit.consumed.json"
    )
    permit_raw = permit_path.read_bytes()
    held_raw = subprocess.run(
        [
            "git",
            "show",
            f"{PRODUCER}:paper/artifacts/lean-correspondence-stage-a-runtime-qualification/offline-qualification-assets/anthropic-held_permit.json",
        ],
        cwd=REPO,
        check=True,
        capture_output=True,
    ).stdout
    require(permit_raw == held_raw, "consumed_permit_exact_held_bytes")
    permit = load(permit_path)
    require(
        permit["run_id"] == RUN_ID
        and permit["status"] == "held"
        and permit["consumed_at"] is None,
        "consumed_permit_identity",
    )
    require(digest(canonical(permit)) == PERMIT_ROOT, "permit_root")

    for name, expected in {
        "packet.json": PACKET_ROOT,
        "provider-schema.json": SCHEMA_ROOT,
        "run.json": RUN_ROOT,
        "expected-request.json": REQUEST_ROOT,
    }.items():
        require(
            digest((PACKAGE / "inputs" / name).read_bytes()) == expected, "input_root"
        )
        retained = {
            "packet.json": "anthropic-neutral_packet.json",
            "provider-schema.json": "anthropic-provider_schema.json",
            "run.json": "anthropic-run_input.json",
            "expected-request.json": "anthropic-request_bytes.json",
        }[name]
        require(
            (PACKAGE / "inputs" / name).read_bytes()
            == (RUNTIME / "offline-qualification-assets" / retained).read_bytes(),
            "input_runtime_binding",
        )
    require(
        (PACKAGE / "raw/request.raw.json").read_bytes()
        == (PACKAGE / "inputs/expected-request.json").read_bytes(),
        "request_exact",
    )
    require(
        digest((PACKAGE / "raw/response.raw.json").read_bytes()) == RESPONSE_ROOT,
        "response_root",
    )
    require(
        digest((PACKAGE / "raw/provider-response-0001.raw.json").read_bytes())
        == PROVIDER_RESPONSE_ROOT,
        "provider_response_root",
    )
    verify_response(load(PACKAGE / "raw/response.raw.json"))

    offline = load(PACKAGE / "inputs/offline-validation-receipt.json")
    require(
        offline
        == load(
            RUNTIME
            / "offline-qualification-assets/anthropic-offline_validation_receipt.json"
        ),
        "offline_receipt",
    )
    require(
        offline["status"] == "pass"
        and offline["endpoint_contact_forbidden"] is True
        and offline["provider_calls"] == 0,
        "offline_pass",
    )
    material = load(PACKAGE / "inputs/materialization-receipt.json")
    require(
        material
        == load(
            RUNTIME
            / "offline-qualification-assets/anthropic-materialization_receipt.json"
        ),
        "materialization_receipt",
    )
    require(
        material["raw_inserted_sha256"] == SCHEMA_ROOT
        and material["parse_reserialization_used"] is False,
        "raw_schema_splice",
    )

    terminal = exact(
        load(PACKAGE / "raw/terminal.json"),
        {
            "schema",
            "run_id",
            "adapter",
            "status",
            "provider_calls",
            "credential_retained",
            "packet_sha256",
            "request_sha256",
            "response_sha256",
        },
        "terminal_shape",
    )
    require(
        terminal["run_id"] == RUN_ID and terminal["status"] == "completed",
        "terminal_status",
    )
    exact_int(terminal["provider_calls"], 1, "terminal_calls")
    require(
        terminal["credential_retained"] is False
        and terminal["packet_sha256"] == PACKET_ROOT
        and terminal["request_sha256"] == REQUEST_ROOT
        and terminal["response_sha256"] == RESPONSE_ROOT,
        "terminal_bindings",
    )
    endpoint = load(PACKAGE / "raw/endpoint-contact-receipt.json")
    attempt = load(PACKAGE / "raw/attempt-terminal.json")
    exact_int(endpoint["provider_calls"], 1, "endpoint_calls")
    require(
        endpoint["endpoint_attempt_receipts"]
        == [{"provider_calls": 1, "type": "endpoint_attempt"}],
        "endpoint_receipts",
    )
    require(
        attempt["status"] == "completed"
        and attempt["run_id"] == RUN_ID
        and attempt["permit_root"] == PERMIT_ROOT,
        "attempt_terminal",
    )
    for key in [
        "provider_calls",
        "bridge_provider_calls",
        "runner_provider_calls",
        "terminal_provider_calls",
        "custody_provider_calls",
    ]:
        exact_int(attempt[key], 1, "cross_layer_calls")
    exact_int(attempt["attempt"], 1, "attempt_count")
    exact_int(attempt["retries"], 0, "retry_count")
    require(
        attempt["endpoint_attempt_receipts"] == endpoint["endpoint_attempt_receipts"],
        "attempt_endpoint_binding",
    )
    exact_int(attempt["participant_permits_released"], 0, "participant_permits")
    require(attempt["openai_neutral_permit_released"] is False, "openai_permit")
    exact_int(attempt["scoring_attempts"], 0, "scoring")
    exact_int(attempt["stage_b_families_selected"], 0, "stage_b")
    require(attempt["authority_effect"] == "none", "authority")

    frames = [
        json.loads(line)
        for line in (PACKAGE / "raw/bridge-to-runner.raw.jsonl")
        .read_text()
        .splitlines()
    ]
    require(
        (PACKAGE / "raw/bridge-to-runner.raw.jsonl").read_bytes()
        == (PACKAGE / "raw/provider-events.raw.jsonl").read_bytes(),
        "provider_event_custody",
    )
    require(
        len(frames) == 3
        and frames[0] == {"type": "endpoint_attempt", "provider_calls": 1}
        and frames[2]["type"] == "terminal"
        and frames[2]["provider_calls"] == 1,
        "provider_frames",
    )
    runner_frames = [
        json.loads(line)
        for line in (PACKAGE / "raw/runner-to-bridge.raw.jsonl")
        .read_text()
        .splitlines()
    ]
    require(
        len(runner_frames) == 1
        and runner_frames[0]["type"] == "provider_request"
        and runner_frames[0]["endpoint"] == "https://api.anthropic.com/v1/messages",
        "request_frame",
    )
    require(
        runner_frames[0]["body"] == load(PACKAGE / "raw/request.raw.json"),
        "request_frame_binding",
    )

    usage = load(PACKAGE / "raw/provider-usage-0001.json")
    require(
        usage["provider_response_sha256"] == PROVIDER_RESPONSE_ROOT
        and usage["response_ordinal"] == 1
        and type(usage["usage"]) is dict,
        "usage_binding",
    )
    teardown = load(PACKAGE / "raw/process-teardown.json")
    require(
        teardown["status"] == "completed"
        and teardown["credential_fd_closed"] is True
        and teardown["credential_retained"] is False
        and teardown["participant_network"] == "none",
        "teardown",
    )
    require(
        all(
            type(x["exit_code"]) is int and x["exit_code"] == 0
            for x in teardown["children"]
        ),
        "child_teardown",
    )
    credential = load(PACKAGE / "raw/credential-nonretention.json")
    require(
        credential["credential_buffer_scrubbed"] is True
        and credential["credential_fd_closed"] is True
        and credential["credential_retained"] is False
        and credential["environment_injection"] is False
        and credential["evidence_scan_no_credential_bytes"] is True
        and credential["source_metadata_stable"] is True,
        "credential_nonretention",
    )
    release = load(PACKAGE / "raw/permit-release.json")
    require(
        release["run_id"] == RUN_ID
        and release["permit_root"] == PERMIT_ROOT
        and release["source_state"] == "held"
        and release["zero_retries"] is True,
        "permit_release",
    )
    exact_int(release["attempt"], 1, "permit_attempt")
    for path in [
        "raw/bridge.stderr",
        "raw/bridge.stdout",
        "raw/container.stderr",
        "raw/orchestrator.stderr",
        "raw/orchestrator.stdout",
    ]:
        require((PACKAGE / path).read_bytes() == b"", "unexpected_process_output")
    stopped = exact(
        build["stopped_state"],
        {
            "authority_effect",
            "openai_neutral_permit_released",
            "participant_calls",
            "participant_permits_released",
            "scoring_attempts",
            "stage_b_families_selected",
        },
        "stopped_state_shape",
    )
    require(
        stopped
        == {
            "authority_effect": "none",
            "openai_neutral_permit_released": False,
            "participant_calls": 0,
            "participant_permits_released": 0,
            "scoring_attempts": 0,
            "stage_b_families_selected": 0,
        },
        "stopped_state",
    )
    return {
        "status": "PASS",
        "artifact_root": manifest["artifact_root"],
        "provider_calls": 1,
        "retries": 0,
        "response_root": RESPONSE_ROOT,
    }


if __name__ == "__main__":
    try:
        print(json.dumps(verify(), sort_keys=True))
    except (
        VerificationError,
        KeyError,
        TypeError,
        ValueError,
        json.JSONDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(json.dumps({"status": "FAIL", "error": str(error)}, sort_keys=True))
        raise SystemExit(1)
