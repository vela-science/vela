---
description: Read-only proposal inspection — list compact records and open one exact Review Packet
argument-hint: "[proposal id]"
allowed-tools: Bash(vela review list:*), Bash(vela review show:*), Bash(vela review diff:*)
---

# /vela:review

Inspect the review queue without entering an authority path. Three invariants
override everything else in this command:

- **Read only.** Never sign, decide, mutate a proposal, write a session file,
  or touch a key.
- **No verdict inference.** Never suggest, default, pre-select, or record an
  accept/reject action. Verifier success is not scientific acceptance.
- **Keep objects distinct.** A Proposal, retained Submission, Verification Records,
  and terminal authority record are separate records with separate roots.

Steps:

1. Fetch compact pending records:
   `vela review list . --limit 50 --json`.
2. If `items` is empty, say the pending queue is clear and stop.
3. If `$ARGUMENTS` contains one full `vpr_` id, open exactly that record with
   `vela review show . <vpr_id> --json`. Otherwise present compact rows and ask
   which proposal the user wants to inspect.
4. For the selected proposal, report:
   - proposal id, actor, standing, claim, type, and recorded time;
   - Submission and Artifact availability;
   - Verification Records and their exact scoped outcomes;
   - caveats and Engine blockers;
   - Proposal, Submission, evidence, and terminal-Decision roots when present; and
   - the route's stated next action.
5. If the user asks to inspect the pending Review Packet exactly as the
   decision planner sees it, run
   `vela review diff . <vpr_id> --json`.
6. Stop after inspection. If the user explicitly gives a semantic decision,
   state that authorization is outside this plugin and name the exact proposal;
   do not convert the statement into a saved answer or invoke a mutating path.
