"""What the resolver must do for the four Frontiers that use it."""

from __future__ import annotations

import json

import pytest

from vela_source_manifest import check, resolve, write_sources_lock
from vela_source_manifest.cli import main

from conftest import FakeGitHub, lock_of, root_of

COMMIT = "a" * 40
TREE = "b" * 40
MOVED = "c" * 40


def test_url_source_locks_a_root_computed_from_the_bytes(frontier):
    root = frontier(
        {
            "oeis_a309370": {
                "source_id": "source:oeis-a309370",
                "kind": "sequence_database",
                "url": "https://oeis.org/A309370?fmt=json",
                "homepage": "https://oeis.org/A309370",
            }
        }
    )
    body = b'{"number": 309370}'
    github = FakeGitHub({"https://oeis.org/A309370?fmt=json": body})

    entry = resolve(root, github).payload["sources"]["oeis_a309370"]

    assert entry["sha256"] == root_of(body)
    # OEIS offers no immutable locator, so the content root is the only pin
    # available: a sha256 and no commit, because there is no commit to lock.
    assert "commit" not in entry
    assert entry["homepage"] == "https://oeis.org/A309370"


def test_ref_backed_url_records_the_commit_the_ref_pointed_at(frontier):
    root = frontier(
        {
            "erdos": {
                "source_id": "source:erdos-problems",
                "kind": "problem_registry",
                "repo": "teorth/erdosproblems",
                "ref": "main",
                "path": "data/problems.yaml",
                "url": "https://raw.githubusercontent.com/teorth/erdosproblems/main/data/problems.yaml",
            }
        }
    )
    body = b"problems: []\n"
    github = FakeGitHub(
        blobs={
            "https://raw.githubusercontent.com/teorth/erdosproblems/main/data/problems.yaml": body
        },
        commits={("teorth/erdosproblems", "main"): (COMMIT, TREE)},
    )

    entry = resolve(root, github).payload["sources"]["erdos"]

    assert entry["sha256"] == root_of(body)
    assert (entry["commit"], entry["tree"]) == (COMMIT, TREE)
    # The declared path is an upstream path, not one in this repository. It is
    # carried through so the lock still says which file of that repo this is.
    assert entry["path"] == "data/problems.yaml"


def test_a_failed_ref_resolution_does_not_discard_a_computed_root(frontier):
    """The content root is the durable pin; the commit only says which revision
    the ref pointed at. Losing a hash computed from bytes we hold to an unrelated
    API failure would be the worse outcome, so the run fails and keeps the root.
    """
    root = frontier(
        {
            "plby": {
                "source_id": "source:plby-lean-proofs",
                "kind": "proof_manifest",
                "repo": "plby/lean-proofs",
                "ref": "main",
                "url": "https://raw.githubusercontent.com/plby/lean-proofs/main/data/sources.yaml",
            }
        }
    )
    body = b"sources: []\n"
    github = FakeGitHub(
        blobs={"https://raw.githubusercontent.com/plby/lean-proofs/main/data/sources.yaml": body}
    )  # no commits: the API is unreachable

    resolution = resolve(root, github)
    entry = resolution.payload["sources"]["plby"]

    assert entry["sha256"] == root_of(body)
    assert "commit" not in entry
    assert any("could not resolve plby/lean-proofs@main" in p for p in resolution.problems)


def test_in_repository_bytes_are_hashed_from_disk(frontier, offline):
    body = b'{"witness": true}\n'
    root = frontier(
        {
            "quantum_retained_certificate": {
                "source_id": "source:quantum-retained-certificate",
                "kind": "frontier_local_artifact",
                "path": "artifacts/quantum-10-1-4.witness.json",
            }
        },
        {"artifacts/quantum-10-1-4.witness.json": body},
    )

    resolution = resolve(root)

    assert resolution.ok
    assert resolution.payload["sources"]["quantum_retained_certificate"]["sha256"] == root_of(body)


