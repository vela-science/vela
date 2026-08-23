"""Compile exact runner/bridge custody into one scoreable canonical capture."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
from pathlib import Path
from typing import Any

try:
    from tools.evidence_qualification.secure_reader import read_regular
except ModuleNotFoundError:
    from secure_reader import read_regular

ROOT_RE = re.compile(r"sha256:[0-9a-f]{64}\Z")
MAX_TOOL_CALLS = 64
INPUT_KEYS = {
    "attempt",
    "cell_id",
    "evidence_catalog_root",
    "files",
    "participant_id",
    "permit_root",
    "ordinal_files",
    "run_id",
    "schema",
    "terminal_status",
    "tool_boundary_root",
    "tool_policy_root",
    "workspace_content_root",
}
FILE_KEYS = {
    "bridge_to_runner",
    "consumed_permit",
    "launch",
    "raw_response",
    "runner_to_bridge",
    "teardown",
    "terminal",
    "usage",
}
ORDINAL_FILE_KEYS = {"provider_requests", "provider_responses", "tool_results"}
REQUEST_CUSTODY_KEYS = {
    "bytes",
    "content_type",
    "decode_count",
    "endpoint_write_prepared",
    "payload_encoding",
    "provider_schema_bytes",
    "provider_schema_occurrences",
    "provider_schema_sha256",
    "schema",
    "sha256",
}
TOOL_ARGUMENT_KEYS = {"operation", "path", "query"}
COMPILED_KEYS = {
    "attempt",
    "capture_root",
    "cell_id",
    "compiled_once",
    "evidence_catalog_root",
    "final_response",
    "lifecycle",
    "participant_id",
    "permit_root",
    "provider_calls",
    "provider_request_roots",
    "provider_response_roots",
    "run_id",
    "schema",
    "source_evidence",
    "source_evidence_root",
    "terminal_status",
    "tool_boundary_root",
    "tool_call_count",
    "tool_calls",
    "tool_policy_root",
    "usage",
    "workspace_content_root",
}


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def exact_json(raw: bytes, label: str) -> Any:
    try:
        return json.loads(raw, object_pairs_hook=_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} is not exact JSON") from error


def exact_json_lines(
    raw: bytes, label: str, *, allow_empty: bool = False
) -> list[dict[str, Any]]:
    if not raw and allow_empty:
        return []
    if not raw or not raw.endswith(b"\n"):
        raise ValueError(f"{label} must be nonempty newline-terminated JSONL")
    rows = []
    for line in raw.splitlines():
        value = exact_json(line, label)
        if type(value) is not dict:
            raise ValueError(f"{label} row must be one object")
        rows.append(value)
    return rows


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def raw_root(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return raw_root(canonical_bytes(value))


def exact_int(value: Any, label: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{label} must be an exact integer")
    return value


def exact_root(value: Any, label: str) -> str:
    if type(value) is not str or ROOT_RE.fullmatch(value) is None:
        raise ValueError(f"{label} must be one SHA-256 root")
    return value


def _read(root: Path, path: Any, label: str) -> bytes:
    if type(path) is not str:
        raise ValueError(f"{label} path must be one string")
    result = read_regular(root, Path(path), label)
    if not isinstance(result, bytes):
        raise TypeError(f"{label} reader contract invalid")
    return result


def _file_receipt(path: str, raw: bytes) -> dict[str, Any]:
    return {"bytes": len(raw), "path": path, "sha256": raw_root(raw)}


def _request_custody(value: Any, body: bytes) -> dict[str, Any]:
    if type(value) is not dict or set(value) != REQUEST_CUSTODY_KEYS:
        raise ValueError("request custody is not closed")
    if (
        value["schema"] != "vela.lossless-provider-request-custody.v1"
        or value["content_type"] != "application/json"
        or value["payload_encoding"] != "base64-rfc4648-canonical"
        or exact_int(value["decode_count"], "request decode_count") != 1
        or exact_int(value["bytes"], "request bytes") != len(body)
        or exact_int(value["provider_schema_bytes"], "provider schema bytes") <= 0
        or exact_int(value["provider_schema_occurrences"], "schema occurrences") != 1
        or value["endpoint_write_prepared"] is not True
        or exact_root(value["sha256"], "request root") != raw_root(body)
        or ROOT_RE.fullmatch(value["provider_schema_sha256"]) is None
    ):
        raise ValueError("request body custody drift")
    return value


def _decode_payload(payload: Any) -> tuple[bytes, dict[str, Any]]:
    keys = {
        "base64",
        "bytes",
        "content_type",
        "encoding",
        "provider_schema_base64",
        "provider_schema_bytes",
        "provider_schema_occurrences",
        "provider_schema_sha256",
        "schema",
        "sha256",
    }
    if type(payload) is not dict or set(payload) != keys:
        raise ValueError("lossless provider payload is not closed")
    try:
        body = base64.b64decode(payload["base64"], validate=True)
        schema = base64.b64decode(payload["provider_schema_base64"], validate=True)
    except (TypeError, ValueError) as error:
        raise ValueError("lossless provider payload base64 invalid") from error
    if (
        base64.b64encode(body).decode() != payload["base64"]
        or base64.b64encode(schema).decode() != payload["provider_schema_base64"]
        or payload["schema"] != "vela.lossless-provider-request-payload.v1"
        or payload["encoding"] != "base64-rfc4648-canonical"
        or payload["content_type"] != "application/json"
        or exact_int(payload["bytes"], "payload bytes") != len(body)
        or exact_int(payload["provider_schema_bytes"], "schema bytes") != len(schema)
        or exact_int(payload["provider_schema_occurrences"], "schema occurrences") != 1
        or payload["sha256"] != raw_root(body)
        or payload["provider_schema_sha256"] != raw_root(schema)
        or body.count(schema) != 1
        or type(exact_json(body, "initial provider request")) is not dict
    ):
        raise ValueError("lossless provider payload custody invalid")
    return body, payload


def _decode_initial(frame: dict[str, Any]) -> tuple[bytes, dict[str, Any]]:
    if set(frame) != {"adapter", "endpoint", "payload", "type"}:
        raise ValueError("initial provider request frame is not closed")
    if (
        frame["type"] != "provider_request"
        or frame["adapter"] != "anthropic-messages-v1"
    ):
        raise ValueError("initial provider request identity invalid")
    return _decode_payload(frame["payload"])


def _decode_response_payload(payload: Any) -> tuple[bytes, int]:
    keys = {"base64", "bytes", "encoding", "http_status", "schema", "sha256"}
    if type(payload) is not dict or set(payload) != keys:
        raise ValueError("lossless provider response payload is not closed")
    try:
        raw = base64.b64decode(payload["base64"], validate=True)
    except (TypeError, ValueError) as error:
        raise ValueError("lossless provider response base64 invalid") from error
    status = exact_int(payload["http_status"], "HTTP status")
    if (
        base64.b64encode(raw).decode() != payload["base64"]
        or payload["schema"] != "vela.lossless-provider-response-payload.v1"
        or payload["encoding"] != "base64-rfc4648-canonical"
        or exact_int(payload["bytes"], "response bytes") != len(raw)
        or payload["sha256"] != raw_root(raw)
        or not 100 <= status <= 599
    ):
        raise ValueError("lossless provider response custody invalid")
    return raw, status


def _anthropic_response(raw: bytes) -> tuple[str, dict[str, Any] | None, Any]:
    value = exact_json(raw, "raw provider response")
    if type(value) is not dict or type(value.get("content")) is not list:
        raise ValueError("Anthropic response shape invalid")
    stop = value.get("stop_reason")
    tools = [item for item in value["content"] if item.get("type") == "tool_use"]
    texts = [item for item in value["content"] if item.get("type") == "text"]
    if tools:
        if len(tools) != 1 or texts or stop != "tool_use":
            raise ValueError("parallel or mixed Anthropic tool response")
        tool = tools[0]
        if type(tool) is not dict or set(tool) != {"id", "input", "name", "type"}:
            raise ValueError("Anthropic tool_use is not closed")
        if (
            tool["name"] != "read_file"
            or type(tool["id"]) is not str
            or not tool["id"]
            or type(tool["input"]) is not dict
            or set(tool["input"]) != TOOL_ARGUMENT_KEYS
        ):
            raise ValueError("Anthropic tool_use invalid")
        return "tool_use", tool, value
    if len(texts) != 1 or stop != "end_turn":
        raise ValueError("Anthropic terminal response invalid")
    text = texts[0]
    if (
        type(text) is not dict
        or set(text) != {"text", "type"}
        or type(text["text"]) is not str
    ):
        raise ValueError("Anthropic terminal text invalid")
    return "terminal", None, exact_json(text["text"].encode(), "terminal response text")


def compile_capture(root: Path, input_value: Any) -> dict[str, Any]:
    """Validate one exact runtime evidence set and compile its canonical capture."""

    if type(input_value) is not dict or set(input_value) != INPUT_KEYS:
        raise ValueError("capture compiler input is not closed")
    if input_value["schema"] != "vela.tooling.runtime-capture-compiler-input.v1":
        raise ValueError("capture compiler input schema invalid")
    if exact_int(input_value["attempt"], "attempt") != 1:
        raise ValueError("capture compiler permits exactly one attempt")
    for key in (
        "evidence_catalog_root",
        "permit_root",
        "tool_boundary_root",
        "tool_policy_root",
        "workspace_content_root",
    ):
        exact_root(input_value[key], key)
    files = input_value["files"]
    if type(files) is not dict or set(files) != FILE_KEYS:
        raise ValueError("capture compiler files are not closed")
    raw_files = {name: _read(root, path, name) for name, path in files.items()}
    receipts = {
        name: _file_receipt(files[name], raw) for name, raw in raw_files.items()
    }
    ordinal_files = input_value["ordinal_files"]
    if type(ordinal_files) is not dict or set(ordinal_files) != ORDINAL_FILE_KEYS:
        raise ValueError("capture compiler ordinal files are not closed")
    for name, paths in ordinal_files.items():
        if type(paths) is not list or any(type(path) is not str for path in paths):
            raise ValueError(f"{name} ordinal file paths invalid")

    permit = exact_json(raw_files["consumed_permit"], "consumed permit")
    if (
        type(permit) is not dict
        or permit.get("schema") != "vela.tooling.closed-launch-permit.v1"
        or permit.get("status") != "consumed"
        or permit.get("consumed_at") is None
        or permit.get("run_id") != input_value["run_id"]
        or permit.get("participant_id") != input_value["participant_id"]
        or permit.get("assignment_id") != input_value["cell_id"]
        or permit.get("tool_boundary_root") != input_value["tool_boundary_root"]
        or permit.get("tool_policy_root") != input_value["tool_policy_root"]
        or permit.get("workspace_content_root") != input_value["workspace_content_root"]
        or permit.get("evidence_manifest_root") != input_value["evidence_catalog_root"]
        or raw_root(raw_files["consumed_permit"]) != input_value["permit_root"]
    ):
        raise ValueError("consumed permit binding invalid")

    outgoing = exact_json_lines(raw_files["runner_to_bridge"], "runner frames")
    incoming = exact_json_lines(
        raw_files["bridge_to_runner"], "bridge frames", allow_empty=True
    )
    if not outgoing:
        raise ValueError("initial runner frame absent")
    initial_body, _initial_payload = _decode_initial(outgoing[0])
    outgoing_tools = outgoing[1:]
    if len(outgoing_tools) > MAX_TOOL_CALLS:
        raise ValueError("tool call bound exceeded")

    request_bodies: list[bytes] = []
    provider_responses: list[bytes] = []
    lifecycle: list[dict[str, Any]] = []
    tools: list[dict[str, Any]] = []
    cursor = 0
    tool_index = 0
    final_response: Any = None
    seen_calls: set[str] = set()
    while cursor < len(incoming):
        request = incoming[cursor]
        cursor += 1
        if (
            type(request) is not dict
            or set(request)
            != {
                "payload",
                "request_custody",
                "type",
            }
            or request["type"] != "request_body"
        ):
            raise ValueError("exact request-body frame absent or reordered")
        body, _payload = _decode_payload(request["payload"])
        custody = _request_custody(request["request_custody"], body)
        if not request_bodies and body != initial_body:
            raise ValueError("initial request frame/body divergence")
        request_bodies.append(body)
        if cursor >= len(incoming):
            raise ValueError("endpoint attempt absent")
        attempt = incoming[cursor]
        cursor += 1
        ordinal = len(request_bodies)
        if (
            type(attempt) is not dict
            or set(attempt) != {"provider_calls", "request_custody", "type"}
            or attempt["type"] != "endpoint_attempt"
            or exact_int(attempt["provider_calls"], "provider calls") != ordinal
            or attempt["request_custody"] != custody
        ):
            raise ValueError("endpoint attempt sequence or custody drift")
        if cursor >= len(incoming):
            raise ValueError("provider response absent after endpoint attempt")
        provider_event = incoming[cursor]
        cursor += 1
        if (
            type(provider_event) is not dict
            or set(provider_event) != {"response", "type"}
            or provider_event["type"] != "provider_event"
        ):
            raise ValueError("raw provider event absent or forged")
        provider_raw, http_status = _decode_response_payload(provider_event["response"])
        provider_responses.append(provider_raw)
        lifecycle.extend(
            (
                {
                    "ordinal": ordinal,
                    "request_root": raw_root(body),
                    "type": "endpoint_attempt",
                },
                {
                    "ordinal": ordinal,
                    "response_root": raw_root(provider_raw),
                    "type": "provider_event",
                },
            )
        )
        if not 200 <= http_status < 300:
            if cursor >= len(incoming):
                raise ValueError("HTTP failure terminal frame absent")
            terminal_frame = incoming[cursor]
            cursor += 1
            expected_error = f"provider returned HTTP status {http_status}"
            if (
                type(terminal_frame) is not dict
                or set(terminal_frame)
                != {"error", "provider_calls", "stop_reason", "type"}
                or terminal_frame["type"] != "terminal"
                or terminal_frame["error"] != expected_error
                or exact_int(terminal_frame["provider_calls"], "terminal calls")
                != ordinal
                or terminal_frame["stop_reason"] != "http_error"
            ):
                raise ValueError("HTTP failure terminal frame drift")
            lifecycle.append(
                {
                    "http_status": http_status,
                    "ordinal": ordinal,
                    "type": "terminal_failure",
                }
            )
            break
        kind, tool, terminal_response = _anthropic_response(provider_raw)
        if kind == "terminal":
            if cursor >= len(incoming):
                raise ValueError("terminal frame absent")
            terminal_frame = incoming[cursor]
            cursor += 1
            if (
                type(terminal_frame) is not dict
                or set(terminal_frame)
                != {"body", "provider_calls", "stop_reason", "type"}
                or terminal_frame["type"] != "terminal"
                or terminal_frame["body"] != terminal_response
                or exact_int(terminal_frame["provider_calls"], "terminal calls")
                != ordinal
                or terminal_frame["stop_reason"] != "end_turn"
            ):
                raise ValueError("terminal frame drift")
            final_response = terminal_response
            lifecycle.append({"ordinal": ordinal, "type": "terminal"})
            break
        if (
            tool is None
            or cursor + 1 >= len(incoming)
            or tool_index >= len(outgoing_tools)
        ):
            raise ValueError("tool lifecycle incomplete")
        tool_request = incoming[cursor]
        tool_result = incoming[cursor + 1]
        execution = outgoing_tools[tool_index]
        cursor += 2
        tool_index += 1
        expected_request = {
            "arguments": tool["input"],
            "call_id": tool["id"],
            "name": tool["name"],
            "type": "tool_request",
        }
        expected_execution = {
            "arguments": tool["input"],
            "call_id": tool["id"],
            "name": tool["name"],
            "type": "execute_offline_tool",
        }
        if tool_request != expected_request or execution != expected_execution:
            raise ValueError("tool request/validation binding drift")
        if (
            type(tool_result) is not dict
            or set(tool_result) != {"call_id", "name", "result", "type"}
            or tool_result["type"] != "tool_result"
            or tool_result["call_id"] != tool["id"]
            or tool_result["name"] != tool["name"]
            or tool_result["call_id"] in seen_calls
            or type(tool_result["result"]) is not dict
            or not tool_result["result"]
            or type(tool_result["result"].get("path")) is not str
        ):
            raise ValueError("tool result missing, duplicate, or forged")
        seen_calls.add(tool_result["call_id"])
        result_raw = canonical_bytes(tool_result["result"])
        tools.append(
            {
                "arguments": tool["input"],
                "call_id": tool["id"],
                "ordinal": tool_index,
                "provider_response_root": raw_root(provider_raw),
                "result": tool_result["result"],
                "result_root": raw_root(result_raw),
                "stderr_bytes": 0,
                "stderr_root": raw_root(b""),
                "stdout_bytes": 0,
                "stdout_root": raw_root(b""),
                "structured_output_bytes": len(result_raw),
                "structured_output_root": raw_root(result_raw),
                "tool_name": tool["name"],
            }
        )
        lifecycle.extend(
            (
                {"call_id": tool["id"], "ordinal": tool_index, "type": "tool_use"},
                {
                    "call_id": tool["id"],
                    "ordinal": tool_index,
                    "result_root": raw_root(result_raw),
                    "type": "tool_result",
                },
            )
        )

    if cursor != len(incoming) or tool_index != len(outgoing_tools):
        raise ValueError("extra or reordered runtime frames")
    provider_calls = len(request_bodies)
    tool_count = len(tools)
    expected_counts = {
        "provider_requests": provider_calls,
        "provider_responses": len(provider_responses),
        "tool_results": tool_count,
    }
    expected_raw = {
        "provider_requests": request_bodies,
        "provider_responses": provider_responses,
        "tool_results": [canonical_bytes(tool["result"]) for tool in tools],
    }
    for name in sorted(ORDINAL_FILE_KEYS):
        paths = ordinal_files[name]
        if len(paths) != expected_counts[name]:
            raise ValueError(f"{name} ordinal file denominator drift")
        for ordinal, (path, expected) in enumerate(
            zip(paths, expected_raw[name], strict=True), start=1
        ):
            raw = _read(root, path, f"{name}_{ordinal:04d}")
            if raw != expected:
                raise ValueError(f"{name} ordinal file custody drift")
            role = f"{name}_{ordinal:04d}"
            receipts[role] = _file_receipt(path, raw)
    status = input_value["terminal_status"]
    if status == "response":
        if final_response is None or provider_calls != tool_count + 1:
            raise ValueError("response lifecycle must have N+1 successful calls")
    elif status in {"failure", "timeout"}:
        if provider_calls == 0:
            if tool_count != 0 or incoming or len(outgoing) != 1:
                raise ValueError("zero-call pre-contact terminal drift")
        elif (
            status != "failure"
            or final_response is not None
            or provider_calls != tool_count + 1
            or not lifecycle
            or lifecycle[-1].get("type") != "terminal_failure"
        ):
            raise ValueError("contact failure lifecycle invalid")
    else:
        raise ValueError("terminal status invalid")

    raw_response = raw_files["raw_response"]
    if status == "response":
        if exact_json(raw_response, "retained response") != final_response:
            raise ValueError("retained response drift")
    elif raw_response:
        raise ValueError("failed run cannot retain a participant response")

    launch = exact_json(raw_files["launch"], "launch")
    usage = exact_json(raw_files["usage"], "usage")
    terminal = exact_json(raw_files["terminal"], "terminal")
    teardown = exact_json(raw_files["teardown"], "teardown")
    if any(type(value) is not dict for value in (launch, usage, terminal, teardown)):
        raise ValueError("runtime receipt must be one object")
    if (
        launch.get("run_id") != input_value["run_id"]
        or launch.get("permit_root") != input_value["permit_root"]
        or launch.get("workspace_content_root") != input_value["workspace_content_root"]
        or launch.get("evidence_catalog_root") != input_value["evidence_catalog_root"]
        or launch.get("tool_boundary_root") != input_value["tool_boundary_root"]
        or launch.get("tool_policy_root") != input_value["tool_policy_root"]
        or exact_int(usage.get("provider_calls"), "usage provider_calls")
        != provider_calls
        or exact_int(usage.get("tool_call_count"), "usage tool count") != tool_count
        or exact_int(terminal.get("provider_calls"), "terminal provider_calls")
        != provider_calls
        or terminal.get("status") != status
        or exact_int(teardown.get("provider_calls"), "teardown provider_calls")
        != provider_calls
        or teardown.get("credential_retained") is not False
        or teardown.get("process_reaped") is not True
    ):
        raise ValueError("runtime receipt cross-layer drift")

    source_evidence = [receipts[name] | {"role": name} for name in sorted(receipts)]
    body = {
        "attempt": 1,
        "cell_id": input_value["cell_id"],
        "compiled_once": True,
        "evidence_catalog_root": input_value["evidence_catalog_root"],
        "final_response": final_response,
        "lifecycle": lifecycle,
        "participant_id": input_value["participant_id"],
        "permit_root": input_value["permit_root"],
        "provider_calls": provider_calls,
        "provider_request_roots": [raw_root(raw) for raw in request_bodies],
        "provider_response_roots": [raw_root(raw) for raw in provider_responses],
        "run_id": input_value["run_id"],
        "schema": "vela.tooling.compiled-runtime-capture.v1",
        "source_evidence": source_evidence,
        "source_evidence_root": canonical_root(source_evidence),
        "terminal_status": status,
        "tool_boundary_root": input_value["tool_boundary_root"],
        "tool_call_count": tool_count,
        "tool_calls": tools,
        "tool_policy_root": input_value["tool_policy_root"],
        "usage": usage,
        "workspace_content_root": input_value["workspace_content_root"],
    }
    return body | {"capture_root": canonical_root(body)}


def compile_to_file(
    root: Path, input_value: Any, output_relative: Path | str
) -> dict[str, Any]:
    """Compile exactly once and publish with O_EXCL below the evidence root."""

    output_relative = Path(output_relative)
    if output_relative.is_absolute() or len(output_relative.parts) != 1:
        raise ValueError("compiled capture output path must be one safe basename")
    compiled = compile_capture(root, input_value)
    raw = canonical_bytes(compiled) + b"\n"
    directory = os.open(root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        descriptor = os.open(
            output_relative.name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o600,
            dir_fd=directory,
        )
        try:
            written = os.write(descriptor, raw)
            if written != len(raw):
                raise ValueError("compiled capture short write")
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        os.close(directory)
    return compiled


def validate_compiled_capture(value: Any) -> dict[str, Any]:
    """Validate a compiler output without reopening its frozen source files."""

    if type(value) is not dict or set(value) != COMPILED_KEYS:
        raise ValueError("compiled capture is not closed")
    body = {key: item for key, item in value.items() if key != "capture_root"}
    if (
        value["schema"] != "vela.tooling.compiled-runtime-capture.v1"
        or value["capture_root"] != canonical_root(body)
        or value["compiled_once"] is not True
        or exact_int(value["attempt"], "compiled attempt") != 1
        or type(value["cell_id"]) is not str
        or type(value["participant_id"]) is not str
        or type(value["run_id"]) is not str
    ):
        raise ValueError("compiled capture identity invalid")
    for key in (
        "capture_root",
        "evidence_catalog_root",
        "permit_root",
        "source_evidence_root",
        "tool_boundary_root",
        "tool_policy_root",
        "workspace_content_root",
    ):
        exact_root(value[key], key)
    provider_calls = exact_int(value["provider_calls"], "provider calls")
    tool_count = exact_int(value["tool_call_count"], "tool call count")
    if (
        not 0 <= tool_count <= MAX_TOOL_CALLS
        or not 0 <= provider_calls <= MAX_TOOL_CALLS + 1
    ):
        raise ValueError("compiled lifecycle count outside registered bound")
    if (
        type(value["provider_request_roots"]) is not list
        or type(value["provider_response_roots"]) is not list
        or len(value["provider_request_roots"]) != provider_calls
        or len(value["provider_response_roots"]) != provider_calls
        or any(
            ROOT_RE.fullmatch(item) is None
            for item in value["provider_request_roots"]
            + value["provider_response_roots"]
        )
        or len(set(value["provider_request_roots"])) != provider_calls
    ):
        raise ValueError("compiled request/response denominator drift")
    tools = value["tool_calls"]
    if type(tools) is not list or len(tools) != tool_count:
        raise ValueError("compiled tool denominator drift")
    for ordinal, tool in enumerate(tools, start=1):
        if (
            type(tool) is not dict
            or exact_int(tool.get("ordinal"), "tool ordinal") != ordinal
            or type(tool.get("call_id")) is not str
            or not tool["call_id"]
            or tool.get("tool_name") != "read_file"
            or type(tool.get("arguments")) is not dict
            or set(tool["arguments"]) != TOOL_ARGUMENT_KEYS
            or type(tool.get("result")) is not dict
            or exact_int(tool.get("structured_output_bytes"), "structured output bytes")
            < 0
            or exact_int(tool.get("stdout_bytes"), "stdout bytes") != 0
            or exact_int(tool.get("stderr_bytes"), "stderr bytes") != 0
            or any(
                ROOT_RE.fullmatch(tool.get(key, "")) is None
                for key in (
                    "provider_response_root",
                    "result_root",
                    "stderr_root",
                    "stdout_root",
                    "structured_output_root",
                )
            )
            or tool["result_root"] != raw_root(canonical_bytes(tool["result"]))
            or tool["structured_output_root"] != tool["result_root"]
        ):
            raise ValueError("compiled tool receipt invalid")
        if (
            tool["provider_response_root"]
            != value["provider_response_roots"][ordinal - 1]
        ):
            raise ValueError("compiled tool/provider response binding drift")
    lifecycle = value["lifecycle"]
    if type(lifecycle) is not list:
        raise ValueError("compiled lifecycle must be one list")
    expected_lifecycle: list[dict[str, Any]] = []
    for ordinal in range(1, provider_calls + 1):
        expected_lifecycle.extend(
            (
                {
                    "ordinal": ordinal,
                    "request_root": value["provider_request_roots"][ordinal - 1],
                    "type": "endpoint_attempt",
                },
                {
                    "ordinal": ordinal,
                    "response_root": value["provider_response_roots"][ordinal - 1],
                    "type": "provider_event",
                },
            )
        )
        if ordinal <= tool_count:
            tool = tools[ordinal - 1]
            expected_lifecycle.extend(
                (
                    {
                        "call_id": tool["call_id"],
                        "ordinal": ordinal,
                        "type": "tool_use",
                    },
                    {
                        "call_id": tool["call_id"],
                        "ordinal": ordinal,
                        "result_root": tool["result_root"],
                        "type": "tool_result",
                    },
                )
            )
    source = value["source_evidence"]
    if type(source) is not list or value["source_evidence_root"] != canonical_root(
        source
    ):
        raise ValueError("compiled source evidence denominator drift")
    for entry in source:
        if (
            type(entry) is not dict
            or set(entry) != {"bytes", "path", "role", "sha256"}
            or exact_int(entry["bytes"], "source bytes") < 0
            or type(entry["path"]) is not str
            or ROOT_RE.fullmatch(entry["sha256"]) is None
        ):
            raise ValueError("compiled source evidence receipt invalid")
    roles = {item["role"] for item in source}
    expected_roles = set(FILE_KEYS)
    expected_roles.update(
        f"provider_requests_{ordinal:04d}" for ordinal in range(1, provider_calls + 1)
    )
    expected_roles.update(
        f"provider_responses_{ordinal:04d}"
        for ordinal in range(1, len(value["provider_response_roots"]) + 1)
    )
    expected_roles.update(
        f"tool_results_{ordinal:04d}" for ordinal in range(1, tool_count + 1)
    )
    if roles != expected_roles or len(source) != len(expected_roles):
        raise ValueError("compiled source evidence role denominator drift")
    source_by_role = {item["role"]: item for item in source}
    for ordinal, root in enumerate(value["provider_request_roots"], start=1):
        if source_by_role[f"provider_requests_{ordinal:04d}"]["sha256"] != root:
            raise ValueError("compiled request source/root binding drift")
    for ordinal, root in enumerate(value["provider_response_roots"], start=1):
        if source_by_role[f"provider_responses_{ordinal:04d}"]["sha256"] != root:
            raise ValueError("compiled response source/root binding drift")
    for ordinal, tool in enumerate(tools, start=1):
        if (
            source_by_role[f"tool_results_{ordinal:04d}"]["sha256"]
            != tool["result_root"]
        ):
            raise ValueError("compiled tool-result source/root binding drift")
    usage = value["usage"]
    if (
        type(usage) is not dict
        or set(usage)
        != {
            "cell_id",
            "input_tokens",
            "output_tokens",
            "provider_calls",
            "restricted_seconds",
            "schema",
            "tool_call_count",
        }
        or exact_int(usage.get("provider_calls"), "usage provider calls")
        != provider_calls
        or exact_int(usage.get("tool_call_count"), "usage tool count") != tool_count
        or exact_int(usage.get("input_tokens"), "input tokens") < 0
        or exact_int(usage.get("output_tokens"), "output tokens") < 0
        or type(usage.get("restricted_seconds")) is not str
        or type(usage.get("cell_id")) is not str
        or type(usage.get("schema")) is not str
    ):
        raise ValueError("compiled usage drift")
    if value["terminal_status"] == "response":
        if (
            provider_calls != tool_count + 1
            or type(value["final_response"]) is not dict
        ):
            raise ValueError("compiled response lifecycle violates N+1")
        expected_lifecycle.append({"ordinal": provider_calls, "type": "terminal"})
    elif value["terminal_status"] in {"failure", "timeout"}:
        if value["final_response"] is not None:
            raise ValueError("compiled failure cannot retain participant response")
        if provider_calls == 0:
            if tool_count != 0:
                raise ValueError("compiled pre-contact lifecycle invalid")
        elif (
            value["terminal_status"] != "failure"
            or provider_calls != tool_count + 1
            or not value["lifecycle"]
            or value["lifecycle"][-1].get("type") != "terminal_failure"
        ):
            raise ValueError("compiled contact failure lifecycle invalid")
        if provider_calls:
            failure = lifecycle[-1]
            if (
                type(failure) is not dict
                or set(failure) != {"http_status", "ordinal", "type"}
                or exact_int(failure["http_status"], "failure HTTP status")
                in range(200, 300)
                or not 100 <= failure["http_status"] <= 599
            ):
                raise ValueError("compiled contact failure terminal invalid")
            expected_lifecycle.append(failure)
        raw_response_receipt = next(
            item for item in source if item["role"] == "raw_response"
        )
        if raw_response_receipt["bytes"] != 0 or raw_response_receipt[
            "sha256"
        ] != raw_root(b""):
            raise ValueError("compiled pre-contact response bytes invalid")
    else:
        raise ValueError("compiled terminal status invalid")
    if lifecycle != expected_lifecycle:
        raise ValueError("compiled lifecycle ordering or root drift")
    return value
