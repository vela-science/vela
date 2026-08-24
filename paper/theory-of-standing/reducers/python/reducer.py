#!/usr/bin/env python3
"""Independent Python reducer for the proof-history interchange format."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, NoReturn

INPUT_FORMAT = "theory-of-standing.proof-history.v1"
RESULT_FORMAT = "theory-of-standing.proof-result.v1"
REJECTION_FORMAT = "theory-of-standing.proof-rejection.v1"
STANDING = {"accepted", "unassessed", "superseded", "retracted"}
MAX_NAT = 9_007_199_254_740_991


class FormatError(Exception):
    pass


def fail(message: str) -> NoReturn:
    raise FormatError(message)


def exact(value: Any, keys: set[str], where: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        fail(f"{where}: expected exact keys {sorted(keys)}")
    return value


def nat(value: Any, where: str) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < 0
        or value > MAX_NAT
    ):
        fail(f"{where}: expected nonnegative safe integer")
    return value


def nat_map(value: Any, where: str) -> dict[int, int]:
    if not isinstance(value, dict):
        fail(f"{where}: expected object")
    result = {}
    for key, item in value.items():
        if not isinstance(key, str) or not key.isascii() or not key.isdecimal():
            fail(f"{where}: invalid resource id")
        resource = int(key)
        if str(resource) != key:
            fail(f"{where}: noncanonical resource id")
        result[resource] = nat(item, f"{where}.{key}")
    return result


def validate_action(value: Any, where: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not isinstance(value.get("kind"), str):
        fail(f"{where}: invalid action")
    kind = value["kind"]
    if kind in {"accept", "reject"}:
        exact(value, {"kind", "claim"}, where)
        nat(value["claim"], f"{where}.claim")
    elif kind == "correct":
        exact(
            value,
            {"kind", "prior_decision", "predecessor", "replacement"},
            where,
        )
        for key in ("prior_decision", "predecessor", "replacement"):
            nat(value[key], f"{where}.{key}")
    else:
        fail(f"{where}: unsupported action")
    return value


def validate_history(value: Any) -> dict[str, Any]:
    history = exact(
        value,
        {
            "format",
            "repository",
            "authorized_performers",
            "initial_versions",
            "descriptive_dependencies",
            "records",
        },
        "history",
    )
    if history["format"] != INPUT_FORMAT:
        fail("history.format: unsupported format")
    nat(history["repository"], "history.repository")
    authorized = history["authorized_performers"]
    if not isinstance(authorized, list):
        fail("history.authorized_performers: expected array")
    for index, actor in enumerate(authorized):
        nat(actor, f"history.authorized_performers[{index}]")
    if authorized != sorted(set(authorized)):
        fail("history.authorized_performers: expected sorted unique values")
    history["initial_versions"] = nat_map(
        history["initial_versions"], "history.initial_versions"
    )
    dependencies = history["descriptive_dependencies"]
    if not isinstance(dependencies, list):
        fail("history.descriptive_dependencies: expected array")
    for index, dependency in enumerate(dependencies):
        exact(dependency, {"dependent", "depends_on"}, f"dependency[{index}]")
        nat(dependency["dependent"], f"dependency[{index}].dependent")
        nat(dependency["depends_on"], f"dependency[{index}].depends_on")
    records = history["records"]
    if not isinstance(records, list):
        fail("history.records: expected array")
    decision_ids: set[int] = set()
    for index, record in enumerate(records):
        if not isinstance(record, dict) or not isinstance(record.get("kind"), str):
            fail(f"record[{index}]: invalid record")
        kind = record["kind"]
        if kind == "submission":
            exact(
                record,
                {"kind", "claim", "producer", "scope", "authenticated"},
                f"record[{index}]",
            )
            for key in ("claim", "producer", "scope"):
                nat(record[key], f"record[{index}].{key}")
            if not isinstance(record["authenticated"], bool):
                fail(f"record[{index}].authenticated: expected boolean")
        elif kind == "verification":
            exact(
                record,
                {"kind", "claim", "scope", "property", "outcome"},
                f"record[{index}]",
            )
            for key in ("claim", "scope", "property"):
                nat(record[key], f"record[{index}].{key}")
            if record["outcome"] not in {"pass", "fail"}:
                fail(f"record[{index}].outcome: unsupported outcome")
        elif kind == "decision":
            exact(
                record,
                {
                    "kind",
                    "id",
                    "repository",
                    "authority_label",
                    "performer",
                    "expected_root",
                    "read_set",
                    "action",
                },
                f"record[{index}]",
            )
            for key in (
                "id",
                "repository",
                "authority_label",
                "performer",
                "expected_root",
            ):
                nat(record[key], f"record[{index}].{key}")
            if record["id"] in decision_ids:
                fail(f"record[{index}].id: duplicate Decision id")
            decision_ids.add(record["id"])
            record["read_set"] = nat_map(
                record["read_set"], f"record[{index}].read_set"
            )
            validate_action(record["action"], f"record[{index}].action")
        else:
            fail(f"record[{index}]: unsupported kind")
    return history


def projection(history: dict[str, Any], state: dict[str, Any]) -> list[dict[str, Any]]:
    corrected = {
        event["action"]["predecessor"]
        for event in state["events"]
        if event["action"]["kind"] == "correct"
    }
    dependencies = history["descriptive_dependencies"]
    result = []
    for claim in sorted(state["standing"]):
        needs = any(
            dependency["dependent"] == claim and dependency["depends_on"] in corrected
            for dependency in dependencies
        )
        result.append(
            {
                "claim": claim,
                "status": "needs_reassessment" if needs else "unaffected",
            }
        )
    return result


def output(history: dict[str, Any], state: dict[str, Any], code: str | None) -> bytes:
    standing = [
        {"claim": claim, "status": status}
        for claim, status in sorted(state["standing"].items())
    ]
    if any(item["status"] not in STANDING for item in standing):
        raise AssertionError("noncanonical Standing")
    events = state["events"]
    result = {
        "events": events,
        "format": REJECTION_FORMAT if code is not None else RESULT_FORMAT,
        "reassessment": projection(history, state),
        "repository": history["repository"],
        "root": state["root"],
        "standing": standing,
    }
    if code is not None:
        result["code"] = code
    return (json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n").encode()


def rejection(
    history: dict[str, Any], state: dict[str, Any], code: str
) -> tuple[bytes, int]:
    return output(history, state, code), 2


def reduce_history(history: dict[str, Any]) -> tuple[bytes, int]:
    state: dict[str, Any] = {
        "events": [],
        "root": 0,
        "standing": {},
        "submissions": [],
        "verifications": [],
        "versions": history["initial_versions"],
    }
    for record in history["records"]:
        kind = record["kind"]
        if kind == "submission":
            if not record["authenticated"]:
                continue
            state["submissions"].append((record["claim"], record["scope"]))
            state["standing"].setdefault(record["claim"], "unassessed")
            state["root"] += 1
            continue
        if kind == "verification":
            if (record["claim"], record["scope"]) not in state["submissions"]:
                continue
            state["verifications"].append((record["claim"], record["outcome"]))
            state["root"] += 1
            continue

        decision_id = record["id"]
        if record["repository"] != history["repository"]:
            return rejection(history, state, "wrong_repository")
        if record["performer"] not in history["authorized_performers"]:
            return rejection(history, state, "unauthorized")
        if record["authority_label"] != record["performer"]:
            return rejection(history, state, "misattributed")
        if record["expected_root"] != state["root"]:
            return rejection(history, state, "stale_root")
        if any(
            state["versions"].get(resource) != version
            for resource, version in record["read_set"].items()
        ):
            return rejection(history, state, "stale_read_set")

        action = record["action"]
        action_kind = action["kind"]
        if action_kind == "accept":
            claim = action["claim"]
            eligible = any(item[0] == claim for item in state["submissions"]) and any(
                item == (claim, "pass") for item in state["verifications"]
            )
        elif action_kind == "reject":
            claim = action["claim"]
            eligible = any(item[0] == claim for item in state["submissions"])
        else:
            replacement = action["replacement"]
            eligible = any(
                item[0] == replacement for item in state["submissions"]
            ) and any(item == (replacement, "pass") for item in state["verifications"])
        if not eligible:
            return rejection(history, state, "ineligible")

        if action_kind == "correct":
            prior = action["prior_decision"]
            predecessor = action["predecessor"]
            valid_reference = (
                any(
                    event["decision_id"] == prior
                    and event["repository"] == record["repository"]
                    and event["action"] == {"kind": "accept", "claim": predecessor}
                    for event in state["events"]
                )
                and state["standing"].get(predecessor) == "accepted"
            )
            if not valid_reference:
                return rejection(history, state, "invalid_correction_reference")

        if action_kind == "accept":
            state["standing"][action["claim"]] = "accepted"
        elif action_kind == "correct":
            state["standing"][action["predecessor"]] = "superseded"
            state["standing"][action["replacement"]] = "accepted"
        event = {
            "action": action,
            "authority_label": record["authority_label"],
            "decision_id": decision_id,
            "performer": record["performer"],
            "repository": record["repository"],
        }
        state["events"].append(event)
        state["root"] += 1
    return output(history, state, None), 0


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: reducer.py HISTORY.json", file=sys.stderr)
        return 64
    try:
        value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
        history = validate_history(value)
        payload, code = reduce_history(history)
    except (OSError, json.JSONDecodeError, FormatError) as error:
        payload = (
            json.dumps(
                {"code": "invalid_format", "format": REJECTION_FORMAT},
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode()
        print(error, file=sys.stderr)
        code = 2
    sys.stdout.buffer.write(payload)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
