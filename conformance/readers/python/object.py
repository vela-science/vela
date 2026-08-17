#!/usr/bin/env python3
"""Verify one current Vela producer or verifier object without Vela or Rust."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path

from canonical import canonical_bytes
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
from jsonschema import Draft202012Validator, FormatChecker

KINDS = {
    "application/vnd.vela.submission.v3+json": (
        "submission",
        "vela.submission.v3",
        "submission.schema.json",
        "vsb",
    ),
    "application/vnd.vela.verification-record.v2+json": (
        "verification",
        "vela.verification-record.v2",
        "verification-record.schema.json",
        "vvr",
    ),
}


def decode_base64(field: str, value: object) -> bytes:
    if not isinstance(value, str):
        raise TypeError(f"{field} must be base64 text")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise ValueError(f"{field} is not canonical base64") from error
    if base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError(f"{field} is not canonical base64")
    return decoded


def pae(payload_type: str, payload: bytes) -> bytes:
    encoded_type = payload_type.encode("utf-8")
    return b" ".join(
        [
            b"DSSEv1",
            str(len(encoded_type)).encode("ascii"),
            encoded_type,
            str(len(payload)).encode("ascii"),
            payload,
        ]
    )


def load_object(path: Path) -> tuple[bytes, dict]:
    raw = path.read_bytes()
    if len(raw) > 8 * 1024 * 1024:
        raise ValueError("object exceeds 8 MiB")
    value = json.loads(raw)
    if not isinstance(value, dict) or canonical_bytes(value) != raw:
        raise ValueError("envelope is not canonical RFC 8785 JSON")
    return raw, value


def verify(path: Path, schema_dir: Path) -> dict[str, str]:
    raw, envelope = load_object(path)
    payload_type = envelope.get("payloadType")
    if payload_type not in KINDS:
        raise ValueError(f"unsupported payload type: {payload_type!r}")
    kind, schema_tag, schema_file, prefix = KINDS[payload_type]

    envelope_schema = json.loads((schema_dir / "dsse-envelope.schema.json").read_text())
    Draft202012Validator(envelope_schema).validate(envelope)

    signatures = envelope.get("signatures")
    if not isinstance(signatures, list) or len(signatures) != 1:
        raise ValueError("current objects require exactly one DSSE signature")
    signature = signatures[0]
    payload = decode_base64("payload", envelope.get("payload"))
    record = json.loads(payload)
    if not isinstance(record, dict) or canonical_bytes(record) != payload:
        raise ValueError("payload is not canonical RFC 8785 JSON")
    if record.get("schema") != schema_tag:
        raise ValueError(f"payload schema is not {schema_tag}")

    schema = json.loads((schema_dir / schema_file).read_text())
    Draft202012Validator(schema, format_checker=FormatChecker()).validate(record)

    identity = record.get("identity")
    if not isinstance(identity, dict):
        raise TypeError("payload has no signer identity")
    public_hex = identity.get("public_key_hex")
    if not isinstance(public_hex, str) or len(public_hex) != 64:
        raise ValueError("identity public key is not 32-byte lowercase hex")
    public_bytes = bytes.fromhex(public_hex)
    if signature.get("keyid") != public_hex:
        raise ValueError("DSSE keyid does not match the payload identity")
    signature_bytes = decode_base64("signature", signature.get("sig"))
    try:
        Ed25519PublicKey.from_public_bytes(public_bytes).verify(
            signature_bytes, pae(payload_type, payload)
        )
    except InvalidSignature as error:
        raise ValueError("Ed25519 signature did not verify") from error

    root = "sha256:" + hashlib.sha256(raw).hexdigest()
    return {
        "schema": "vela.reference-read-result.v1",
        "kind": kind,
        "id": f"{prefix}_{root[len('sha256:'):][:16]}",
        "root": root,
        "payload_schema": schema_tag,
        "signer": identity["actor_id"],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("object", type=Path)
    parser.add_argument(
        "--schema-dir",
        type=Path,
        default=Path(__file__).resolve().parents[3] / "schemas",
    )
    arguments = parser.parse_args()
    result = verify(arguments.object, arguments.schema_dir)
    print(canonical_bytes(result).decode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
