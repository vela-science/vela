# ADR 0011: Protected decision sessions and producer withdrawal

- Status: Proposed
- Target release: Vela `v0.901.0`
- Protocol effect: one signed, non-scientific `proposal.withdrawn` event
- Local product effect: a portable one-shot signer-helper contract and identity v2
- Authority effect: none
- Last research review: 2026-07-17

## Context

Vela's ordinary human decision path still resembles a batch signing utility.
`vela sign` can resume saved answers for a queue, accepts key paths, and asks a
terminal user to translate a rendered decision into command flags. This is
useful for historical replay and detached files, but it is a poor everyday
approval surface. It also gives an agent no safe way to prepare and request one
decision while leaving the final authority act to the human.

The original 0.901 design made macOS LocalAuthentication and Keychain retrieval
the decision mechanism. That conflated four different concerns:

1. **authentication**: is the current operator allowed to use a local signer?
2. **key custody**: where is an Ed25519 secret stored and used?
3. **transaction authorization**: did the operator approve this exact proposal,
   action, reason, and Decision Plan?
4. **scientific authority**: is the signer registered and authorized by the
   frontier's existing rules?

It also would have required Touch ID or a passcode for every decision and would
not have worked coherently on Windows or Linux. Authentication can be cached in
a bounded session. Transaction authorization cannot be inferred from that
session: the human must still see and approve the exact decision.

The current Erdős queue makes the distinction concrete. Proposal
`vpr_f54338a5a453c1bf` preserves a valid bounded producer result, but its exact
Decision Brief reports no independent verifier attachments and no surviving
adversarial probe. Acceptance is blocked by `engine_gate_blocked`; rejection is
available. An untracked legacy sign-session file contains answers for two other
proposals. A targeted decision must ignore that file completely.

Producer lifecycle has a separate gap. Receipt v1 already embeds a self-signed
agent identity binding. After landing, however, the producer cannot close its
own still-pending proposal when the run is abandoned or superseded. Human
review is therefore burdened with producer-owned queue hygiene.

## Research basis

The decision is based on primary standards and current product behavior:

- NIST SP 800-63B-4 says session management is preferable to continual
  credential presentation because repeated prompts encourage workarounds. A
  session needs its own secret, inactivity and overall expiry, logout, and
  reauthentication behavior.[^nist-session]
- OWASP's transaction-authorization guidance requires What You See Is What You
  Sign, a final authorization gate, limited lifetime, and authorization material
  unique to one operation.[^owasp-transaction]
- Codex app-server approval requests carry the exact command or file change,
  bind the response to the active item/turn, and distinguish one-shot from
  session-scoped approval.[^codex-approvals]
- RFC 9987 standardizes Ed25519-capable SSH agents and per-key lifetime and
  confirmation constraints. It also warns that the agent protocol has no
  authentication or transport security of its own and that socket access can
  steal use of an unconstrained key.[^rfc9987]
- 1Password's cross-platform SSH agent keeps keys inside the application,
  authorizes clients, and remembers grants until lock, quit, or a configured
  timeout. Its prompt authorizes key use, not Vela scientific semantics.[^1p]
- Apple's data-protection Keychain requires code-signing entitlements and an
  app-like bundle; a raw CLI does not get that isolation. Secure Enclave keys
  cannot import Vela's existing Ed25519 seed.[^apple-keychain]
- Windows Hello keeps its application credential private and requires a user
  gesture, but Microsoft's documented application key is RSA. DPAPI protects
  arbitrary bytes for a Windows user/device but does not prove approval of an
  exact Vela decision.[^windows]
- Linux Secret Service supports locked collections and service-owned prompts.
  Polkit warns that cached `*_KEEP` authorization can succeed when request
  variables differ, so it cannot stand in for a decision-specific gate.[^linux]
- WebAuthn distinguishes user presence from user verification and scopes newly
  created credentials to a relying party. Replacing an existing Vela Ed25519
  identity with a passkey would require a separate protocol migration.[^webauthn]
