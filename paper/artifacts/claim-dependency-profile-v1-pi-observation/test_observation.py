#!/usr/bin/env python3
"""No-auth, no-model falsifiers for the frozen Pi observation packet."""

from __future__ import annotations

import base64
import copy
import hashlib
import io
import json
import os
import shutil
import stat
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

import materialize
import scorer


PACKET = Path(__file__).resolve().parent
REPOSITORY = PACKET.parents[2]
IMAGE_ID = os.environ.get("VELA_PI_TEST_IMAGE_ID")
ROOT = "sha256:"


def digest(data: bytes) -> str:
    return ROOT + hashlib.sha256(data).hexdigest()


def compact(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":")).encode() + b"\n"


def read_json(path: Path) -> dict:
    value = json.loads(path.read_bytes(), object_pairs_hook=scorer.strict_pairs)
    if not isinstance(value, dict):
        raise AssertionError(f"expected JSON object: {path}")
    return value


def tree_bytes(root: Path) -> dict[str, tuple[int, bytes]]:
    result: dict[str, tuple[int, bytes]] = {}
    for path in sorted(root.rglob("*")):
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not (
            stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)
        ):
            raise AssertionError(f"nonregular generated path: {path}")
        if stat.S_ISREG(metadata.st_mode):
            result[path.relative_to(root).as_posix()] = (
                stat.S_IMODE(metadata.st_mode),
                path.read_bytes(),
            )
    return result


class QuietStream:
    def __init__(self) -> None:
        self.buffer = io.BytesIO()

    def write(self, value: str) -> int:
        self.buffer.write(value.encode())
        return len(value)

    def flush(self) -> None:
        return None


def materialize_quiet(arguments: list[str]) -> int:
    with mock.patch.object(materialize.os.sys, "stdout", QuietStream()), mock.patch.object(
        materialize.os.sys, "stderr", QuietStream()
    ):
        return materialize.main(arguments)


def jwt(account: str = "synthetic-capture-account", expiry: int = 4_102_444_800) -> str:
    encode = lambda value: base64.urlsafe_b64encode(compact(value).rstrip(b"\n")).rstrip(b"=").decode()
    return f"{encode({'alg': 'none', 'typ': 'JWT'})}.{encode({'exp': expiry, 'https://api.openai.com/auth': {'chatgpt_account_id': account}})}.synthetic"


