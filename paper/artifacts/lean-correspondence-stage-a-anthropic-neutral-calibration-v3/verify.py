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
ACTUAL_REQUEST_ROOT = (
    "sha256:2f151e66b7bfcb1cab61c0ae37386180c4fae3bd9a8278535af2621ffba4e63a"
)
RESPONSE_ROOT = (
    "sha256:00e3175172e7d4659400a599d686ae22c2ff0d50ba179516268557b18fb5abc6"
)
PROVIDER_RESPONSE_ROOT = (
    "sha256:0b5eafca1b2572c606bb462a57da53a94882eda6b2a62cc13b021afeb52faa44"
)
RAW_DIGESTS = {
    "attempt-terminal.json": "sha256:619bd27be1c8092fb772ff06648b14d4178086ec0104885d2e70fa51951f1dc3",
    "bridge-to-runner.raw.jsonl": "sha256:bac188c1dc7e5170def6ce21a5a9ce915dabfd9de4200325f77d369fafb1bbb7",
    "bridge.stderr": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "bridge.stdout": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "container.stderr": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "credential-nonretention.json": "sha256:db7ed3d2496565f2618d7c9948559a4bdf3690f381ad5ecf49f1c5b043b8f754",
    "endpoint-contact-receipt.json": "sha256:e0615bd59e62a73694e9d48ae02b650b7d699d6d4bf6edd7058874fe4c5623a7",
    "orchestrator.stderr": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "orchestrator.stdout": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "packet-custody.json": "sha256:addd9b8641a13227d387bbc4590b7b5e2a5df7be1e3691a1f4184d2b686bf124",
    "permit-release.json": "sha256:f8631f009d9bf6f4a37ddb462560b6f66e66c0445a28c89549a1baa349f7a63f",
    "process-teardown.json": "sha256:fab9f127ee884b25201bfff670d7e1bd3f8a2706d1ebbb134cf6e107aa9b6006",
    "provider-events.raw.jsonl": "sha256:bac188c1dc7e5170def6ce21a5a9ce915dabfd9de4200325f77d369fafb1bbb7",
    "provider-response-0001.raw.json": PROVIDER_RESPONSE_ROOT,
    "provider-usage-0001.json": "sha256:a006c593bf92e483b3de98fc348bc37b39953586ee5960b14ad5048158b561cf",
    "request.raw.json": REQUEST_ROOT,
    "response.raw.json": RESPONSE_ROOT,
    "runner-to-bridge.raw.jsonl": "sha256:3f7a01bf0cdd5453b3f9fa230033811f64b6b0c9b6f6fa373d17663ab29ae040",
    "terminal.json": "sha256:95a01102560a14c3e49d0a03cfe9ef4671c7cbdd8d3728a58188419c1741ea30",
}
EXPECTED_FILES = frozenset(
    {
        "README.md",
        "execution-build.json",
        "extract_actual_request.py",
        "seal.py",
        "test_verify.py",
        "terminal-outcome.json",
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
        "raw/actual-transmitted-body.raw.json",
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


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        require(key not in value, "duplicate_json_field")
        value[key] = item
    return value


def parse_json(raw: bytes | str) -> object:
    return json.loads(raw, object_pairs_hook=reject_duplicates)


def load(path: Path) -> object:
    return parse_json(path.read_bytes())


def load_jsonl(path: Path) -> list[object]:
    raw = path.read_bytes()
    require(raw.endswith(b"\n") and b"\r" not in raw, "jsonl_termination")
    return [parse_json(line) for line in raw.splitlines()]


def exact(value: object, keys: set[str], code: str) -> dict[str, object]:
    require(type(value) is dict and set(value) == keys, code)
    return value


def exact_int(value: object, expected: int, code: str) -> None:
    require(type(value) is int and value == expected, code)


def nonnegative_int(value: object, code: str) -> None:
    require(type(value) is int and value >= 0, code)


def validate_usage(value: object, code: str) -> dict[str, object]:
    usage = exact(
        value,
        {
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
            "cache_creation",
            "output_tokens",
            "output_tokens_details",
            "service_tier",
            "inference_geo",
        },
        code,
    )
    for key in (
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens",
    ):
        nonnegative_int(usage[key], f"{code}_{key}")
    cache = exact(
        usage["cache_creation"],
        {"ephemeral_5m_input_tokens", "ephemeral_1h_input_tokens"},
        f"{code}_cache",
    )
    for item in cache.values():
        nonnegative_int(item, f"{code}_cache_count")
    details = exact(
        usage["output_tokens_details"], {"thinking_tokens"}, f"{code}_details"
    )
    nonnegative_int(details["thinking_tokens"], f"{code}_thinking")
    require(
        usage["service_tier"] == "standard" and usage["inference_geo"] == "global",
        f"{code}_labels",
    )
    return usage


def extract_actual_request(raw: bytes) -> bytes:
    prefix = (
        b'{"type":"provider_request","adapter":"anthropic-messages-v1",'
        b'"endpoint":"https://api.anthropic.com/v1/messages","body":'
    )
    require(raw.startswith(prefix) and raw.endswith(b"}\n"), "request_frame_lexical")
    require(raw.count(b"\n") == 1, "request_frame_line_count")
    body = raw[len(prefix) : -2]
    frame = exact(
        parse_json(raw), {"type", "adapter", "endpoint", "body"}, "request_frame_shape"
    )
    require(frame["body"] == parse_json(body), "request_frame_body_slice")
    return body


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

    for name, expected in RAW_DIGESTS.items():
        require(
            digest((PACKAGE / "raw" / name).read_bytes()) == expected,
            "raw_original_bytes",
        )
    for path in EXPECTED_FILES:
        if path.endswith(".py") or path == "README.md":
            continue
        raw = (PACKAGE / path).read_bytes()
        require(
            b"sk-ant-" not in raw and b"ANTHROPIC_API_KEY=" not in raw,
            "secret_shaped_bytes",
        )

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
    runner_frame_raw = (PACKAGE / "raw/runner-to-bridge.raw.jsonl").read_bytes()
    actual_request_raw = (PACKAGE / "raw/actual-transmitted-body.raw.json").read_bytes()
    require(
        extract_actual_request(runner_frame_raw) == actual_request_raw,
        "actual_request_extraction",
    )
    require(len(actual_request_raw) == 3363, "actual_request_bytes")
    require(digest(actual_request_raw) == ACTUAL_REQUEST_ROOT, "actual_request_root")
    pre_request_raw = (PACKAGE / "raw/request.raw.json").read_bytes()
    require(len(pre_request_raw) == 4278, "pre_request_bytes")
    require(pre_request_raw != actual_request_raw, "request_bytes_must_differ")
    require(
        parse_json(pre_request_raw) == parse_json(actual_request_raw),
        "request_semantic_equality",
    )
    schema_raw = (PACKAGE / "inputs/provider-schema.json").read_bytes()
    compact_schema = json.dumps(
        parse_json(schema_raw), sort_keys=True, separators=(",", ":")
    ).encode()
    occurrence_counts = {
        "pre_frame_frozen_schema": pre_request_raw.count(schema_raw),
        "pre_frame_compact_schema": pre_request_raw.count(compact_schema),
        "actual_frozen_schema": actual_request_raw.count(schema_raw),
        "actual_compact_schema": actual_request_raw.count(compact_schema),
    }
    require(
        occurrence_counts
        == {
            "pre_frame_frozen_schema": 1,
            "pre_frame_compact_schema": 0,
            "actual_frozen_schema": 0,
            "actual_compact_schema": 1,
        },
        "schema_occurrence_counts",
    )

    outcome = exact(
        load(PACKAGE / "terminal-outcome.json"),
        {
            "schema",
            "classification",
            "request_comparison",
            "response",
            "attempt",
            "cause",
            "stopped_state",
        },
        "outcome_shape",
    )
    require(
        outcome["schema"] == "vela.stage-a-anthropic-neutral-v3-terminal-outcome.v1",
        "outcome_schema",
    )
    classification = exact(
        outcome["classification"],
        {"provider_response", "calibration_outcome", "positive_qualification"},
        "classification_shape",
    )
    require(
        classification
        == {
            "provider_response": "terminal_success",
            "calibration_outcome": "non_result_failed_exact_request",
            "positive_qualification": False,
        },
        "classification",
    )
    comparison = exact(
        outcome["request_comparison"],
        {
            "pre_frame_request",
            "actual_transmitted_body",
            "semantic_equal",
            "byte_equal",
            "schema_occurrences",
        },
        "comparison_shape",
    )
    pre_meta = exact(
        comparison["pre_frame_request"], {"path", "bytes", "sha256"}, "pre_meta_shape"
    )
    actual_meta = exact(
        comparison["actual_transmitted_body"],
        {"path", "bytes", "sha256", "extraction"},
        "actual_meta_shape",
    )
    require(
        pre_meta
        == {"path": "raw/request.raw.json", "bytes": 4278, "sha256": REQUEST_ROOT},
        "pre_meta",
    )
    require(
        actual_meta
        == {
            "path": "raw/actual-transmitted-body.raw.json",
            "bytes": 3363,
            "sha256": ACTUAL_REQUEST_ROOT,
            "extraction": "exact_body_slice_from_retained_runner_to_bridge_provider_request_frame",
        },
        "actual_meta",
    )
    require(
        comparison["semantic_equal"] is True and comparison["byte_equal"] is False,
        "comparison_truth",
    )
    require(
        comparison["schema_occurrences"] == occurrence_counts, "comparison_occurrences"
    )
    response_meta = exact(
        outcome["response"],
        {"provider_response_sha256", "parsed_response_sha256", "schema_valid"},
        "response_meta_shape",
    )
    require(
        response_meta
        == {
            "provider_response_sha256": PROVIDER_RESPONSE_ROOT,
            "parsed_response_sha256": RESPONSE_ROOT,
            "schema_valid": True,
        },
        "response_meta",
    )
    outcome_attempt = exact(
        outcome["attempt"],
        {
            "run_id",
            "permit_root",
            "permit_consumed",
            "provider_calls",
            "no_retry",
            "no_reuse",
        },
        "outcome_attempt_shape",
    )
    require(
        outcome_attempt
        == {
            "run_id": RUN_ID,
            "permit_root": PERMIT_ROOT,
            "permit_consumed": True,
            "provider_calls": 1,
            "no_retry": True,
            "no_reuse": True,
        },
        "outcome_attempt",
    )
    cause = exact(
        outcome["cause"],
        {
            "component",
            "mechanism",
            "prospective_lossless_byte_payload_transport",
            "prospective_fresh_permit",
        },
        "cause_shape",
    )
    require(
        cause
        == {
            "component": "outer_provider_request_frame",
            "mechanism": "go_encoding_json_compacted_raw_message_during_outer_frame_marshal",
            "prospective_lossless_byte_payload_transport": "not_implemented",
            "prospective_fresh_permit": "not_implemented",
        },
        "cause",
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
    endpoint = exact(
        load(PACKAGE / "raw/endpoint-contact-receipt.json"),
        {
            "schema",
            "run_id",
            "provider",
            "endpoint",
            "source",
            "provider_calls",
            "endpoint_attempt_receipts",
        },
        "endpoint_shape",
    )
    require(
        endpoint["schema"] == "vela.stage-a-endpoint-contact-receipt.v2"
        and endpoint["run_id"] == RUN_ID
        and endpoint["provider"] == "Anthropic"
        and endpoint["endpoint"] == "https://api.anthropic.com/v1/messages"
        and endpoint["source"] == "host-tee-of-bridge-to-runner-frame-stream",
        "endpoint_binding",
    )
    attempt = exact(
        load(PACKAGE / "raw/attempt-terminal.json"),
        {
            "schema",
            "run_id",
            "status",
            "attempt",
            "retries",
            "permit_root",
            "provider_calls",
            "bridge_provider_calls",
            "runner_provider_calls",
            "terminal_provider_calls",
            "custody_provider_calls",
            "endpoint_attempt_receipts",
            "runner_terminal_present",
            "orchestrator_exit_code",
            "credential_retained",
            "credential_fd_closed",
            "participant_permits_released",
            "openai_neutral_permit_released",
            "scoring_attempts",
            "stage_b_families_selected",
            "authority_effect",
        },
        "attempt_shape",
    )
    require(
        attempt["schema"] == "vela.stage-a-anthropic-neutral-attempt.v2",
        "attempt_schema",
    )
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
    exact_int(attempt["orchestrator_exit_code"], 0, "orchestrator_exit")
    require(
        attempt["runner_terminal_present"] is True
        and attempt["credential_retained"] is False
        and attempt["credential_fd_closed"] is True,
        "attempt_terminal_custody",
    )
    require(
        attempt["endpoint_attempt_receipts"] == endpoint["endpoint_attempt_receipts"],
        "attempt_endpoint_binding",
    )
    exact_int(attempt["participant_permits_released"], 0, "participant_permits")
    require(attempt["openai_neutral_permit_released"] is False, "openai_permit")
    exact_int(attempt["scoring_attempts"], 0, "scoring")
    exact_int(attempt["stage_b_families_selected"], 0, "stage_b")
    require(attempt["authority_effect"] == "none", "authority")

    frames = load_jsonl(PACKAGE / "raw/bridge-to-runner.raw.jsonl")
    require(
        (PACKAGE / "raw/bridge-to-runner.raw.jsonl").read_bytes()
        == (PACKAGE / "raw/provider-events.raw.jsonl").read_bytes(),
        "provider_event_custody",
    )
    require(len(frames) == 3, "provider_frame_count")
    endpoint_frame = exact(
        frames[0], {"type", "provider_calls"}, "endpoint_frame_shape"
    )
    require(
        endpoint_frame == {"type": "endpoint_attempt", "provider_calls": 1},
        "endpoint_frame",
    )
    provider_event = exact(frames[1], {"type", "raw"}, "provider_event_shape")
    require(
        provider_event["type"] == "provider_event"
        and type(provider_event["raw"]) is str,
        "provider_event",
    )
    terminal_frame = exact(
        frames[2], {"type", "body", "provider_calls"}, "terminal_frame_shape"
    )
    require(terminal_frame["type"] == "terminal", "terminal_frame_type")
    exact_int(terminal_frame["provider_calls"], 1, "terminal_frame_calls")
    response = load(PACKAGE / "raw/response.raw.json")
    require(terminal_frame["body"] == response, "terminal_frame_body")

    runner_frames = load_jsonl(PACKAGE / "raw/runner-to-bridge.raw.jsonl")
    require(len(runner_frames) == 1, "request_frame_count")
    request_frame = exact(
        runner_frames[0], {"type", "adapter", "endpoint", "body"}, "request_frame_shape"
    )
    require(
        request_frame["type"] == "provider_request"
        and request_frame["adapter"] == "anthropic-messages-v1"
        and request_frame["endpoint"] == "https://api.anthropic.com/v1/messages",
        "request_frame",
    )
    require(
        request_frame["body"] == parse_json(actual_request_raw),
        "request_frame_actual_body",
    )

    provider_response_raw = (
        PACKAGE / "raw/provider-response-0001.raw.json"
    ).read_bytes()
    require(
        provider_event["raw"].encode() == provider_response_raw,
        "provider_event_response_bytes",
    )
    provider_response = exact(
        parse_json(provider_response_raw),
        {
            "model",
            "id",
            "type",
            "role",
            "content",
            "stop_reason",
            "stop_sequence",
            "stop_details",
            "usage",
        },
        "provider_response_shape",
    )
    require(
        provider_response["model"] == "claude-opus-5"
        and type(provider_response["id"]) is str
        and provider_response["id"].startswith("msg_")
        and provider_response["type"] == "message"
        and provider_response["role"] == "assistant",
        "provider_response_identity",
    )
    require(
        provider_response["stop_reason"] == "end_turn"
        and provider_response["stop_sequence"] is None
        and provider_response["stop_details"] is None,
        "provider_response_terminal",
    )
    content = provider_response["content"]
    require(type(content) is list and len(content) == 2, "provider_content")
    thinking = exact(content[0], {"type", "thinking", "signature"}, "thinking_shape")
    require(
        thinking["type"] == "thinking"
        and thinking["thinking"] == ""
        and type(thinking["signature"]) is str
        and bool(thinking["signature"]),
        "thinking_block",
    )
    text_block = exact(content[1], {"type", "text"}, "text_shape")
    require(
        text_block["type"] == "text" and type(text_block["text"]) is str, "text_block"
    )
    require(parse_json(text_block["text"]) == response, "parsed_response_text")
    require(
        json.dumps(response, sort_keys=True, separators=(",", ":")).encode()
        == (PACKAGE / "raw/response.raw.json").read_bytes(),
        "parsed_response_canonical",
    )
    provider_usage = validate_usage(provider_response["usage"], "provider_usage")

    usage_receipt = exact(
        load(PACKAGE / "raw/provider-usage-0001.json"),
        {"schema", "response_ordinal", "provider_response_sha256", "usage"},
        "usage_receipt_shape",
    )
    require(
        usage_receipt["schema"] == "vela.stage-a-anthropic-usage-custody.v1"
        and usage_receipt["provider_response_sha256"] == PROVIDER_RESPONSE_ROOT,
        "usage_binding",
    )
    exact_int(usage_receipt["response_ordinal"], 1, "usage_ordinal")
    require(
        validate_usage(usage_receipt["usage"], "receipt_usage") == provider_usage,
        "usage_exact_copy",
    )
    require(
        provider_usage
        == {
            "input_tokens": 1891,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 0,
                "ephemeral_1h_input_tokens": 0,
            },
            "output_tokens": 423,
            "output_tokens_details": {"thinking_tokens": 106},
            "service_tier": "standard",
            "inference_geo": "global",
        },
        "usage_frozen_values",
    )

    packet_custody = exact(
        load(PACKAGE / "raw/packet-custody.json"),
        {
            "schema",
            "path",
            "bytes",
            "sha256",
            "link_count",
            "open_mode",
            "canonical_json_object",
            "recursive_duplicate_keys_rejected",
            "recursive_objects_arrays_primitives_canonical",
            "number_lexemes_preserved",
            "inline_reconstruction",
            "injection",
            "request_sha256",
        },
        "packet_custody_shape",
    )
    exact_int(packet_custody["bytes"], 657, "packet_bytes")
    exact_int(packet_custody["link_count"], 1, "packet_links")
    require(
        packet_custody
        == {
            "schema": "vela.stage-a-neutral-packet-custody.v1",
            "path": "/input/packet.json",
            "bytes": 657,
            "sha256": PACKET_ROOT,
            "link_count": 1,
            "open_mode": "read_only_no_follow",
            "canonical_json_object": True,
            "recursive_duplicate_keys_rejected": True,
            "recursive_objects_arrays_primitives_canonical": True,
            "number_lexemes_preserved": True,
            "inline_reconstruction": False,
            "injection": "messages[0].content_exact_prompt_newline_packet_bytes",
            "request_sha256": REQUEST_ROOT,
        },
        "packet_custody",
    )

    teardown = exact(
        load(PACKAGE / "raw/process-teardown.json"),
        {
            "schema",
            "status",
            "credential_fd_closed",
            "credential_retained",
            "bridge_fd_closed",
            "participant_network",
            "children",
        },
        "teardown_shape",
    )
    require(
        teardown["status"] == "completed"
        and teardown["schema"] == "vela.stage-a-anthropic-neutral-process-teardown.v2"
        and teardown["credential_fd_closed"] is True
        and teardown["credential_retained"] is False
        and teardown["bridge_fd_closed"] is True
        and teardown["participant_network"] == "none",
        "teardown",
    )
    children = teardown["children"]
    require(type(children) is list and len(children) == 2, "child_count")
    for child in children:
        exact(child, {"name", "exit_code"}, "child_shape")
        exact_int(child["exit_code"], 0, "child_exit")
    require(
        children
        == [
            {"name": "participant_runner_container", "exit_code": 0},
            {"name": "anthropic_host_bridge", "exit_code": 0},
        ],
        "child_teardown",
    )
    credential = exact(
        load(PACKAGE / "raw/credential-nonretention.json"),
        {
            "schema",
            "credential_source",
            "source_metadata_stable",
            "injection",
            "environment_injection",
            "credential_fd_closed",
            "credential_buffer_scrubbed",
            "evidence_scan_no_credential_bytes",
            "credential_retained",
        },
        "credential_shape",
    )
    require(
        credential["schema"] == "vela.stage-a-credential-nonretention.v1"
        and credential["credential_source"] == "authorized_exact_file"
        and credential["injection"] == "inherited_descriptor_only"
        and credential["credential_buffer_scrubbed"] is True
        and credential["credential_fd_closed"] is True
        and credential["credential_retained"] is False
        and credential["environment_injection"] is False
        and credential["evidence_scan_no_credential_bytes"] is True
        and credential["source_metadata_stable"] is True,
        "credential_nonretention",
    )
    release = exact(
        load(PACKAGE / "raw/permit-release.json"),
        {
            "schema",
            "run_id",
            "permit_root",
            "source_state",
            "consumed_path",
            "attempt",
            "zero_retries",
            "released_at",
        },
        "release_shape",
    )
    require(
        release["schema"] == "vela.stage-a-anthropic-neutral-permit-release.v2"
        and release["run_id"] == RUN_ID
        and release["permit_root"] == PERMIT_ROOT
        and release["source_state"] == "held"
        and release["consumed_path"]
        == "neutral-calibration-anthropic-json-v3-replacement.permit.consumed.json"
        and type(release["released_at"]) is str
        and release["released_at"] == "2026-08-22T21:09:41.606628Z"
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
    require(outcome["stopped_state"] == stopped, "outcome_stopped_state")
    return {
        "status": "PASS_STOPPED_NON_RESULT",
        "artifact_root": manifest["artifact_root"],
        "calibration_outcome": "non_result_failed_exact_request",
        "positive_qualification": False,
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
