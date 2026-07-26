---
description: Build and land Receipt v1 from an exact private work session
argument-hint: "[target id | foreign receipt path]"
---

# /vela:land

Cross one selected result from private work into the shared frontier. The
agent authors producer evidence. A signed policy may admit a narrow class;
otherwise the result waits for an accountable authority transition. Do not
sign, decide, ask for a human key, or present receipt authoring as a trust
ceremony.

1. Select the work session.

   - Treat a target id in `$ARGUMENTS` as the explicit `--work <target>`.
   - With no target argument, let `vela land` infer the session only when this
     actor owns exactly one active lease.
   - If the CLI reports several owned sessions, present those target ids and
     ask which work result to land. Selection prevents ambiguity; it grants no
     scientific authority.

2. Derive the receipt facts from the completed work and files in the frontier:

   - `claim`: one concrete, bounded sentence supported by the artifacts;
   - `type`: `computational`, `theoretical`, `empirical`, `negative`, or
     `contradiction`;
   - `replayability`: `exact`, `bounded`, `approximate`, `unavailable`, or
     `unknown`;
   - one or more existing frontier-relative artifacts with an honest kind; and
   - at least one caveat that states the claim's limit. If no material limit is
     known, say that rather than omitting the field.

   Include only verifier outcomes that ran. Run a required verifier before
   landing; never invent a pass. Ask a factual question only when the workspace
   does not contain enough information to state the result. Do not ask a human
   to approve or confirm an agent-authored receipt before landing it.

3. Use flag authoring. Do not write or edit `receipt.json` on this path.

   ```
   vela land --work <target> \
     --claim "<bounded result>" \
     --type <claim-type> \
     --replayability <class> \
     --artifact <path>:<kind> \
     --caveat "<limit>" \
     --as agent:<name> \
     --json
   ```

   Use `$VELA_ACTOR_ID` when set; otherwise use `agent:claude`. Omit `--work`
   only for exact-one inference. Repeat `--artifact` and `--caveat` as needed.
   Vela builds canonical Receipt v1 from the typed private session, hashes the
   artifacts, lands the proposal, and routes it through the signed policy.

4. Reserve file import for a canonical Receipt v1 emitted by a foreign or
   stateless producer. When `$ARGUMENTS` is an explicit receipt path supplied
   for that purpose, run:

   ```
   vela land <receipt.json> --as agent:<name> --json
   ```

   Do not combine a receipt file with `--work`. Do not convert the normal plugin
   path into hand-authored JSON.

5. Report the JSON route and session state:

   - `policy_admitted`: the named signed policy authorized admission. Report
     its policy and event ids. The installed transaction closes only the typed
     `session.json`; unrelated scratch remains.
   - `deferred`: the proposal is in the review queue. The installed transaction
     closes only `session.json`. `/vela:review` may inspect the exact proposal;
     authority remains outside the plugin.
   - `exact_retry`: Vela reused the recorded durable result. Report the original
     route and publication status.
   - Deny or error: report the exact message and repair action. The session and
     lease remain available for correction; do not delete or rewrite them.

An abandoned session uses `vela work <target> --drop --reason "<why>"` under
the owning agent identity. That command signs the exact lease release before
removing private scratch.