class StaticPacketTests(unittest.TestCase):
    def test_frozen_scientific_waist_and_custody_are_exact(self) -> None:
        materialize.validate_frozen_sources()
        for name in materialize.COPIED_V0:
            frozen = subprocess.run(
                ["git", "show", f"{materialize.V0_COMMIT}:{materialize.V0_PATH}/{name}"],
                cwd=REPOSITORY,
                check=True,
                capture_output=True,
            ).stdout
            self.assertEqual((PACKET / name).read_bytes(), frozen, name)
        plan = read_json(PACKET / "plan.json")
        self.assertEqual(
            plan["frozen_v0_packet"]["manifest_raw_root"],
            "sha256:7caedc9c981d7c2012b7de73895479a4a615f231fda08ee43cadb760b6ac04f4",
        )
        custody = plan["executor_reset"]
        self.assertEqual(custody["excluded_custody_commit"], "a3e09e472c7a12654ee613cd93e996dcfe3e6859")
        self.assertFalse(custody["prior_participant_outputs_available_to_v1_participant"])
        self.assertFalse(custody["prior_participant_outputs_imported_into_v1"])
        self.assertTrue(custody["custody_review_observed_output_shape"])

    def test_supply_chain_and_docker_context_are_closed(self) -> None:
        lock_bytes = (PACKET / "package-lock.json").read_bytes()
        lock = json.loads(lock_bytes)
        packages = lock["packages"]
        self.assertEqual(len(packages) - 1, 144)
        self.assertFalse(
            [name for name, row in packages.items() if name and row.get("resolved") and not str(row.get("integrity", "")).startswith("sha512-")]
        )
        pi = packages["node_modules/@earendil-works/pi-coding-agent"]
        self.assertEqual(pi["version"], "0.84.1")
        self.assertEqual(
            pi["integrity"],
            "sha512-ncAqFrG+iybuPGOhMiZoEHkEzTpJgz3guYD32pD+M7ucc0WeHmauP6wa7qwP8V/KWvsZDVNa5XGsdZ7fkC7w7A==",
        )
        nested = {
            "pi-agent-core": "sha512-evyzXYWCLQGmcaBYHlmSku02r8qoN4SGI60GZABo6iV+H+nqX+P9ud8fEZ4GmRq9mUSREvvfX+w9dA9ThF9C6w==",
            "pi-ai": "sha512-wMsAdJMxuNri08vLqTyYVI201DQQezGhPSTkzYsHdw5dYX3rCNwEmSvpaAwhi7ELKI/2tE/CEgSWg/6iRxSgdQ==",
            "pi-client": "sha512-/V5hGHE4Zq+jG0GtwIB9PyBUOGd6gBLZ7lkQYFKchKnxYHeH3rmWC5xw4kpnZKKBuBuFTdLVbU9vEjlAGMMb2A==",
            "pi-protocol": "sha512-Ox1pciyeSPGEEUcxvR0/dJcrY7C6hrEGA8y71rOsvSIUlXN1Cbp/be/eoL71OGDBk5O97TeQPfWN6Ju/2Ehjww==",
            "pi-telemetry": "sha512-180/xGJtsq7IoR3p9EKWjRd0e9M4DkxInhlo9xyD7prDC7Qrhqq+nhvwrW0lFjPfXcEI2FSHmGCSyvSJE9GsaQ==",
            "pi-tui": "sha512-udeXFbgEhJ6JiB0uguwNVNkDy2FENfmtQwPcY+/iJ8GWeq18wkal1tKqa5YyeH0IqtX1vG0cGh8zfSYzyzVuLA==",
        }
        for name, integrity in nested.items():
            path = f"node_modules/@earendil-works/pi-coding-agent/node_modules/@earendil-works/{name}"
            self.assertEqual(packages[path]["integrity"], integrity)
        encoded = b"".join((PACKET / "LICENSE.pi-v0.84.1.base64").read_bytes().split())
        license_bytes = base64.b64decode(encoded, validate=True)
        self.assertEqual(len(license_bytes), 1_069)
        self.assertEqual(digest(license_bytes), "sha256:0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48")
        self.assertFalse(license_bytes.endswith(b"\n"))
        dockerignore = (PACKET / ".dockerignore").read_text().splitlines()
        self.assertEqual(dockerignore[0], "**")
        self.assertNotIn("!answer-key.json", dockerignore)
        self.assertNotIn("!scorer.py", dockerignore)

    def test_sdk_and_runner_contract_spelling_is_exact(self) -> None:
        participant = (PACKET / "participant.mjs").read_text()
        for spelling in (
            "SettingsManager.inMemory({",
            'transport: "sse"',
            "compaction: { enabled: false }",
            "retry: { enabled: false, maxRetries: 0, provider: { maxRetries: 0, maxRetryDelayMs: 0 } }",
            "noExtensions: true",
            "noSkills: true",
            "noPromptTemplates: true",
            "noThemes: true",
            "noContextFiles: true",
            "await resourceLoader.reload()",
            "modelsPath: null",
            "allowModelNetwork: false",
            "refreshOnCreate: false",
            'noTools: "all"',
            "tools: []",
            "session.setAutoRetryEnabled(false)",
            "session.setAutoCompactionEnabled(false)",
            "session.prompt(request.user_message, { expandPromptTemplates: false })",
        ):
            self.assertIn(spelling, participant)
        self.assertNotIn("extensionFactories", participant)
        runner = (PACKET / "run-participant.sh").read_text()
        self.assertIn("--network none", runner)
        self.assertIn("absolute wall deadline", runner)
        self.assertIn("nonsecret failure evidence retained", runner)
        self.assertNotIn('>"$answer"', runner)

    def test_source_manifest_exact_census_refuses_extra_manifest_and_symlink(self) -> None:
        rows, manifest = materialize.packet_manifest()
        actual = {
            path.relative_to(PACKET).as_posix()
            for path in PACKET.rglob("*")
            if path.is_file()
            and path.relative_to(PACKET).as_posix() != "manifest.json"
        }
        self.assertEqual(set(rows), actual)
        self.assertFalse(manifest["claim_credit"])
        with tempfile.TemporaryDirectory() as temporary:
            clone = Path(temporary) / "packet"
            shutil.copytree(PACKET, clone)
            with mock.patch.object(materialize, "PACKET", clone):
                (clone / "nested").mkdir()
                (clone / "nested" / "manifest.json").write_text("{}\n")
                with self.assertRaises(materialize.ContractError):
                    materialize.packet_manifest()
                (clone / "nested" / "manifest.json").unlink()
                (clone / "nested" / "escape").symlink_to(clone / "plan.json")
                with self.assertRaises(materialize.ContractError):
                    materialize.packet_manifest()


