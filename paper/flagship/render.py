#!/usr/bin/env python3
"""Render the flagship paper with commit-bound deterministic PDF metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

PANDOC_VERSION = "pandoc 3.9"
PDFLATEX_VERSION = "pdfTeX 3.141592653-2.6-1.40.26 (TeX Live 2024)"
RESULT_SCHEMA = "vela.flagship-paper-render-result.v1"


class RenderError(RuntimeError):
    """Raised when an exact render prerequisite fails."""


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RenderError(f"{' '.join(command)} failed: {detail}")
    return completed.stdout


def first_line(command: list[str], root: Path) -> str:
    return run(command, cwd=root).splitlines()[0]


def file_root(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("output/pdf/vela-flagship-working-paper.pdf"),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parent.parent.parent
    source = root / "paper" / "flagship" / "manuscript.md"
    filter_path = root / "paper" / "filters" / "break-exact-values.lua"
    preamble = root / "paper" / "preamble.tex"
    output = args.output if args.output.is_absolute() else root / args.output

    if first_line(["pandoc", "--version"], root) != PANDOC_VERSION:
        raise RenderError(f"Pandoc must equal {PANDOC_VERSION}")
    if first_line(["pdflatex", "--version"], root) != PDFLATEX_VERSION:
        raise RenderError(f"pdfLaTeX must equal {PDFLATEX_VERSION}")
    if run(["git", "status", "--porcelain"], cwd=root):
        raise RenderError("Vela worktree must be clean")

    commit = run(["git", "rev-parse", "HEAD"], cwd=root).strip()
    tree = run(["git", "rev-parse", "HEAD^{tree}"], cwd=root).strip()
    source_date_epoch = run(
        ["git", "show", "-s", "--format=%ct", commit], cwd=root
    ).strip()
    if not source_date_epoch.isdigit():
        raise RenderError("HEAD commit timestamp is not an integer epoch")

    output.parent.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update(
        {
            "LANG": "C",
            "LC_ALL": "C",
            "SOURCE_DATE_EPOCH": source_date_epoch,
            "TZ": "UTC",
        }
    )
    run(
        [
            "pandoc",
            str(source),
            "--from=gfm+yaml_metadata_block",
            "--standalone",
            "--pdf-engine=pdflatex",
            "--lua-filter",
            str(filter_path),
            "--include-in-header",
            str(preamble),
            "-V",
            "papersize:letter",
            "-V",
            "geometry:margin=0.8in",
            "-V",
            "fontsize=10pt",
            "-V",
            "colorlinks=true",
            "-V",
            "linkcolor=black",
            "-V",
            "urlcolor=blue",
            "-o",
            str(output),
        ],
        cwd=root,
        env=environment,
    )
    result = {
        "schema": RESULT_SCHEMA,
        "vela_commit": commit,
        "vela_tree": tree,
        "source_date_epoch": int(source_date_epoch),
        "source_root": file_root(source),
        "filter_root": file_root(filter_path),
        "preamble_root": file_root(preamble),
        "pandoc_version": PANDOC_VERSION,
        "pdflatex_version": PDFLATEX_VERSION,
        "pdf_root": file_root(output),
        "pdf_bytes": output.stat().st_size,
        "output": str(output),
        "qualifying_clean_build": True,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RenderError as error:
        print(f"flagship paper render: {error}", file=sys.stderr)
        raise SystemExit(1) from error
