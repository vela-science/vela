#!/usr/bin/env python3
"""Validate the split Lean axiom reports and their compatibility union."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


ALLOWED_PROTOCOL_AXIOMS = {"propext", "Quot.sound", "Classical.choice"}
DENIED_AXIOMS = {"sorryAx", "Lean.ofReduceBool", "Lean.trustCompiler"}
REPORTS = {
    "protocol": "Vela/ProtocolAxiomAudit.lean",
    "research": "Vela/ResearchAxiomAudit.lean",
    "combined": "Vela/AxiomAudit.lean",
}


def run_report(project: Path, report: str) -> dict[str, frozenset[str]]:
    result = subprocess.run(
        ["lake", "env", "lean", report],
        cwd=project,
        check=False,
        capture_output=True,
        text=True,
    )
    output = "\n".join((result.stdout, result.stderr))
    if result.returncode != 0:
        raise SystemExit(f"{report} failed:\n{output[-4000:]}")

    closures: dict[str, frozenset[str]] = {}
    for line in output.splitlines():
        match = re.search(r"AXIOMS\s+([^|]+?)\s*\|\s*(.*)$", line.strip())
        if not match:
            continue
        declaration = match.group(1).strip()
        if declaration in closures:
            raise SystemExit(f"{report}: duplicate declaration {declaration}")
        closures[declaration] = frozenset(
            axiom.strip() for axiom in match.group(2).split(",") if axiom.strip()
        )
    if not closures:
        raise SystemExit(f"{report}: no AXIOMS records emitted")
    return closures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--project",
        type=Path,
        default=Path("lean"),
        help="Lean project directory (default: lean)",
    )
    args = parser.parse_args()
    project = args.project.resolve()
    if not (project / "lakefile.lean").is_file():
        raise SystemExit(f"not a Lean project: {project}")

    reports = {
        name: run_report(project, path)
        for name, path in REPORTS.items()
    }
    protocol = reports["protocol"]
    research = reports["research"]
    combined = reports["combined"]

    overlap = sorted(set(protocol) & set(research))
    if overlap:
        raise SystemExit(f"protocol/research registry overlap: {', '.join(overlap)}")

    unexpected_protocol = {
        declaration: sorted(axioms - ALLOWED_PROTOCOL_AXIOMS)
        for declaration, axioms in protocol.items()
        if axioms - ALLOWED_PROTOCOL_AXIOMS
    }
    if unexpected_protocol:
        raise SystemExit(
            "protocol audit contains non-policy axioms: "
            + repr(unexpected_protocol)
        )

    denied = {
        declaration: sorted(axioms & DENIED_AXIOMS)
        for declaration, axioms in combined.items()
        if axioms & DENIED_AXIOMS
    }
    if denied:
        raise SystemExit(f"denied axioms found: {denied!r}")

    expected_combined = {**protocol, **research}
    if combined != expected_combined:
        missing = sorted(set(expected_combined) - set(combined))
        extra = sorted(set(combined) - set(expected_combined))
        changed = sorted(
            declaration
            for declaration in set(combined) & set(expected_combined)
            if combined[declaration] != expected_combined[declaration]
        )
        raise SystemExit(
            "combined audit drift: "
            f"missing={missing}, extra={extra}, changed={changed}"
        )

    conditional_research = sum(bool(axioms) for axioms in research.values())
    print(
        "Lean axiom audits agree: "
        f"{len(protocol)} protocol, {len(research)} research, "
        f"{conditional_research} research declarations with explicit axioms"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
