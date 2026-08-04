# Agent quickstart

The product loop is:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

An ordinary producer agent may inspect the map, select one bounded Target, do
the work in its native environment, and submit evidence. A separately declared
verifier may report its scoped check. Only a human authority may decide whether
the proposed change enters Standing; no agent receives that authority from a
producer or verifier role.

For a new repository, initialization is deliberately structural:

```bash
vela init ./frontier --name "<name>" --scope "<bounded question>" --json
```

It creates no MCP configuration, verifier, policy, or authority. `vela status`
will report `Authority: not configured`; canonical Submission intake
remains fail-closed until the repository's standard authority profile is
provisioned.

```bash
vela status . --json
vela next . --limit 1 --json
# Optional write-free Target briefing.
vela start <target> --frontier . --json

vela submit --frontier . \
  --claim "<scoped result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json
```

The agent, workbench, notebook, proof assistant, or laboratory system performs
the work directly. Vela does not wrap it. `start` writes no Attempt, lease,
budget, or workflow state; it only returns the exact Target briefing. The
producer retains native files and submits only the
bounded artifact and Claim needed for review. Benchmarks use Harbor tasks
directly rather than a Vela-owned runner.

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
