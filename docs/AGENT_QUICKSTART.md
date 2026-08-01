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

Agents may reproduce evidence and explain these links. They may not invoke
repository-authority decisions, use authority credentials, treat a verifier
pass as acceptance, or rewrite retained history.
