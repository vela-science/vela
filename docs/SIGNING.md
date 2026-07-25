# Signing and protected administration

> **Era-0 release behavior.** This document describes the writer shipped in
> Vela `0.915.1`. Proposed
> [ADR 0020](adr/0020-attributed-repository-authority-and-standard-delegation.md)
> replaces the personal-key helper, copied root/time confirmation, and custom
> AcceptancePolicy writer with attributed repository authority, restricted
> Cedar policy, and ordinary authentication. Until a Frontier crosses its
> explicit migration boundary, the released behavior below remains the only
> live writer and historical verification remains unchanged.

The candidate recognizes the closed `vela.authority-model-migration.v1`
bridge and verifies contiguous DSSE authority-record coverage after it. A
CLI-unreachable disposable writer now proves initial installation, ordinary
transactions, exact keyset and policy rotation, and terminal emergency close
through the existing recoverable journal. It exposes no signing command,
does not make the proposed Era-1 path live, and has not migrated an active
Frontier.

Emergency close is a current-authority transaction, not a recovery bypass. It
requires the human-only `authority_close` action, installs an empty terminal
successor keyset, records the exact incident boundary, and makes all later
authority records invalid. It preserves historical verification and cannot
reopen the lineage.

The candidate also recognizes stable issuer-subject principals and retained,
short-lived capability claims for agents and workloads. Those objects contain
no bearer credential and cannot express human review, policy, membership,
recovery, quorum, bulk-correction, destructive, migration, rotation, or
revocation authority. The candidate now has an injectable, filesystem-free
runtime preflight that validates a provider observation, live revocation
input, exact principal, and derived Cedar context. Platform local-session,
passkey, OIDC, GitHub App, or SciTokens integration remains a replaceable
adapter and none is a released writer.

The authority record retains only a closed
`vela.authentication-observation.v1`: exact principal and issuer-subject,
method, assurance, non-secret session root, bounded times, presence and
verification facts, recovery context, and revocation reference. It never
retains a cookie, bearer token, provider assertion, or raw session ID.

Vela uses signatures to attribute exact authority-bearing actions. A
signature proves control of one registered key over one canonical input; it
does not prove scientific truth. Git publication and verifier success are
also not acceptance.

Human keys remain behind the protected operating-system signer. An agent may
prepare and invoke an exact request, but it may not approve the OS card, read
the seed, use a legacy file key autonomously, or claim the request itself
changed standing.

## Protect a human identity

```bash
vela id create
vela id protect --json
vela id show --json
```

Protection authenticates before reading the source seed, proves that the seed
derives the existing public key, installs identity v2 atomically in the
platform credential store, removes the plaintext source, and pins the exact
Vela and signer-helper bytes. Interrupted cleanup leaves protected decisions
disabled until the same command safely resumes.

macOS uses the data-protection Keychain and LocalAuthentication; Windows uses
the protected Windows signer and Windows Hello; Linux uses its supported
credential-store and polkit path. Provider diagnostics describe local custody,
not the scientific identity. `vela id lock` closes the bounded local approval
session without changing a Frontier or deleting the protected identity.

## Exact protected decisions

Review and policy decisions are two phase:

```bash
vela review decide . <vpr_id> --reject --reason "<reason>" --json
vela review decide . <vpr_id> --reject --reason "<reason>" \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json

vela policy decide . --activate <vap_id> --reason "<reason>" --json
vela policy decide . --activate <vap_id> --reason "<reason>" \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

Preview reads no key. Matching execution rederives the proposal or policy,
action, reason, roots, authority, pinned binary/helper, and transaction read
set before showing one exact OS card. Action, reason, target, timestamp, or
root drift invalidates confirmation. Cancellation, timeout, or authentication
failure writes no event, signature, committed journal marker, or Git commit.

The protected path accepts no key path, batch, wildcard, persistent approval,
saved session answer, or `--yes`. Legacy `vela sign` and `policy sign` remain
advanced compatibility surfaces and are not the ordinary workflow.

## First repository administrator

A Profile v1 structural genesis is unsigned and cannot choose its own
administrator. Bootstrap therefore has two explicit protected steps.

First replace only the canonical empty actor registry:

```bash
vela actor add . --json
git status --short
git diff
git add <exact-paths-from-the-bootstrap-delta>
git commit -m "Bootstrap Frontier administrator"
```

The local identity must be a protected human; the subsequent boundary requires
its ID to be in the `reviewer:` or `steward:` namespace. The one-shot signer
proof binds the exact actor record and registry delta. Agent and plaintext-file
identities are refused. An established registry cannot be extended or replaced
through this command.

Then establish the first signed repository boundary:

```bash
vela frontier bind . --reason "establish the first administrator" --json
vela frontier bind . --reason "establish the first administrator" \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

The boundary binds Frontier identity, dependencies, administrator, Git and
Vela anchors, actor registry, retained objects, reason, and plan root. It is
non-scientific and changes no finding standing. Apply installs the matching
local consumer pin but leaves the exact boundary delta unstaged and
uncommitted for inspection.

Other consumers independently review the full first-boundary content root and
install it:

```bash
vela frontier trust pin . --boundary-root <sha256:...> --json
vela frontier trust pin . --boundary-root <sha256:...> \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

The resulting `vela.repository-trust-anchor.v1` record is public local
configuration. It is not a secret, actor registration, policy, event, or
scientific authority. Repository bytes never manufacture their own external
pin.

## Legacy migration

`vela migrate ... --to frontier-repo-v1` uses the same protected-plan pattern.
Preview binds the exact legacy Git and Vela anchors, retained bytes, candidate
profile, dependency resolutions, Target Index v2 candidate, administrator,
reason, and touched paths. Apply revalidates all facts before one OS approval.

The operation preserves all pre-boundary event, proposal, Receipt,
registration, policy, finding, artifact, evidence, and signature bytes. It
appends one non-scientific signed boundary and leaves the exact delta
uncommitted.

## Fail-closed rules

- A valid boundary signature without the consumer's independently installed
  first-boundary pin cannot authorize canonical writes.
- Missing Git objects, non-ancestor anchors, altered retained bytes, boundary
  forks, registry drift, helper/binary drift, and stale plans fail before key
  use or transaction journaling.
- Non-strict inspection reports repository defects but never grants an
  identity, dependency, signature, or historical exemption.
- Protected decisions cannot be triggered through MCP or local HTTP.
- Profile v1 actor-registry rotation and administrator recovery are not hidden
  behind a generic command; until a separately governed contract exists,
  registry drift fails closed.
- A candidate Era-1 history without one exact legacy-signed migration bridge,
  or with any later legacy write, coverage gap, duplicate coverage,
  transaction substitution, root drift, authority fork, keyset substitution,
  or policy substitution, fails closed and never falls back to Era-0.
