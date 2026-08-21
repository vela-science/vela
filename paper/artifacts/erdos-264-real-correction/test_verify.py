from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MODULE_PATH = ROOT / "verify.py"
SPEC = importlib.util.spec_from_file_location(
    "erdos_264_real_correction_verify", MODULE_PATH
)
assert SPEC is not None and SPEC.loader is not None
VERIFIER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFIER)


def run(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
    ).stdout


class VerifierUnitTests(unittest.TestCase):
    def test_definition_bytes_include_one_terminal_newline(self) -> None:
        source = (
            b"namespace Erdos264\n"
            b"def IsIrrationalitySequence (a : Nat -> Nat) : Prop := True\n\n"
            b"/-- consumer -/\n"
            b"theorem erdos_264.parts.i : IsIrrationalitySequence id := by sorry\n"
        )
        self.assertEqual(
            VERIFIER.definition_bytes(source),
            b"def IsIrrationalitySequence (a : Nat -> Nat) : Prop := True\n",
        )

    def test_direct_consumers_are_closed_to_signature_references(self) -> None:
        source = (
            b"namespace Erdos264\n"
            b"def IsIrrationalitySequence (a : Nat -> Nat) : Prop := True\n\n"
            b"theorem erdos_264.parts.i : IsIrrationalitySequence id := by sorry\n\n"
            b"theorem helper : True := by\n"
            b'  have note := "IsIrrationalitySequence"\n'
            b"  trivial\n"
        )
        self.assertEqual(
            VERIFIER.direct_consumers(source),
            ["Erdos264.erdos_264.parts.i"],
        )

    def test_canonical_diff_ignores_abbreviation_configuration(self) -> None:
        with tempfile.TemporaryDirectory(prefix="vela-erdos264-diff-") as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            run(source, "init", "-q")
            run(source, "config", "user.name", "Vela Test")
            run(source, "config", "user.email", "vela-test@example.invalid")
            tracked = source / "264.lean"
            tracked.write_text("def value : Nat := 1\n", encoding="utf-8")
            run(source, "add", "264.lean")
            run(source, "commit", "-q", "-m", "predecessor")
            predecessor = run(source, "rev-parse", "HEAD").decode().strip()
            tracked.write_text("def value : Nat := 2\n", encoding="utf-8")
            run(source, "commit", "-qam", "successor")
            successor = run(source, "rev-parse", "HEAD").decode().strip()

            short = root / "short"
            long = root / "long"
            run(root, "clone", "-q", str(source), str(short))
            run(root, "clone", "-q", str(source), str(long))
            run(short, "config", "core.abbrev", "7")
            run(long, "config", "core.abbrev", "16")
            short_diff = VERIFIER.canonical_diff(
                short, predecessor, successor, "264.lean"
            )
            long_diff = VERIFIER.canonical_diff(
                long, predecessor, successor, "264.lean"
            )
            self.assertEqual(short_diff, long_diff)
            index_line = next(
                line for line in short_diff.splitlines() if line.startswith(b"index ")
            )
            old_blob, new_blob = index_line.split()[1].split(b"..")
            self.assertEqual((len(old_blob), len(new_blob)), (40, 40))

    def test_case_root_is_immutable(self) -> None:
        case = (ROOT / "case.json").read_bytes()
        self.assertEqual(VERIFIER.sha256(case), VERIFIER.CASE_ROOT)
        changed = json.loads(case)
        changed["authority"] = "authoritative"
        changed_bytes = json.dumps(changed, sort_keys=True).encode()
        self.assertNotEqual(VERIFIER.sha256(changed_bytes), VERIFIER.CASE_ROOT)

    def test_manifest_binds_every_tracked_artifact_file(self) -> None:
        manifest_path = ROOT / "manifest.json"
        manifest = json.loads(manifest_path.read_bytes())
        files = manifest["files"]
        expected = {"README.md", "case.json", "test_verify.py", "verify.py"}
        self.assertEqual(set(files), expected)
        for relative, metadata in files.items():
            encoded = (ROOT / relative).read_bytes()
            self.assertEqual(metadata["bytes"], len(encoded))
            self.assertEqual(
                metadata["sha256"], f"sha256:{hashlib.sha256(encoded).hexdigest()}"
            )
        canonical = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
        self.assertEqual(manifest["artifact_root"], VERIFIER.sha256(canonical))


if __name__ == "__main__":
    unittest.main()
