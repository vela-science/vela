"""Producer-side source declarations and locks for Vela Repositories.

One resolver, one pair of schemas. See the package README for
why this lives beside `crates/` rather than in it.
"""

from __future__ import annotations

from .resolver import (
    DECLARATION_FILE,
    LOCK_FILE,
    Resolution,
    read_declaration,
    read_lock,
    resolve,
    write_sources_lock,
)
from .schema import (
    DECLARATION_SCHEMA,
    LOCK_SCHEMA,
    SCHEMA_NAMES,
    load_schema,
    schema_path,
    schema_text,
)
from .verify import check

__version__ = "1.0.0"

__all__ = [
    "DECLARATION_FILE",
    "DECLARATION_SCHEMA",
    "LOCK_FILE",
    "LOCK_SCHEMA",
    "Resolution",
    "SCHEMA_NAMES",
    "__version__",
    "check",
    "load_schema",
    "read_declaration",
    "read_lock",
    "resolve",
    "schema_path",
    "schema_text",
    "write_sources_lock",
]
