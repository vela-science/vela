# ADR 0011: Human-governed authority and producer withdrawal

- Status: Proposed
- Release gate: pending protected rebind, current-platform UX review,
  cross-platform package evidence, and final tag publication
- Target release: Vela `v0.901.0`
- Protocol effect: one signed, non-scientific `proposal.withdrawn` event
- Local product effect: protected custody, bounded authority sessions, and identity v2
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

## Internal product direction

This ADR follows the architecture already developed in the Constellate memos
instead of inventing a parallel security ceremony:

- `vela_human_governance_memo.md` gives the governing rule: humans sign
  policies, delegations, exceptions, and irreversible commitments; machines
  sign executions, evidence, and receipts. It also says human governance is an
  authority property rather than a click count and that visible complexity
  should fall as protocol rigor rises.
- `vela_research_harness_architecture_memo.md` keeps models outside the human
  key path and Vela as the sole authority plane. The harness produces evidence
  and proposals; it does not impersonate a reviewer.
- `constellate_vela_product_ux_memo.md` defines a State Transition Card and a
  review inbox modeled on a pull-request workflow. The review object is the
  scientific change and its evidence, not the cryptographic operation.
- `vela_agentic_git_entire_cursor_origin_memo_2026-07-10.md` requires autonomy
  to be consequence-sensitive. Reading and producing evidence are not the same
  authority tier as accepting canonical scientific state.
- `vela-adr-audit.md` warns against expanding the protocol before a blind
  handoff proves the gap and rejects model or browser possession of a human
  signing path.

The product must preserve the authority boundary while moving human attention
from routine execution to policy design and genuine exceptions.

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
- GitHub's signing guidance treats the signature as an automatic Git operation
  once signing is configured. It recommends Keychain, Gpg4win, or an agent so
  people do not re-enter a passphrase for every commit.[^github-signing]
- 1Password authorizes a key for a specific application or terminal session.
  Later operations in that process use the key without another prompt until
  lock, quit, or the configured timeout.[^1p-security]
- Claude Code uses permission modes and allow rules to make bounded tool use
  silent. Tools that explicitly require user interaction cannot be approved by
  a non-interactive permission broker.[^claude-permissions]
- Apple's alert guidance says alerts must be rare, short, and actionable; use
  action-specific button labels rather than Yes/No, reserve warning treatment
  for genuinely unexpected destructive actions, and put advanced material
  behind disclosure controls.[^apple-alerts]
- GitHub deployment review presents the pending deployment in its existing
  workflow context, then offers Approve and deploy or Reject with an optional
  comment. It does not present a cryptographic signing utility.[^github-review]

These sources support a familiar product pattern: authenticate a bounded local
session, show the exact consequential action, require one approve/decline
interaction for that action, bind the authorization to immutable input, and
verify again at the execution edge.

No cited product uses one prompt to solve every layer. Their relevant behavior
is:

| Product or standard | What the user authorizes | Prompt lifetime | What Vela should borrow | What Vela must not infer |
| --- | --- | --- | --- | --- |
| Git/GitHub signing | use of a configured signing identity | cached by Keychain or an agent | signatures are automatic infrastructure | a valid signature means a person reviewed the content |
| 1Password SSH agent | one key for one app/process or terminal session | until lock, quit, or timeout | client-bound key-use sessions | SSH authorization is scientific acceptance |
| Codex | one command/change, a session capability, or a policy amendment | one action, session, or persisted rule | inline approval in the active task and explicit scope | the model may answer its own approval request |
| Claude Code | a tool rule or permission mode | session/configured rule | bounded operations become silent | `bypassPermissions` is appropriate for authority events |
| GitHub deployment review | deploy or reject the displayed environment/job | one contextual review action | semantic verbs and optional rationale | the reviewer needs to see signing internals |
| NIST sessions | continued authenticated use | inactivity and overall timeout | reauthenticate on session boundaries | session possession proves a later semantic decision |
| OWASP transaction authorization | significant data for one consequential operation | short, operation-bound | What You See Is What You Sign for exceptions | every low-risk execution needs a transaction prompt |
| WebAuthn/passkeys | an RP-scoped challenge with user presence/verification | normally per assertion | future direct-human credentials | existing Ed25519 identities migrate without governance |
| Sigstore keyless | workload/person identity for one short-lived signing flow | ephemeral certificate lifetime | producer attribution without long-lived keys | OIDC identity is scientific authority |

