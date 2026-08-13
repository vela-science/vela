# Agent quickstart

The product loop is:

```text
MAP -> ADVANCE -> REMAP
```

Map reads Problems, Claims, Standing, dependencies, Corrections, and open
Obligations from exact roots. Advance means native work that may produce a
bounded proposed change. Remap replays Standing and derives what remains
current, affected, blocked, or open. The exact operator loop inside it is:

```text
init -> submit -> verify -> decide -> replay
```

An ordinary producer agent may inspect the map, select one bounded obligation,
do the work in its native environment, and submit evidence. A separately
declared verifier may report its scoped check. An eligible human or agent may
decide whether the proposed change enters Standing through Repository
authority. No performer receives that authority merely from a producer or
verifier role; an agent Decision must name its `agent:` identity and should bind
the source-owned session or checkpoint with `--session-ref`.

For a new repository, initialization installs its repository filesystem state
through one recoverable transaction. Its Git commit and local trust anchor are
separate post-transaction steps and are never performed by `vela recover`.
Load a dedicated Ed25519 identity into the standard OpenSSH agent, then run:

```bash
vela init ./my-repository --name "<name>" --scope "<bounded question>" --json
```

`vela init` creates the Profile, repository origin, initial keyset and
authorization model, sequence-one authority record, local trust anchor, and
initial Git commit. It creates no MCP configuration, verifier, Claim, Decision, or
scientific Standing. If signing is unavailable, load the key and rerun the
same command; the retained Profile makes that retry safe. When more than one
Ed25519 identity is loaded, select the intended full fingerprint with
`--key SHA256:...`.

```bash
vela status . --json

vela submit --repo . \
  --claim "<scoped result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json
```

The agent, workbench, notebook, proof assistant, or laboratory system performs
the work directly. A source-owning Repository or read product may expose an
exact next obligation and rooted packet under its own contract. Vela does not
wrap that work, own a work catalogue or planner, or publish `next`/`start`; the
producer retains native files and submits only the bounded artifact and Claim
needed for review. Benchmarks use Harbor tasks directly rather than a Vela-owned runner.

Then inspect the exact objects:

```bash
vela show . <object_id> --json
vela review show . <vpr_id> --json
vela why . <claim_id> --json
```

The signed Submission is retained producer input, and the Proposal binds its
exact bytes. A Verification Record reports one scoped check. A Proposal requests a
change. A Decision authorizes or refuses that change. An Event enters canonical
history. Standing is derived from replay.

Agents may reproduce evidence and explain these links. A trusted native agent
may invoke an exact repository-authority Decision only when the operator has
explicitly authorized that named Decision or campaign. It must use the current
Inbox root and the standard provider, preserve every policy and semantic
check, and report the resulting authority record. It may not infer authority
from a verifier pass, broaden the authorized scope, or rewrite retained
history.
