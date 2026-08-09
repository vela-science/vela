"""What `--check` catches, and what it deliberately does not."""

from __future__ import annotations

import json

import yaml

from vela_source_manifest import check, write_sources_lock
from vela_source_manifest.cli import main

from conftest import FakeGitHub, root_of

COMMIT = "a" * 40
TREE = "b" * 40
WITNESS = b'{"witness": true}\n'


def build(repository):
    return repository(
        {
            "quantum_retained_certificate": {
                "source_id": "source:quantum-retained-certificate",
                "kind": "frontier_local_artifact",
                "path": "artifacts/quantum-10-1-4.witness.json",
            },
            "codetables": {
                "source_id": "source:codetables-stabilizer",
                "kind": "reference_only",
                "url": "https://codetables.de/",
            },
        },
        {"artifacts/quantum-10-1-4.witness.json": WITNESS},
    )


def test_a_freshly_written_lock_checks_out_offline(repository, offline):
    root = build(repository)
    assert write_sources_lock(root).ok
    assert check(root) == []
    assert main([str(root), "--check"]) == 0


def test_a_missing_lock_is_reported_rather_than_written(repository, offline):
    root = build(repository)
    assert main([str(root), "--check"]) == 1
    assert not (root / "sources.lock.json").exists()


def test_an_edited_in_repository_artifact_leaves_its_lock_behind(repository, offline):
    """The one content root a check can settle without the network, settled."""
    root = build(repository)
    write_sources_lock(root)
    (root / "artifacts" / "quantum-10-1-4.witness.json").write_bytes(b'{"witness": false}\n')

    problems = check(root)

    assert any(root_of(WITNESS) in p and "hashes to" in p for p in problems)


def test_a_source_added_to_the_declaration_is_not_silently_uncovered(repository, offline):
    root = build(repository)
    write_sources_lock(root)
    declaration = yaml.safe_load((root / "sources.yaml").read_text())
    declaration["sources"]["oeis"] = {
        "source_id": "source:oeis-a309370",
        "kind": "sequence_database",
        "url": "https://oeis.org/A309370?fmt=json",
    }
    (root / "sources.yaml").write_text(yaml.safe_dump(declaration))

    assert any("oeis: declared in sources.yaml but absent from the lock" in p
               for p in check(root))


def test_a_hand_edited_locator_is_caught(repository, offline):
    root = build(repository)
    write_sources_lock(root)
    lock = json.loads((root / "sources.lock.json").read_text())
    lock["sources"]["codetables"]["url"] = "https://example.invalid/"
    (root / "sources.lock.json").write_text(json.dumps(lock, indent=2, sort_keys=True) + "\n")

    assert any("codetables: sources.yaml declares url=" in p for p in check(root))


def test_a_lock_carrying_an_error_never_checks_out(repository):
    root = repository(
        {
            "jayyhk": {
                "source_id": "source:jayyhk-erdos-lean",
                "kind": "proof_manifest",
                "url": "https://raw.githubusercontent.com/Jayyhk/erdos-lean/main/x.yaml",
            }
        }
    )
    write_sources_lock(root, FakeGitHub())

    assert any("the lock records a gap" in p for p in check(root))


def test_the_default_check_does_not_reach_upstream(repository, offline):
    """A lock records what was acquired at a moment. Some are stale on purpose,
    so asking upstream is opt-in; the `offline` fixture proves the default does
    not ask. Erdős's live-fetched pins are the reason this matters.
    """
    root = repository(
        {
            "erdos": {
                "source_id": "source:erdos-problems",
                "kind": "problem_registry",
                "repo": "teorth/erdosproblems",
                "ref": "main",
                "url": "https://raw.githubusercontent.com/teorth/erdosproblems/main/p.yaml",
            }
        }
    )
    was = b"problems: [1]\n"
    github = FakeGitHub(
        blobs={"https://raw.githubusercontent.com/teorth/erdosproblems/main/p.yaml": was},
        commits={("teorth/erdosproblems", "main"): (COMMIT, TREE)},
    )
    write_sources_lock(root, github)

    assert check(root) == []

    now = b"problems: [1, 2]\n"
    github.blobs["https://raw.githubusercontent.com/teorth/erdosproblems/main/p.yaml"] = now
    problems = check(root, refetch=True, fetch=github)

    assert any(f"now serves {root_of(now)}" in p for p in problems)
