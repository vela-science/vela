"""Access to the two normative schemas, and validation against them.

The schemas ship as package data rather than as Python literals so a consumer in
another language can read the same file instead of restating the shape. A
restated schema is a second opinion, and a second opinion drifts: the reason
this package exists at all is that one 276-line resolver had been copied into
three repositories and a fourth had an older one of its own.
"""

from __future__ import annotations

import json
from importlib import resources
from typing import Any

import jsonschema

DECLARATION_SCHEMA = "sources.schema.json"
LOCK_SCHEMA = "sources-lock.schema.json"
SCHEMA_NAMES = (DECLARATION_SCHEMA, LOCK_SCHEMA)


def schema_path(name: str):
    """The on-disk path of a bundled schema, for a consumer that wants the file."""
    if name not in SCHEMA_NAMES:
        raise KeyError(f"unknown schema {name!r}; this package ships {', '.join(SCHEMA_NAMES)}")
    return resources.files(__package__).joinpath("schemas", name)


def schema_text(name: str) -> str:
    return schema_path(name).read_text(encoding="utf-8")


def load_schema(name: str) -> dict[str, Any]:
    return json.loads(schema_text(name))


def _validator(name: str) -> jsonschema.protocols.Validator:
    schema = load_schema(name)
    cls = jsonschema.validators.validator_for(schema)
    cls.check_schema(schema)
    return cls(schema)


def validate(document: Any, name: str, origin: str) -> list[str]:
    """Validate `document`, returning one line per violation.

    Violations are returned rather than raised because a lock run reports every
    problem it found in one go. Being told about the first of four bad
    declarations, four times running, is how a fix loop turns into an afternoon.
    """
    problems = []
    for error in sorted(_validator(name).iter_errors(document), key=lambda e: list(e.absolute_path)):
        where = "/".join(str(part) for part in error.absolute_path)
        problems.append(f"{origin}{'/' + where if where else ''}: {error.message}")
    return problems
