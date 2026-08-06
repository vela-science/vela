"""Check a committed ``sources.lock.json`` without writing one.

This is what CI runs. It is deliberately offline by default, and that choice is
worth stating: a lock is a record of what a Frontier acquired *at a moment*, so
upstream having moved since is not by itself a defect in the lock. Some locks are
stale on purpose — the Erdős Frontier's live-fetched pins record what was
acquired when its Claims were accepted, and regenerating them would destroy the
evidence rather than refresh it. A CI job that refetched on every run would go
red for the healthiest possible reason and teach everyone to ignore it.

So the default check asks only what can be answered from the two files and the
working tree:

  * both files match their schemas, including the reader invariant;
  * the lock covers exactly the sources declared, no more and no fewer;
  * every field the resolver copies through verbatim still agrees with the
    declaration, and a declared commit or tree still agrees with the locked one;
  * every in-repository content root still matches the bytes on disk;
  * no entry carries an ``error``.

`--refetch` adds the network question — do the recorded roots still describe what
upstream serves — for the caller who actually wants to ask it.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .resolver import (
    DECLARATION_FILE,
    LOCK_FILE,
    Fetch,
    read_declaration,
    read_lock,
    resolve,
    sha256,
)

# Fields the resolver copies from the declaration without interpreting them. If
# one of these differs, either the lock was hand-edited or it predates a change
# to the declaration; both mean the lock no longer describes what this Frontier
# says it acquires.
VERBATIM = ("kind", "repo", "ref", "path", "paths", "home", "homepage", "url",
            "pages_commit", "pages_commit_resolved", "acquired_by")

# Fields the resolver reads back from GitHub. The declaration's value is an
# assertion the generating run already checked, so a disagreement now means the
# lock and the declaration were written against different upstream states.
ASSERTED = ("commit", "tree")


def check(root: str | Path, refetch: bool = False, fetch: Fetch | None = None) -> list[str]:
    """Return one line per problem. An empty list means the lock is good."""
    root = Path(root)
    problems: list[str] = []

    if not (root / DECLARATION_FILE).is_file():
        return [f"{root / DECLARATION_FILE} does not exist; this is not a Frontier root"]
    if not (root / LOCK_FILE).is_file():
        return [f"{root / LOCK_FILE} does not exist; run vela-source-lock to write it"]

    document, declaration_problems = read_declaration(root)
    problems.extend(declaration_problems)
    lock, lock_problems = read_lock(root)
    problems.extend(lock_problems)

    declared = document.get("sources") or {}
    locked = lock.get("sources") or {}

    for name in sorted(set(declared) - set(locked)):
        problems.append(f"{name}: declared in {DECLARATION_FILE} but absent from the lock")
    for name in sorted(set(locked) - set(declared)):
        problems.append(f"{name}: locked but no longer declared in {DECLARATION_FILE}")

    for name in sorted(set(declared) & set(locked)):
        problems.extend(_check_entry(root, name, declared[name], locked[name]))

    if refetch:
        problems.extend(_check_against_upstream(root, locked, fetch))
    return problems


def _check_entry(
    root: Path, name: str, spec: dict[str, Any], entry: dict[str, Any]
) -> list[str]:
    problems: list[str] = []

    for field in VERBATIM:
        if spec.get(field) is not None and entry.get(field) != spec[field]:
            problems.append(
                f"{name}: {DECLARATION_FILE} declares {field}={spec[field]!r}, "
                f"but the lock records {entry.get(field)!r}"
            )
    for field in ASSERTED:
        if spec.get(field) is not None and entry.get(field) not in (None, spec[field]):
            problems.append(
                f"{name}: {DECLARATION_FILE} pins {field} {spec[field]}, "
                f"but the lock records {entry[field]}"
            )

    if "error" in entry:
        problems.append(f"{name}: the lock records a gap — {entry['error']}")

    if spec.get("exact_roots"):
        declared_roots, locked_roots = spec["exact_roots"], entry.get("exact_roots") or {}
        for key in sorted(set(declared_roots) - set(locked_roots)):
            problems.append(f"{name}/{key}: declared as an exact root but absent from the lock")
        for key in sorted(set(locked_roots) - set(declared_roots)):
            problems.append(f"{name}/{key}: locked as an exact root but no longer declared")
        for key in sorted(set(declared_roots) & set(locked_roots)):
            declared_root, locked_root = declared_roots[key], locked_roots[key]
            if declared_root["path"] != locked_root["path"]:
                problems.append(
                    f"{name}/{key}: declared path {declared_root['path']}, "
                    f"locked path {locked_root['path']}"
                )
            if declared_root.get("sha256") and declared_root["sha256"] != locked_root["sha256"]:
                problems.append(
                    f"{name}/{key}: {DECLARATION_FILE} declares {declared_root['sha256']}, "
                    f"but the lock records {locked_root['sha256']}"
                )

    # In-repository bytes are the one content root a check can settle without the
    # network, so it settles it. This is what catches a lock left behind by an
    # edit to a retained artifact.
    path = spec.get("path")
    if path and (root / path).is_file():
        if spec.get("url") is not None:
            # The same ambiguity the resolver refuses to guess at. A check that
            # compared the local file against a root fetched from the url would
            # report a mismatch that is really two different files.
            problems.append(
                f"{name}: ambiguous declaration — {path} exists in this repository and "
                f"{spec['url']} is declared as well, so which of the two the lock's "
                "content root describes is not stated"
            )
        elif "sha256" in entry:
            observed = sha256((root / path).read_bytes())
            if observed != entry["sha256"]:
                problems.append(
                    f"{name}: the lock records {entry['sha256']} for {path}, "
                    f"but the file in this repository hashes to {observed}"
                )
    return problems


def _check_against_upstream(root: Path, locked: dict[str, Any], fetch: Fetch | None) -> list[str]:
    """Re-resolve and compare content roots. Opt-in, because upstream having
    moved is a fact about upstream, not a defect in a lock that recorded a
    moment. Only the caller knows which of the two they are asking about.
    """
    fresh = resolve(root, fetch)
    problems = list(fresh.problems)
    for name, entry in sorted(fresh.payload["sources"].items()):
        previous = locked.get(name)
        if previous is None:
            continue
        if entry.get("sha256") and previous.get("sha256") != entry["sha256"]:
            problems.append(
                f"{name}: the lock records {previous.get('sha256')}, "
                f"but the source now serves {entry['sha256']}"
            )
        for key, root_entry in sorted((entry.get("exact_roots") or {}).items()):
            was = (previous.get("exact_roots") or {}).get(key, {}).get("sha256")
            if was != root_entry["sha256"]:
                problems.append(
                    f"{name}/{key}: the lock records {was}, "
                    f"but the source now serves {root_entry['sha256']}"
                )
    return problems