def test_a_declared_path_that_is_not_there_is_a_recorded_gap(frontier, offline):
    root = frontier(
        {
            "fidelity": {
                "source_id": None,
                "kind": "frozen_snapshot",
                "path": "sources/fidelity_cache.json",
            }
        }
    )

    resolution = resolve(root)

    entry = resolution.payload["sources"]["fidelity"]
    assert entry["error"] == "declared path sources/fidelity_cache.json does not exist in this repository"
    assert resolution.problems


def _upstream_path_and_url() -> dict:
    # The shape four Erdős entries have: `path` is where the file lives in the
    # *upstream* repository, and the url is the locator. Every other Frontier
    # uses `path` for bytes retained locally.
    return {
        "erdos": {
            "source_id": "source:erdos-problems",
            "kind": "problem_registry",
            "repo": "teorth/erdosproblems",
            "ref": "main",
            "path": "data/problems.yaml",
            "url": "https://raw.githubusercontent.com/teorth/erdosproblems/main/data/problems.yaml",
        }
    }


def test_an_upstream_path_alongside_a_url_locks_the_url(frontier):
    body = b"problems: []\n"
    root = frontier(_upstream_path_and_url())
    github = FakeGitHub(
        blobs={
            "https://raw.githubusercontent.com/teorth/erdosproblems/main/data/problems.yaml": body
        },
        commits={("teorth/erdosproblems", "main"): ("a" * 40, "b" * 40)},
    )

    resolution = resolve(root, github)

    entry = resolution.payload["sources"]["erdos"]
    assert resolution.ok
    assert entry["sha256"] == root_of(body)
    assert entry["path"] == "data/problems.yaml"  # kept as provenance, not used as a locator


def test_a_local_file_at_a_declared_upstream_path_is_refused_not_guessed(frontier, offline):
    # If this Frontier ever vendored a file at the upstream path, hashing it
    # would switch the pin from upstream bytes to local bytes under an entry
    # that still names the url, and the lock would look exactly as it does now.
    root = frontier(_upstream_path_and_url(), {"data/problems.yaml": b"a vendored copy\n"})

    resolution = resolve(root)

    entry = resolution.payload["sources"]["erdos"]
    assert "sha256" not in entry
    assert "ambiguous declaration" in entry["error"]
    assert resolution.problems


def test_the_checker_refuses_the_same_ambiguity(frontier, offline):
    root = frontier(_upstream_path_and_url())
    (root / "sources.lock.json").write_text(
        json.dumps(
            {
                "generated_at": "2026-08-06T00:00:00+00:00",
                "sources": {
                    "erdos": {
                        "kind": "problem_registry",
                        "repo": "teorth/erdosproblems",
                        "ref": "main",
                        "path": "data/problems.yaml",
                        "url": "https://raw.githubusercontent.com/teorth/erdosproblems/main/data/problems.yaml",
                        "sha256": root_of(b"upstream bytes\n"),
                    }
                },
            }
        )
        + "\n"
    )
    assert check(root) == []

    (root / "data").mkdir()
    (root / "data" / "problems.yaml").write_bytes(b"a vendored copy\n")

    problems = check(root)

    assert len(problems) == 1
    assert "ambiguous declaration" in problems[0]


def test_reference_only_is_never_fetched(frontier, offline):
    """Hashing the page would record a content root for something this Frontier
    does not retain, and a false pin is indistinguishable from a real one once it
    is in the lock. The `offline` fixture turns any fetch into a failure.
    """
    root = frontier(
        {
            "codetables": {
                "source_id": "source:codetables-stabilizer",
                "kind": "reference_only",
                "url": "https://codetables.de/",
            }
        }
    )

    resolution = resolve(root)

    entry = resolution.payload["sources"]["codetables"]
    assert resolution.ok
    assert entry["unlocked"].startswith("reference only:")
    assert "sha256" not in entry


