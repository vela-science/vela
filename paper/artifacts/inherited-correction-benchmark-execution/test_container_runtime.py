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
CANARY_02 = ROOT / "neutral-canary-02"
RUNTIME = ROOT / "container-runtime"
IMAGE_01 = "sha256:0ce56e0a4d72dc6ab26cdfcfc1d0280ac0c419dd687e26dda9312d4a09257285"
IMAGE_02 = "sha256:13b753749787d68d628cea899f6b9875c0fc51c43877599b9aabf2009fe83388"


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
        self.assertEqual(freeze["image_digest"], IMAGE_01)

    def test_canary_02_frozen_roots_recompute(self) -> None:
        freeze = load(CANARY_02 / "prelaunch-freeze.json")
        self.assertEqual(freeze["amendment_root"], canonical_root(load(CANARY_02 / "amendment.json")))
        self.assertEqual(freeze["registration_root"], canonical_root(load(CANARY_02 / "registration.json")))
        self.assertEqual(freeze["participant_configuration_root"], canonical_root(load(CANARY_02 / "input/participant-configuration.json")))
        self.assertEqual(freeze["assignment_root"], canonical_root(load(CANARY_02 / "input/assignment.json")))
        self.assertEqual(freeze["authorization_root"], canonical_root(load(CANARY_02 / "authorization.json")))
        self.assertEqual(freeze["permit_root"], canonical_root(load(CANARY_02 / "permit-template/neutral-canary-02.permit.json")))
        self.assertEqual(freeze["prompt_root"], digest((CANARY_02 / "input/prompt.txt").read_bytes()))
        self.assertEqual(freeze["image_digest"], IMAGE_02)
        amendment = load(CANARY_02 / "amendment.json")
        self.assertEqual(amendment["parent_canary_commit"], "2c7aed112f9701b58b8373c156c244b68e616886")
        self.assertEqual(amendment["parent_canary_disposition"], "terminal calibration evidence; unchanged; no retry")

    def test_canary_02_terminal_capture_is_closed(self) -> None:
        result = load(CANARY_02 / "canary-result.json")
        self.assertEqual(result["status"], "non_result_timeout_before_model_response")
        self.assertEqual(result["attempt"], 1)
        self.assertEqual(result["retries"], 0)
        self.assertTrue(result["permit_consumed"])
        self.assertTrue(result["provider_execution_started"])
        self.assertEqual(result["model_responses"], 0)
        self.assertEqual(result["tool_calls"], 0)
        self.assertFalse(result["confirmatory_denominator_credit"])
        self.assertFalse(result["confirmatory_registration_frozen"])
        self.assertFalse(result["credential_retained"])
        self.assertEqual(result["container_teardown"], "clean")
        for item in result["capture_files"]:
            self.assertEqual(item["bytes"], digest((CANARY_02 / item["path"]).read_bytes()))
        receipt = load(CANARY_02 / "capture/evidence/terminal-receipt.json")
        self.assertEqual(receipt["status"], "non_result")
        self.assertEqual(receipt["validation_error"], "timeout")
        self.assertTrue(receipt["process_timed_out"])
        self.assertEqual(receipt["timeout_seconds"], 600)
        self.assertIsNone(receipt["response_bytes"])
        self.assertFalse(receipt["credential_retained"])

    def test_strict_preflight_and_legacy_regression_are_offline(self) -> None:
        for mode, expected_exit in (("corrected", 0), ("legacy", 1)):
            receipt = load(CANARY_02 / f"offline-preflight/{mode}/receipt.json")
            self.assertTrue(receipt["strict_parse_passed"])
            self.assertFalse(receipt["provider_contact_possible"])
            self.assertEqual(receipt["process_exit_code"], expected_exit)
            self.assertEqual((CANARY_02 / f"offline-preflight/{mode}/provider-events.jsonl").read_bytes(), b"")
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            for mode in ("corrected", "legacy"):
                output = temporary_path / mode
                output.mkdir()
                subprocess.run(
                    [str(RUNTIME / "preflight-config.sh"), mode, IMAGE_02, str(output)],
                    check=True,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual((output / "provider-events.jsonl").read_bytes(), b"")
                self.assertTrue(load(output / "receipt.json")["strict_parse_passed"])

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
            shutil.copytree(CANARY_02 / "input", input_dir)
            shutil.copytree(CANARY_02 / "permit-template", permit_dir)
            evidence_dir.mkdir()
            shutil.copy(permit_dir / "hold-state.default.json", permit_dir / "hold-state.json")
            result = subprocess.run(
                [str(RUNTIME / "launch-one.sh"), "--run-id", "neutral-canary-02", IMAGE_02, str(input_dir), str(permit_dir), str(evidence_dir)],
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("launch_on_hold", result.stderr)
            self.assertTrue((permit_dir / "neutral-canary-02.permit.json").exists())
            self.assertFalse((permit_dir / "neutral-canary-02.permit.consumed.json").exists())

            shutil.copy(CANARY_02 / "permit-template/hold-state.json", permit_dir / "hold-state.json")
            (input_dir / "prompt.txt").write_bytes((input_dir / "prompt.txt").read_bytes() + b"drift")
            result = subprocess.run(
                [str(RUNTIME / "launch-one.sh"), "--run-id", "neutral-canary-02", IMAGE_02, str(input_dir), str(permit_dir), str(evidence_dir)],
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("permit_prompt_root", result.stderr)
            self.assertTrue((permit_dir / "neutral-canary-02.permit.json").exists())
            self.assertFalse((permit_dir / "neutral-canary-02.permit.consumed.json").exists())


if __name__ == "__main__":
    unittest.main()
