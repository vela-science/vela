#!/usr/bin/env python3
"""Focused vectors for deterministic parent/child graph handoff artifacts."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "reference"))

from graph_handoff import GraphError, canonical_bytes, child_artifacts, parent_artifacts  # noqa: E402


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def mutate(case: dict, operation: str) -> None:
    graph = case["graph_a"]
    if operation == "none":
        return
    if operation == "reverse_edges":
        graph["edges"] = list(reversed(graph["edges"]))
    elif operation == "add_triangle":
        graph["edges"] = sorted(graph["edges"] + [[0, 2]])
    elif operation == "wrong_root":
        graph["canonical_graph_root"] = "sha256:" + "f" * 64
    elif operation == "substitute_parent":
        graph["edges"] = graph["edges"][:-1]
    else:
        raise AssertionError(operation)


def main() -> None:
    registered = json.loads((ROOT / "registration/graph-case.json").read_text())
    vectors = json.loads((ROOT / "vectors/graph-handoff-cases.json").read_text())
    checked = 0
    for vector in vectors["cases"]:
        case = copy.deepcopy(registered)
        mutate(case, vector["mutation"])
        try:
            parent = parent_artifacts(case)
            child = child_artifacts(canonical_bytes(parent["graph"]))
            require(child["parent_root"] == parent["graph_root"], "parent root not consumed")
            require(child["child"]["vertices"] == 23, "child vertex count")
            actual = "pass"
        except GraphError as error:
            actual = str(error)
        require(actual == vector["expected"], f"{vector['id']}: {actual}")
        checked += 1
    print(f"graph-handoff: {checked}/{checked} vectors; exact parent consumed; child chi=5")


if __name__ == "__main__":
    main()
