#!/usr/bin/env python3
from __future__ import annotations

import base64
import datetime as dt
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO = Path("/Users/williamblair/.codex/worktrees/stage-a-runtime-qualification/vela")
PACKAGE = REPO / "paper/artifacts/lean-correspondence-stage-a-runtime-qualification"
STOPPED = (
    REPO / "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration"
)
STOPPED_V3 = (
    REPO
    / "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3"
)
CREDENTIAL = Path("/Users/williamblair/episteme/atlas-platform/apps/radar/.env.local")
EXECUTION = Path("/private/tmp/vela-stage-a-anthropic-neutral-execution-v4")
INPUT = EXECUTION / "input"
EVIDENCE = EXECUTION / "evidence"
PERMIT = EXECUTION / "permit"
WORKSPACE = EXECUTION / "workspace"
ORCHESTRATOR = Path(
    "/Users/williamblair/Documents/Codex/2026-08-21/realtime-voice-chat-2/work/stage_a_anthropic_v4/orchestrator"
)
IMAGE = "sha256:a95b75cfc449afc2ecb87a5808542339b1776ced9b15d91a27e795993acdbba0"
RUN_ID = "neutral-calibration-anthropic-json-v4-lossless"
PERMIT_ROOT = "sha256:dfc9f20e029b7ea51eb28c6b3d81f70eace063c681d56d2c9ce7356b3dbe8b63"
ARTIFACT_ROOT = (
    "sha256:57d49f290bcecb665b004ec54399361142b83590ed40d7291b8aabe00c8c0a2e"
)
OFFLINE_ROOT = "sha256:7a89479a46e004317cc69b78ffa1ea0c5fe7130a65c257de1dd43c9e31d6578e"
REGISTRATION_ROOT = (
    "sha256:2ddcd97a0dfff125ac88a6c102e58a0f380c929c6bc243a8e8298eb742dc6ef3"
)
PACKET_ROOT = "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
PROMPT_ROOT = "sha256:3443fa942b90f84718cc4e6918ebf6d121ebc40cf58f5b3c610f4e983c4d4ed9"
SCHEMA_ROOT = "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
RUN_ROOT = "sha256:ab8b5541536e3c5c88df7783150973cee1d3ba7dd75ebd79efa389973e2813bd"
REQUEST_ROOT = "sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c"
OFFLINE_RECEIPT_ROOT = (
    "sha256:1f857493dbecf40001dbc3a9e1b5be17ac46dd166096b48e7906a1da7451fddd"
)
TRANSPORT_CUSTODY_ROOT = (
    "sha256:d1813ba1ad996442e38efe92ec8556210ddc11ffc8037a23ebb0c5b007157fb6"
)
TRUST_ROOT = "sha256:9dae8d76e55cb08991f2b672d58999ea15560d910759c16b544f843bdffbb994"
STOPPED_ROOT = "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b"
STOPPED_COMMIT = "30210517f3b1bee420bc61e9a4484ecff8b68ae7"
STOPPED_V3_ROOT = (
    "sha256:63cbbdf6ae6c7e906268b31f33198d06b8db0757e6db48b6187286cacd08dcb9"
)
STOPPED_V3_COMMIT = "37a5a92c314b4f0345eb2d8aadf1890b4e59682d"
EXPECTED_HEAD = "0aee2129f2f7824f328d9576f72e42f240e08932"
EXPECTED_TREE = "e8929efa3559e6abd552a66bbd5e023d9461fad6"
QUALIFIER_ROOT = (
    "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013"
)
QUALIFIER_DIR = Path("/private/tmp/vela-stage-a-runtime-qualification-maintained-v1")


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def sha256(raw: bytes | bytearray) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def clear(value: bytearray) -> None:
    for index in range(len(value)):
        value[index] = 0


