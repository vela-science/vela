#!/usr/bin/env python3
"""Deterministic graph artifacts for the authority-free ADR 0006 rehearsal."""

from __future__ import annotations

import hashlib
import json
from typing import Any


class GraphError(ValueError):
    pass


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def root(value: Any) -> str:
    return f"sha256:{hashlib.sha256(canonical_bytes(value)).hexdigest()}"


def adjacency(vertices: int, edges: list[list[int]]) -> list[set[int]]:
    if not isinstance(vertices, int) or isinstance(vertices, bool) or vertices < 1:
        raise GraphError("vertices")
    graph = [set() for _ in range(vertices)]
    previous: tuple[int, int] | None = None
    for edge in edges:
        if not isinstance(edge, list) or len(edge) != 2:
            raise GraphError("edge_shape")
        left, right = edge
        if (
            not isinstance(left, int)
            or isinstance(left, bool)
            or not isinstance(right, int)
            or isinstance(right, bool)
            or not 0 <= left < right < vertices
        ):
            raise GraphError("edge_canonical")
        pair = (left, right)
        if previous is not None and pair <= previous:
            raise GraphError("edge_order")
        previous = pair
        graph[left].add(right)
        graph[right].add(left)
    return graph


def triangle_free(graph: list[set[int]]) -> bool:
    return all(
        not graph[left].intersection(graph[right])
        for left in range(len(graph))
        for right in graph[left]
        if left < right
    )


def colouring(graph: list[set[int]], colors: int) -> list[int] | None:
    assigned = [-1] * len(graph)

    def search(done: int) -> bool:
        if done == len(graph):
            return True
        remaining = [vertex for vertex, color in enumerate(assigned) if color < 0]
        vertex = max(
            remaining,
            key=lambda item: (
                len({assigned[n] for n in graph[item] if assigned[n] >= 0}),
                len(graph[item]),
                -item,
            ),
        )
        forbidden = {assigned[n] for n in graph[vertex] if assigned[n] >= 0}
        for color in range(colors):
            if color in forbidden:
                continue
            assigned[vertex] = color
            if search(done + 1):
                return True
            assigned[vertex] = -1
        return False

    return assigned if search(0) else None


def verify_colouring(graph: list[set[int]], witness: list[int], colors: int) -> None:
    if len(witness) != len(graph) or any(
        not isinstance(color, int) or isinstance(color, bool) or not 0 <= color < colors
        for color in witness
    ):
        raise GraphError("colouring_shape")
    if any(
        witness[left] == witness[right]
        for left in range(len(graph))
        for right in graph[left]
        if left < right
    ):
        raise GraphError("colouring_edge")


def dimacs(graph: list[set[int]], colors: int) -> bytes:
    def variable(vertex: int, color: int) -> int:
        return vertex * colors + color + 1

    clauses: list[list[int]] = []
    for vertex in range(len(graph)):
        clauses.append([variable(vertex, color) for color in range(colors)])
        for left in range(colors):
            for right in range(left + 1, colors):
                clauses.append([-variable(vertex, left), -variable(vertex, right)])
    for left in range(len(graph)):
        for right in sorted(graph[left]):
            if left < right:
                for color in range(colors):
                    clauses.append([-variable(left, color), -variable(right, color)])
    lines = [f"p cnf {len(graph) * colors} {len(clauses)}"]
    lines.extend(" ".join(map(str, clause)) + " 0" for clause in clauses)
    return ("\n".join(lines) + "\n").encode()


def mycielski(graph: list[set[int]]) -> list[set[int]]:
    size = len(graph)
    child = [set() for _ in range(2 * size + 1)]
    apex = 2 * size
    for left in range(size):
        for right in graph[left]:
            if left >= right:
                continue
            for a, b in (
                (left, right),
                (left, size + right),
                (right, size + left),
            ):
                child[a].add(b)
                child[b].add(a)
    for duplicate in range(size, 2 * size):
        child[duplicate].add(apex)
        child[apex].add(duplicate)
    return child


def graph_object(graph: list[set[int]]) -> dict[str, Any]:
    return {
        "edges": [
            [left, right]
            for left in range(len(graph))
            for right in sorted(graph[left])
            if left < right
        ],
        "vertices": len(graph),
    }


def parent_artifacts(case: dict[str, Any]) -> dict[str, Any]:
    source = case["graph_a"]
    parent = {"edges": source["edges"], "vertices": source["vertices"]}
    graph = adjacency(parent["vertices"], parent["edges"])
    if root(parent) != source["canonical_graph_root"]:
        raise GraphError("parent_root")
    if not triangle_free(graph):
        raise GraphError("parent_triangle")
    four = colouring(graph, 4)
    if four is None or colouring(graph, 3) is not None:
        raise GraphError("parent_chromatic")
    verify_colouring(graph, four, 4)
    return {
        "graph": parent,
        "graph_root": root(parent),
        "four_colouring": four,
        "three_colour_dimacs": dimacs(graph, 3).decode(),
    }


def child_artifacts(parent_bytes: bytes) -> dict[str, Any]:
    parent = json.loads(parent_bytes)
    if not isinstance(parent, dict) or set(parent) != {"edges", "vertices"}:
        raise GraphError("parent_shape")
    graph = adjacency(parent["vertices"], parent["edges"])
    child = mycielski(graph)
    child_object = graph_object(child)
    five = colouring(child, 5)
    if five is None or colouring(child, 4) is not None:
        raise GraphError("child_chromatic")
    if not triangle_free(child):
        raise GraphError("child_triangle")
    verify_colouring(child, five, 5)
    return {
        "parent_root": root(parent),
        "child": child_object,
        "child_root": root(child_object),
        "five_colouring": five,
        "four_colour_dimacs": dimacs(child, 4).decode(),
    }
