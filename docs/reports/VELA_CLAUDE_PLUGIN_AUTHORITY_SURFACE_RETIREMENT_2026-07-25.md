# Claude plugin authority-surface retirement

**Date:** 2026-07-25  
**Decision:** remove the obsolete agent-mediated batch-signing workflow before
the final active Frontier migration  
**Protocol effect:** none  
**Frontier effect:** none

## Problem

The maintained Claude Code plugin still taught a superseded authority path:

- `/vela:review` collected human verdicts into
  `.vela/sign-session.json`;
- `/vela:sign-prep` inspected saved answers and binary pins; and
- the session hook and plugin documentation directed the user toward the
  legacy batch `vela sign` ceremony.

That surface contradicted the generated Vela agent charter, which permits
agents to inspect exact review records but forbids them from participating in
a human-key trust path. It also duplicated the current object-first review
surface and retained pre-0.9 product language.

The workflow is not required to replay any Era-0 event, verify any
AcceptancePolicy certificate, or perform the temporary sequence-1
repository-authority migration. It was therefore safely removable before the
Erdős ceremony.

## Change

The plugin now exposes four commands only:

```text
status
next
land
review
```

`review` is read-only. It uses `vela review list`, `review show`, and
`review preview`; it writes no answer or session file and invokes no decision
or signing command. `status` reads compact status, review, and offer
projections. The session hook uses `vela.status.v1` fields directly and
preserves diagnostic output when strict state is blocked.

`sign-prep.md` is deleted. A prelaunch regression rejects that path and any
return of `vela sign`, `sign-session.json`, `id pin-binary`, or `sign-prep`
inside the active Claude plugin.

The plugin metadata advances from its stale `0.760.1` declaration to the exact
current `0.930.0-rc.7` candidate identity. This is source alignment, not a
separate plugin release.

## Preserved boundaries

The change does not remove:

- Era-0 event, policy, proposal, Receipt, or decision parsing;
- the temporary protected legacy continuity signature required by
  `vela authority migrate`;
- the read-only `review list`, `show`, and `preview` contracts;
- Vela protocol conformance fixtures; or
- canonical Frontier or scientific-state bytes.

The `vela-signer` helper, protected identity, rebind, and migration writer
remain only because Erdős has not crossed the sequence-1 boundary. ADR 0020
still requires their deletion before final `0.930.0`.

## Verification

```text
plugin JSON:                         valid
session hook shell syntax:           valid
prelaunch surface:                   pass
retired plugin authority patterns:   absent
plugin command count:                4
git diff check:                       pass
```

A real read-only hook smoke over Erdős with released Vela `0.915.1` reported:

```text
Vela frontier: erdos-frontier
State: 2770 findings, replay reproduced, strict blocked, policy absent,
       15 pending proposal(s).
Loop: next -> work -> land; accountable principals authorize.
```

The smoke made no write, did not read a key, and preserved the exact canonical
Frontier.