- Sigstore keyless signing depends on OIDC, short-lived certificates, and a
  transparency service. That is a different online trust model, not a local
  custody improvement for 0.901.[^sigstore]

These sources support a familiar product pattern: authenticate a bounded local
session, show the exact consequential action, require one approve/decline
interaction for that action, bind the authorization to immutable input, and
verify again at the execution edge.

## Decision

Vela adds a local one-shot `vela-signer` helper, a two-phase `review decide`
flow, and one producer-withdrawal event. Only withdrawal changes frontier wire
semantics.

### Routine automation remains policy, not repeated approval

Existing signed `Permit` policy remains the no-prompt path for mechanically
bounded work. A human authorizes a closed verifier and claim class once;
matching Receipts can land without a human key. The signer helper is for
`Defer`, policy administration, recovery, and other exceptional authority
operations. Vela does not make humans click through routine producer work.

### Portable signer helper

The Vela CLI never reads a protected human seed after enrollment. For each
decision it starts the pinned `vela-signer` executable, sends one closed
`vela.signer-request.v1` over an inherited input pipe, reads one closed
`vela.signer-response.v1` from an inherited output pipe, and waits for the
helper to exit. There is no listener, daemon, forwarded socket, hosted service,
or persistent signing API. These are local product contracts, not frontier
objects.

The request binds:

```text
schema
nonce
expires_at
vela_binary_path
vela_binary_sha256
helper_sha256
frontier_id
frontier_path
proposal_id
proposal_root
action
reason
reviewer_actor
reviewer_public_key
observed_at
decision_plan_root
gate_state
provider
protection_grade
protection_mode
events
```

The response binds:

```text
schema
request_root
reviewer_public_key
helper_version
helper_sha256
provider
protection_grade
provider_session
approved_at
protection_mode
signatures
```

The helper accepts one request, requires its nonce and expiry to be valid,
reconstructs the canonical event signing inputs from the structured unsigned
events, and verifies that the displayed proposal, action, reason, actor, and
Decision Plan root match those event cores. It signs those inputs only. The CLI
verifies the helper digest, response root, public key, and every returned
signature, rechecks the transaction read set, and writes only if all state is
unchanged.

Anonymous inherited pipes avoid a reusable local endpoint. The helper rejects
terminal arguments, environment-carried requests, additional requests, and
unknown fields. It disables core dumps/debug attachment where the platform
permits, zeroizes exported seeds before exit, and never logs secret or signing
input bytes.

### Provider sessions and exact decision cards

The default `session` mode uses the platform authenticator on first use, then
records a local signer session with a 15-minute inactivity limit and a one-hour
overall limit. The record is non-authoritative, contains no factor or secret,
is mode `0600` where supported, and binds the actor, public key, provider,
protection mode, and helper digest. The protected identity signs the record,
so local edits invalidate it. It cannot approve a decision or produce a
signature. Expiry or binding drift requires platform reauthentication. The underlying OS store
still controls key release, so locking or invalidating that provider makes the
next key read fail or reauthenticate according to the provider.

This short receipt gives Vela deterministic prompt timing without inventing a
password or retaining biometric/passcode material. Deleting or corrupting it
only forces reauthentication.

Every deferred proposal still receives a separate decision card. It shows:

- accept or reject;
- full proposal ID and proposal root;
- frontier ID;
- exact reason;
- Decision Plan root;
- Engine/gate standing;
- custody provider, protection grade, and provider-session state.

The card has explicit Approve and Decline actions. Approval is unique to one
nonce and expires with the request. Changing the proposal, action, reason,
root, event core, or observation time invalidates it. This is the human
authority act; merely authenticating a signer session is not.

An optional `always` mode asks the platform authenticator or a confirming
external agent to reauthenticate for every decision-signing operation after the decision card.
It is required when the requester can control the desktop UI, or by local
policy. There is no Vela "approve for session" or "approve all decisions"
option: the authentication session never authorizes proposal semantics.

