---
name: vela-frontier
description: Working in a repository that has a .vela/ directory (a Vela frontier). Use when picking targets, running verifiers, writing receipts, landing results, or inspecting protected review. The producer loop is next → work → land; humans authorize exact decisions.
---

# Vela frontier work

Vela is Git-native, authority-scoped state for scientific work. Findings,
evidence, provenance, and proofs are bound to content-addressed, signed,
replayable events; everything else (frontier.json, proof packets, rollups) is a
derived view. Activity is not state: a script that ran is activity; a witness
plus a declared frozen verifier is evidence for that verifier's scoped result;
only an authorized accepted event changes frontier state. A repository with a
`.vela/` directory is a frontier, and this skill is how an agent works inside
one.

## The loop

Three producer verbs; everything else is reading, review, or plumbing.

```text
next -> work -> land
```

- `vela next --json` — the offer: ranked open targets with the compounding
  payload pre-loaded (premises to build on, banked routes, prior attempts,
  dead channels). Returns `{targets: [{lane, id, title, why, next_command, task?}]}`.
  Trust the ranking; it already encodes what the frontier knows.
- `vela work <target> --as agent:<you> --json` — claim the lease, load the
  briefing, and write one typed private `session.json` under `.vela/work/`.
  A same-actor retry returns that exact active session without another lease
  event. Read the returned briefing before working; do not edit the session
  record.
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
- `vela review list . --json` — the pending queue, newest first. Each compact
  row includes `created_at`; use `vela review show . <vpr_id> --json` for one
  exact pending Decision Brief or signed terminal decision record.
- When a task supplies a full `vpr_` ID, start with `vela review show`; it
  returns the pending brief or signed terminal decision. Rejected proposal
  findings are intentionally absent from accepted `finding show` and `log`
  views. That absence is not deletion.
- `vela review decide . <vpr_id> --accept|--reject --reason <text> --json` — a
  key-free exact Decision Plan. An agent may prepare and invoke the protected
  request, but only the registered human may approve its decision card.

For a frozen-verifier witness, run `vela reproduce <witness>` first, then land
the result through the active work session with `vela land --work <target>
--artifact <witness>:witness --as agent:<you> --json`. A producer outside the
frontier can instead emit the same portable Receipt v1 and call `vela land
receipt.json`. Both paths cross the one Receipt/land write edge. Retired CI
auto-verdicts are not an authority path.

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

Era-0 landing routes by a retained signed policy:

- **Permit** — admitted canonically with no key ceremony. The human's
  authority arrived earlier, once, as the retained policy signature; the event carries
  the certificate and replay verifies it.
- **Defer** — parked for an exact protected human `review decide` decision.
- **Deny** — refuses canonical admission. The returned structured result states
  what, if anything, was retained; zero-delta Deny is not assumed by clients.

Truth-bearing claims stay human-keyed in every mode. There is no configuration
in which an agent's proposal becomes accepted state without a human key.

## Custody

- Never run legacy `sign`. You may inspect `review list` and `review show`, and
  prepare one exact `review decide` request. Never approve its decision card or
  claim that requesting it caused acceptance.
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
policy state and Permit readiness, sign-queue depth, compounding metrics).
`vela finding show <dir> <vf_id> --view standing` is one finding's claim-state
cell; `vela log <dir> <vf_id>` is its history. The MCP
server (`vela serve . --profile draft`) exposes the read surface plus the
non-finalizing `work` tool (claim|land|drop);
`decide` is excluded by construction, so nothing an agent does through MCP
finalizes state.
