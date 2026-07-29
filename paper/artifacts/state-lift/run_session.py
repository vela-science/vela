#!/usr/bin/env python3
"""Run one isolated fresh-session arm of the registered state-lift pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


TASK_ROOT = "sha256:a95a505fee521c811c44f91f78e5e4ac8e903f77b1e4d9ec99794444188bca89"
FRONTIER_COMMIT = "c25e11d332cfbc12b048c314880662d507df53e0"
VELA_ROOT = "sha256:b4b85550aed52134ad2e21a3b1a163390ca1f16673811274b55b3b0f2089ed9c"
RUNTIME_ROOT = "sha256:1da3f4e0e96028b8a771814293c3033dafd1971f943f6c7e79b0897fe705f590"
MODEL_ID = "gpt-5.6-sol"
ARMS = {"git", "vela"}
TOKEN_LIMIT = 50_000


class SessionError(ValueError):
    """Raised when a session cannot satisfy the frozen execution contract."""


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SessionError(message)


def extract_answer_from_events(events: bytes) -> tuple[bytes, int]:
    """Recover the final structured answer from a completed Codex event stream."""
    final_text: str | None = None
    final_line = 0
    for line_number, line in enumerate(events.splitlines(), start=1):
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise SessionError(
                f"Codex event stream has invalid JSON at line {line_number}"
            ) from error
        if (
            event.get("type") == "item.completed"
            and isinstance(event.get("item"), dict)
            and event["item"].get("type") == "agent_message"
            and isinstance(event["item"].get("text"), str)
        ):
            final_text = event["item"]["text"]
            final_line = line_number

    require(final_text is not None, "Codex event stream contains no final answer")
    try:
        parsed = json.loads(final_text)
    except json.JSONDecodeError as error:
        raise SessionError("final Codex answer is not a JSON object") from error
    require(isinstance(parsed, dict), "final Codex answer is not a JSON object")
    return (final_text.strip() + "\n").encode(), final_line


def extract_usage_from_events(events: bytes) -> dict[str, int]:
    """Return the terminal usage counters from a completed Codex event stream."""
    final_usage: dict[str, int] | None = None
    for line_number, line in enumerate(events.splitlines(), start=1):
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise SessionError(
                f"Codex event stream has invalid JSON at line {line_number}"
            ) from error
        if event.get("type") == "turn.completed":
            usage = event.get("usage")
            require(isinstance(usage, dict), "completed turn has no usage object")
            required = (
                "input_tokens",
                "cached_input_tokens",
                "output_tokens",
                "reasoning_output_tokens",
            )
            require(
                all(isinstance(usage.get(key), int) for key in required),
                "completed turn has invalid usage counters",
            )
            final_usage = {key: usage[key] for key in required}
    require(final_usage is not None, "Codex event stream contains no completed turn")
    final_usage["observed_tokens"] = (
        final_usage["input_tokens"] + final_usage["output_tokens"]
    )
    return final_usage


def run(
    argv: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: int | None = None,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def prompt(session_id: str, arm: str, vela: Path | None) -> str:
    tool_contract = (
        "Use only Git, jq, and rg. Vela is unavailable; do not search for or "
        "invoke any Vela binary."
        if arm == "git"
        else (
            "You may use Git, jq, rg, and only these read-only Vela commands "
            f"through the exact binary {vela}: status, show, why, review show, "
            "check, and log."
        )
    )
    return f"""You are session {session_id} in the {arm} arm of a frozen first-party
cold-session study. Work only inside the current exact Erdős Frontier clone.
Do not inspect parent directories, user configuration, authentication files,
or any path outside this clone except the one exact Vela binary named below.
Do not use the network. Do not write, stage, commit, or invoke any mutating
command. Repository and scientific authority credentials are absent.

{tool_contract}

Starting object: Proposal vpr_23f32f95d4f073e8.

Using the retained repository evidence, answer:
1. What is the current Standing of the predecessor Claim and replacement Claim?
2. What exact source predicate changed, at which commits and file roots?
3. What Submission, Verification, Decision, and Event evidence controls the
   current state?
4. Did registration or Verification change accepted state?
5. What is the next scientifically and operationally valid action?
6. What does the terminal result explicitly not establish?

