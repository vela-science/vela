#!/usr/bin/env python3
"""Build and verify a deterministic source-only Vela paper artifact."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import subprocess
import sys
import tarfile
from pathlib import Path, PurePosixPath


MANIFEST_PATH = "vela-paper-artifact/manifest.json"


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def git_identity(repo: Path, commit: str = "HEAD") -> tuple[str, str]:
    resolved = git(repo, "rev-parse", commit).decode().strip()
    tree = git(repo, "rev-parse", f"{resolved}^{{tree}}").decode().strip()
    return resolved, tree


def tracked_members(repo: Path, commit: str) -> list[tuple[str, bytes, int, str | None]]:
    encoded = git(repo, "ls-tree", "-r", "-z", commit)
    members: list[tuple[str, bytes, int, str | None]] = []
    for record in encoded.split(b"\0"):
        if not record:
            continue
        metadata, raw_path = record.split(b"\t", 1)
        mode, kind, _object_id = metadata.decode().split()
        require(kind == "blob", f"unsupported Git object kind {kind}")
        path = raw_path.decode("utf-8")
        data = git(repo, "show", f"{commit}:{path}")
        if mode == "120000":
            members.append((path, b"", 0o777, data.decode("utf-8")))
        else:
            members.append((path, data, 0o755 if mode == "100755" else 0o644, None))
    return members


def external_members(
    sources: dict[str, object],
    erdos: Path,
    formal: Path,
) -> list[tuple[str, bytes, int, str | None, dict[str, object]]]:
    members: list[tuple[str, bytes, int, str | None, dict[str, object]]] = []
    erdos_spec = sources["erdos_frontier"]
    erdos_commit, erdos_tree = git_identity(erdos, erdos_spec["commit"])
    require(erdos_commit == erdos_spec["commit"], "Erdős commit mismatch")
    require(erdos_tree == erdos_spec["tree"], "Erdős tree mismatch")
    for member in erdos_spec["members"]:
        data = git(erdos, "show", f"{erdos_commit}:{member['path']}")
        require(sha256(data) == member["root"], f"Erdős member root mismatch: {member['path']}")
        archive_path = f"vela-paper-artifact/external/erdos-frontier/{member['path']}"
        members.append(
            (
                archive_path,
                data,
                0o644,
                None,
                {
                    "repository": erdos_spec["repository"],
                    "commit": erdos_commit,
                    "tree": erdos_tree,
                    "source_path": member["path"],
                },
            )
        )

    formal_spec = sources["formal_conjectures"]
    source_path = formal_spec["path"]
    for version in formal_spec["versions"]:
        commit, tree = git_identity(formal, version["commit"])
        require(commit == version["commit"], f"Formal Conjectures {version['label']} commit mismatch")
        require(tree == version["tree"], f"Formal Conjectures {version['label']} tree mismatch")
        data = git(formal, "show", f"{commit}:{source_path}")
        require(sha256(data) == version["root"], f"Formal Conjectures {version['label']} root mismatch")
        archive_path = (
            "vela-paper-artifact/external/formal-conjectures/"
            f"{version['label']}/{source_path}"
        )
        members.append(
            (
                archive_path,
                data,
                0o644,
                None,
                {
                    "repository": formal_spec["repository"],
                    "commit": commit,
                    "tree": tree,
                    "source_path": source_path,
                    "label": version["label"],
                },
            )
        )
    return members


def tar_info(path: str, mode: int, size: int = 0) -> tarfile.TarInfo:
    info = tarfile.TarInfo(path)
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    info.size = size
    return info


def add_member(
    archive: tarfile.TarFile,
    path: str,
    data: bytes,
    mode: int,
    link: str | None,
) -> None:
    info = tar_info(path, mode, len(data))
    if link is not None:
        info.type = tarfile.SYMTYPE
        info.linkname = link
        info.size = 0
        archive.addfile(info)
    else:
        archive.addfile(info, io.BytesIO(data))


def build(args: argparse.Namespace) -> int:
    vela = args.vela.resolve()
    erdos = args.erdos_frontier.resolve()
    formal = args.formal_conjectures.resolve()
    output = args.output.resolve()
    require(not output.exists(), "output archive already exists")
    require(not git(vela, "status", "--porcelain=v1"), "Vela worktree must be clean")

    sources_path = vela / "paper" / "artifact-sources.json"
    sources_bytes = sources_path.read_bytes()
    sources = json.loads(sources_bytes)
    require(sources["schema"] == "vela.paper-artifact-sources.v1", "artifact source schema mismatch")
    vela_commit, vela_tree = git_identity(vela)

    members: list[tuple[str, bytes, int, str | None, dict[str, object]]] = []
    for path, data, mode, link in tracked_members(vela, vela_commit):
        members.append(
            (
                f"vela-paper-artifact/source/vela/{path}",
                data,
                mode,
                link,
                {
                    "repository": sources["vela"]["repository"],
                    "commit": vela_commit,
                    "tree": vela_tree,
                    "source_path": path,
                },
            )
        )
    members.extend(external_members(sources, erdos, formal))
    members.sort(key=lambda member: member[0])

    manifest_members = []
    for path, data, mode, link, provenance in members:
        manifest_members.append(
            {
                "path": path,
                "kind": "symlink" if link is not None else "file",
                "mode": f"{mode:04o}",
                "bytes": len(data) if link is None else len(link.encode()),
                "root": sha256(data if link is None else link.encode()),
                "provenance": provenance,
            }
        )
    manifest = {
        "schema": "vela.paper-artifact-manifest.v1",
        "source_manifest_root": sha256(sources_bytes),
        "vela": {
            "repository": sources["vela"]["repository"],
            "commit": vela_commit,
            "tree": vela_tree,
        },
        "member_count": len(manifest_members),
        "members": manifest_members,
        "nonclaims": [
            "This package does not establish the protocol breakthrough benchmark.",
            "First-party evidence does not establish external reproduction.",
            "Verification artifacts do not constitute scientific acceptance.",
        ],
    }
    manifest_bytes = f"{json.dumps(manifest, sort_keys=True, separators=(',', ':'))}\n".encode()

    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                add_member(archive, MANIFEST_PATH, manifest_bytes, 0o644, None)
                for path, data, mode, link, _provenance in members:
                    add_member(archive, path, data, mode, link)
    print(
        json.dumps(
            {
                "schema": "vela.paper-artifact-build-result.v1",
                "output": str(output),
                "archive_root": sha256(output.read_bytes()),
                "manifest_root": sha256(manifest_bytes),
                "member_count": len(manifest_members),
                "vela_commit": vela_commit,
                "vela_tree": vela_tree,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


def safe_member_name(name: str) -> bool:
    path = PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts


def verify(args: argparse.Namespace) -> int:
    archive_path = args.archive.resolve()
    with tarfile.open(archive_path, mode="r:gz") as archive:
        names = archive.getnames()
        require(len(names) == len(set(names)), "archive contains duplicate member names")
        require(all(safe_member_name(name) for name in names), "archive contains an unsafe member path")
        require(MANIFEST_PATH in names, "archive manifest missing")
        manifest_file = archive.extractfile(MANIFEST_PATH)
        require(manifest_file is not None, "archive manifest is not a file")
        manifest_bytes = manifest_file.read()
        manifest = json.loads(manifest_bytes)
        require(manifest["schema"] == "vela.paper-artifact-manifest.v1", "artifact manifest schema mismatch")
        expected_names = {MANIFEST_PATH}
        for member in manifest["members"]:
            name = member["path"]
            expected_names.add(name)
            info = archive.getmember(name)
            if member["kind"] == "symlink":
                require(info.issym(), f"{name} is not the declared symlink")
                encoded = info.linkname.encode()
            else:
                require(info.isfile(), f"{name} is not the declared file")
                stream = archive.extractfile(info)
                require(stream is not None, f"{name} cannot be read")
                encoded = stream.read()
            require(sha256(encoded) == member["root"], f"{name} root mismatch")
            require(len(encoded) == member["bytes"], f"{name} byte count mismatch")
        require(set(names) == expected_names, "archive members differ from the manifest")
    print(
        json.dumps(
            {
                "schema": "vela.paper-artifact-verification-result.v1",
                "ok": True,
                "archive_root": sha256(archive_path.read_bytes()),
                "manifest_root": sha256(manifest_bytes),
                "member_count": manifest["member_count"],
                "vela_commit": manifest["vela"]["commit"],
                "vela_tree": manifest["vela"]["tree"],
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    subcommands = parser.add_subparsers(dest="command", required=True)
    build_parser = subcommands.add_parser("build")
    build_parser.add_argument("--vela", type=Path, default=Path.cwd())
    build_parser.add_argument("--erdos-frontier", type=Path, required=True)
    build_parser.add_argument("--formal-conjectures", type=Path, required=True)
    build_parser.add_argument("--output", type=Path, required=True)
    build_parser.set_defaults(handler=build)
    verify_parser = subcommands.add_parser("verify")
    verify_parser.add_argument("archive", type=Path)
    verify_parser.set_defaults(handler=verify)
    args = parser.parse_args()
    try:
        return args.handler(args)
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError, tarfile.TarError) as error:
        print(f"paper artifact error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
