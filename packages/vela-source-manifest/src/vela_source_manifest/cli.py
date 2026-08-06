"""``vela-source-lock`` — write or check a Frontier's ``sources.lock.json``.

Run it from a Frontier's root:

    vela-source-lock                # write sources.lock.json
    vela-source-lock --check        # verify the committed one, offline
    vela-source-lock --check --refetch
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any, Sequence

from . import schema
from .resolver import DECLARATION_FILE, LOCK_FILE, write_sources_lock
from .verify import check

FAILED = (
    "\nNothing above was guessed at. Fix the source or the declaration; "
    "do not hand-edit the lock."
)


def summarize(name: str, entry: dict[str, Any]) -> str:
    if "sha256" in entry:
        state = entry["sha256"]
    elif "exact_roots" in entry:
        state = f"{len(entry['exact_roots'])} exact roots at {entry['commit'][:12]}"
    elif "error" in entry:
        state = "ERROR: " + entry["error"]
    else:
        state = "unlocked: " + entry["unlocked"].split(":")[0]
    return f"  {name:>28}  {state}"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vela-source-lock",
        description=(
            f"Read a Frontier's {DECLARATION_FILE} and write its {LOCK_FILE}, "
            "computing every content root from bytes actually fetched or read."
        ),
    )
    parser.add_argument(
        "root", nargs="?", default=".", type=Path, help="the Frontier root (default: .)"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help=f"verify the committed {LOCK_FILE} without writing; offline unless --refetch",
    )
    parser.add_argument(
        "--refetch",
        action="store_true",
        help=(
            "with --check, also re-resolve every source and report where upstream has "
            "moved. Off by default: a lock records a moment, and some are stale on purpose."
        ),
    )
    parser.add_argument(
        "--print-schema",
        metavar="NAME",
        choices=schema.SCHEMA_NAMES,
        help="print a bundled JSON Schema to stdout and exit (%(choices)s)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    if args.print_schema:
        sys.stdout.write(schema.schema_text(args.print_schema))
        return 0

    if args.refetch and not args.check:
        sys.stderr.write("--refetch only means anything with --check\n")
        return 2

    if args.check:
        problems = check(args.root, refetch=args.refetch)
        if problems:
            sys.stderr.write(
                f"{args.root / LOCK_FILE} does not check out:\n  "
                + "\n  ".join(problems)
                + FAILED
                + "\n"
            )
            return 1
        sys.stdout.write(f"{args.root / LOCK_FILE} checks out\n")
        return 0

    resolution = write_sources_lock(args.root)
    for name, entry in sorted(resolution.payload["sources"].items()):
        sys.stdout.write(summarize(name, entry) + "\n")

    if resolution.problems:
        # The lock is on disk either way. Reporting the failure after writing is
        # the point: a run that could not pin something leaves the gap in the
        # file, where the next reader will see it, rather than only in a terminal.
        sys.stderr.write(
            f"{LOCK_FILE} written, but the run FAILED:\n  "
            + "\n  ".join(resolution.problems)
            + FAILED
            + "\n"
        )
        return 1
    # No timestamp: the lock carries none, and printing one here would suggest
    # the file does. What a reader wants after this line is `git diff`, which
    # now says something, because an unchanged inventory rewrites byte for byte.
    sys.stdout.write(f"wrote {LOCK_FILE}: {len(resolution.payload['sources'])} sources\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
