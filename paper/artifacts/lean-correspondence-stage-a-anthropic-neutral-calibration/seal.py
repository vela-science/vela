#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path

sys.dont_write_bytecode = True

PACKAGE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "anthropic_terminal_verify", PACKAGE / "verify.py"
)
if SPEC is None or SPEC.loader is None:
    raise SystemExit("FAIL: verifier import unavailable")
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)

raw = (
    json.dumps(VERIFY.seal_manifest(PACKAGE), indent=2, sort_keys=True) + "\n"
).encode()
descriptor = os.open(PACKAGE / "artifact-root.json", os.O_WRONLY | os.O_TRUNC)
try:
    os.write(descriptor, raw)
    os.fsync(descriptor)
finally:
    os.close(descriptor)