The combined conclusion is narrower than the original design. Authentication
opens custody. Client authorization controls which process may ask for a
signature. Signed policy controls routine autonomous scope. A contextual review
action records an exceptional human judgment. The event signature binds the
result. None of these facts substitutes for another.

### 2026-07-17 dogfood result: security pass, product failure

The first live candidate disproved the original UI design. Enrollment moved
`reviewer:will-blair` into OS custody, verified the public key, removed the
plaintext source, and pinned the exact CLI/helper pair. A targeted rejection of
`vpr_f54338a5a453c1bf` then bound the expected proposal, Receipt, reason, and
Decision Plan roots and emitted one valid `review.rejected` event. The CLI did
not read the key and no unrelated proposal changed.

The human-facing surface was nevertheless unacceptable. A generic warning
alert showed implementation terms, full roots, custody metadata, and Yes/No
buttons. The user had to leave the active task, scan a wall of text, and infer
that Yes meant Reject. This is exactly the prompt fatigue and context loss that
the cited products avoid. It also asked a human to reject under-evidenced agent
work that the producer should have retained as run evidence or withdrawn.

This is a release-blocking NO-GO, not a polish issue. The candidate stays
unreleased, this ADR stays Proposed, and the valid historical rejection is not
rewritten. Successful cryptography is insufficient acceptance evidence.

### 2026-07-17 cross-platform release-candidate evidence