def metadata(path: Path) -> os.stat_result:
    current = Path("/")
    for part in path.parts[1:]:
        current /= part
        item = os.lstat(current)
        if stat.S_ISLNK(item.st_mode):
            raise RuntimeError("credential path component is symbolic")
    item = os.lstat(path)
    if (
        not stat.S_ISREG(item.st_mode)
        or item.st_uid != os.getuid()
        or stat.S_IMODE(item.st_mode) != 0o600
        or item.st_nlink != 1
    ):
        raise RuntimeError("credential metadata precondition failed")
    names = subprocess.run(
        ["/usr/bin/xattr", str(path)], check=True, capture_output=True, text=True
    ).stdout.splitlines()
    if names not in ([], ["com.apple.provenance"]):
        raise RuntimeError("credential xattr drift")
    acl = subprocess.run(
        ["/bin/ls", "-lde", str(path)], check=True, capture_output=True, text=True
    ).stdout.splitlines()
    if len(acl) != 1:
        raise RuntimeError("credential ACL is not empty")
    return item


def stable_metadata(before: os.stat_result, after: os.stat_result) -> bool:
    fields = (
        "st_dev",
        "st_ino",
        "st_uid",
        "st_gid",
        "st_mode",
        "st_nlink",
        "st_size",
        "st_mtime_ns",
    )
    return all(getattr(before, key) == getattr(after, key) for key in fields)


def read_exact_key(path: Path, before: os.stat_result) -> bytearray:
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    raw = bytearray()
    try:
        opened = os.fstat(fd)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise RuntimeError("credential inode changed before open")
        while True:
            chunk = os.read(fd, 4096)
            if not chunk:
                break
            raw.extend(chunk)
            if len(raw) > 64 * 1024:
                raise RuntimeError("credential file exceeds bound")
    finally:
        os.close(fd)
    after = metadata(path)
    if not stable_metadata(before, after):
        clear(raw)
        raise RuntimeError("credential metadata changed during read")

    match: tuple[int, int] | None = None
    cursor = 0
    while cursor <= len(raw):
        end = raw.find(b"\n", cursor)
        if end < 0:
            end = len(raw)
        start = cursor
        while start < end and raw[start] in b" \t\r":
            start += 1
        finish = end
        while finish > start and raw[finish - 1] in b" \t\r":
            finish -= 1
        if start < finish and raw[start] != ord("#"):
            if raw[start : min(start + 7, finish)] == b"export ":
                start += 7
                while start < finish and raw[start] in b" \t":
                    start += 1
            equals = raw.find(b"=", start, finish)
            if equals >= 0:
                name_end = equals
                while name_end > start and raw[name_end - 1] in b" \t":
                    name_end -= 1
                name = bytes(raw[start:name_end])
                value_start = equals + 1
                while value_start < finish and raw[value_start] in b" \t":
                    value_start += 1
                value_end = finish
                if name.startswith(b"ANTHROPIC"):
                    if name != b"ANTHROPIC_API_KEY" or match is not None:
                        clear(raw)
                        raise RuntimeError("ambiguous Anthropic credential variable")
                    if (
                        value_end - value_start >= 2
                        and raw[value_start] == raw[value_end - 1]
                        and raw[value_start] in (ord("'"), ord('"'))
                    ):
                        value_start += 1
                        value_end -= 1
                    if value_start >= value_end or 0 in raw[value_start:value_end]:
                        clear(raw)
                        raise RuntimeError("Anthropic credential value invalid")
                    match = (value_start, value_end)
        if end == len(raw):
            break
        cursor = end + 1
    if match is None:
        clear(raw)
        raise RuntimeError("exactly one Anthropic Platform key variable required")
    credential = bytearray(raw[match[0] : match[1]])
    clear(raw)
    return credential


def write_exclusive(path: Path, raw: bytes) -> None:
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        view = memoryview(raw)
        while view:
            written = os.write(fd, view)
            view = view[written:]
        os.fsync(fd)
    finally:
        os.close(fd)


def read_json(path: Path) -> Any:
    return json.loads(path.read_bytes())


def require(condition: bool, label: str) -> None:
    if not condition:
        raise RuntimeError(label)


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO, check=True, capture_output=True, text=True
    ).stdout.strip()


REQUEST_CUSTODY_KEYS = {
    "schema",
    "content_type",
    "bytes",
    "sha256",
    "payload_encoding",
    "decode_count",
    "provider_schema_bytes",
    "provider_schema_sha256",
    "provider_schema_occurrences",
    "endpoint_write_prepared",
}


