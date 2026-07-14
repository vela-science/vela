---
name: vela-frontier
description: Working in a repository that has a .vela/ directory (a Vela frontier). Use when picking targets, running verifiers, writing receipts, or landing results — any time vela, receipt.json, frontier state, or the sign queue comes up. The loop is next → work → land; humans sign.
---

# Vela frontier work

Vela is version control for scientific state. Findings, evidence, provenance,
and proofs live as content-addressed, signed, replayable events; everything
else (frontier.json, proof packets, rollups) is a derived view. Activity is not
state: a script that ran is activity; a witness plus a frozen verifier anyone
can re-run is state. A repository with a `.vela/` directory is a frontier, and
this skill is how an agent works inside one.

## The loop

Four verbs; everything else is reading or plumbing.

```text
next -> work -> land -> sign
```

- `vela next --json` — the offer: ranked open targets with the compounding
  payload pre-loaded (premises to build on, banked routes, prior attempts,
  dead channels). Returns `{targets: [{lane, id, title, why, next_command}]}`.
  Trust the ranking; it already encodes what the frontier knows.
- `vela work <target> --as agent:<you> --json` — claim the lease, load the
  briefing, and write one typed private `session.json` under `.vela/work/`.
  Read the returned briefing before working; do not edit the session record.
- `vela land --work <target> --claim <result> --artifact <path>:<kind>
  --caveat <limit> --as agent:<you> --json` — build Receipt v1 from the exact
  session and use the shared write edge. With exactly one active session for
  this actor, `--work` is inferred. A committed Permit or Defer closes only
  `session.json`; Deny or invalid input keeps it for repair. Foreign producers
  may still use `vela land receipt.json`.
- `vela work <target> --drop --reason <why> --as agent:<you> --json` — sign a
  same-owner zero-TTL lease update, then remove private scratch. Deleting files
  by hand does not release a lease.
- `vela artifact retract <frontier> <va_id> --as agent:<you> --reason <why>
  --json` — draft retirement of a malformed or obsolete artifact. It remains
  pending; only the human ceremony may remove its active proof-readiness weight.
- `vela sign` — the one human ceremony. Not yours: it refuses `agent:` actors
  (exit 4) by design.

For the single-witness producer case — contributing one frozen-verified beat to
a frontier you do not maintain (a fork PR) — reach for `vela submit <witness>
--as agent:<you> --json` instead of driving `land` by hand. It is one
transactional verb: reproduce the witness, land it, bind it to the finding as a
content-addressed artifact, admit it through the exact lane, and materialize —
in that order. It writes locally; `--push` publishes. The CI counterpart that
decides the merge is `vela ci verdict` (it runs in the frontier's Action, not
here). `submit` is porcelain over `land`; `land` stays the general write edge.

Every verb takes `--json` and returns one object with `ok` and `command`; no
prose leaks into a JSON stream. Exit codes: 0 ok, 1 domain failure, 2 usage,
3 not found, 4 custody refused, 5 already exists. The frontier argument is
discovered by walking upward from the current directory, like git.

## The receipt

A Vela Receipt (`vela.receipt.v1`) is the portable JSON any tool exports —
a notebook, a search run, a proof attempt:

```json
{
  "schema": "vela.receipt.v1",
  "claim": "what is now known / bounded / refuted",
  "type": "computational | theoretical | empirical | negative",
  "artifacts": [{"path": "witness.json", "kind": "witness"}],
  "caveats": ["what this does NOT establish"],
  "verifier_runs": [{"method": "…", "outcome": "pass", "log": "…"}]
}
```

The claim is one scoped sentence a skeptical reviewer could sign against.
Artifact paths must exist — they are hashed at land time. Caveats state what
the work does not establish; write at least one unless the claim is genuinely
unconditional. Verifier runs record only what actually ran, with honest
outcomes. Negative results (channel exhausted, bound unimproved) are landable
state and save the next session the respend.

## Landing routes

Landing routes by the frontier's signed policy:

- **Permit** — admitted canonically with no key ceremony. The human's
  authority arrived earlier, once, as the policy signature; the event carries
  the certificate and replay verifies it.
- **Defer** — parked in the human's sign queue for `vela sign`.
- **Deny** — refuses canonical admission. The returned structured result states
  what, if anything, was retained; zero-delta Deny is not assumed by clients.

Truth-bearing claims stay human-keyed in every mode. There is no configuration
in which an agent's proposal becomes accepted state without a human key.

## Custody

- Never run the decision verbs bare: `sign`, `accept`, `review`, and
  `proposals reject` are key-custody human acts, and the engine refuses
  `agent:` actors on them.
- Every write carries an explicit acting identity: `--as agent:<you>`, or set
  `VELA_ACTOR_ID=agent:<you>` for the session. Never write as a human.
- Never sign anything, never read or handle key material, never sit in a
  trust path. A model may produce a candidate witness; only a frozen verifier
  may check it, and only a key-holding human accepts a truth-bearing proposal.
- Never pre-fill a verdict the human did not explicitly give. Presenting
  evidence is yours; the judgment is not.
- Artifact retirement preserves the record and its historical audit issues. It
  does not retract or judge the truth or quality of linked findings.
- Never hand-edit accepted events or derived views (`frontier.json`, proof
  packets); regenerate with `vela frontier materialize`. Never bulk-move
  Vela-canonical paths (`examples/`, `projects/`, `lean/`, `.vela/`).

## The gate

Frontier repos carry a conformance gate. When the harness supports suites, run
the suites selected from the affected paths and require 0 FAIL in each one.
Trust-path changes run every deterministic suite selected from their affected
paths. Release certification runs the deterministic full Vela union. Live
network and platform-pinned adapter checks stay explicit and cannot block an
unrelated Vela release. A selected suite fails if a required verifier toolchain
is absent; a non-selected suite is not a pass.
`vela check . --strict` is the same frontier-state bar the hub's ingestor
enforces. `vela reproduce <frontier>` re-runs the frozen verifiers over stored
witnesses from scratch. Run it before claiming a reproduction, and never
silently break the reproduction of a banked result.

## Reading state

`vela status --json` is the one-screen summary (findings, replay integrity,
policy mode, sign-queue depth, compounding metrics). `vela state <vf_id>` is
one finding's claim-state cell; `vela log <dir> <vf_id>` its history. The MCP
server (`vela serve . --profile draft`) exposes the read surface plus the
non-finalizing writes (`propose`, `work` — claim|land|drop|deposit) as tools;
`decide` is excluded by construction, so nothing an agent does through MCP
finalizes state.
