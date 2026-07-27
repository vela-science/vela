---
description: The offer — ranked frontier targets; pick one and start an Attempt
argument-hint: "[target id]"
---

# /vela:next

Present the frontier's offer and start an Attempt on the target the user picks.

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
   vela start <target> --as agent:claude --json
   ```

   Use `$VELA_ACTOR_ID` as the identity if it is set; otherwise `agent:claude`.
   Agent writes always carry an explicit `agent:` identity.
5. Summarize the returned briefing and task contract: target, exact lease,
   premises, banked routes, prior attempts, dead channels, required checks, and
   authority ceiling. Report the returned `session_path`. Vela stores one typed
   private `session.json` under a collision-safe `.vela/work/` directory. Do
   not ask the user or agent to edit or stage it.
6. Close with the next action: do the work, run the selected verifier, then use
   `/vela:land <target>`. The land command builds Receipt v1 from flags and the
   exact session; the producer does not author protocol JSON.
7. If another actor holds the lease, report its actor and expiry. Do not release
   it. An owner who abandons work uses a signed release with a truthful reason:

   ```
   vela start <target> --drop --reason "<why the attempt stopped>" \
     --as agent:<name> --json
   ```

   Vela commits the same-owner zero-TTL lease update before removing scratch.
   Deleting the private directory does not release the lease.