def exact_request_custody() -> dict[str, Any]:
    return {
        "schema": "vela.lossless-provider-request-custody.v1",
        "content_type": "application/json",
        "bytes": 4278,
        "sha256": REQUEST_ROOT,
        "payload_encoding": "base64-rfc4648-canonical",
        "decode_count": 1,
        "provider_schema_bytes": 2384,
        "provider_schema_sha256": SCHEMA_ROOT,
        "provider_schema_occurrences": 1,
        "endpoint_write_prepared": True,
    }


def validate_request_custody(value: Any) -> None:
    validate_request_custody_shape(value)
    require(value == exact_request_custody(), "request custody binding")


def validate_request_custody_shape(value: Any) -> None:
    require(
        type(value) is dict and set(value) == REQUEST_CUSTODY_KEYS,
        "request custody shape",
    )
    for key in (
        "bytes",
        "decode_count",
        "provider_schema_bytes",
        "provider_schema_occurrences",
    ):
        require(type(value[key]) is int, f"request custody integer type: {key}")
    require(
        value["schema"] == "vela.lossless-provider-request-custody.v1"
        and value["content_type"] == "application/json"
        and value["bytes"] > 0
        and value["sha256"].startswith("sha256:")
        and len(value["sha256"]) == 71
        and value["payload_encoding"] == "base64-rfc4648-canonical"
        and value["decode_count"] == 1
        and value["provider_schema_bytes"] == 2384
        and value["provider_schema_sha256"] == SCHEMA_ROOT
        and value["provider_schema_occurrences"] == 1
        and value["endpoint_write_prepared"] is True,
        "request custody semantics",
    )


def endpoint_receipts(path: Path) -> tuple[list[dict[str, Any]], list[bytes]]:
    receipts: list[dict[str, Any]] = []
    responses: list[bytes] = []
    if not path.exists():
        return receipts, responses
    for raw_line in path.read_bytes().splitlines():
        if not raw_line:
            continue
        frame = json.loads(raw_line)
        if frame.get("type") == "endpoint_attempt":
            require(
                set(frame) == {"type", "provider_calls", "request_custody"}
                and type(frame["provider_calls"]) is int
                and frame["provider_calls"] == len(receipts) + 1,
                "endpoint attempt receipt drift",
            )
            validate_request_custody_shape(frame["request_custody"])
            if not receipts:
                validate_request_custody(frame["request_custody"])
            receipts.append(frame)
        elif frame.get("type") == "provider_event":
            require(type(frame.get("raw")) is str, "provider event raw bytes absent")
            responses.append(frame["raw"].encode())
    return receipts, responses


