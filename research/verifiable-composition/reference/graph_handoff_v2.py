#!/usr/bin/env python3
"""Independent bitset verifier for first-party graph handoff parity."""

from __future__ import annotations

import hashlib
import json
from typing import Any


class GraphV2Error(ValueError):
    pass


def encoded(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def digest(value: Any) -> str:
    return f"sha256:{hashlib.sha256(encoded(value)).hexdigest()}"


def bit_graph(value: dict[str, Any]) -> tuple[int, tuple[int, ...]]:
    if set(value) != {"edges", "vertices"}:
        raise GraphV2Error("parent_shape")
    size = value["vertices"]
    if not isinstance(size, int) or isinstance(size, bool) or size < 1:
        raise GraphV2Error("vertices")
    masks = [0] * size
    previous: tuple[int, int] | None = None
    for edge in value["edges"]:
        if not isinstance(edge, list) or len(edge) != 2:
            raise GraphV2Error("edge_shape")
        left, right = edge
        if (
            not isinstance(left, int)
            or isinstance(left, bool)
            or not isinstance(right, int)
            or isinstance(right, bool)
            or not 0 <= left < right < size
        ):
            raise GraphV2Error("edge_canonical")
        pair = (left, right)
        if previous is not None and pair <= previous:
            raise GraphV2Error("edge_order")
        previous = pair
        masks[left] |= 1 << right
        masks[right] |= 1 << left
    return size, tuple(masks)


def no_triangle(masks: tuple[int, ...]) -> bool:
    return all(
        masks[left] & masks[right] == 0
        for left, mask in enumerate(masks)
        for right in range(left + 1, len(masks))
        if mask & (1 << right)
    )


def colorable(masks: tuple[int, ...], colors: int) -> tuple[int, ...] | None:
    assigned = [-1] * len(masks)
    order = sorted(
        range(len(masks)),
        key=lambda vertex: (-masks[vertex].bit_count(), vertex),
    )

    def visit(position: int) -> bool:
        if position == len(order):
            return True
        vertex = order[position]
        forbidden = 0
        neighbors = masks[vertex]
        for other, color in enumerate(assigned):
            if color >= 0 and neighbors & (1 << other):
                forbidden |= 1 << color
        for color in range(colors - 1, -1, -1):
            if forbidden & (1 << color):
                continue
            assigned[vertex] = color
            if visit(position + 1):
                return True
            assigned[vertex] = -1
        return False

    return tuple(assigned) if visit(0) else None


def valid_coloring(
    masks: tuple[int, ...], witness: list[int], colors: int
) -> None:
    if len(witness) != len(masks) or any(
        not isinstance(color, int)
        or isinstance(color, bool)
        or not 0 <= color < colors
        for color in witness
    ):
        raise GraphV2Error("colouring_shape")
    for left, mask in enumerate(masks):
        for right in range(left + 1, len(masks)):
            if mask & (1 << right) and witness[left] == witness[right]:
                raise GraphV2Error("colouring_edge")


def verify_parent(case: dict[str, Any]) -> dict[str, Any]:
    source = case["graph_a"]
    value = {"edges": source["edges"], "vertices": source["vertices"]}
    _, masks = bit_graph(value)
    if digest(value) != source["canonical_graph_root"]:
        raise GraphV2Error("parent_root")
    if not no_triangle(masks):
        raise GraphV2Error("parent_triangle")
    if colorable(masks, 3) is not None or colorable(masks, 4) is None:
        raise GraphV2Error("parent_chromatic")
    return {"graph_root": digest(value), "vertices": len(masks)}


def derived_child(parent: dict[str, Any]) -> dict[str, Any]:
    size, masks = bit_graph(parent)
    edges: set[tuple[int, int]] = set()
    for left in range(size):
        for right in range(left + 1, size):
            if not masks[left] & (1 << right):
                continue
            edges.add((left, right))
            edges.add(tuple(sorted((left, size + right))))
            edges.add(tuple(sorted((right, size + left))))
    apex = 2 * size
    for duplicate in range(size, 2 * size):
        edges.add((duplicate, apex))
    return {
        "edges": [[left, right] for left, right in sorted(edges)],
        "vertices": 2 * size + 1,
    }


def verify_child(
    parent_bytes: bytes,
    child: dict[str, Any],
    witness: list[int],
) -> dict[str, Any]:
    parent = json.loads(parent_bytes)
    if not isinstance(parent, dict):
        raise GraphV2Error("parent_shape")
    expected = derived_child(parent)
    if encoded(child) != encoded(expected):
        raise GraphV2Error("child_derivation")
    _, masks = bit_graph(child)
    if not no_triangle(masks):
        raise GraphV2Error("child_triangle")
    valid_coloring(masks, witness, 5)
    if colorable(masks, 4) is not None or colorable(masks, 5) is None:
        raise GraphV2Error("child_chromatic")
    return {
        "parent_root": digest(parent),
        "child_root": digest(child),
        "vertices": len(masks),
    }
