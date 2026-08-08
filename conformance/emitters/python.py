#!/usr/bin/env python3
"""Emit signed current objects from a clean-room Python implementation.

The second independent emitter. `javascript.mjs` was the first, and one
independent implementation proves the specification is followable; two prove it
is followable *the same way*. Both are held to the same fixtures by
`verify_current_objects.py`, so a divergence between them is a defect in the
specification rather than a preference.

The two differ where a shared assumption would hide: independent Ed25519
stacks, independent argument parsing, independent JSON handling. No specific
canonicalization seam is claimed here — RFC 8785 section 3.2.3 mandates UTF-16
code-unit ordering, which is what `rfc8785` implements and what
`Object.keys().sort()` already does, so the two agree on ordering by
construction rather than by luck. A seam worth checking would need a fixture
with a non-BMP key; there is none, and asserting one exists would be worse than
saying so.

Ed25519 comes from `cryptography`, declared alongside the other conformance
dependencies. The JavaScript emitter uses Node's `crypto`, so neither borrows
the other's signing stack.

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
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise SystemExit("seed file must be a regular file, not a symlink")
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
    """Sign a draft. The draft may not supply anything signing produces.

    `{**draft}` is spread after `schema` and the blanked id, so a draft key
    wins — which quietly breaks the invariant the identity binding above states:
    that both are outputs of the hash and cannot be inputs to it. A draft still
    carrying `submission_id` from a previous emission is hashed with that id
    inside the preimage; the emitter exits 0 and prints a result, and the Rust
    verifier rejects the object for a mismatched id and an invalid signature,
    because it recomputes with both fields cleared. The same shape would let a
    draft set `schema` and be signed under a type its caller never asked for.

    A draft that supplies them is a mistake, not an override.
    """
    supplied = [field for field in ("schema", id_field, "authentication") if field in draft]
    if supplied:
        raise SystemExit(
            f"draft supplies {', '.join(supplied)}, which signing produces. Pass a draft, not a signed object."
        )
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

    # Not `.resolve()`: resolving follows a symlink and then lstat sees the
    # target, so the symlink check below would never fire. The JavaScript
    # emitter resolves and then lstats too — and rejects symlinks — so this
    # keeps the two saying the same thing.
    key = read_seed(options.seed_file)
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
