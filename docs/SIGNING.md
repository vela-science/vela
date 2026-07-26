# Signing and protected administration

> **Migration-candidate behavior.** Vela `0.915.1` is retained only for
> historical Era-0 replay. The `0.930.0-rc.12` source candidate implements Proposed
> [ADR 0020](adr/0020-attributed-repository-authority-and-standard-delegation.md).
> It replaces the personal-key helper, copied root/time confirmation, and custom
> AcceptancePolicy writer with attributed repository authority, restricted
> Cedar policy, and ordinary authentication. Until a Frontier crosses its
> explicit migration boundary, the released behavior below remains the only
> live writer and historical verification remains unchanged.

The candidate recognizes the closed `vela.authority-model-migration.v1`
bridge and verifies contiguous DSSE authority-record coverage after it. Its
writer performs initial installation, ordinary work-lease transactions,
Receipt-bound pending submissions, exact keyset and policy rotation, and
terminal emergency close through the existing recoverable journal. Erdős is
an active migrated Frontier and has exercised the signed-agent claim/release
lifecycle without changing scientific state.

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
input, exact principal, and derived Cedar context. Signed lease events cover
coordination; signed activity records cover exact Receipt-bound pending
submissions. Fresh platform-owned user presence covers the narrow `authority
enable-work` policy rotation without reading a human Vela key.
Passkey, OIDC, GitHub App, or SciTokens integrations remain replaceable
adapters.

The authority record retains only a closed
`vela.authentication-observation.v1`: exact principal and issuer-subject,
method, assurance, non-secret session root, bounded times, presence and
verification facts, recovery context, and revocation reference. It never
retains a cookie, bearer token, provider assertion, or raw session ID.

Vela uses signatures to attribute exact authority-bearing actions. A
signature proves control of one registered key over one canonical input; it
does not prove scientific truth. Git publication and verifier success are
also not acceptance.

Before repository-authority migration, human keys remain behind the protected
operating-system signer for historical Era-0 writes. An agent may prepare and
invoke an exact request, but it may not approve the OS card, read the seed, use
a legacy file key autonomously, or claim the request itself changed standing.
After migration, exceptional review uses platform user presence only: the
provider returns a bearer-free authentication observation, Cedar authorizes the
exact action, and the repository authority signs the covering transaction.
The provider never retrieves or signs with a personal Vela key.

## Enable routine producer work after migration

Frontiers migrated before the routine-work policy was added remain fail-closed
until one exact policy rotation:

```bash
vela authority enable-work . \
  --reason "Enable exact signed-agent leases and Receipt-bound pending submissions while preserving human decisions." --json
vela authority enable-work . \
  --reason "Enable exact signed-agent leases and Receipt-bound pending submissions while preserving human decisions." \
  --apply --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

Preview is key-free. Apply obtains fresh platform user presence for the exact
old and new policy roots, then asks the repository authority to sign the
covering record. It never reads the legacy human key. The resulting Cedar
rules permit only an exact agent-signed lease and an exact signed,
Receipt-bound pending submission. Landing covers proposal and evidence bytes
without appending an accepted scientific event. The rules cannot authorize
review, acceptance, policy administration, membership, recovery, or key
rotation.

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

On a repository-authority Frontier, each human decision is one exact request:

```bash
vela review decide . <vpr_id> --accept --reason "<reason>" --json
vela review decide . <vpr_id> --reject --reason "<reason>" --json
```

Vela derives and locks the proposal, reason, Decision Brief, policy, local
issuer-subject principal, binary/helper pair, read set, and Decision Plan
before asking the platform for one user-presence approval. The approval request
names the action, proposal, proposal root, policy root, reason, and intent
root. A successful provider response contains no credential and grants no
reusable approval. Cedar must separately permit `review_reject` for that
principal and proposal; the repository-authority DSSE record then covers the
new `review.rejected` event and proposal postimage. Rejection changes no
scientific root.

Repository-authority acceptance uses the same protected exact-intent request
only after the Decision Brief and strict aggregate Engine gate permit the
action. The authority transaction installs both the scientific domain event
and `review.accepted`; the review payload points to the domain event's
transaction-independent semantic ID, while the DSSE record covers the stored
transaction-attributed event IDs and object postimages. Dual-log replay rejects
a missing, duplicate, or ambiguous applied semantic event. Leaving an
Engine-blocked proposal pending remains the correct outcome.

Era-0 review decisions remain two phase while selected frontiers complete
repository-authority migration:

```bash
vela review decide . <vpr_id> --reject --reason "<reason>" --json
vela review decide . <vpr_id> --reject --reason "<reason>" \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json

```

Preview reads no key. Matching execution rederives the proposal, action,
reason, roots, authority, pinned binary/helper, and transaction read set before
showing one exact OS card. Action, reason, target, timestamp, or root drift
invalidates confirmation. Cancellation, timeout, or authentication failure
writes no event, signature, committed journal marker, or Git commit.

The protected path accepts no key path, batch, wildcard, persistent approval,
saved session answer, or `--yes`. AcceptancePolicy authoring and policy signing
porcelain are retired; only read-only Era-0 policy verification remains.

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

## Authority-model migration candidate

The proposed `0.930.0-rc.12` transition is deliberately separate from the
older repository-profile migration:

```bash
vela authority migrate <frontier> \
  --repository-key-id <stable-key-id> \
  --repository-public-key <64-lowercase-ed25519-hex> \
  --reason "<semantic reason>" \
  --json
```

Preview reads public identity metadata but never asks the protected store for
the legacy seed and writes no Frontier byte. It binds the clean `main` commit
and tree, exact legacy event and registry roots, retained legacy policy bytes,
one local issuer-subject principal, the initial restricted Cedar bundle, the
repository keyset, binary digest, reason, observation time, and complete
candidate event.

The agent or operator may then invoke the returned exact plan:

```bash
vela authority migrate <frontier> \
  --repository-key-id <stable-key-id> \
  --repository-public-key <64-lowercase-ed25519-hex> \
  --reason "<same semantic reason>" \
  --apply \
  --confirm-root <plan-root> \
  --confirm-at <observed-at> \
  --json
```

The human action is the one semantic OS approval card; they do not sign a
file or reveal a seed. The helper accepts only the unsigned
`authority.model_migrated` event in that exact plan, performs fresh user
presence, reads the protected continuity seed once, signs, zeroizes, and
exits. The repository record is separately signed through the standard
OpenSSH agent. The resulting canonical delta remains unstaged for inspection.

This is temporary migration scaffolding, not the new everyday approval model.
It disappears with the old helper after every active Frontier has crossed the
boundary. Cancellation, helper refusal, wrong key, stale Git or policy state,
non-`main` or dirty worktrees, an existing authority history, binary/helper
drift, or transaction drift produces no canonical delta. Historical event
files retain their exact bytes even when their JSON formatting predates
canonical output.

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