### Cross-platform custody profiles

Identity v2 uses one portable helper backend:

```text
helper {
  provider
  key_id
  public_key
  protection_grade
  mode
  pending_source_removal
}
```

Identity v1 and `file` identities remain readable for historical and agent use.
The following providers are product profiles, not different frontier protocols:

| Profile | At-rest custody | Authentication | Important limit |
| --- | --- | --- | --- |
| macOS | login Keychain through the one-shot helper | LocalAuthentication | raw CLI storage is user-session protection, not app-isolated data-protection Keychain |
| Windows | current-user DPAPI/Credential Manager in the signed helper | Windows Hello/user consent | DPAPI alone is user-scoped storage, not transaction consent |
| Linux | Secret Service in the login session | Secret Service prompt plus non-cached polkit reauth | service implementations and process isolation vary |
| RFC 9987 agent (future profile) | Ed25519 key remains in the external agent | provider policy or confirmation constraint | socket access can steal key use; 0.901 does not ship this profile |

Vela reports one of these protection grades instead of pretending the profiles
are equivalent:

```text
file
user_session
app_isolated
external_confirmed
hardware_nonexportable
```

`session` mode requires at least `user_session`. `always` mode requires an
actual per-use platform or agent confirmation and reports the achieved grade.
Hardware-nonexportable Ed25519 providers may be added behind the helper
contract. Changing the registered algorithm or public key requires a separate
ADR and governed identity migration.

Enrollment authenticates before reading the source key, verifies owner and
permissions, checks that the seed derives the registered public key, stores and
reads it back through the chosen provider, atomically installs identity v2, and
only then deletes the source. A local journal makes interruption resumable.
Protected decisions remain disabled while source removal is pending. Success is
never reported while a plaintext source key remains.

### Exact one-proposal decision

```bash
vela review decide <frontier> <vpr_id> \
  (--accept | --reject) --reason <text> \
  [--confirm-root <sha256:...> --confirm-at <RFC3339>] [--json]
```

The first phase reads no key and writes nothing. It returns
`vela.review-decision.v1` containing one exact Decision Brief, action, reason,
reviewer, observation time, and Decision Plan root.

The second phase requires the matching root and time. Before contacting the
helper, Vela checks confirmation freshness, action eligibility, reviewer
registration, policy and Engine inputs, binary identity, and the complete
transaction read set. The protected path has no `--key`, `--yes`, wildcard,
batch, saved-answer, or persistent-approval input. Cancellation or timeout
produces no event, proposal change, journal marker, or Git commit.

Success emits the existing signed `review.accepted` or `review.rejected` event
through the recoverable frontier transaction and exact Git publication path.
Agent/model provenance may identify the requester or co-author; it never
changes the registered human signer or reviewer. `vela sign` remains under
advanced help for historical batch sessions and detached bytes.

### Producer withdrawal primitive

Vela adds:

```text
kind: proposal.withdrawn
payload schema: vela.proposal-withdrawal.v1
```

The closed payload is:

```text
proposal_id
proposal_root
receipt_root
identity_binding_id
```

The event uses `vela.event.v0.1`, an `agent` actor, null before/after scientific
roots, a mandatory reason, and the ordinary Ed25519 event signature.

```bash
vela review withdraw <frontier> <vpr_id> \
  --as <agent_id> --reason <text> [--json]
```

Withdrawal is valid only when the full proposal resolves exactly once, remains
`pending_review`, has the exact proposal root, and binds a valid Receipt v1.
The Receipt root and embedded self-signed identity binding must match the
proposal actor, supplied agent ID, public key, and withdrawal signature.

A valid event projects the proposal to `withdrawn`. It deletes nothing and
changes no finding, accepted event, or scientific root. A decided proposal
cannot be withdrawn; a withdrawn proposal cannot later be decided. Decision
and withdrawal share the frontier transaction barrier, so exactly one
concurrent operation wins. Repeating the exact valid withdrawal is idempotent.

