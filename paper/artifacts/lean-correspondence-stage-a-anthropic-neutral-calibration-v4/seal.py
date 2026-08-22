#!/usr/bin/env python3
from __future__ import annotations

import json
import sys

sys.dont_write_bytecode = True

from verify import PACKAGE, seal_manifest


def main() -> None:
    target = PACKAGE / "artifact-root.json"
    target.write_text(
        json.dumps(seal_manifest(PACKAGE), indent=2, sort_keys=True) + "\n"
    )


if __name__ == "__main__":
    main()
