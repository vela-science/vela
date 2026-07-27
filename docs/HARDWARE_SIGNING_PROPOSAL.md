# Hardware touch-to-sign

Status: historical design study. The current authority path uses the operating
system principal, restricted Cedar authorization, and a standard OpenSSH agent
repository-authority key. There is no current `vela sign` writer. This document
is retained only as design history and must not be used as current setup
guidance.

## The problem it closes

The signing key is a 32-byte Ed25519 seed on disk. `generate_keypair` now writes
it `0600` (owner-only), which stops a stray backup or shared machine from leaking
it, but a fully compromised workstation can still read the file and forge a
signature with no human present. The only defense against that is a key that
never leaves hardware and a signature that cannot be produced without a physical
touch.

## Two paths, and which one to build first

### Primary: PKCS#11 / OpenPGP-card Ed25519 (raw Ed25519, zero verifier change)

A YubiKey 5 (PIV or OpenPGP mode), a Nitrokey, or any PKCS#11 token can hold a
raw Ed25519 key and sign arbitrary bytes on a touch. Because the token signs the
**exact event preimage bytes** (`event_signing_bytes` → `signing_input`), the
output is an ordinary Ed25519 signature over the same content-addressed state the
file key would have signed. It drops straight into the existing `signature`
field. **Verification is completely unchanged** — a mirror, a second reducer, an
offline clone all verify it with the registered public key exactly as today.

This is the path to build first: it adds a signer at the edge, not a branch in
the trust path.

Shape:

- A `Signer` trait with two implementations — `FileSigner` (today's behavior) and
  `Pkcs11Signer { module, slot, key_id }` — both returning a raw 64-byte Ed25519
  signature over the preimage.
- `resolve_signing_key*` (`config/cli_identity.rs`) returns a `Box<dyn Signer>`
  instead of a bare `SigningKey`; the one key read in `sign_session.rs` becomes
  one `signer.sign(preimage)` call, which prompts the touch.
- Identity records the token: `identity.json` gains an optional
  `signer: { kind: "pkcs11", module, slot, key_id }`; absent = file key.
- Enrollment binds the token's public key with the normal `vela id` /
  `actor.registered` flow — the pubkey is a pubkey regardless of where the
  private half lives.

Verifier delta: **none.** The signature is a pure Ed25519 signature; the id
preimage is unchanged; conformance fixtures are unchanged.

### Stronger, heavier: FIDO2 `ed25519-sk` (a second envelope)

A resident FIDO2 credential (Apple secure-intent, Yubico FIDO2) gives phishing-
resistant, per-credential-counter, secure-element assurance. But a FIDO2
assertion signs over `sha256(clientDataHash ‖ authenticatorData)`, **not** the
raw message, so it is not a pure Ed25519 signature over the event preimage and
cannot reuse the `signature` field. It needs the second envelope from
`THREAT_MODEL.md`:

```jsonc
"signature_sk": {
  "alg": "ed25519-sk",
  "credential_id": "...",
  "auth_data_b64": "...",
  "client_data": { "challenge": "<event content preimage sha256>", "origin": "vela:sign" },
  "signature_hex": "..."
}
```

Verified by reconstructing the FIDO2 signing input from the stored `auth_data`
plus the **re-derived** event preimage hash (so the signed claim is still
content-addressed state, never agent-supplied bytes), and checking the credential
counter is monotonic (anti-clone). Events carry EITHER `signature` (pure) OR
`signature_sk`; `verify_event_signature` dispatches on presence; the id preimage
excludes both. This is real wire surface in the most safety-critical code, so it
is gated behind the primary path landing and its own dual-verify fixture.

## Recommendation

Build the PKCS#11 raw-Ed25519 path. It delivers the whole security win — key
never leaves hardware, signature needs a physical touch — with no change to
verification, replay, or conformance. Add `ed25519-sk` only if a specific
authority requires FIDO2-class attestation, and treat it as a trust-path change
with its own negative fixtures, not a convenience.

## Enrollment runbook (PKCS#11 path, once implemented)

```bash
# 1. Generate the key ON the token (it never exists off-device).
#    YubiKey PIV example (slot 9c = digital signature):
ykman piv keys generate --algorithm ED25519 9c /tmp/pub.pem
ykman piv certificates generate --subject "CN=vela" 9c /tmp/pub.pem

# 2. Point your Vela identity at the token instead of a file.
vela id import --pkcs11 --module /usr/lib/opensc-pkcs11.so --slot 0 --key-id 9c

# 3. Register the token's public key on the frontiers you steward
#    (same actor.registered flow; the pubkey is just a pubkey).
vela id show          # confirm actor id + pubkey now resolve to the token

# 4. Sign as usual; each acceptance now requires a touch.
vela sign             # prompts: "touch your key to sign N item(s)"
```

Until this lands, keep the file key `0600`, keep it out of any agent-writable
sandbox, and rely on the binary pin + the single-confirm ceremony. `vela sign
--sk` will keep refusing and naming this document.