class MaterializationAndScorerTests(unittest.TestCase):
    def test_double_materialization_and_unchanged_scorer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            first, second = parent / "first", parent / "second"
            self.assertEqual(materialize_quiet(["--output", str(first), "--development-worktree"]), 0)
            self.assertEqual(materialize_quiet(["--output", str(second), "--development-worktree"]), 0)
            self.assertEqual(tree_bytes(first), tree_bytes(second))
            study = read_json(first / "study-manifest.json")
            self.assertFalse(study["ready_for_participant_runs"])
            self.assertIsNone(study["execution_attestation"])
            self.assertEqual(study["run_order"], [run_id for run_id, _, _ in materialize.RUNS])
            self.assertFalse(any("excluded" in name or "prior" in name for name in tree_bytes(first)))
            baseline = (first / "arms/disciplined-git-ro-crate/user-message.txt").read_bytes()
            treatment = (first / "arms/rooted-source-plus-profile/user-message.txt").read_bytes()
            self.assertEqual(treatment.count(b"virtual_path: profile.json\n"), 1)
            profile = materialize.source_file(read_json(PACKET / "input-manifests/rooted-source-plus-profile.json")["files"][-1])
            block = materialize.render_block("SCIENTIFIC ARM INPUT", "profile.json", profile)
            self.assertEqual(treatment.replace(block, b"", 1), baseline)

            canned = subprocess.run(
                ["git", "show", f"{materialize.V0_COMMIT}:{materialize.V0_PATH}/canned-answer.json"],
                cwd=REPOSITORY,
                check=True,
                capture_output=True,
            ).stdout
            answer = json.loads(canned)
            for arm in materialize.ARMS:
                arm_answer = copy.deepcopy(answer)
                if arm == "rooted-source-plus-profile":
                    arm_answer["classifications"][0]["evidence"][0] = {"path": "profile.json", "pointer": "/dependencies/0"}
                    arm_answer["classifications"][2]["evidence"][0] = {"path": "profile.json", "pointer": "/dependencies/1"}
                answer_path = parent / f"{arm}.answer.json"
                answer_path.write_bytes(materialize.json_bytes(arm_answer))
                observed = scorer.score(
                    answer_path,
                    first / "verifier/answer-key.json",
                    first / f"arms/{arm}/input-manifest.json",
                    first / f"verifier/{arm}/input",
                )
                self.assertTrue(observed["safe_completion"], arm)
                self.assertEqual(observed["affected_set_precision"], 1.0)
                self.assertTrue(all(item["status"] == "not_measured" for item in observed["milestones"]))

    def test_staging_is_removed_on_build_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            output = parent / "failed"
            with mock.patch.object(materialize, "build", side_effect=materialize.ContractError("synthetic refusal")):
                self.assertEqual(materialize_quiet(["--output", str(output), "--development-worktree"]), 1)
            self.assertFalse(output.exists())
            self.assertFalse(list(parent.glob(".failed-*")))


class AuthPreflightTests(unittest.TestCase):
    def run_auth(self, *arguments: str, timeout: float = 4.0) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["node", str(PACKET / "auth-preflight.mjs"), *arguments],
            cwd="/",
            capture_output=True,
            timeout=timeout,
        )

    def test_least_credential_derivation_and_mutation_refusal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            root.chmod(0o700)
            source = root / "codex.json"
            output = root / "pi.json"
            real_refresh = "real-refresh-must-never-cross"
            access = jwt()
            source.write_bytes(compact({"tokens": {"access_token": access, "refresh_token": real_refresh, "account_id": "synthetic-capture-account"}}))
            source.chmod(0o600)
            result = self.run_auth("derive", "--codex-auth", str(source), "--output", str(output))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o400)
            self.assertNotIn(real_refresh.encode(), output.read_bytes())
            self.assertNotIn(real_refresh.encode(), result.stdout)
            self.assertNotIn(str(source).encode(), result.stdout + result.stderr)
            self.assertNotIn(b"synthetic-capture-account", result.stdout)
            derived = read_json(output)["openai-codex"]
            self.assertEqual(derived["access"], access)
            self.assertEqual(derived["refresh"], "vela-nonrefreshable-sentinel-v1")

            module = (PACKET / "auth-preflight.mjs").as_uri()
            program = (
                f'import {{loadFrozenOAuth,createReadOnlyCredentialStore}} from {json.dumps(module)};'
                'const store=createReadOnlyCredentialStore(loadFrozenOAuth(process.argv[1]));'
                'if((await store.list()).length!==1)process.exit(2);'
                'let refused=0; for(const name of ["modify","delete"])try{await store[name]("openai-codex",()=>{process.exit(9)})}catch{refused++}'
                'if(refused!==2)process.exit(3);'
            )
            store = subprocess.run(["node", "--input-type=module", "-e", program, str(output)], capture_output=True)
            self.assertEqual(store.returncode, 0, store.stderr)

    def test_existing_output_and_nonregular_inputs_fail_without_mutation_or_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            root.chmod(0o700)
            source = root / "source.json"
            source.write_bytes(compact({"tokens": {"access_token": jwt(), "refresh_token": "never-copy"}}))
            source.chmod(0o600)
            existing = root / "existing.json"
            sentinel = b"existing-sentinel\n"
            existing.write_bytes(sentinel)
            existing.chmod(0o400)
            result = self.run_auth("derive", "--codex-auth", str(source), "--output", str(existing))
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(existing.read_bytes(), sentinel)
            self.assertEqual(stat.S_IMODE(existing.stat().st_mode), 0o400)
            for kind in ("directory", "symlink", "fifo"):
                candidate = root / kind
                if kind == "directory":
                    candidate.mkdir()
                elif kind == "symlink":
                    candidate.symlink_to(source)
                else:
                    os.mkfifo(candidate, 0o600)
                started = time.monotonic()
                refused = self.run_auth("check", "--auth", str(candidate), timeout=2.0)
                self.assertNotEqual(refused.returncode, 0, kind)
                self.assertLess(time.monotonic() - started, 2.0, kind)
                self.assertNotIn(str(candidate).encode(), refused.stderr)


