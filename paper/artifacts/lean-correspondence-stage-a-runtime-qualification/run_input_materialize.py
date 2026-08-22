"""Materialize a runner input while preserving provider-schema bytes exactly."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from collections.abc import Callable
from pathlib import Path
from typing import Any

SENTINEL = "__VELA_EXACT_PROVIDER_SCHEMA_BYTES__"


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def read_exact_regular(
    path: Path, after_open: Callable[[], None] | None = None
) -> tuple[bytes, os.stat_result]:
    absolute = path.absolute()
    current = Path(absolute.anchor)
    for part in absolute.parts[1:]:
        current /= part
        info = os.lstat(current)
        if stat.S_ISLNK(info.st_mode):
            raise ValueError("schema_path_contains_symlink")
    before = os.lstat(absolute)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise ValueError("schema_not_single_link_regular_file")
    fd = os.open(absolute, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
            raise ValueError("schema_open_inode_drift")
        if after_open:
            after_open()
        raw = b""
        while True:
            chunk = os.read(fd, 1024 * 1024)
            if not chunk:
                break
            raw += chunk
    finally:
        os.close(fd)
    after = os.lstat(absolute)
    if (
        (opened.st_dev, opened.st_ino) != (after.st_dev, after.st_ino)
        or after.st_nlink != 1
        or not stat.S_ISREG(after.st_mode)
        or len(raw) != opened.st_size
    ):
        raise ValueError("schema_post_read_inode_drift")
    return raw, opened


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
    }
    materialize(args.schema, args.run, args.receipt, fields)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
