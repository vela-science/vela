---
description: One-screen frontier state — integrity, pending review, and next targets
allowed-tools: Bash(vela status:*), Bash(vela review list:*), Bash(vela next:*)
---

# /vela:status

Render the frontier dashboard. The plugin's SessionStart hook already emitted
a compact brief (frontier name, state one-liner, pending-review count, top
target) as session context — this command is the full render, for when the
user wants the whole picture or the brief has gone stale. Run these three commands (any directory inside a
frontier works — vela discovers `.vela/` by walking upward, like git finds `.git`):

1. `vela status . --json`
2. `vela review list . --limit 3 --json`
3. `vela next . --json`

If the `vela` binary is missing, say so and point at the install
(https://github.com/vela-science/vela). `ok: false` is a real state, not a
rendering failure: report `integrity.strict`, `integrity.blocker_count`,
`integrity.blockers_by_code`, and the policy fields exactly. Do not hide the
review records or invent a producer offer when strict state is blocked.

Then render a restrained dashboard in chat. Prose-first — numbers inline in
sentences, no giant tables, under roughly twenty lines:

- **Frontier.** Name, Claim count, replay integrity
  (`integrity.replay`), strict standing and blocker count, policy byte state
  (`policy.state`), and Permit readiness (`policy.permit_readiness`).
- **Review.** `counts.pending_review` from status and the compact records from
  `review list`. If nonzero, give each returned item one headline line:
  proposal id, standing, claim, and recorded time. These await an accountable
  authority transition. Never characterize them as yours to resolve, and
  never suggest what the decision should be.
- **Next.** The top three `targets` from `vela next --json`: lane, id, title,
  and the one-line `why`. Mention that `/vela:next` starts one.
- **Warnings.** If `unpublished_store_files > 0`, flag it plainly: signed
  state is sitting uncommitted on this machine, and the non-JSON `vela status`
  names the fix.
