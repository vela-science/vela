#!/usr/bin/env python3
"""Run the frozen v3 same-model usability instrumentation."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import time
from pathlib import Path


HERE = Path(__file__).resolve().parent
PARENT = HERE.parent
OUTPUT = HERE / "sessions"
CASES = {
    "flat": PARENT / "flat-baseline.v1.md",
    "dossier": HERE / "dossier-view.v3.md",
}


def main() -> int:
    OUTPUT.mkdir(exist_ok=True)
    observations: dict[str, object] = {
        "schema": "vela.result-dossier-usability-observations.v3",
        "iteration": 3,
        "sessions": [],
    }
    for arm, case_path in CASES.items():
        for index in range(1, 5):
            with tempfile.TemporaryDirectory(prefix=f"vela-dossier-v3-{arm}-") as raw:
                root = Path(raw)
                shutil.copyfile(case_path, root / "case.md")
                shutil.copyfile(PARENT / "questions.v1.md", root / "questions.md")
                shutil.copyfile(PARENT / "answer-schema.v1.json", root / "schema.json")
                answer_path = root / "answer.json"
                command = [
                    "codex",
                    "exec",
                    "--ephemeral",
                    "--ignore-user-config",
                    "--ignore-rules",
                    "--sandbox",
                    "read-only",
                    "--skip-git-repo-check",
                    "-m",
                    "gpt-5.6-sol",
                    "-c",
                    'model_reasoning_effort="high"',
                    "-C",
                    str(root),
                    "--output-schema",
                    str(root / "schema.json"),
                    "--output-last-message",
                    str(answer_path),
                    "Read questions.md and answer them using only case.md. Do not inspect parent paths, prior context, or the network. Return only the required JSON object.",
                ]
                started = time.monotonic()
                result = subprocess.run(
                    command,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=180,
                    check=False,
                )
                elapsed = round(time.monotonic() - started, 2)
                if result.returncode != 0 or not answer_path.exists():
                    print(result.stderr)
                    raise RuntimeError(f"{arm}-{index} failed with exit {result.returncode}")
                answer = json.loads(answer_path.read_text(encoding="utf-8"))
                destination = OUTPUT / f"{arm}-{index}.json"
                destination.write_text(
                    json.dumps(answer, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                observations["sessions"].append(
                    {
                        "arm": arm,
                        "index": index,
                        "wall_seconds": elapsed,
                        "output": f"sessions/{destination.name}",
                    }
                )
                print(f"{arm}-{index}: {elapsed:.2f}s")
    (HERE / "observations.v3.json").write_text(
        json.dumps(observations, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
