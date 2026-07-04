---
description: Author a Vela receipt from the work in context, confirm it, land it
argument-hint: "[receipt path]"
---

# /vela:land

Cross the work in this session from activity into state. A landing is only as
good as its receipt — write the receipt as if a skeptical reviewer will read
nothing else.

1. From the conversation and workspace, draft a `vela.receipt.v1`:

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

   - `claim` — one sentence, concrete and scoped. This is what a human will
     eventually sign against; no reach beyond what the artifacts support.
   - `type` — computational, theoretical, empirical, or negative. Negative
     results (a channel exhausted, a bound not improved) are landable state.
   - `artifacts` — every file the claim leans on: witnesses, logs, proofs.
     Paths must exist; they are hashed at land time.
   - `caveats` — what the work does not establish. Write at least one unless
     the claim is genuinely unconditional; a missing caveat is the classic
     failure mode.
   - `verifier_runs` — only runs that actually happened (`vela reproduce`,
     `vela-verify`, test suites), with honest outcomes. Never invent a run.

2. SHOW the receipt JSON in chat and confirm with the user before writing
   anything (AskUserQuestion or a plain question). Apply their edits.
3. Write it to `receipt.json` (or the path in `$ARGUMENTS`, or one the user
   prefers).
4. Land it. Prefer the MCP `work` tool when the vela-local server is attached
   (the plugin's `.mcp.json` serves the draft profile, which exposes it):
   call `work` with `action: "land"`, the receipt path, and the agent
   identity. If the MCP tool is unavailable, shell out:

   ```
   vela land receipt.json --as agent:<name> --json
   ```

   Identity: `$VELA_ACTOR_ID` if set, else `agent:claude`. Agent writes always
   carry an explicit `agent:` identity — never land bare. Both paths are the
   same write edge: the receipt is hashed, a pending record lands, and the
   frontier's signed policy routes it.
5. Report the route from the JSON result:
   - **policy_admitted** — the signed policy ruled; the record is canonical
     state with no ceremony needed. Name the policy id if the output carries
     one.
   - **deferred** — parked in the human's sign queue. Point at `/vela:review`
     to triage and `vela sign` (theirs to run, not yours) to finish.
   - **denied or error** — report exactly what the CLI said, including
     `error.hint`. Exit codes: 1 domain failure, 4 custody refused.
