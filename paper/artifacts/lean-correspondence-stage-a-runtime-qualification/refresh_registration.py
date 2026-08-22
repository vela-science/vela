"""Rebind the prospective held registration to regenerated offline receipts."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

PACKAGE = Path(__file__).resolve().parent


def write_json(path: Path, value: Any) -> None:
    path.write_bytes((json.dumps(value, indent=2, sort_keys=True) + "\n").encode())


def main() -> None:
    registration_path = PACKAGE / "registration.json"
    registration = json.loads(registration_path.read_bytes())
    offline = json.loads((PACKAGE / "offline-qualification.json").read_bytes())
    records = {item["provider_adapter"]: item for item in offline["provider_records"]}
    registration["schema"] = (
        "vela.lean-correspondence-stage-a-runtime-qualification-candidate.v5"
    )
    registration["status"] = "held_offline_validated_pending_independent_review"
    registration["authorization"].update(
        {
            "reason": "replacement Anthropic neutral permit is held and non-releasable pending independent exact review; no execution is authorized",
            "requires_new_independent_review_after_repair": True,
        }
    )
    registration["blockers"] = [
        {
            "id": "independent_exact_review_required",
            "observed": "prospective runtime correction is self-verified and remains held",
            "required": "independent exact PASS before any credential access or neutral permit release",
        }
    ]
    for credential in registration["credentials"]:
        credential["presence"] = "not_checked_in_this_correction"
        credential["value_observed"] = False
        credential["retained"] = False
    for permit in registration["neutral_calibration_permits"]:
        record = records[permit["provider_adapter"]]
        permit.update(
            {
                "run_id": record["held_neutral_run_id"],
                "permit_root": record["qualification_receipt"][
                    "participant_permit_root"
                ],
                "consumed": False,
                "status": "held_non_releasable_pending_independent_review",
                "offline_pre_request_validation": "pass",
            }
        )
    for configuration in registration["participant_configurations"]:
        receipt = records[configuration["provider_adapter"]]["qualification_receipt"]
        configuration.update(
            {
                "configuration_root": receipt["configuration_root"],
                "image_digest": receipt["image_digest"],
                "qualification_root": receipt["qualification_root"],
                "tool_boundary_root": receipt["tool_boundary_root"],
                "status": "candidate_configuration_exact_schema_pre_request_validated_held",
            }
        )
    for image in registration["runtime_boundary"]["runtime_images"]:
        record = records[image["provider_adapter"]]
        receipt = record["qualification_receipt"]
        image.update(
            {
                "image_digest": receipt["image_digest"],
                "runtime_source_root": receipt["runtime_source_root"],
                "launchability_receipt_sha256": record["retained"]["launchability"][
                    "sha256"
                ],
                "run_input_sha256": record["retained"]["run_input"]["sha256"],
                "materialization_receipt_sha256": record["retained"][
                    "materialization_receipt"
                ]["sha256"],
                "offline_validation_receipt_sha256": record["retained"][
                    "offline_validation_receipt"
                ]["sha256"],
                "request_bytes_sha256": record["retained"]["request_bytes"]["sha256"],
                "request_transport_custody_sha256": record["retained"][
                    "request_transport_custody"
                ]["sha256"],
            }
        )
    registration["runtime_boundary"].update(
        {
            "run_input_materialization": "exact_raw_schema_file_byte_splice_no_parse_reserialization",
            "offline_same_input_pre_request_validation": True,
            "provider_calls_derived_from_endpoint_write_receipts_only": True,
            "provider_request_transport": "canonical_base64_lossless_single_decode_exact_endpoint_write",
            "provider_request_payload_schema": "vela.lossless-provider-request-payload.v1",
            "provider_request_custody_schema": "vela.lossless-provider-request-custody.v1",
        }
    )
    for retirement in registration["retired_neutral_calibration_permits"]:
        retirement["successor_permit_root"] = records[retirement["provider_adapter"]][
            "qualification_receipt"
        ]["participant_permit_root"]
    registration["prior_consumed_non_call"] = offline["prior_consumed_non_call"]
    registration["prior_consumed_failed_exact_request"] = offline[
        "prior_consumed_failed_exact_request"
    ]
    registration["provider_call_derivation"] = offline["provider_call_derivation"]
    registration["offline_qualification"].update(
        {
            "record_root": offline["record_root"],
            "status": "qualified_hold_exact_schema_launchable_runtimes_and_offline_same_input_preflight",
        }
    )
    registration["corrective_ancestry"].update(
        {
            "stopped_evidence_commit": offline["prior_consumed_non_call"][
                "producer_commit"
            ],
            "stopped_evidence_tree": offline["prior_consumed_non_call"][
                "producer_tree"
            ],
            "prospective_successor_direct_parent_commit": offline[
                "prior_consumed_failed_exact_request"
            ]["producer_commit"],
        }
    )
    write_json(registration_path, registration)


if __name__ == "__main__":
    main()
