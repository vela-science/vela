#!/usr/bin/env python3
"""Offline qualification for the one-shot container and frozen canary."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CANARY = ROOT / "neutral-canary"
RUNTIME = ROOT / "container-runtime"
IMAGE = "sha256:0ce56e0a4d72dc6ab26cdfcfc1d0280ac0c419dd687e26dda9312d4a09257285"


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def canonical_root(value: object) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def load(path: Path) -> object:
    return json.loads(path.read_text())


class RuntimeTests(unittest.TestCase):
    def test_all_frozen_roots_recompute(self) -> None:
        freeze = load(CANARY / "prelaunch-freeze.json")
        self.assertEqual(freeze["registration_root"], canonical_root(load(CANARY / "registration.json")))
        self.assertEqual(freeze["participant_configuration_root"], canonical_root(load(CANARY / "input/participant-configuration.json")))
        self.assertEqual(freeze["assignment_root"], canonical_root(load(CANARY / "input/assignment.json")))
        self.assertEqual(freeze["authorization_root"], canonical_root(load(CANARY / "authorization.json")))
        self.assertEqual(freeze["permit_root"], canonical_root(load(CANARY / "permit-template/neutral-canary-01.permit.json")))
        self.assertEqual(freeze["prompt_root"], digest((CANARY / "input/prompt.txt").read_bytes()))
        self.assertEqual(freeze["image_digest"], IMAGE)

    def test_neutral_prompt_excludes_study_facts(self) -> None:
        prompt = (CANARY / "input/prompt.txt").read_text().lower()
        for forbidden in ("git-documents", "bounded-calibration", "calibration-a", "yield-b", "stability-c", "installation-d", "aggregate-e"):
            self.assertNotIn(forbidden, prompt)

    def test_pilot_manifest_and_stop_are_closed(self) -> None:
        manifest = load(ROOT / "pilot-capture-manifest.json")
        self.assertEqual(manifest["started_runs"], 6)
        self.assertFalse(manifest["confirmatory_credit"])
        self.assertFalse(manifest["scoring_authorized"])
        self.assertEqual(len(manifest["files"]), 138)
        for item in manifest["files"]:
            content = (ROOT / "pilot-capture" / item["path"]).read_bytes()
            self.assertEqual(item["bytes"], len(content))
            self.assertEqual(item["sha256"], digest(content))
        stop = load(ROOT / "pilot-stop-record.json")
        self.assertEqual(stop["status"], "stopped_before_run_07")
        self.assertEqual(stop["unstarted_first_run_id"], "run-07")

    def test_source_has_one_shot_consume_before_spawn_and_no_scheduler(self) -> None:
        runner = (RUNTIME / "run-once.mjs").read_text()
        launcher = (RUNTIME / "launch-one.sh").read_text()
        self.assertLess(runner.index("renameSync(permitPath, consumedPath)"), runner.index('spawn("codex"'))
        self.assertNotIn("run-01 through run-16", runner + launcher)
        self.assertNotIn("for assignment", runner + launcher)
        self.assertIn('args.length !== 2 || args[0] !== "--run-id"', runner)
        self.assertIn('if [ "$#" -ne 6 ] || [ "$1" != "--run-id" ]', launcher)

    def test_default_hold_and_binding_failure_do_not_consume(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            input_dir = base / "input"
            permit_dir = base / "permit"
            evidence_dir = base / "evidence"
            shutil.copytree(CANARY / "input", input_dir)
            shutil.copytree(CANARY / "permit-template", permit_dir)
            evidence_dir.mkdir()
            shutil.copy(permit_dir / "hold-state.default.json", permit_dir / "hold-state.json")
            result = subprocess.run(
                [str(RUNTIME / "launch-one.sh"), "--run-id", "neutral-canary-01", IMAGE, str(input_dir), str(permit_dir), str(evidence_dir)],
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("launch_on_hold", result.stderr)
            self.assertTrue((permit_dir / "neutral-canary-01.permit.json").exists())
            self.assertFalse((permit_dir / "neutral-canary-01.permit.consumed.json").exists())

            shutil.copy(CANARY / "permit-template/hold-state.json", permit_dir / "hold-state.json")
            (input_dir / "prompt.txt").write_bytes((input_dir / "prompt.txt").read_bytes() + b"drift")
            result = subprocess.run(
                [str(RUNTIME / "launch-one.sh"), "--run-id", "neutral-canary-01", IMAGE, str(input_dir), str(permit_dir), str(evidence_dir)],
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("permit_prompt_root", result.stderr)
            self.assertTrue((permit_dir / "neutral-canary-01.permit.json").exists())
            self.assertFalse((permit_dir / "neutral-canary-01.permit.consumed.json").exists())


if __name__ == "__main__":
    unittest.main()
