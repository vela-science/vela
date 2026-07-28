---
name: vela-frontier
description: Working in a repository that has a .vela/ directory (a Vela Frontier). Use when inspecting standing, starting Attempts, submitting evidence, importing Verification Records, or reviewing exact Proposals. The producer loop is next → start → submit; only authorized Decisions change standing.
---

# Vela frontier work

Vela is Git-native, authority-scoped state for scientific work. Claims,
evidence, provenance, and proofs are bound to content-addressed, signed,
replayable events; everything else (frontier.json, proof packets, rollups) is a
derived view. Activity is not state: a script that ran is activity; a witness
plus a declared frozen verifier is evidence for that verifier's scoped result;
only an authorized accepted event changes frontier state. A repository with a
`.vela/` directory is a frontier, and this skill is how an agent works inside
one.

## The loop

Three producer verbs; everything else is inspection, verification, decision,
or plumbing.

```text
next -> start -> submit
```

- `vela next --json` — the offer: ranked open targets with the compounding
  payload pre-loaded (premises to build on, banked routes, prior attempts,
  dead channels). Returns `{targets: [{lane, id, title, why, next_command, task?}]}`.
  Trust the ranking; it already encodes what the frontier knows.
- `vela start <target> --as agent:<you> --json` — claim the lease, load the
  briefing, and write one typed private Attempt under `.vela/work/`.
  A same-actor retry returns that exact active Attempt without another lease
  event. Read the returned briefing before working; do not edit the Attempt
  record.
- `vela submit --attempt <vat_id> --claim <result> --type <type>
  --replayability <class> --artifact <path>:<kind> --caveat <limit>
  --as agent:<you> --json` — build and register Submission v1 from the exact
  Attempt. Registration creates a pending Proposal and no accepted-state
  change. A foreign producer may pass one signed `submission.json`.
- `vela start <target> --drop --reason <why> --as agent:<you> --json` — sign a
  same-owner zero-TTL lease update, then remove private scratch. Deleting files
  by hand does not release a lease.
- `vela artifact retract <frontier> <va_id> --as agent:<you> --reason <why>
  --json` — draft retirement of a malformed or obsolete artifact. It remains
  pending; only the human ceremony may remove its active proof-readiness weight.
- `vela review list . --json` — the pending queue, newest first. Each compact
  row includes `created_at`; use `vela review show . <vpr_id> --json` for one
  exact pending Review Packet or signed terminal Decision record.
- When a task supplies a full `vpr_` ID, start with `vela review show`; it
  returns the pending Review Packet or signed terminal Decision. Rejected
  Proposals remain inspectable; they do not create accepted Claim standing.
- `vela review accept . <vpr_id> --reason <text> --json` or
  `vela review reject . <vpr_id> --reason <text> --json` prepares one exact
  Decision Plan and executes it through repository authority. An agent may
  prepare or explain the exact command, but may not invoke either action.

For a frozen-verifier witness, run `vela reproduce <witness>` first, then
submit the result through the active Attempt with `vela submit --attempt <vat_id>
--artifact <witness>:witness --as agent:<you> --json`. A producer outside the
frontier can emit the same portable Submission v1 and call `vela submit
submission.json`. Producer checks remain producer-reported; an independent
Verification Record is separate.

Every verb takes `--json` and returns one object with `ok` and `command`; no
prose leaks into a JSON stream. Exit codes: 0 ok, 1 domain failure, 2 usage,
3 not found, 4 custody refused, 5 already exists. The frontier argument is
discovered by walking upward from the current directory, like git.

## The submission

A Vela Submission (`vela.submission.v1`) is authenticated producer input from
a notebook, search run, proof attempt, or research harness. It can request a
change, but it cannot assert Standing, mint Verification, or create a Decision:

```json
{
  "schema": "vela.submission.v1",
  "claim": {
    "assertion": "what is now known / bounded / refuted",
    "type": "computational",
    "conditions": ["the exact bounded range"]
  },
  "artifacts": [{
    "path": "witness.json",
    "kind": "witness",
    "digest": "sha256:..."
  }],
  "caveats": ["what this does NOT establish"],
  "producer_checks": [{
    "method": "local replay",
    "outcome": "pass",
    "authority": "producer_reported"
  }]
}
```

The claim is one scoped sentence a skeptical reviewer could sign against.
Artifact paths must exist and match their full digest. Caveats state what the
work does not establish. Producer checks report only what the producer ran;
independent Verification is a separate signed record. Bounded negative results
remain useful submissions and save the next Attempt from repeating them.

## Registration and decision

`vela submit` registers the exact Submission and a pending Proposal. It returns
a Vela-issued Registration Record proving intake, not truth. Independent
Verification Records may then attach scoped results. Only `review accept` or
`review reject` creates an authorized Decision; registration and verification
never imply acceptance.

## Custody

- Never run legacy `sign`. You may inspect `review list` and `review show`, and
  prepare or explain one exact `review accept` or `review reject` command.
  Never invoke either action, access repository-authority credentials, or
  claim that preparing a command caused a Decision.
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

`vela status --json` is the one-screen summary of Claims, replay integrity,
policy state, review depth, and the next bounded action.
`vela show <dir> <typed_id>` inspects an exact object; `vela why <dir>
<claim_id>` explains its standing; `vela log <dir>` reads canonical history. The MCP
server (`vela serve . --profile draft`) exposes the read surface plus the
non-finalizing Attempt and Submission tools;
`decide` is excluded by construction, so nothing an agent does through MCP
finalizes state.