def retain_lossless_initial_request(transcript: Path) -> dict[str, Any]:
    lines = transcript.read_bytes().splitlines(keepends=True)
    require(
        lines and all(line.endswith(b"\n") for line in lines), "outbound frame custody"
    )
    raw_frame = lines[0]
    pairs: list[tuple[str, Any]] = json.loads(
        raw_frame, object_pairs_hook=lambda value: value
    )
    require(
        [key for key, _ in pairs] == ["type", "adapter", "endpoint", "payload"],
        "provider request frame shape or duplicate drift",
    )
    frame = dict(pairs)
    require(
        frame["type"] == "provider_request"
        and frame["adapter"] == "anthropic-messages-v1"
        and frame["endpoint"] == "https://api.anthropic.com/v1/messages",
        "provider request frame identity",
    )
    payload_pairs = frame["payload"]
    require(type(payload_pairs) is list, "provider request payload object absent")
    expected_keys = [
        "schema",
        "encoding",
        "content_type",
        "bytes",
        "sha256",
        "base64",
        "provider_schema_bytes",
        "provider_schema_sha256",
        "provider_schema_base64",
        "provider_schema_occurrences",
    ]
    require(
        [key for key, _ in payload_pairs] == expected_keys,
        "provider request payload shape or duplicate drift",
    )
    payload = dict(payload_pairs)
    encoded = payload["base64"]
    encoded_schema = payload["provider_schema_base64"]
    require(type(encoded) is str and type(encoded_schema) is str, "payload base64 type")
    try:
        body = base64.b64decode(encoded, validate=True)
        schema = base64.b64decode(encoded_schema, validate=True)
    except ValueError as error:
        raise RuntimeError("payload base64 invalid") from error
    require(base64.b64encode(body).decode() == encoded, "payload base64 noncanonical")
    require(
        base64.b64encode(schema).decode() == encoded_schema,
        "provider schema base64 noncanonical",
    )
    require(
        payload["schema"] == "vela.lossless-provider-request-payload.v1"
        and payload["encoding"] == "base64-rfc4648-canonical"
        and payload["content_type"] == "application/json"
        and type(payload["bytes"]) is int
        and payload["bytes"] == len(body) == 4278
        and payload["sha256"] == sha256(body) == REQUEST_ROOT
        and type(payload["provider_schema_bytes"]) is int
        and payload["provider_schema_bytes"] == len(schema) == 2384
        and payload["provider_schema_sha256"] == sha256(schema) == SCHEMA_ROOT
        and type(payload["provider_schema_occurrences"]) is int
        and payload["provider_schema_occurrences"] == body.count(schema) == 1,
        "lossless payload binding",
    )
    require(
        body == (INPUT / "expected-request.json").read_bytes(), "network body drift"
    )
    write_exclusive(EVIDENCE / "provider-request-frame.raw.jsonl", raw_frame)
    write_exclusive(EVIDENCE / "actual-network-body-0001.raw.json", body)
    return {
        "schema": "vela.stage-a-lossless-network-request-custody.v1",
        "request_ordinal": 1,
        "outbound_frame_count": len(lines),
        "frame_sha256": sha256(raw_frame),
        "frame_payload_encoding": "base64-rfc4648-canonical",
        "frame_decode_count": 1,
        "pre_frame_request_bytes": len(body),
        "pre_frame_request_sha256": REQUEST_ROOT,
        "decoded_network_body_bytes": len(body),
        "decoded_network_body_sha256": sha256(body),
        "endpoint_write_request_bytes": len(body),
        "endpoint_write_request_sha256": REQUEST_ROOT,
        "provider_schema_bytes": len(schema),
        "provider_schema_sha256": sha256(schema),
        "provider_schema_occurrences": body.count(schema),
        "byte_identical": True,
        "json_reserialization": False,
    }


def usage_record(raw: bytes, response_root: str, ordinal: int) -> dict[str, Any]:
    value = json.loads(raw)
    usage = value.get("usage")
    require(type(usage) is dict, "provider usage absent")
    for key, item in usage.items():
        require(type(key) is str and type(item) in (int, str, dict, list), "usage type")
        if type(item) is int:
            require(item >= 0, "usage count negative")
    return {
        "schema": "vela.stage-a-anthropic-usage-custody.v1",
        "response_ordinal": ordinal,
        "provider_response_sha256": response_root,
        "usage": usage,
    }