Invalid withdrawal bytes never grant terminal standing. Strict verification
blocks on a missing or altered Receipt, wrong root/actor/key, invalid binding,
ambiguous ID, invalid event ID/signature, duplicate conflict, or illegal state
transition. Non-strict verification reports the signal and leaves the proposal
pending. Legacy proposals without a Receipt replay unchanged but cannot be
withdrawn by a producer.

## Alternatives rejected

### Let an agent use the human key

This removes the authority boundary instead of improving it. A model, worker,
browser, MCP server, or background process must never receive a human seed or
approve its own request.

### Require a biometric or passcode for every ordinary decision

This confuses authentication with transaction authorization, creates prompt
fatigue, and is not portable. The exact decision card is mandatory; repeated
platform authentication is an optional or policy-required `always` mode.

### Unlock once and allow blanket session signing

An authenticated session does not prove approval of later proposal semantics.
The helper never exposes a generic signing endpoint and never offers an
"approve all decisions" grant.

### Treat Keychain, DPAPI, Secret Service, polkit, or SSH-agent approval as the decision

These mechanisms protect storage, authenticate a user/session, or authorize key
use. Their prompts do not reliably show the full Vela transaction. They may
support custody and high-assurance confirmation, but the exact Vela decision
card and final state recheck remain mandatory.

### Replace the identity with Secure Enclave, Windows Hello, a passkey, or Sigstore

These are valuable future profiles, but they change algorithms, public keys,
or trust roots. They cannot silently preserve the existing Ed25519 authority.

### Keep batch signing as the ordinary path

Saved answers and queue-wide context are unnecessary inputs for one decision.
Batch and detached compatibility remain under advanced help.

### Delete producer proposals

Deletion erases audit and Receipt evidence. Withdrawal is append-only and
non-scientific.

## Adversarial cases

- An expired or future-dated helper request fails. A response can apply only to
  the exact request root and event IDs; replay against another plan fails, and
  replay after the proposal reaches terminal standing is idempotent or refused
  by the transaction edge.
- Any change to displayed decision data invalidates the request root.
- A forged helper response, wrong helper identity, wrong public key, invalid
  signature, or mismatched request root fails before frontier writes.
- Binary replacement, identity drift, registry drift, proposal drift, gate
  drift, or transaction read-set drift invalidates approval.
- There is no helper socket to steal or forward. The helper accepts exactly one
  request through inherited pipes, and helper paths or custody endpoints are
  never mounted into workers or verifiers.
- Desktop automation can click ordinary UI, so an automation-capable requester
  requires `always` mode. Canopus workers receive neither desktop control nor
  helper IPC.
- OS-store or external-agent failure, lock, or ambiguous protection grade fails
  closed and leaves the file-key identity unchanged during enrollment.
- Source-key deletion failure leaves identity v2 pending and disables protected
  signing; it never reports migration success.
- Backdated or altered withdrawal events receive no special treatment: their
  canonical signature, Receipt binding, roots, and current proposal standing
  must all verify.

## Compatibility and migration

- Identity v1 and file-key replay remain supported.
- Identity v2 and helper contracts are local files, not accepted scientific
  state.
- Existing event, proposal, Receipt, policy, registration, and artifact bytes
  replay unchanged.
- Existing `review.*` signatures and acceptance rules are unchanged.
- `proposal.withdrawn` is the sole 0.901 wire addition; 0.900 readers may reject
  it at the intentional protocol boundary.
- A frontier remains portable through Git because no canonical event refers to
  an OS credential-store identifier or provider session.
- Canopus may retain a proposal-scoped copy of its own producer key after a
  successful land. It is never mounted into a worker or verifier and cannot
  sign a human decision.

## Conformance contract

Signer and decision tests must prove:

- accept/reject roots differ and target, action, reason, time, event core, or
  Decision Plan drift fails;
- malformed or unknown fields, expiry beyond two minutes, response replay
  against another request, wrong binary, wrong helper identity, wrong public
  key, and forged signatures fail closed;
