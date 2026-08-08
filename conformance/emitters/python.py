#!/usr/bin/env python3
"""Emit signed current objects from a clean-room Python implementation.

The second independent emitter. `javascript.mjs` was the first, and one
independent implementation proves the specification is followable; two prove it
is followable *the same way*. Both are held to the same fixtures by
`verify_current_objects.py`, so a divergence between them is a defect in the
specification rather than a preference.

The two differ in exactly one interesting place, and it is the one worth having
a second implementation for. `javascript.mjs` hand-rolls canonicalization and
sorts object keys with `Object.keys().sort()`, which orders by UTF-16 code
unit. This file calls `rfc8785`, which orders by code point, as JCS specifies.
Every key in every current object is ASCII, where the two orders coincide — so
the fixtures agree today, and would not agree on a key outside the Basic
Multilingual Plane. `jcs-shadow-audit.v1.json` records that seam. Two emitters
that made the same shortcut would have hidden it.

Ed25519 comes from the standard library's `hashlib` and a small pure-Python
implementation would be the wrong trade here; `cryptography` is used instead
and declared alongside the other conformance dependencies.

Usage:
  python conformance/emitters/python.py submission --draft <json> --seed-file <path> --output <json>
  python conformance/emitters/python.py verification --draft <json> --seed-file <path> --output <json>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import stat
import sys
from pathlib import Path

import rfc8785
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

MAX_DRAFT_BYTES = 8 * 1024 * 1024


def canonical(value: object) -> bytes:
    """RFC 8785 JCS. The one function every root in the protocol rests on."""
    return rfc8785.dumps(value)


def sha256_hex(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_draft(path: Path) -> dict:
    payload = path.read_bytes()
    if len(payload) > MAX_DRAFT_BYTES:
        raise SystemExit(f"{path.name} exceeds 8 MiB")
    draft = json.loads(payload)
    if not isinstance(draft, dict):
        raise SystemExit("draft must be a JSON object")
    return draft


def read_seed(path: Path) -> Ed25519PrivateKey:
    """A signing seed is read under the same rules the JavaScript emitter applies.

    Refusing a group- or world-readable seed is not decoration: this file is
    run in CI over a fixture seed, and the habit it teaches is the one an
    implementer will carry to a real one.
    """
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise SystemExit("seed file must be a regular file")
    if info.st_mode & 0o077:
        raise SystemExit("seed file permissions must be 0600 or stricter")
    encoded = path.read_text(encoding="utf-8").strip()
    if len(encoded) != 64 or any(character not in "0123456789abcdef" for character in encoded):
        raise SystemExit("seed file must contain exactly one lowercase 32-byte hex seed")
    return Ed25519PrivateKey.from_private_bytes(bytes.fromhex(encoded))


def public_key_hex(key: Ed25519PrivateKey) -> str:
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

    raw = key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    return raw.hex()


def identity_binding(actor_id: str, created_at: str, key: Ed25519PrivateKey) -> dict:
    """The binding signs its own preimage, with id and signature blank.

    Both fields are outputs of the hash, so they cannot be inputs to it. Every
    signed object here follows the same two-pass shape.
    """
    binding = {
        "schema": "vela.identity_binding.v0.1",
        "binding_id": "",
        "actor_id": actor_id,
        "actor_class": "agent",
        "public_key_hex": public_key_hex(key),
        "created_at": created_at,
        "signature": "",
    }
    preimage = canonical(binding)
    signature = key.sign(preimage)
    key.public_key().verify(signature, preimage)
    return {
        **binding,
        "binding_id": f"vib_{sha256_hex(preimage)[:16]}",
        "signature": signature.hex(),
    }


def sign_object(
    *,
    schema: str,
    id_field: str,
    id_prefix: str,
    draft: dict,
    actor_id: str,
    created_at: str,
    key: Ed25519PrivateKey,
) -> dict:
    binding = identity_binding(actor_id, created_at, key)
    unsigned = {
        "schema": schema,
        id_field: "",
        **draft,
        "authentication": {
            "algorithm": "ed25519",
            "identity_binding": binding,
            "signature": "",
        },
    }
    preimage = canonical(unsigned)
    return {
        **unsigned,
        id_field: f"{id_prefix}_{sha256_hex(preimage)[:16]}",
        "authentication": {
            **unsigned["authentication"],
            "signature": key.sign(preimage).hex(),
        },
    }


def build_submission(draft: dict, key: Ed25519PrivateKey) -> dict:
    provenance = draft.get("provenance") or {}
    producer = provenance.get("producer")
    emitted_at = provenance.get("emitted_at")
    if not isinstance(producer, str) or not producer.startswith("agent:"):
        raise SystemExit("submission provenance.producer must start with agent:")
    if not isinstance(emitted_at, str) or not emitted_at:
        raise SystemExit("submission provenance.emitted_at is required")
    return sign_object(
        schema="vela.submission.v1",
        id_field="submission_id",
        id_prefix="vsb",
        draft=draft,
        actor_id=producer,
        created_at=emitted_at,
        key=key,
    )


def build_verification(draft: dict, key: Ed25519PrivateKey) -> dict:
    verifier = draft.get("verifier")
    started_at = draft.get("started_at")
    if not isinstance(verifier, str) or not verifier:
        raise SystemExit("verification verifier is required")
    if not isinstance(started_at, str) or not started_at:
        raise SystemExit("verification started_at is required")
    return sign_object(
        schema="vela.verification-record.v1",
        id_field="verification_record_id",
        id_prefix="vvr",
        draft=draft,
        actor_id=verifier,
        created_at=started_at,
        key=key,
    )


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("kind", choices=("submission", "verification"))
    parser.add_argument("--draft", required=True, type=Path)
    parser.add_argument("--seed-file", required=True, type=Path, dest="seed_file")
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()

    key = read_seed(options.seed_file.resolve())
    draft = read_draft(options.draft)
    builder = build_submission if options.kind == "submission" else build_verification
    obj = builder(draft, key)

    # The retained bytes are the canonical encoding plus one newline; the root
    # is over the canonical bytes alone. Writing exclusively and read-only
    # matches the JavaScript emitter, so neither can quietly overwrite a
    # fixture it was meant to be compared against.
    payload = canonical(obj)
    with open(options.output, "xb") as handle:
        handle.write(payload + b"\n")
    options.output.chmod(0o444)

    identifier = obj["submission_id" if options.kind == "submission" else "verification_record_id"]
    sys.stdout.buffer.write(
        canonical(
            {
                "schema": "vela.reference-emission-result.v1",
                "kind": options.kind,
                "id": identifier,
                "root": f"sha256:{sha256_hex(payload)}",
                "output": str(options.output.resolve()),
            }
        )
        + b"\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
