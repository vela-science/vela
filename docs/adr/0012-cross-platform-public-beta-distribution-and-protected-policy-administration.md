# ADR 0012: Cross-platform public-beta distribution and protected policy administration

- Status: Proposed
- Target: Vela `v0.910.0`
- Protocol effect: none

## Context

Vela `v0.901.0` proved protected one-proposal decisions on macOS and compiled
the signer contracts on Linux and Windows. Its release assets are checksum
addressed, but are not yet an adoption-grade distribution: they carry no SBOM
or build attestation, macOS is not Developer-ID signed and notarized, Windows
is not Authenticode signed, and the protected policy ceremony still teaches a
legacy key path and `--yes` flow.

Canopus and the site are deliberately removable. Moving Canopus into Vela's
Rust workspace would obscure that boundary without improving replay. The
composition problem is version pinning, not source ownership.

The July 17–18 Science Frontier briefs reinforce the same product shape:
canonical Git state, projected task context, executable plans, bounded patches,
and explicit admission. They do not establish a need for an active context
graph, temporal claim primitive, generic memory layer, or method registry.

A July 18 authority review also identified limits that the beta must state
honestly. Generic OS credential storage does not provide uniform same-user
process isolation, a signed local session record is tamper-evident rather than
caller-bound, and an OS authentication surface is not a scientific decision
surface. Vela therefore reports the current cross-platform grade as
`user_session`, not `app_isolated` or hardware-backed, and makes no claim to
resist a compromised OS or arbitrary same-user malware.

## Decision

Vela 0.910 is a product and release-trust change with no event, reducer,
Receipt, proposal, signature-algorithm, or accepted-state change.

1. The parent publishes `vela.ecosystem-lock.v1`, explicitly labeled
   non-protocol composition. It binds exact Vela, Canopus, site, and Codex
   releases and artifacts. Canopus remains a separate repository and is not a
   submodule.
2. Release assets carry checksums, an SBOM, and GitHub build attestations.
   macOS artifacts and the signer helper are Developer-ID signed and notarized.
   Windows executables are Authenticode signed and RFC 3161 timestamped.
3. First-party Unix and PowerShell installers verify the locked archive before
   installation. They never inspect or edit a frontier.
4. `doctor --all` and `id show` report installed binary/helper identity,
   protected backend health, stale rebind state, and one recovery action, but
   no custody bytes.
5. Ordinary policy administration becomes the same two-phase protected flow
   as `review decide`:

   ```text
   vela policy decide <frontier> --activate <vap_id> --reason <text>
   vela policy decide <frontier> --rotate <vap_id> --reason <text>
   vela policy decide <frontier> --revoke --reason <text>
   ```

   A preview is key-free and returns `vela.policy-decision.v1`. Confirmation
   supplies only the exact plan root and observation time. Vela rederives the
   policy, action, frontier roots, signer authority, binary pin, and transaction
   read set before requesting one protected OS decision card. Cancellation or
   authentication failure writes nothing. Legacy `policy sign` and `--key`
   remain advanced replay/compatibility surfaces and are never consumed by
   `policy decide`.

6. A replacement policy is a complete desired rule set. Rotation never carries
   an omitted rule forward implicitly. The key-free plan and protected card
   show added, removed, changed, and unchanged authority counts, with removed
   rule IDs named explicitly.
7. The signer derives the policy card from the exact typed selected policy,
   prior policy, action, and event material. It does not accept caller-authored
   policy facts or consequences. The rationale remains caller-supplied text and
   is visibly bound into the exact plan.
8. Policy activation, rotation, and revocation are rare standing-authority
   changes and always request fresh platform authentication after the semantic
   card. Ordinary one-proposal decisions may continue to use a bounded custody
   session; that session never approves later semantics.

The policy card names the action, policy ID and full root, frontier, expiry,
rule summary, authority diff, reason, and plan-root prefix. Every policy action
receives its own exact card and fresh authentication.

This release deliberately does not add a resident authority daemon, TUF root,
passkey actor, generic signing socket, or new policy language. A later broker
may move session state behind a signed application boundary and independently
replay the complete frontier. That work requires a separate threat-model ADR
and platform evidence. Until then, routine binary rebind remains an explicit
beta limitation rather than being hidden behind an unauthenticated update path.

## Compatibility and failure semantics

- Existing identity v1/v2 records, policy v0.1 bytes, policy-head events, and
  historical frontiers replay unchanged.
- Missing signing credentials block the corresponding public-beta platform;
  unsigned candidates must not be described as released public-beta assets.
- Any binary/helper drift, root drift, stale policy, wrong actor, invalid
  confirmation, or unavailable protected backend fails before custody access.
- The ecosystem lock grants no authority and cannot override Vela replay or a
  signed frontier policy.

## Conformance

- exact preview/confirmation binding for activate, rotate, and revoke;
- cancellation and authentication failure produce zero writes;
- wrong action, policy, reason, timestamp, binary, registry, or roots fail
  before the helper request;
- omitted policy rules are removed, authority diffs are exact, and every policy
  action reauthenticates after the card;
- old policy events and identities replay unchanged;
- archive checksum, SBOM, attestation, notarization, and Authenticode checks;
- fresh install, upgrade, rebind, and uninstall on macOS Apple silicon,
  Ubuntu 24.04 x86-64, and Windows 11 x86-64.

ADR 0012 becomes Accepted only when the exact released artifacts and live
platform ceremonies pass. Candidate code does not satisfy the release gate.
