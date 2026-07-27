---
description: Build and register Submission v1 from one exact Attempt
argument-hint: "[target id | foreign submission path]"
---

# /vela:submit

Register one selected result with the shared Frontier. The agent authors
producer evidence. Successful ordinary intake creates a pending Proposal and
no accepted-state change. Do not sign, decide, ask for a human key, or present
Submission authoring as a trust ceremony.

1. Select the work session.

   - Treat a target id in `$ARGUMENTS` as the Attempt's exact Target.
   - With no target argument, let `vela submit` infer only when this actor owns
     exactly one active Attempt.
   - If the CLI reports several owned Attempts, present their IDs and Targets
     and ask which result to submit. Selection prevents ambiguity; it grants no
     scientific authority.

2. Derive the Submission facts from the completed Attempt and files in the Frontier:

   - `claim`: one concrete, bounded sentence supported by the artifacts;
   - `type`: `computational`, `theoretical`, `empirical`, `negative`, or
     `contradiction`;
   - `replayability`: `exact`, `bounded`, `approximate`, `unavailable`, or
     `unknown`;
   - one or more existing frontier-relative artifacts with an honest kind; and
   - at least one caveat that states the claim's limit. If no material limit is
     known, say that rather than omitting the field.

   Include only producer checks that ran. Run a required verifier before
   submission; never invent a pass. Ask a factual question only when the workspace
   does not contain enough information to state the result. Do not ask a human
   to approve or confirm an agent-authored Submission before registering it.

3. Use flag authoring. Do not write or edit `submission.json` on this path.

   ```
   vela submit --attempt <vat_id> \
     --claim "<bounded result>" \
     --type <claim-type> \
     --replayability <class> \
     --artifact <path>:<kind> \
     --caveat "<limit>" \
     --as agent:<name> \
     --json
   ```

   Use `$VELA_ACTOR_ID` when set; otherwise use `agent:claude`. Omit
   `--attempt` only for exact-one inference. Repeat `--artifact` and `--caveat`
   as needed. Vela builds canonical Submission v1 from the typed Attempt,
   hashes the Artifacts, registers exact bytes, and routes the resulting
   Proposal through current authority.

4. Reserve file import for a canonical Submission v1 emitted by a foreign or
   stateless producer. When `$ARGUMENTS` is an explicit Submission path supplied
   for that purpose, run:

   ```
   vela submit <submission.json> --as agent:<name> --json
   ```

   Do not combine a Submission file with `--attempt`. Do not convert the normal plugin
   path into hand-authored JSON.

5. Report the JSON route, Attempt state, and `accepted_state_changed`:

   - `pending_review`: the Submission and Registration Record exist; the
     Proposal awaits review; accepted scientific state did not change.
   - `accepted_by_policy`: the named signed policy authorized the exact lane.
     Report its policy, Decision, and Event IDs without describing this as
     human review.
   - `exact_retry`: Vela reused the recorded durable result. Report the original
     route and state effect.
   - Refusal or error: report the exact layer, message, and repair action. The
     Attempt and lease remain available for correction; do not delete or
     rewrite them.

An abandoned session uses `vela start <target> --drop --reason "<why>"` under
the owning agent identity. That command signs the exact lease release before
removing private scratch.