Return exactly one object matching the supplied vela.state-lift-answer.v1
schema. Set task_instance_root to {TASK_ROOT}, session_id to {session_id}, and
arm to {arm}. The allowed next-action and scope-limit codes in the schema are
response vocabulary, not evidence; select them only when the repository
supports them. Do not include prose outside the JSON object."""


def make_read_only(root: Path) -> None:
    for path in [root, *root.rglob("*")]:
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            continue
        path.chmod(mode & ~0o222)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--arm", choices=sorted(ARMS), required=True)
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--codex-home", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--vela", type=Path, required=True)
    parser.add_argument("--frontier", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()

    here = Path(__file__).resolve().parent
    schema = here / "answer.schema.json"
    require(args.session_id.startswith(f"{args.arm}-"), "session ID/arm mismatch")
    require(not output.exists(), "output already exists")
    require((args.codex_home / "auth.json").is_file(), "ephemeral model auth is missing")
    require(args.codex.is_file(), "Codex binary is missing")
    require(args.vela.is_file(), "Vela binary is missing")
    require(sha256_file(args.codex) == RUNTIME_ROOT, "Codex binary root drift")
    require(sha256_file(args.vela) == VELA_ROOT, "Vela binary root drift")
    require(schema.is_file(), "answer schema is missing")

    head = run(["git", "rev-parse", "HEAD"], cwd=args.frontier)
    require(head.returncode == 0, "cannot read source Frontier head")
    require(head.stdout.decode().strip() == FRONTIER_COMMIT, "Frontier head drift")
    status = run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=args.frontier,
    )
    require(status.returncode == 0 and not status.stdout.strip(), "source Frontier is dirty")

    output.mkdir(parents=True)
    with tempfile.TemporaryDirectory(prefix=f"vela-state-lift-{args.session_id}-") as temp:
        temporary = Path(temp)
        clone = temporary / "frontier"
        cloned = run(
            [
                "git",
                "clone",
                "--quiet",
                "--no-local",
                "--no-hardlinks",
                str(args.frontier),
                str(clone),
            ]
        )
        require(cloned.returncode == 0, "fresh clone failed")
        require(
            run(["git", "checkout", "--quiet", FRONTIER_COMMIT], cwd=clone).returncode
            == 0,
            "exact Frontier checkout failed",
        )
        require(
            run(["git", "remote", "remove", "origin"], cwd=clone).returncode == 0,
            "cannot remove clone remote",
        )

        session_vela: Path | None = None
        if args.arm == "vela":
            binary_dir = temporary / "bin"
            binary_dir.mkdir()
            session_vela = binary_dir / "vela"
            shutil.copyfile(args.vela, session_vela)
            session_vela.chmod(0o500)
            require(sha256_file(session_vela) == VELA_ROOT, "session Vela root drift")

        make_read_only(clone)
        events_path = output / "events.jsonl"
        answer_path = output / "answer.v1.json"
        stderr_path = output / "stderr.txt"
        command = [
            str(args.codex),
            "exec",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--sandbox",
            "read-only",
            "--model",
            MODEL_ID,
            "-c",
            'model_reasoning_effort="high"',
            "-c",
            'shell_environment_policy.inherit="none"',
            "--output-schema",
            str(schema),
            "--output-last-message",
            str(answer_path),
            "--json",
            "--cd",
            str(clone),
            prompt(args.session_id, args.arm, session_vela),
        ]
        environment = {
            "CODEX_HOME": str(args.codex_home.resolve()),
            "PATH": "/opt/homebrew/bin:/usr/bin:/bin",
            "LANG": "C",
            "LC_ALL": "C",
            "NO_COLOR": "1",
            "GIT_OPTIONAL_LOCKS": "0",
        }
        started = time.time()
        result = run(
            command,
            cwd=clone,
            env=environment,
            timeout=900,
        )
        completed = time.time()
        events_path.write_bytes(result.stdout)
        stderr_path.write_bytes(result.stderr)
        usage = (
            extract_usage_from_events(result.stdout)
            if result.returncode == 0
            else None
        )
        answer_capture = "output_last_message"
        answer_event_line: int | None = None
        if result.returncode == 0 and not answer_path.is_file():
            recovered_answer, answer_event_line = extract_answer_from_events(
                result.stdout
            )
            answer_path.write_bytes(recovered_answer)
            answer_capture = "final_agent_message_event"

        post_status = run(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=clone,
        )
        answer_root = sha256_file(answer_path) if answer_path.is_file() else None
        record: dict[str, Any] = {
            "schema": "vela.state-lift-session-record.v1",
            "session_id": args.session_id,
            "arm": args.arm,
            "task_instance_root": TASK_ROOT,
            "started_at_unix": started,
            "completed_at_unix": completed,
            "duration_seconds": round(completed - started, 3),
            "exit_code": result.returncode,
            "events_root": sha256_file(events_path),
            "stderr_root": sha256_file(stderr_path),
            "answer_root": answer_root,
            "answer_capture": answer_capture,
            "answer_event_line": answer_event_line,
            "usage": usage,
            "observed_token_limit": TOKEN_LIMIT,
            "within_observed_token_limit": (
                usage is not None and usage["observed_tokens"] <= TOKEN_LIMIT
            ),
            "workspace_dirty_after": bool(post_status.stdout.strip()),
            "network_allowed_to_tools": False,
            "authority_credentials_available": False,
            "model_auth_custody": "supervisor-owned ephemeral CODEX_HOME",
        }
        (output / "record.v1.json").write_text(
            json.dumps(record, sort_keys=True, indent=2) + "\n",
            encoding="utf-8",
        )
        require(result.returncode == 0, "Codex session failed")
        require(answer_path.is_file(), "Codex session emitted no answer")
        require(not post_status.stdout.strip(), "session changed the Frontier")
        require(
            usage is not None and usage["observed_tokens"] <= TOKEN_LIMIT,
            "session exceeded the registered observed-token limit",
        )

    print(
        json.dumps(
            {
                "ok": True,
                "session_id": args.session_id,
                "arm": args.arm,
                "output": str(output),
                "answer_root": answer_root,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (SessionError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
