# Authority and attribution

Vela separates evidence production, verification, authorization, and
scientific Standing.

```text
producer evidence
  -> scoped verification
  -> pending Proposal
  -> authorized semantic Decision
  -> repository-authority record
  -> Standing
```

A signature proves control of a key over exact bytes. It does not prove a
Claim true. Verification does not imply acceptance, and Git publication
implies neither.

## Repository authority

Every current Frontier has:

- an append-only repository-authority history;
- a full-root Ed25519 authority keyset;
- a retained restricted Cedar authorization bundle;
- exact attributed principals;
- one DSSE authority record covering each Decision or administrative
  transaction; and
- an independently distributed sequence-one authority-record root.

Repository authority is a service identity, not the scientific reviewer. Its
signature attests that the principal, authorization, semantic action, read-set
recheck, and canonical write matched. The record attributes the human, agent,
or workload responsible for the action.

The initial local provider is the normal OpenSSH agent. Vela asks it to sign
the exact authority record with the key selected by the Frontier keyset. Vela
does not create, store, reveal, or recover a human signing seed.

`vela.authority-keyset.v1` deliberately fixes Ed25519. The algorithm field is
explicit so a future keyset profile can add a qualified P-256 hardware provider
or ML-DSA archival co-signature without guessing or reinterpreting v1 history.
Passkeys authenticate people; repository keys authenticate the service role.

## Human Decisions

Inspect one pending Proposal:

```bash
vela review show . <vpr_id> --json
```

Then perform exactly one action:

```bash
vela review reject . <vpr_id> \
  --reason "The retained evidence does not satisfy the stated conditions." \
  --json
```

or, when the Proposal says acceptance is eligible:

```bash
vela review accept . <vpr_id> \
  --reason "The exact claim, evidence, verification, and conditions support acceptance." \
  --json
```

The command is the semantic action. The repository-authority key is a service
identity held by the standard OpenSSH agent, not a personal approval key. Load
the dedicated key once for the current operating-system session:

```bash
# macOS: retain the key in the login Keychain and load it into the current agent
ssh-add --apple-use-keychain ~/.ssh/vela_repository_authority_ed25519

# Linux: bound unattended use to the current eight-hour work session
ssh-add -t 8h ~/.ssh/vela_repository_authority_ed25519
```

Do not add `-c` unless you deliberately want OpenSSH to prompt for every
signature. Vela does not require that prompt. The authenticated operating-
system session establishes the human principal; the exact CLI action, Cedar
evaluation, compare-and-swap root, reason, read set, and signed postimage
establish the Decision.

On macOS, Vela consults launchd for the standard login-session agent when a
long-running GUI process has no inherited `SSH_AUTH_SOCK`. An explicitly set
socket always wins. This discovery reconnects to OpenSSH only; Vela does not
run an agent daemon, cache a private key, or invent a second signing session.

Vela:

1. derives the exact Proposal and transaction plan;
2. binds the Proposal, action, reason, principal, policy, authority head,
   binary identity, read set, and canonical delta;
3. authenticates the local operating-system principal;
4. evaluates the retained policy;
5. rechecks every transaction input;
6. asks the OpenSSH agent to sign the covering authority record;
7. installs the transaction through the recoverable journal; and
8. publishes the exact delta as one local Git commit.

There is no copied root or timestamp, custom signer helper, Vela human key,
approval session, batch mode, wildcard, or `--yes`. OpenSSH owns local key
custody; Vela owns validation of the exact authority payload. A trusted native
agent session may execute a Decision that the operator explicitly authorized.
That authorization does not weaken Vela's per-Decision policy, current-root,
semantic, or replay checks and does not authorize unrelated Decisions.

`SSH_AUTH_SOCK` is a powerful local capability. Do not forward it to remote,
untrusted, or proposal-supplied code. Hosted and shared deployments should
replace the local provider with a policy-constrained workload or KMS signer,
not forward a developer's SSH agent.

## Producer identity

Locally authored producer work names one actor with `--as agent:<name>` or
`VELA_ACTOR_ID=agent:<name>`. Vela creates and reuses one local per-actor key
on first write; there is no separate identity enrollment command or profile.
Imported signed Submissions and Verification Records carry their own actor and
key binding.

A native workbench may retain its own run or attempt identity as provenance,
but `vela start` creates no signed or local protocol object. Producer identity
cannot authorize review, acceptance, policy
administration, recovery, membership, or repository-key changes.