def preflight() -> os.stat_result:
    require(sys.version_info >= (3, 11), "controller Python too old")
    require(
        not any(name.startswith("ANTHROPIC") for name in os.environ),
        "ambient Anthropic credential forbidden",
    )
    require(git("rev-parse", "HEAD^{commit}") == EXPECTED_HEAD, "producer drift")
    require(git("rev-parse", "HEAD^{tree}") == EXPECTED_TREE, "producer tree drift")
    require(
        git("rev-parse", "origin/codex/stage-a-two-provider-runtime-qualification")
        == EXPECTED_HEAD,
        "remote ref drift",
    )
    require(not any(EVIDENCE.iterdir()), "execution evidence directory is not empty")
    image_id = subprocess.run(
        ["docker", "image", "inspect", "--format", "{{.Id}}", IMAGE],
        check=True,
        capture_output=True,
        text=True,
        env={"PATH": "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"},
    ).stdout.strip()
    require(image_id == IMAGE, "loaded image identity drift")

    verifier = subprocess.run(
        [
            sys.executable,
            "-B",
            str(PACKAGE / "verify.py"),
            "--skip-credential-presence",
        ],
        cwd=REPO,
        check=True,
        capture_output=True,
    )
    verified = json.loads(verifier.stdout)
    require(verified["artifact_root"] == ARTIFACT_ROOT, "artifact root drift")
    require(verified["offline_record_root"] == OFFLINE_ROOT, "offline root drift")
    require(verified["provider_calls"] == 0, "runtime call state drift")
    require(
        read_json(PACKAGE / "artifact-root.json")["artifact_root"] == ARTIFACT_ROOT,
        "artifact manifest drift",
    )
    require(
        sha256(canonical(read_json(PACKAGE / "registration.json")))
        == REGISTRATION_ROOT,
        "registration root drift",
    )

    require(
        git("cat-file", "-e", STOPPED_COMMIT + "^{commit}") == "",
        "stopped commit absent",
    )
    require(
        subprocess.run(
            [
                "git",
                "diff",
                "--quiet",
                STOPPED_COMMIT,
                "--",
                "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration",
            ],
            cwd=REPO,
            check=False,
        ).returncode
        == 0,
        "stopped evidence bytes drift",
    )
    require(
        read_json(STOPPED / "artifact-root.json")["artifact_root"] == STOPPED_ROOT,
        "stopped evidence root drift",
    )
    require(
        git("cat-file", "-e", STOPPED_V3_COMMIT + "^{commit}") == "",
        "stopped v3 commit absent",
    )
    require(
        subprocess.run(
            [
                "git",
                "diff",
                "--quiet",
                STOPPED_V3_COMMIT,
                "--",
                "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3",
            ],
            cwd=REPO,
            check=False,
        ).returncode
        == 0,
        "stopped v3 evidence bytes drift",
    )
    require(
        read_json(STOPPED_V3 / "artifact-root.json")["artifact_root"]
        == STOPPED_V3_ROOT,
        "stopped v3 evidence root drift",
    )

    package_inputs = {
        "packet.json": PACKAGE / "neutral-calibration/packet.json",
        "provider-schema.json": PACKAGE
        / "offline-qualification-assets/anthropic-provider_schema.json",
        "run.json": PACKAGE / "offline-qualification-assets/anthropic-run_input.json",
        "materialization-receipt.json": PACKAGE
        / "offline-qualification-assets/anthropic-materialization_receipt.json",
        "request-transport-custody.json": PACKAGE
        / "offline-qualification-assets/anthropic-request_transport_custody.json",
    }
    for name, source in package_inputs.items():
        require(
            (INPUT / name).read_bytes() == source.read_bytes(), f"staged {name} drift"
        )
    require(
        sha256((INPUT / "packet.json").read_bytes()) == PACKET_ROOT, "packet root drift"
    )
    require(
        sha256((PACKAGE / "neutral-calibration/prompt.txt").read_bytes())
        == PROMPT_ROOT,
        "prompt root drift",
    )
    require(
        sha256((INPUT / "provider-schema.json").read_bytes()) == SCHEMA_ROOT,
        "schema root drift",
    )
    require(sha256((INPUT / "run.json").read_bytes()) == RUN_ROOT, "run root drift")
    require(
        sha256((INPUT / "ca-certificates.crt").read_bytes()) == TRUST_ROOT,
        "trust root drift",
    )
    require(
        (INPUT / "offline-validation-receipt.json").read_bytes()
        == (
            PACKAGE
            / "offline-qualification-assets/anthropic-offline_validation_receipt.json"
        ).read_bytes(),
        "offline validation receipt bytes drift",
    )
    require(
        sha256((INPUT / "offline-validation-receipt.json").read_bytes())
        == OFFLINE_RECEIPT_ROOT,
        "offline validation receipt root drift",
    )
    receipt = read_json(INPUT / "offline-validation-receipt.json")
    require(
        receipt
        == {
            "adapter": "anthropic-messages-v1",
            "bridge_decode_count": 1,
            "bridge_decoded_request_bytes": 4278,
            "bridge_decoded_request_sha256": REQUEST_ROOT,
            "credential_secret": False,
            "dummy_credential_fd": True,
            "endpoint_contact_forbidden": True,
            "endpoint_write_prepared": True,
            "endpoint_write_receipts": 0,
            "mounted_schema_root": SCHEMA_ROOT,
            "participant_validation_path": "exact_runner_prepare_lossless_frame_bridge_decode_and_write_preparation",
            "provider_calls": 0,
            "provider_schema_occurrences": 1,
            "request_bytes": 4278,
            "request_payload_encoding": "base64-rfc4648-canonical",
            "request_payload_sha256": REQUEST_ROOT,
            "request_schema_sha256": SCHEMA_ROOT,
            "request_sha256": REQUEST_ROOT,
            "run_id": RUN_ID,
            "run_json_sha256": RUN_ROOT,
            "schema": "vela.stage-a-offline-pre-request-validation.v1",
            "status": "pass",
        },
        "offline validation receipt semantics drift",
    )
    require(
        sha256((INPUT / "request-transport-custody.json").read_bytes())
        == TRANSPORT_CUSTODY_ROOT,
        "transport custody receipt root drift",
    )
    validate_request_custody(read_json(INPUT / "request-transport-custody.json"))
    require(
        (INPUT / "expected-request.json").read_bytes()
        == (
            PACKAGE / "offline-qualification-assets/anthropic-request_bytes.json"
        ).read_bytes(),
        "expected request bytes drift",
    )
    require(
        sha256((INPUT / "expected-request.json").read_bytes()) == REQUEST_ROOT,
        "request root drift",
    )

    offline = read_json(PACKAGE / "offline-qualification.json")
    require(offline["record_root"] == OFFLINE_ROOT, "offline record root field drift")
    require(
        all(
            offline[key] == 0
            for key in (
                "provider_calls",
                "neutral_calibrations_run",
                "participant_calls",
            )
        ),
        "runtime zero-call state drift",
    )
    prior = offline["prior_consumed_non_call"]
    require(
        prior["producer_commit"] == STOPPED_COMMIT
        and prior["provider_calls"] == 0
        and prior["permit_consumed"] is True
        and prior["endpoint_contact_receipt_bytes"]
        == "sha256:798a8733f655c0e5aa4e16ddec6dc8471d3fb2897b6c3eeb5940907e0f58ac4f",
        "prior consumed non-call drift",
    )
    prior_v3 = offline["prior_consumed_failed_exact_request"]
    require(
        prior_v3["producer_commit"] == STOPPED_V3_COMMIT
        and prior_v3["artifact_root"] == STOPPED_V3_ROOT
        and type(prior_v3["provider_calls"]) is int
        and prior_v3["provider_calls"] == 1
        and prior_v3["permit_consumed"] is True
        and prior_v3["positive_qualification"] is False
        and prior_v3["calibration_outcome"] == "non_result_failed_exact_request",
        "prior consumed v3 failed exact request drift",
    )
    openai = read_json(PACKAGE / "offline-qualification-assets/openai-held_permit.json")
    require(
        openai["status"] == "held" and openai["consumed_at"] is None,
        "OpenAI permit drift",
    )
    stage_a = PACKAGE.parent / "lean-correspondence-stage-a-open-pilot"
    state = read_json(stage_a / "prelaunch-state.json")
    require(
        state["fixed_denominator"] == 12
        and all(
            state[key] == 0
            for key in (
                "provider_calls",
                "released_permits",
                "participant_responses",
                "scoring_attempts",
                "stage_b_families_selected",
            )
        ),
        "Stage A stopped state drift",
    )

    permit_path = PERMIT / f"{RUN_ID}.permit.json"
    source_permit = PACKAGE / "offline-qualification-assets/anthropic-held_permit.json"
    require(
        permit_path.read_bytes() == source_permit.read_bytes(),
        "staged permit bytes drift",
    )
    held = read_json(permit_path)
    require(
        held["run_id"] == RUN_ID
        and held["status"] == "held"
        and held["consumed_at"] is None
        and held["image_digest"] == IMAGE,
        "fresh permit state drift",
    )
    sys.path.insert(0, str(QUALIFIER_DIR))
    from tools.evidence_qualification import qualification as q

    require(
        sha256(
            (
                QUALIFIER_DIR / "tools/evidence_qualification/qualification.py"
            ).read_bytes()
        )
        == QUALIFIER_ROOT,
        "maintained qualifier drift",
    )
    require(q.canonical_root(held) == PERMIT_ROOT, "fresh permit root drift")
    require(
        subprocess.run(
            ["git", "-C", str(WORKSPACE), "status", "--short"],
            check=True,
            capture_output=True,
        ).stdout
        == b"",
        "workspace is not clean",
    )
    return metadata(CREDENTIAL)