Release-candidate run
[`29620812022`](https://github.com/vela-science/vela/actions/runs/29620812022)
passed the signer contracts and release build on macOS ARM64, Linux x86-64,
and Windows x86-64. The downloaded checksum sidecars verified independently.
The archive roots are:

- macOS ARM64: `sha256:e6dd60076310372bd71d3e7455398cb32f394cb466a72ad7bf81af5af8c93f03`;
- Linux x86-64: `sha256:81a69b195e1896a6a9a4bfed5106552171cdda2e9d82920e23cb2f9a87b2a35b`;
- Windows x86-64: `sha256:38f242c07952c1434b8a96ec1176e67dd37ce094906d4265a285124158f460d0`.

Every archive contains the paired `vela` and `vela-signer` binaries; the Linux
archive also contains the scoped polkit policy. The extracted macOS binary
reports `vela 0.901.0`. This proves packaging and platform compilation, not the
remaining current-identity rebind or human-facing decision-card acceptance
gate. An RC publication also exposed a release-workflow defect: a hyphenated
tag was not marked as a prerelease. The workflow now derives `--prerelease`
from the tag before publication; immutable release metadata is not rewritten.

## Decision

Vela makes signing an implementation detail, not a recurring user task. It
adds one producer-withdrawal event and a local protected-custody boundary. Only
withdrawal changes frontier wire semantics.

The ordinary product has three lanes:

1. **Routine agent work.** The agent signs its own Receipt and events. Existing
   signed `Permit` policy may admit a closed verifier and claim class. No human
   signing or approval prompt occurs.
2. **Producer lifecycle.** A producer retains a failed/null run as evidence or
   withdraws its own Receipt-bound pending proposal. A human is not asked to
   perform queue hygiene.
3. **Human scientific authority.** A genuinely deferred accept/reject decision
   appears in the active review context. The person approves the semantic
   consequence, not a signature. Protected signing follows silently inside the
   same operation.

The target prompt budget is therefore zero prompts per signature, zero human
prompts for routine or producer-owned work, one platform authentication when a
bounded signer session opens or expires, and one concise semantic action only
when human scientific judgment is actually required.

### Routine automation remains policy, not repeated approval

Existing signed `Permit` policy remains the no-prompt path for mechanically
bounded work. A human authorizes a closed verifier and claim class once;
matching Receipts can land without a human key. The signer helper is for
`Defer`, policy administration, recovery, and other exceptional authority
operations. Vela does not make humans click through routine producer work.

Canopus must not land a result merely to force a human rejection when the
durable Engine gate already says it is not reviewable. It preserves the run and
verifier evidence without a proposal, or uses its retained producer capability
to withdraw a proposal it already created. Pending review is reserved for work
that is genuinely ready for human judgment.

### Protected custody and the approval edge

The Vela CLI and model never read a protected human seed after enrollment. In
the 0.901 profile, the one-shot custody helper itself owns the final semantic
approval card. Any same-user process may request that card, but only the
person's action on the platform UI can approve it. The helper exits after one
closed request. A raw command, environment variable, model response, session
record, or generic OS-store read is never a decision approval.

This deliberately accepts one concise click for a genuinely human scientific
decision. It does not accept a password or biometric prompt per signature.
The semantic card answers one exceptional decision; authentication opens or
refreshes bounded custody only when needed; the signature then happens
invisibly. A future
in-process Codex, Claude, or review-inbox adapter may replace the helper's card
with its native user-interaction channel, but it must return a fresh
one-operation authorization capability that the model cannot obtain or forge.
That adapter is not required for the 0.901 protocol boundary and must not add
an authority service.

Enrollment has no separate Vela confirmation alert. The explicit `vela id
protect` invocation is the request, and one platform authentication authorizes
the one-time move into protected custody. Reading the source, installing the
protected copy, readback, public-key verification, identity replacement, and
plaintext deletion remain ordered and recoverable. This removes a redundant
prompt without weakening the only factor that protects the migration.

The identity pins the exact signer-helper digest. A package update cannot use
the protected key until the person reruns the same explicit command. With no
plaintext source present, the installed helper authenticates once, proves
possession of the existing Ed25519 key, and signs a local rebind response that
covers the old and new helper digests, old and new protection modes, actor,
public key, old and new Vela binary digests, provider, time, and request root.
Only then may the CLI update the public identity profile and binary pin. Each
file is replaced atomically; partial local completion remains fail-closed and
rerunning the same authenticated command resumes it. An unchanged installation
does not rewrite either pin. Changing `always` to `session` uses this same
ceremony; editing the JSON does not.

The pending enrollment record also binds the exact Vela and helper digests.
If plaintext deletion succeeds but the final identity write is interrupted,
the exact pair may authenticate, prove possession of the protected key, and
finish the pending record. Recovery cannot simultaneously upgrade binaries or
change modes. A pending record without its original source or digest binding
fails with an explicit restore action.

The local rebind request is closed and root-bound:

```text
purpose: upgrade | enrollment_recovery
nonce
expires_at
vela_binary_path
previous_vela_binary_sha256
vela_binary_sha256
previous_helper_sha256
helper_sha256
actor
public_key
provider
previous_protection_mode
protection_mode
```

The response repeats the actor, public key, new helper/provider/mode, request
root, and authorization time and is signed by the existing protected Ed25519
key. `upgrade` requires at least one actual pin or mode change;
`enrollment_recovery` requires all three to remain unchanged. A successful
session-mode enrollment or rebind signs and opens the new bounded signer
session, so the next decision does not immediately repeat platform
authentication.

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
display {
  frontier_name
  claim
  requester
  decisive_facts
  consequence
}
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

The display material is derived from the same Decision Brief, bounded, stripped
of control characters, and covered by the request root. The helper reconstructs
canonical event inputs, signs only those inputs, and returns no secret material.
Vela then rechecks the complete transaction read set before any write. A future
client adapter additionally binds its capability to the request root, action,
proposal, actor, client session, and expiry.

### Provider sessions and semantic review

The default `session` mode uses the platform authenticator after the first
semantic decision approval in an expired session, then
records a local signer session with a 15-minute inactivity limit and a one-hour
overall limit. The record is non-authoritative, contains no factor or secret,
is mode `0600` where supported, and binds the actor, public key, provider,
protection mode, and helper digest. The protected identity signs the record,
so local edits invalidate it. It cannot approve a decision or produce a
signature. Expiry or binding drift requires platform reauthentication. The
underlying OS store still controls key release, so locking or invalidating that
provider makes the next key read fail or reauthenticate according to the
provider.

This short receipt gives Vela deterministic prompt timing without inventing a
password or retaining biometric/passcode material. Deleting or corrupting it
only forces reauthentication.

Cancellation occurs before session creation, refresh, platform authentication,
or key access. An expired session therefore does not make a person authenticate
for a decision they decline.

Every deferred proposal receives a semantic review item. In 0.901, Codex or the
operator may invoke the exact command, but the human acts only on the platform
card. A later review inbox may render the same request. The default card
contains only:

- the plain-language action, using `Accept result` or `Reject proposal`;
- the claim or result being decided;
- the decisive evidence or blocker;
- the consequence for accepted state; and
- the requester/producer.

The card includes short proposal and Decision Plan references for correlation.
Full roots, custody provider, signatures, and event cores remain in the
key-free JSON preview and machine request; a future review inbox places them
under one `Technical details` disclosure. They are fully machine-bound but are
not the primary human language. Buttons name the result and include `Cancel`;
the UI never uses Yes/No. Warning treatment is reserved for an unexpected
irreversible consequence, not ordinary review.

The item remains unique to one nonce and request root. Changing the proposal,
action, reason, root, event core, or observation time invalidates it. The 0.901
helper returns the signed response only after the exact card action. A future
trusted client returns a one-operation approval capability. Merely
authenticating a signer session is never approval.

An optional `always` mode asks the platform authenticator or a confirming
external agent to reauthenticate for every authority operation. The default
does not. Authentication and client authorization are session-scoped;
scientific semantics remain policy-scoped or operation-scoped. A client may
offer a session grant only for an already closed non-scientific capability.
It may never turn `accept all proposals` into a session permission.

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
| macOS | login Keychain through an authorized client | LocalAuthentication at session open or expiry | raw CLI storage is user-session protection, not app-isolated data-protection Keychain |
| Windows | current-user DPAPI/Credential Manager through an authorized client | Windows Hello/user consent at session open or expiry | DPAPI alone is user-scoped storage, not transaction consent |
| Linux | Secret Service through an authorized client | Secret Service plus non-cached polkit at session open or expiry | service implementations and process isolation vary |
| External agent | Ed25519 key remains in 1Password or an RFC 9987 agent | provider application/session policy | a generic agent socket needs explicit client authorization or confirmation constraints |

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

### Review command and future client adapters

The normal 0.901 entry point is the initiating agent task or `vela review
decide`. The agent may prepare and invoke the root-bound request; the person
does not copy commands, roots, or timestamps. The helper shows the State
Transition Card, not a signing utility. `vela review decide --json` remains the
adapter and test contract for a later review inbox.

The client requests one decision using `vela.review-decision.v1`. The preview
is key-free and includes the Decision Brief, action, typed rationale, reviewer,
observation time, and Decision Plan root. Before signing, Vela checks freshness,
eligibility, reviewer authority, policy and Engine inputs, binary identity, the
human card result, and the complete transaction read set. A future external
client also supplies its one-operation authorization capability.

The ordinary path has no `--key`, `--yes`, wildcard, saved-answer, or generic
signing input. A homogeneous, policy-bounded review set may use a visible batch
manifest with per-item exclusion and one batch root. Heterogeneous semantic or
governance decisions may not be batched. Cancellation or timeout writes
nothing.

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
fatigue, and is not portable. Platform authentication opens a bounded custody
session. It recurs only at expiry, provider lock, or an explicit high-assurance
policy boundary.

### Unlock once and let an agent impersonate the human

An authenticated session does not prove approval of later proposal semantics.
Routine autonomy uses an agent or service identity under a human-signed scoped
policy. It does not emit events that falsely claim the human reviewed each
item. A human key never becomes a generic session signing endpoint.

### Treat Keychain, DPAPI, Secret Service, polkit, or SSH-agent approval as the decision

These mechanisms protect storage, authenticate a user/session, or authorize key
use. Their prompts do not reliably show a Vela state transition. They support
custody and high-assurance confirmation; the contextual review item and final
state recheck supply semantic authorization.

### Replace the identity with Secure Enclave, Windows Hello, a passkey, or Sigstore

These are valuable future profiles, but they change algorithms, public keys,
or trust roots. They cannot silently preserve the existing Ed25519 authority.

### Keep batch signing as the ordinary path

Saved answers and queue-wide context are unnecessary inputs for one decision.
A batch is allowed only for a homogeneous, policy-bounded set with visible
membership, per-item exclusion, and one frozen root. Detached compatibility
remains under advanced help.

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
- A future custody or review-client endpoint authenticates its caller and binds
  grants to the client process/session, actor, frontier, request root, and
  expiry. A copied socket, named pipe, bearer capability, or stale client grant
  cannot authorize another process or decision. The 0.901 one-shot helper does
  not expose such an endpoint.
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
- `proposal.withdrawn` is the sole proposed wire addition; readers from before
  its eventual release may reject it at the intentional protocol boundary.
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
- an installed Vela binary or helper whose digest differs from its pin cannot
  sign; binary, helper, or protection-mode rebind requires one platform
  authentication and a valid response signature from the existing protected
  key;
- session mode authenticates at session open or expiry and never prompts merely
  because another signature is produced;
- only genuinely deferred human-authority items require a semantic action;
- routine policy-permitted work and producer withdrawal require no human
  prompt and never use the human key;
- always mode requires both the contextual review action and a per-use
  platform/agent check;
- cancellation, timeout, authentication failure, and post-approval state drift
  produce zero frontier writes;
- enrollment has exactly one human-facing platform authentication and no
  preceding generic confirmation alert;
- identity v1 replay, successful v1-to-v2 enrollment, crash recovery, readback,
  public-key mismatch, and plaintext removal;
- the CLI never receives protected seed bytes and rejects an authentic signature
  for the wrong request;
- macOS, Windows, and Linux profiles satisfy the same behavioral contract,
  with live current-platform custody checks at release;
- no wildcard, key path, `--yes`, legacy sign-session state, or model-produced
  boolean can authorize a human decision;
- the default card uses action-specific buttons, contains no caution icon for
  ordinary review, omits full keys, paths, provider internals, and roots from
  the primary view, and remains usable by keyboard and screen reader;
- a homogeneous batch proves visible membership, exclusion, policy bounds, and
  a frozen root; heterogeneous batches fail closed.

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
The deterministic release union runs once at the eventual release boundary.
External Lean, Diderot, live-network, hosted authority, model-held signing, and
unrelated suites remain excluded.

## Acceptance evidence required

Acceptance requires packages for macOS, Windows, and Linux, current-platform
live custody, and cold-use UX evidence. Four fresh users must complete routine
producer work with zero human prompts and identify that the agent, not the
human, signed it. Four fresh reviewers must resolve a deferred item from its
active task or platform card without copying terminal commands or roots,
encountering signing language, or seeing unexplained OS dialogs. They must
correctly state the semantic effect, why the item was deferred, and whether
accepted state changes.

The exact Erdős rejection at commit `0041be301ae2a9aa966e85d2d530de60c6c9192e`
is retained as a cryptographic success and usability failure. It is not reused
as positive UX evidence. Future dogfood uses non-authoritative fixtures until
the user reviews the new surface. Cancellation must change nothing; approval
must change only the displayed item; clean-clone replay must agree. ADR 0011
cannot become Accepted merely because signer tests pass.

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
[^github-signing]: [GitHub, Signing commits](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)
[^1p-security]: [1Password, About SSH Agent security](https://www.1password.dev/ssh/agent/security)
[^claude-permissions]: [Claude Code CLI permissions](https://code.claude.com/docs/en/cli-usage)
[^apple-alerts]: [Apple Human Interface Guidelines, Alerts](https://developer.apple.com/design/human-interface-guidelines/alerts) and [Disclosure controls](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)
[^github-review]: [GitHub, Reviewing deployments](https://docs.github.com/en/actions/how-tos/managing-workflow-runs-and-deployments/managing-deployments/reviewing-deployments)