```bash
vela next . --json
# Optional write-free Target briefing.
vela start <target> --repo . --json
vela submit submission.json --repo . --json
```

Submission intake creates no Verification Record, Decision, Event, or accepted
Standing. Submission and Verification intake verify the producer or verifier
signature, retain append-only content-addressed evidence, and rebuild the
deterministic repository projection without reading the repository-authority
key. Only a later human Decision creates an Authority Record.

## Initialize a new Frontier

Load one dedicated Ed25519 repository-authority identity into the normal
OpenSSH agent, then run:

```bash
vela init . \
  --name "Bounded question" \
  --scope "Does X hold under Y?" \
  --json
```

Vela automatically selects the key only when exactly one plain Ed25519
identity is loaded. Otherwise, select the full OpenSSH fingerprint with
`--key SHA256:<fingerprint>`.

Initialization writes the Profile, initial keyset, Cedar bundle, exact policy
material, initialization Event, and covering sequence-one DSSE record. It
changes no scientific Standing. The creator's matching local trust anchor is
installed in the same operation; repeating the returned root as a second local
setup ceremony is unnecessary. If signing fails, load the key and rerun the
same `vela init` command.

Distribute the returned full sequence-one authority-record root independently
of the Frontier checkout.

## Release distribution identity

Publishing a binary is not a scientific Decision, so it is not signed by the
repository authority. `scripts/release.sh` emits one
`vela.release-bundle-manifest.v1` per bundle and signs it under a separate
distribution identity; `scripts/release.sh` refuses any key whose path names the
repository-authority identity, because a release signed by that key would be
indistinguishable from a Decision to anyone checking signatures.

The signing path takes the **public** key and routes through `ssh-agent`
(`ssh-keygen -Y sign -U`), so neither the script nor any workflow reads private
key material.

The current distribution identity is published in `allowed_signers` at the
repository root, so a verifier needs a clone and nothing else:

```text
release@vela.space  SHA256:MX3Eo1o9S5pLnx2kiNyAy2aME7PAWDtvqtUBljJst1M
```

That entry carries `namespaces="vela-release"`, which scopes the key to release
manifests. Without it the line would accept a signature this identity made over
anything at all, which is a wider claim than the one it exists to support.

Verify a published manifest against it:

```bash
ssh-keygen -Y verify -f allowed_signers -I release@vela.space \
  -n vela-release -s release-manifest.json.sig < release-manifest.json
```

A good signature reports the fingerprint above. Any edit to the manifest — one
byte is enough — reports `Signature verification failed`.

The manifest is unsigned when CI builds it, deliberately. Putting the private
half into Actions would re-couple the artifact to the provider the manifest
exists to be independent of, so a signed manifest is produced by a human running
the same script on a machine holding the identity. Per-asset build provenance
(`actions/attest-build-provenance`) is OIDC-bound to GitHub and stays
provider-bound; it attests who built an asset, not who published the release.

## Consumer trust

Install the independently obtained public root:

```bash
vela authority trust pin . --record-root sha256:... --json
```

The anchor is a closed record binding the Frontier and first authority-record
root. That record already commits to the initial keyset, policy, principal,
Events, and execution claim.

Pinning writes only local consumer configuration. It reads no private key,
grants no authority, and changes no Frontier byte.

After a separately verified repository-origin transition establishes a new
sequence-one authority record for the same Frontier, advance the local pin
with an exact compare-and-swap:

```bash
vela authority trust pin . \
  --record-root sha256:<new-sequence-1-root> \
  --previous-record-root sha256:<exact-installed-root> \
  --json
```

The new root must match the current repository's sequence-one record, and the
previous root must match the installed local pin. Repeating an already applied
pin is idempotent. This operation still reads no authority key, grants no
authority, and changes no Frontier byte.

## Failure behavior

Vela refuses a repository-authority transaction when any required input is
missing, stale, ambiguous, or invalid, including:

- the sequence-one trust anchor or authority history;
- exact principal attribution;
- required scoped authorization or human action;
- the Proposal, Submission, Verification Record, Claim, or Artifact binding;
- the transaction read set or canonical delta;
- the selected repository-authority key or signature; or
- recovery-journal and commit-marker integrity.

Preflight, authentication, authorization, signing, or cancellation failure
creates no committed authority record, Event, Proposal mutation, or Standing
change.

## Historical verification

Predecessor tags and source archives preserve earlier repository contracts.
Use their pinned binaries to verify their bytes. The current binary does not
keep historical writer commands, personal key products, or compatibility
aliases alive.
