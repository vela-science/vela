#!/usr/bin/env python3
"""Exact four-cell periodic heat-step reference computation."""

from __future__ import annotations

import argparse
import json
from fractions import Fraction
from pathlib import Path


def encode(value: Fraction) -> str:
    return f"{value.numerator}/{value.denominator}"


def compute() -> dict[str, object]:
    state = [Fraction(100), Fraction(0), Fraction(0), Fraction(0)]
    states = [[encode(value) for value in state]]
    totals = [encode(sum(state))]
    for _ in range(16):
        state = [
            (state[(index - 1) % 4] + 2 * state[index] + state[(index + 1) % 4])
            / 4
            for index in range(4)
        ]
        states.append([encode(value) for value in state])
        totals.append(encode(sum(state)))
    return {
        "schema": "vela.example.periodic-heat-result.v1",
        "method": "u_i(t+1) = (u_(i-1)(t) + 2*u_i(t) + u_(i+1)(t)) / 4",
        "arithmetic": "exact rational",
        "boundary": "periodic four-cell grid",
        "steps": 16,
        "states": states,
        "totals": totals,
        "claim": "The exact discrete total is 100 at every retained step.",
        "does_not_establish": [
            "Accuracy for a continuum heat equation.",
            "Scientific acceptance or Standing in any Vela Repository.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", type=Path)
    arguments = parser.parse_args()
    expected = compute()
    rendered = json.dumps(expected, indent=2, sort_keys=True) + "\n"
    if arguments.check is None:
        print(rendered, end="")
        return 0
    observed = arguments.check.read_text(encoding="utf-8")
    if observed != rendered:
        raise SystemExit(f"{arguments.check} does not match the exact computation")
    if set(expected["totals"]) != {"100/1"}:
        raise SystemExit("the retained total is not conserved")
    print("periodic-heat-example: exact total conserved for 16 steps")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
