from __future__ import annotations

import datetime as _dt
import hashlib
import importlib.resources
import json
from pathlib import Path
from typing import Any

RECEIPT_SCHEMA = "vela.receipt.v1"


def receipt_schema() -> dict[str, Any]:
    with importlib.resources.files(__package__).joinpath("vela.receipt.v1.schema.json").open(
        "r", encoding="utf-8"
    ) as f:
        return json.load(f)


_SCHEMA = receipt_schema()
CLAIM_TYPES = set(_SCHEMA["properties"]["type"]["enum"])
REPLAYABILITY = set(_SCHEMA["properties"]["replayability"]["enum"])
STATUS_KINDS = set(_SCHEMA["properties"]["status"]["properties"]["kind"]["enum"])
STATUS_AUTHORITIES = set(_SCHEMA["properties"]["status"]["properties"]["authority"]["enum"])
VERIFIER_OUTCOMES = set(_SCHEMA["$defs"]["verifier_run"]["properties"]["outcome"]["enum"])
REQUIRED_FIELDS = set(_SCHEMA["required"])
PROVENANCE_REQUIRED = set(_SCHEMA["properties"]["provenance"]["required"])
STATUS_REQUIRED = set(_SCHEMA["properties"]["status"]["required"])
ARTIFACT_REQUIRED = set(_SCHEMA["$defs"]["artifact"]["required"])
VERIFIER_REQUIRED = set(_SCHEMA["$defs"]["verifier_run"]["required"])


def _utc_now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _artifact(spec: str, base_dir: Path | None) -> dict[str, Any]:
    path, kind = (spec.split(":", 1) + ["witness"])[:2] if ":" in spec else (spec, "witness")
    item: dict[str, Any] = {"path": path, "kind": kind or "witness"}
    if base_dir is not None:
        candidate = (base_dir / path).resolve()
        if candidate.exists() and candidate.is_file():
            item["sha256"] = _sha256(candidate)
    return item


def emit_receipt(
    *,
    claim: str,
    artifacts: list[str],
    caveats: list[str],
    claim_type: str = "computational",
    replayability: str = "unknown",
    verifier_runs: list[dict[str, str]] | None = None,
    generated_by: str = "vela-receipt-v0",
    submitter_actor: str | None = None,
    source_system: str | None = None,
    source_uri: str | None = None,
    run_id: str | None = None,
    base_dir: str | Path | None = None,
    conditions: list[str] | None = None,
    verification_requirements: list[str] | None = None,
    state_diff: dict[str, Any] | None = None,
) -> dict[str, Any]:
    root = Path(base_dir).resolve() if base_dir else None
    environment: dict[str, Any] = {}
    source = {k: v for k, v in {
        "system": source_system,
        "source_uri": source_uri,
        "run_id": run_id,
        "exported_at": _utc_now() if (source_system or source_uri or run_id) else None,
    }.items() if v}
    if source:
        environment["source"] = source
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "claim": claim,
        "type": claim_type,
        "replayability": replayability,
        "artifacts": [_artifact(a, root) for a in artifacts],
        "caveats": caveats,
        "verifier_runs": verifier_runs or [],
        "conditions": conditions or [],
        "verification_requirements": verification_requirements or [],
        "state_diff": state_diff or {},
        "environment": environment,
        "provenance": {
            "generated_by": generated_by,
            "emitted_at": _utc_now(),
            "submitter": {"actor": submitter_actor} if submitter_actor else {},
        },
        "status": {
            "kind": "emitted",
            "authority": "producer",
            "note": "Producer emission only. Vela landing and human acceptance are separate.",
        },
    }
    validate_receipt(receipt)
    return receipt


def validate_receipt(receipt: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for field in sorted(REQUIRED_FIELDS):
        if field not in receipt:
            errors.append(f"{field} is required by the bundled schema")
    if receipt.get("schema") != RECEIPT_SCHEMA:
        errors.append(f"schema must be {RECEIPT_SCHEMA}")
    if not isinstance(receipt.get("claim"), str) or not receipt["claim"].strip():
        errors.append("claim must be a non-empty string")
    if receipt.get("type") not in CLAIM_TYPES:
        errors.append(f"type must be one of {', '.join(sorted(CLAIM_TYPES))}")
    if receipt.get("replayability") not in REPLAYABILITY:
        errors.append(f"replayability must be one of {', '.join(sorted(REPLAYABILITY))}")
    artifacts = receipt.get("artifacts")
    if not isinstance(artifacts, list):
        errors.append("artifacts must be an array")
    else:
        for i, artifact in enumerate(artifacts):
            if not isinstance(artifact, dict):
                errors.append(f"artifacts[{i}] must be an object")
                continue
            for field in sorted(ARTIFACT_REQUIRED):
                if not artifact.get(field):
                    errors.append(f"artifacts[{i}].{field} is required")
            sha = artifact.get("sha256")
            if sha is not None and (not isinstance(sha, str) or len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha)):
                errors.append(f"artifacts[{i}].sha256 must be lowercase hex sha256")
    caveats = receipt.get("caveats")
    if not isinstance(caveats, list) or not caveats or any(not isinstance(c, str) or not c.strip() for c in caveats):
        errors.append("caveats must contain at least one non-empty string")
    verifier_runs = receipt.get("verifier_runs")
    if not isinstance(verifier_runs, list):
        errors.append("verifier_runs must be an array")
    else:
        for i, run in enumerate(verifier_runs):
            if not isinstance(run, dict):
                errors.append(f"verifier_runs[{i}] must be an object")
                continue
            for field in sorted(VERIFIER_REQUIRED):
                if not run.get(field):
                    errors.append(f"verifier_runs[{i}].{field} is required")
            if run.get("outcome") not in VERIFIER_OUTCOMES:
                errors.append(f"verifier_runs[{i}].outcome must be one of {', '.join(sorted(VERIFIER_OUTCOMES))}")
    if not isinstance(receipt.get("environment"), dict):
        errors.append("environment must be an object")
    provenance = receipt.get("provenance")
    if not isinstance(provenance, dict):
        errors.append("provenance must be an object")
    else:
        for field in sorted(PROVENANCE_REQUIRED):
            if not provenance.get(field):
                errors.append(f"provenance.{field} is required")
    status = receipt.get("status")
    if not isinstance(status, dict):
        errors.append("status must be an object")
    else:
        for field in sorted(STATUS_REQUIRED):
            if not status.get(field):
                errors.append(f"status.{field} is required")
        if status.get("kind") not in STATUS_KINDS:
            errors.append(f"status.kind must be one of {', '.join(sorted(STATUS_KINDS))}")
        if status.get("authority") not in STATUS_AUTHORITIES:
            errors.append(f"status.authority must be one of {', '.join(sorted(STATUS_AUTHORITIES))}")
    if errors:
        raise ValueError("\n".join(errors))
    return []


def load_json(path: str | Path) -> dict[str, Any]:
    with Path(path).open("r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("receipt JSON must be an object")
    return data


def dump_json(data: dict[str, Any], path: str | Path | None = None) -> None:
    text = json.dumps(data, indent=2, sort_keys=True) + "\n"
    if path:
        Path(path).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
