from __future__ import annotations

import hashlib
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.evidence_qualification.secure_reader import (
    read_absolute_regular,
    read_regular,
)


class SecureReaderTests(unittest.TestCase):
    def make_root(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory(dir=Path.cwd())
        return temporary, Path(temporary.name)

    def test_valid_multi_root_and_exact_receipt(self) -> None:
        first_tmp, first = self.make_root()
        second_tmp, second = self.make_root()
        self.addCleanup(first_tmp.cleanup)
        self.addCleanup(second_tmp.cleanup)
        target = second / "nested" / "evidence.json"
        target.parent.mkdir()
        raw = b'{"closed":true}\n'
        target.write_bytes(raw)
        observed = read_absolute_regular(
            target,
            "evidence",
            trusted_roots=(first, second),
            expected_bytes=len(raw),
            expected_sha256=hashlib.sha256(raw).hexdigest(),
        )
        self.assertEqual(observed, raw)

    def test_parent_path_replacement_during_validation_fails(self) -> None:
        temporary, root = self.make_root()
        self.addCleanup(temporary.cleanup)
        parent = root / "registered"
        child = parent / "nested"
        child.mkdir(parents=True)
        (child / "capture.json").write_bytes(b"{}\n")

        def replace_parent(_raw: bytes) -> None:
            parent.rename(root / "original")
            replacement = root / "registered" / "nested"
            replacement.mkdir(parents=True)
            (replacement / "capture.json").write_bytes(b"{}\n")

        with self.assertRaisesRegex(ValueError, "parent_custody_drift"):
            read_regular(
                root,
                "registered/nested/capture.json",
                "capture",
                validator=replace_parent,
            )

    def test_open_interposition_parent_identity_change_fails(self) -> None:
        temporary, root = self.make_root()
        self.addCleanup(temporary.cleanup)
        parent = root / "registered"
        parent.mkdir()
        (parent / "capture.json").write_bytes(b"{}\n")
        original_open = os.open
        replaced = False

        def interposed_open(
            path: object, flags: int, *args: object, **kwargs: object
        ) -> int:
            nonlocal replaced
            if (
                path == "registered"
                and kwargs.get("dir_fd") is not None
                and not replaced
            ):
                replaced = True
                parent.rename(root / "original")
                parent.mkdir()
                (parent / "capture.json").write_bytes(b"{}\n")
            return original_open(path, flags, *args, **kwargs)

        with (
            mock.patch(
                "tools.evidence_qualification.secure_reader.os.open", interposed_open
            ),
            self.assertRaisesRegex(ValueError, "directory_custody_drift"),
        ):
            read_regular(root, "registered/capture.json", "capture")

    def test_external_hardlink_fails(self) -> None:
        temporary, root = self.make_root()
        self.addCleanup(temporary.cleanup)
        target = root / "capture.json"
        target.write_bytes(b"{}\n")
        os.link(target, root / "external-alias.json")
        with self.assertRaisesRegex(ValueError, "not_regular_single_link"):
            read_regular(root, "capture.json", "capture")

    def test_duplicate_final_identity_registry_fails(self) -> None:
        temporary, root = self.make_root()
        self.addCleanup(temporary.cleanup)
        target = root / "capture.json"
        target.write_bytes(b"{}\n")
        identities: set[tuple[int, int]] = set()
        self.assertEqual(
            read_regular(root, "capture.json", "capture", identity_registry=identities),
            b"{}\n",
        )
        with self.assertRaisesRegex(ValueError, "duplicate_path_identity"):
            read_regular(root, "capture.json", "capture", identity_registry=identities)


if __name__ == "__main__":
    unittest.main()
