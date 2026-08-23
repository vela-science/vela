from __future__ import annotations

import base64
import json
import tempfile
import unittest
from pathlib import Path

from tools.evidence_qualification.runtime_capture import (
    canonical_bytes,
    canonical_root,
    compile_capture,
    compile_to_file,
    raw_root,
    validate_compiled_capture,
)


def write_json(path: Path, value: object) -> bytes:
    raw = canonical_bytes(value) + b"\n"
    path.write_bytes(raw)
    return raw


def payload(body: bytes, schema: bytes) -> dict[str, object]:
    return {
        "base64": base64.b64encode(body).decode(),
        "bytes": len(body),
        "content_type": "application/json",
        "encoding": "base64-rfc4648-canonical",
        "provider_schema_base64": base64.b64encode(schema).decode(),
        "provider_schema_bytes": len(schema),
        "provider_schema_occurrences": 1,
        "provider_schema_sha256": raw_root(schema),
        "schema": "vela.lossless-provider-request-payload.v1",
        "sha256": raw_root(body),
    }


def custody(body: bytes, schema: bytes) -> dict[str, object]:
    return {
        "bytes": len(body),
        "content_type": "application/json",
        "decode_count": 1,
        "endpoint_write_prepared": True,
        "payload_encoding": "base64-rfc4648-canonical",
        "provider_schema_bytes": len(schema),
        "provider_schema_occurrences": 1,
        "provider_schema_sha256": raw_root(schema),
        "schema": "vela.lossless-provider-request-custody.v1",
        "sha256": raw_root(body),
    }


def response_payload(raw: bytes, status: int = 200) -> dict[str, object]:
    return {
        "base64": base64.b64encode(raw).decode(),
        "bytes": len(raw),
        "encoding": "base64-rfc4648-canonical",
        "http_status": status,
        "schema": "vela.lossless-provider-response-payload.v1",
        "sha256": raw_root(raw),
    }


class RuntimeCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="runtime-capture-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def fixture(
        self, *, zero_call: bool = False, http_error: bool = False
    ) -> dict[str, object]:
        schema = b'{"type":"object"}'
        initial = b'{"schema":{"type":"object"},"turn":1}\n'
        continuation = b'{"schema":{"type":"object"},"turn":2}\n'
        response = {
            "assignment_id": "lc-fixture",
            "authority_scientific_inference": {
                "repository_authority_effect": "none",
                "scientific_status": "not_established",
            },
            "change_classification": "neither",
            "impact_closure": [],
            "relation_validation": "valid",
            "schema": "lean-correspondence.review-response.v1",
            "uncertainty": ["offline actual-frame fixture"],
        }
        tool_input = {
            "operation": "read",
            "path": "/workspace/assignment-manifest.json",
            "query": "",
        }
        tool_provider = {
            "content": [
                {
                    "id": "tool-1",
                    "input": tool_input,
                    "name": "read_file",
                    "type": "tool_use",
                }
            ],
            "stop_reason": "tool_use",
        }
        final_provider = {
            "content": [{"text": canonical_bytes(response).decode(), "type": "text"}],
            "stop_reason": "end_turn",
        }
        outgoing = [
            {
                "adapter": "anthropic-messages-v1",
                "endpoint": "https://api.anthropic.com/v1/messages",
                "payload": payload(initial, schema),
                "type": "provider_request",
            }
        ]
        incoming: list[dict[str, object]] = []
        status = "failure" if zero_call or http_error else "response"
        provider_calls = 0 if zero_call else (1 if http_error else 2)
        tool_calls = 0 if zero_call or http_error else 1
        if not zero_call:
            if not http_error:
                outgoing.append(
                    {
                        "arguments": tool_input,
                        "call_id": "tool-1",
                        "name": "read_file",
                        "type": "execute_offline_tool",
                    }
                )
            result = {
                "bytes": 6,
                "content": "BOUND\n",
                "path": "/workspace/assignment-manifest.json",
                "sha256": raw_root(b"BOUND\n"),
            }
            turns = (
                ((1, initial, {"error": "bad request"}),)
                if http_error
                else (
                    (1, initial, tool_provider),
                    (2, continuation, final_provider),
                )
            )
            for ordinal, body, provider in turns:
                request_custody = custody(body, schema)
                incoming.extend(
                    [
                        {
                            "payload": payload(body, schema),
                            "request_custody": request_custody,
                            "type": "request_body",
                        },
                        {
                            "provider_calls": ordinal,
                            "request_custody": request_custody,
                            "type": "endpoint_attempt",
                        },
                        {
                            "response": response_payload(
                                canonical_bytes(provider), 400 if http_error else 200
                            ),
                            "type": "provider_event",
                        },
                    ]
                )
                if ordinal == 1 and not http_error:
                    incoming.extend(
                        [
                            {
                                "arguments": tool_input,
                                "call_id": "tool-1",
                                "name": "read_file",
                                "type": "tool_request",
                            },
                            {
                                "call_id": "tool-1",
                                "name": "read_file",
                                "result": result,
                                "type": "tool_result",
                            },
                        ]
                    )
            if http_error:
                incoming.append(
                    {
                        "error": "provider returned HTTP status 400",
                        "provider_calls": 1,
                        "stop_reason": "http_error",
                        "type": "terminal",
                    }
                )
            else:
                incoming.append(
                    {
                        "body": response,
                        "provider_calls": 2,
                        "stop_reason": "end_turn",
                        "type": "terminal",
                    }
                )

        roots = {
            name: raw_root(name.encode())
            for name in ("evidence", "permit", "boundary", "policy", "workspace")
        }
        permit = {
            "assignment_id": "cell-1",
            "consumed_at": "2026-08-23T00:00:00Z",
            "evidence_manifest_root": roots["evidence"],
            "participant_id": "participant-1",
            "run_id": "run-1",
            "schema": "vela.tooling.closed-launch-permit.v1",
            "status": "consumed",
            "tool_boundary_root": roots["boundary"],
            "tool_policy_root": roots["policy"],
            "workspace_content_root": roots["workspace"],
        }
        permit_raw = write_json(self.root / "permit.json", permit)
        roots["permit"] = raw_root(permit_raw)
        files = {
            "bridge_to_runner": "bridge.jsonl",
            "consumed_permit": "permit.json",
            "launch": "launch.json",
            "raw_response": "response.json",
            "runner_to_bridge": "runner.jsonl",
            "teardown": "teardown.json",
            "terminal": "terminal.json",
            "usage": "usage.json",
        }
        (self.root / "runner.jsonl").write_bytes(
            b"".join(canonical_bytes(row) + b"\n" for row in outgoing)
        )
        (self.root / "bridge.jsonl").write_bytes(
            b"".join(canonical_bytes(row) + b"\n" for row in incoming)
        )
        ordinal_files = {
            "provider_requests": [],
            "provider_responses": [],
            "tool_results": [],
        }
        if not zero_call:
            request_values = [initial] if http_error else [initial, continuation]
            provider_values = (
                [{"error": "bad request"}]
                if http_error
                else [tool_provider, final_provider]
            )
            for ordinal, raw in enumerate(request_values, start=1):
                path = f"provider-request-{ordinal:04d}.raw.json"
                (self.root / path).write_bytes(raw)
                ordinal_files["provider_requests"].append(path)
            for ordinal, value in enumerate(provider_values, start=1):
                path = f"provider-response-{ordinal:04d}.raw.json"
                (self.root / path).write_bytes(canonical_bytes(value))
                ordinal_files["provider_responses"].append(path)
            if not http_error:
                path = "tool-result-0001.raw.json"
                (self.root / path).write_bytes(canonical_bytes(result))
                ordinal_files["tool_results"].append(path)
        write_json(
            self.root / "launch.json",
            {
                "evidence_catalog_root": roots["evidence"],
                "permit_root": roots["permit"],
                "run_id": "run-1",
                "tool_boundary_root": roots["boundary"],
                "tool_policy_root": roots["policy"],
                "workspace_content_root": roots["workspace"],
            },
        )
        write_json(
            self.root / "usage.json",
            {
                "cell_id": "cell-1",
                "input_tokens": 10 if provider_calls else 0,
                "output_tokens": 5 if provider_calls else 0,
                "provider_calls": provider_calls,
                "restricted_seconds": "1200" if zero_call else "10.5",
                "schema": "vela.lean-correspondence-anthropic-open-diagnostic-usage.v3",
                "tool_call_count": tool_calls,
            },
        )
        write_json(
            self.root / "terminal.json",
            {"provider_calls": provider_calls, "status": status},
        )
        write_json(
            self.root / "teardown.json",
            {
                "credential_retained": False,
                "process_reaped": True,
                "provider_calls": provider_calls,
            },
        )
        if zero_call or http_error:
            (self.root / "response.json").write_bytes(b"")
        else:
            write_json(self.root / "response.json", response)
        return {
            "attempt": 1,
            "cell_id": "cell-1",
            "evidence_catalog_root": roots["evidence"],
            "files": files,
            "participant_id": "participant-1",
            "permit_root": roots["permit"],
            "ordinal_files": ordinal_files,
            "run_id": "run-1",
            "schema": "vela.tooling.runtime-capture-compiler-input.v1",
            "terminal_status": status,
            "tool_boundary_root": roots["boundary"],
            "tool_policy_root": roots["policy"],
            "workspace_content_root": roots["workspace"],
        }

    def test_actual_frame_one_tool_compiles_with_two_calls(self) -> None:
        compiled = compile_capture(self.root, self.fixture())
        self.assertEqual(compiled["tool_call_count"], 1)
        self.assertEqual(compiled["provider_calls"], 2)
        self.assertEqual(validate_compiled_capture(compiled), compiled)

    def test_zero_contact_terminal_is_retained(self) -> None:
        compiled = compile_capture(self.root, self.fixture(zero_call=True))
        self.assertEqual(compiled["provider_calls"], 0)
        self.assertIsNone(compiled["final_response"])

    def test_non_success_response_bytes_are_retained_without_result(self) -> None:
        compiled = compile_capture(self.root, self.fixture(http_error=True))
        self.assertEqual(compiled["provider_calls"], 1)
        self.assertEqual(compiled["tool_call_count"], 0)
        self.assertEqual(len(compiled["provider_response_roots"]), 1)
        self.assertIsNone(compiled["final_response"])
        self.assertEqual(validate_compiled_capture(compiled), compiled)

    def test_missing_tool_result_and_call_count_drift_fail(self) -> None:
        fixture = self.fixture()
        bridge = self.root / "bridge.jsonl"
        rows = bridge.read_bytes().splitlines()
        bridge.write_bytes(b"\n".join(rows[:4] + rows[5:]) + b"\n")
        with self.assertRaises(ValueError):
            compile_capture(self.root, fixture)

    def test_emitted_ordinal_request_response_and_tool_files_are_bound(self) -> None:
        for key in ("provider_requests", "provider_responses", "tool_results"):
            fixture = self.fixture()
            path = self.root / fixture["ordinal_files"][key][0]
            path.write_bytes(path.read_bytes() + b" ")
            with self.assertRaises(ValueError):
                compile_capture(self.root, fixture)

    def test_resealed_compiled_lifecycle_drift_fails(self) -> None:
        compiled = compile_capture(self.root, self.fixture())
        compiled["lifecycle"][0]["ordinal"] = 2
        body = {key: value for key, value in compiled.items() if key != "capture_root"}
        compiled["capture_root"] = canonical_root(body)
        with self.assertRaises(ValueError):
            validate_compiled_capture(compiled)
        fixture = self.fixture()
        terminal = json.loads((self.root / "terminal.json").read_bytes())
        terminal["provider_calls"] = 1
        write_json(self.root / "terminal.json", terminal)
        with self.assertRaises(ValueError):
            compile_capture(self.root, fixture)

    def test_compile_output_is_one_shot(self) -> None:
        fixture = self.fixture()
        compile_to_file(self.root, fixture, "capture.json")
        with self.assertRaises(FileExistsError):
            compile_to_file(self.root, fixture, "capture.json")


if __name__ == "__main__":
    unittest.main()
