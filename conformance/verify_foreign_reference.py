#!/usr/bin/env python3
"""Clean-room reader for the derived non-authoritative foreign reference."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath

from readers.python.canonical import canonical_bytes


ROOT = Path(__file__).resolve().parent
FIXTURES = ROOT / "fixtures" / "transfer"
REQUIRED_ROLES = {
    "applied_event",
    "authority_record",
    "claim",
    "decision_event",
    "proposal",
    "repository_manifest",
    "submission",
    "verification",
}


def sha256(value: object) -> str:
    return f"sha256:{hashlib.sha256(canonical_bytes(value)).hexdigest()}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def require_exact_keys(value: dict[str, object], keys: set[str], label: str) -> None:
    require(set(value) == keys, f"{label} fields")


def full_sha(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r"sha256:[0-9a-f]{64}", value) is not None


def prefixed(value: object, prefix: str, length: int) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(rf"{re.escape(prefix)}[0-9a-f]{{{length}}}", value) is not None
    )


def relative_path(value: object) -> bool:
    if not isinstance(value, str) or not value or value.strip() != value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and "\\" not in value
        and all(part not in ("", ".", "..") for part in value.split("/"))
    )


def assess(reference: object) -> dict[str, object]:
    require(isinstance(reference, dict), "reference must be an object")
    require_exact_keys(
        reference,
        {
            "schema",
            "source",
            "objects",
            "object_set_root",
            "completeness",
            "authority",
            "does_not_establish",
        },
        "reference",
    )
    require(reference.get("schema") == "vela.foreign-reference.v1", "schema")

    source = reference.get("source")
    require(isinstance(source, dict), "source")
    require_exact_keys(
        source,
        {
            "frontier_id",
            "git_commit",
            "git_tree",
            "repository_root",
            "claim",
            "submission",
            "proposal",
            "verification",
            "decision_event",
            "applied_event",
            "authority_record",
            "standing",
        },
        "source",
    )
    require(prefixed(source.get("frontier_id"), "vfr_", 16), "frontier id")
    require(re.fullmatch(r"[0-9a-f]{40}", source.get("git_commit", "")) is not None, "commit")
    require(re.fullmatch(r"[0-9a-f]{40}", source.get("git_tree", "")) is not None, "tree")
    require(full_sha(source.get("repository_root")), "repository root")
    for field, prefix, length in (
        ("claim", "vcl_", 64),
        ("submission", "vsb_", 16),
        ("proposal", "vpr_", 16),
        ("verification", "vvr_", 16),
        ("decision_event", "vev_", 16),
        ("applied_event", "vev_", 16),
        ("authority_record", "var_", 16),
    ):
        exact = source.get(field)
        require(isinstance(exact, dict), field)
        require_exact_keys(exact, {"id", "root"}, field)
        require(prefixed(exact.get("id"), prefix, length), f"{field} id")
        require(full_sha(exact.get("root")), f"{field} root")
    require(source.get("standing") == "accepted", "source standing")

    authority = reference.get("authority")
    require(isinstance(authority, dict), "authority")
    require_exact_keys(
        authority,
        {"source_standing", "local_standing_effect", "requires_local_decision"},
        "authority",
    )
    require(authority.get("source_standing") == source["standing"], "standing mismatch")
    require(authority.get("local_standing_effect") == "none", "authority escalation")
    require(authority.get("requires_local_decision") is True, "local decision")

    objects = reference.get("objects")
    require(isinstance(objects, list), "objects")
    keys: list[tuple[str, str, str, str, str]] = []
    for item in objects:
        require(isinstance(item, dict), "object")
        require_exact_keys(
            item,
            {"role", "id", "root", "bytes_root", "path"},
            "object",
        )
        key = (
            item.get("role"),
            item.get("id"),
            item.get("root"),
            item.get("bytes_root"),
            item.get("path"),
        )
        require(all(isinstance(value, str) and value for value in key), "object field")
        require(full_sha(item["root"]), "object root")
        require(full_sha(item["bytes_root"]), "object bytes root")
        require(relative_path(item["path"]), "object path")
        keys.append(key)
    require(keys == sorted(keys), "object order")
    for index in range(5):
        require(len({key[index] for key in keys}) == len(keys), "duplicate object field")
    require(reference.get("object_set_root") == sha256(objects), "object-set root")
    by_role = {item["role"]: item for item in objects}

    bindings = {
        "repository_manifest": ("repository-manifest", source["repository_root"], ".vela/repository.json"),
        "claim": (source["claim"]["id"], source["claim"]["root"], None),
        "submission": (
            source["submission"]["id"],
            source["submission"]["root"],
            None,
        ),
        "proposal": (source["proposal"]["id"], source["proposal"]["root"], None),
        "verification": (
            source["verification"]["id"],
            source["verification"]["root"],
            None,
        ),
        "decision_event": (
            source["decision_event"]["id"],
            source["decision_event"]["root"],
            None,
        ),
        "applied_event": (
            source["applied_event"]["id"],
            source["applied_event"]["root"],
            None,
        ),
        "authority_record": (
            source["authority_record"]["id"],
            source["authority_record"]["root"],
            None,
        ),
    }
    completeness = reference.get("completeness")
    require(isinstance(completeness, dict), "completeness")
    require_exact_keys(completeness, {"status", "missing_roles"}, "completeness")
    missing = completeness.get("missing_roles")
    require(
        isinstance(missing, list)
        and missing == sorted(set(missing))
        and set(missing) <= REQUIRED_ROLES,
        "missing roles",
    )
    actual_missing = REQUIRED_ROLES - set(by_role)
    require(actual_missing == set(missing), "completeness mismatch")
    require(
        (completeness.get("status") == "complete" and not missing)
        or (completeness.get("status") == "incomplete" and bool(missing)),
        "completeness status",
    )
    for role, (object_id, root, path) in bindings.items():
        if role in missing:
            continue
        item = by_role.get(role)
        require(item is not None, f"missing role {role}")
        require(item["id"] == object_id and item["root"] == root, f"binding {role}")
        if path is not None:
            require(item["path"] == path, f"path {role}")

    nonclaims = reference.get("does_not_establish")
    require(
        isinstance(nonclaims, list)
        and bool(nonclaims)
        and len(set(nonclaims)) == len(nonclaims)
        and all(isinstance(value, str) and value and value.strip() == value for value in nonclaims),
        "nonclaims",
    )

    return {
        "schema": "vela.foreign-reference-assessment.v1",
        "status": completeness["status"],
        "reference_root": sha256(reference),
        "object_set_root": reference["object_set_root"],
        "source_frontier_id": source["frontier_id"],
        "source_git_commit": source["git_commit"],
        "source_git_tree": source["git_tree"],
        "source_repository_root": source["repository_root"],
        "source_claim_id": source["claim"]["id"],
        "source_claim_root": source["claim"]["root"],
        "source_submission_id": source["submission"]["id"],
        "source_submission_root": source["submission"]["root"],
        "source_proposal_id": source["proposal"]["id"],
        "source_proposal_root": source["proposal"]["root"],
        "source_verification_id": source["verification"]["id"],
        "source_verification_root": source["verification"]["root"],
        "source_decision_event_id": source["decision_event"]["id"],
        "source_decision_event_root": source["decision_event"]["root"],
        "source_applied_event_id": source["applied_event"]["id"],
        "source_applied_event_root": source["applied_event"]["root"],
        "source_authority_record_id": source["authority_record"]["id"],
        "source_authority_record_root": source["authority_record"]["root"],
        "source_standing": source["standing"],
        "local_standing_effect": authority["local_standing_effect"],
        "requires_local_decision": authority["requires_local_decision"],
        "diagnostics": [f"missing_role:{role}" for role in missing],
    }


def main() -> int:
    try:
        reference = json.loads(
            (FIXTURES / "foreign-reference-input.v1.json").read_text(encoding="utf-8")
        )
        expected = json.loads(
            (FIXTURES / "foreign-reference-expected.v1.json").read_text(encoding="utf-8")
        )
        observed = assess(reference)
        require(observed == expected, "assessment mismatch")

        escalated = json.loads(json.dumps(reference))
        escalated["authority"]["local_standing_effect"] = "accepted"
        try:
            assess(escalated)
        except ValueError:
            pass
        else:
            raise ValueError("authority escalation passed")

        print(
            "foreign-reference: ok "
            f"(root={observed['reference_root']}, local_standing_effect=none)"
        )
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"foreign-reference: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
