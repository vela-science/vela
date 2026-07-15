---
description: One-screen frontier state — sign queue, next targets, autonomy ratio
allowed-tools: Bash(vela status:*), Bash(vela sign --frontier . --json:*), Bash(vela next:*)
---

# /vela:status

Render the frontier dashboard. The plugin's SessionStart hook already emitted
a compact brief (frontier name, state one-liner, sign-queue depth, top
target) as session context — this command is the full render, for when the
user wants the whole picture or the brief has gone stale. Run these three commands (any directory inside a
frontier works — vela discovers `.vela/` by walking upward, like git finds `.git`):

1. `vela status --json`
2. `vela sign --frontier . --json`
3. `vela next --json`

If the `vela` binary is missing, say so and point at the install
(https://github.com/vela-science/vela); if `status` returns `ok: false`, report
`policy.state`, `policy.permit_readiness`, `policy.reason_codes`, and
`policy.error` verbatim and stop.

Then render a restrained dashboard in chat. Prose-first — numbers inline in
sentences, no giant tables, under roughly twenty lines:

- **Frontier.** Name, findings total and by status, replay integrity
  (`replay.ok`), policy byte state (`policy.state`: absent / staged_unsigned /
  active / broken), and Permit readiness (`policy.permit_readiness`: ready /
  human_only / blocked) with any `reason_codes`.
- **Sign queue.** `signable_total` from the sign JSON. If nonzero, give each
  item one headline line: lane, id, the first clause of `title`, and
  `why_here`. These await a human key. Never characterize them as yours to
  resolve, and never suggest what the verdict should be.
- **Next.** The top three `targets` from `vela next --json`: lane, id, title,
  and the one-line `why`. Mention that `/vela:next` starts one.
- **Autonomy.** From `status.compounding`: the `autonomy_ratio` (share of
  landings the signed policy admitted without ceremony), plus anything notable
  in `dead_channel_coverage` or `attempts_avoided`.
- **Warnings.** If `unpublished_store_files > 0`, flag it plainly: signed
  state is sitting uncommitted on this machine, and the non-JSON `vela status`
  names the fix.
