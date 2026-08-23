"""Generate the deterministic held Anthropic-only diagnostic package."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any

import derive_execution_packet as packet_derivation

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
VELA = ROOT.parents[2]
STAGE_A = ROOT.parent / "lean-correspondence-stage-a-open-pilot"
RUNTIME = ROOT.parent / "lean-correspondence-stage-a-runtime-qualification"
V4 = ROOT.parent / "lean-correspondence-stage-a-anthropic-neutral-calibration-v4"
FOUNDRY = ROOT.parent / "lean-correspondence-foundry-study"
REAL_CORRECTION = ROOT.parent / "real-correction-study"

SCHEMA_ROOT = "sha256:b2d9bee1c76bc1f25f134fd50697f4e4a820a36bd61a84081edd5c542d749268"
STAGE_A_ARTIFACT_ROOT = (
    "sha256:f89d335912adbbd0e3b3f1cb98ec3f4fa78a27f3742652ac7244eaa86ed6aca8"
)
STAGE_A_ASSIGNMENT_ROOT = (
    "sha256:11fbe1785066cdc4183028cb71b26e2eedf9cb5fb35163440237f83470930b48"
)
STAGE_A_PERMIT_SET_ROOT = (
    "sha256:75bfa627a70465ae7b1ffe8dbf07d980db1f58ed3e68d7745fae22f851c12f1c"
)
STAGE_A_CASE_SELECTION_BYTES = (
    "sha256:fe57578532a02c93b811059a97b1ccd133ee291f27aa2d4865763005b80126b3"
)
STAGE_A_COMMIT = "c4a90b63d2f35c16876ff8b36f48742ae2c6ea7d"
STAGE_A_TREE = "af0cd1e65bf0efb1ca60897f47bab1cbc6e69232"
METHOD_COMMIT = "8a999f5e8ca543531f5e1241fbdef391c78d068a"
METHOD_TREE = "85b43ec7023d16cac404a6f8b6c8d8117584027a"
METHOD_ARTIFACT_ROOT = (
    "sha256:2d909b874eedc765546010e799d6fde709c88f3fcc623b45ab46130c3dfa68e4"
)
RUNTIME_ARTIFACT_ROOT = (
    "sha256:ff310cd5c3bcefa5ece6589df441b1c0c2f115333013b1c42fa92629853f4a64"
)
RUNTIME_RECORD_ROOT = (
    "sha256:c8b0126bb69c69d48836d3c36652dd64608f4596347577a1b6b0c91657e01db2"
)
RUNTIME_REGISTRATION_ROOT = (
    "sha256:ce4330143a091a2602ad74287fac510fad8e687fba4af23d561c118d833b46fd"
)
RUNTIME_SOURCE_ROOT = (
    "sha256:e37927ac5eb1085eeb47b4ee7fc60d63ba95b1a3c7b8876a5746f35ec3233614"
)
ANTHROPIC_CONFIGURATION_ROOT = (
    "sha256:fe8c8a8f320f8179f343202403d3cf37b50f77c2b38495d2dc9ef739ab34f4fa"
)
ANTHROPIC_QUALIFICATION_ROOT = (
    "sha256:cfcfa46ba16d8c20b80ef8170867c3b55aa76a1bbfa88816b32219d2121bcf7b"
)
ANTHROPIC_TOOL_POLICY_ROOT = (
    "sha256:c98932d0d5bcd5956b8c578f0f42bd3c94e827d5c5ca5b8034ca9bfd1799ffd2"
)
ANTHROPIC_IMAGE_DIGEST = (
    "sha256:3a8ee2d16720fc4ff91f6fd54ad698cd18aa7e387e3ce78a0ea751f7cf78dda7"
)
ANTHROPIC_IMAGE_ARCHIVE = (
    "sha256:779b51ed33cacbb2df756a0068c26f2b05fb4b8823f4b4bd04e1fd825374f5eb"
)
ANTHROPIC_IMAGE_CONFIG = (
    "sha256:3754bc0e038435c0eba42764209740fc76299d6e99a29c716680d4562a79c066"
)
ANTHROPIC_PROVIDER_SCHEMA = (
    "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
)
ANTHROPIC_RUNNER = (
    "sha256:fded48fc1d556d6b8d12aa1bf9b3233dc3a0872050a7b60f1cea22d6f897eede"
)
ANTHROPIC_BRIDGE = (
    "sha256:9c159b013640fbc9676eae828233f06b198d3afbfe564a01012c921fddf711de"
)
ANTHROPIC_CONTRACT = (
    "sha256:9629b9a7467eda0a3991a718f3b652c9b90dd8d0ea049da31f19510510a4857c"
)
ANTHROPIC_LAUNCHABILITY = (
    "sha256:2ec169cafa34959cb0e8457530a0afee80c3647b47431e24469a5f1311a3f9a3"
)
ANTHROPIC_OFFLINE_VALIDATION = (
    "sha256:5464cd11e3c1300eae021325363a138e4af796cac23efa7d8d1db5bd418926fe"
)
ANTHROPIC_MATERIALIZATION = (
    "sha256:54b7114d81fe8f09c90a548065c23cc80bb9d31a1086341e446dd0d67df82628"
)
ANTHROPIC_TRANSPORT_CUSTODY = (
    "sha256:fe9a30521be317f05404aef6e239605021aa8877f87b0bc1e40cb5062c396dcd"
)
NEUTRAL_PACKET_ROOT = (
    "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
)
NEUTRAL_PROMPT_ROOT = (
    "sha256:3443fa942b90f84718cc4e6918ebf6d121ebc40cf58f5b3c610f4e983c4d4ed9"
)
NEUTRAL_CONTENT_EQUIVALENCE_ROOT = (
    "sha256:1818fb5da3c0b3c57f24083eaa54acb448ce9cd14f59cbd0bd0d2aaef2dac8b8"
)
V4_REQUEST_ROOT = (
    "sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c"
)
V4_PERMIT_ROOT = (
    "sha256:dfc9f20e029b7ea51eb28c6b3d81f70eace063c681d56d2c9ce7356b3dbe8b63"
)
V4_RESPONSE_ROOT = (
    "sha256:95bf0c205c10167f57d769a0f77daef57ede2db3a6061464957c496e50eddc46"
)
V4_EXECUTION_ARTIFACT_ROOT = (
    "sha256:58f2995e8993045d9f9653371c1b49a770e23c59ca85d79cc26ff536a1537a0a"
)
V4_AMENDED_ARTIFACT_ROOT = (
    "sha256:958817c1de7d31497846294208b7178d785a4081e971b578cfd6cdbb8e4fca4a"
)
V4_AMENDMENT_COMMIT = "a9ee9e43a2152abf7267de9d38e57049ae64b499"
V4_AMENDMENT_TREE = "76d4d969177a19ec0bbb0b896f6cdfe8ffda5e58"
V4_REVIEW_COMMIT = "42f5c86bb337dd8a87507c5bb85ffe02a3829afc"
V4_REVIEW_TREE = "28a881f125cd2965dbf03fc44c0388f4b7fd495f"
V4_REVIEW_REPORT = (
    "sha256:31f66a81b62a7b7bc2f3402e0146f67013b316f9576ded2e20615b2b6cfcc580"
)
V4_REVIEW_VERDICT = (
    "sha256:0694575d8f370bd7e9c8113ff68d24cf383e155b17a987feb918e53c9fdc3333"
)
PRIOR_RESULT_REVIEW_COMMIT = "e6d8348bea3a57e88c5f9426d44a480b7a026fbd"
PRIOR_RESULT_REVIEW_TREE = "d4c6f9063b317be2a536af8a25344c2fa9931bbe"
PRIOR_RESULT_REVIEW_REPORT = (
    "sha256:63b058d0aadbaa9abb1a43e4b5598072e003bba6dbff0c10f65e2f5f52075a7c"
)
PRIOR_RESULT_REVIEW_VERDICT = (
    "sha256:fb79caf645df89df528c92959275d002871b6347c10e2cd833f431118798edf3"
)
PRIOR_RESULT_COMMIT = "4524c8f776943a267e04e03e9a237ecaed14bc2c"
PRIOR_RESULT_TREE = "4d5650a999ac0be59e71d5bd664e885cad5192c7"
PRIOR_RESULT_BYTES = (
    "sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372"
)
V2_ROOT = "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b"
V3_ROOT = "sha256:63cbbdf6ae6c7e906268b31f33198d06b8db0757e6db48b6187286cacd08dcb9"
SEED = b"vela-anthropic-open-diagnostic-pilot-v1\n"
CREDENTIAL_PATH = "/Users/williamblair/episteme/atlas-platform/apps/radar/.env.local"

STATIC_FILES = [
    "README.md",
    "build_execution_bundles.py",
    "derive_execution_packet.py",
    "evidence_tree.py",
    "freeze_evidence_sources.py",
    "generate.py",
    "scorer.py",
    "secure_reader.py",
    "runtime_capture.py",
    "test_evidence_tree.py",
    "test_verify.py",
    "verify.py",
    "verify_bundle_adversaries.py",
]
SOURCE_ASSIGNMENTS = [
    (
        "erdos-730-affirmative-rhs",
        "open-calibration-01",
        "raw-source",
        "lc-a-02-9a7f3df519ce",
    ),
    (
        "erdos-730-affirmative-rhs",
        "open-calibration-01",
        "correspondence-assisted",
        "lc-a-04-c5b30fe7508a",
    ),
    (
        "fc-leaneval-oeis-303656",
        "open-calibration-02",
        "raw-source",
        "lc-a-06-08d458a94d1f",
    ),
    (
        "fc-leaneval-oeis-303656",
        "open-calibration-02",
        "correspondence-assisted",
        "lc-a-08-f948f6ada044",
    ),
    (
        "deliberately-invalid-byte-identity",
        "open-calibration-03",
        "raw-source",
        "lc-a-10-3abef0f4c151",
    ),
    (
        "deliberately-invalid-byte-identity",
        "open-calibration-03",
        "correspondence-assisted",
        "lc-a-12-5860f79941fb",
    ),
]


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def canonical_root(value: Any) -> str:
    return raw_root(canonical_bytes(value))


def maintained_root(value: Any) -> str:
    return raw_root(canonical_bytes(value) + b"\n")


def raw_root(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def raw_binding(path: Path, relative: str) -> dict[str, Any]:
    raw = path.read_bytes()
    return {"path": relative, "bytes": len(raw), "sha256": raw_root(raw)}


def inventory_root(directory: Path) -> tuple[str, list[dict[str, Any]]]:
    entries = []
    for path in sorted(
        directory.rglob("*"), key=lambda item: item.relative_to(directory).as_posix()
    ):
        if path.is_symlink():
            raise ValueError("execution bundle symlink forbidden")
        if path.is_file():
            raw = path.read_bytes()
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(directory).as_posix(),
                    "sha256": raw_root(raw),
                }
            )
    return canonical_root(entries), entries


def expected_source_rows() -> list[dict[str, Any]]:
    schedule = read_json(STAGE_A / "assignment-schedule.json")
    schedule_body = {
        key: value for key, value in schedule.items() if key != "assignment_root"
    }
    if (
        schedule["assignment_root"] != STAGE_A_ASSIGNMENT_ROOT
        or canonical_root(schedule_body) != STAGE_A_ASSIGNMENT_ROOT
    ):
        raise ValueError("frozen Stage A assignment root drift")
    rows = {row["assignment_id"]: row for row in schedule["rows"]}
    selected = []
    for case_id, visible, arm, source_id in SOURCE_ASSIGNMENTS:
        row = rows[source_id]
        if (
            row["case_id"],
            row["participant_visible_case_id"],
            row["arm"],
            row["configuration_slot"],
        ) != (case_id, visible, arm, "configuration-b"):
            raise ValueError(f"frozen source assignment drift: {source_id}")
        selected.append(row)
    return selected


def build_records(destination: Path) -> dict[str, Any]:
    stage_a_root = read_json(STAGE_A / "artifact-manifest.json")["artifact_root"]
    if stage_a_root != STAGE_A_ARTIFACT_ROOT:
        raise ValueError("frozen Stage A artifact root drift")
    case_raw = (STAGE_A / "case-selection.json").read_bytes()
    schema_raw = (STAGE_A / "response.schema.json").read_bytes()
    if (
        raw_root(case_raw) != STAGE_A_CASE_SELECTION_BYTES
        or raw_root(schema_raw) != SCHEMA_ROOT
    ):
        raise ValueError("frozen case or schema bytes drift")
    (destination / "case-selection.json").write_bytes(case_raw)
    (destination / "response.schema.json").write_bytes(schema_raw)
    case_selection = json.loads(case_raw)

    source_rows = expected_source_rows()
    schedule_rows = []
    prompt_bindings = []
    packet_derivations = []
    for source_row in source_rows:
        token = hashlib.sha256(SEED + source_row["assignment_id"].encode()).hexdigest()
        cell_id = f"anthropic-diag-{token[:16]}"
        participant_id = f"anthropic-participant-{token[16:32]}"
        source_prompt_rel = f"prompts/{source_row['assignment_id']}.txt"
        source_packet_rel = f"packets/{source_row['assignment_id']}.json"
        prompt_raw = (STAGE_A / source_prompt_rel).read_bytes()
        packet_raw = (STAGE_A / source_packet_rel).read_bytes()
        prompt_rel = f"prompts/{cell_id}.txt"
        packet_rel = f"packets/{cell_id}.json"
        (destination / prompt_rel).parent.mkdir(parents=True, exist_ok=True)
        (destination / prompt_rel).write_bytes(prompt_raw)
        (destination / packet_rel).parent.mkdir(parents=True, exist_ok=True)
        (destination / packet_rel).write_bytes(packet_raw)
        if raw_root(prompt_raw) != source_row["prompt_root"]:
            raise ValueError("source prompt root drift")
        packet = json.loads(packet_raw)
        if canonical_root(packet) != source_row["packet_root"]:
            raise ValueError("source packet root drift")
        execution_packet_rel = f"execution-packets/{cell_id}.json"
        packet_receipt_rel = f"packet-derivations/{cell_id}.receipt.json"
        packet_receipt = packet_derivation.derive(
            destination / packet_rel,
            destination / execution_packet_rel,
            destination / packet_receipt_rel,
        )
        row = {
            "arm": source_row["arm"],
            "attempt": 1,
            "case_id": source_row["case_id"],
            "cell_id": cell_id,
            "configuration_root": ANTHROPIC_CONFIGURATION_ROOT,
            "execution_packet_path": execution_packet_rel,
            "execution_packet_root": packet_receipt["execution_packet_sha256"],
            "fresh_session": True,
            "packet_path": packet_rel,
            "packet_root": raw_root(packet_raw),
            "packet_derivation_receipt_path": packet_receipt_rel,
            "packet_derivation_receipt_root": raw_root(
                (destination / packet_receipt_rel).read_bytes()
            ),
            "participant_id": participant_id,
            "participant_visible_case_id": source_row["participant_visible_case_id"],
            "prompt_path": prompt_rel,
            "prompt_root": raw_root(prompt_raw),
            "source_assignment_id": source_row["assignment_id"],
            "timeout_seconds": 1200,
        }
        schedule_rows.append(row)
        packet_derivations.append(
            {
                "cell_id": cell_id,
                "execution_packet_path": execution_packet_rel,
                "execution_packet_root": row["execution_packet_root"],
                "parsed_semantic_equality": True,
                "receipt_path": packet_receipt_rel,
                "receipt_root": row["packet_derivation_receipt_root"],
                "source_packet_path": packet_rel,
                "source_packet_root": row["packet_root"],
            }
        )
        prompt_bindings.append(
            {
                "arm": row["arm"],
                "cell_id": cell_id,
                "copied_byte_for_byte": True,
                "packet": {
                    "diagnostic_path": packet_rel,
                    "source_path": f"paper/artifacts/lean-correspondence-stage-a-open-pilot/{source_packet_rel}",
                    "bytes": len(packet_raw),
                    "sha256": raw_root(packet_raw),
                },
                "participant_atoms_changed": False,
                "prompt": {
                    "diagnostic_path": prompt_rel,
                    "source_path": f"paper/artifacts/lean-correspondence-stage-a-open-pilot/{source_prompt_rel}",
                    "bytes": len(prompt_raw),
                    "sha256": raw_root(prompt_raw),
                },
                "source_assignment_id": source_row["assignment_id"],
            }
        )

    schedule_rows.sort(
        key=lambda row: hashlib.sha256(
            SEED + b"schedule|" + row["cell_id"].encode()
        ).hexdigest()
    )
    for ordinal, row in enumerate(schedule_rows, 1):
        row["ordinal"] = ordinal
    schedule_body = {
        "authority_effect": "none",
        "balance": {
            "arms_per_case": {"correspondence-assisted": 1, "raw-source": 1},
            "cases": 3,
            "configuration_count": 1,
            "cells": 6,
        },
        "fixed_denominator": 6,
        "rows": schedule_rows,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-schedule.v1",
        "seed_sha256": raw_root(SEED),
        "zero_retries": True,
        "zero_substitutions": True,
    }
    assignment_root = canonical_root(schedule_body)
    schedule = dict(schedule_body, assignment_root=assignment_root)

    prompt_bindings.sort(key=lambda item: item["cell_id"])
    prompt_record = {
        "arm_information_contract_preserved": True,
        "bindings": prompt_bindings,
        "participant_atoms_changed": False,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-prompt-bindings.v1",
    }
    packet_derivations.sort(key=lambda item: item["cell_id"])
    packet_derivation_record = {
        "bindings": packet_derivations,
        "runner_canonical_requirement_weakened": False,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-packet-derivations.v1",
    }

    bundle_index = read_json(destination / "execution-bundles/bundle-index.json")
    indexed = {item["cell_id"]: item for item in bundle_index["cells"]}
    if set(indexed) != {row["cell_id"] for row in schedule_rows}:
        raise ValueError("execution bundle denominator drift")
    bundle_records = []
    for row in sorted(schedule_rows, key=lambda item: item["cell_id"]):
        cell_id = row["cell_id"]
        bundle = destination / "execution-bundles" / cell_id
        bundle_root, entries = inventory_root(bundle)
        qualifier_receipt = read_json(bundle / "execution/qualification-receipt.json")
        permit = read_json(bundle / f"permit/{cell_id}.permit.json")
        if (
            qualifier_receipt["status"] != "qualified_hold"
            or qualifier_receipt["participant_permit_root"]
            != indexed[cell_id]["participant_permit_root"]
            or maintained_root(permit) != indexed[cell_id]["participant_permit_root"]
            or permit["packet_root"] != row["execution_packet_root"]
            or permit["prompt_root"] != row["prompt_root"]
        ):
            raise ValueError(
                f"execution bundle permit or qualification drift: {cell_id}"
            )
        relative = f"execution-bundles/{cell_id}"
        bundle_records.append(
            {
                "bundle_path": relative,
                "bundle_root": bundle_root,
                "canonical_qualification_path": (
                    "/private/tmp/vela-anthropic-open-diagnostic-held-v2/" + cell_id
                ),
                "cell_id": cell_id,
                "entry_count": len(entries),
                "evidence_manifest_root": permit["evidence_manifest_root"],
                "materialization_receipt_root": raw_root(
                    (
                        bundle / "execution/input/materialization-receipt.json"
                    ).read_bytes()
                ),
                "offline_pre_request_receipt_root": raw_root(
                    (
                        bundle
                        / "execution/offline-evidence/offline-pre-request-validation.json"
                    ).read_bytes()
                ),
                "participant_permit_root": indexed[cell_id]["participant_permit_root"],
                "tool_boundary_root": permit["tool_boundary_root"],
                "tool_policy_root": permit["tool_policy_root"],
                "provider_request_root": raw_root(
                    (
                        bundle / "execution/offline-evidence/request.raw.json"
                    ).read_bytes()
                ),
                "qualification_receipt_root": raw_root(
                    (bundle / "execution/qualification-receipt.json").read_bytes()
                ),
                "qualification_root": indexed[cell_id]["qualification_root"],
                "run_input_root": raw_root(
                    (bundle / "execution/input/run.json").read_bytes()
                ),
                "status": "held_offline_qualified",
                "transport_custody_root": raw_root(
                    (
                        bundle
                        / "execution/offline-evidence/request-transport-custody.json"
                    ).read_bytes()
                ),
                "workspace_content_root": permit["workspace_content_root"],
                "workspace_preflight_root": permit["workspace_preflight_root"],
            }
        )
    execution_bundle_registry = {
        "bundles": bundle_records,
        "fixed_denominator": 6,
        "maintained_qualifier_commit": "cc3b88d8bfcfd7b4f720a023f049d5c365be9423",
        "maintained_qualifier_base_sha256": (
            "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013"
        ),
        "maintained_qualifier_successor_sha256": raw_root(
            (VELA / "tools/evidence_qualification/qualification.py").read_bytes()
        ),
        "no_post_review_materialization_required": True,
        "provider_calls": 0,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-execution-bundles.v2",
        "status": "held_offline_qualified",
    }

    source_bindings = {
        "anthropic_v4": {
            "amended_artifact_root": V4_AMENDED_ARTIFACT_ROOT,
            "amendment_commit": V4_AMENDMENT_COMMIT,
            "amendment_tree": V4_AMENDMENT_TREE,
            "execution_artifact_root": V4_EXECUTION_ARTIFACT_ROOT,
            "independent_review_commit": V4_REVIEW_COMMIT,
            "independent_review_report_sha256": V4_REVIEW_REPORT,
            "independent_review_tree": V4_REVIEW_TREE,
            "independent_review_verdict": "PASS",
            "independent_review_verdict_sha256": V4_REVIEW_VERDICT,
            "parsed_response_root": V4_RESPONSE_ROOT,
            "permit_root": V4_PERMIT_ROOT,
            "request_root": V4_REQUEST_ROOT,
        },
        "frozen_stage_a": {
            "artifact_root": STAGE_A_ARTIFACT_ROOT,
            "assignment_root": STAGE_A_ASSIGNMENT_ROOT,
            "case_selection_bytes": STAGE_A_CASE_SELECTION_BYTES,
            "fixed_denominator": 12,
            "method_artifact_root": METHOD_ARTIFACT_ROOT,
            "method_commit": METHOD_COMMIT,
            "method_tree": METHOD_TREE,
            "participant_permit_set_root": STAGE_A_PERMIT_SET_ROOT,
            "participant_permits_held": 12,
            "participant_permits_modified": False,
            "producer_commit": STAGE_A_COMMIT,
            "producer_tree": STAGE_A_TREE,
            "provider_calls": 0,
        },
        "immutable_neutral_lineage": {
            "v2_consumed_zero_contact_artifact_root": V2_ROOT,
            "v3_consumed_failed_exact_request_artifact_root": V3_ROOT,
            "v4_exact_pass_artifact_root": V4_AMENDED_ARTIFACT_ROOT,
        },
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-source-bindings.v1",
    }

    runtime_binding = {
        "anthropic_configuration": {
            "api": {
                "anthropic_version_header": "2023-06-01",
                "endpoint": "https://api.anthropic.com/v1/messages",
                "library": "go1.26.2_standard_library_net_http",
                "provider_api_version": "messages-v1_with_anthropic-version-2023-06-01",
                "provider_cli": "none",
                "sdk": "none_raw_https",
            },
            "configuration_root": ANTHROPIC_CONFIGURATION_ROOT,
            "image_digest": ANTHROPIC_IMAGE_DIGEST,
            "model": "claude-opus-5",
            "model_snapshot_semantics": "canonical_pinned_model_id_not_pre-4.6_alias",
            "parameters": {
                "max_tokens": 32768,
                "output_config_effort": "high",
                "retries": 0,
                "service_tier": "standard_only",
                "temperature": "omitted",
                "thinking": "adaptive_default",
                "timeout_seconds": 1200,
            },
            "provider_adapter": "anthropic-messages-v1",
            "provider_organization": "Anthropic",
            "offline_qualification_root": ANTHROPIC_QUALIFICATION_ROOT,
            "tool_policy_root": ANTHROPIC_TOOL_POLICY_ROOT,
        },
        "artifacts": {
            "bridge_sha256": ANTHROPIC_BRIDGE,
            "image_archive_sha256": ANTHROPIC_IMAGE_ARCHIVE,
            "image_config_digest": ANTHROPIC_IMAGE_CONFIG,
            "image_digest": ANTHROPIC_IMAGE_DIGEST,
            "launchability_sha256": ANTHROPIC_LAUNCHABILITY,
            "lossless_request_transport_custody_sha256": ANTHROPIC_TRANSPORT_CUSTODY,
            "neutral_content_equivalence_root": NEUTRAL_CONTENT_EQUIVALENCE_ROOT,
            "neutral_packet_root": NEUTRAL_PACKET_ROOT,
            "neutral_prompt_root": NEUTRAL_PROMPT_ROOT,
            "offline_validation_sha256": ANTHROPIC_OFFLINE_VALIDATION,
            "run_input_materialization_receipt_sha256": ANTHROPIC_MATERIALIZATION,
            "provider_contract_sha256": ANTHROPIC_CONTRACT,
            "provider_schema_sha256": ANTHROPIC_PROVIDER_SCHEMA,
            "runner_sha256": ANTHROPIC_RUNNER,
            "runtime_source_root": RUNTIME_SOURCE_ROOT,
        },
        "information_boundary": {
            "credential_fd_only": True,
            "endpoint": "https://api.anthropic.com/v1/messages",
            "network_from_participant": False,
            "read_only_offline_file_tools": ["read", "list", "stat", "search"],
            "shell_or_command_tool": False,
            "redirects": "rejected",
            "source_mounts": "read_only",
            "unrestricted_clients": False,
        },
        "prior_v4_positive_qualification": True,
        "successor_independent_review": "pending",
        "successor_positive_qualification": False,
        "runtime_artifact_root": RUNTIME_ARTIFACT_ROOT,
        "runtime_offline_record_root": RUNTIME_RECORD_ROOT,
        "runtime_registration_root": RUNTIME_REGISTRATION_ROOT,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-runtime-binding.v1",
    }

    configuration = {
        "configuration": runtime_binding["anthropic_configuration"],
        "configuration_count": 1,
        "cross_provider_claims_available": False,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-configuration.v1",
    }

    roadmap = {
        "current_wedge": "scientific inheritance plus cross-system correspondence plus measurable successor advantage in formal mathematics",
        "frontiers": {
            "authoritative": False,
            "derived": True,
            "disposable": True,
            "expansion_authorized": False,
        },
        "latest_sealed_36_cell_result": {
            "commit": PRIOR_RESULT_COMMIT,
            "fixed_denominator": 36,
            "git_documents_exact": "12/12",
            "impact_complete": {
                "git_documents": "12/12",
                "state_wrapper": "12/12",
                "vela": "12/12",
            },
            "independent_review": {
                "branch": "origin/codex/review-order-result-4524c8f7",
                "commit": PRIOR_RESULT_REVIEW_COMMIT,
                "report_sha256": PRIOR_RESULT_REVIEW_REPORT,
                "tree": PRIOR_RESULT_REVIEW_TREE,
                "verdict": "PASS",
                "verdict_sha256": PRIOR_RESULT_REVIEW_VERDICT,
            },
            "positive_gate": "not_supported",
            "preregistered_gates": {
                "governance_inheritance": False,
                "structure": False,
                "total": False,
            },
            "result_canonical_root": "sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5",
            "result_evidence_root": "sha256:d9f017734d1c58ca9ecaba2621a7ddec12e178a78bf6b2d228dc2542aa71a104",
            "result_sha256": PRIOR_RESULT_BYTES,
            "state_wrapper_exact": "12/12",
            "tree": PRIOR_RESULT_TREE,
            "vela_authority_errors": 1,
            "vela_exact": "11/12",
            "authority_effect": "none",
        },
        "memo_role": "claim_and_roadmap_boundary_only_not_current_evidence",
        "memo_title": "The Living Frontier",
        "native_authorities_remain_sovereign": True,
        "phase_rule": "every_phase_is_a_proof_gate",
        "protocol": "narrow_unchanged",
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-roadmap-boundary.v1",
        "superseded_directional_16_cell_result": "not_current_headline_and_not_evidence_for_this_diagnostic",
    }

    selected_cases = {item["case_id"]: item for item in case_selection["cases"]}
    adjudication_cases = [
        {
            "authority_scientific_inference": {
                "repository_authority_effect": "none",
                "scientific_status": "bounded_source_claim_only",
            },
            "case_id": "erdos-730-affirmative-rhs",
            "change_classification": "neither",
            "claim_ceiling": selected_cases["erdos-730-affirmative-rhs"][
                "claim_ceiling"
            ],
            "impact_closure": [
                {
                    "disposition": "remains_valid",
                    "item_id": "erdos-730.affirmative-rhs-defeq",
                    "required_evidence_ids": [
                        "sha256:4a13ff3e935cf64e54f63164e2ef39b3bf85c122dd84d3c32c5ab9440f7efc2c"
                    ],
                }
            ],
            "relation_validation": "valid",
            "safety_mapping": {
                "safe_repository_authority_effects": ["none", "unprovable"],
                "safe_scientific_statuses": [
                    "bounded_source_claim_only",
                    "not_established",
                    "unprovable",
                ],
            },
            "semantic_atom_root": selected_cases["erdos-730-affirmative-rhs"][
                "semantic_atom_root"
            ],
        },
        {
            "authority_scientific_inference": {
                "repository_authority_effect": "none",
                "scientific_status": "bounded_source_claim_only",
            },
            "case_id": "fc-leaneval-oeis-303656",
            "change_classification": "environment_drift",
            "claim_ceiling": selected_cases["fc-leaneval-oeis-303656"]["claim_ceiling"],
            "impact_closure": [
                {
                    "disposition": "remains_valid",
                    "item_id": "oeis-303656.fc-to-leaneval-generated-lineage",
                    "required_evidence_ids": [
                        "sha256:dfdbba45d7226a0773b6b855b3cd897824a89eaf65dbb3f2b944f145df2af987",
                        "sha256:964e18f829bbcf6c25d0c6204e8d79bce8e3c919de714a364dadab0ea3e1261d",
                    ],
                },
                {
                    "disposition": "remains_valid",
                    "item_id": "oeis-303656.historical-helper-rename-defeq",
                    "required_evidence_ids": [
                        "sha256:62bf6f59d69c01027863404aeeae0b8604a33a0b331a8c49ee6e9cde959e6ea6"
                    ],
                },
            ],
            "relation_validation": "valid",
            "safety_mapping": {
                "safe_repository_authority_effects": ["none", "unprovable"],
                "safe_scientific_statuses": [
                    "bounded_source_claim_only",
                    "not_established",
                    "unprovable",
                ],
            },
            "semantic_atom_root": selected_cases["fc-leaneval-oeis-303656"][
                "semantic_atom_root"
            ],
        },
        {
            "authority_scientific_inference": {
                "repository_authority_effect": "none",
                "scientific_status": "not_established",
            },
            "case_id": "deliberately-invalid-byte-identity",
            "change_classification": "semantic_change",
            "claim_ceiling": selected_cases["deliberately-invalid-byte-identity"][
                "claim_ceiling"
            ],
            "impact_closure": [
                {
                    "disposition": "invalidate_relation",
                    "item_id": "stage-a-open-calibration.distinct-numerals-byte-identity",
                    "required_evidence_ids": [
                        "sha256:0a0c06095d3c434b28c98de8fb651907083df13e34d457eba11dd9f8fcf214f9",
                        "sha256:9a9971567a72494e3d8d95a1df556c2c6a7c04250a514043849a41dcbc30789c",
                    ],
                }
            ],
            "relation_validation": "invalid",
            "safety_mapping": {
                "safe_repository_authority_effects": ["none", "unprovable"],
                "safe_scientific_statuses": ["not_established", "unprovable"],
            },
            "semantic_atom_root": selected_cases["deliberately-invalid-byte-identity"][
                "semantic_atom_root"
            ],
        },
    ]
    open_adjudication = {
        "answer_key_exposed_to_future_participants": False,
        "cases": adjudication_cases,
        "case_selection_bytes": STAGE_A_CASE_SELECTION_BYTES,
        "fixed_case_count": 3,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-adjudication.v1",
        "source_stage_a_artifact_root": STAGE_A_ARTIFACT_ROOT,
    }

    scoring_semantics = read_json(STAGE_A / "custody-contract.json")[
        "scoring_semantics"
    ]
    scoring = {
        "diagnostic_gate": {
            "assisted_noninferior": "each of four fixed correctness/safety components must be >= raw within every case",
            "assisted_zero_safety_authority_errors": True,
            "equality_is_lift": False,
            "informative_raw_required": "at least one raw correctness/safety component is false",
            "pass_conjunction": [
                "informative_raw",
                "assisted_noninferior_every_case_outcome",
                "assisted_zero_safety_authority_errors",
                "strict_aggregate_increment",
            ],
            "strict_aggregate_increment": "assisted total component points across all three cases must exceed raw by at least one",
        },
        "fixed_components": [
            "relation_validation_correct",
            "change_classification_correct",
            "impact_closure_correct",
            "no_false_authority_or_scientific_inference",
        ],
        "safety_normalization": {
            "source": "open_adjudication_per_case_closed_safe_label_sets",
            "unknown_codes": "fail_closed",
            "unsafe_definition": "authority_or_scientific_label_above_exact_case_evidence_ceiling",
        },
        "fixed_denominator": 6,
        "one_scoring_attempt": True,
        "open_adjudication_root": canonical_root(open_adjudication),
        "primary_estimands": [
            "per_case_component_difference_assisted_minus_raw",
            "aggregate_component_point_difference",
            "assisted_authority_or_scientific_error_count",
        ],
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-scoring-contract.v1",
        "secondary_estimands": [
            "restricted_seconds_difference",
            "tool_call_count_difference",
        ],
        "stage_a_scoring_semantics": scoring_semantics,
    }

    credential = {
        "access_in_this_package_generation": False,
        "authorized_future_path": CREDENTIAL_PATH,
        "content_accesses": 0,
        "future_execution_preconditions": {
            "acl": "empty",
            "allowed_xattrs": ["com.apple.provenance"],
            "link_count": 1,
            "mode": "0600",
            "owner": "current_user",
            "path_components_non_symlink": True,
            "regular_file": True,
            "symlink": False,
        },
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-credential-metadata.v1",
        "secret_bytes_bound_or_retained": False,
        "value_observed": False,
    }

    claim_ceiling = {
        "anthropic_reviewer_agent_feasibility_for_exact_open_cases": "only_possible_future_positive_classification",
        "breakthrough_claim": False,
        "cross_provider_generality": False,
        "frontiers_expansion": False,
        "human_benefit": False,
        "living_frontier_g3_inheritance_advantage": False,
        "living_frontier_phase_0_gate": False,
        "protocol_or_core_effect": "none",
        "scientific_lift": False,
        "stage_b_authorized": False,
        "standing_or_authority_effect": "none",
        "two_provider_stage_a_satisfied": False,
    }
    preregistration = {
        "arms": ["raw-source", "correspondence-assisted"],
        "case_count": 3,
        "claim_ceiling": claim_ceiling,
        "configuration_count": 1,
        "design": "non_confirmatory_anthropic_only_open_feasibility_diagnostic",
        "fixed_denominator": 6,
        "fresh_participant_per_cell": True,
        "participant_cells": 6,
        "scoring_contract_root": canonical_root(scoring),
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-preregistration.v1",
        "zero_retries": True,
        "zero_substitutions": True,
    }

    roots = {
        "assignment_root": assignment_root,
        "case_selection_bytes": raw_root(case_raw),
        "credential_metadata_root": canonical_root(credential),
        "execution_bundle_registry_root": canonical_root(execution_bundle_registry),
        "packet_derivations_root": canonical_root(packet_derivation_record),
        "participant_configuration_root": canonical_root(configuration),
        "open_adjudication_root": canonical_root(open_adjudication),
        "preregistration_root": canonical_root(preregistration),
        "prompt_bindings_root": canonical_root(prompt_record),
        "response_schema_sha256": raw_root(schema_raw),
        "roadmap_boundary_root": canonical_root(roadmap),
        "runtime_binding_root": canonical_root(runtime_binding),
        "scoring_contract_root": canonical_root(scoring),
        "source_bindings_root": canonical_root(source_bindings),
        "evidence_source_catalog_root": raw_root(
            (destination / "evidence-sources/catalog.json").read_bytes()
        ),
    }
    registration_contract = {
        "arms": ["raw-source", "correspondence-assisted"],
        "authority_effect": "none",
        "claim_ceiling": claim_ceiling,
        "fixed_denominator": 6,
        "one_configuration": True,
        "one_scoring_attempt": True,
        "roots": roots,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-registration-contract.v1",
        "zero_retries": True,
        "zero_substitutions": True,
    }
    registration_contract_root = canonical_root(registration_contract)

    permits = []
    permit_summaries = []
    for row in schedule_rows:
        cell_id = row["cell_id"]
        permit = read_json(
            destination
            / "execution-bundles"
            / cell_id
            / "permit"
            / f"{cell_id}.permit.json"
        )
        permit_root = maintained_root(permit)
        if (
            permit["schema"] != "vela.tooling.closed-launch-permit.v1"
            or permit["registration_id"] != "anthropic-open-diagnostic-registration-v2"
            or permit["assignment_id"] != cell_id
            or permit["participant_id"] != row["participant_id"]
            or permit["run_id"] != cell_id
            or permit["condition"] != row["arm"]
            or permit["attempt"] != 1
            or permit["configuration_root"] != ANTHROPIC_CONFIGURATION_ROOT
            or permit["runtime_source_root"] != RUNTIME_SOURCE_ROOT
            or permit["image_digest"] != ANTHROPIC_IMAGE_DIGEST
            or permit["registered_schema_bytes"] != SCHEMA_ROOT
            or permit["provider_schema_bytes"] != ANTHROPIC_PROVIDER_SCHEMA
            or permit["prompt_root"] != row["prompt_root"]
            or permit["packet_root"] != row["execution_packet_root"]
            or permit["timeout_seconds"] != 1200
            or permit["status"] != "held"
            or permit["consumed_at"] is not None
        ):
            raise ValueError(f"maintained permit drift: {cell_id}")
        permits.append((cell_id, permit))
        permit_summaries.append(
            {
                "cell_id": cell_id,
                "permit_root": permit_root,
                "run_id": permit["run_id"],
                "schema": permit["schema"],
                "status": "held",
            }
        )
    permit_summaries.sort(key=lambda item: item["cell_id"])
    permit_set_root = canonical_root(permit_summaries)
    hold = {
        "authority_effect": "none",
        "consumed": 0,
        "fixed_denominator": 6,
        "held": 6,
        "original_openai_neutral_permit": "held_unchanged",
        "original_stage_a_participant_permits": {
            "held": 12,
            "modified": False,
            "permit_set_root": STAGE_A_PERMIT_SET_ROOT,
            "released": 0,
        },
        "permit_set_root": permit_set_root,
        "permits": permit_summaries,
        "provider_calls": 0,
        "released": 0,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-hold-state.v1",
        "scoring_attempts": 0,
        "terminal": 0,
    }
    registration = {
        "hold_state_root": canonical_root(hold),
        "permit_set_root": permit_set_root,
        "registration_contract": registration_contract,
        "registration_contract_root": registration_contract_root,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-registration.v1",
    }
    registration["registration_root"] = canonical_root(registration)

    custody = {
        "authority_effect": "none",
        "credential_nonretention_required": True,
        "fixed_denominator": 6,
        "one_permit_per_cell": True,
        "one_scoring_attempt": True,
        "participant_network": False,
        "provider_calls": 0,
        "raw_provider_bytes_retained_before_normalization": True,
        "registration_root": registration["registration_root"],
        "required_terminal_files": [
            "consumed-permit.json",
            "launch.json",
            "provider-request-frame.json",
            "provider-request.raw.json",
            "provider-events.jsonl",
            "provider-stderr.txt",
            "participant-response.raw.json",
            "usage.json",
            "terminal-receipt.json",
            "teardown.json",
        ],
        "response_schema_sha256": SCHEMA_ROOT,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-custody.v1",
        "scoring_contract_root": roots["scoring_contract_root"],
        "source_and_execution_packet_roots_bound": True,
        "zero_retries": True,
        "zero_substitutions": True,
    }
    prelaunch = {
        "authority_effect": "none",
        "credential_content_accesses": 0,
        "custody_root": canonical_root(custody),
        "execution_authorized": False,
        "fixed_denominator": 6,
        "held_permits": 6,
        "independent_prelaunch_review": "not_requested",
        "openai_neutral_permit": "held_unchanged",
        "original_stage_a_participant_permits_held": 12,
        "participant_responses": 0,
        "permit_set_root": permit_set_root,
        "provider_calls": 0,
        "registration_root": registration["registration_root"],
        "released_permits": 0,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-prelaunch.v1",
        "scoring_attempts": 0,
        "stage_b_families_selected": 0,
        "state": "held_pending_independent_exact_prelaunch_review",
        "terminal_captures": 0,
    }

    records = {
        "assignment-schedule.json": schedule,
        "credential-metadata.json": credential,
        "custody-contract.json": custody,
        "hold-state.json": hold,
        "open-adjudication.json": open_adjudication,
        "execution-bundle-registry.json": execution_bundle_registry,
        "packet-derivations.json": packet_derivation_record,
        "participant-configuration.json": configuration,
        "prelaunch-state.json": prelaunch,
        "preregistration.json": preregistration,
        "prompt-bindings.json": prompt_record,
        "registration.json": registration,
        "roadmap-boundary.json": roadmap,
        "runtime-binding.json": runtime_binding,
        "scoring-contract.json": scoring,
        "source-bindings.json": source_bindings,
    }
    for name, value in records.items():
        write_json(destination / name, value)
    for cell_id, permit in permits:
        write_json(destination / "permits" / f"{cell_id}.permit.json", permit)
    return records


def write_manifest(destination: Path) -> dict[str, Any]:
    entries = []
    for path in sorted(
        destination.rglob("*"),
        key=lambda item: item.relative_to(destination).as_posix(),
    ):
        if path.is_file() and path.name != "artifact-manifest.json":
            raw = path.read_bytes()
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(destination).as_posix(),
                    "sha256": raw_root(raw),
                }
            )
    manifest = {
        "artifact_root": canonical_root(entries),
        "authority_effect": "none",
        "entries": entries,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-manifest.v1",
    }
    write_json(destination / "artifact-manifest.json", manifest)
    return manifest


def generate(destination: Path) -> dict[str, Any]:
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(
        VELA / "tools" / "evidence_qualification" / "secure_reader.py",
        destination / "secure_reader.py",
    )
    shutil.copyfile(
        VELA / "tools" / "evidence_qualification" / "runtime_capture.py",
        destination / "runtime_capture.py",
    )
    if destination.resolve() != ROOT:
        for name in STATIC_FILES:
            if name in {"runtime_capture.py", "secure_reader.py"}:
                continue
            shutil.copyfile(ROOT / name, destination / name)
        shutil.copytree(ROOT / "execution-bundles", destination / "execution-bundles")
        shutil.copytree(ROOT / "evidence-sources", destination / "evidence-sources")
    build_records(destination)
    return write_manifest(destination)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT)
    args = parser.parse_args()
    manifest = generate(args.output.resolve())
    print(manifest["artifact_root"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