def main() -> int:
    before = preflight()
    if sys.argv[1:] == ["--preflight-only"]:
        print("one-shot v4 lossless preflight: PASS")
        return 0
    if sys.argv[1:]:
        raise RuntimeError("unknown controller argument")

    credential = read_exact_key(CREDENTIAL, before)
    try:
        sys.path.insert(0, str(QUALIFIER_DIR))
        from tools.evidence_qualification import qualification as q

        held_path = PERMIT / f"{RUN_ID}.permit.json"
        held = read_json(held_path)
        consumed = q.consume_permit(PERMIT.resolve(), RUN_ID, q.permit_identity(held))
        require(
            consumed.name == f"{RUN_ID}.permit.consumed.json", "consumed path drift"
        )
        require(
            consumed.read_bytes() == canonical_pretty(held),
            "consumed permit bytes drift",
        )
        released_at = (
            dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")
        )
        write_exclusive(
            EVIDENCE / "permit-release.json",
            canonical(
                {
                    "schema": "vela.stage-a-anthropic-neutral-permit-release.v2",
                    "run_id": RUN_ID,
                    "permit_root": PERMIT_ROOT,
                    "source_state": "held",
                    "consumed_path": consumed.name,
                    "attempt": 1,
                    "zero_retries": True,
                    "released_at": released_at,
                }
            ),
        )

        read_fd, write_fd = os.pipe()
        process = subprocess.Popen(
            [str(ORCHESTRATOR)],
            stdin=read_fd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={"PATH": "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"},
        )
        os.close(read_fd)
        try:
            view = memoryview(credential)
            while view:
                written = os.write(write_fd, view)
                view = view[written:]
        finally:
            os.close(write_fd)
        stdout, stderr = process.communicate(timeout=1300)
        require(
            stdout.find(credential) < 0 and stderr.find(credential) < 0,
            "credential appeared in controller child output",
        )
        write_exclusive(EVIDENCE / "orchestrator.stdout", stdout)
        write_exclusive(EVIDENCE / "orchestrator.stderr", stderr)

        network_request_custody = retain_lossless_initial_request(
            EVIDENCE / "runner-to-bridge.raw.jsonl"
        )
        write_exclusive(
            EVIDENCE / "lossless-network-request-custody.json",
            canonical(network_request_custody),
        )
        bridge_receipts, provider_responses = endpoint_receipts(
            EVIDENCE / "bridge-to-runner.raw.jsonl"
        )
        runner_receipts, runner_responses = endpoint_receipts(
            EVIDENCE / "provider-events.raw.jsonl"
        )
        require(
            bridge_receipts == runner_receipts, "bridge/runner endpoint receipt drift"
        )
        require(
            provider_responses == runner_responses,
            "bridge/runner provider response drift",
        )
        provider_calls = len(bridge_receipts)
        require(provider_calls >= 1, "endpoint attempt receipt absent")
        initial_request_custody = bridge_receipts[0]["request_custody"]
        validate_request_custody(initial_request_custody)
        for ordinal, response in enumerate(provider_responses, start=1):
            response_root = sha256(response)
            write_exclusive(
                EVIDENCE / f"provider-response-{ordinal:04d}.raw.json", response
            )
            write_exclusive(
                EVIDENCE / f"provider-usage-{ordinal:04d}.json",
                canonical(usage_record(response, response_root, ordinal)),
            )

        runner_terminal_path = EVIDENCE / "terminal.json"
        runner_terminal: dict[str, Any] | None = None
        if runner_terminal_path.exists():
            runner_terminal = read_json(runner_terminal_path)
            require(
                type(runner_terminal.get("provider_calls")) is int,
                "runner call count type",
            )
            require(
                runner_terminal["provider_calls"] == provider_calls,
                "runner call count drift",
            )
        require(
            (EVIDENCE / "request.raw.json").read_bytes()
            == (INPUT / "expected-request.json").read_bytes(),
            "request bytes drift",
        )
        require(
            sha256((EVIDENCE / "request.raw.json").read_bytes()) == REQUEST_ROOT,
            "request custody root drift",
        )
        require(
            (EVIDENCE / "request-transport-custody.json").read_bytes()
            == (INPUT / "request-transport-custody.json").read_bytes(),
            "runner transport custody bytes drift",
        )
        validate_request_custody(read_json(EVIDENCE / "request-transport-custody.json"))
        require(
            (EVIDENCE / "actual-network-body-0001.raw.json").read_bytes()
            == (EVIDENCE / "request.raw.json").read_bytes()
            == (INPUT / "expected-request.json").read_bytes(),
            "pre-frame/network/request byte drift",
        )
        packet_custody = read_json(EVIDENCE / "packet-custody.json")
        require(
            packet_custody["sha256"] == PACKET_ROOT
            and packet_custody["request_sha256"] == REQUEST_ROOT,
            "packet/request custody drift",
        )

        teardown = read_json(EVIDENCE / "process-teardown.json")
        require(
            teardown["credential_fd_closed"] is True
            and teardown["credential_retained"] is False
            and teardown["bridge_fd_closed"] is True
            and teardown["participant_network"] == "none",
            "teardown drift",
        )
        after = metadata(CREDENTIAL)
        require(
            stable_metadata(before, after), "credential metadata drift after teardown"
        )
        for path in EVIDENCE.iterdir():
            if path.is_file():
                require(
                    path.read_bytes().find(credential) < 0,
                    "credential retained in evidence",
                )
        clear(credential)

        exact_request_custody_complete = (
            provider_calls == 1 and network_request_custody["outbound_frame_count"] == 1
        )
        terminal_success = process.returncode == 0 and exact_request_custody_complete
        terminal = {
            "schema": "vela.stage-a-anthropic-neutral-attempt.v4",
            "run_id": RUN_ID,
            "permit_root": PERMIT_ROOT,
            "attempt": 1,
            "retries": 0,
            "provider_calls": provider_calls,
            "endpoint_attempt_receipts": bridge_receipts,
            "bridge_provider_calls": provider_calls,
            "runner_provider_calls": provider_calls,
            "terminal_provider_calls": provider_calls,
            "custody_provider_calls": provider_calls,
            "runner_terminal_present": runner_terminal is not None,
            "orchestrator_exit_code": process.returncode,
            "status": "completed" if terminal_success else "failed_terminal",
            "provider_response_terminal_success": process.returncode == 0,
            "calibration_outcome": (
                "result_pending_independent_review"
                if terminal_success
                else "non_result_failed_terminal"
            ),
            "positive_qualification": False,
            "lossless_initial_request_custody": initial_request_custody,
            "lossless_network_request_custody": network_request_custody,
            "exact_request_custody_complete": exact_request_custody_complete,
            "credential_retained": False,
            "credential_fd_closed": True,
            "participant_permits_released": 0,
            "openai_neutral_permit_released": False,
            "scoring_attempts": 0,
            "stage_b_families_selected": 0,
            "authority_effect": "none",
        }
        write_exclusive(EVIDENCE / "attempt-terminal.json", canonical(terminal))
        write_exclusive(
            EVIDENCE / "endpoint-contact-receipt.json",
            canonical(
                {
                    "schema": "vela.stage-a-endpoint-contact-receipt.v4",
                    "run_id": RUN_ID,
                    "provider": "Anthropic",
                    "endpoint": "https://api.anthropic.com/v1/messages",
                    "endpoint_attempt_receipts": bridge_receipts,
                    "provider_calls": provider_calls,
                    "source": "host-tee-of-bridge-to-runner-frame-stream",
                    "call_count_derivation": "closed_sequential_endpoint_attempt_receipts_only",
                    "initial_request_custody": initial_request_custody,
                    "pre_frame_request_sha256": REQUEST_ROOT,
                    "actual_network_body_sha256": network_request_custody[
                        "decoded_network_body_sha256"
                    ],
                    "byte_identical": network_request_custody["byte_identical"],
                }
            ),
        )
        write_exclusive(
            EVIDENCE / "credential-nonretention.json",
            canonical(
                {
                    "schema": "vela.stage-a-credential-nonretention.v1",
                    "credential_source": "authorized_exact_file",
                    "injection": "inherited_descriptor_only",
                    "environment_injection": False,
                    "credential_fd_closed": True,
                    "credential_buffer_scrubbed": True,
                    "evidence_scan_no_credential_bytes": True,
                    "source_metadata_stable": True,
                    "credential_retained": False,
                }
            ),
        )
        return 0 if terminal_success else 1
    finally:
        clear(credential)


if __name__ == "__main__":
    raise SystemExit(main())
