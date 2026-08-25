#!/usr/bin/env python3
"""Deterministic bounded adversarial histories for the accepted model layer."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

FORMAT = "theory-of-standing.proof-history.v1"
GENERATOR = "theory-of-standing.adversarial-templates.v1"
DEPENDENCY = [{"dependent": 20, "depends_on": 10}]


@dataclass(frozen=True)
class Case:
    id: str
    classes: tuple[str, ...]
    history: dict[str, Any]
    expected: dict[str, Any]
    lean_sample: bool = False


def submission(
    claim: int, *, authenticated: bool = True, scope: int = 7
) -> dict[str, Any]:
    return {
        "authenticated": authenticated,
        "claim": claim,
        "kind": "submission",
        "producer": 900,
        "scope": scope,
    }


def verification(
    claim: int, *, outcome: str = "pass", property_id: int = 42, scope: int = 7
) -> dict[str, Any]:
    return {
        "claim": claim,
        "kind": "verification",
        "outcome": outcome,
        "property": property_id,
        "scope": scope,
    }


def accept(claim: int) -> dict[str, Any]:
    return {"claim": claim, "kind": "accept"}


def reject(claim: int) -> dict[str, Any]:
    return {"claim": claim, "kind": "reject"}


def correct(prior: int, predecessor: int, replacement: int) -> dict[str, Any]:
    return {
        "kind": "correct",
        "predecessor": predecessor,
        "prior_decision": prior,
        "replacement": replacement,
    }


def decision(
    identifier: int,
    action: dict[str, Any],
    root: int,
    *,
    repository: int = 1,
    performer: int = 101,
    authority_label: int | None = None,
    read_set: dict[str, int] | None = None,
) -> dict[str, Any]:
    return {
        "action": action,
        "authority_label": performer if authority_label is None else authority_label,
        "expected_root": root,
        "id": identifier,
        "kind": "decision",
        "performer": performer,
        "read_set": {"0": 0} if read_set is None else read_set,
        "repository": repository,
    }


def history(
    records: list[dict[str, Any]],
    *,
    repository: int = 1,
    authorized: list[int] | None = None,
    versions: dict[str, int] | None = None,
    dependencies: list[dict[str, int]] | None = None,
) -> dict[str, Any]:
    return {
        "authorized_performers": [101] if authorized is None else authorized,
        "descriptive_dependencies": [] if dependencies is None else dependencies,
        "format": FORMAT,
        "initial_versions": {"0": 0} if versions is None else versions,
        "records": records,
        "repository": repository,
    }


def expected(
    root: int,
    standing: list[str],
    *,
    claims: list[int] | None = None,
    events: list[int] | None = None,
    rejections: list[tuple[int, str]] | None = None,
    reassessment: list[str] | None = None,
    last_action: str | None = None,
) -> dict[str, Any]:
    if claims is None:
        claims = {0: [], 1: [10], 3: [10, 11, 20]}[len(standing)]
    if len(claims) != len(standing):
        raise AssertionError("Standing claims and statuses must have equal length")
    reassessment_statuses = (
        ["unaffected"] * len(claims) if reassessment is None else reassessment
    )
    if len(reassessment_statuses) != len(claims):
        raise AssertionError("reassessment claims and statuses must have equal length")
    value: dict[str, Any] = {
        "event_ids": [] if events is None else events,
        "reassessment": [
            {"claim": claim, "status": status}
            for claim, status in zip(claims, reassessment_statuses, strict=True)
        ],
        "rejections": [
            {"code": code, "record_index": index}
            for index, code in ([] if rejections is None else rejections)
        ],
        "root": root,
        "standing": [
            {"claim": claim, "status": status}
            for claim, status in zip(claims, standing, strict=True)
        ],
    }
    if last_action is not None:
        value["last_action"] = last_action
    return value


S10 = submission(10)
V10 = verification(10)
A10 = decision(1, accept(10), 2)
S20 = submission(20)
V20 = verification(20, property_id=43)
A20 = decision(2, accept(20), 5)
S11 = submission(11)
V11 = verification(11, property_id=44)
PREFIX = [S10, V10, A10, S20, V20, A20, S11, V11]
FRESH = decision(3, correct(1, 10, 11), 8)
SUFFIX_CORRECTION = decision(4, correct(1, 10, 11), 8)


def cases() -> list[Case]:
    rows: list[Case] = []

    def add(
        identifier: str,
        classes: tuple[str, ...],
        value: dict[str, Any],
        result: dict[str, Any],
        *,
        lean: bool = False,
    ) -> None:
        rows.append(Case(identifier, classes, value, result, lean))

    add(
        "empty",
        ("zero_rejections", "record_order"),
        history([]),
        expected(0, []),
    )
    add(
        "authenticated_submission",
        ("submission", "standing_no_effect"),
        history([S10]),
        expected(1, ["unassessed"]),
    )
    add(
        "unauthenticated_submission_noop",
        ("unauthenticated_submission_noop", "record_order"),
        history([submission(30, authenticated=False), S10]),
        expected(1, ["unassessed"]),
        lean=True,
    )
    add(
        "unmatched_verification_noop",
        ("unmatched_verification_noop", "record_order"),
        history([V10, S10]),
        expected(1, ["unassessed"]),
        lean=True,
    )
    add(
        "matching_verification_no_standing",
        ("verification", "standing_no_effect"),
        history([S10, V10]),
        expected(2, ["unassessed"]),
    )

    for actor, label in [(101, "first"), (202, "last")]:
        add(
            f"authorized_performer_{label}",
            ("authorized_performer_position", "current_root", "current_read_set"),
            history(
                [S10, V10, decision(1, accept(10), 2, performer=actor)],
                authorized=[101, 202],
            ),
            expected(3, ["accepted"], events=[1], last_action="accept"),
            lean=actor == 202,
        )

    for actor, position in [(100, "before"), (150, "between"), (303, "after")]:
        bad = decision(1, accept(10), 2, performer=actor)
        suffix = decision(2, accept(10), 2, performer=202)
        add(
            f"unauthorized_performer_{position}",
            ("unauthorized", "unauthorized_performer_position", "suffix_continuation"),
            history([S10, V10, bad, suffix], authorized=[101, 202]),
            expected(3, ["accepted"], events=[2], rejections=[(2, "unauthorized")]),
            lean=position == "between",
        )

    semantic_suffixes = [
        (
            "wrong_repository_suffix",
            "wrong_repository",
            decision(3, correct(1, 10, 11), 8, repository=2),
        ),
        (
            "misattributed_suffix",
            "misattributed",
            decision(3, correct(1, 10, 11), 8, authority_label=202),
        ),
        ("stale_root_lower_suffix", "stale_root", decision(3, correct(1, 10, 11), 7)),
        ("stale_root_future_suffix", "stale_root", decision(3, correct(1, 10, 11), 9)),
        (
            "stale_read_set_suffix",
            "stale_read_set",
            decision(3, correct(1, 10, 11), 8, read_set={"0": 1}),
        ),
        (
            "ineligible_suffix",
            "ineligible",
            decision(3, correct(1, 10, 99), 8),
        ),
        (
            "invalid_correction_reference_suffix",
            "invalid_correction_reference",
            decision(3, correct(999, 10, 11), 8),
        ),
    ]
    for identifier, code, bad in semantic_suffixes:
        add(
            identifier,
            (code, "one_rejection", "suffix_continuation", "rejected_decision_noop"),
            history([*PREFIX, bad, SUFFIX_CORRECTION], dependencies=DEPENDENCY),
            expected(
                9,
                ["superseded", "accepted", "accepted"],
                events=[1, 2, 4],
                rejections=[(8, code)],
                reassessment=["unaffected", "unaffected", "needs_reassessment"],
                last_action="correct",
            ),
            lean=identifier != "stale_root_future_suffix",
        )

    for stale_resource, identifier in [(0, "first"), (1, "second")]:
        read_set = {"0": 0, "1": 1}
        read_set[str(stale_resource)] += 1
        add(
            f"stale_read_set_{identifier}_entry",
            ("stale_read_set", "read_set_entry_position", "suffix_continuation"),
            history(
                [
                    S10,
                    V10,
                    decision(1, accept(10), 2, read_set=read_set),
                    decision(2, accept(10), 2, read_set={"0": 0, "1": 1}),
                ],
                versions={"0": 0, "1": 1},
            ),
            expected(3, ["accepted"], events=[2], rejections=[(2, "stale_read_set")]),
        )
    add(
        "current_multi_entry_read_set",
        ("current_read_set", "read_set_entry_position"),
        history(
            [S10, V10, decision(1, accept(10), 2, read_set={"0": 0, "1": 1})],
            versions={"0": 0, "1": 1},
        ),
        expected(3, ["accepted"], events=[1]),
    )

    ineligible_rows = [
        ("accept_unsubmitted", accept(99)),
        ("reject_unsubmitted", reject(99)),
        ("correction_unsubmitted_replacement", correct(1, 10, 99)),
    ]
    for offset, (name, action) in enumerate(ineligible_rows, start=1):
        add(
            f"ineligible_{name}",
            ("ineligible", "ineligible_action", "suffix_continuation"),
            history(
                [
                    S10,
                    V10,
                    decision(offset, action, 2),
                    decision(offset + 10, accept(10), 2),
                ]
            ),
            expected(
                3, ["accepted"], events=[offset + 10], rejections=[(2, "ineligible")]
            ),
        )
    add(
        "ineligible_missing_pass",
        ("ineligible", "ineligible_action", "suffix_continuation"),
        history(
            [
                S11,
                verification(11, outcome="fail"),
                decision(1, accept(11), 2),
                decision(2, reject(11), 2),
            ]
        ),
        expected(
            3,
            ["unassessed"],
            claims=[11],
            events=[2],
            rejections=[(2, "ineligible")],
        ),
    )

    add(
        "valid_correction",
        ("valid_correction_reference", "zero_rejections", "correction"),
        history([*PREFIX, FRESH], dependencies=DEPENDENCY),
        expected(
            9,
            ["superseded", "accepted", "accepted"],
            events=[1, 2, 3],
            reassessment=["unaffected", "unaffected", "needs_reassessment"],
            last_action="correct",
        ),
        lean=True,
    )
    add(
        "correction_wrong_predecessor",
        (
            "invalid_correction_reference",
            "correction_reference_variant",
            "suffix_continuation",
        ),
        history([*PREFIX, decision(3, correct(1, 20, 11), 8), SUFFIX_CORRECTION]),
        expected(
            9,
            ["superseded", "accepted", "accepted"],
            events=[1, 2, 4],
            rejections=[(8, "invalid_correction_reference")],
        ),
    )
    add(
        "correction_retry_after_rejection",
        ("correction_retry", "stale_root", "suffix_continuation"),
        history(
            [*PREFIX, decision(3, correct(1, 10, 11), 7), SUFFIX_CORRECTION],
            dependencies=DEPENDENCY,
        ),
        expected(
            9,
            ["superseded", "accepted", "accepted"],
            events=[1, 2, 4],
            rejections=[(8, "stale_root")],
            reassessment=["unaffected", "unaffected", "needs_reassessment"],
            last_action="correct",
        ),
    )
    add(
        "multiple_ordered_rejections",
        ("multiple_rejections", "stale_root", "unauthorized", "suffix_continuation"),
        history(
            [
                *PREFIX,
                decision(3, correct(1, 10, 11), 7),
                decision(4, correct(1, 10, 11), 8, performer=404),
                decision(5, correct(1, 10, 11), 8),
            ],
            dependencies=DEPENDENCY,
        ),
        expected(
            9,
            ["superseded", "accepted", "accepted"],
            events=[1, 2, 5],
            rejections=[(8, "stale_root"), (9, "unauthorized")],
            reassessment=["unaffected", "unaffected", "needs_reassessment"],
            last_action="correct",
        ),
        lean=True,
    )
    add(
        "verification_before_submission_then_recovery",
        (
            "record_order",
            "unmatched_verification_noop",
            "ineligible",
            "suffix_continuation",
        ),
        history(
            [V10, S10, decision(1, accept(10), 1), V10, decision(2, accept(10), 2)]
        ),
        expected(3, ["accepted"], events=[2], rejections=[(2, "ineligible")]),
    )

    add(
        "plural_authority_accept",
        ("plural_authority", "same_external_evidence"),
        history([S10, V10, decision(1, accept(10), 2)]),
        expected(3, ["accepted"], events=[1]),
        lean=True,
    )
    add(
        "plural_authority_reject",
        ("plural_authority", "same_external_evidence"),
        history(
            [S10, V10, decision(1, reject(10), 2, repository=2, performer=202)],
            repository=2,
            authorized=[202],
        ),
        expected(3, ["unassessed"], events=[1]),
        lean=True,
    )

    for name, dependencies, reassessment in [
        (
            "dependency_present",
            DEPENDENCY,
            ["unaffected", "unaffected", "needs_reassessment"],
        ),
        ("dependency_absent", [], ["unaffected", "unaffected", "unaffected"]),
        (
            "dependency_unrelated",
            [{"dependent": 20, "depends_on": 99}],
            ["unaffected", "unaffected", "unaffected"],
        ),
    ]:
        add(
            name,
            ("descriptive_dependency_mutation", "standing_independence"),
            history([*PREFIX, FRESH], dependencies=dependencies),
            expected(
                9,
                ["superseded", "accepted", "accepted"],
                events=[1, 2, 3],
                reassessment=reassessment,
            ),
            lean=name != "dependency_unrelated",
        )

    identifiers = [row.id for row in rows]
    if len(set(identifiers)) != len(rows):
        raise AssertionError("case identifiers must be unique and stable")
    return rows


if __name__ == "__main__":
    import json

    summary = {
        "case_count": len(cases()),
        "case_ids": [case.id for case in cases()],
        "generator": GENERATOR,
    }
    print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
