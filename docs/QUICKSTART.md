# Vela quickstart

Vela is version control for scientific state: a protocol and toolchain for
governed, replayable scientific-state transitions in ordinary Git repositories.
This guide starts with a real public scientific history, then walks through one
Submission, one scoped Verification, and one attributed Decision.

The stages have separate authority effects:

| Stage | Meaning | Effect on Standing |
| --- | --- | --- |
| Submission and pending Proposal | Authenticated producer input and one candidate transition | None |
| Verification Record | One scoped check over exact retained inputs, including its outcome and nonclaims | None |
| Decision | An attributed accept or reject action admitted through Repository authority | The only operation that can change Standing |
| Standing | The derived accepted scientific state reconstructed from admitted Events | Result of replay, not a separate approval step |

Replay verifies the retained objects, roots, signatures, authority history,
Artifacts, and derived Standing. It does not rerun the source-owned scientific
Method. Vela also does not choose or perform scientific work, replace native
tools or scientific judgment, or turn a successful check, signature, Git
commit, or Web view into acceptance.

## 1. Install the signed CLI

Vela `v0.977.6` is the current signed release for Linux x86-64 and macOS Apple
silicon. The exact-tag installer verifies the platform release manifest before
it installs the binary.

Before installing, provide:

| Requirement | Public install path |
| --- | --- |
| Platform | Linux x86-64 or macOS Apple silicon |
| Download and verification | `bash`, `curl`, `ssh-keygen`, and either `sha256sum` or `shasum` |
| Archive extraction | `tar` with gzip support on Linux; `ditto` on macOS |
| Network | HTTPS access to the pinned installer and GitHub release assets, unless using a retained mirror |
| Install location | A writable `/usr/local/bin`, `sudo`, or a writable `VELA_INSTALL_PREFIX` |

The rest of this guide also uses a complete Git client. Repository writes use
the standard OpenSSH agent as described in section 4.

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.977.6/install.sh | \
  VELA_VERSION=v0.977.6 bash

vela --version
```

Expected version:

```text
vela 0.977.6
```

## 2. Replay a public Repository

```bash
git clone https://github.com/vela-science/math.git math
git -C math checkout 5de716c896065c03c0a470d015ba2a328a527f73

vela authority trust pin math \
  --record-root sha256:efae3e02b5be6dfccf6701ebe26f87f00bb64f5b4372674e572a633844d95469 \
  --json
vela status math
vela claims math
vela replay math
```

The root is independently published in Vela's
[qualified authority fixture](../conformance/fixtures/authority/math-coh-00/trust-anchor.json),
not derived from the cloned Math Repository. Pinning grants no authority and
changes no Repository byte. This requires no Vela account, daemon, hosted
service, or Repository authority key.
Use a complete clone. Strict reads refuse shallow, partial, alternate, or
grafted Git object stores because missing Git history can mean missing
scientific history.

At the pinned commit, strict replay reports Repository root
`sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`,
3 accepted Claims, 6 Submissions, 6 Verification Records, and 0 pending
reviews.

Add `--json` to any read command when another program will consume the result:

```bash
vela status math --json
vela claims math --json
vela replay math --json
```

## 3. Trace one Decision

Use `why` to reconstruct the current Erdős 321 Standing:

```bash
vela why math \
  vcl_b9c6915de55e15c69d06b9aeed786b0e632986374a347d77ff447ad244f67a2e
```

The explanation binds:

- the exact current Claim and its correction predecessor;
- the authenticated Submission;
- the scoped Verification Record;
- the attributed Decision performer;
- the Repository authority event; and
- the derived accepted Standing.

It also retains what the record does not establish. The current Claim is a
bounded candidate answer for one exact Formal Conjectures occurrence, not a
proof resolving Erdős 321.

## 4. Create a bounded Repository

Skip this section if you only need to read existing Repositories.

### First-time authority key setup

Vela uses one dedicated Ed25519 key from the standard OpenSSH agent. It does
not create, read, or store the private key.

Create a key once if needed:

```bash
ssh-keygen -t ed25519 -a 64 -f ~/.ssh/vela-authority \
  -C "Vela repository authority"
```

Load it for the current login session on macOS:

```bash
ssh-add --apple-use-keychain ~/.ssh/vela-authority
```

Or load it for an eight-hour Linux agent session:

```bash
eval "$(ssh-agent -s)"
ssh-add -t 8h ~/.ssh/vela-authority
```

Inspect the full fingerprint, then initialize one bounded Repository:

```bash
ssh-add -l

vela init ./my-repository \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --check --json

