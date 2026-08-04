# Vela quickstart

Vela is version control for scientific state. Git publishes exact bytes;
agents submit authenticated evidence; Verification Records report scoped
checks; only an authorized human Decision changes Standing.

## Read an existing Frontier

```bash
git clone <frontier-url>
cd <frontier>
vela check . --json
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

This local public pin grants no authority and changes no Frontier byte.

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

Inspect the resulting objects without writing:

```bash
vela show . <vsb_or_vpr_id> --json
vela why . <claim_id> --json
```

## Create a new Frontier

```bash
vela init ./frontier \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --json
```

Before running `init`, load one dedicated Ed25519 repository-authority key into
your standard OpenSSH agent with per-use confirmation (`ssh-add -c`). `init`
creates the Profile, root-bound sequence-1 authority record, local trust anchor,
and initial Git commit in one command. It creates no Claim, Decision, or
scientific Standing. When the agent exposes more than one Ed25519 identity,
add `--key SHA256:<full-fingerprint>`. If signing is unavailable, load the key
and rerun the same `vela init ./frontier --json`; the retained Profile makes
that retry safe. Vela reads no private-key file. Do not expose the authority
agent socket to an agent Campaign.

## Predecessor repositories

Predecessor tags and archives retain the old source and executable needed to
verify their bytes. Never hand-edit or relabel an old checkout. The current CLI
does not expose predecessor parsers, migration writers, or predecessor writer
commands.

## What to read next

- Producers and agents: [AGENT_QUICKSTART.md](AGENT_QUICKSTART.md) and
  [PRODUCER_QUICKSTART.md](PRODUCER_QUICKSTART.md)
- Commands: [CLI.md](CLI.md)
- Repository layout: [FRONTIER_REPOSITORY_PROFILE.md](FRONTIER_REPOSITORY_PROFILE.md)
- Authority and attribution: [SIGNING.md](SIGNING.md)
- Byte and root meanings: [ROOTS.md](ROOTS.md)
- Protocol semantics: [PROTOCOL.md](PROTOCOL.md)
