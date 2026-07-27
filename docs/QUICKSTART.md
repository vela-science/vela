# Vela quickstart

Vela is version control for scientific state. Git publishes exact bytes;
agents submit authenticated evidence; Verification Records report scoped
checks; an authorized Decision changes standing.

## Read an existing Frontier

```bash
git clone <frontier-url>
cd <frontier>
vela check . --strict --json
vela status . --json
vela reproduce .
```

Strict checking validates the current repository epoch, exact Git anchors and
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
vela start <target> --as agent:<name> --json

# Run the exact verifier and retain its artifact.

vela submit --attempt <vat_id> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --as agent:<name> \
  --json
```

`next` returns Offers from the fresh Target Index v2. `start` binds one exact
Target and packet into an Attempt. `submit` registers the resulting Submission,
issues a Registration Record, and creates a pending Proposal. It does not
create Verification, a Decision, an Event, or accepted scientific state.

Inspect the resulting objects without writing:

```bash
vela show . <vsb_or_vrr_or_vpr_id> --json
vela why . <claim_id> --json
```

## Create a new Frontier

```bash
vela init ./frontier \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --json
```

The result is a minimal current repository with structural identity and no
scientific decision. Load one dedicated Ed25519 repository-authority key into
your standard OpenSSH agent, then establish the writer:

```bash
vela authority init ./frontier \
  --reason "Establish the repository writer for this bounded Frontier." \
  --json
```

When the agent exposes more than one Ed25519 identity, add
`--key SHA256:<full-fingerprint>`. Vela reads no private-key file. The command
is valid only over untouched structural genesis, creates one root-bound
sequence-1 authority record, and enables routine signed-agent work. It grants
no scientific standing. Distribute the returned full authority-record root
independently, then install the local consumer pin before any later
repository-authority write:

```bash
vela authority trust pin ./frontier \
  --record-root sha256:<full-sequence-1-root> \
  --json
```

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
