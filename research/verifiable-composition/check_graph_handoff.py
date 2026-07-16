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
from graph_handoff_v2 import GraphV2Error, verify_child, verify_parent  # noqa: E402


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
            v2_parent = verify_parent(case)
            child = child_artifacts(canonical_bytes(parent["graph"]))
            v2_child = verify_child(
                canonical_bytes(parent["graph"]),
                child["child"],
                child["five_colouring"],
            )
            require(child["parent_root"] == parent["graph_root"], "parent root not consumed")
            require(v2_parent["graph_root"] == parent["graph_root"], "V2 parent root")
            require(v2_child["child_root"] == child["child_root"], "V2 child root")
            require(child["child"]["vertices"] == 23, "child vertex count")
            actual = "pass"
        except (GraphError, GraphV2Error) as error:
            actual = str(error)
        require(actual == vector["expected"], f"{vector['id']}: {actual}")
        checked += 1
    parent = parent_artifacts(registered)
    parent_bytes = canonical_bytes(parent["graph"])
    child = child_artifacts(parent_bytes)
    for vector in vectors["child_cases"]:
        delivered_parent = parent_bytes
        delivered_child = copy.deepcopy(child["child"])
        delivered_colouring = list(child["five_colouring"])
        if vector["mutation"] == "child_edge":
            delivered_child["edges"] = delivered_child["edges"][:-1]
        elif vector["mutation"] == "child_colouring":
            edge = delivered_child["edges"][0]
            delivered_colouring[edge[1]] = delivered_colouring[edge[0]]
        elif vector["mutation"] == "delivered_parent":
            substituted = copy.deepcopy(parent["graph"])
            substituted["edges"] = substituted["edges"][:-1]
            delivered_parent = canonical_bytes(substituted)
        elif vector["mutation"] != "none":
            raise AssertionError(vector["mutation"])
        try:
            verify_child(delivered_parent, delivered_child, delivered_colouring)
            actual = "pass"
        except GraphV2Error as error:
            actual = str(error)
        require(actual == vector["expected"], f"{vector['id']}: {actual}")
        checked += 1
    print(
        f"graph-handoff: {checked}/{checked} vectors; V1/V2 parity; "
        "exact parent consumed; child chi=5"
    )


if __name__ == "__main__":
    main()
