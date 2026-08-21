#!/usr/bin/env python3
"""Freeze the replacement inherited-correction confirmatory prelaunch bytes."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
BENCHMARK = ROOT.parent / "inherited-correction-benchmark"
RUNTIME = ROOT / "container-runtime"
OUTPUT = ROOT / "confirmatory-replacement-study"
STOP_RECORD = ROOT / "confirmatory-execution/stopped-registration.json"
SEED = "f36aef6ede45ec476d8a1b1a0613efaa42d3e202fa51335a3ae9845bf20d61f5"
FROZEN_AT = "2026-08-21T19:21:13Z"
PERMIT_EXPIRY = "2026-09-22T23:59:59Z"
IMAGE = "sha256:1dee2374077c83e3dbdb2e09d32ef4fa3a414d200b800839857353e13d3c4e09"
BASE_IMAGE = "sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85"
TRUST_BUNDLE_PATH = "/etc/ssl/certs/ca-certificates.crt"
TRUST_BUNDLE_BYTES = (
    "sha256:714d457d580922dbf1d0be8bd35ba236a842b50b0072ae791582a19adef772a5"
)
BENCHMARK_REGISTRATION = json.loads((BENCHMARK / "preregistration.json").read_text())[
    "registration_root"
]
RUNTIME_PASS_PRODUCER = "4c7bd6a811bbd0cf1ebd357d3ad72abb9127442a"
RUNTIME_PASS_REVIEW = "2ebf1ad8cb0f5d16b7bcee8e5510f3aed5dc1395"
SCORING_PASS_PRODUCER = "7596c12291c22a5b4b81a1ab1eb49189318f57de"
SCORING_PASS_REVIEW = "59c104efefd163d2e4c86e1bd535ac5f7c03f17d"
RUNTIME_PASS_CANARY_RESULT = (
    "sha256:53d7b376ca90b0dc33db2c53703a63e0700068159b991c8250ce8f1f47fba018"
)
F04_BLOCKED_PRODUCER = "b4bd2542b2fb71944a0d1e7e487b007392c008b6"
F04_BLOCKED_REVIEW = "23986b9b02ea5ba1324cd9aad91545f969db8a56"
F04_BLOCKED_REGISTRATION = (
    "sha256:1ae5d204c369bdd71fc502192cafbbd17db89dc5544ee4cd2ebe3b75ac8147ec"
)
F04_BLOCKED_FREEZE = (
    "sha256:56eaacf7a1dc30402d86740744e42312deee9103eee7af78cce237e990156659"
)
F05_BLOCKED_PRODUCER = "f919654a7e55d336c5588719e962e2d67a699710"
F05_BLOCKED_REVIEW = "4dd3efc45d222e3d41defef38a5ce8306a1c6391"
F05_BLOCKED_REGISTRATION = (
    "sha256:78cc6e0154ec1baaa4eb86a15131ad9910802a5a2aeae2b82779de3c3ba6e67b"
)
F05_BLOCKED_FREEZE = (
    "sha256:db5f37730f8f0d0291c9bab467acc58a1c3f58210d662fa748ad8990f7aba37b"
)
PACKET_ROOTS = {
    "git-documents": "sha256:bdda8e39a17e50607a4587993dc7fe855fae9408dad2dd0ae11dc47ee281cb6e",
    "vela": "sha256:2bc904703cfd47419846e0a9771c5e9c3933dba5465ec9f48440d1850ace4c97",
}
CALIBRATION_BYTES = {
    "pilot_stop_record": "sha256:68a2816861f942fc105d60a7985c6dd9e8f5758e800d71dbdaafa9bac91498a5",
    "pilot_capture_manifest": "sha256:b9fd90197c99960f201053803e270beaf83e837afac9ef78268f958865d6e679",
    "canary_01_prelaunch": "sha256:d392ded1b1fe82abcc522ef92248de8b8b3c442bb9a825d83ecc6ed66bfa741a",
    "canary_01_result": "sha256:9fa7cce6b30abce0c156e8225315e3590c05dcfeff6c221e9f720f8e189cd701",
    "canary_02_prelaunch": "sha256:de9da5cf0b1ef92f0ac177ce707c4da3ec99db442fc3614d1cdcb3d88b6b479a",
    "canary_02_result": "sha256:ab6edf787916aedaf734cfc0ad0e8dda90a006097e81b3e7ed5beab0157ea702",
    "canary_03_prelaunch": "sha256:1405e33a4c5627c510d85174f602b6951c7b2c17e3903a28708867299682e1c4",
    "canary_03_result": "sha256:ea6c2a8c3da7aa3b09f32f7eb23414316f6ee1df6158e0eba44d4ac9c54bc1d7",
}


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))


def tree_manifest(
    directory: Path, excluded: set[str] | None = None
) -> list[dict[str, Any]]:
    excluded = excluded or set()
    files = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        relative = path.relative_to(directory).as_posix()
        if relative in excluded:
            continue
        content = path.read_bytes()
        files.append(
            {"path": relative, "bytes": len(content), "sha256": digest(content)}
        )
    return files


def packet_root(directory: Path) -> str:
    return canonical_root(tree_manifest(directory))


def schedule() -> list[dict[str, str]]:
    labels = [f"git-documents-{index:02d}" for index in range(1, 9)] + [
        f"vela-{index:02d}" for index in range(1, 9)
    ]
    ordered = sorted(
        labels, key=lambda label: hashlib.sha256(f"{SEED}:{label}".encode()).hexdigest()
    )
    assignments = []
    for index, label in enumerate(ordered, 1):
        condition = label.rsplit("-", 1)[0]
        assignments.append(
            {
                "run_id": f"replacement-run-{index:02d}",
                "participant_instance_id": f"replacement-sol-{index:02d}",
                "condition": condition,
                "packet_root": PACKET_ROOTS[condition],
            }
        )
    return assignments


def strict_overrides() -> list[str]:
    command = [
        "node",
        "--input-type=module",
        "-e",
        f"import {{STRICT_OVERRIDES}} from {json.dumps((RUNTIME / 'strict-config.mjs').as_uri())}; process.stdout.write(JSON.stringify(STRICT_OVERRIDES))",
    ]
    return json.loads(subprocess.run(command, check=True, capture_output=True).stdout)


def make_prompt(condition: str, destination: Path) -> None:
    with tempfile.TemporaryDirectory() as temporary:
        prompt_packet = Path(temporary) / "packet"
        shutil.copytree(BENCHMARK / "conditions" / condition, prompt_packet)
        shutil.copy2(
            ROOT / "response-schema.json", prompt_packet / "response-schema.json"
        )
        subprocess.run(
            [
                "python3",
                str(RUNTIME / "prepare-prompt.py"),
                "--packet",
                str(prompt_packet),
                "--output",
                str(destination),
                "--condition",
                condition,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )


def main() -> int:
    global OUTPUT
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    OUTPUT = args.output.resolve()
    if OUTPUT.exists():
        raise SystemExit(f"refusing to overwrite existing {OUTPUT}")
    stopped = json.loads(STOP_RECORD.read_text())
    stop_root = stopped.get("stop_root")
    stopped_without_root = dict(stopped)
    stopped_without_root.pop("stop_root", None)
    if stop_root != canonical_root(stopped_without_root):
        raise SystemExit("stopped registration root drifted")
    if (
        stopped.get("status") != "stopped_after_one_terminal_harness_non_result"
        or stopped.get("protected_adjudication_access_count") != 0
        or stopped.get("score_status") != "not_run_and_forbidden"
        or len(stopped.get("terminal_runs", [])) != 1
        or stopped["terminal_runs"][0].get("replacement_denominator_credit")
        is not False
        or len(stopped.get("unissued_runs", [])) != 15
    ):
        raise SystemExit("stopped registration disposition invalid")
    equivalence = json.loads((BENCHMARK / "input-equivalence.json").read_text())
    if equivalence["condition_packet_roots"] != PACKET_ROOTS:
        raise SystemExit("registered packet roots drifted")
    for condition, expected in PACKET_ROOTS.items():
        if packet_root(BENCHMARK / "conditions" / condition) != expected:
            raise SystemExit(f"packet bytes drifted: {condition}")

    assignments = schedule()
    seed_commitment = digest(bytes.fromhex(SEED))
    seed_record = {
        "schema": "vela.inherited-correction-assignment-seed.v3",
        "generated_by": "openssl rand -hex 32",
        "generated_at": FROZEN_AT,
        "seed_hex": SEED,
        "seed_commitment": seed_commitment,
        "allocation": "sort eight git-documents-NN and eight vela-NN labels by SHA-256(seed_hex + ':' + label); assign in order to replacement-run-01 through replacement-run-16",
        "replacement_outputs_observed_before_freeze": 0,
        "stopped_registration_terminal_outputs_observed_before_freeze": 1,
    }
    write(OUTPUT / "assignment-seed.json", seed_record)

    amendment = {
        "schema": "vela.inherited-correction-validator-amendment.v1",
        "status": "prospective_replacement_before_any_replacement_call",
        "frozen_at": FROZEN_AT,
        "stopped_registration_root": stop_root,
        "stopped_registration_bytes": digest(STOP_RECORD.read_bytes()),
        "stopped_terminal_execution_commit": stopped["terminal_execution_commit"],
        "stopped_terminal_runs_observed": 1,
        "stopped_terminal_run_id": stopped["terminal_runs"][0]["run_id"],
        "stopped_terminal_run_status": stopped["terminal_runs"][0]["harness_status"],
        "stopped_terminal_run_disposition": "sole stopped-registration denominator entry; retained exact; never retried, substituted, scored, or reinterpreted",
        "stopped_unissued_runs_forbidden": stopped["unissued_runs"],
        "protected_adjudication_access_count_before_amendment": 0,
        "replacement_provider_calls_before_amendment": 0,
        "defect": "default Ajv mode could not compile the exact Draft 2020-12 response schema",
        "repair": "select ajv/dist/2020.js from the existing locked Ajv 8.17.1 dependency in the actual one-shot entrypoint",
        "old_image_digest": stopped["image_digest"],
        "replacement_image_digest": IMAGE,
        "unchanged": [
            "response schema",
            "participant task",
            "Git/documents packet",
            "Vela packet",
            "prompt serialization",
            "scoring algorithm and positive gate",
            "gpt-5.6-sol high/default participant",
            "one prompt and one model turn",
            "no tools, MCP, web, memory, or plugins",
            "600-second timeout",
            "attempt=1 and zero retries or substitutions",
            "eight sessions per arm",
        ],
        "scientific_or_authority_effect": "none",
    }
    amendment_root = canonical_root(amendment)
    write(OUTPUT / "amendment.json", amendment)

    response_schema_bytes = digest((ROOT / "response-schema.json").read_bytes())
    strict = strict_overrides()
    strict_root = canonical_root(strict)
    prompts: dict[str, str] = {}
    for condition in PACKET_ROOTS:
        input_dir = OUTPUT / "conditions" / condition / "input"
        input_dir.mkdir(parents=True)
        shutil.copy2(ROOT / "response-schema.json", input_dir / "response-schema.json")
        make_prompt(condition, input_dir / "prompt.txt")
        prompts[condition] = digest((input_dir / "prompt.txt").read_bytes())

    runtime_source_root = canonical_root(
        [
            item
            for item in tree_manifest(RUNTIME)
            if "/node_modules/" not in f"/{item['path']}/"
        ]
    )
    schema_preflight_dir = OUTPUT / "offline-preflight/schema"
    schema_preflight_dir.mkdir(parents=True)
    subprocess.run(
        [str(RUNTIME / "preflight-schema.sh"), IMAGE, str(schema_preflight_dir)],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    schema_preflight_receipt = json.loads(
        (schema_preflight_dir / "receipt.json").read_text()
    )
    if schema_preflight_receipt != {
        "schema": "vela.inherited-correction-schema-preflight.v1",
        "validator": "ajv/dist/2020.js@8.17.1",
        "provider_contact_possible": False,
        "container_network": "none",
        "cases": {
            "valid": True,
            "unknown-field": False,
            "missing-required": False,
            "invalid-enum": False,
            "invalid-shape": False,
        },
    }:
        raise SystemExit("schema preflight receipt invalid")
    if (schema_preflight_dir / "provider-events.jsonl").read_bytes() != b"":
        raise SystemExit("schema preflight contacted provider")
    schema_preflight_root = canonical_root(tree_manifest(schema_preflight_dir))
    scoring_bindings = {
        "benchmark_registration_root": BENCHMARK_REGISTRATION,
        "validator_amendment_root": amendment_root,
        "benchmark_implementation_bytes": digest(
            (BENCHMARK / "benchmark.py").read_bytes()
        ),
        "benchmark_tests_bytes": digest((BENCHMARK / "test_benchmark.py").read_bytes()),
        "adjudication_root": "sha256:6b2e94c7bfce7c41353eb48cd4962243e3f177fdaccb8c7da48567d99dfca557",
        "response_schema_bytes": response_schema_bytes,
        "positive_gate": "registered unchanged in benchmark preregistration",
        "runtime_custody_bridge_bytes": digest(
            (ROOT / "confirmatory-custody.py").read_bytes()
        ),
        "runtime_custody_tests_bytes": digest(
            (ROOT / "test_confirmatory_custody.py").read_bytes()
        ),
    }
    registration = {
        "schema": "vela.inherited-correction-confirmatory-registration.v3",
        "status": "frozen_prelaunch_independent_review_required",
        "frozen_at": FROZEN_AT,
        "benchmark_registration_root": BENCHMARK_REGISTRATION,
        "validator_amendment_root": amendment_root,
        "runtime_pass_producer_commit": RUNTIME_PASS_PRODUCER,
        "runtime_pass_review_commit": RUNTIME_PASS_REVIEW,
        "runtime_pass_canary_result_root": RUNTIME_PASS_CANARY_RESULT,
        "scoring_pass_producer_commit": SCORING_PASS_PRODUCER,
        "scoring_pass_review_commit": SCORING_PASS_REVIEW,
        "f04_blocked_producer_commit": F04_BLOCKED_PRODUCER,
        "f04_blocked_review_commit": F04_BLOCKED_REVIEW,
        "f04_blocked_registration_root": F04_BLOCKED_REGISTRATION,
        "f04_blocked_freeze_root": F04_BLOCKED_FREEZE,
        "f04_blocked_disposition": "terminal held prelaunch evidence; zero calls; prospectively superseded only by the custody bridge",
        "prospective_custody_repair": "require exact consumed permit, terminal receipt, provider events, runtime response, and authorized shared-to-condition configuration mapping before capture or scoring",
        "f05_blocked_producer_commit": F05_BLOCKED_PRODUCER,
        "f05_blocked_review_commit": F05_BLOCKED_REVIEW,
        "f05_blocked_registration_root": F05_BLOCKED_REGISTRATION,
        "f05_blocked_freeze_root": F05_BLOCKED_FREEZE,
        "f05_blocked_disposition": "terminal held prelaunch evidence; zero calls; prospectively superseded only by capture-bound immutable scoring bytes",
        "prospective_score_snapshot_repair": "buffer and digest-check all 16 capture-bound run and response bytes before protected adjudication access; derive the scored capture root from and score only that immutable snapshot",
        "stopped_registration_root": stop_root,
        "stopped_registration_bytes": digest(STOP_RECORD.read_bytes()),
        "stopped_registration_disposition": "terminal stopped after one harness non-result; run 01 retained as its sole denominator entry; runs 02-16 forbidden; outside replacement denominator",
        "prospective_validator_repair": "use the locked Ajv 8.17.1 Draft 2020 implementation in the actual one-shot entrypoint; response schema and scientific content unchanged",
        "schema_preflight_root": schema_preflight_root,
        "image_digest": IMAGE,
        "image_build_command": "docker build --pull=false --provenance=false",
        "base_image_digest": BASE_IMAGE,
        "runtime_source_root": runtime_source_root,
        "trust_bundle_path": TRUST_BUNDLE_PATH,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "trust_provenance_root": "sha256:4cf369b944b78e938b4a8b2c824bc40fea0853ee1e8df0eced381429c3f00fa8",
        "condition_packet_roots": PACKET_ROOTS,
        "condition_prompt_roots": prompts,
        "response_schema_bytes": response_schema_bytes,
        "strict_overrides_root": strict_root,
        "assignment_seed_commitment": seed_commitment,
        "assignment_policy": "fresh 16 context-isolated participant instances; 8 per arm; fixed before provider access",
        "participant": {
            "provider": "openai-chatgpt-oauth-codex",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "high",
            "service_tier": "default",
            "codex_cli_version": "0.149.0",
        },
        "execution": {
            "sessions": 16,
            "sessions_per_condition": 8,
            "one_prompt": True,
            "one_model_turn": True,
            "tools": "none",
            "timeout_seconds": 600,
            "output_token_ceiling": 8192,
            "attempt": 1,
            "retries": 0,
            "substitutions": 0,
            "scheduler": "none",
            "default_state": "hold",
            "permit_policy": "one root-bound single-use permit per predetermined cell",
            "failures": "retained in fixed denominator without retry or substitution",
            "provider_usage": "telemetry only except genuine provider context/output-limit failure",
        },
        "authentication": {
            "mode": "read-only ChatGPT OAuth auth.json mount into disposable CODEX_HOME",
            "secret_retention": False,
            "automatic_purchase_or_top_up": False,
            "expected_platform_api_charge_usd": 0,
            "hard_contingency_ceiling_usd": 25,
        },
        "protected_scoring": "outside participant container; inaccessible until all 16 terminal captures and capture root freeze",
        "scoring_bindings": scoring_bindings,
        "calibration_only_bytes": CALIBRATION_BYTES,
        "replacement_confirmatory_calls_before_freeze": 0,
        "stopped_registration_terminal_calls_disclosed": 1,
        "scientific_or_authority_effect": "none",
    }
    registration_root = canonical_root(registration)
    write(OUTPUT / "registration.json", registration)

    runtime_assignment = {
        "schema": "vela.inherited-correction-runtime-assignment.v2",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "seed_commitment": seed_commitment,
        "assignments": assignments,
    }
    assignment_root = canonical_root(runtime_assignment)
    for condition in PACKET_ROOTS:
        write(
            OUTPUT / "conditions" / condition / "input/assignment.json",
            runtime_assignment,
        )
    write(OUTPUT / "assignment-schedule.json", runtime_assignment)

    condition_configuration_roots: dict[str, str] = {}
    for condition in PACKET_ROOTS:
        configuration = {
            "schema": "vela.inherited-correction-oci-participant-configuration.v3",
            "registration_root": registration_root,
            "image_digest": IMAGE,
            "base_image_digest": BASE_IMAGE,
            "codex_cli_version": "0.149.0",
            "schema_validator": "ajv/dist/2020.js@8.17.1",
            "authentication": "read-only ChatGPT OAuth auth.json mount into disposable CODEX_HOME",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "high",
            "service_tier": "default",
            "trust_bundle_path": TRUST_BUNDLE_PATH,
            "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
            "prompt_root": prompts[condition],
            "response_schema_bytes": response_schema_bytes,
            "strict_overrides_root": strict_root,
            "strict_overrides": strict,
            "one_prompt": True,
            "one_model_turn": True,
            "tools": "none",
            "tool_boundary": "supported disables plus immediate streaming abort and terminal failure on any tool event",
            "workdir": "empty read-only participant workdir",
            "store": "ephemeral",
            "timeout_seconds": 600,
            "output_token_ceiling": 8192,
            "provider_usage_disposition": "cost telemetry only; only genuine provider context/output-limit failure invalidates",
            "attempt": 1,
            "retries": 0,
        }
        path = (
            OUTPUT / "conditions" / condition / "input/participant-configuration.json"
        )
        write(path, configuration)
        condition_configuration_roots[condition] = canonical_root(configuration)

    study_configuration = {
        "schema": "vela.inherited-correction-fixed-participant-configuration.v3",
        "registration_root": registration_root,
        "fixed_across_all_sessions": True,
        "condition_specific_fields": [
            "packet_root",
            "prompt_root",
            "condition_runtime_configuration_root",
        ],
        "condition_runtime_configuration_roots": condition_configuration_roots,
        "shared_runtime": {
            "image_digest": IMAGE,
            "codex_cli_version": "0.149.0",
            "schema_validator": "ajv/dist/2020.js@8.17.1",
            "provider": "openai-chatgpt-oauth-codex",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "high",
            "service_tier": "default",
            "strict_overrides_root": strict_root,
            "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
            "timeout_seconds": 600,
            "output_token_ceiling": 8192,
            "attempt": 1,
            "retries": 0,
            "substitutions": 0,
            "tools": "none",
            "one_model_turn": True,
        },
    }
    study_configuration_root = canonical_root(study_configuration)
    write(OUTPUT / "participant-configuration.json", study_configuration)

    benchmark_assignments = [
        {key: item[key] for key in ("run_id", "participant_instance_id", "condition")}
        for item in assignments
    ]
    authorization = {
        "schema": "vela.inherited-correction-run-authorization.v1",
        "registration_root": BENCHMARK_REGISTRATION,
        "status": "authorized",
        "authorized_by": "user via coordinator task 01a024dc-f015-7950-be0f-181931282ebc",
        "authorized_at": FROZEN_AT,
        "participant_class": "gpt-5.6-sol high/default through pinned Codex 0.149.0 OAuth container runtime",
        "participant_configuration_root": study_configuration_root,
        "assignment_seed_commitment": seed_commitment,
        "max_sessions": 16,
        "assignments": benchmark_assignments,
    }
    authorization_root = canonical_root(authorization)
    write(OUTPUT / "authorization.json", authorization)
    configuration_mapping = {
        "schema": "vela.inherited-correction-authorized-configuration-mapping.v1",
        "status": "authorized",
        "authorization_root": authorization_root,
        "shared_study_configuration_root": study_configuration_root,
        "condition_runtime_configuration_roots": condition_configuration_roots,
    }
    configuration_mapping_root = canonical_root(configuration_mapping)
    write(OUTPUT / "configuration-mapping.json", configuration_mapping)

    permit_roots = {}
    for item in assignments:
        permit = {
            "schema": "vela.inherited-correction-launch-permit.v1",
            "status": "authorized",
            "expires_at": PERMIT_EXPIRY,
            "registration_root": registration_root,
            "image_digest": IMAGE,
            "participant_configuration_root": condition_configuration_roots[
                item["condition"]
            ],
            "assignment_root": assignment_root,
            "run_id": item["run_id"],
            "condition": item["condition"],
            "participant_instance_id": item["participant_instance_id"],
            "prompt_root": prompts[item["condition"]],
            "packet_root": item["packet_root"],
            "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
            "attempt": 1,
        }
        permit_roots[item["run_id"]] = canonical_root(permit)
        write(OUTPUT / "permit-template" / f"{item['run_id']}.permit.json", permit)
    hold = {
        "schema": "vela.inherited-correction-hold.v1",
        "status": "hold",
        "reason": "default; independent prelaunch PASS and one explicit exact-run release required before any permit consumption",
        "updated_at": FROZEN_AT,
    }
    write(OUTPUT / "permit-template/hold-state.default.json", hold)
    write(OUTPUT / "permit-template/hold-state.json", hold)

    freeze = {
        "schema": "vela.inherited-correction-confirmatory-prelaunch-freeze.v3",
        "status": "frozen_prelaunch_0_of_16_independent_review_required",
        "frozen_at": FROZEN_AT,
        "benchmark_registration_root": BENCHMARK_REGISTRATION,
        "validator_amendment_root": amendment_root,
        "registration_root": registration_root,
        "participant_configuration_root": study_configuration_root,
        "condition_runtime_configuration_roots": condition_configuration_roots,
        "authorized_configuration_mapping_root": configuration_mapping_root,
        "assignment_seed_commitment": seed_commitment,
        "assignment_root": assignment_root,
        "authorization_root": authorization_root,
        "packet_roots": PACKET_ROOTS,
        "prompt_roots": prompts,
        "permit_roots": permit_roots,
        "image_digest": IMAGE,
        "runtime_source_root": runtime_source_root,
        "trust_bundle_bytes": TRUST_BUNDLE_BYTES,
        "strict_overrides_root": strict_root,
        "scoring_bindings_root": canonical_root(scoring_bindings),
        "schema_preflight_root": schema_preflight_root,
        "stopped_registration_root": stop_root,
        "stopped_registration_bytes": digest(STOP_RECORD.read_bytes()),
        "runtime_pass_review_commit": RUNTIME_PASS_REVIEW,
        "scoring_pass_review_commit": SCORING_PASS_REVIEW,
        "f04_blocked_review_commit": F04_BLOCKED_REVIEW,
        "f05_blocked_review_commit": F05_BLOCKED_REVIEW,
        "confirmatory_provider_calls": 0,
        "permits_consumed": [],
        "hold_status": "hold",
        "scheduler": "none",
        "calibration_artifacts_unchanged": CALIBRATION_BYTES,
        "files": tree_manifest(OUTPUT, {"prelaunch-freeze.json"}),
        "claim_ceiling": "prospective runtime and custody qualification only; no empirical lift, scientific acceptance, Standing, authority, or Decision effect",
    }
    write(OUTPUT / "prelaunch-freeze.json", freeze)
    print(
        json.dumps(
            {
                key: value
                for key, value in freeze.items()
                if key.endswith("_root")
                or key.endswith("_roots")
                or key in {"image_digest", "trust_bundle_bytes", "status"}
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
