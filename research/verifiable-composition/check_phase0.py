#!/usr/bin/env python3
"""Dependency-free checks for the frozen ADR 0004 Phase 0 registration."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent
REGISTRATION = ROOT / "registration"
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_RELEASE = {
    "repository": "https://github.com/vela-science/vela.git",
    "tag": "v0.800.12",
    "commit": "80c2bfd84c9ce248bb130afe22ac17c5273a7c7a",
    "binaries": {
        "linux_x86_64": {
            "asset": "vela-linux-x86_64",
            "kind": "vela_cli_executable",
            "platform": "linux-x86_64",
            "bytes": 31018440,
            "sha256": "sha256:6cda3f7bfcf929a9004182a79a1f504a5ff0c6448e471616e3aeabf7e4c1a260",
        },
        "macos_aarch64": {
            "asset": "vela-macos-aarch64",
            "kind": "vela_cli_executable",
            "platform": "macos-aarch64",
            "bytes": 25229360,
            "sha256": "sha256:1d63b2fa1a979045c3a79776508ecbc1eae2ac76225d2d819879767f11ca23fb",
        },
    },
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load(path: Path) -> object:
    def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate object name {key!r} in {path}")
            result[key] = value
        return result

    return json.loads(path.read_text(), object_pairs_hook=no_duplicates)


def canonical_root(value: object) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def adjacency(vertices: int, edges: list[list[int]]) -> list[set[int]]:
    graph = [set() for _ in range(vertices)]
    previous: tuple[int, int] | None = None
    for edge in edges:
        require(len(edge) == 2, f"edge is not a pair: {edge}")
        left, right = edge
        require(0 <= left < right < vertices, f"edge is not canonical: {edge}")
        pair = (left, right)
        require(
            previous is None or previous < pair, f"edges not strictly sorted: {edge}"
        )
        previous = pair
        graph[left].add(right)
        graph[right].add(left)
    return graph


def is_triangle_free(graph: list[set[int]]) -> bool:
    return all(
        not (graph[left] & graph[right])
        for left in range(len(graph))
        for right in graph[left]
        if left < right
    )


def is_k_colorable(graph: list[set[int]], colors: int) -> bool:
    assigned = [-1] * len(graph)

    def choose_vertex() -> int:
        remaining = [index for index, color in enumerate(assigned) if color < 0]
        return max(
            remaining,
            key=lambda index: (
                len({assigned[n] for n in graph[index] if assigned[n] >= 0}),
                len(graph[index]),
                -index,
            ),
        )

    def search(colored: int) -> bool:
        if colored == len(graph):
            return True
        vertex = choose_vertex()
        forbidden = {assigned[n] for n in graph[vertex] if assigned[n] >= 0}
        for color in range(colors):
            if color in forbidden:
                continue
            assigned[vertex] = color
            if search(colored + 1):
                return True
            assigned[vertex] = -1
        return False

    return search(0)


def mycielski(graph: list[set[int]]) -> list[set[int]]:
    size = len(graph)
    child = [set() for _ in range(2 * size + 1)]
    apex = 2 * size
    for left in range(size):
        for right in graph[left]:
            if left >= right:
                continue
            child[left].add(right)
            child[right].add(left)
            child[left].add(size + right)
            child[size + right].add(left)
            child[right].add(size + left)
            child[size + left].add(right)
    for duplicate in range(size, 2 * size):
        child[duplicate].add(apex)
        child[apex].add(duplicate)
    return child


def main() -> None:
    graph_case = load(REGISTRATION / "graph-case.json")
    phase0 = load(REGISTRATION / "phase0.json")
    interventions = load(REGISTRATION / "intervention-log.json")
    require(isinstance(graph_case, dict), "graph case must be an object")
    require(isinstance(phase0, dict), "Phase 0 registration must be an object")
    require(isinstance(interventions, dict), "intervention log must be an object")

    registered_root = phase0["case"]["canonical_root"]
    require(
        registered_root == canonical_root(graph_case), "graph registration root drift"
    )
    require(bool(SHA256.fullmatch(registered_root)), "graph root is not a full SHA-256")
    require(
        bool(COMMIT.fullmatch(phase0["release"]["commit"])),
        "release commit is not a full Git SHA-1 object ID",
    )
    require(
        all(
            SHA256.fullmatch(value["sha256"])
            for value in phase0["release"]["binaries"].values()
        ),
        "release binary root is not a full SHA-256",
    )
    require(
        phase0["release"] == EXPECTED_RELEASE,
        "exact registered v0.800.12 release coordinates drifted",
    )
    require(phase0["run_class"] == "internal_fixture", "run class widened")
    require(
        phase0["approval"]["adr_0003_remains_active"] is True,
        "ADR 0003 active-goal boundary changed",
    )
    require(
        interventions["run_class"] == "internal_fixture",
        "intervention log run class widened",
    )
    require(interventions["entries"] == [], "unexpected support intervention")

    graph_a = graph_case["graph_a"]
    graph = adjacency(graph_a["vertices"], graph_a["edges"])
    graph_bytes = {
        "edges": graph_a["edges"],
        "vertices": graph_a["vertices"],
    }
    require(
        graph_a["canonical_graph_encoding"]
        == "RFC 8785 canonical JSON of exactly the object with keys edges and vertices from this graph_a record",
        "canonical graph encoding drift",
    )
    require(
        graph_a["canonical_graph_root"] == canonical_root(graph_bytes),
        "canonical graph byte root drift",
    )
    require(
        graph_a["registered_properties"]
        == {"triangle_free": True, "chromatic_number": 4},
        "registered graph properties drift",
    )
    require(len(graph_a["edges"]) == 20, "registered graph edge count drift")
    require(is_triangle_free(graph), "registered graph is not triangle-free")
    require(not is_k_colorable(graph, 3), "registered graph is 3-colorable")
    require(is_k_colorable(graph, 4), "registered graph is not 4-colorable")

    child = mycielski(graph)
    expected_child = graph_case["child_task"]["registered_properties"]
    require(
        expected_child
        == {"vertices": 23, "triangle_free": True, "chromatic_number": 5},
        "registered child properties drift",
    )
    require(
        len(child) == expected_child["vertices"],
        "registered child vertex count drift",
    )
    require(
        expected_child["triangle_free"] is True and is_triangle_free(child),
        "registered child is not triangle-free",
    )
    require(not is_k_colorable(child, 4), "registered child is 4-colorable")
    require(is_k_colorable(child, 5), "registered child is not 5-colorable")

    print(
        "phase0: full-shape v0.800.12 pin recorded; internal-fixture boundary; "
        "G(11,20) triangle-free chi=4; M(G)(23) triangle-free chi=5"
    )


if __name__ == "__main__":
    main()
