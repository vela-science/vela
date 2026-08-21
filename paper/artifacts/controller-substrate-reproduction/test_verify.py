from __future__ import annotations

import ast
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify


class ReproductionTests(unittest.TestCase):
    def test_complete_offline_reproduction(self) -> None:
        result = verify.verify_all()
        self.assertEqual(result["status"], "verified")
        self.assertEqual(result["trace"]["accepted_after_verification"], 2771)
        self.assertEqual(result["trace"]["accepted_after_decision"], 2772)
        self.assertEqual(result["trace"]["authority_effect"], "none")

    def test_controller_authority_command_fails_closed(self) -> None:
        source = """\
from pathlib import Path
def x(vela: Path, repo: Path):
    return run_json([str(vela), \"review\", \"accept\", str(repo), \"vpr_x\"])
"""
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "materializer.py"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(verify.ReproductionError, "read-only"):
                verify.verify_controller_boundary(path)

    def test_manifest_hash_drift_fails_closed(self) -> None:
        manifest = verify.load_json(verify.MANIFEST_PATH)
        mutated = json.loads(json.dumps(manifest))
        mutated["files"][0]["sha256"] = "sha256:" + "0" * 64
        with self.assertRaisesRegex(verify.ReproductionError, "hash drift"):
            verify.verify_manifest(mutated)

    def test_agent_semantic_approval_fails_closed(self) -> None:
        manifest = verify.load_json(verify.MANIFEST_PATH)
        decision = manifest["trace"]["authority_records"]["decision"]
        decision_record = {
            "principal": {"principal_class": "agent", "principal_id": "agent:x"},
            "semantic_approvals": [{"action": "review_accept"}],
            "event_ids": decision["event_ids"],
        }
        original = verify.read_authority_record

        def substitute(repository: Path, commit: str, path: str):
            if path == decision["path"]:
                return decision_record, decision["root"]
            return original(repository, commit, path)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            erdos = root / "erdos"
            web = root / "web"
            verify.reconstruct_bundle(
                verify.ARTIFACT_DIR / "bundles/erdos-map-target-loop.bundle",
                manifest["bundles"]["erdos"],
                erdos,
            )
            verify.reconstruct_bundle(
                verify.ARTIFACT_DIR / "bundles/vela-web-map-implementation.bundle",
                manifest["bundles"]["vela_web"],
                web,
            )
            with (
                mock.patch.object(verify, "read_authority_record", substitute),
                self.assertRaisesRegex(verify.ReproductionError, "not human"),
            ):
                verify.verify_trace(manifest, erdos, web)

    def test_materializer_ast_is_parseable(self) -> None:
        source = (
            verify.REPOSITORY_ROOT
            / "paper/artifacts/map-target-loop/materialize_post_decision.py"
        ).read_text(encoding="utf-8")
        self.assertIsInstance(ast.parse(source), ast.Module)


if __name__ == "__main__":
    unittest.main()