vela init ./my-repository \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?"
```

If the agent exposes more than one Ed25519 key, add
`--key SHA256:<full-fingerprint>`. `init` creates the Profile, sequence-one
authority record, local trust anchor, and initial Git commit. It creates no
Claim, Verification, Decision, or scientific Standing.

The manual evidence and Method commits later in this guide require a Git author
identity. Configure it globally if that is your normal Git policy, or only for
this Repository:

```bash
git -C my-repository config user.name "<your name>"
git -C my-repository config user.email "<your email>"
```

Distribute the sequence-one authority-record root through an independent
trusted channel. Do not forward the authority-agent socket to remote,
untrusted, or proposal-supplied code.

`init --check` is read-only. Supply the same name and scope intended for
initialization; it checks the target, local runtime identity, and selected
OpenSSH-agent identity before any Profile, Git repository, journal, or trust
pin exists. Linux containers must provide a stable container-local
32-hex machine ID plus newline at `/etc/machine-id`; do not mount the host
machine ID. Missing or malformed identity fails as `vela.error.v1` with
`changed: false` and an actionable `error.code`.

## 5. Submit one bounded Result

Do the scientific work in its native tool. Track the exact evidence file in
the Repository, then submit the bounded Result:

```bash
vela submit --repo ./my-repository \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --requires-verification "<exact property the independent check must establish>" \
  --as agent:<producer> \
  --json
```

Record the returned `vpr_...` Proposal ID. `submit` authenticates producer
input and creates a pending Proposal. It cannot create a Verification Record,
Decision, Event, or accepted Standing.

Vela owns no work catalogue, planner, or scientific runner. Source
repositories, researchers, agents, and native tools decide what work to do.

## 6. Record a scoped Verification

Retain the Verification Method as tracked, clean Repository bytes before
recording the observation:

```bash
git -C my-repository add -- verification/method.json
git -C my-repository commit -m "Retain verification method"

vela verification record ./my-repository <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --independent-of agent:<producer> \
  --as verifier:<reviewer> \
  --json
```

The Method path must be Repository-relative, tracked, clean, and present in
the current Git commit. A passing Verification reports that its declared
property passed. It does not accept the Claim.

Only use `--independent-of` when the verifier is genuinely independent of the
named producer for the declared scope. Record shared dependencies with
`--shared-dependency` instead of manufacturing independence.

Optional starting Methods for common review scopes live in
[`examples/review-methods`](../examples/review-methods/).
Validate canonical bytes and their intended lifecycle bindings before running
the Method or creating a Verification Record:

```bash
vela verification check my-repository/verification/method.json \
  --profile exact-replay-v1 \
  --property "<exact registered property>" \
  --as verifier:<reviewer> \
  --does-not-establish "Scientific acceptance." \
  --json
```

This uses the same parser and profile, property, actor, and nonclaim checks as
`verification record`; it reads no Repository and changes no state.

## 7. Accept or reject the Proposal

Inspect the current Decision Inbox and its exact entry root:

```bash
vela review inbox ./my-repository --json
vela review show ./my-repository <vpr_id> --json
```

After the authorized reviewer has made the scientific judgment, record either
acceptance or rejection. Acceptance uses the exact current Inbox entry root:

```bash
vela review accept ./my-repository <vpr_id> \
  --reason "<bounded scientific reason>" \
  --if-entry-root sha256:... \
  --as human:<reviewer> \
  --json
```

Use `vela review reject` for the symmetric rejection path. `--as` records the
human or agent performer. It never grants authority or chooses the signing
key. The JSON response keeps performer, authority principal, authentication,
and signer separate.

Read back the result:

```bash
vela replay ./my-repository --json
vela claims ./my-repository --json
vela why ./my-repository <claim_id> --json
```

## 8. Publish with ordinary Git

Vela commits its canonical records locally. Publish them using the Repository's
normal Git policy:

```bash
git -C my-repository status
git -C my-repository log --oneline -5
git -C my-repository push
```

A Git push publishes bytes. It does not itself create scientific acceptance.

## Trust pins and recovery

Every governed read requires the sequence-one authority root obtained through
an independent channel:

```bash
vela authority trust pin ./my-repository \
  --record-root sha256:... \
  --json
```

The pin grants no authority and changes no Repository byte.

If a repository transaction is interrupted, inspect the exact retained
operation before recovering it:

```bash
vela recover --repo ./my-repository --inspect --json
vela recover --repo ./my-repository <operation_id> --json
vela replay ./my-repository --json
```

Recovery completes or aborts only the named durable transaction. It does not
continue the scientific command or publish Git state.

## Read next

- [CLI contract](CLI.md) for every shipped command and JSON schema.
- [Repository profile](REPOSITORY_PROFILE.md) for the source-owned layout.
- [Verification](VERIFICATION.md) for check and Decision boundaries.
- [Authority and attribution](SIGNING.md) before operating a signer.
- [Roots](ROOTS.md) for object, authority, and Repository identity.
- [Release guidance](RELEASES.md) for installer and recovery details.
- [Protocol 1](PROTOCOL.md) for normative semantics.
