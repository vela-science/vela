from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import sys
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
    value = VERIFY.seal_manifest(package)
    (package / "artifact-root.json").write_bytes(
        (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    )


class TerminalEvidenceTests(unittest.TestCase):
    def assert_extra_rejected(self, create) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "artifact"
            shutil.copytree(PACKAGE, candidate)
            create(candidate)
            with self.assertRaises(VERIFY.VerificationError):
                VERIFY.seal_manifest(candidate)
            with (
                mock.patch.object(VERIFY, "PACKAGE", candidate),
                self.assertRaises(VERIFY.VerificationError),
            ):
                VERIFY.verify()

    def mutate_manifest(self, edit) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "artifact"
            shutil.copytree(PACKAGE, candidate)
            path = candidate / "artifact-root.json"
            value = json.loads(path.read_bytes())
            edit(value["files"])
            value["artifact_root"] = digest(canonical(value["files"]))
            path.write_bytes(
                (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
            )
            with (
                mock.patch.object(VERIFY, "PACKAGE", candidate),
                self.assertRaises(VERIFY.VerificationError),
            ):
                VERIFY.verify()

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
        self.assertTrue(sys.dont_write_bytecode)
        before = VERIFY.filesystem_inventory(PACKAGE)
        self.assertEqual(VERIFY.verify()["provider_calls"], 0)
        self.assertEqual(VERIFY.verify()["provider_calls"], 0)
        self.assertEqual(VERIFY.filesystem_inventory(PACKAGE), before)

    def test_undeclared_harmless_file_fails(self) -> None:
        self.assert_extra_rejected(
            lambda candidate: (candidate / "harmless.txt").write_text("harmless\n")
        )

    def test_python_bytecode_cache_fails(self) -> None:
        def create(candidate: Path) -> None:
            cache = candidate / "__pycache__"
            cache.mkdir()
            (cache / "test_verify.cpython-313.pyc").write_bytes(b"not bytecode\n")

        self.assert_extra_rejected(create)

    def test_symlink_extra_fails(self) -> None:
        self.assert_extra_rejected(
            lambda candidate: os.symlink("README.md", candidate / "readme-link")
        )

    def test_hardlink_extra_fails(self) -> None:
        self.assert_extra_rejected(
            lambda candidate: os.link(
                candidate / "README.md", candidate / "readme-hardlink"
            )
        )

    def test_directory_extra_fails(self) -> None:
        self.assert_extra_rejected(lambda candidate: (candidate / "cache").mkdir())

    def test_manifest_omission_fails(self) -> None:
        self.mutate_manifest(lambda files: files.pop())

    def test_manifest_extra_fails(self) -> None:
        self.mutate_manifest(
            lambda files: files.append(
                {
                    "path": "undeclared.txt",
                    "bytes": 0,
                    "sha256": digest(b""),
                }
            )
        )

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
