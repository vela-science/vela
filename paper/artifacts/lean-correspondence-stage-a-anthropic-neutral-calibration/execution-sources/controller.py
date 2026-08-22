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


REPO = Path("/Users/williamblair/.codex/worktrees/stage-a-runtime-qualification/vela")
PACKAGE = REPO / "paper/artifacts/lean-correspondence-stage-a-runtime-qualification"
CREDENTIAL = Path("/Users/williamblair/episteme/atlas-platform/apps/radar/.env.local")
EXECUTION = Path("/private/tmp/vela-stage-a-anthropic-neutral-execution-v1")
INPUT = EXECUTION / "input"
EVIDENCE = EXECUTION / "evidence"
PERMIT = EXECUTION / "permit"
WORKSPACE = EXECUTION / "workspace"
ORCHESTRATOR = Path("/Users/williamblair/Documents/Codex/2026-08-21/realtime-voice-chat-2/work/stage_a_anthropic_once/orchestrator")
IMAGE = "sha256:26fa80f822ebc0357670e03b4358d01d8c2190803696b7fd8aefec83e3e84fcf"
RUN_ID = "neutral-calibration-anthropic-json-v2"
PERMIT_ROOT = "sha256:b9ba39cf1c511043324ca8dfbc02b6c59d91f457a2e560a37d78d32a1b84cdbe"
PACKET_ROOT = "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
PROMPT_ROOT = "sha256:3443fa942b90f84718cc4e6918ebf6d121ebc40cf58f5b3c610f4e983c4d4ed9"
SCHEMA_ROOT = "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
TRUST_ROOT = "sha256:9dae8d76e55cb08991f2b672d58999ea15560d910759c16b544f843bdffbb994"


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


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


def read_exact_key(path: Path, before: os.stat_result) -> bytearray:
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(fd)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise RuntimeError("credential inode changed before open")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(fd, 4096)
            if not chunk:
                break
            total += len(chunk)
            if total > 64 * 1024:
                raise RuntimeError("credential file exceeds bound")
            chunks.append(chunk)
        raw = bytearray(b"".join(chunks))
    finally:
        os.close(fd)
    after = metadata(path)
    stable = ("st_dev", "st_ino", "st_uid", "st_gid", "st_mode", "st_nlink", "st_size", "st_mtime_ns")
    if any(getattr(before, key) != getattr(after, key) for key in stable):
        clear(raw)
        raise RuntimeError("credential metadata changed during read")
    matches: list[bytearray] = []
    for source_line in raw.splitlines():
        line = bytearray(source_line.strip())
        if not line or line.startswith(b"#"):
            clear(line)
            continue
        if line.startswith(b"export "):
            del line[:7]
        if b"=" not in line:
            clear(line)
            continue
        name, value = line.split(b"=", 1)
        name = name.strip()
        value = value.strip()
        if name.startswith(b"ANTHROPIC"):
            if name != b"ANTHROPIC_API_KEY":
                clear(line)
                clear(raw)
                raise RuntimeError("ambiguous Anthropic credential variable")
            if len(value) >= 2 and value[:1] == value[-1:] and value[:1] in (b"'", b'"'):
                value = value[1:-1]
            if not value or b"\x00" in value or b"\n" in value or b"\r" in value:
                clear(line)
                clear(raw)
                raise RuntimeError("Anthropic credential value invalid")
            matches.append(bytearray(value))
        clear(line)
    clear(raw)
    if len(matches) != 1:
        for value in matches:
            clear(value)
        raise RuntimeError("exactly one Anthropic Platform key variable required")
    return matches[0]


def clear(value: bytearray) -> None:
    for index in range(len(value)):
        value[index] = 0


def write_exclusive(path: Path, raw: bytes) -> None:
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        os.write(fd, raw)
        os.fsync(fd)
    finally:
        os.close(fd)


