from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

PACKAGE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "anthropic_terminal_verify", PACKAGE / "verify.py"
)
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def reseal(package: Path) -> None:
    files = []
    for path in sorted(package.rglob("*")):
        if path.is_file() and path.name != "artifact-root.json":
            raw = path.read_bytes()
            files.append(
                {
                    "path": path.relative_to(package).as_posix(),
                    "bytes": len(raw),
                    "sha256": digest(raw),
                }
            )
    value = {
        "schema": "vela.stage-a-anthropic-neutral-terminal-artifact.v1",
        "files": files,
        "artifact_root": digest(canonical(files)),
    }
    (package / "artifact-root.json").write_bytes(
        (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    )


class TerminalEvidenceTests(unittest.TestCase):
    def mutate(self, relative: str, edit) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "artifact"
            shutil.copytree(PACKAGE, candidate)
            path = candidate / relative
            value = json.loads(path.read_bytes())
            edit(value)
            path.write_bytes(
                (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
            )
            reseal(candidate)
            with (
                mock.patch.object(VERIFY, "PACKAGE", candidate),
                self.assertRaises(VERIFY.VerificationError),
            ):
                VERIFY.verify()

    def test_valid_terminal_evidence(self) -> None:
        self.assertEqual(VERIFY.verify()["provider_calls"], 0)

    def test_endpoint_call_inflation_fails_after_reseal(self) -> None:
        self.mutate(
            "endpoint-contact-receipt.json",
            lambda value: value.__setitem__("provider_calls", 1),
        )

    def test_raw_controller_cannot_become_authoritative(self) -> None:
        self.mutate(
            "terminal-outcome.json",
            lambda value: value["custody"].__setitem__(
                "controller_raw_terminal_authoritative", True
            ),
        )

    def test_retry_inflation_fails_after_reseal(self) -> None:
        self.mutate(
            "terminal-outcome.json",
            lambda value: value["attempt"].__setitem__("retries", 1),
        )

    def test_endpoint_contact_inflation_fails_after_reseal(self) -> None:
        self.mutate(
            "endpoint-contact-receipt.json",
            lambda value: value.__setitem__("endpoint_contacted", True),
        )


if __name__ == "__main__":
    unittest.main()