def test_cited_not_acquired_is_never_fetched(frontier, offline):
    root = frontier(
        {
            "openai_ten_proofs": {
                "source_id": "source:openai-ten-proofs",
                "kind": "formal_library",
                "repo": "openai/ten-proofs",
                "commit": COMMIT,
                "tree": TREE,
                "url": "https://github.com/openai/ten-proofs",
                "acquired_by": "formal-conjectures",
            }
        }
    )

    entry = resolve(root).payload["sources"]["openai_ten_proofs"]

    assert entry["unlocked"].startswith("cited, not acquired:")
    assert (entry["commit"], entry["tree"]) == (COMMIT, TREE)
    assert entry["acquired_by"] == "formal-conjectures"


def test_a_repository_landing_page_is_pinned_by_commit_not_by_html(frontier):
    root = frontier(
        {
            "openai_ten_proofs": {
                "source_id": "source:openai-ten-proofs",
                "kind": "formal_library",
                "repo": "openai/ten-proofs",
                "commit": COMMIT,
                "tree": TREE,
                "url": "https://github.com/openai/ten-proofs",
            }
        }
    )
    github = FakeGitHub(
        blobs={"https://github.com/openai/ten-proofs": b"<html>a rendered page</html>"},
        commits={("openai/ten-proofs", COMMIT): (COMMIT, TREE)},
    )

    resolution = resolve(root, github)
    entry = resolution.payload["sources"]["openai_ten_proofs"]

    assert resolution.ok
    assert "sha256" not in entry
    assert entry["unlocked"].startswith("no content locator:")
    assert "https://github.com/openai/ten-proofs" not in github.requested


def test_a_moved_commit_fails_the_run(frontier):
    root = frontier(
        {
            "openai_ten_proofs": {
                "source_id": "source:openai-ten-proofs",
                "kind": "formal_library",
                "repo": "openai/ten-proofs",
                "commit": COMMIT,
                "tree": TREE,
                "url": "https://github.com/openai/ten-proofs",
            }
        }
    )
    github = FakeGitHub(commits={("openai/ten-proofs", COMMIT): (MOVED, TREE)})

    resolution = resolve(root, github)

    assert not resolution.ok
    assert any(f"pins commit {COMMIT}, but GitHub resolved it to {MOVED}" in p
               for p in resolution.problems)
    # What GitHub reports is what gets recorded. The declaration is the thing
    # that was wrong, and the lock must not restate it.
    assert resolution.payload["sources"]["openai_ten_proofs"]["commit"] == MOVED


def _physlib(declared_license_root: str) -> dict:
    return {
        "physlib": {
            "source_id": "source:physlib",
            "kind": "formal_library",
            "repo": "leanprover-community/physlib",
            "commit": COMMIT,
            "tree": TREE,
            "url": "https://github.com/leanprover-community/physlib",
            "exact_roots": {
                "license": {"path": "LICENSE", "sha256": declared_license_root},
                "lean_toolchain": {"path": "lean-toolchain"},
            },
        }
    }


def _physlib_github(license_bytes: bytes, toolchain_bytes: bytes) -> FakeGitHub:
    github = FakeGitHub(commits={("leanprover-community/physlib", COMMIT): (COMMIT, TREE)})
    github.raw("leanprover-community/physlib", COMMIT, "LICENSE", license_bytes)
    github.raw("leanprover-community/physlib", COMMIT, "lean-toolchain", toolchain_bytes)
    return github


def test_exact_roots_are_recomputed_and_the_declared_one_is_only_an_assertion(frontier):
    license_bytes, toolchain_bytes = b"Apache 2.0\n", b"leanprover/lean4:v4.30.0\n"
    root = frontier(_physlib(root_of(license_bytes)))

    resolution = resolve(root, _physlib_github(license_bytes, toolchain_bytes))
    roots = resolution.payload["sources"]["physlib"]["exact_roots"]

    assert resolution.ok
    assert roots["license"]["sha256"] == root_of(license_bytes)
    # A file with no declared hash is still locked; the declaration is an
    # optional assertion, not the source of the value.
    assert roots["lean_toolchain"]["sha256"] == root_of(toolchain_bytes)
    assert roots["license"]["url"].endswith(f"/{COMMIT}/LICENSE")


