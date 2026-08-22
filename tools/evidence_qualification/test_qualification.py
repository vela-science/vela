from __future__ import annotations

import io
import json
import subprocess
import sys
import tarfile
import tempfile
import threading
import unittest
from decimal import Decimal
from pathlib import Path

from tools.evidence_qualification.qualification import (
    EVENT_SCHEMA,
    LAUNCH_SCHEMA,
    PERMIT_SCHEMA,
    QUALIFIER,
    RUNNER_VERSION,
    TEARDOWN_SCHEMA,
    TERMINAL_SCHEMA,
    QualificationError,
    canonical_json_bytes,
    canonical_root,
    consume_permit,
    digest,
    normalize_closed_set,
    normalize_shadow_account,
    parse_json,
    pre_key_snapshot,
    provider_derivative,
    qualify_bundle,
    rounded_decimal,
    tree_manifest,
    validate_events,
    validate_schema_boundary,
)


def encoded(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


class BundleFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.registered = {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": False,
            "required": ["schema", "items"],
            "properties": {
                "schema": {"const": "neutral-response.v1"},
                "items": {
                    "type": "array",
                    "minItems": 3,
                    "maxItems": 3,
                    "uniqueItems": True,
                    "items": {
                        "type": "object",
                        "additionalProperties": False,
                        "required": ["id", "value"],
                        "properties": {
                            "id": {"enum": ["a", "b", "c"]},
                            "value": {"type": "integer"},
                        },
                    },
                },
            },
        }
        self.provider = provider_derivative(
            self.registered, ["/properties/items/uniqueItems"]
        )
        self.response = {
            "schema": "neutral-response.v1",
            "items": [
                {"id": "c", "value": 3},
                {"id": "a", "value": 1},
                {"id": "b", "value": 2},
            ],
        }
        self._build()

    def write(self, relative: str, raw: bytes) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def write_json(self, relative: str, value: object) -> Path:
        return self.write(relative, encoded(value))

    def load(self, relative: str) -> dict:
        return json.loads((self.root / relative).read_text())

    def update(self, relative: str, mutate) -> None:
        value = self.load(relative)
        mutate(value)
        self.write_json(relative, value)

    def refresh_capture_bindings(self) -> None:
        evidence = self.root / "fixture/evidence"
        terminal_path = evidence / "terminal-receipt.json"
        terminal = self.load("fixture/evidence/terminal-receipt.json")
        terminal.update(
            {
                "permit_bytes": digest(
                    (
                        self.root
                        / "fixture/permit/neutral-qualification-01.permit.consumed.json"
                    ).read_bytes()
                ),
                "launch_bytes": digest((evidence / "launch.json").read_bytes()),
                "provider_events_bytes": digest(
                    (evidence / "provider-events.jsonl").read_bytes()
                ),
                "provider_stderr_bytes": digest(
                    (evidence / "provider-stderr.txt").read_bytes()
                ),
                "raw_response_bytes": digest(
                    (evidence / "response.raw.json").read_bytes()
                ),
                "teardown_receipt_bytes": digest(
                    (evidence / "teardown.json").read_bytes()
                ),
            }
        )
        self.write_json("fixture/evidence/terminal-receipt.json", terminal)
        paths = [
            self.root / "fixture/permit/neutral-qualification-01.permit.consumed.json",
            evidence / "launch.json",
            evidence / "provider-events.jsonl",
            evidence / "provider-stderr.txt",
            evidence / "response.raw.json",
            terminal_path,
            evidence / "teardown.json",
        ]
        entries = [
            {
                "path": path.relative_to(self.root / "fixture").as_posix(),
                "bytes": path.stat().st_size,
                "sha256": digest(path.read_bytes()),
            }
            for path in paths
        ]
        entries.sort(key=lambda entry: entry["path"])
        capture = {
            "schema": "vela.tooling.neutral-capture-manifest.v1",
            "entries": entries,
        }
        capture["capture_root"] = canonical_root(capture)
        self.write_json("fixture/capture-manifest.json", capture)

    def rewrite_oci(self, mutate) -> None:
        source = self.root / "runtime/a.oci.tar"
        with tarfile.open(source, mode="r") as archive:
            files = [
                (member.name, archive.extractfile(member).read())
                for member in archive.getmembers()
            ]
        mutate(files)
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w") as archive:
            for name, raw in files:
                info = tarfile.TarInfo(name)
                info.size = len(raw)
                info.mtime = 1_757_289_600
                info.mode = 0o644
                archive.addfile(info, io.BytesIO(raw))
        self.write("runtime/a.oci.tar", buffer.getvalue())
        self.write("runtime/b.oci.tar", buffer.getvalue())

    def _oci(self) -> tuple[bytes, str, str, list[str], str]:
        config_raw = b'{"architecture":"arm64","os":"linux"}\n'
        config_digest = digest(config_raw)
        layer_raws = [b"first deterministic layer\n", b"second deterministic layer\n"]
        layer_digests = [digest(raw) for raw in layer_raws]
        manifest = {
            "schemaVersion": 2,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": config_digest,
                "size": len(config_raw),
            },
            "layers": [
                {
                    "mediaType": "application/vnd.oci.image.layer.v1.tar",
                    "digest": layer_digest,
                    "size": len(layer_raw),
                }
                for layer_raw, layer_digest in zip(
                    layer_raws, layer_digests, strict=True
                )
            ],
        }
        manifest_raw = canonical_json_bytes(manifest)
        manifest_digest = digest(manifest_raw)
        index = {
            "schemaVersion": 2,
            "manifests": [
                {
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "digest": manifest_digest,
                    "size": len(manifest_raw),
                }
            ],
        }
        layout_raw = b'{"imageLayoutVersion":"1.0.0"}\n'
        files = {
            "index.json": canonical_json_bytes(index),
            "oci-layout": layout_raw,
            "blobs/sha256/" + manifest_digest.removeprefix("sha256:"): manifest_raw,
            "blobs/sha256/" + config_digest.removeprefix("sha256:"): config_raw,
        }
        for layer_raw, layer_digest in zip(layer_raws, layer_digests, strict=True):
            files["blobs/sha256/" + layer_digest.removeprefix("sha256:")] = layer_raw
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w") as archive:
            for name, raw in sorted(files.items()):
                info = tarfile.TarInfo(name)
                info.size = len(raw)
                info.mtime = 1_757_289_600
                info.mode = 0o644
                archive.addfile(info, io.BytesIO(raw))
        return (
            buffer.getvalue(),
            manifest_digest,
            config_digest,
            layer_digests,
            digest(layout_raw),
        )

    def _build(self) -> None:
        self.write_json("schemas/registered.json", self.registered)
        self.write_json("schemas/provider.json", self.provider)
        self.write_json("schemas/valid-response.json", self.response)
        dockerfile = b"""FROM scratch
ARG SOURCE_DATE_EPOCH
COPY vendor/ca-certificates.deb /inputs/ca-certificates.deb
RUN --network=none test -s /inputs/ca-certificates.deb
"""
        self.write("runtime/source/Dockerfile", dockerfile)
        package = self.write(
            "runtime/source/vendor/ca-certificates.deb", b"vendored-ca-package\n"
        )
        license_path = self.write(
            "runtime/source/vendor/ca-certificates.LICENSE", b"fixture license\n"
        )
        trust = self.write(
            "runtime/source/vendor/ca-certificates.crt",
            b"-----BEGIN CERTIFICATE-----\nneutral-fixture\n-----END CERTIFICATE-----\n",
        )
        shadow_a = self.write(
            "runtime/shadow-day-a",
            b"root:!:20000:0:99999:7:::\nparticipant:!:20001:0:99999:7:::\n",
        )
        shadow_b = self.write(
            "runtime/shadow-day-b",
            b"root:!:20000:0:99999:7:::\nparticipant:!:20002:0:99999:7:::\n",
        )
        normalized = normalize_shadow_account(
            shadow_a.read_bytes(), "participant", 20339
        )
        manifest = tree_manifest(self.root / "runtime/source")
        self.write_json("runtime/source-manifest.json", manifest)
        build_inputs = {
            "schema": "vela.tooling.vendored-build-inputs.v1",
            "inputs": [
                {
                    "path": package.relative_to(self.root).as_posix(),
                    "bytes": package.stat().st_size,
                    "sha256": digest(package.read_bytes()),
                    "source_url": "https://example.invalid/ca-certificates.deb",
                    "source_sha256": digest(package.read_bytes()),
                    "license_path": license_path.relative_to(self.root).as_posix(),
                }
            ],
        }
        self.write_json("runtime/build-inputs.json", build_inputs)
        source_root = canonical_root(manifest)
        build_inputs_root = canonical_root(build_inputs)
        oci_raw, image_digest, config_digest, layer_digests, layout_digest = self._oci()
        self.write("runtime/a.oci.tar", oci_raw)
        self.write("runtime/b.oci.tar", oci_raw)
        controls = {
            "no_cache": True,
            "provenance": False,
            "pull": False,
            "rewrite_timestamp": True,
        }
        for builder in ("independent-a", "independent-b"):
            self.write_json(
                f"runtime/{builder}.json",
                {
                    "schema": "vela.tooling.oci-build-receipt.v1",
                    "builder": builder,
                    "empty_cache": True,
                    "network_during_build": False,
                    "platform": "linux/arm64",
                    "source_date_epoch": 1_757_289_600,
                    "source_root": source_root,
                    "build_inputs_root": build_inputs_root,
                    "controls": controls,
                    "image_digest": image_digest,
                    "config_digest": config_digest,
                    "layer_digests": layer_digests,
                    "oci_layout_bytes": layout_digest,
                    "oci_tar_bytes": digest(oci_raw),
                },
            )
        strict_arguments = [
            "approval_policy=never",
            "web_search=disabled",
            "tools=none",
        ]
        configuration = {
            "runner_version": RUNNER_VERSION,
            "model": "neutral-model",
            "reasoning_effort": "high",
            "service_tier": "default",
            "timeout_seconds": 600,
            "output_token_ceiling": 8192,
            "attempt": 1,
            "retries": 0,
            "tools": "none",
            "strict_arguments": strict_arguments,
            "compatibility_receipt": "config/compatibility.json",
        }
        self.write_json(
            "config/compatibility.json",
            {
                "schema": "vela.tooling.strict-config-compatibility.v1",
                "runner_version": "neutral-runner 1",
                "strict_parse_passed": True,
                "provider_contact_possible": False,
                "accepted_arguments": strict_arguments,
                "stderr_sha256": digest(b""),
                "image_digest": image_digest,
                "configuration_root": canonical_root(configuration),
                "runtime_source_root": source_root,
                "dockerfile_bytes": digest(dockerfile),
            },
        )
        self.update(
            "config/compatibility.json",
            lambda value: value.update({"runner_version": RUNNER_VERSION}),
        )
        permit_bindings = {
            "attempt": 1,
            "runner_version": RUNNER_VERSION,
            "runtime_source_root": source_root,
            "configuration_root": canonical_root(configuration),
            "image_digest": image_digest,
            "registered_schema_bytes": digest(
                (self.root / "schemas/registered.json").read_bytes()
            ),
            "provider_schema_bytes": digest(
                (self.root / "schemas/provider.json").read_bytes()
            ),
            "timeout_seconds": 600,
        }
        participant_identity = {
            "registration_id": "registration-01",
            "assignment_id": "assignment-01",
            "participant_id": "participant-01",
            "run_id": "participant-run-01",
            "condition": "participant",
            "prompt_root": digest(b"participant prompt\n"),
            "packet_root": digest(b"participant packet\n"),
        }
        participant_permit = {
            "schema": PERMIT_SCHEMA,
            **participant_identity,
            **permit_bindings,
            "status": "held",
            "issued_at": "2026-08-21T00:00:00Z",
            "consumed_at": None,
        }
        self.write_json(
            "permit/hold-state.json",
            {
                "schema": "vela.tooling.participant-hold.v1",
                "status": "hold",
                "reason": "qualification_incomplete",
                "registration_id": participant_identity["registration_id"],
                "assignment_id": participant_identity["assignment_id"],
            },
        )
        self.write_json("permit/participant-run-01.permit.json", participant_permit)
        fixture_identity = {
            "registration_id": "qualification-registration",
            "assignment_id": "neutral-assignment",
            "participant_id": "neutral-fixture",
            "run_id": "neutral-qualification-01",
            "condition": "neutral-no-science",
            "prompt_root": digest(b"neutral prompt\n"),
            "packet_root": digest(b"neutral packet\n"),
        }
        fixture_template = {
            "schema": PERMIT_SCHEMA,
            **fixture_identity,
            **permit_bindings,
            "status": "held",
            "issued_at": "2026-08-21T00:00:00Z",
            "consumed_at": None,
        }
        self.write_json(
            "fixture/permit/neutral-qualification-01.permit.template.json",
            fixture_template,
        )
        fixture_consumed = dict(fixture_template)
        fixture_consumed["status"] = "consumed"
        fixture_consumed["consumed_at"] = "2026-08-21T00:00:00Z"
        consumed_path = self.write_json(
            "fixture/permit/neutral-qualification-01.permit.consumed.json",
            fixture_consumed,
        )
        launch_path = self.write_json(
            "fixture/evidence/launch.json",
            {
                "schema": LAUNCH_SCHEMA,
                "run_id": "neutral-qualification-01",
                "attempt": 1,
                "runner_version": RUNNER_VERSION,
                "permit_bytes": digest(consumed_path.read_bytes()),
                "configuration_root": canonical_root(configuration),
                "runtime_source_root": source_root,
                "image_digest": image_digest,
                "started_at": "2026-08-21T00:00:00Z",
            },
        )
        response_path = self.write_json(
            "fixture/evidence/response.raw.json", self.response
        )
        response_text = response_path.read_text().strip()
        events = (
            json.dumps(
                {
                    "schema": EVENT_SCHEMA,
                    "type": "thread.started",
                    "run_id": "neutral-qualification-01",
                    "thread_id": "thread-01",
                    "at": "2026-08-21T00:00:00.100000Z",
                }
            )
            + "\n"
            + json.dumps(
                {
                    "schema": EVENT_SCHEMA,
                    "type": "turn.started",
                    "run_id": "neutral-qualification-01",
                    "thread_id": "thread-01",
                    "turn_id": "turn-01",
                    "at": "2026-08-21T00:00:00.200000Z",
                }
            )
            + "\n"
            + json.dumps(
                {
                    "type": "item.completed",
                    "schema": EVENT_SCHEMA,
                    "run_id": "neutral-qualification-01",
                    "thread_id": "thread-01",
                    "turn_id": "turn-01",
                    "response_id": "response-01",
                    "at": "2026-08-21T00:00:00.500000Z",
                    "item": {"type": "agent_message", "text": response_text},
                }
            )
            + "\n"
            + json.dumps(
                {
                    "type": "turn.completed",
                    "schema": EVENT_SCHEMA,
                    "run_id": "neutral-qualification-01",
                    "thread_id": "thread-01",
                    "turn_id": "turn-01",
                    "response_id": "response-01",
                    "at": "2026-08-21T00:00:00.800000Z",
                    "usage": {
                        "input_tokens": 900_000,
                        "cached_input_tokens": 899_000,
                        "output_tokens": 100,
                        "tool_call_count": 0,
                    },
                }
            )
            + "\n"
        ).encode()
        events_path = self.write("fixture/evidence/provider-events.jsonl", events)
        stderr_path = self.write("fixture/evidence/provider-stderr.txt", b"")
        teardown_path = self.write_json(
            "fixture/evidence/teardown.json",
            {
                "schema": TEARDOWN_SCHEMA,
                "run_id": "neutral-qualification-01",
                "attempt": 1,
                "status": "completed",
                "exit_code": 0,
                "started_at": "2026-08-21T00:00:00.900000Z",
                "process_reaped": True,
                "network_disabled": True,
                "mounts_detached": True,
                "completed_at": "2026-08-21T00:00:01Z",
                "duration_seconds": 0.1,
                "permit_bytes": digest(consumed_path.read_bytes()),
                "launch_bytes": digest(launch_path.read_bytes()),
                "provider_stderr_bytes": digest(stderr_path.read_bytes()),
            },
        )
        canonical_response = normalize_closed_set(
            self.response, "items", "id", ["a", "b", "c"]
        )
        terminal_path = self.write_json(
            "fixture/evidence/terminal-receipt.json",
            {
                "schema": TERMINAL_SCHEMA,
                "status": "completed",
                "run_id": "neutral-qualification-01",
                "attempt": 1,
                "runner_version": RUNNER_VERSION,
                "runtime_source_root": source_root,
                "started_at": "2026-08-21T00:00:00Z",
                "completed_at": "2026-08-21T00:00:00.900000Z",
                "duration_seconds": 0.9,
                "exit_code": 0,
                "permit_bytes": digest(consumed_path.read_bytes()),
                "launch_bytes": digest(launch_path.read_bytes()),
                "provider_events_bytes": digest(events_path.read_bytes()),
                "provider_stderr_bytes": digest(stderr_path.read_bytes()),
                "raw_response_bytes": digest(response_path.read_bytes()),
                "teardown_receipt_bytes": digest(teardown_path.read_bytes()),
                "registered_schema_bytes": digest(
                    (self.root / "schemas/registered.json").read_bytes()
                ),
                "provider_schema_bytes": digest(
                    (self.root / "schemas/provider.json").read_bytes()
                ),
                "canonical_response_root": canonical_root(canonical_response),
                "configuration_root": canonical_root(configuration),
                "image_digest": image_digest,
                "trust_bundle_sha256": digest(trust.read_bytes()),
                "cumulative_provider_usage_is_telemetry_only": True,
                "credential_retained": False,
            },
        )
        capture_paths = [
            consumed_path,
            launch_path,
            events_path,
            stderr_path,
            response_path,
            terminal_path,
            teardown_path,
        ]
        capture_entries = [
            {
                "path": path.relative_to(self.root / "fixture").as_posix(),
                "bytes": path.stat().st_size,
                "sha256": digest(path.read_bytes()),
            }
            for path in capture_paths
        ]
        capture_entries.sort(key=lambda entry: entry["path"])
        capture = {
            "schema": "vela.tooling.neutral-capture-manifest.v1",
            "entries": capture_entries,
        }
        capture["capture_root"] = canonical_root(capture)
        self.write_json("fixture/capture-manifest.json", capture)
        scoring_input = self.write_json(
            "fixture/scoring/run.json",
            {"schema": "neutral-run.v1", "run_id": "neutral-qualification-01"},
        )
        snapshot_body = {
            "schema": "vela.tooling.pre-key-snapshot.v1",
            "entries": [
                {
                    "path": scoring_input.relative_to(self.root).as_posix(),
                    "bytes": scoring_input.stat().st_size,
                    "sha256": digest(scoring_input.read_bytes()),
                },
                {
                    "path": response_path.relative_to(self.root).as_posix(),
                    "bytes": response_path.stat().st_size,
                    "sha256": digest(response_path.read_bytes()),
                },
            ],
        }
        snapshot = dict(snapshot_body)
        snapshot["snapshot_root"] = canonical_root(snapshot_body)
        self.write_json("fixture/scoring-snapshot.json", snapshot)
        config = {
            "schema": "vela.tooling.evidence-qualification.v1",
            "status": "hold",
            "configuration": configuration,
            "schemas": {
                "registered": "schemas/registered.json",
                "provider": "schemas/provider.json",
                "deleted_pointers": ["/properties/items/uniqueItems"],
                "valid_response": "schemas/valid-response.json",
                "closed_set": {
                    "field": "items",
                    "key": "id",
                    "expected": ["a", "b", "c"],
                },
            },
            "runtime": {
                "source_dir": "runtime/source",
                "source_manifest": "runtime/source-manifest.json",
                "build_inputs": "runtime/build-inputs.json",
                "oci_archives": ["runtime/a.oci.tar", "runtime/b.oci.tar"],
                "oci_receipts": [
                    "runtime/independent-a.json",
                    "runtime/independent-b.json",
                ],
                "platform": "linux/arm64",
                "source_date_epoch": 1_757_289_600,
                "trust_bundle": trust.relative_to(self.root).as_posix(),
                "trust_bundle_sha256": digest(trust.read_bytes()),
                "trust_bundle_container_path": "/etc/ssl/certs/ca-certificates.crt",
                "ssl_cert_file": "/etc/ssl/certs/ca-certificates.crt",
                "mounts": [
                    {
                        "source": str((self.root / "schemas").resolve()),
                        "target": "/input",
                        "read_only": True,
                    },
                    {
                        "source": str(trust.resolve()),
                        "target": "/etc/ssl/certs/ca-certificates.crt",
                        "read_only": True,
                    },
                ],
                "account_database": {
                    "account": "participant",
                    "expected_accounts": ["root", "participant"],
                    "fixed_day": 20339,
                    "fixtures": [
                        {
                            "path": shadow_a.relative_to(self.root).as_posix(),
                            "source_day": 20001,
                            "sha256": digest(shadow_a.read_bytes()),
                        },
                        {
                            "path": shadow_b.relative_to(self.root).as_posix(),
                            "source_day": 20002,
                            "sha256": digest(shadow_b.read_bytes()),
                        },
                    ],
                    "normalized_sha256": digest(normalized),
                },
            },
            "participant_permit": {
                "hold": "permit/hold-state.json",
                "permit": "permit/participant-run-01.permit.json",
                "consumed_permit": "permit/participant-run-01.permit.consumed.json",
                "identity": participant_identity,
            },
            "neutral_fixture": {
                "directory": "fixture",
                "permit_template": "fixture/permit/neutral-qualification-01.permit.template.json",
                "consumed_permit": "fixture/permit/neutral-qualification-01.permit.consumed.json",
                "launch": "fixture/evidence/launch.json",
                "events": "fixture/evidence/provider-events.jsonl",
                "stderr": "fixture/evidence/provider-stderr.txt",
                "raw_response": "fixture/evidence/response.raw.json",
                "terminal_receipt": "fixture/evidence/terminal-receipt.json",
                "teardown_receipt": "fixture/evidence/teardown.json",
                "capture_manifest": "fixture/capture-manifest.json",
                "identity": fixture_identity,
            },
            "scoring_snapshot": "fixture/scoring-snapshot.json",
            "self_verification": {
                "command": [
                    sys.executable,
                    str(QUALIFIER),
                    "--bundle",
                    str(self.root.resolve()),
                ],
                "qualifier_sha256": digest(QUALIFIER.read_bytes()),
                "environment_prefix": sys.prefix,
                "jsonschema_module": __import__("jsonschema").__file__,
            },
        }
        self.write_json("qualification.json", config)


class EvidenceQualificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.fixture = BundleFixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def assertBlocked(self, message: str) -> None:
        with self.assertRaisesRegex(QualificationError, message):
            qualify_bundle(self.root)

    def test_end_to_end_neutral_gate_qualifies_hold_before_participant_execution(
        self,
    ) -> None:
        receipt = qualify_bundle(self.root)
        self.assertEqual(receipt["status"], "qualified_hold")
        self.assertEqual(receipt["provider_calls"], 0)
        self.assertEqual(receipt["scientific_sessions"], 0)
        self.assertEqual(receipt["participant_permits_consumed"], 0)
        self.assertTrue(all(receipt["gates"].values()))

    def test_deterministic_regeneration_is_byte_identical(self) -> None:
        first = canonical_json_bytes(qualify_bundle(self.root))
        second = canonical_json_bytes(qualify_bundle(self.root))
        self.assertEqual(first, second)

    def test_provider_derivative_is_exact_and_full_schema_is_draft_2020_12(
        self,
    ) -> None:
        validate_schema_boundary(
            self.fixture.registered,
            self.fixture.provider,
            ["/properties/items/uniqueItems"],
            self.fixture.response,
        )
        with self.assertRaisesRegex(QualificationError, "not_exact_derivative"):
            validate_schema_boundary(
                self.fixture.registered,
                self.fixture.registered,
                ["/properties/items/uniqueItems"],
                self.fixture.response,
            )
        legacy = dict(self.fixture.registered)
        legacy["$schema"] = "http://json-schema.org/draft-07/schema#"
        with self.assertRaisesRegex(QualificationError, "not_draft_2020_12"):
            validate_schema_boundary(
                legacy,
                self.fixture.provider,
                ["/properties/items/uniqueItems"],
                self.fixture.response,
            )
        with self.assertRaisesRegex(QualificationError, "deleted_keyword_not_proven"):
            provider_derivative(self.fixture.registered, ["/required"])

    def test_valid_order_variants_do_not_become_false_non_results_but_bad_sets_close(
        self,
    ) -> None:
        reversed_response = dict(self.fixture.response)
        reversed_response["items"] = list(reversed(self.fixture.response["items"]))
        first = normalize_closed_set(
            self.fixture.response, "items", "id", ["a", "b", "c"]
        )
        second = normalize_closed_set(reversed_response, "items", "id", ["a", "b", "c"])
        self.assertEqual(canonical_root(first), canonical_root(second))
        self.assertNotEqual(encoded(self.fixture.response), encoded(reversed_response))
        for items in (
            [{"id": "a", "value": 1}] * 3,
            self.fixture.response["items"][:-1],
            self.fixture.response["items"][:-1] + [{"id": "z", "value": 3}],
        ):
            candidate = dict(self.fixture.response)
            candidate["items"] = items
            with self.assertRaises(QualificationError):
                normalize_closed_set(candidate, "items", "id", ["a", "b", "c"])

    def test_atomic_single_use_permit_has_one_winner(self) -> None:
        directory = self.root / "race"
        directory.mkdir()
        expected = dict(
            self.fixture.load("qualification.json")["participant_permit"]["identity"]
        )
        configuration = self.fixture.load("qualification.json")["configuration"]
        expected.update(
            {
                "attempt": 1,
                "runner_version": RUNNER_VERSION,
                "runtime_source_root": canonical_root(
                    self.fixture.load("runtime/source-manifest.json")
                ),
                "configuration_root": canonical_root(configuration),
                "image_digest": self.fixture.load("config/compatibility.json")[
                    "image_digest"
                ],
                "registered_schema_bytes": digest(
                    (self.root / "schemas/registered.json").read_bytes()
                ),
                "provider_schema_bytes": digest(
                    (self.root / "schemas/provider.json").read_bytes()
                ),
                "timeout_seconds": 600,
            }
        )
        permit = {
            "schema": PERMIT_SCHEMA,
            **expected,
            "run_id": "race-01",
            "status": "held",
            "issued_at": "2026-08-21T00:00:00Z",
            "consumed_at": None,
        }
        expected["run_id"] = "race-01"
        (directory / "race-01.permit.json").write_bytes(encoded(permit))
        outcomes = []
        lock = threading.Lock()

        def attempt() -> None:
            try:
                consume_permit(directory, "race-01", expected)
                outcome = "won"
            except QualificationError:
                outcome = "blocked"
            with lock:
                outcomes.append(outcome)

        threads = [threading.Thread(target=attempt) for _ in range(8)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(outcomes.count("won"), 1)
        self.assertFalse((directory / "race-01.permit.json").exists())
        self.assertTrue((directory / "race-01.permit.consumed.json").is_file())

    def test_capture_bridge_requires_raw_event_response_terminal_and_teardown_bytes(
        self,
    ) -> None:
        (self.root / "fixture/evidence/teardown.json").unlink()
        self.assertBlocked("neutral_fixture_teardown")

    def test_pre_key_snapshot_closes_scorer_reread_race(self) -> None:
        manifest = self.fixture.load("fixture/scoring-snapshot.json")
        frozen = pre_key_snapshot(self.root, manifest)
        path = self.root / "fixture/scoring/run.json"
        original = dict(frozen.entries)["fixture/scoring/run.json"]
        path.write_text('{"changed":true}\n')
        self.assertEqual(dict(frozen.entries)["fixture/scoring/run.json"], original)
        with self.assertRaisesRegex(QualificationError, "snapshot_entry_drift"):
            pre_key_snapshot(self.root, manifest)

    def test_decimal_serialization_is_binary_float_free_and_byte_stable(self) -> None:
        value = rounded_decimal(Decimal(1) / Decimal(3), Decimal("0.000000000000001"))
        expected = b'{"rate":0.333333333333333}\n'
        self.assertEqual(canonical_json_bytes({"rate": value}), expected)
        self.assertEqual(
            canonical_json_bytes(parse_json(expected, "decimal")), expected
        )
        with self.assertRaisesRegex(QualificationError, "binary_float_forbidden"):
            canonical_json_bytes({"rate": float(value)})

    def test_strict_configuration_rejects_unknown_or_stale_arguments(self) -> None:
        self.fixture.update(
            "qualification.json",
            lambda value: value["configuration"].update({"legacy_option": False}),
        )
        self.assertBlocked("configuration_fields_invalid")

    def test_strict_configuration_receipt_is_bound_to_the_qualified_image(self) -> None:
        self.fixture.update(
            "config/compatibility.json",
            lambda value: value.update({"image_digest": "sha256:" + "0" * 64}),
        )
        self.assertBlocked("configuration_compatibility_invalid")

    def test_relative_or_noncanonical_mounts_fail_closed(self) -> None:
        self.fixture.update(
            "qualification.json",
            lambda value: value["runtime"]["mounts"][0].update({"source": "schemas"}),
        )
        self.assertBlocked("mount_source_not_canonical_absolute")

    def test_tls_bundle_is_required_and_byte_bound(self) -> None:
        self.fixture.update(
            "qualification.json",
            lambda value: value["runtime"].update(
                {"trust_bundle_sha256": digest(b"wrong")}
            ),
        )
        self.assertBlocked("trust_bundle_invalid")

    def test_clean_build_oci_archives_must_be_byte_identical_and_receipt_bound(
        self,
    ) -> None:
        with (self.root / "runtime/b.oci.tar").open("ab") as handle:
            handle.write(b"drift")
        self.assertBlocked("oci_archives_not_byte_identical")

    def test_account_normalization_removes_date_and_rejects_malformed_records(
        self,
    ) -> None:
        first = b"participant:!:20001:0:99999:7:::\n"
        second = b"participant:!:20002:0:99999:7:::\n"
        self.assertEqual(
            normalize_shadow_account(first, "participant", 20339),
            normalize_shadow_account(second, "participant", 20339),
        )
        adversaries = (
            b"participant:!:notaday:0:99999:7:::\n",
            first + first,
            b"participant:!:20001:0:99999:7::::extra\n",
        )
        for raw in adversaries:
            with self.subTest(raw=raw), self.assertRaises(QualificationError):
                normalize_shadow_account(raw, "participant", 20339)

    def test_time_sensitive_package_metadata_and_incomplete_provenance_fail(
        self,
    ) -> None:
        dockerfile = self.root / "runtime/source/Dockerfile"
        dockerfile.write_text(dockerfile.read_text() + "RUN apt-get update\n")
        self.assertBlocked("source_manifest_drift")
        manifest = tree_manifest(self.root / "runtime/source")
        self.fixture.write_json("runtime/source-manifest.json", manifest)
        source_root = canonical_root(manifest)
        for receipt_name in (
            "runtime/independent-a.json",
            "runtime/independent-b.json",
        ):
            self.fixture.update(
                receipt_name, lambda value: value.update({"source_root": source_root})
            )
        self.assertBlocked("dockerfile_not_reproducible")

    def test_vendored_build_input_requires_complete_source_and_license_provenance(
        self,
    ) -> None:
        self.fixture.update(
            "runtime/build-inputs.json",
            lambda value: value["inputs"][0].update(
                {"source_sha256": digest(b"different-source")}
            ),
        )
        self.assertBlocked("build_input_provenance_incomplete")

    def test_launch_receipt_must_bind_the_consumed_permit(self) -> None:
        self.fixture.update(
            "fixture/evidence/launch.json",
            lambda value: value.update({"permit_bytes": digest(b"other-permit")}),
        )
        launch = self.root / "fixture/evidence/launch.json"
        self.fixture.update(
            "fixture/evidence/terminal-receipt.json",
            lambda value: value.update({"launch_bytes": digest(launch.read_bytes())}),
        )
        self.assertBlocked("launch_binding_invalid")

    def test_self_verification_cannot_target_a_predecessor_artifact(self) -> None:
        self.fixture.update(
            "qualification.json",
            lambda value: value["self_verification"]["command"].__setitem__(
                1, "/tmp/predecessor.py"
            ),
        )
        self.assertBlocked("targets_predecessor")

    def test_large_cumulative_input_telemetry_is_valid_but_output_limit_is_not(
        self,
    ) -> None:
        events = (self.root / "fixture/evidence/provider-events.jsonl").read_bytes()
        summary = validate_events(events, 8192)
        self.assertEqual(summary["usage"]["input_tokens"], 900_000)
        mutated = events.replace(b'"output_tokens": 100', b'"output_tokens": 8193')
        with self.assertRaisesRegex(QualificationError, "output_token_ceiling"):
            validate_events(mutated, 8192)

    def test_participant_release_or_preexisting_consumption_blocks_qualification(
        self,
    ) -> None:
        self.fixture.update(
            "permit/hold-state.json", lambda value: value.update({"status": "release"})
        )
        self.assertBlocked("participant_permit_not_held")

    def test_locked_interpreter_stays_inside_environment_without_user_packages(
        self,
    ) -> None:
        executable = Path(sys.executable)
        self.assertIn(Path(sys.prefix), executable.parents)
        environment = {
            "PATH": str(executable.parent),
            "PYTHONNOUSERSITE": "1",
        }
        result = subprocess.run(
            [sys.executable, "-I", str(QUALIFIER), "--bundle", str(self.root)],
            check=False,
            capture_output=True,
            env=environment,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        resolved = str(executable.resolve())
        if resolved != sys.executable:
            self.fixture.update(
                "qualification.json",
                lambda value: value["self_verification"]["command"].__setitem__(
                    0, resolved
                ),
            )
            self.assertBlocked("targets_predecessor")

    def test_runner_version_is_closed_exact_and_bound_to_all_runtime_roots(
        self,
    ) -> None:
        original = self.fixture.load("config/compatibility.json")
        variants = [
            {**original, "runner_version": False},
            {key: value for key, value in original.items() if key != "runner_version"},
            {**original, "runner_version": "neutral-runner/0"},
            {**original, "runtime_source_root": digest(b"stale")},
            {**original, "configuration_root": digest(b"stale")},
            {**original, "dockerfile_bytes": digest(b"stale")},
        ]
        for variant in variants:
            with self.subTest(variant=variant):
                self.fixture.write_json("config/compatibility.json", variant)
                self.assertBlocked("configuration_compatibility")
        self.fixture.write_json("config/compatibility.json", original)
        config = self.fixture.load("qualification.json")
        config["configuration"]["runner_version"] = False
        self.fixture.write_json("qualification.json", config)
        self.assertBlocked("configuration_contract_invalid")

    def test_cross_day_account_fixtures_are_distinct_and_closed(self) -> None:
        qualification = self.fixture.load("qualification.json")
        original = qualification["runtime"]["account_database"]
        first = original["fixtures"][0]
        adversaries = []
        duplicate = json.loads(json.dumps(original))
        duplicate["fixtures"] = [first, dict(first)]
        adversaries.append(duplicate)
        same_day = json.loads(json.dumps(original))
        same_day["fixtures"][1]["source_day"] = 20001
        adversaries.append(same_day)
        malformed_day = json.loads(json.dumps(original))
        malformed_day["fixtures"][1]["source_day"] = "20002"
        adversaries.append(malformed_day)
        wrong_accounts = json.loads(json.dumps(original))
        wrong_accounts["expected_accounts"] = ["root", "intruder"]
        adversaries.append(wrong_accounts)
        for account in adversaries:
            with self.subTest(account=account):
                qualification["runtime"]["account_database"] = account
                self.fixture.write_json("qualification.json", qualification)
                self.assertBlocked("account_")

    def test_cross_day_account_unexpected_metadata_fails(self) -> None:
        shadow = self.root / "runtime/shadow-day-b"
        shadow.write_bytes(
            b"root:!:20000:0:99999:7:::\nparticipant:!:20002:1:99999:7:::\n"
        )
        config = self.fixture.load("qualification.json")
        fixture = config["runtime"]["account_database"]["fixtures"][1]
        fixture["sha256"] = digest(shadow.read_bytes())
        self.fixture.write_json("qualification.json", config)
        self.assertBlocked("account_fixture_metadata_drift")

    def test_comment_only_dockerfile_controls_fail_as_non_executable(self) -> None:
        dockerfile = self.root / "runtime/source/Dockerfile"
        dockerfile.write_text(
            "FROM scratch\n# ARG SOURCE_DATE_EPOCH\n"
            "# RUN --network=none true\nCOPY vendor/ca-certificates.deb /inputs/x\n"
            "RUN true\n"
        )
        self.fixture.write_json(
            "runtime/source-manifest.json",
            tree_manifest(self.root / "runtime/source"),
        )
        self.assertBlocked("dockerfile_not_reproducible")

    def test_oci_custody_rejects_missing_duplicate_extra_or_substituted_blobs(
        self,
    ) -> None:
        original = (self.root / "runtime/a.oci.tar").read_bytes()

        def layer_indexes(files):
            return [
                index
                for index, (name, _) in enumerate(files)
                if name.startswith("blobs/sha256/")
            ][-2:]

        mutations = {
            "missing": lambda files: files.pop(layer_indexes(files)[0]),
            "duplicate": lambda files: files.append(files[layer_indexes(files)[0]]),
            "extra": lambda files: files.append(("unexpected", b"extra")),
            "substituted": lambda files: files.__setitem__(
                layer_indexes(files)[0],
                (files[layer_indexes(files)[0]][0], b"substituted"),
            ),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name):
                self.write_oci_pair(original)
                self.fixture.rewrite_oci(mutate)
                self.assertBlocked("oci_")

    def write_oci_pair(self, raw: bytes) -> None:
        self.fixture.write("runtime/a.oci.tar", raw)
        self.fixture.write("runtime/b.oci.tar", raw)

    def test_oci_reordered_layers_or_manifest_descriptor_fails(self) -> None:
        original = (self.root / "runtime/a.oci.tar").read_bytes()

        def reorder_layers(files) -> None:
            manifest_index = next(
                index
                for index, (name, raw) in enumerate(files)
                if name.startswith("blobs/sha256/") and b'"layers"' in raw
            )
            name, raw = files[manifest_index]
            manifest = json.loads(raw)
            manifest["layers"].reverse()
            files[manifest_index] = (name, canonical_json_bytes(manifest))

        self.fixture.rewrite_oci(reorder_layers)
        self.assertBlocked("oci_manifest_bytes_drift")
        self.write_oci_pair(original)

        def reorder_index(files) -> None:
            index = next(i for i, (name, _) in enumerate(files) if name == "index.json")
            name, raw = files[index]
            value = json.loads(raw)
            value["manifests"].append(dict(value["manifests"][0]))
            value["manifests"].reverse()
            files[index] = (name, canonical_json_bytes(value))

        self.fixture.rewrite_oci(reorder_index)
        self.assertBlocked("oci_manifest_count_invalid")

    def test_bundle_rejects_internal_symlink_traversal_and_file_aliases(self) -> None:
        alias = self.root / "schema-alias"
        alias.symlink_to(self.root / "schemas", target_is_directory=True)
        self.assertBlocked("bundle_symlink_forbidden")
        alias.unlink()
        self.fixture.update(
            "qualification.json",
            lambda value: value["schemas"].update(
                {"registered": "schemas/../schemas/registered.json"}
            ),
        )
        self.assertBlocked("path_unsafe")

    def test_bundle_rejects_hardlink_alias_between_distinct_roles(self) -> None:
        alias = self.root / "schemas/registered-alias.json"
        alias.hardlink_to(self.root / "schemas/registered.json")
        self.assertBlocked("bundle_file_alias_forbidden")

    def test_closed_participant_permit_rejects_review_adversaries(self) -> None:
        path = "permit/participant-run-01.permit.json"
        original = self.fixture.load(path)
        variants = []
        extra = dict(original)
        extra["forged"] = True
        variants.append(extra)
        missing = dict(original)
        missing.pop("packet_root")
        variants.append(missing)
        boolean_attempt = dict(original)
        boolean_attempt["attempt"] = True
        variants.append(boolean_attempt)
        wrong_run = dict(original)
        wrong_run["run_id"] = "other-run"
        variants.append(wrong_run)
        cross_assignment = dict(original)
        cross_assignment["assignment_id"] = "assignment-02"
        variants.append(cross_assignment)
        wrong_schema = dict(original)
        wrong_schema["schema"] = "forged"
        variants.append(wrong_schema)
        for permit in variants:
            with self.subTest(permit=permit):
                self.fixture.write_json(path, permit)
                self.assertBlocked("permit_")

    def test_preexisting_consumed_participant_permit_is_replay(self) -> None:
        source = self.root / "permit/participant-run-01.permit.json"
        consumed = self.root / "permit/participant-run-01.permit.consumed.json"
        consumed.write_bytes(source.read_bytes())
        self.assertBlocked("already_consumed")

    def test_closed_lifecycle_rejects_forged_schemas_and_reversed_order(self) -> None:
        self.fixture.update(
            "fixture/evidence/launch.json",
            lambda value: value.update({"schema": "forged-launch"}),
        )
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("launch_binding_invalid")

    def test_closed_events_reject_reversed_lifecycle_and_boolean_usage(self) -> None:
        events_path = self.root / "fixture/evidence/provider-events.jsonl"
        original = events_path.read_bytes()
        lines = original.splitlines()
        events_path.write_bytes(b"\n".join([lines[3], *lines[:3]]) + b"\n")
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("event_sequence_invalid")
        events_path.write_bytes(
            original.replace(b'"tool_call_count": 0', b'"tool_call_count": true')
        )
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("provider_usage_invalid")

    def test_closed_teardown_rejects_negative_or_reversed_time(self) -> None:
        self.fixture.update(
            "fixture/evidence/teardown.json",
            lambda value: value.update({"duration_seconds": -1}),
        )
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("teardown_duration_invalid")

    def test_terminal_and_teardown_bind_exact_roots_and_schema_labels(self) -> None:
        self.fixture.update(
            "fixture/evidence/terminal-receipt.json",
            lambda value: value.update({"schema": "forged-terminal"}),
        )
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("terminal_receipt_drift")

    def test_stale_launch_root_and_forged_event_schema_fail(self) -> None:
        self.fixture.update(
            "fixture/evidence/launch.json",
            lambda value: value.update({"configuration_root": digest(b"stale")}),
        )
        self.fixture.refresh_capture_bindings()
        self.assertBlocked("launch_binding_invalid")

    def test_cli_self_check_targets_current_bundle_and_qualifier(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(QUALIFIER),
                "--bundle",
                str(self.root),
            ],
            check=False,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        receipt = parse_json(result.stdout, "cli_receipt")
        self.assertEqual(receipt["status"], "qualified_hold")


if __name__ == "__main__":
    unittest.main()
