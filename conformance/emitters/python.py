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
  python conformance/emitters/python.py <submission|verification> \\
    --draft <json> --seed-file <path> --actor <id> --actor-class <human|agent|org> \\
    --declared-at <rfc3339> --output <json>
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import stat
import sys
from pathlib import Path

import rfc8785
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

MAX_DRAFT_BYTES = 8 * 1024 * 1024

# schema tag, DSSE payload type, and handle prefix, per object kind.
KINDS = {
    "submission": (
        "vela.submission.v3",
        "application/vnd.vela.submission.v3+json",
        "vsb",
    ),
    "verification": (
        "vela.verification-record.v2",
        "application/vnd.vela.verification-record.v2+json",
        "vvr",
    ),
}


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


def pae(payload_type: str, payload: bytes) -> bytes:
    """DSSE Pre-Authentication Encoding.

    `DSSEv1 SP LEN(payloadType) SP payloadType SP LEN(payload) SP payload`.
    The signature covers this and only this, so a payload cannot be lifted into
    an envelope claiming a different type.
    """
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


def build(kind: str, draft: dict, key: Ed25519PrivateKey, options: argparse.Namespace) -> dict:
    """Seal a draft into its DSSE envelope.

    A draft may not supply `schema` or `identity`: both are the emitter's, and
    the spread below would let a draft carrying either be signed under a type
    or an actor its caller never asked for. `javascript.mjs` refuses
    identically.
    """
    supplied = [field for field in ("schema", "identity") if field in draft]
    if supplied:
        raise SystemExit(
            f"draft supplies {', '.join(supplied)}, which the emitter produces. "
            "Pass a draft, not a signed object."
        )
    schema, payload_type, _ = KINDS[kind]
    if kind == "submission" and (draft.get("provenance") or {}).get("producer") != options.actor:
        raise SystemExit("submission provenance.producer must be the declared signer")

    obj = {
        "schema": schema,
        "identity": {
            "schema": "vela.signer-identity.v1",
            "actor_id": options.actor,
            "actor_class": options.actor_class,
            "public_key_hex": public_key_hex(key),
            "declared_at": options.declared_at,
        },
        **draft,
    }
    payload = canonical(obj)
    signature = key.sign(pae(payload_type, payload))
    key.public_key().verify(signature, pae(payload_type, payload))
    return {
        "payloadType": payload_type,
        "payload": base64.b64encode(payload).decode("ascii"),
        "signatures": [
            {
                "keyid": public_key_hex(key),
                "sig": base64.b64encode(signature).decode("ascii"),
            }
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("kind", choices=tuple(KINDS))
    parser.add_argument("--draft", required=True, type=Path)
    parser.add_argument("--seed-file", required=True, type=Path, dest="seed_file")
    parser.add_argument("--actor", required=True)
    parser.add_argument(
        "--actor-class", required=True, choices=("human", "agent", "org"), dest="actor_class"
    )
    parser.add_argument("--declared-at", required=True, dest="declared_at")
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()

    # Not `.resolve()`: resolving follows a symlink and then lstat sees the
    # target, so the symlink check below would never fire. The JavaScript
    # emitter resolves and then lstats too — and rejects symlinks — so this
    # keeps the two saying the same thing.
    key = read_seed(options.seed_file)
    envelope = build(options.kind, read_draft(options.draft), key, options)

    # The retained bytes are the canonical envelope exactly, with no trailing
    # newline: the published root is over the file a reader is handed. Writing
    # exclusively and read-only matches the JavaScript emitter, so neither can
    # quietly overwrite a fixture it was meant to be compared against.
    payload = canonical(envelope)
    with open(options.output, "xb") as handle:
        handle.write(payload)
    options.output.chmod(0o444)

    root = f"sha256:{sha256_hex(payload)}"
    sys.stdout.buffer.write(
        canonical(
            {
                "schema": "vela.reference-emission-result.v1",
                "kind": options.kind,
                "id": f"{KINDS[options.kind][2]}_{root[len('sha256:'):][:16]}",
                "root": root,
                "output": str(options.output.resolve()),
            }
        )
        + b"\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