@unittest.skipUnless(IMAGE_ID, "set VELA_PI_TEST_IMAGE_ID to run no-model container gates")
class NoModelContainerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not isinstance(IMAGE_ID, str) or len(IMAGE_ID) != 71 or not IMAGE_ID.startswith("sha256:"):
            raise AssertionError("VELA_PI_TEST_IMAGE_ID must be an exact image ID")
        cls.temporary = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temporary.name)
        cls.study = cls.root / "study"
        if materialize_quiet(["--output", str(cls.study), "--development-worktree"]) != 0:
            raise AssertionError("container test materialization failed")
        cls.uid, cls.gid = os.getuid(), os.getgid()
        cls.capture_reports = []

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temporary.cleanup()

    @classmethod
    def docker(cls, *arguments: str, timeout: float = 120.0) -> subprocess.CompletedProcess:
        return subprocess.run(["docker", *arguments], capture_output=True, timeout=timeout)

    def test_all_four_network_dead_captures(self) -> None:
        auth = self.study / "capture/synthetic-auth.json"
        for run_id, _, _ in materialize.RUNS:
            request = self.study / f"runs/{run_id}/request.json"
            result = self.docker(
                "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL",
                "--security-opt", "no-new-privileges", "--user", f"{self.uid}:{self.gid}", "--workdir", "/workspace",
                "--mount", f"type=bind,src={request},dst=/run/request.json,readonly",
                "--mount", f"type=bind,src={auth},dst=/run/auth.json,readonly",
                "--entrypoint", "node", IMAGE_ID,
                "/opt/participant/request-capture.mjs", "--request", "/run/request.json", "--auth", "/run/auth.json",
            )
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            report = json.loads(result.stdout)
            self.assertEqual(report["run_id"], run_id)
            self.assertEqual(report["external_network_calls"], 0)
            self.assertEqual(report["tool_definition_count"], 0)
            self.capture_reports.append(report)

    def test_nonroot_readonly_container(self) -> None:
        request = self.study / "runs/block-1-profile/request.json"
        auth = self.study / "capture/synthetic-auth.json"
        program = r'''const fs=require("fs"),c=require("crypto"); const lock=fs.readFileSync("/opt/participant/package-lock.json"); const license=fs.readFileSync("/licenses/pi-MIT.txt"); let refused=0; for(const p of ["/run/request.json","/run/auth.json"])try{fs.appendFileSync(p,"x")}catch{refused++} process.stdout.write(JSON.stringify({uid:process.getuid(),node:process.version,pi:require("/opt/participant/node_modules/@earendil-works/pi-coding-agent/package.json").version,lock:"sha256:"+c.createHash("sha256").update(lock).digest("hex"),license:"sha256:"+c.createHash("sha256").update(license).digest("hex"),license_bytes:license.length,refused}));'''
        result = self.docker(
            "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            "--user", f"{self.uid}:{self.gid}", "--mount", f"type=bind,src={request},dst=/run/request.json,readonly",
            "--mount", f"type=bind,src={auth},dst=/run/auth.json,readonly", "--entrypoint", "node", IMAGE_ID, "-e", program,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        report = json.loads(result.stdout)
        self.assertNotEqual(report["uid"], 0)
        self.assertEqual(report["node"], "v24.12.0")
        self.assertEqual(report["pi"], "0.84.1")
        self.assertEqual(report["lock"], digest((PACKET / "package-lock.json").read_bytes()))
        self.assertEqual(report["license"], "sha256:0457f5bcec3b3b211605dfb5d1a49042fd638f3686a410fe099c24a25af13c48")
        self.assertEqual(report["license_bytes"], 1_069)
        self.assertEqual(report["refused"], 2)

    def test_network_none_cross_container_socket_refuses_invalid_request(self) -> None:
        request = self.study / "runs/block-1-profile/request.json"
        auth = self.study / "capture/synthetic-auth.json"
        socket_dir = self.root / "socket"
        socket_dir.mkdir(mode=0o700)
        socket_path = socket_dir / "inference.sock"
        broker = subprocess.Popen(
            [
                "docker", "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL",
                "--security-opt", "no-new-privileges", "--user", f"{self.uid}:{self.gid}",
                "--mount", f"type=bind,src={request},dst=/run/request.json,readonly",
                "--mount", f"type=bind,src={auth},dst=/run/auth.json,readonly",
                "--mount", f"type=bind,src={socket_dir},dst=/broker", "--entrypoint", "node", IMAGE_ID,
                "/opt/participant/egress-broker.mjs", "--socket", "/broker/inference.sock", "--request", "/run/request.json", "--auth", "/run/auth.json",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            deadline = time.monotonic() + 20
            while not socket_path.exists() and broker.poll() is None and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertTrue(socket_path.exists(), "broker socket did not appear")
            client = self.docker(
                "run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
                "--user", f"{self.uid}:{self.gid}", "--mount", f"type=bind,src={socket_dir},dst=/broker,readonly",
                "--entrypoint", "node", IMAGE_ID, "-e",
                'const h=require("http");const r=h.request({socketPath:"/broker/inference.sock",path:"/wrong",method:"POST"},s=>{s.resume();s.on("end",()=>process.exit(s.statusCode===400?0:2))});r.on("error",()=>process.exit(3));r.end("x")',
            )
            self.assertEqual(client.returncode, 0, client.stderr.decode())
            stdout, stderr = broker.communicate(timeout=20)
            self.assertEqual(broker.returncode, 0, stderr.decode())
            rows = [json.loads(line) for line in stdout.splitlines()]
            self.assertEqual([row["kind"] for row in rows], ["ready", "refused"])
            self.assertFalse(any(row["kind"] == "validated_request" for row in rows))
        finally:
            if broker.poll() is None:
                broker.terminate()
                broker.wait(timeout=10)

    def test_wrapper_failure_retains_nonsecret_custody_and_scrubs_auth(self) -> None:
        failure_root = (self.root / "wrapper-failure").resolve()
        failure_root.mkdir(mode=0o700)
        request = read_json(self.study / "runs/block-1-profile/request.json")
        request["model"] = "deliberate-pre-prompt-refusal"
        request_path = failure_root / "request.json"
        request_path.write_bytes(compact(request))
        request_path.chmod(0o444)
        credential = read_json(self.study / "capture/synthetic-auth.json")["openai-codex"]
        source_refresh = "synthetic-source-refresh-must-not-cross"
        source_auth = failure_root / "codex-auth.json"
        source_auth.write_bytes(compact({"tokens": {"access_token": credential["access"], "refresh_token": source_refresh, "account_id": credential["accountId"]}}))
        source_auth.chmod(0o600)
        answer = failure_root / "answer.json"
        audit = failure_root / "audit.jsonl"
        result = subprocess.run(
            [str(PACKET / "run-participant.sh"), IMAGE_ID, str(request_path), str(source_auth), str(answer), str(audit)],
            capture_output=True,
            timeout=90,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(answer.exists())
        self.assertFalse(audit.exists())
        self.assertFalse(list(failure_root.glob(".vela-pi-sensitive.*")))
        evidence = list(failure_root.glob(".vela-pi-evidence.*"))
        self.assertEqual(len(evidence), 1)
        custody = read_json(evidence[0] / "failure-custody.json")
        self.assertEqual(custody["image_id"], IMAGE_ID)
        self.assertEqual(custody["request_raw_root"], digest(request_path.read_bytes()))
        self.assertFalse(custody["retry_authorized"])
        self.assertTrue(custody["sensitive_cleanup_complete"])
        self.assertEqual(custody["answer_usable_status"], "not_determined")
        retained = b"".join(path.read_bytes() for path in evidence[0].iterdir() if path.is_file())
        self.assertNotIn(source_refresh.encode(), retained)
        self.assertNotIn(credential["access"].encode(), retained)


if __name__ == "__main__":
    unittest.main(verbosity=2)
