#!/usr/bin/env python3
"""Validate frozen product-compression sources without participant execution."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any


PLAN_SCHEMA = "vela.product-compression-plan.v1"


class ValidationError(ValueError):
    """Raised when a frozen source or product fact drifts."""


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as error:
        raise ValidationError(f"cannot read {path}: {error}") from error


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValidationError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValidationError(f"{path} must contain one JSON object")
    return value


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def run(
    argv: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def git(repo: Path, *args: str) -> bytes:
    completed = run(["git", "-C", str(repo), *args])
    if completed.returncode != 0:
        raise ValidationError(
            f"git {' '.join(args)} failed in {repo}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    return completed.stdout


def git_text(repo: Path, *args: str) -> str:
    return git(repo, *args).decode().strip()


def command_json(
    binary: Path,
    args: list[str],
    *,
    expected_success: bool = True,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    completed = run([str(binary), *args], env=env)
    if expected_success and completed.returncode != 0:
        raise ValidationError(
            f"{binary.name} {' '.join(args)} failed: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if not expected_success and completed.returncode == 0:
        raise ValidationError(
            f"{binary.name} {' '.join(args)} unexpectedly succeeded"
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValidationError(
            f"{binary.name} {' '.join(args)} emitted invalid JSON"
        ) from error
    if not isinstance(value, dict):
        raise ValidationError("command JSON must be one object")
    return value


def verify_binary(binary: Path, expected: dict[str, Any]) -> None:
    require(binary.is_file(), f"missing binary {binary}")
    require(
        sha256_file(binary) == expected["sha256"],
        f"binary root drift for {binary}",
    )
    completed = run([str(binary), "--version"])
    require(completed.returncode == 0, f"cannot execute {binary}")
    require(
        completed.stdout.decode().strip() == expected["version"],
        f"binary version drift for {binary}",
    )


def verify_repository(
    name: str, repo: Path, expected: dict[str, Any]
) -> None:
    require(repo.is_dir(), f"missing {name} repository {repo}")
    require(
        git_text(repo, "rev-parse", "HEAD") == expected["commit"],
        f"{name} head drift",
    )
    require(
        git_text(repo, "rev-parse", "HEAD^{tree}") == expected["tree"],
        f"{name} tree drift",
    )


def verify_fixture_files(
    repo: Path, commit: str, fixtures: list[dict[str, Any]]
) -> None:
    for fixture in fixtures:
        content = git(repo, "show", f"{commit}:{fixture['path']}")
        require(
            sha256_bytes(content) == fixture["sha256"],
            f"fixture drift at {fixture['path']}",
        )


def verify_harness_files(
    vela_repository: Path, files: list[dict[str, Any]]
) -> None:
    for entry in files:
        path = vela_repository / entry["path"]
        require(
            sha256_file(path) == entry["sha256"],
            f"harness source drift at {entry['path']}",
        )


def verify_pre_output_refreeze(
    vela_repository: Path, plan: dict[str, Any]
) -> None:
    amendment = plan.get("amendment")
    if amendment is None:
        return
    require(isinstance(amendment, dict), "plan.amendment must be an object")
    require(
        amendment.get("kind") == "pre_output_environment_refreeze",
        "unsupported plan amendment",
    )
    require(
        amendment.get("participant_outputs_before_refreeze") == 0,
        "refreeze occurred after participant output",
    )
    for field in (
        "task_facts_changed",
        "expected_answers_changed",
        "arms_changed",
        "budgets_changed",
        "scoring_changed",
        "success_threshold_changed",
    ):
        require(amendment.get(field) is False, f"refreeze changed {field}")
    prior = amendment.get("prior_plan")
    require(isinstance(prior, dict), "amendment.prior_plan must be an object")
    relative_path = Path(prior.get("path", ""))
    require(
        bool(relative_path.parts)
        and not relative_path.is_absolute()
        and ".." not in relative_path.parts,
        "prior plan path must stay inside the Vela repository",
    )
    require(
        sha256_file(vela_repository / relative_path) == prior.get("sha256"),
        "prior plan root drift",
    )
    prior_plan = load_object(vela_repository / relative_path)
    require(
        prior_plan.get("schema") == plan.get("schema"),
        "refreeze changed the plan schema",
    )
    require(
        prior_plan.get("status") == plan.get("status"),
        "refreeze changed the plan status",
    )
    protected_sections = (
        "study",
        "frontiers",
        "current_lifecycle_fixtures",
        "terminal_correction_fixture",
        "task",
        "arms",
        "assignment",
        "budgets",
        "scoring",
        "stop_conditions",
        "publication",
    )
    for section in protected_sections:
        require(
            prior_plan.get(section) == plan.get(section),
            f"refreeze changed protected section {section}",
        )
    prior_vela = prior_plan.get("vela")
    current_vela = plan.get("vela")
    require(
        isinstance(prior_vela, dict) and isinstance(current_vela, dict),
        "both plans must bind Vela",
    )
    require(
        prior_vela.get("historical_binary")
        == current_vela.get("historical_binary"),
        "refreeze changed the historical Vela binary",
    )
    require(
        prior_vela.get("current_binary", {}).get("version")
        == current_vela.get("current_binary", {}).get("version"),
        "refreeze changed the current Vela version",
    )
    require(
        [entry.get("path") for entry in prior_vela.get("context_files", [])]
        == [entry.get("path") for entry in current_vela.get("context_files", [])],
        "refreeze changed the Vela context file set",
    )
    require(
        [entry.get("path") for entry in prior_plan.get("harness_files", [])]
        == [entry.get("path") for entry in plan.get("harness_files", [])],
        "refreeze changed the harness file set",
    )
    require(
        run(
            [
                "git",
                "-C",
                str(vela_repository),
                "merge-base",
                "--is-ancestor",
                prior_vela["source_commit"],
                current_vela["source_commit"],
            ]
        ).returncode
        == 0,
        "refrozen Vela source is not a descendant of the prior source",
    )


def verify_commit_files(
    repository: Path,
    commit: str,
    files: list[dict[str, Any]],
    label: str,
) -> None:
    for entry in files:
        content = git(repository, "show", f"{commit}:{entry['path']}")
        require(
            sha256_bytes(content) == entry["sha256"],
            f"{label} drift at {entry['path']}",
        )


def verify_current_selection(
    binary: Path,
    repositories: dict[str, Path],
    expected: dict[str, Any],
) -> None:
    key_by_name = {
        "erdos": "erdos",
        "formal_conjectures": "formal_conjectures",
        "quantum_codes": "quantum_codes",
        "sidon_sets": "sidon_sets",
    }
    observed_counts: dict[str, int] = {}
    for name, repo in repositories.items():
        offer = command_json(binary, ["next", str(repo), "--json"])
        require(offer.get("ok") is True, f"{name} next failed")
        observed_counts[key_by_name[name]] = offer["availability"]["fresh"]
        if name == "erdos":
            require(
                offer["frontier_id"]
                == expected["selection"]["chosen_frontier_id"],
                "chosen Frontier ID drift",
            )
            require(
                offer["repository_root"]
                == expected["selection"]["chosen_frontier_repository_root"],
                "chosen Frontier repository root drift",
            )
            require(len(offer["targets"]) == 1, "Erdős must expose one target")
            target = offer["targets"][0]
            expected_target = expected["target"]
            require(
                target["target_id"] == expected_target["target_id"],
                "first Target drift",
            )
            require(target["rank"] == expected_target["rank"], "Target rank drift")
            require(
                target["packet"]["sha256"] == expected_target["packet_root"],
                "Target packet root drift",
            )
            require(
                offer["target_index_root"]
                == expected_target["target_index_root"],
                "Target Index root drift",
            )
    require(
        observed_counts == expected["selection"]["fresh_target_counts"],
        f"fresh Target counts drift: {observed_counts}",
    )


def verify_current_inbox(
    binary: Path, erdos: Path, expected: dict[str, Any]
) -> None:
    inbox = command_json(binary, ["review", "inbox", str(erdos), "--json"])
    proposal_id = expected["inbox"]["proposal_id"]
    entries = [
        entry
        for entry in inbox.get("entries", [])
        if entry.get("proposal_id") == proposal_id
    ]
    require(len(entries) == 1, "expected Proposal is not unique in Decision Inbox")
    entry = entries[0]
    require(
        entry["entry_root"] == expected["inbox"]["entry_root"],
        "Decision Inbox entry root drift",
    )
    require(
        entry["readiness"]["protocol_gate"]
        == expected["inbox"]["protocol_gate"],
        "Decision Inbox readiness drift",
    )
    require(
        entry["readiness"]["human_decision_required"]
        == expected["inbox"]["human_decision_required"],
        "human Decision requirement drift",
    )
    require(
        entry["readiness"]["rejection_available"]
        == expected["inbox"]["rejection_available"],
        "rejection availability drift",
    )
    require(
        entry["standing_diff"]["transition"]
        == expected["inbox"]["standing_diff"]["transition"],
        "Standing transition drift",
    )
    require(
        entry["standing_diff"]["target_claim_id"]
        == expected["inbox"]["standing_diff"]["target_claim_id"],
        "Standing target Claim drift",
    )
    verifier_ids = {
        record["verification_record_id"]
        for record in entry["verification_records"]
    }
    require(
        expected["inbox"]["verification_record_id"] in verifier_ids,
        "expected Verification is absent from Decision Inbox",
    )


def isolated_environment(home: Path) -> dict[str, str]:
    return {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "LC_ALL": "C",
        "TZ": "UTC",
    }


def clone_at(source: Path, destination: Path, commit: str) -> None:
    cloned = run(
        [
            "git",
            "clone",
            "--quiet",
            "--no-local",
            "--no-hardlinks",
            str(source),
            str(destination),
        ]
    )
    require(cloned.returncode == 0, "fixture clone failed")
    require(
        run(["git", "-C", str(destination), "checkout", "--quiet", commit]).returncode
        == 0,
        "fixture checkout failed",
    )
    require(
        run(["git", "-C", str(destination), "remote", "remove", "origin"]).returncode
        == 0,
        "cannot remove fixture remote",
    )


def verify_attempt_and_replay_boundary(
    binary: Path,
    erdos: Path,
    current_commit: str,
    expected: dict[str, Any],
) -> None:
    with tempfile.TemporaryDirectory(
        prefix="vela-product-compression-attempt-"
    ) as temporary:
        root = Path(temporary)
        clone = root / "frontier"
        home = root / "home"
        home.mkdir()
        clone_at(erdos, clone, current_commit)
        environment = isolated_environment(home)
        started = command_json(
            binary,
            [
                "start",
                expected["target"]["target_id"],
                "--frontier",
                str(clone),
                "--as",
                expected["attempt"]["required_actor"],
                "--json",
            ],
            env=environment,
        )
        require(started["ok"] is True, "private Attempt did not start")
        require(
            started["authorization"]["allowed_operations"]
            == expected["attempt"]["allowed_operations"],
            "Attempt operations drift",
        )
        require(
            started["authorization"]["allowed_artifact_classes"]
            == expected["attempt"]["allowed_artifact_classes"],
            "Attempt Artifact classes drift",
        )
        require(
            started["authorization"]["budget"] == expected["attempt"]["budget"],
            "Attempt budget drift",
        )
        require(
            started["authorization"]["consequence_ceiling"]
            == expected["attempt"]["consequence_ceiling"],
            "Attempt consequence ceiling drift",
        )
        require(
            started["starting_roots"]["task_contract"]
            == expected["attempt"]["task_contract_root"],
            "Attempt task contract drift",
        )
        require(
            started["canonical_write"] is False
            and started["authority_key_read"] is False,
            "Attempt crossed the authority boundary",
        )
        require(
            not git(clone, "status", "--porcelain=v1", "--untracked-files=all"),
            "private Attempt dirtied tracked or untracked Git state",
        )

        replay = command_json(
            binary,
            [
                "reproduce",
                str(clone),
                "--proposal",
                expected["inbox"]["proposal_id"],
                "--json",
            ],
            expected_success=False,
            env=environment,
        )
        message = replay.get("error", {}).get("message", "")
        require(
            "has no frontier-local frozen witness to reproduce" in message
            and "producer's exact replay bundle" in message,
            "replay-boundary diagnostic drift",
        )


def verify_terminal_correction(
    binary: Path,
    erdos: Path,
    expected: dict[str, Any],
) -> None:
    correction = expected["terminal_correction"]
    with tempfile.TemporaryDirectory(
        prefix="vela-product-compression-correction-"
    ) as temporary:
        root = Path(temporary)
        clone = root / "frontier"
        clone_at(erdos, clone, correction["fixture_commit"])
        review = command_json(
            binary,
            [
                "review",
                "show",
                str(clone),
                correction["proposal_id"],
                "--json",
            ],
        )
        require(review["standing"] == "accepted", "correction Standing drift")
        require(
            review["claim"]["claim_id"]
            == correction["replacement"]["claim_id"],
            "correction replacement drift",
        )
        require(
            review["decision"]["event_id"] == correction["decision_event_id"],
            "correction Decision drift",
        )
        require(
            review["decision"]["event_root"]
            == correction["decision_event_root"],
            "correction Decision root drift",
        )
        require(
            review["decision"]["applied_event_id"]
            == correction["applied_event_id"],
            "correction applied Event drift",
        )
        verification_ids = {
            item["record"]["verification_record_id"]
            for item in review["verification_records"]
        }
        require(
            correction["verification_record_id"] in verification_ids,
            "correction Verification drift",
        )


def validate(args: argparse.Namespace) -> dict[str, Any]:
    plan = load_object(args.plan)
    require(plan.get("schema") == PLAN_SCHEMA, "wrong plan schema")
    require(
        plan.get("status") == "frozen_before_participant_output",
        "plan is not frozen before participant output",
    )
    expected = load_object(args.answer_key)["expected"]
    vela_repository = args.vela_repository.resolve()
    verify_pre_output_refreeze(vela_repository, plan)
    require(
        git_text(vela_repository, "cat-file", "-t", plan["vela"]["source_commit"])
        == "commit",
        "frozen Vela source commit is unavailable",
    )
    require(
        git_text(
            vela_repository,
            "rev-parse",
            f"{plan['vela']['source_commit']}^{{tree}}",
        )
        == plan["vela"]["source_tree"],
        "frozen Vela source tree drift",
    )
    verify_commit_files(
        vela_repository,
        plan["vela"]["source_commit"],
        plan["vela"]["context_files"],
        "frozen Vela context",
    )
    verify_harness_files(vela_repository, plan["harness_files"])
    verify_binary(args.current_vela.resolve(), plan["vela"]["current_binary"])
    verify_binary(
        args.historical_vela.resolve(), plan["vela"]["historical_binary"]
    )

    repositories = {
        "erdos": args.erdos_frontier.resolve(),
        "formal_conjectures": args.formal_frontier.resolve(),
        "quantum_codes": args.quantum_frontier.resolve(),
        "sidon_sets": args.sidon_frontier.resolve(),
    }
    for name, repo in repositories.items():
        verify_repository(name, repo, plan["frontiers"][name])

    # All product reads use fresh, remote-free exact-commit clones. Ignored
    # private Attempt state or other mutable checkout-local files must not
    # influence a frozen fixture.
    with tempfile.TemporaryDirectory(
        prefix="vela-product-compression-current-"
    ) as temporary:
        clone_root = Path(temporary)
        exact_repositories: dict[str, Path] = {}
        for name, source in repositories.items():
            destination = clone_root / name
            clone_at(source, destination, plan["frontiers"][name]["commit"])
            exact_repositories[name] = destination
        verify_fixture_files(
            exact_repositories["erdos"],
            plan["frontiers"]["erdos"]["commit"],
            plan["current_lifecycle_fixtures"],
        )
        verify_current_selection(
            args.current_vela.resolve(), exact_repositories, expected
        )
        verify_current_inbox(
            args.current_vela.resolve(), exact_repositories["erdos"], expected
        )
        verify_attempt_and_replay_boundary(
            args.current_vela.resolve(),
            exact_repositories["erdos"],
            plan["frontiers"]["erdos"]["commit"],
            expected,
        )
    verify_terminal_correction(
        args.historical_vela.resolve(), repositories["erdos"], expected
    )
    result_without_root = {
        "schema": "vela.product-compression-plan-validation.v1",
        "plan_root": sha256_file(args.plan),
        "valid": True,
        "participant_outputs_created": False,
        "frontier_count": len(repositories),
        "current_target_count": sum(
            expected["selection"]["fresh_target_counts"].values()
        ),
        "authority_actions": 0,
    }
    return {
        **result_without_root,
        "result_root": sha256_bytes(
            json.dumps(
                result_without_root,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument(
        "--answer-key",
        type=Path,
        default=Path(__file__).with_name("answer-key.v1.json"),
    )
    parser.add_argument("--vela-repository", type=Path, required=True)
    parser.add_argument("--current-vela", type=Path, required=True)
    parser.add_argument("--historical-vela", type=Path, required=True)
    parser.add_argument("--erdos-frontier", type=Path, required=True)
    parser.add_argument("--formal-frontier", type=Path, required=True)
    parser.add_argument("--quantum-frontier", type=Path, required=True)
    parser.add_argument("--sidon-frontier", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    try:
        result = validate(parse_args())
    except ValidationError as error:
        print(f"error: {error}")
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