- helper exit, OS lock, provider lock, identity drift, and binary drift prevent
  continued signing according to the declared provider contract;
- session mode requires one decision-card approval but does not prompt for a
  password/biometric on every decision;
- always mode requires both the exact card and a per-use platform/agent check;
- cancellation, timeout, authentication failure, and post-approval state drift
  produce zero frontier writes;
- identity v1 replay, successful v1-to-v2 enrollment, crash recovery, readback,
  public-key mismatch, and plaintext removal;
- the CLI never receives protected seed bytes and rejects an authentic signature
  for the wrong request;
- macOS, Windows, and Linux profiles satisfy the same behavioral contract,
  with live current-platform custody checks at release;
- no batch, wildcard, key path, `--yes`, or sign-session state affects
  `review decide`.

Withdrawal tests must prove valid withdrawal, wrong key/actor, altered
Receipt/binding, full-ID resolution, tampered event, duplicate conflict,
decided refusal, decision race, strict/non-strict behavior, accepted-state
invariance, and old-frontier replay.

The focused gate is:

```bash
cargo test -p vela-protocol proposal_withdrawal
cargo test -p vela-signer
cargo test -p vela-signer signer_contract
cargo test -p vela-cli review_decide
cargo test -p vela-signer protected_signer
cargo test -p vela-cli review_withdraw
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
./scripts/full-conformance.sh --suite core --mode=ci
./scripts/full-conformance.sh --suite frontier --mode=ci
```

The release matrix must compile and test macOS, Windows, and Linux profiles.
Each release bundle contains the exact `vela` and `vela-signer` pair. The Linux
bundle also contains the non-caching polkit policy. The installers verify the
archive checksum before placing either executable.
The deterministic release union runs once at the actual `v0.901.0` boundary.
External Lean, Diderot, live-network, hosted authority, browser/MCP signing, and
unrelated suites remain excluded.

## Acceptance evidence required

Acceptance requires released packages for macOS, Windows, and Linux plus the
current-platform live custody fixture. It also requires the released binary to
reproduce the Erdős baseline at
`48e7944d29dc773a7c5b74950f9092403c9825fa`, ignore the unrelated legacy
sign-session file, and render the exact proposal and Receipt roots. Acceptance
must remain blocked and rejection available. The agent may submit the exact
rejection request, but only the user's decision-card action may authorize it.
Approval must change exactly one proposal; cancellation must change none.
Clean-clone strict replay must agree before this ADR becomes Accepted.

## References

[^nist-session]: [NIST SP 800-63B-4, Session Management](https://pages.nist.gov/800-63-4/sp800-63b/session/)
[^owasp-transaction]: [OWASP Transaction Authorization Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Transaction_Authorization_Cheat_Sheet.html)
[^codex-approvals]: [OpenAI Codex app-server approvals](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md#approvals)
[^rfc9987]: [RFC 9987, Secure Shell Agent Protocol](https://datatracker.ietf.org/doc/html/rfc9987)
[^1p]: [1Password SSH agent](https://www.1password.dev/ssh/agent) and [authorization model](https://www.1password.dev/ssh/get-started#step-6-authorize-the-ssh-request)
[^apple-keychain]: [Apple TN3137: On Mac keychain APIs and implementations](https://developer.apple.com/documentation/Technotes/tn3137-on-mac-keychains) and [Protecting keys with the Secure Enclave](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
[^windows]: [Windows Hello for apps](https://learn.microsoft.com/en-us/windows/apps/develop/security/windows-hello) and [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
[^linux]: [Secret Service API](https://specifications.freedesktop.org/secret-service/latest-single/) and [polkit reference](https://polkit.pages.freedesktop.org/polkit/polkit.8.html)
[^webauthn]: [Web Authentication Level 3](https://www.w3.org/TR/webauthn-3/)
[^sigstore]: [Sigstore signing overview](https://docs.sigstore.dev/cosign/signing/overview/)
