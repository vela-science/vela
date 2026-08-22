#!/usr/bin/env python3
from __future__ import annotations

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
CREDENTIAL = Path("/Users/williamblair/episteme/atlas-platform/apps/radar/.env.local")
EXECUTION = Path("/private/tmp/vela-stage-a-anthropic-neutral-execution-v3")
INPUT = EXECUTION / "input"
EVIDENCE = EXECUTION / "evidence"
PERMIT = EXECUTION / "permit"
WORKSPACE = EXECUTION / "workspace"
ORCHESTRATOR = Path(
    "/Users/williamblair/Documents/Codex/2026-08-21/realtime-voice-chat-2/work/stage_a_anthropic_v3/orchestrator"
)
IMAGE = "sha256:d314adbd8b3765d9aada03bd5bd87ec77826cd81fdc8c8aab3982dce3165385d"
RUN_ID = "neutral-calibration-anthropic-json-v3-replacement"
PERMIT_ROOT = "sha256:7ddf24c9dbeac2cdce1a4ca1972a0984287dbcf528881ae01cbfe297217e2f32"
ARTIFACT_ROOT = (
    "sha256:60a517ddb2ada4c631681e040aae2b18d0b5d525059b0d137105659fb32ab5d2"
)
OFFLINE_ROOT = "sha256:2314d77a6e0fb8f85a88ad398b84725eb75deb43c8c6062e3418affd5d893004"
REGISTRATION_ROOT = (
    "sha256:e0903af7d0e5d9c120601bdd786e8d9b3ca0c7532bac1a4a60e718690f32091e"
)
PACKET_ROOT = "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
PROMPT_ROOT = "sha256:3443fa942b90f84718cc4e6918ebf6d121ebc40cf58f5b3c610f4e983c4d4ed9"
SCHEMA_ROOT = "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
RUN_ROOT = "sha256:efade9842484fe6e96a7e6fe4ced922b1b4da497351237208d6d45927861fc3d"
REQUEST_ROOT = "sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c"
OFFLINE_RECEIPT_ROOT = (
    "sha256:6b82168a774f4412100b8de7c7eadf12022d329e176b50686c07e0003ee0729b"
)
TRUST_ROOT = "sha256:9dae8d76e55cb08991f2b672d58999ea15560d910759c16b544f843bdffbb994"
STOPPED_ROOT = "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b"
STOPPED_COMMIT = "30210517f3b1bee420bc61e9a4484ecff8b68ae7"
EXPECTED_HEAD = "2f99d225a5a3e675e32264b4398fa346d9c3bf97"
EXPECTED_TREE = "63c18f5812d9c4add3590ac798064ac207ac9d93"
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
                set(frame) == {"type", "provider_calls"}
                and type(frame["provider_calls"]) is int
                and frame["provider_calls"] == len(receipts) + 1,
                "endpoint attempt receipt drift",
            )
            receipts.append(frame)
        elif frame.get("type") == "provider_event":
            require(type(frame.get("raw")) is str, "provider event raw bytes absent")
            responses.append(frame["raw"].encode())
    return receipts, responses


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

    package_inputs = {
        "packet.json": PACKAGE / "neutral-calibration/packet.json",
        "provider-schema.json": PACKAGE
        / "offline-qualification-assets/anthropic-provider_schema.json",
        "run.json": PACKAGE / "offline-qualification-assets/anthropic-run_input.json",
        "materialization-receipt.json": PACKAGE
        / "offline-qualification-assets/anthropic-materialization_receipt.json",
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
            "credential_secret": False,
            "dummy_credential_fd": True,
            "endpoint_contact_forbidden": True,
            "endpoint_write_receipts": 0,
            "mounted_schema_root": SCHEMA_ROOT,
            "participant_validation_path": "exact_runner_prepare_and_request_construction",
            "provider_calls": 0,
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
        print("one-shot v3 preflight: PASS")
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

        terminal = {
            "schema": "vela.stage-a-anthropic-neutral-attempt.v2",
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
            "status": "completed" if process.returncode == 0 else "failed_terminal",
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
                    "schema": "vela.stage-a-endpoint-contact-receipt.v2",
                    "run_id": RUN_ID,
                    "provider": "Anthropic",
                    "endpoint": "https://api.anthropic.com/v1/messages",
                    "endpoint_attempt_receipts": bridge_receipts,
                    "provider_calls": provider_calls,
                    "source": "host-tee-of-bridge-to-runner-frame-stream",
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
        return process.returncode
    finally:
        clear(credential)


if __name__ == "__main__":
    raise SystemExit(main())
