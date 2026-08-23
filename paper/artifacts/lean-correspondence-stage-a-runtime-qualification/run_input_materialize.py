"""Materialize a runner input while preserving provider-schema bytes exactly."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import stat
from collections.abc import Callable
from pathlib import Path
from typing import Any

_reader_spec = importlib.util.spec_from_file_location(
    "vela_runtime_secure_reader", Path(__file__).with_name("secure_reader.py")
)
if _reader_spec is None or _reader_spec.loader is None:
    raise RuntimeError("maintained secure reader unavailable")
_reader_module = importlib.util.module_from_spec(_reader_spec)
_reader_spec.loader.exec_module(_reader_module)
read_absolute_regular = _reader_module.read_absolute_regular

SENTINEL = "__VELA_EXACT_PROVIDER_SCHEMA_BYTES__"


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def read_exact_regular(
    path: Path, after_open: Callable[[], None] | None = None
) -> tuple[bytes, os.stat_result]:
    def validate(_raw: bytes) -> None:
        if after_open:
            after_open()

    result = read_absolute_regular(path, "schema", validator=validate)
    if not isinstance(result, tuple):
        raise TypeError("schema_reader_contract_invalid")
    raw, _validated = result
    return raw, os.stat_result((stat.S_IFREG, 0, 0, 1, 0, 0, len(raw), 0, 0, 0))


def materialize(
    schema_path: Path,
    run_path: Path,
    receipt_path: Path,
    fields: dict[str, Any],
    after_open: Callable[[], None] | None = None,
) -> tuple[bytes, bytes]:
    schema_raw, opened = read_exact_regular(schema_path, after_open)
    if not schema_raw or len(schema_raw) > 16 * 1024 * 1024:
        raise ValueError("schema_size_invalid")
    value = dict(fields)
    value.update(
        {
            "materialization_receipt_path": "/input/materialization-receipt.json",
            "provider_schema": SENTINEL,
            "provider_schema_bytes": len(schema_raw),
            "provider_schema_path": "/input/provider-schema.json",
            "provider_schema_sha256": digest(schema_raw),
        }
    )
    template = canonical(value)
    needle = json.dumps(SENTINEL).encode()
    if template.count(needle) != 1:
        raise ValueError("schema_splice_not_unique")
    start = template.index(needle)
    run_raw = template[:start] + schema_raw + template[start + len(needle) :]
    end = start + len(schema_raw)
    receipt = {
        "schema": "vela.stage-a-run-input-materialization.v1",
        "source_path": "/input/provider-schema.json",
        "source_regular": True,
        "source_single_link": True,
        "source_no_follow": True,
        "source_pre_post_same_inode": True,
        "source_bytes": opened.st_size,
        "source_sha256": digest(schema_raw),
        "raw_inserted_start": start,
        "raw_inserted_end": end,
        "raw_inserted_sha256": digest(run_raw[start:end]),
        "run_json_sha256": digest(run_raw),
        "mounted_schema_root": digest(schema_raw),
        "request_schema_sha256": digest(schema_raw),
        "parse_reserialization_used": False,
    }
    receipt_raw = canonical(receipt)
    run_path.write_bytes(run_raw)
    receipt_path.write_bytes(receipt_raw)
    return run_raw, receipt_raw


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--prompt-file", type=Path, required=True)
    parser.add_argument("--packet-file", type=Path, required=True)
    parser.add_argument("--permit-root", required=True)
    parser.add_argument("--workspace-content-root", required=True)
    parser.add_argument("--evidence-catalog-root", required=True)
    parser.add_argument("--tool-boundary-root", required=True)
    parser.add_argument("--tool-policy-root", required=True)
    parser.add_argument("--workspace-preflight-root", required=True)
    args = parser.parse_args()
    packet = args.packet_file.read_bytes()
    fields = {
        "run_id": args.run_id,
        "model": args.model,
        "prompt": args.prompt_file.read_text(),
        "packet_path": "/input/packet.json",
        "packet_bytes": len(packet),
        "packet_sha256": digest(packet),
        "output_dir": "/evidence",
        "permit_root": args.permit_root,
        "workspace_content_root": args.workspace_content_root,
        "evidence_catalog_root": args.evidence_catalog_root,
        "tool_boundary_root": args.tool_boundary_root,
        "tool_policy_root": args.tool_policy_root,
        "workspace_preflight_root": args.workspace_preflight_root,
    }
    materialize(args.schema, args.run, args.receipt, fields)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
