#!/usr/bin/env python3
"""Deterministic and adversarial checks for the frozen observation packet."""

from __future__ import annotations

import copy
import json
import os
import shutil
import stat
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

import materialize
import scorer


PACKET = Path(__file__).resolve().parent
REPOSITORY = PACKET.parents[2]


def read_json(path: Path) -> dict:
    value = json.loads(path.read_bytes(), object_pairs_hook=scorer.strict_pairs)
    if not isinstance(value, dict):
        raise AssertionError(f"expected JSON object: {path}")
    return value


def input_copy(root: Path, arm: str) -> tuple[Path, Path]:
    manifest_path = PACKET / "input-manifests" / f"{arm}.json"
    manifest = read_json(manifest_path)
    input_root = root / "input"
    for row in manifest["files"]:
        source = REPOSITORY / row["source_path"]
        target = input_root / row["mounted_path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        target.chmod(0o444)
    return manifest_path, input_root


def tree_bytes(root: Path) -> dict[str, tuple[int, bytes]]:
    return {
        path.relative_to(root).as_posix(): (
            stat.S_IMODE(path.stat().st_mode),
            path.read_bytes(),
        )
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


class ObservationPacketTests(unittest.TestCase):
    def test_experiment_and_instruction_are_frozen(self) -> None:
        tree = subprocess.run(
            [
                "git",
                "rev-parse",
                "4e3f942dfff55ca5fd00b16f5e2ff41c156c3be6:conformance/experiments/claim-dependency-profile-v0",
            ],
            cwd=REPOSITORY,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        self.assertEqual(tree, "02bac2c905f7bf773313dea0096818a80fee2166")
        changed = subprocess.run(
            [
                "git",
                "diff",
                "--quiet",
                "4e3f942dfff55ca5fd00b16f5e2ff41c156c3be6",
                "--",
                "conformance/experiments/claim-dependency-profile-v0",
            ],
            cwd=REPOSITORY,
            check=False,
        )
        self.assertEqual(changed.returncode, 0)
        rows, _ = materialize.packet_manifest()
        self.assertEqual(
            materialize.render_instruction(rows),
            (PACKET / "task/instruction.md").read_bytes(),
        )

    def test_exact_answer_and_mutations(self) -> None:
        answer = read_json(PACKET / "canned-answer.json")
        key = PACKET / "answer-key.json"
        for arm in materialize.ARMS:
            with self.subTest(arm=arm), tempfile.TemporaryDirectory() as temporary:
                temporary_root = Path(temporary)
                manifest, inputs = input_copy(temporary_root, arm)
                answer_path = temporary_root / "answer.json"
                arm_answer = copy.deepcopy(answer)
                if arm == "rooted-source-plus-profile":
                    arm_answer["classifications"][0]["evidence"][0] = {
                        "path": "profile.json",
                        "pointer": "/dependencies/0",
                    }
                    arm_answer["classifications"][2]["evidence"][0] = {
                        "path": "profile.json",
                        "pointer": "/dependencies/1",
                    }
                answer_path.write_bytes(materialize.json_bytes(arm_answer))
                result = scorer.score(answer_path, key, manifest, inputs)
                self.assertTrue(result["safe_completion"])
                self.assertTrue(result["eligible"])
                self.assertEqual(result["affected_set_precision"], 1.0)
                self.assertEqual(result["affected_set_recall"], 1.0)
                self.assertEqual(
                    result["hidden_maintainer_interventions"]["status"], "not_measured"
                )
                self.assertIsNone(result["hidden_maintainer_interventions"]["value"])
                self.assertTrue(
                    all(
                        item["status"] == "not_measured"
                        for item in result["milestones"]
                    )
                )

                mutations = []
                wrong_status = copy.deepcopy(arm_answer)
                wrong_status["classifications"][0]["status"] = "unaffected"
                mutations.append((wrong_status, "affected_set_recall"))
                wrong_root = copy.deepcopy(arm_answer)
                wrong_root["classifications"][0]["claim_root"] = "sha256:" + "0" * 64
                mutations.append((wrong_root, "claim_bindings_exact"))
                wrong_nonclaim = copy.deepcopy(arm_answer)
                wrong_nonclaim["does_not_establish"][0] += " drift"
                mutations.append((wrong_nonclaim, "does_not_establish_exact"))
                pointer_mutations = (
                    ("claim", ("classifications", 0, "evidence", 3), "/claims/1/label"),
                    (
                        "repository",
                        ("classifications", 0, "evidence", 1),
                        "/repository_context/repository_id",
                    ),
                    (
                        "lifecycle",
                        ("classifications", 0, "evidence", 2),
                        "/claims/0/label",
                    ),
                    (
                        "requires",
                        ("classifications", 0, "evidence", 0),
                        "/dependencies/0/kind"
                        if arm == "rooted-source-plus-profile"
                        else "/statements/2/text",
                    ),
                    (
                        "no-edge",
                        ("classifications", 1, "evidence", 0),
                        "/statements/3/text",
                    ),
                    (
                        "verification",
                        ("stale_verifications", 0, "evidence", 0),
                        "/claims/1/verification/verification_id",
                    ),
                )
                for label, location, pointer in pointer_mutations:
                    mutation = copy.deepcopy(arm_answer)
                    target = mutation
                    for component in location:
                        target = target[component]
                    target["pointer"] = pointer
                    mutations.append(
                        (mutation, "evidence_pointers_valid", f"pointer:{label}")
                    )
                extra_key = copy.deepcopy(arm_answer)
                extra_key["unexpected"] = True
                mutations.append((extra_key, "schema_valid", "extra-key"))
                normalized = [
                    (*mutation, f"base:{index}") if len(mutation) == 2 else mutation
                    for index, mutation in enumerate(mutations)
                ]
                for mutation, field, label in normalized:
                    answer_path.write_bytes(materialize.json_bytes(mutation))
                    observed = scorer.score(answer_path, key, manifest, inputs)
                    self.assertFalse(observed["safe_completion"], (arm, label, field))
                    self.assertTrue(observed["eligible"])
                    if field == "affected_set_recall":
                        self.assertLess(observed[field], 1.0)
                    else:
                        self.assertFalse(observed[field])

                malformed_answers = (
                    b'{"schema":"broken",\n',
                    b'{"schema":NaN}',
                    b'{"schema":9007199254740992}',
                    b'{"schema":"\\ud800"}',
                )
                for malformed in malformed_answers:
                    with self.subTest(arm=arm, malformed=malformed):
                        answer_path.write_bytes(malformed)
                        malformed_result = scorer.score(
                            answer_path, key, manifest, inputs
                        )
                        self.assertEqual(
                            malformed_result["answer_raw_root"],
                            scorer.raw_root(malformed),
                        )
                        self.assertIsNone(malformed_result["answer_canonical_root"])
                        self.assertEqual(
                            malformed_result["stop_reason"], "answer_malformed"
                        )
                        self.assertTrue(malformed_result["eligible"])

                answer_path.unlink()
                missing = scorer.score(answer_path, key, manifest, inputs)
                self.assertEqual(missing["stop_reason"], "answer_missing")
                self.assertTrue(missing["eligible"])

                answer_path.write_bytes(materialize.json_bytes(arm_answer))
                excluded = scorer.score(
                    answer_path,
                    key,
                    manifest,
                    inputs,
                    "pre_output_infrastructure_failure",
                )
                self.assertFalse(excluded["eligible"])
                self.assertFalse(excluded["safe_completion"])

    def test_materialization_is_byte_identical_and_separated(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            first, second = parent / "first", parent / "second"
            development = ["--development-worktree"]
            self.assertEqual(
                materialize.main(["--output", str(first), *development]), 0
            )
            self.assertEqual(
                materialize.main(["--output", str(second), *development]), 0
            )
            self.assertEqual(tree_bytes(first), tree_bytes(second))
            study = read_json(first / "study-manifest.json")
            self.assertFalse(study["ready_for_participant_runs"])
            self.assertIsNone(study["execution_attestation"])
            self.assertEqual(study["source_packet"]["status"], "uncommitted_test_only")
            self.assertIsNone(study["source_packet"]["packet_tree"])
            self.assertEqual(
                study["run_order"], [run_id for run_id, _ in materialize.RUNS]
            )
            self.assertEqual(
                sorted(path.name for path in (first / "tasks").iterdir()),
                sorted(materialize.ARMS),
            )
            self.assertEqual(len(list((first / "jobs").glob("*.json"))), 4)
            for arm in materialize.ARMS:
                task = first / "tasks" / arm
                config = tomllib.loads((task / "task.toml").read_text())
                self.assertEqual(config["verifier"]["environment_mode"], "separate")
                self.assertEqual(config["verifier"]["network_mode"], "no-network")
                self.assertEqual(config["environment"]["network_mode"], "no-network")
                environment_files = {
                    path.relative_to(task / "environment").as_posix()
                    for path in (task / "environment").rglob("*")
                    if path.is_file()
                }
                manifest = read_json(PACKET / "input-manifests" / f"{arm}.json")
                observed_map = tuple(
                    (row["source_path"], row["mounted_path"])
                    for row in manifest["files"]
                )
                expected_map = (
                    materialize.COMMON_INPUT_MAP + (materialize.PROFILE_INPUT,)
                    if arm == "rooted-source-plus-profile"
                    else materialize.COMMON_INPUT_MAP
                )
                self.assertEqual(observed_map, expected_map)
                expected = {"Dockerfile", "answer.schema.json"} | {
                    f"input/{row['mounted_path']}" for row in manifest["files"]
                }
                self.assertEqual(environment_files, expected)
                self.assertNotIn("answer-key.json", environment_files)
                self.assertNotIn("scorer.py", environment_files)
                self.assertTrue((task / "tests/answer-key.json").is_file())
                self.assertTrue((task / "tests/scorer.py").is_file())
                instruction = (task / "instruction.md").read_bytes()
                self.assertEqual(
                    instruction, (PACKET / "task/instruction.md").read_bytes()
                )

            bad_task = parent / "bad-task"
            shutil.copytree(first, bad_task)
            task_path = bad_task / "tasks" / materialize.ARMS[0] / "task.toml"
            task_text = task_path.read_text()
            task_path.write_text(
                task_text.replace("\n[task]\n", "\nunknown = true\n\n[task]\n")
            )
            with self.assertRaises(materialize.ContractError):
                materialize.validate_raw_harbor_configs(bad_task)

            bad_job = parent / "bad-job"
            shutil.copytree(first, bad_job)
            job_path = bad_job / "jobs" / f"{materialize.RUNS[0][0]}.json"
            job = read_json(job_path)
            job["unknown"] = True
            job_path.write_bytes(materialize.json_bytes(job))
            with self.assertRaises(materialize.ContractError):
                materialize.validate_raw_harbor_configs(bad_job)

            attestation_path = parent / "attestation.json"
            attestation = {
                "schema": "vela.claim-dependency-observation-execution-attestation.v0",
                "harbor_version": "0.20.0",
                "docker_client_version": "test-only",
                "docker_server_version": "test-only",
                "agent_image_ids": {
                    arm: "sha256:" + "1" * 64 for arm in materialize.ARMS
                },
                "verifier_image_ids": {
                    arm: "sha256:" + "2" * 64 for arm in materialize.ARMS
                },
                "linux_codex_binary_raw_root": "sha256:" + "3" * 64,
                "task_roots": study["task_roots"],
                "shell_probe": "passed",
                "auth_mechanism": "codex_auth_json_transport",
                "codex_force_auth_json": "1",
                "codex_auth_json_path_set": False,
                "openai_api_key_set": False,
                "openai_base_url": None,
            }
            attestation_path.write_bytes(materialize.json_bytes(attestation))
            attestation_path.chmod(0o644)
            self.assertEqual(
                materialize.validate_attestation(attestation, study["task_roots"]),
                attestation,
            )
            alternate_auth = copy.deepcopy(attestation)
            alternate_auth["auth_mechanism"] = "openai_api_key_transport"
            with self.assertRaises(materialize.ContractError):
                materialize.validate_attestation(alternate_auth, study["task_roots"])
            final = parent / "final"
            self.assertEqual(
                materialize.main(
                    [
                        "--output",
                        str(final),
                        "--execution-attestation",
                        str(attestation_path),
                        "--development-worktree",
                    ]
                ),
                1,
            )
            self.assertFalse(final.exists())

    def test_hostile_file_and_path_refusals(self) -> None:
        for value in ("", "/absolute", "../escape", "a/../escape", "nul\x00path"):
            with (
                self.subTest(value=value),
                self.assertRaises(materialize.ContractError),
            ):
                materialize.relative_path(value, "test path")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            regular = root / "regular"
            regular.write_bytes(b"bytes")
            regular.chmod(0o644)
            self.assertEqual(materialize.read_regular(regular, 5, 0o644), b"bytes")
            regular.chmod(0o755)
            with self.assertRaises(materialize.ContractError):
                materialize.read_regular(regular, 5, 0o644)
            symlink = root / "symlink"
            symlink.symlink_to(regular)
            with self.assertRaises(materialize.ContractError):
                materialize.read_regular(symlink, 5)
            directory = root / "directory"
            directory.mkdir()
            with self.assertRaises(materialize.ContractError):
                materialize.read_regular(directory, 5)
            fifo = root / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(materialize.ContractError):
                materialize.read_regular(fifo, 5)

        output_in_repository = REPOSITORY / ".claim-dependency-observation-test"
        self.assertFalse(output_in_repository.exists())

        rows, _ = materialize.packet_manifest()
        drifted_rows = copy.deepcopy(rows)
        drifted_rows["README.md"]["raw_root"] = "sha256:" + "0" * 64
        with self.assertRaises(materialize.ContractError):
            materialize.packet_file(drifted_rows, "README.md")

        with tempfile.TemporaryDirectory() as temporary:
            packet_copy = Path(temporary) / "packet"
            shutil.copytree(PACKET, packet_copy)
            original_packet = materialize.PACKET
            try:
                materialize.PACKET = packet_copy
                directory_link = packet_copy / "unexpected-directory-link"
                directory_link.symlink_to(
                    packet_copy / "task", target_is_directory=True
                )
                with self.assertRaises(materialize.ContractError):
                    materialize.packet_manifest()
                directory_link.unlink()
                os.mkfifo(packet_copy / "unexpected-fifo")
                with self.assertRaises(materialize.ContractError):
                    materialize.packet_manifest()
            finally:
                materialize.PACKET = original_packet
        with mock.patch.dict(
            os.environ,
            {"GIT_DIR": "/dev/null", "GIT_WORK_TREE": "/"},
            clear=False,
        ):
            self.assertEqual(
                Path(
                    materialize.git_text(
                        ["rev-parse", "--show-toplevel"], "test repository root"
                    )
                ).resolve(),
                REPOSITORY,
            )
            self.assertEqual(
                materialize.main(
                    [
                        "--output",
                        str(output_in_repository),
                        "--development-worktree",
                    ]
                ),
                1,
            )
        self.assertFalse(output_in_repository.exists())


if __name__ == "__main__":
    unittest.main()
