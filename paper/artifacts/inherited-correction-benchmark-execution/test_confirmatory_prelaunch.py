"""Offline checks for the frozen replacement confirmatory prelaunch state."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
STUDY = ROOT / "confirmatory-study"
BENCHMARK = ROOT.parent / "inherited-correction-benchmark"
FREEZER = ROOT / "freeze-confirmatory-prelaunch.py"


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def load(path: Path) -> Any:
    return json.loads(path.read_text())


def packet_root(directory: Path) -> str:
    manifest = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        content = path.read_bytes()
        manifest.append(
            {
                "path": path.relative_to(directory).as_posix(),
                "bytes": len(content),
                "sha256": digest(content),
            }
        )
    return canonical_root(manifest)


class ConfirmatoryPrelaunchTests(unittest.TestCase):
    def test_passed_runtime_and_calibration_lineage_are_unchanged(self) -> None:
        freeze = load(STUDY / "prelaunch-freeze.json")
        paths = {
            "pilot_stop_record": ROOT / "pilot-stop-record.json",
            "pilot_capture_manifest": ROOT / "pilot-capture-manifest.json",
            "canary_01_prelaunch": ROOT / "neutral-canary/prelaunch-freeze.json",
            "canary_01_result": ROOT / "neutral-canary/canary-result.json",
            "canary_02_prelaunch": ROOT / "neutral-canary-02/prelaunch-freeze.json",
            "canary_02_result": ROOT / "neutral-canary-02/canary-result.json",
            "canary_03_prelaunch": ROOT / "neutral-canary-03/prelaunch-freeze.json",
            "canary_03_result": ROOT / "neutral-canary-03/canary-result.json",
        }
        for identity, path in paths.items():
            self.assertEqual(
                freeze["calibration_artifacts_unchanged"][identity],
                digest(path.read_bytes()),
            )
        canary_freeze = load(ROOT / "neutral-canary-03/prelaunch-freeze.json")
        registration = load(STUDY / "registration.json")
        self.assertEqual(registration["image_digest"], canary_freeze["image_digest"])
        self.assertEqual(
            registration["trust_bundle_bytes"], canary_freeze["trust_bundle_bytes"]
        )
        self.assertEqual(
            registration["trust_provenance_root"],
            canary_freeze["trust_provenance_root"],
        )

    def test_all_frozen_roots_and_files_recompute(self) -> None:
        freeze = load(STUDY / "prelaunch-freeze.json")
        self.assertEqual(
            freeze["registration_root"],
            canonical_root(load(STUDY / "registration.json")),
        )
        self.assertEqual(
            freeze["participant_configuration_root"],
            canonical_root(load(STUDY / "participant-configuration.json")),
        )
        self.assertEqual(
            freeze["assignment_root"],
            canonical_root(load(STUDY / "assignment-schedule.json")),
        )
        self.assertEqual(
            freeze["authorization_root"],
            canonical_root(load(STUDY / "authorization.json")),
        )
        for condition, expected in freeze[
            "condition_runtime_configuration_roots"
        ].items():
            self.assertEqual(
                expected,
                canonical_root(
                    load(
                        STUDY
                        / "conditions"
                        / condition
                        / "input/participant-configuration.json"
                    )
                ),
            )
        for run_id, expected in freeze["permit_roots"].items():
            self.assertEqual(
                expected,
                canonical_root(
                    load(STUDY / "permit-template" / f"{run_id}.permit.json")
                ),
            )
        for item in freeze["files"]:
            content = (STUDY / item["path"]).read_bytes()
            self.assertEqual(item["bytes"], len(content))
            self.assertEqual(item["sha256"], digest(content))

    def test_fixed_assignment_and_authorization_are_valid(self) -> None:
        assignment = load(STUDY / "assignment-schedule.json")
        authorization = load(STUDY / "authorization.json")
        rows = assignment["assignments"]
        self.assertEqual(len(rows), 16)
        self.assertEqual(len({row["run_id"] for row in rows}), 16)
        self.assertEqual(len({row["participant_instance_id"] for row in rows}), 16)
        self.assertEqual(sum(row["condition"] == "git-documents" for row in rows), 8)
        self.assertEqual(sum(row["condition"] == "vela" for row in rows), 8)
        self.assertEqual(
            [
                {
                    key: row[key]
                    for key in ("run_id", "participant_instance_id", "condition")
                }
                for row in rows
            ],
            authorization["assignments"],
        )
        spec = importlib.util.spec_from_file_location(
            "benchmark", BENCHMARK / "benchmark.py"
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        preregistration = load(BENCHMARK / "preregistration.json")
        self.assertEqual(
            module.validate_authorization(authorization, preregistration), authorization
        )

    def test_each_permit_is_exact_root_bound_and_unconsumed(self) -> None:
        freeze = load(STUDY / "prelaunch-freeze.json")
        assignment = load(STUDY / "assignment-schedule.json")
        rows = {row["run_id"]: row for row in assignment["assignments"]}
        for run_id, row in rows.items():
            permit = load(STUDY / "permit-template" / f"{run_id}.permit.json")
            condition = row["condition"]
            self.assertEqual(permit["registration_root"], freeze["registration_root"])
            self.assertEqual(permit["assignment_root"], freeze["assignment_root"])
            self.assertEqual(
                permit["participant_configuration_root"],
                freeze["condition_runtime_configuration_roots"][condition],
            )
            self.assertEqual(permit["prompt_root"], freeze["prompt_roots"][condition])
            self.assertEqual(permit["packet_root"], freeze["packet_roots"][condition])
            self.assertEqual(
                permit["participant_instance_id"], row["participant_instance_id"]
            )
            self.assertEqual(permit["attempt"], 1)
            self.assertFalse(
                (STUDY / "permit-template" / f"{run_id}.permit.consumed.json").exists()
            )
        for name in ("hold-state.default.json", "hold-state.json"):
            self.assertEqual(load(STUDY / "permit-template" / name)["status"], "hold")
        self.assertEqual(freeze["confirmatory_provider_calls"], 0)
        self.assertEqual(freeze["permits_consumed"], [])
        self.assertEqual(freeze["scheduler"], "none")

    def test_prompts_preserve_exact_packets_and_add_only_shared_schema(self) -> None:
        freeze = load(STUDY / "prelaunch-freeze.json")
        for condition in ("git-documents", "vela"):
            packet = BENCHMARK / "conditions" / condition
            self.assertEqual(packet_root(packet), freeze["packet_roots"][condition])
            prompt = (
                STUDY / "conditions" / condition / "input/prompt.txt"
            ).read_bytes()
            self.assertEqual(digest(prompt), freeze["prompt_roots"][condition])
            envelope = json.loads(prompt.split(b"\n\n", 1)[1])
            self.assertEqual(envelope["condition"], condition)
            files = envelope["virtual_files"]
            self.assertEqual(
                [item["path"] for item in files], sorted(item["path"] for item in files)
            )
            decoded = {
                item["path"]: base64.b64decode(item["content_base64"]) for item in files
            }
            expected_paths = {
                path.relative_to(packet).as_posix()
                for path in packet.rglob("*")
                if path.is_file()
            }
            self.assertEqual(set(decoded), expected_paths | {"response-schema.json"})
            for path in packet.rglob("*"):
                if path.is_file():
                    self.assertEqual(
                        decoded[path.relative_to(packet).as_posix()], path.read_bytes()
                    )
            self.assertEqual(
                decoded["response-schema.json"],
                (ROOT / "response-schema.json").read_bytes(),
            )

    def test_isolated_regeneration_is_byte_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "confirmatory-study"
            subprocess.run(
                ["python3", str(FREEZER), "--output", str(output)],
                check=True,
                capture_output=True,
            )
            committed = {
                path.relative_to(STUDY).as_posix(): path.read_bytes()
                for path in STUDY.rglob("*")
                if path.is_file()
            }
            regenerated = {
                path.relative_to(output).as_posix(): path.read_bytes()
                for path in output.rglob("*")
                if path.is_file()
            }
            self.assertEqual(regenerated, committed)


if __name__ == "__main__":
    unittest.main()
