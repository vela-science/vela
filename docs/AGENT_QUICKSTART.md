# Agent quickstart

The product loop is:

```text
map -> target -> run -> verify -> commit -> compound
```

An ordinary producer agent may map state, select one bounded Target, run the
work, and submit evidence. A separately declared verifier may report its scoped
check. No agent receives Frontier commit authority from either role.

For a new repository, initialization is deliberately structural:

```bash
vela init ./frontier --name "<name>" --scope "<bounded question>" --json
```

It creates no MCP configuration, verifier, policy, or authority. `vela status`
will report `Authority: not configured`; canonical Submission registration
remains fail-closed until the repository's standard authority profile is
provisioned.

```bash
vela status . --json
vela next . --limit 1 --json
vela start <target> --frontier . --as agent:<name> --json

# Optional: let the private, removable Agent helper produce and check the
# bounded artifact without receiving repository authority.
# The helper is source-only while its deletion test remains open. From a Vela
# source checkout, build it once and point the CLI at the exact bundle:
bun install --frozen-lockfile
bun run --cwd packages/canopus build
export VELA_AGENT_BIN="$PWD/packages/canopus/dist/vela-agent"

vela agent doctor
vela agent run --attempt <vat_id>
vela agent show <run.json>
vela agent replay <run.json>
vela agent export <run.json>

vela submit --frontier . \
  --attempt <vat_id> \
  --claim "<scoped result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json
```

Then inspect the exact objects:

```bash
vela show . <object_id> --json
vela review show . <vpr_id> --json
vela why . <claim_id> --json
```

The Submission is producer input. The Registration Record says Vela retained
it. A Verification Record reports one scoped check. A Proposal requests a
change. A Decision authorizes or refuses that change. An Event enters canonical
history. Standing is derived from replay.

Agents may reproduce evidence and explain these links. They may not invoke
repository-authority decisions, use authority credentials, treat a verifier
pass as acceptance, or rewrite retained history.
