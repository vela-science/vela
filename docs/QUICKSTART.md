# Vela quickstart

Vela is version control for scientific state. Git publishes exact bytes;
agents submit authenticated evidence; Verification Records report scoped
checks; only an authorized human Decision changes Standing.

## Read an existing repository

```bash
git clone <repository-url>
cd <repository>
vela replay . --json
vela status . --json
vela reproduce .
```

Strict checking validates the current repository origin, exact Git anchors and
ancestry, retained canonical objects, authority-history continuity, and the
consumer's independent sequence-one trust anchor. Obtain the full first
authority-record root through an independent channel and install it with:

```bash
vela authority trust pin . --record-root sha256:... --json
```

This local public pin grants no authority and changes no repository byte.

## Produce one bounded result

```bash
vela next . --limit 1 --json
# Optional: print the exact stateless Target briefing.
vela start <target> --json

# Run the exact verifier and retain its artifact.

vela submit \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --as agent:<name> \
  --json
```

`next` returns Offers from the fresh Target Index v5. `start` revalidates one
exact Target and prints a write-free briefing; the native agent, workbench, or
scientific tool performs the work. `submit` authenticates and retains the
resulting Submission and creates a pending Proposal. It does not create
Verification, a Decision, an Event, or accepted scientific state.

## Verify and decide

Retain the exact source-local verification method before recording its result:

```bash
git add -- verification/method.json
git commit -m "Retain verification method"

vela verification record . <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --independent-of agent:<producer> \
  --as verifier:<name> \
  --json
```

The method path must be repository-relative, tracked, clean, and present in the
current Git commit. This makes the observed method bytes reproducible; a
passing Verification still does not accept the Claim.

Inspect the consequence-complete packet and use its exact entry root:

```bash
vela review inbox . --json
vela review show . <vpr_id> --json

# Authorized operator only; reject is the symmetric alternative.
vela review accept . <vpr_id> \
  --reason "<bounded scientific reason>" \
  --if-entry-root sha256:... \
  --json

vela replay . --json
vela why . <claim_id> --json
```

Only `review accept` or `review reject` changes Standing. A producer or
verifier hands the rooted inbox entry to the authorized operator rather than
making that Decision.

Inspect the resulting objects without writing:

```bash
vela show . <vsb_or_vpr_id> --json
vela why . <claim_id> --json
```

## Create a new repository

### First-time authority key setup

Vela uses one dedicated Ed25519 key from the standard OpenSSH agent. Vela
does not create, read, or store the private key. If you do not already have a
dedicated key, create one once with a passphrase:

```bash
ssh-keygen -t ed25519 -a 64 -f ~/.ssh/vela-authority \
  -C "Vela repository authority"
```

Load it once for the current login session on macOS:

```bash
ssh-add --apple-use-keychain ~/.ssh/vela-authority
```

Or start an agent and load the key for an eight-hour session on Linux:

```bash
eval "$(ssh-agent -s)"
ssh-add -t 8h ~/.ssh/vela-authority
```

`ssh-add -l` should now list the key's full SHA256 fingerprint. If the agent
contains multiple Ed25519 keys, pass the intended full fingerprint to
`vela init --key SHA256:...`. Do not use OpenSSH's per-signature `-c` option;
Vela performs its own exact policy, current-root, read-set, and local signature
checks for every Decision.

### Initialize the repository

```bash
vela init ./my-repository \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --json
```

Before running `init`, load one dedicated Ed25519 repository-authority key once
for the current operating-system session. `init`
creates the Profile, root-bound sequence-1 authority record, local trust anchor,
and initial Git commit in one command. It creates no Claim, Decision, or
scientific Standing. When the agent exposes more than one Ed25519 identity,
add `--key SHA256:<full-fingerprint>`. If signing is unavailable, load the key
and rerun the same `vela init ./my-repository --json`; the retained Profile
makes that retry safe. Vela reads no private-key file. Do not forward the
authority agent socket to remote or untrusted code.

## Predecessor repositories

Predecessor tags and archives retain the old source and executable needed to
verify their bytes. Never hand-edit or relabel an old checkout. The current CLI
does not expose predecessor parsers, migration writers, or predecessor writer
commands.

## What to read next

- Producers and agents: [AGENT_QUICKSTART.md](AGENT_QUICKSTART.md) and
  [PRODUCER_QUICKSTART.md](PRODUCER_QUICKSTART.md)
- Commands: [CLI.md](CLI.md)
- Repository layout: [REPOSITORY_PROFILE.md](REPOSITORY_PROFILE.md)
- Authority and attribution: [SIGNING.md](SIGNING.md)
- Byte and root meanings: [ROOTS.md](ROOTS.md)
- Protocol semantics: [PROTOCOL.md](PROTOCOL.md)
