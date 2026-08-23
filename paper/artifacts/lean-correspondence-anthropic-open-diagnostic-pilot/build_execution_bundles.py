"""Materialize the six reviewed held bundles without provider contact."""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
VELA = ROOT.parents[2]
RUNTIME = ROOT.parent / "lean-correspondence-stage-a-runtime-qualification"
DEFAULT_BASE = Path("/private/tmp/vela-stage-a-runtime-qualification-bundle-v1")
DEFAULT_OUTPUT = Path("/private/tmp/vela-anthropic-open-diagnostic-held-v2")
QUALIFIER = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-maintained-v1/"
    "tools/evidence_qualification/qualification.py"
)
QUALIFIER_PYTHON = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-python-v1/.venv/bin/python"
)
ISSUED_AT = "2026-08-22T00:00:00Z"


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError(f"module unavailable: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def stage_cell(base: Path, target: Path, row: dict[str, Any]) -> dict[str, Any]:
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(base, target)
    config_path = target / "qualification.json"
    config = load(config_path)
    run_id = row["cell_id"]
    identity = {
        "registration_id": "anthropic-open-diagnostic-registration-v2",
        "assignment_id": row["cell_id"],
        "participant_id": row["participant_id"],
        "run_id": run_id,
        "condition": row["arm"],
        "prompt_root": row["prompt_root"],
        "packet_root": row["execution_packet_root"],
    }
    permit_relative = f"permit/{run_id}.permit.json"
    old_permit = target / config["participant_permit"]["permit"]
    permit = load(old_permit)
    permit.update(identity)
    permit.update(
        {
            "attempt": 1,
            "issued_at": ISSUED_AT,
            "status": "held",
            "consumed_at": None,
            "timeout_seconds": 1200,
        }
    )
    new_permit = target / permit_relative
    write(new_permit, permit)
    if old_permit != new_permit:
        old_permit.unlink()
    hold_path = target / config["participant_permit"]["hold"]
    hold = load(hold_path)
    hold.update(
        {
            "registration_id": identity["registration_id"],
            "assignment_id": identity["assignment_id"],
        }
    )
    write(hold_path, hold)
    config["participant_permit"] = {
        "hold": config["participant_permit"]["hold"],
        "permit": permit_relative,
        "consumed_permit": f"permit/{run_id}.permit.consumed.json",
        "identity": identity,
    }
    config["runtime"]["mounts"] = [
        {
            "source": str((target / "schemas").resolve()),
            "target": "/input",
            "read_only": True,
        },
        {
            "source": str(
                (target / "runtime/source/vendor/ca-certificates.crt").resolve()
            ),
            "target": "/etc/ssl/certs/ca-certificates.crt",
            "read_only": True,
        },
    ]
    config["self_verification"] = {
        "command": [
            str(QUALIFIER_PYTHON),
            str(QUALIFIER),
            "--bundle",
            str(target.resolve()),
        ],
        "qualifier_sha256": config["self_verification"]["qualifier_sha256"],
        "environment_prefix": str(QUALIFIER_PYTHON.parent.parent),
        "jsonschema_module": config["self_verification"]["jsonschema_module"],
    }
    write(config_path, config)

    input_dir = target / "execution/input"
    evidence_dir = target / "execution/offline-evidence"
    input_dir.mkdir(parents=True)
    evidence_dir.mkdir(parents=True)
    shutil.copyfile(
        target / "schemas/provider.json", input_dir / "provider-schema.json"
    )
    shutil.copyfile(ROOT / row["prompt_path"], input_dir / "prompt.txt")
    shutil.copyfile(ROOT / row["execution_packet_path"], input_dir / "packet.json")
    materializer = RUNTIME / "run_input_materialize.py"
    subprocess.run(
        [
            sys.executable,
            str(materializer),
            "--schema",
            str(input_dir / "provider-schema.json"),
            "--run",
            str(input_dir / "run.json"),
            "--receipt",
            str(input_dir / "materialization-receipt.json"),
            "--run-id",
            run_id,
            "--model",
            "claude-opus-5",
            "--prompt-file",
            str(input_dir / "prompt.txt"),
            "--packet-file",
            str(input_dir / "packet.json"),
        ],
        check=True,
    )
    return identity


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-bundle", type=Path, default=DEFAULT_BASE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--skip-preflight", action="store_true")
    args = parser.parse_args()
    base = args.base_bundle.resolve()
    output = args.output.resolve()
    if not base.is_dir() or not QUALIFIER.is_file() or not QUALIFIER_PYTHON.is_file():
        raise ValueError(
            "maintained base bundle or fixed qualifier environment missing"
        )
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    schedule = load(ROOT / "assignment-schedule.json")
    offline = load_module("diagnostic_offline_qualify", RUNTIME / "offline_qualify.py")
    sys.path.insert(0, str(QUALIFIER.parents[3]))
    qualifier = load_module("diagnostic_maintained_qualifier", QUALIFIER)
    receipts = []
    for row in schedule["rows"]:
        target = output / row["cell_id"]
        identity = stage_cell(base, target, row)
        if not args.skip_preflight:
            launchability = offline.verify_launchable_image(
                (target / "runtime/a.oci.tar").read_bytes(),
                "anthropic-messages-v1",
                qualifier,
                target / "execution/input",
                target / "execution/offline-evidence",
            )
            write(target / "execution/launchability.json", launchability)
        result = subprocess.run(
            [str(QUALIFIER_PYTHON), str(QUALIFIER), "--bundle", str(target)],
            check=True,
            capture_output=True,
        )
        receipt = json.loads(result.stdout)
        write(target / "execution/qualification-receipt.json", receipt)
        receipts.append(
            {
                "cell_id": row["cell_id"],
                "identity": identity,
                "participant_permit_root": receipt["participant_permit_root"],
                "qualification_root": receipt["qualification_root"],
                "status": receipt["status"],
            }
        )
    write(
        output / "bundle-index.json",
        {
            "cells": receipts,
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-held-bundle-index.v2",
            "status": "held_offline_qualified",
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
