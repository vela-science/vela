# ADR 0012: Cross-platform public-beta distribution and protected policy administration

- Status: Partially superseded (2026-08-01)
- Target: Vela `v0.910.0`
- Protocol effect: none

> Current disposition: the custody and Decision-boundary analysis remains
> historical design evidence. Vela `0.960.0` supersedes this ADR's distribution
> section with one GitHub-attested `vela` binary; crates.io/npm publication,
> package-registry smoke jobs, and custom release-trust metadata are retired.
> Vela `0.965.3` further narrows the supported binary surface to Linux x86-64
> and macOS Apple silicon. Windows artifacts, installer, action path, and release
> smoke are retired until native Windows support becomes outcome-critical.

## Context

Vela `v0.901.0` proved that one exact proposal decision can remain human-governed
without giving a model the human seed. It also exposed a product failure: custody,
authentication, transaction approval, scientific authority, and release signing
were presented as if they were one Apple-shaped identity ceremony. They are not.

The Vela identity is the registered Ed25519 actor and public key. Keychain,
Credential Manager, Secret Service, and external agents are replaceable local
custody adapters. Touch ID, a device passcode, Windows Hello, and polkit establish
or refresh a local authenticated session. A semantic decision card authorizes one
exact state transition. A signed frontier policy delegates a closed class of
routine work to an agent. Developer ID, Authenticode, package-registry provenance,
and GitHub attestations authenticate distributed software, not scientific actors.

Conflating these layers creates the worst of both worlds: repeated prompts that
encourage workarounds, implementation details in human-facing cards, and the false
impression that an operating-system account is a Vela authority. Letting a model
use the human key would remove the prompt but would also remove the meaning of the
human signature. The useful automation boundary is delegation: an agent signs its
own executions and Receipts, while a previously human-signed exact policy decides
whether matching work may enter accepted state or must Defer.

NIST SP 800-63B-4 distinguishes authentication from session continuity and
supports inactivity, overall, and step-up reauthentication. OWASP transaction
authorization requires the consequential operation to remain visible and bound to
one short-lived authorization. Codex approval requests likewise keep requester and
approver separate and bind the response to the active operation. ADR 0011 applies
that model to one-proposal decisions; this ADR makes it the whole product model.

Release trust has a separate portability problem. Apple Developer ID is required
for a normal Gatekeeper experience for direct-download macOS binaries. Microsoft
offers managed Artifact Signing for Windows executables. Neither credential should
be required to publish a source-installable, provenance-verifiable public beta on
all platforms. Requiring both made two vendor accounts a universal Vela release
gate and kept already verified source and packages artificially unreleased.

Canopus and the site remain deliberately removable. Moving Canopus into Vela's
Rust workspace would obscure the authority boundary without improving replay. The
composition problem is exact version and artifact binding, handled by the
non-protocol ecosystem lock.

## Decision

Vela 0.910 is a product, custody, and release-trust change. It adds no event,
reducer rule, Receipt field, proposal schema, signature algorithm, actor type, or
accepted-state authority.

### One identity, several hidden custody adapters

The only Vela identity shown in ordinary product language is the actor ID and
registered public key. Identity v2 retains the existing helper record for replay,
but ordinary output calls it a protected approval identity. Provider names,
credential-store paths, helper roots, binary roots, and protection grades appear
only in `--json` or `doctor --all` diagnostics.

`vela id protect` becomes a one-time setup command with safe defaults. It
authenticates before reading the plaintext seed, verifies the derived public key,
stores and reads back the seed through the platform adapter, atomically installs
identity v2, and removes the plaintext source. Existing explicit safety flags
remain accepted for script compatibility but disappear from ordinary help.

The default is `session` mode. A successful platform authentication opens a
signed, actor-, public-key-, provider-, helper-, and mode-bound local session with
a 15-minute inactivity limit and one-hour overall limit. `vela id show` reports
whether that session is active, expired, invalid, or closed. `vela id lock`
deletes only local session state and never touches the protected seed, actor
record, frontier, or Git state. `always` remains an explicit high-assurance mode
for environments where the requester can automate the desktop.

No automated fixture may display an OS prompt. `VELA_NO_USER_INTERACTION=1` is a
fail-closed latch at every authentication and semantic-card edge, and all automated
tests use injected approval and custody fakes. A fixture identity, including an
`atlas-fixture` identity, is never enrolled into an operator's real credential
store.

### Signing becomes exceptional

The ordinary product has three paths:

1. **Routine agent work.** An agent signs its own lease, evidence, Receipt, and
   producer events. A human-signed AcceptancePolicy may Permit only the exact
   target, packet, profile, verifier capsule, result contract, and producer
   credential it names. Matching work needs no human prompt. Everything outside
   the closed policy Defer or fails.
2. **Human exception.** `vela review decide` displays one semantic card for one
   proposal and action. A live bounded session avoids another biometric or
   passcode prompt, but never pre-approves the semantic card. The card uses action
   verbs and scientific consequences; it does not display custody internals.
3. **Standing-authority change.** `vela policy decide` displays one exact policy
   activation, rotation, or revocation card and then requires fresh step-up
   authentication. Policy changes are rare and can authorize many subsequent
   routine runs. A policy session can never become `accept all`.

