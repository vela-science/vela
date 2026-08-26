#!/usr/bin/env python3
"""Exhaustively check one retained two-variable Boolean equivalence."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
from pathlib import Path
from typing import Any


def evaluate(expression: dict[str, Any], assignment: dict[str, bool]) -> bool:
    if set(expression) == {"var"}:
        variable = expression["var"]
        if variable not in assignment:
            raise ValueError(f"unknown variable: {variable!r}")
        return assignment[variable]
    operation = expression.get("op")
    if operation == "not" and set(expression) == {"op", "arg"}:
        return not evaluate(expression["arg"], assignment)
    if operation in {"and", "or"} and set(expression) == {"op", "args"}:
        arguments = expression["args"]
        if not isinstance(arguments, list) or len(arguments) != 2:
            raise ValueError(f"{operation} requires exactly two arguments")
        values = [evaluate(argument, assignment) for argument in arguments]
        return all(values) if operation == "and" else any(values)
    raise ValueError(f"unsupported closed expression: {expression!r}")


def check(statement_path: Path) -> dict[str, Any]:
    statement_bytes = statement_path.read_bytes()
    statement = json.loads(statement_bytes)
    if set(statement) != {"description", "left", "right", "variables"}:
        raise ValueError("statement must use the closed four-field example shape")
    if statement["variables"] != ["p", "q"]:
        raise ValueError("this bounded verifier requires variables [p, q]")

    counterexamples = []
    for values in itertools.product([False, True], repeat=2):
        assignment = dict(zip(statement["variables"], values, strict=True))
        left = evaluate(statement["left"], assignment)
        right = evaluate(statement["right"], assignment)
        if left != right:
            counterexamples.append(
                {"assignment": assignment, "left": left, "right": right}
            )

    return {
        "checked_assignments": 4,
        "counterexamples": counterexamples,
        "outcome": "pass" if not counterexamples else "fail",
        "statement_description": statement["description"],
        "statement_sha256": hashlib.sha256(statement_bytes).hexdigest(),
        "verifier_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("statement", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-outcome", choices=["pass", "fail"])
    arguments = parser.parse_args()

    report = check(arguments.statement)
    arguments.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(
        f"finite-boolean-verifier: {report['outcome']} "
        f"({len(report['counterexamples'])} counterexamples)"
    )
    if arguments.expect_outcome is not None:
        return 0 if report["outcome"] == arguments.expect_outcome else 1
    return 0 if report["outcome"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
