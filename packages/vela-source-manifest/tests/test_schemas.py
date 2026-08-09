"""The two schemas are the definition, so they get held to it directly."""

from __future__ import annotations

import json
import os
from pathlib import Path

import jsonschema
import pytest

from vela_source_manifest import (
    DECLARATION_SCHEMA,
    LOCK_SCHEMA,
    SCHEMA_NAMES,
    load_schema,
    read_declaration,
    read_lock,
    schema_text,
)
from vela_source_manifest.cli import main
from vela_source_manifest.schema import validate

COMMIT = "a" * 40
GOOD_ROOT = "sha256:" + "0" * 64


@pytest.mark.parametrize(
    ("name", "per_source"),
    [(DECLARATION_SCHEMA, "declaration"), (LOCK_SCHEMA, "entry")],
)
def test_each_schema_is_valid_2020_12_and_closed(name, per_source):
    schema = load_schema(name)
    jsonschema.validators.Draft202012Validator.check_schema(schema)
    assert schema["$schema"] == "https://json-schema.org/draft/2020-12/schema"
    assert schema["additionalProperties"] is False
    assert schema["$defs"][per_source]["additionalProperties"] is False


@pytest.mark.parametrize("name", SCHEMA_NAMES)
def test_a_consumer_in_another_language_can_read_the_schema_off_the_cli(name, capsys):
    assert main(["--print-schema", name]) == 0
    assert json.loads(capsys.readouterr().out) == json.loads(schema_text(name))


def test_an_unknown_declaration_key_is_rejected():
    """Closed on purpose. A misspelled `pages_commmit` that a loose schema waves
    through becomes a pin nobody is checking.
    """
    document = {
        "sources": {
            "x": {"source_id": "source:x", "kind": "k", "pages_commmit": COMMIT}
        }
    }
    assert validate(document, DECLARATION_SCHEMA, "sources.yaml")


def test_an_abbreviated_commit_is_rejected():
    document = {"sources": {"x": {"source_id": "source:x", "kind": "k", "commit": "a" * 7}}}
    assert validate(document, DECLARATION_SCHEMA, "sources.yaml")


def test_exact_roots_without_a_commit_are_rejected():
    document = {
        "sources": {
            "x": {
                "source_id": "source:x",
                "kind": "formal_library",
                "repo": "o/r",
                "exact_roots": {"license": {"path": "LICENSE"}},
            }
        }
    }
    assert validate(document, DECLARATION_SCHEMA, "sources.yaml")


def test_a_reference_only_source_may_not_claim_retained_bytes():
    document = {
        "sources": {
            "x": {
                "source_id": "source:x",
                "kind": "reference_only",
                "url": "https://codetables.de/",
                "path": "sources/codetables.json",
            }
        }
    }
    assert validate(document, DECLARATION_SCHEMA, "sources.yaml")


@pytest.mark.parametrize(
    "entry",
    [
        pytest.param({"kind": "k"}, id="no content state at all"),
        pytest.param({"kind": "k", "sha256": GOOD_ROOT, "unlocked": "why"}, id="two states"),
        pytest.param(
            {"kind": "k", "unlocked": "why", "error": "boom"}, id="unlocked and error"
        ),
    ],
)
def test_the_reader_invariant_is_enforced_by_the_lock_schema(entry):
    """Exactly one of sha256, exact_roots, unlocked, error. An entry nobody could
    pin must never read as one nobody bothered to pin, and an entry with both a
    root and a reason is a resolver bug that must not reach a reader.
    """
    lock = {"sources": {"x": entry}}
    assert validate(lock, LOCK_SCHEMA, "sources.lock.json")


def test_a_hand_written_root_of_the_wrong_shape_is_rejected():
    """The abbreviation a person types when they edit a lock instead of running
    the generator. Held here because this schema is the only place that decides
    what a content root looks like: the repository linter used to validate locks
    against this same file a second time, and no longer does.
    """
    lock = {
        "sources": {"x": {"kind": "k", "sha256": "32c4f405"}},
    }
    assert validate(lock, LOCK_SCHEMA, "sources.lock.json") == [
        "sources.lock.json/sources/x/sha256: "
        "'32c4f405' does not match '^sha256:[0-9a-f]{64}$'"
    ]


def test_a_well_formed_lock_entry_passes():
    lock = {
        "sources": {"x": {"kind": "k", "sha256": GOOD_ROOT}},
    }
    assert validate(lock, LOCK_SCHEMA, "sources.lock.json") == []


# Optional, and pointed at the real thing: set VELA_REPOSITORY_ROOTS to a
# colon-separated list of Repository checkouts to hold the schemas to every
# declaration and lock actually in use. Off by default so the suite stays
# hermetic, on in an environment that has the Repositories checked out.
#
# This gates a skip, not a failure. An environment still exporting the retired
# VELA_FRONTIER_ROOTS gets a silent skip rather than an error, so the rename has
# to reach the caller as well as this file.
ROOTS = [Path(p) for p in os.environ.get("VELA_REPOSITORY_ROOTS", "").split(":") if p]


@pytest.mark.skipif(not ROOTS, reason="set VELA_REPOSITORY_ROOTS to check real Repositories")
@pytest.mark.parametrize("root", ROOTS, ids=lambda p: p.name)
def test_the_real_repositories_match_the_schemas(root):
    # Read through the package's own loader: `yaml.safe_load` turns an unquoted
    # `2026-08-05T21:22:46Z` into a datetime, which is not the string the
    # Repository wrote and not what the schema describes.
    assert read_declaration(root)[1] == []
    assert read_lock(root)[1] == []
