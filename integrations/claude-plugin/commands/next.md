---
description: The offer — ranked frontier targets; pick one and open a work session
argument-hint: "[target id]"
---

# /vela:next

Present the frontier's offer and open a work session on the target the user picks.

1. Run `vela next --json`. It returns
   `{targets: [{lane, id, title, why, next_command}]}`.
2. If `$ARGUMENTS` names a target id, skip the menu and go straight to step 4
   with it.
3. Render the offer conversationally — for each target (top five at most), one
   short paragraph: the id, what it is, and why it ranks where it does. The
   ranking already encodes the compounding payload (banked routes, dead
   channels, prior attempts), so trust the order; do not re-rank. Then ask
   which one to take (AskUserQuestion is natural here). "None" is a fine
   answer — stop there.
4. On a pick, open the session:

   ```
   vela work <target> --as agent:claude --json
   ```

   Use `$VELA_ACTOR_ID` as the identity if it is set; otherwise `agent:claude`.
   Agent writes always carry an explicit `agent:` identity.
5. Summarize the briefing: the claimed target and lease, the session directory
   (`.vela/work/<target>/`, with `offer.json` holding the full briefing), and
   the payload highlights — premises available to build on, banked routes,
   prior attempts, dead channels to avoid. Close with the shape of what comes
   next: do the work, then `/vela:land` to write the receipt and cross it into
   state. If the lease is already held, report whose it is and offer
   `vela work <target> --drop` only if the user says the session is theirs to
   release.