def test_a_tampered_declared_exact_root_writes_the_lock_and_then_fails(frontier, capsys):
    """First of the two negative cases this resolver was built against.

    `sources.yaml` declares a root the pinned commit does not serve. The run must
    record what the commit actually serves, keep the declared value out of the
    lock entirely, write the lock so the disagreement is on the record, and exit
    non-zero.
    """
    license_bytes, toolchain_bytes = b"Apache 2.0\n", b"leanprover/lean4:v4.30.0\n"
    tampered = "sha256:" + "0" * 64
    root = frontier(_physlib(tampered))
    github = _physlib_github(license_bytes, toolchain_bytes)

    resolution = write_sources_lock(root, github)

    assert not resolution.ok
    assert any(f"physlib/license: sources.yaml declares {tampered} for LICENSE" in p
               for p in resolution.problems)
    on_disk = lock_of(root)["sources"]["physlib"]["exact_roots"]["license"]["sha256"]
    assert on_disk == root_of(license_bytes) != tampered

    assert main([str(root), "--check"]) == 1
    assert tampered in capsys.readouterr().err


def test_an_unreachable_url_writes_the_lock_and_then_fails(frontier):
    """Second of the two negative cases.

    A source that should have been lockable and was not leaves an `error` in the
    lock, where the next reader sees it, rather than vanishing from the inventory
    or being retained at whatever it hashed to last time.
    """
    root = frontier(
        {
            "jayyhk": {
                "source_id": "source:jayyhk-erdos-lean",
                "kind": "proof_manifest",
                "repo": "Jayyhk/erdos-lean",
                "ref": "main",
                "url": "https://raw.githubusercontent.com/Jayyhk/erdos-lean/main/data/problems.yaml",
            }
        }
    )

    resolution = write_sources_lock(root, FakeGitHub())

    entry = lock_of(root)["sources"]["jayyhk"]
    assert entry["error"].startswith("URLError:")
    assert "sha256" not in entry
    assert not resolution.ok
    assert any("jayyhk: could not lock" in p for p in resolution.problems)


@pytest.mark.parametrize("case", ["tampered", "unreachable"])
def test_both_negative_cases_exit_one_through_the_cli(frontier, monkeypatch, case):
    if case == "tampered":
        license_bytes = b"Apache 2.0\n"
        root = frontier(_physlib("sha256:" + "0" * 64))
        github = _physlib_github(license_bytes, b"leanprover/lean4:v4.30.0\n")
    else:
        root = frontier(
            {
                "jayyhk": {
                    "source_id": "source:jayyhk-erdos-lean",
                    "kind": "proof_manifest",
                    "url": "https://raw.githubusercontent.com/Jayyhk/erdos-lean/main/x.yaml",
                }
            }
        )
        github = FakeGitHub()
    monkeypatch.setattr("vela_source_manifest.resolver.urlopen_fetch", github)

    assert main([str(root)]) == 1
    assert (root / "sources.lock.json").is_file()


def test_a_pages_backed_source_keeps_its_deployment_provenance(frontier):
    root = frontier(
        {
            "formal_conjectures": {
                "source_id": "source:formal-conjectures",
                "kind": "formal_statement_registry",
                "repo": "google-deepmind/formal-conjectures",
                "pages_commit": COMMIT,
                "pages_commit_resolved": "2026-08-05T21:22:46Z",
                "url": "https://google-deepmind.github.io/formal-conjectures/data/conjectures.json",
            }
        }
    )
    body = b'{"conjectures": []}'
    github = FakeGitHub(
        {"https://google-deepmind.github.io/formal-conjectures/data/conjectures.json": body}
    )

    entry = resolve(root, github).payload["sources"]["formal_conjectures"]

    assert entry["sha256"] == root_of(body)
    # There is no ref the bytes can be re-fetched from, so the entry locks a
    # sha256 and no commit. The deployment provenance is carried alongside it.
    assert "commit" not in entry
    assert entry["pages_commit"] == COMMIT
    assert entry["pages_commit_resolved"] == "2026-08-05T21:22:46Z"


def test_a_declaration_with_no_sources_fails(frontier, offline):
    root = frontier({})
    assert not resolve(root).ok