def main() -> int:
    if sys.version_info < (3, 11):
        raise RuntimeError("controller Python too old")
    expected_head = "404adad5f03ccf22f0bcf46770dec59b868acc64"
    expected_tree = "ec90f9b6d51bcd48d78be7657e47a99764ffd9af"
    head = subprocess.run(["git", "rev-parse", "HEAD^{commit}"], cwd=REPO, check=True, capture_output=True, text=True).stdout.strip()
    tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=REPO, check=True, capture_output=True, text=True).stdout.strip()
    remote = subprocess.run(["git", "rev-parse", "origin/codex/stage-a-two-provider-runtime-qualification"], cwd=REPO, check=True, capture_output=True, text=True).stdout.strip()
    if (head, tree, remote) != (expected_head, expected_tree, expected_head):
        raise RuntimeError("producer or remote ref drift")

    if any(EVIDENCE.iterdir()):
        raise RuntimeError("execution evidence directory is not empty")
    image_id = subprocess.run(
        ["docker", "image", "inspect", "--format", "{{.Id}}", IMAGE],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if image_id != IMAGE:
        raise RuntimeError("loaded image identity drift")
    packet = (PACKAGE / "neutral-calibration/packet.json").read_bytes()
    prompt = (PACKAGE / "neutral-calibration/prompt.txt").read_bytes()
    schema = (PACKAGE / "offline-qualification-assets/anthropic-provider_schema.json").read_bytes()
    trust = (INPUT / "ca-certificates.crt").read_bytes()
    if (
        sha256(packet) != PACKET_ROOT
        or sha256(prompt) != PROMPT_ROOT
        or sha256(schema) != SCHEMA_ROOT
        or sha256(trust) != TRUST_ROOT
        or (INPUT / "packet.json").read_bytes() != packet
        or (INPUT / "provider-schema.json").read_bytes() != schema
    ):
        raise RuntimeError("packet, prompt, or provider schema binding drift")
    run_input = json.loads((INPUT / "run.json").read_bytes())
    if (
        run_input.get("run_id") != RUN_ID
        or run_input.get("model") != "claude-opus-5"
        or run_input.get("prompt") != prompt.decode()
        or run_input.get("packet_path") != "/input/packet.json"
        or run_input.get("packet_bytes") != len(packet)
        or run_input.get("packet_sha256") != PACKET_ROOT
        or run_input.get("provider_schema") != json.loads(schema)
        or run_input.get("output_dir") != "/evidence"
    ):
        raise RuntimeError("run input binding drift")

    offline = json.loads((PACKAGE / "offline-qualification.json").read_bytes())
    if any(offline[key] != 0 for key in ("provider_calls", "neutral_calibrations_run", "participant_calls")):
        raise RuntimeError("runtime zero-call state drift")
    openai = json.loads((PACKAGE / "offline-qualification-assets/openai-held_permit.json").read_bytes())
    if openai.get("status") != "held" or openai.get("consumed_at") is not None:
        raise RuntimeError("OpenAI neutral permit drift")
    stage_a = PACKAGE.parent / "lean-correspondence-stage-a-open-pilot"
    prelaunch = json.loads((stage_a / "prelaunch-state.json").read_bytes())
    if (
        prelaunch.get("fixed_denominator") != 12
        or any(prelaunch[key] != 0 for key in ("provider_calls", "released_permits", "participant_responses", "scoring_attempts", "stage_b_families_selected"))
    ):
        raise RuntimeError("Stage A participant stopped state drift")

    permit_path = PERMIT / f"{RUN_ID}.permit.json"
    held = json.loads(permit_path.read_bytes())
    if held["run_id"] != RUN_ID or held["status"] != "held" or held["consumed_at"] is not None:
        raise RuntimeError("held permit state drift")
    qualifier_dir = Path("/private/tmp/vela-stage-a-runtime-qualification-maintained-v1")
    sys.path.insert(0, str(qualifier_dir))
    from tools.evidence_qualification import qualification as q
    if sha256((qualifier_dir / "tools/evidence_qualification/qualification.py").read_bytes()) != "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013":
        raise RuntimeError("maintained qualifier drift")
    if q.canonical_root(held) != PERMIT_ROOT:
        raise RuntimeError("permit canonical root drift")

    before = metadata(CREDENTIAL)
    if sys.argv[1:] == ["--preflight-only"]:
        print("one-shot preflight: PASS")
        return 0
    if sys.argv[1:]:
        raise RuntimeError("unknown controller argument")
    credential = read_exact_key(CREDENTIAL, before)
    try:
        consumed = q.consume_permit(PERMIT.resolve(), RUN_ID, q.permit_identity(held))
        if consumed.name != f"{RUN_ID}.permit.consumed.json" or sha256(consumed.read_bytes()) != sha256(canonical_pretty(held)):
            raise RuntimeError("atomic permit consumption custody drift")
        released_at = dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")
        write_exclusive(EVIDENCE / "permit-release.json", canonical({
            "schema": "vela.stage-a-anthropic-neutral-permit-release.v1",
            "run_id": RUN_ID,
            "permit_root": PERMIT_ROOT,
            "source_state": "held",
            "consumed_path": consumed.name,
            "attempt": 1,
            "zero_retries": True,
            "released_at": released_at,
        }))

        command = [str(ORCHESTRATOR)]
        read_fd, write_fd = os.pipe()
        process = subprocess.Popen(command, stdin=read_fd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        os.close(read_fd)
        try:
            view = memoryview(credential)
            while view:
                written = os.write(write_fd, view)
                view = view[written:]
        finally:
            os.close(write_fd)
        stdout, stderr = process.communicate()
        if bytes(credential) in stdout or bytes(credential) in stderr:
            raise RuntimeError("credential appeared in process output")
        clear(credential)
        write_exclusive(EVIDENCE / "docker.stdout", stdout)
        write_exclusive(EVIDENCE / "docker.stderr", stderr)
        after = metadata(CREDENTIAL)
        stable = ("st_dev", "st_ino", "st_uid", "st_gid", "st_mode", "st_nlink", "st_size", "st_mtime_ns")
        if any(getattr(before, key) != getattr(after, key) for key in stable):
            raise RuntimeError("credential metadata drift after teardown")
        terminal = {
            "schema": "vela.stage-a-anthropic-neutral-attempt.v1",
            "run_id": RUN_ID,
            "permit_root": PERMIT_ROOT,
            "attempt": 1,
            "retries": 0,
            "provider_calls": 1,
            "container_exit_code": process.returncode,
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
        return process.returncode
    finally:
        clear(credential)


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


if __name__ == "__main__":
    raise SystemExit(main())
