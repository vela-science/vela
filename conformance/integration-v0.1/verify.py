#!/usr/bin/env python3
import subprocess
from pathlib import Path

raise SystemExit(
    subprocess.call(
        [
            "cargo",
            "test",
            "--quiet",
            "--locked",
            "-p",
            "vela-cli",
            "integration::tests::fixture_packet_and_hostiles",
        ],
        cwd=Path(__file__).resolve().parents[2],
    )
)