An agent may prepare and invoke either exact request. It may not answer the card,
control the protected UI, access the human seed, or emit a human event. Model or
requester provenance never changes the registered signer. Legacy `vela sign`,
key paths, `--yes`, saved answers, and batch compatibility remain advanced-only.

The first call to `review decide` or `policy decide` remains key-free and returns
the complete typed plan. Confirmation supplies only its exact root and observation
time. Vela rederives eligibility, actor authority, policy or Engine inputs, binary
identity, frontier roots, and transaction read set before the helper is invoked.
Cancellation or authentication failure writes no event, proposal mutation,
journal commit marker, or Git commit.

### Provenance-first public-beta distribution

The public beta has one universal release identity and optional platform-native
trust tiers:

1. The source tag, crates.io packages, checksums, SPDX SBOMs, and GitHub build
   attestations bind the exact source and built artifacts. The crates are published
   in dependency order and are immutable. `cargo install vela-cli --locked
   --version <exact>` is the universal source-build installation contract.
2. A first-party Homebrew tap may provide a source-built macOS formula bound to
   the exact crate or source archive and checksum. It does not pretend a locally
   built binary is Developer-ID notarized.
3. GitHub release archives remain provenance-verifiable portable artifacts. An
   unsigned or unnotarized archive is labeled `portable, platform signature
   absent`; it is never described as a platform-trusted native installer.
4. Developer-ID signing and notarization are the macOS native-download tier. They
   remain required before advertising a no-warning direct-download macOS build.
5. Windows native signing uses managed Azure Artifact Signing with GitHub OIDC
   when available, avoiding a reusable PFX secret. It remains required before
   advertising a platform-trusted Windows download.

Absence of Apple or Microsoft signing credentials does not block the universal
source/package-manager public beta. It blocks only the corresponding native trust
tier. This separation is explicit in release metadata, installers, the ecosystem
lock, site copy, and `doctor`.

The parent publishes `vela.ecosystem-lock.v1`, labeled non-protocol composition,
binding exact Vela, Canopus, site, Codex, package integrity, source commits,
attestations, and available platform trust tiers. The lock grants no frontier
authority.

## Compatibility and failure semantics

- Existing identity v1/v2 records, policy bytes, policy-head events, signer
  sessions, and historical frontiers replay unchanged.
- Existing explicit `id protect` safety flags remain accepted. The simpler command
  changes only CLI defaults and help.
- Missing, expired, invalid, or drifted sessions cause reauthentication; they do
  not authorize or deny a scientific decision.
- Binary/helper drift, stale roots, wrong actor, ineligible action, invalid
  confirmation, unavailable custody, or a changed transaction read set fails
  before a frontier write.
- A valid session proves authenticated custody continuity, not approval of a later
  proposal or policy.
- A compromised operating system or arbitrary same-user GUI automation is outside
  the `session` profile. Operators needing resistance to desktop automation use
  `always` mode and accept per-operation user presence.
- A missing native platform signature downgrades only that distribution artifact.
  It never changes replay, policy, event, or scientific standing.

## Conformance

- simple and legacy-compatible identity protection; v1 replay and v1-to-v2 crash
  recovery; seed/public-key mismatch and plaintext-removal failure;
- session active, idle-expired, overall-expired, invalid, and explicitly locked
  states; locking changes no protected key or frontier byte;
- no automated fixture can reach an OS authentication or semantic-card edge;
- exact review and policy preview/confirmation binding; cancellation and
  authentication failure produce zero writes;
- wrong action, policy, proposal, reason, timestamp, binary, helper, registry,
  roots, or transaction read set fails before key use;
- policy replacement is complete, authority diffs are exact, and policy actions
  always step up after the exact card;
- a matching agent Receipt can Permit without a human prompt, while missing or
  substituted target, packet, profile, capsule, result contract, or producer
  credential cannot;
- dependency-ordered `cargo package` and publish/install smoke from exact registry
  bytes on macOS Apple silicon, Ubuntu 24.04 x86-64, and Windows 11 x86-64;
- checksum, SBOM, GitHub attestation, archive labeling, and ecosystem-lock checks;
- Developer ID/notarization and Azure Artifact Signing checks only when the
  corresponding native trust tier is declared.

ADR 0012 is accepted for `v0.910.0` on the following release evidence:

- the exact stable candidate at commit
  `8796ef94b296faefc308cca6c657096f477a1856` built and passed fresh
  install/upgrade/uninstall smoke on macOS ARM64, Linux x86-64, and Windows
  x86-64 in GitHub Actions run
  [29695967268](https://github.com/vela-science/vela/actions/runs/29695967268);
- the deterministic local release union passed 45 gates with zero failures;
  the one warning is the already-declared Sidon/formal frontier reconciliation
  boundary, and external Lean and live-network suites remained excluded;
- protected signer, review-decision, policy-decision, cancellation, help-surface,
  package, installer, action, and old-replay conformance passed against exact
  `0.910.0` bytes; and
- the release publisher is required to publish and then verify the immutable
  six-crate graph before the public tag is pushed. A failed publication or final
  registry smoke prevents the release rather than weakening this decision.

No live scientific policy was activated merely to test the product. The first
real policy approval remains an exact human ceremony for a concrete frontier;
deterministic fakes cover the protected card and cancellation paths without
creating synthetic authority history. Native platform tiers remain independently
gated and must not be inferred from the universal beta release.
