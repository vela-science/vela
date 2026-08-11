#!/usr/bin/env python3
"""Held-out exact scorer for the claim-dependency observation pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
from pathlib import Path, PurePosixPath
from typing import Any, Sequence

import rfc8785


ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
CLAIM = re.compile(r"^vcl_[0-9a-f]{64}$")
UUID4 = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
)
POINTER = re.compile(r"^(?:/(?:[^~/]|~[01])*)+$")
TOP_KEYS = {
    "schema",
    "experiment_id",
    "repository_id",
    "repository_origin_root",
    "transition",
    "classifications",
    "stale_verifications",
    "repair_batches",
    "authority_effect",
    "does_not_establish",
}
CLASSIFICATION_KEYS = {
    "label",
    "claim_id",
    "claim_root",
    "repository_id",
    "repository_origin_root",
    "status",
    "evidence",
}
STALE_KEYS = {
    "verification_id",
    "verification_root",
    "input_claim_root",
    "claim_label",
    "evidence",
}
EVIDENCE_KEYS = {"path", "pointer"}
MILESTONES = (
    "time_to_identify_correction",
    "time_to_identify_affected_and_unaffected",
    "time_to_locate_decisive_evidence",
    "time_to_state_next_valid_repair",
)
EXCLUSIONS = {
    "pre_output_infrastructure_failure",
    "arm_contamination",
    "executor_contract_drift",
}


class ContractError(ValueError):
    """Stable local contract failure."""


def strict_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def reject_constant(constant: str) -> None:
    raise ContractError(f"unsupported JSON constant: {constant}")


def output_bytes(value: Any) -> bytes:
    return rfc8785.dumps(value) + b"\n"


def raw_root(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def read_regular(path: Path, maximum: int, *, missing_ok: bool = False) -> bytes | None:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
    except FileNotFoundError:
        if missing_ok:
            return None
        raise ContractError(f"missing file: {path}") from None
    except OSError as exc:
        raise ContractError(f"cannot open regular file {path}: {exc}") from exc
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_size > maximum:
            raise ContractError(f"file type or size refused: {path}")
        chunks: list[bytes] = []
        remaining = before.st_size + 1
        while remaining:
            chunk = os.read(descriptor, min(65536, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        current = path.lstat()
    except OSError as exc:
        raise ContractError(f"file identity changed: {path}: {exc}") from exc
    if (
        len(data) != before.st_size
        or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        or (before.st_dev, before.st_ino) != (current.st_dev, current.st_ino)
    ):
        raise ContractError(f"file changed while reading: {path}")
    return data


def parse_json(data: bytes, label: str) -> Any:
    try:
        value = json.loads(
            data,
            object_pairs_hook=strict_pairs,
            parse_constant=reject_constant,
        )
        rfc8785.dumps(value)
        return value
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
        ContractError,
        rfc8785.CanonicalizationError,
    ) as exc:
        raise ContractError(f"invalid JSON in {label}: {exc}") from exc


def read_json(
    path: Path, maximum: int, *, missing_ok: bool = False
) -> tuple[Any, bytes] | None:
    data = read_regular(path, maximum, missing_ok=missing_ok)
    if data is None:
        return None
    return parse_json(data, str(path)), data


def valid_evidence(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and set(value) == EVIDENCE_KEYS
        and isinstance(value["path"], str)
        and len(value["path"]) <= 160
        and isinstance(value["pointer"], str)
        and len(value["pointer"]) <= 256
        and POINTER.fullmatch(value["pointer"]) is not None
    )


def shape_valid(answer: Any) -> bool:
    if not isinstance(answer, dict) or set(answer) != TOP_KEYS:
        return False
    if (
        answer["schema"] != "vela.claim-dependency-participant-answer.v0"
        or answer["experiment_id"] != "synthetic-counterfactual-erdos-321-v0"
        or not isinstance(answer["repository_id"], str)
        or UUID4.fullmatch(answer["repository_id"]) is None
        or not isinstance(answer["repository_origin_root"], str)
        or ROOT.fullmatch(answer["repository_origin_root"]) is None
        or answer["authority_effect"] != "none"
    ):
        return False
    transition = answer["transition"]
    if not isinstance(transition, dict) or set(transition) != {
        "kind",
        "predecessor",
        "successor",
    }:
        return False
    if transition["kind"] != "counterfactual_supersession":
        return False
    for endpoint in (transition["predecessor"], transition["successor"]):
        if (
            not isinstance(endpoint, dict)
            or set(endpoint) != {"claim_id", "claim_root"}
            or not isinstance(endpoint["claim_id"], str)
            or CLAIM.fullmatch(endpoint["claim_id"]) is None
            or not isinstance(endpoint["claim_root"], str)
            or ROOT.fullmatch(endpoint["claim_root"]) is None
        ):
            return False
    classifications = answer["classifications"]
    if not isinstance(classifications, list) or len(classifications) != 3:
        return False
    for item in classifications:
        if not isinstance(item, dict) or set(item) != CLASSIFICATION_KEYS:
            return False
        if (
            item["label"] not in {"B", "D", "E"}
            or not isinstance(item["claim_id"], str)
            or CLAIM.fullmatch(item["claim_id"]) is None
            or not isinstance(item["claim_root"], str)
            or ROOT.fullmatch(item["claim_root"]) is None
            or item["repository_id"] != answer["repository_id"]
            or item["repository_origin_root"] != answer["repository_origin_root"]
            or item["status"] not in {"review_required", "unaffected", "incomplete"}
            or not isinstance(item["evidence"], list)
            or not 1 <= len(item["evidence"]) <= 8
            or not all(valid_evidence(evidence) for evidence in item["evidence"])
            or item["evidence"]
            != sorted(item["evidence"], key=lambda row: (row["path"], row["pointer"]))
        ):
            return False
    if [item["label"] for item in classifications] != ["B", "D", "E"]:
        return False
    stale = answer["stale_verifications"]
    if not isinstance(stale, list) or len(stale) > 3:
        return False
    for item in stale:
        if not isinstance(item, dict) or set(item) != STALE_KEYS:
            return False
        if (
            item["verification_id"] not in {"fixture:V_B", "fixture:V_D", "fixture:V_E"}
            or not isinstance(item["verification_root"], str)
            or ROOT.fullmatch(item["verification_root"]) is None
            or not isinstance(item["input_claim_root"], str)
            or ROOT.fullmatch(item["input_claim_root"]) is None
            or item["claim_label"] not in {"B", "D", "E"}
            or not isinstance(item["evidence"], list)
            or not 1 <= len(item["evidence"]) <= 8
            or not all(valid_evidence(evidence) for evidence in item["evidence"])
            or item["evidence"]
            != sorted(item["evidence"], key=lambda row: (row["path"], row["pointer"]))
        ):
            return False
    if [item["verification_id"] for item in stale] != sorted(
        item["verification_id"] for item in stale
    ):
        return False
    batches = answer["repair_batches"]
    if not isinstance(batches, list) or len(batches) > 3:
        return False
    for item in batches:
        if (
            not isinstance(item, dict)
            or set(item) != {"batch", "labels"}
            or not isinstance(item["batch"], int)
            or isinstance(item["batch"], bool)
            or not 1 <= item["batch"] <= 3
            or not isinstance(item["labels"], list)
            or not 1 <= len(item["labels"]) <= 3
            or any(label not in {"B", "D", "E"} for label in item["labels"])
            or item["labels"] != sorted(item["labels"])
        ):
            return False
    if [item["batch"] for item in batches] != sorted(item["batch"] for item in batches):
        return False
    nonclaims = answer["does_not_establish"]
    return (
        isinstance(nonclaims, list)
        and len(nonclaims) == 4
        and all(isinstance(item, str) and 1 <= len(item) <= 512 for item in nonclaims)
    )


def decode_pointer(pointer: str, value: Any) -> Any:
    if POINTER.fullmatch(pointer) is None:
        raise ContractError("invalid RFC 6901 pointer")
    current = value
    for raw_token in pointer.split("/")[1:]:
        token = raw_token.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and token in current:
            current = current[token]
        elif isinstance(current, list) and re.fullmatch(r"0|[1-9][0-9]*", token):
            index = int(token)
            if index >= len(current):
                raise ContractError("pointer array index is out of bounds")
            current = current[index]
        else:
            raise ContractError("pointer does not resolve")
    return current


def evidence_valid(answer: Any, manifest: dict[str, Any], input_root: Path) -> bool:
    if not shape_valid(answer):
        return False
    allowed: dict[str, Any] = {}
    try:
        for row in manifest["files"]:
            mounted = row["mounted_path"]
            path = PurePosixPath(mounted)
            if (
                path.is_absolute()
                or not path.parts
                or any(part in {"", ".", ".."} for part in path.parts)
            ):
                return False
            source = input_root.joinpath(*path.parts)
            parsed = read_json(source, 1_048_576)
            if parsed is None:
                return False
            value, data = parsed
            if len(data) != row["bytes"] or raw_root(data) != row["raw_root"]:
                return False
            allowed[mounted] = value
    except (ContractError, KeyError, TypeError):
        return False

    evidence_rows: list[tuple[str, str, str]] = []
    for item in answer["classifications"]:
        evidence_rows.extend(
            (item["label"], row["path"], row["pointer"]) for row in item["evidence"]
        )
    for item in answer["stale_verifications"]:
        evidence_rows.extend(
            (item["verification_id"], row["path"], row["pointer"])
            for row in item["evidence"]
        )
    try:
        for _, path, pointer in evidence_rows:
            decode_pointer(pointer, allowed[path])
    except (ContractError, KeyError):
        return False

    by_owner: dict[str, set[tuple[str, str]]] = {}
    for owner, path, pointer in evidence_rows:
        by_owner.setdefault(owner, set()).add((path, pointer))

    def has(owner: str, path: str, pointer: str) -> bool:
        return (path, pointer) in by_owner.get(owner, set())

    profile_present = "profile.json" in allowed
    requirements = (
        has("B", "state.json", "/claims/1")
        and has("B", "shared-scope.json", "/repository_context")
        and has("B", "state.json", "/claims/0/lifecycle")
        and (
            has("B", "baseline/raw-source.json", "/statements/2/requires")
            or (profile_present and has("B", "profile.json", "/dependencies/0"))
        )
        and has("D", "state.json", "/claims/4")
        and has("D", "shared-scope.json", "/repository_context")
        and has("D", "baseline/raw-source.json", "/statements/3")
        and has("E", "state.json", "/claims/2")
        and has("E", "shared-scope.json", "/repository_context")
        and has("E", "state.json", "/claims/1/lifecycle")
        and (
            has("E", "baseline/raw-source.json", "/statements/4/requires")
            or (profile_present and has("E", "profile.json", "/dependencies/1"))
        )
        and has("fixture:V_B", "state.json", "/claims/1/verification")
        and has("fixture:V_E", "state.json", "/claims/2/verification")
    )
    return requirements


def projection(
    answer: Any, key: dict[str, Any], manifest: dict[str, Any], input_root: Path
) -> dict[str, Any]:
    valid_shape = shape_valid(answer)
    classifications = (
        answer.get("classifications", []) if isinstance(answer, dict) else []
    )
    classification_map = {
        item.get("label"): item
        for item in classifications
        if isinstance(item, dict) and isinstance(item.get("label"), str)
    }
    expected_claims = {item["label"]: item for item in key["claims"]}
    repository_context_exact = bool(
        isinstance(answer, dict)
        and answer.get("repository_id") == key["repository_id"]
        and answer.get("repository_origin_root") == key["repository_origin_root"]
        and all(
            item.get("repository_id") == key["repository_id"]
            and item.get("repository_origin_root") == key["repository_origin_root"]
            for item in classifications
            if isinstance(item, dict)
        )
        and len(classifications) == 3
    )
    transition_exact = (
        isinstance(answer, dict) and answer.get("transition") == key["transition"]
    )
    claim_bindings_exact = set(classification_map) == set(expected_claims) and all(
        classification_map[label].get("claim_id") == expected["claim_id"]
        and classification_map[label].get("claim_root") == expected["claim_root"]
        for label, expected in expected_claims.items()
    )
    predicted_affected = sorted(
        label
        for label, item in classification_map.items()
        if item.get("status") == "review_required"
    )
    expected_affected = sorted(
        label
        for label, item in expected_claims.items()
        if item["status"] == "review_required"
    )
    true_positive = len(set(predicted_affected) & set(expected_affected))
    false_positive = len(set(predicted_affected) - set(expected_affected))
    false_negative = len(set(expected_affected) - set(predicted_affected))
    precision = true_positive / len(predicted_affected) if predicted_affected else 0.0
    recall = true_positive / len(expected_affected)
    predicted_unaffected = sorted(
        label
        for label, item in classification_map.items()
        if item.get("status") == "unaffected"
    )
    predicted_incomplete = sorted(
        label
        for label, item in classification_map.items()
        if item.get("status") == "incomplete"
    )
    stale_projection = []
    if isinstance(answer, dict) and isinstance(answer.get("stale_verifications"), list):
        for item in answer["stale_verifications"]:
            if isinstance(item, dict):
                stale_projection.append(
                    {
                        key_name: item.get(key_name)
                        for key_name in STALE_KEYS - {"evidence"}
                    }
                )
    expected_stale = [
        {key_name: item[key_name] for key_name in STALE_KEYS - {"evidence"}}
        for item in key["stale_verifications"]
    ]
    authority_errors = 0
    if not isinstance(answer, dict) or answer.get("authority_effect") != "none":
        authority_errors += 1
    if (
        not isinstance(answer, dict)
        or answer.get("does_not_establish") != key["does_not_establish"]
    ):
        authority_errors += 1
    evidence_pointers_valid = evidence_valid(answer, manifest, input_root)
    result = {
        "schema_valid": valid_shape,
        "repository_context_exact": repository_context_exact,
        "transition_exact": transition_exact,
        "claim_bindings_exact": claim_bindings_exact,
        "predicted_affected": predicted_affected,
        "affected_true_positive_count": true_positive,
        "affected_false_positive_count": false_positive,
        "affected_false_negative_count": false_negative,
        "affected_set_precision": precision,
        "affected_set_recall": recall,
        "unaffected_exact": predicted_unaffected == ["D"],
        "incomplete_exact": predicted_incomplete == [],
        "stale_verifications_exact": stale_projection == expected_stale,
        "repair_batches_exact": isinstance(answer, dict)
        and answer.get("repair_batches") == key["repair_batches"],
        "evidence_pointers_valid": evidence_pointers_valid,
        "does_not_establish_exact": isinstance(answer, dict)
        and answer.get("does_not_establish") == key["does_not_establish"],
        "false_authority_or_acceptance_inferences": authority_errors,
        "hidden_maintainer_interventions": {
            "status": "not_measured",
            "value": None,
            "reason": "The held-out scorer observes only the retained answer artifact.",
        },
    }
    exact_booleans = (
        "schema_valid",
        "repository_context_exact",
        "transition_exact",
        "claim_bindings_exact",
        "unaffected_exact",
        "incomplete_exact",
        "stale_verifications_exact",
        "repair_batches_exact",
        "evidence_pointers_valid",
        "does_not_establish_exact",
    )
    result["safe_completion"] = (
        all(result[field] is True for field in exact_booleans)
        and precision == 1.0
        and recall == 1.0
        and authority_errors == 0
    )
    return result


def milestones() -> list[dict[str, Any]]:
    return [
        {
            "name": name,
            "status": "not_measured",
            "value": None,
            "unit": "seconds",
            "denominator": None,
            "source_run_ids": [],
        }
        for name in MILESTONES
    ]


def score(
    answer_path: Path,
    key_path: Path,
    manifest_path: Path,
    input_root: Path,
    exclusion_reason: str | None = None,
) -> dict[str, Any]:
    key_result = read_json(key_path, 262_144)
    manifest_result = read_json(manifest_path, 262_144)
    if key_result is None or manifest_result is None:
        raise ContractError("answer key and input manifest are required")
    key, _ = key_result
    manifest, _ = manifest_result
    if not isinstance(key, dict) or not isinstance(manifest, dict):
        raise ContractError("answer key and input manifest must be objects")
    answer: Any = None
    answer_data: bytes | None = None
    parse_error: str | None = None
    try:
        answer_data = read_regular(answer_path, 262_144, missing_ok=True)
        if answer_data is not None:
            answer = parse_json(answer_data, str(answer_path))
    except ContractError as exc:
        parse_error = str(exc)
    metrics = projection(answer, key, manifest, input_root)
    if exclusion_reason is not None and exclusion_reason not in EXCLUSIONS:
        raise ContractError(f"unsupported exclusion reason: {exclusion_reason}")
    eligible = exclusion_reason is None
    metrics["safe_completion"] = metrics["safe_completion"] and eligible
    stop_reason = (
        "excluded"
        if exclusion_reason is not None
        else "answer_missing"
        if answer_data is None and parse_error is None
        else "answer_malformed"
        if parse_error is not None
        else "completed_safe"
        if metrics["safe_completion"]
        else "completed_inexact"
    )
    return {
        "schema": "vela.claim-dependency-observation-score.v0",
        "experiment_id": key["experiment_id"],
        "arm": manifest["arm"],
        "answer_raw_root": raw_root(answer_data) if answer_data is not None else None,
        "answer_canonical_root": (
            raw_root(rfc8785.dumps(answer))
            if answer_data is not None and parse_error is None
            else None
        ),
        "answer_parse_error": parse_error,
        **metrics,
        "milestones": milestones(),
        "participant_final_message": {
            "status": "not_measured",
            "byte_identical_to_answer_artifact": None,
            "reason": "No audited Harbor final-message extraction seam is frozen in this tranche.",
        },
        "eligible": eligible,
        "exclusion_reason": exclusion_reason,
        "stop_reason": stop_reason,
        "authority_effect": "none",
        "claim_credit": False,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--answer", type=Path, required=True)
    result.add_argument("--answer-key", type=Path, required=True)
    result.add_argument("--input-manifest", type=Path, required=True)
    result.add_argument("--input-root", type=Path, required=True)
    result.add_argument("--verification-output", type=Path, required=True)
    result.add_argument("--reward-output", type=Path, required=True)
    result.add_argument("--exclusion-reason", choices=sorted(EXCLUSIONS))
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = score(
            args.answer,
            args.answer_key,
            args.input_manifest,
            args.input_root,
            args.exclusion_reason,
        )
        args.verification_output.parent.mkdir(parents=True, exist_ok=True)
        args.reward_output.parent.mkdir(parents=True, exist_ok=True)
        args.verification_output.write_bytes(output_bytes(result))
        args.reward_output.write_bytes(
            output_bytes(
                {
                    "eligible": int(result["eligible"]),
                    "safe_completion": int(
                        result["safe_completion"] and result["eligible"]
                    ),
                }
            )
        )
        return 0
    except (ContractError, OSError, KeyError, TypeError) as exc:
        print(f"error: {exc}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
