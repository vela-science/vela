---
description: Pre-flight for the sign ceremony — queue depth, saved answers, binary pin
allowed-tools: Bash(vela sign --frontier . --json:*), Bash(vela id pin-binary:*), Read
---

# /vela:sign-prep

Pre-flight only. This command changes nothing and signs nothing.

1. **Queue.** Run `vela sign --frontier . --json`. Report `signable_total` and
   one headline per item: lane, id, the first clause of `title`. If the queue
   is empty, say so and stop.
2. **Saved answers.** Read `.vela/sign-session.json` if present (shape:
   `{"answers": {"<id>": "..."}}`). Report how many of the signable items
   already have an answer — "3 of 5 pre-answered; the ceremony will ask the
   rest" — and point at `/vela:review` to fill in the remainder
   conversationally. Do not print the verdicts themselves unless asked.
3. **Binary pin.** Probe `vela id pin-binary --help`. If the subcommand exists,
   run `vela id pin-binary --status` and report one of:
   - pinned and matching — clear to sign;
   - pinned and MISMATCHED — flag loudly: the ceremony will refuse under this
     binary; re-pin (`vela id pin-binary`) only if the human deliberately
     upgraded;
   - no pin recorded — ceremonies run unpinned; recording one is the human's
     call.
   If the subcommand does not exist in this build, skip the check silently —
   older binary, nothing to verify.
4. Close by printing the one command the human runs in their terminal:

   ```
   vela sign
   ```

Never run `vela sign` yourself — it refuses agents by design (exit 4), and the
verdicts inside it are the human's alone.
