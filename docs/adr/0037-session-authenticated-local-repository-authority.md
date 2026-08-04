# ADR 0037: Session-authenticated local repository authority

- Status: Accepted
- Accepted: 2026-08-04
- Protocol effect: none; authority-record, keyset, policy, Decision, and replay
  bytes are unchanged
- Product effect: repository authority no longer requires an interactive SSH
  confirmation for every signature, and macOS GUI clients rediscover the
  launchd-owned login-session agent when their inherited environment is stale

## Context

Vela used OpenSSH's `ssh-add -c` constraint as a second approval prompt for
every repository-authority signature. That made one exact Decision take tens
of seconds even after the operator had authenticated the operating-system
session, authorized the work in the native agent runner, inspected the current
Inbox packet, supplied the compare-and-swap root, and requested the semantic
action.

The prompt protected key use but did not understand a Vela Decision. It could
not validate the Proposal, reason, policy result, read set, Standing
consequence, or postimage. Treating it as semantic approval duplicated
interaction without strengthening Vela's scientific boundary.

Vela also carried a broad private implementation of the SSH-agent wire
protocol. A dependency audit found that the focused standalone Rust clients
were small, lightly governed projects, while the mature `russh` implementation
would import an async SSH client/server stack for two local messages. Pulling
either boundary into the release was less legible than retaining the minimal
framing Vela actually needs. Implementing cryptography or SSH key encodings in
Vela would be worse still.

## Decision

For the supported local product:

1. The operating-system login session authenticates the human principal.
2. The operator authorizes the action directly or through the native agent
   runner's task or tool boundary.
3. Vela independently prepares one exact Decision, evaluates restricted Cedar,
   validates the semantic consequence, rechecks the current Inbox and read-set
   roots, and constructs the canonical authority record.
4. A dedicated Ed25519 repository service key, loaded once into the standard
   OpenSSH agent for the current OS session, signs only that DSSE PAE payload.
5. Vela locally verifies the returned signature against the active keyset
   before installing or publishing any state.

The unit of authorization remains one exact Decision. The unit of interactive
authentication is the operating-system or native-runner session. Vela adds no
approval token, signer cache, daemon, batch Decision command, wildcard, or
automatic acceptance path.

The local signer composes [RustCrypto SSH](https://github.com/RustCrypto/SSH)'s
maintained `ssh-key` and `ssh-encoding` crates with a bounded Unix-socket
adapter for the standard
[request-identities and sign-request messages](https://datatracker.ietf.org/doc/html/draft-miller-ssh-agent-13#section-4).
The adapter has no cryptographic
implementation, algorithm negotiation, key storage, agent daemon, or general
SSH surface. Frames and identity counts are capped, unsupported identity blobs
are ignored, and malformed, oversized, truncated, ambiguous, certificate, or
non-Ed25519 responses fail closed. Vela retains its provider rules: deferred
agent access until authentication and policy succeed, exact plain-Ed25519 key
matching, the authority-record payload-type allowlist, DSSE
pre-authentication encoding, and local signature verification through
`ed25519-dalek`.

On macOS, an explicitly supplied `SSH_AUTH_SOCK` remains authoritative. When
it is absent, Vela resolves the login-session socket from launchd at signing
time. This lets a long-running GUI process observe the standard agent session
without a Vela daemon, socket file, key cache, or restart. Linux continues to
use the explicit inherited OpenSSH endpoint.

Ed25519 is the closed algorithm profile of `vela.authority-keyset.v1`, not an
eternal protocol assumption. A future algorithm requires a new keyset and
envelope profile with explicit replay rules; the current runtime does not
silently negotiate algorithms.

## Algorithm and custody choice

Vela keeps Ed25519 as the current repository-signature algorithm:

- [FIPS 186-5](https://csrc.nist.gov/pubs/fips/186-5/final) includes EdDSA;
  NIST identifies Edwards curves' simpler implementation and side-channel
  properties relative to traditional curves;
- [RFC 8032](https://www.rfc-editor.org/info/rfc8032/) specifies deterministic
  Ed25519 signing and verification;
- the 32-byte public key and 64-byte signature keep retained authority history
  compact and the signing operation is deterministic;
- OpenSSH, [AWS KMS](https://docs.aws.amazon.com/kms/latest/developerguide/symm-asymm-choose-key-spec.html),
  and [Google Cloud KMS](https://cloud.google.com/kms/docs/algorithms) support
  raw Ed25519 signing; and
- the existing independent replay implementation already verifies the exact
  profile.

P-256/ES256 is the preferred hardware and passkey interoperability option.
Apple [Secure Enclave signing](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
is P-256-only, and [WebAuthn Level 3](https://www.w3.org/TR/webauthn-3/)
recommends ES256 among the broad interoperability set. Hosted Vela may use
those credentials to authenticate a human,
but should not conflate the human credential with the repository service key.
A hosted repository signer may use P-256 only through a new explicit keyset
profile with fixed signature encoding and cross-reader conformance.

ML-DSA is standardized by [FIPS 204](https://csrc.nist.gov/pubs/fips/204/final)
and available in current cloud KMS products. Vela does not make it the v1
default: its larger keys and signatures,
newer implementation ecosystem, and weaker local hardware integration add
cost without improving the present single-operator workflow. A future archival
profile should qualify an ML-DSA co-signature through an explicit v2 keyset and
threshold, retaining Ed25519 during the transition. It must not reinterpret or
re-sign existing history.

RSA, secp256k1, algorithm inference, and provider-selected negotiation are not
current candidates. The keyset names the exact algorithm; the verifier never
guesses from key or signature shape.

## Security boundary

An unconstrained SSH-agent socket can sign arbitrary bytes with every loaded
key. The local provider is therefore appropriate only for trusted processes in
one operator's session, with a dedicated repository key. The socket must not be
forwarded to remote, untrusted, or proposal-supplied code.

Hosted or shared Vela deployments should implement the existing signer
provider boundary with a policy-constrained KMS or workload signer. Sigstore
keyless signing and SPIFFE workload identity are useful distribution and
service-identity systems, but neither supplies Vela's human scientific
Decision. They may authenticate a hosted signer without replacing the signed
Decision record, policy evaluation, or Standing transition.

## Consequences

- Multiple explicitly authorized Decisions complete without redundant
  per-signature popups.
- A macOS GUI process can reconnect to the launchd-owned agent after the key is
  loaded, even when that process started before the shell session.
- A rejected authentication, policy evaluation, stale root, or semantic check
  still touches no signer.
- A signer substitution or malformed signature still fails before state
  installation.
- The selected release graph contains no RSA implementation, agent daemon, or
  Windows transport dependency. Vela's only agent-specific code is the bounded
  framing adapter for two standard messages; key and signature encoding remain
  upstream.
- The repository history and independent replay contract do not change.
- Local convenience does not become a claim of safe untrusted multi-tenant
  operation.
