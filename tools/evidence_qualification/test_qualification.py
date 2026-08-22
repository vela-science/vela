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
    QUALIFIER,
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

    def _oci(self) -> tuple[bytes, str, str]:
        config_raw = b'{"architecture":"arm64","os":"linux"}\n'
        config_digest = digest(config_raw)
        manifest = {
            "schemaVersion": 2,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": config_digest,
                "size": len(config_raw),
            },
            "layers": [],
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
        files = {
            "index.json": canonical_json_bytes(index),
            "oci-layout": b'{"imageLayoutVersion":"1.0.0"}\n',
            "blobs/sha256/" + manifest_digest.removeprefix("sha256:"): manifest_raw,
            "blobs/sha256/" + config_digest.removeprefix("sha256:"): config_raw,
        }
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w") as archive:
            for name, raw in sorted(files.items()):
                info = tarfile.TarInfo(name)
                info.size = len(raw)
                info.mtime = 1_757_289_600
                info.mode = 0o644
                archive.addfile(info, io.BytesIO(raw))
        return buffer.getvalue(), manifest_digest, config_digest

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
        oci_raw, image_digest, config_digest = self._oci()
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
                    "oci_tar_bytes": digest(oci_raw),
                },
            )
        strict_arguments = [
            "approval_policy=never",
            "web_search=disabled",
            "tools=none",
        ]
        configuration = {
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
            },
        )
        participant_permit = {
            "schema": "neutral-launch-permit.v1",
            "run_id": "participant-01",
            "status": "held",
            "expires_at": "not_authorized",
            "attempt": 1,
            "image_digest": image_digest,
        }
        self.write_json(
            "permit/hold-state.json",
            {"schema": "neutral-hold.v1", "status": "hold", "reason": "qualification"},
        )
        self.write_json("permit/participant-01.permit.json", participant_permit)
        fixture_template = dict(participant_permit)
        fixture_template["run_id"] = "neutral-qualification-01"
        self.write_json(
            "fixture/permit/neutral-qualification-01.permit.template.json",
            fixture_template,
        )
        fixture_consumed = dict(fixture_template)
        fixture_consumed["status"] = "authorized"
        fixture_consumed["expires_at"] = "2027-01-01T00:00:00Z"
        consumed_path = self.write_json(
            "fixture/permit/neutral-qualification-01.permit.consumed.json",
            fixture_consumed,
        )
        launch_path = self.write_json(
            "fixture/evidence/launch.json",
            {
                "schema": "neutral-launch.v1",
                "run_id": "neutral-qualification-01",
                "permit_bytes": digest(consumed_path.read_bytes()),
            },
        )
        response_path = self.write_json(
            "fixture/evidence/response.raw.json", self.response
        )
        response_text = response_path.read_text().strip()
        events = (
            json.dumps({"type": "thread.started", "thread_id": "neutral"})
            + "\n"
            + json.dumps({"type": "turn.started"})
            + "\n"
            + json.dumps(
                {
                    "type": "item.completed",
                    "item": {"type": "agent_message", "text": response_text},
                }
            )
            + "\n"
            + json.dumps(
                {
                    "type": "turn.completed",
                    "usage": {
                        "input_tokens": 900_000,
                        "cached_input_tokens": 899_000,
                        "output_tokens": 100,
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
                "schema": "neutral-teardown.v1",
                "process_reaped": True,
                "network_disabled": True,
                "mounts_detached": True,
                "completed_at": "2026-08-21T00:00:01Z",
            },
        )
        canonical_response = normalize_closed_set(
            self.response, "items", "id", ["a", "b", "c"]
        )
        terminal_path = self.write_json(
            "fixture/evidence/terminal-receipt.json",
            {
                "schema": "neutral-terminal-receipt.v1",
                "status": "completed",
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
                    "fixed_day": 20339,
                    "fixtures": [
                        shadow_a.relative_to(self.root).as_posix(),
                        shadow_b.relative_to(self.root).as_posix(),
                    ],
                    "normalized_sha256": digest(normalized),
                },
            },
            "participant_permit": {
                "hold": "permit/hold-state.json",
                "permit": "permit/participant-01.permit.json",
                "consumed_permit": "permit/participant-01.permit.consumed.json",
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
            },
            "scoring_snapshot": "fixture/scoring-snapshot.json",
            "self_verification": {
                "command": [
                    str(Path(sys.executable).resolve()),
                    str(QUALIFIER),
                    "--bundle",
                    str(self.root.resolve()),
                ],
                "qualifier_sha256": digest(QUALIFIER.read_bytes()),
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
        (directory / "race-01.permit.json").write_text("{}\n")
        outcomes = []
        lock = threading.Lock()

        def attempt() -> None:
            try:
                consume_permit(directory, "race-01")
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
        self.assertBlocked("configuration_compatibility_image_drift")

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

    def test_cli_self_check_targets_current_bundle_and_qualifier(self) -> None:
        result = subprocess.run(
            [
                str(Path(sys.executable).resolve()),
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
