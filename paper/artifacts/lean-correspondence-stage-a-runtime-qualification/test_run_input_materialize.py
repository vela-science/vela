from __future__ import annotations

import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("run_input_materialize.py")
SPEC = importlib.util.spec_from_file_location("run_input_materialize", MODULE_PATH)
assert SPEC and SPEC.loader
MATERIALIZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MATERIALIZER)


class MaterializerTests(unittest.TestCase):
    def fields(self) -> dict[str, object]:
        return {
            "run_id": "neutral-test",
            "model": "held-model",
            "prompt": "neutral\n",
            "packet_path": "/input/packet.json",
            "packet_bytes": 3,
            "packet_sha256": "sha256:" + "0" * 64,
            "output_dir": "/evidence",
        }

    def test_exact_raw_splice_preserves_whitespace_and_key_order(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as temporary:
            root = Path(temporary)
            schema = root / "schema.json"
            raw = b'{"z": 1, "a": {"y": 2, "x": 1}}\n'
            schema.write_bytes(raw)
            run, receipt = MATERIALIZER.materialize(
                schema, root / "run.json", root / "receipt.json", self.fields()
            )
            value = json.loads(receipt)
            self.assertEqual(
                run[value["raw_inserted_start"] : value["raw_inserted_end"]], raw
            )
            self.assertEqual(run.count(raw), 1)
            self.assertFalse(value["parse_reserialization_used"])

    def test_rejects_symlink_and_hardlink(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as temporary:
            root = Path(temporary)
            schema = root / "schema.json"
            schema.write_bytes(b"{}\n")
            link = root / "link.json"
            link.symlink_to(schema)
            with self.assertRaisesRegex(ValueError, "symlink"):
                MATERIALIZER.materialize(
                    link, root / "run.json", root / "receipt.json", self.fields()
                )
            hard = root / "hard.json"
            os.link(schema, hard)
            with self.assertRaisesRegex(ValueError, "single_link"):
                MATERIALIZER.materialize(
                    schema, root / "run.json", root / "receipt.json", self.fields()
                )

    def test_rejects_inode_race(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as temporary:
            root = Path(temporary)
            schema = root / "schema.json"
            replacement = root / "replacement.json"
            schema.write_bytes(b"{}\n")
            replacement.write_bytes(b'{"type":"object"}\n')

            def swap() -> None:
                os.replace(replacement, schema)

            with self.assertRaisesRegex(ValueError, "post_read_inode_drift"):
                MATERIALIZER.materialize(
                    schema,
                    root / "run.json",
                    root / "receipt.json",
                    self.fields(),
                    after_open=swap,
                )


if __name__ == "__main__":
    unittest.main()
