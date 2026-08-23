"""Deterministic content-addressed assignment evidence trees."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import sys
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

ROOT_RE = "sha256:"


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_json(path: Path) -> Any:
    return json.loads(path.read_bytes(), object_pairs_hook=_pairs)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def raw_root(raw: bytes) -> str:
    return ROOT_RE + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return raw_root(canonical_bytes(value) + b"\n")


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )


def safe_relative(value: Any, label: str) -> Path:
    if type(value) is not str:
        raise ValueError(f"{label} path must be one string")
    path = Path(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ValueError(f"{label} path is unsafe")
    return path


def regular_bytes(path: Path, label: str) -> bytes:
    metadata = os.lstat(path)
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise ValueError(f"{label} must be one regular single-link file")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        raw = b""
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            raw += chunk
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        (opened.st_dev, opened.st_ino, opened.st_nlink, opened.st_size)
        != (after.st_dev, after.st_ino, after.st_nlink, after.st_size)
        or after.st_nlink != 1
        or len(raw) != after.st_size
    ):
        raise ValueError(f"{label} custody drift")
    return raw


def inventory(directory: Path) -> list[dict[str, Any]]:
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("workspace must be one real directory")
    entries = []
    identities = set()
    for path in sorted(directory.rglob("*")):
        metadata = os.lstat(path)
        if stat.S_ISLNK(metadata.st_mode):
            raise ValueError("workspace symlink forbidden")
        if stat.S_ISREG(metadata.st_mode):
            if metadata.st_nlink != 1:
                raise ValueError("workspace hardlink forbidden")
            identity = (metadata.st_dev, metadata.st_ino)
            if identity in identities:
                raise ValueError("workspace inode reuse forbidden")
            identities.add(identity)
            raw = regular_bytes(path, "workspace file")
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(directory).as_posix(),
                    "sha256": raw_root(raw),
                }
            )
    return entries


def materialize(
    *,
    cell_id: str,
    packet_path: Path,
    source_root: Path,
    destination: Path,
) -> dict[str, Any]:
    """Materialize exactly the catalog entries authorized by one packet."""

    packet_raw = regular_bytes(packet_path, "execution packet")
    packet = json.loads(packet_raw, object_pairs_hook=_pairs)
    if type(packet) is not dict or type(packet.get("assignment_id")) is not str:
        raise ValueError("execution packet identity invalid")
    catalog = load_json(source_root / "catalog.json")
    if (
        type(catalog) is not dict
        or set(catalog) != {"assignment_cases", "cases", "schema", "source_commits"}
        or catalog.get("schema")
        != "vela.lean-correspondence-evidence-source-catalog.v1"
        or type(catalog.get("cases")) is not dict
    ):
        raise ValueError("evidence source catalog invalid")
    case_id = catalog["assignment_cases"].get(packet["assignment_id"])
    case = catalog["cases"].get(case_id)
    if (
        type(case) is not dict
        or set(case) != {"participant_visible_case_id", "supplemental_entries"}
        or case["participant_visible_case_id"]
        != packet.get("participant_visible_case_id")
        or type(case.get("supplemental_entries")) is not list
    ):
        raise ValueError("evidence case catalog missing")
    packet_atoms = packet.get("base_semantic_atoms", []) + packet.get(
        "derived_mechanism_atoms", []
    )
    packet_entries = []
    for atom in packet_atoms:
        if type(atom) is not dict or set(atom) != {"bytes", "path", "sha256"}:
            raise ValueError("packet atom shape invalid")
        packet_entries.append(
            {
                "bytes": atom["bytes"],
                "kind": "packet_atom",
                "logical_path": atom["path"],
                "sha256": atom["sha256"],
                "source": {"packet_assignment_id": packet["assignment_id"]},
            }
        )
    entries = packet_entries + case["supplemental_entries"]
    paths = [entry.get("logical_path") for entry in entries]
    if len(paths) != len(set(paths)):
        raise ValueError("evidence logical path duplicate")
    if destination.exists():
        if destination.is_symlink() or not destination.is_dir():
            raise ValueError("existing evidence destination unsafe")
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    bindings = []
    for entry in sorted(entries, key=lambda item: item["logical_path"]):
        if type(entry) is not dict or set(entry) != {
            "bytes",
            "kind",
            "logical_path",
            "sha256",
            "source",
        }:
            raise ValueError("evidence catalog entry shape invalid")
        relative = safe_relative(entry["logical_path"], "evidence")
        sha256 = entry["sha256"]
        if type(sha256) is not str or not sha256.startswith(ROOT_RE):
            raise ValueError("evidence SHA-256 invalid")
        object_path = source_root / "objects" / sha256.removeprefix(ROOT_RE)
        raw = regular_bytes(object_path, "evidence source object")
        if (
            type(entry["bytes"]) is not int
            or type(entry["bytes"]) is bool
            or entry["bytes"] != len(raw)
            or raw_root(raw) != sha256
        ):
            raise ValueError("evidence source object binding drift")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        bindings.append(
            {
                "bytes": len(raw),
                "kind": entry["kind"],
                "logical_path": relative.as_posix(),
                "mounted_path": "/workspace/" + relative.as_posix(),
                "sha256": sha256,
                "source": entry["source"],
            }
        )
    body = {
        "assignment_id": packet["assignment_id"],
        "bindings": bindings,
        "case_id": case_id,
        "cell_id": cell_id,
        "evidence_tree_root": canonical_root(bindings),
        "packet_bytes": len(packet_raw),
        "packet_sha256": raw_root(packet_raw),
        "schema": "vela.lean-correspondence-assignment-evidence-manifest.v1",
        "workspace_mount": "/workspace",
    }
    body["evidence_manifest_root"] = canonical_root(body)
    write_json(destination / "assignment-manifest.json", body)
    workspace_inventory = inventory(destination)
    mount_content_root = canonical_root(workspace_inventory)
    return {
        "entry_count": len(bindings),
        "evidence_manifest_root": body["evidence_manifest_root"],
        "evidence_tree_root": body["evidence_tree_root"],
        "inventory": workspace_inventory,
        "mount_content_root": mount_content_root,
        "workspace_content_root": canonical_root(
            [{"content_root": mount_content_root, "target": "/workspace"}]
        ),
    }
