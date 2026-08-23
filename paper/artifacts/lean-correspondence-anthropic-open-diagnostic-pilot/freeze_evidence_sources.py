"""Freeze exact Git-object evidence used by the six diagnostic cells."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
STAGE_A = ROOT.parent / "lean-correspondence-stage-a-open-pilot"
DEFAULT_OUTPUT = ROOT / "evidence-sources"
CANDIDATE_COMMIT = "148e18cce542f397ccb60b21a896ba063f6d6cca"
CORRESPONDENCE_COMMIT = "01d0b3253227bc41d2edc13e5cb318bdae53fc88"


def sha256(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def run(repo: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=True,
        capture_output=True,
    ).stdout


def git_file(repo: Path, commit: str, path: str) -> bytes:
    raw = run(repo, "show", f"{commit}:{path}")
    blob = run(repo, "rev-parse", f"{commit}:{path}").decode().strip()
    actual_blob = (
        subprocess.run(
            ["git", "hash-object", "--stdin"],
            input=raw,
            check=True,
            capture_output=True,
        )
        .stdout.decode()
        .strip()
    )
    if blob != actual_blob:
        raise ValueError(f"Git blob drift: {repo} {commit}:{path}")
    return raw


def commit_receipt(repo: Path, name: str, commit: str) -> bytes:
    value = {
        "commit": commit,
        "parents": run(repo, "show", "-s", "--format=%P", commit)
        .decode()
        .strip()
        .split(),
        "repository": name,
        "schema": "vela.bounded-git-history-receipt.v1",
        "tree": run(repo, "show", "-s", "--format=%T", commit).decode().strip(),
    }
    return canonical(value) + b"\n"


def add_object(objects: Path, raw: bytes) -> str:
    root = sha256(raw)
    path = objects / root.removeprefix("sha256:")
    if path.exists() and path.read_bytes() != raw:
        raise ValueError("content-addressed object collision")
    path.write_bytes(raw)
    return root


def entry(
    objects: Path,
    *,
    raw: bytes,
    logical_path: str,
    kind: str,
    source: dict[str, Any],
) -> dict[str, Any]:
    return {
        "bytes": len(raw),
        "kind": kind,
        "logical_path": logical_path,
        "sha256": add_object(objects, raw),
        "source": source,
    }


def repo_file_entry(
    objects: Path,
    *,
    repo: Path,
    name: str,
    commit: str,
    path: str,
    expected_sha256: str | None = None,
) -> dict[str, Any]:
    raw = git_file(repo, commit, path)
    if (
        expected_sha256 is not None
        and hashlib.sha256(raw).hexdigest() != expected_sha256
    ):
        raise ValueError(f"SHA-256 drift: {name} {commit}:{path}")
    return entry(
        objects,
        raw=raw,
        logical_path=f"repositories/{name}@{commit}/{path}",
        kind="git_blob",
        source={"commit": commit, "path": path, "repository": name},
    )


def history_entry(
    objects: Path, *, repo: Path, name: str, commit: str
) -> dict[str, Any]:
    return entry(
        objects,
        raw=commit_receipt(repo, name, commit),
        logical_path=f"histories/{name}@{commit}.json",
        kind="bounded_git_history",
        source={"commit": commit, "repository": name},
    )


def packet_atoms(
    objects: Path,
    lean_proofs: Path,
    lean_correspondence: Path,
) -> None:
    candidate_prefixes = {
        "lean-correspondence-v0/cases/erdos-730/": "lean-correspondence-v0/cases/erdos-730/",
        "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/": "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/",
    }
    for packet_path in sorted((ROOT / "execution-packets").glob("*.json")):
        packet = json.loads(packet_path.read_bytes())
        for atom in packet["base_semantic_atoms"] + packet["derived_mechanism_atoms"]:
            path = atom["path"]
            raw: bytes | None = None
            for prefix, candidate_prefix in candidate_prefixes.items():
                if path.startswith(prefix):
                    raw = git_file(
                        lean_proofs,
                        CANDIDATE_COMMIT,
                        candidate_prefix + path.removeprefix(prefix),
                    )
                    break
            if raw is None and path in {"lean-toolchain", "lake-manifest.json"}:
                raw = git_file(lean_proofs, CANDIDATE_COMMIT, path)
            if raw is None and path.startswith("invalid-fixture/"):
                raw = (STAGE_A / path).read_bytes()
            if raw is None and path.startswith("cases/"):
                raw = git_file(lean_correspondence, CORRESPONDENCE_COMMIT, path)
            if raw is None:
                raise ValueError(f"unresolved packet atom: {path}")
            if len(raw) != atom["bytes"] or sha256(raw) != atom["sha256"]:
                raise ValueError(f"packet atom drift: {path}")
            add_object(objects, raw)


def erdos_entries(
    objects: Path, formal: Path, lean_proofs: Path
) -> list[dict[str, Any]]:
    packet = json.loads(
        git_file(
            lean_proofs,
            CANDIDATE_COMMIT,
            "lean-correspondence-v0/cases/erdos-730/packet.json",
        )
    )
    entries: list[dict[str, Any]] = []
    mappings = {
        "formal_conjectures": (formal, "formal-conjectures"),
        "lean_proofs": (lean_proofs, "lean-proofs"),
    }
    for key, (repo, name) in mappings.items():
        record = packet["roots"][key]
        commit = record["commit"]
        entries.append(history_entry(objects, repo=repo, name=name, commit=commit))
        for item in record["files"]:
            entries.append(
                repo_file_entry(
                    objects,
                    repo=repo,
                    name=name,
                    commit=commit,
                    path=item["path"],
                    expected_sha256=item["sha256"],
                )
            )
    entries.append(
        history_entry(
            objects,
            repo=lean_proofs,
            name="lean-proofs-candidate-packet",
            commit=CANDIDATE_COMMIT,
        )
    )
    return entries


def fc_entries(
    objects: Path,
    formal: Path,
    lean_eval: Path,
    lean_generator: Path,
    lean_proofs: Path,
) -> list[dict[str, Any]]:
    packet = json.loads(
        git_file(
            lean_proofs,
            CANDIDATE_COMMIT,
            "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/packet.json",
        )
    )
    roots = packet["roots"]
    entries: list[dict[str, Any]] = []
    source = roots["formal_conjectures_source"]
    source_commit = source["commit"]
    entries.append(
        history_entry(
            objects, repo=formal, name="formal-conjectures", commit=source_commit
        )
    )
    for path, digest in [
        (source["path"], source["sha256"]),
        ("lean-toolchain", source["lean_toolchain_sha256"]),
        ("lake-manifest.json", source["lake_manifest_sha256"]),
    ]:
        entries.append(
            repo_file_entry(
                objects,
                repo=formal,
                name="formal-conjectures",
                commit=source_commit,
                path=path,
                expected_sha256=digest,
            )
        )
    adapter = roots["formal_conjectures_adapter"]
    entries.append(
        history_entry(
            objects,
            repo=formal,
            name="formal-conjectures-adapter",
            commit=adapter["commit"],
        )
    )
    for path, _blob, digest in adapter["files"]:
        entries.append(
            repo_file_entry(
                objects,
                repo=formal,
                name="formal-conjectures-adapter",
                commit=adapter["commit"],
                path=path,
                expected_sha256=digest,
            )
        )
    target = roots["lean_eval_target"]
    entries.append(
        history_entry(
            objects, repo=lean_eval, name="lean-eval", commit=target["commit"]
        )
    )
    for path, digest in [
        ("lean-toolchain", target["lean_toolchain_sha256"]),
        ("lake-manifest.json", target["lake_manifest_sha256"]),
    ]:
        entries.append(
            repo_file_entry(
                objects,
                repo=lean_eval,
                name="lean-eval",
                commit=target["commit"],
                path=path,
                expected_sha256=digest,
            )
        )
    generator_commit = roots["generator"]["commit"]
    entries.append(
        history_entry(
            objects,
            repo=lean_generator,
            name="lean-eval-generator",
            commit=generator_commit,
        )
    )
    for path in ["schemas/request-v1.schema.json", "schemas/response-v1.schema.json"]:
        entries.append(
            repo_file_entry(
                objects,
                repo=lean_generator,
                name="lean-eval-generator",
                commit=generator_commit,
                path=path,
            )
        )
    entries.append(
        history_entry(
            objects,
            repo=lean_proofs,
            name="lean-proofs-candidate-packet",
            commit=CANDIDATE_COMMIT,
        )
    )
    return entries


def invalid_entries(objects: Path) -> list[dict[str, Any]]:
    raw = (STAGE_A / "invalid-fixture/fixture.json").read_bytes()
    return [
        entry(
            objects,
            raw=raw,
            logical_path="invalid-fixture/fixture.json",
            kind="fixture_lineage",
            source={
                "path": "paper/artifacts/lean-correspondence-stage-a-open-pilot/invalid-fixture/fixture.json",
                "producer_parent": "2d3b53575bd9465a0331b0e9fbf99510b05001f9",
            },
        )
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--formal-conjectures", type=Path, required=True)
    parser.add_argument("--lean-eval", type=Path, required=True)
    parser.add_argument("--lean-eval-generator", type=Path, required=True)
    parser.add_argument("--lean-proofs", type=Path, required=True)
    parser.add_argument("--lean-correspondence", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        shutil.rmtree(output)
    objects = output / "objects"
    objects.mkdir(parents=True)
    packet_atoms(objects, args.lean_proofs, args.lean_correspondence)
    cases = {
        "deliberately-invalid-byte-identity": {
            "participant_visible_case_id": "open-calibration-03",
            "supplemental_entries": invalid_entries(objects),
        },
        "erdos-730-affirmative-rhs": {
            "participant_visible_case_id": "open-calibration-01",
            "supplemental_entries": erdos_entries(
                objects, args.formal_conjectures, args.lean_proofs
            ),
        },
        "fc-leaneval-oeis-303656": {
            "participant_visible_case_id": "open-calibration-02",
            "supplemental_entries": fc_entries(
                objects,
                args.formal_conjectures,
                args.lean_eval,
                args.lean_eval_generator,
                args.lean_proofs,
            ),
        },
    }
    assignments: dict[str, str] = {}
    for packet_path in sorted((ROOT / "execution-packets").glob("*.json")):
        packet = json.loads(packet_path.read_bytes())
        schedule = json.loads((ROOT / "assignment-schedule.json").read_bytes())
        row = next(
            item
            for item in schedule["rows"]
            if item["source_assignment_id"] == packet["assignment_id"]
        )
        assignments[packet["assignment_id"]] = row["case_id"]
    value = {
        "assignment_cases": assignments,
        "cases": cases,
        "schema": "vela.lean-correspondence-evidence-source-catalog.v1",
        "source_commits": {
            "candidate_packet": CANDIDATE_COMMIT,
            "correspondence": CORRESPONDENCE_COMMIT,
        },
    }
    (output / "catalog.json").write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "catalog_root": sha256(canonical(value)),
                "objects": len(list(objects.iterdir())),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
