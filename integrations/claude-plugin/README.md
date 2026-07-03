# The Vela plugin for Claude Code

Trusted scientific state, driven from your agent. The plugin puts the Vela
loop — `next → work → land` — inside Claude Code, and keeps the one thing that
must stay outside it outside: `vela sign` is the human's terminal ceremony,
and no command here will ever run it, touch a key, or suggest a verdict.

Requires the `vela` binary on PATH (https://github.com/constellate-science/vela)
and a repository with a `.vela/` directory.

## Install

Local, today:

```bash
claude --plugin-dir /path/to/vela/integrations/claude-plugin
```

Marketplace, later: once published, `claude plugin install vela`.

## Commands

- `/vela:status` — one-screen dashboard: frontier state, sign-queue depth and
  headlines, top next targets, autonomy ratio, unpublished-state warnings.
- `/vela:next` — the offer. Ranked targets rendered conversationally; pick one
  and it opens the work session (`vela work`) and summarizes the briefing.
- `/vela:review` — the triage room. Walks the sign queue one item at a time,
  shows each claim with its evidence, prior, and caveats, records the verdicts
  you dictate into `.vela/sign-session.json` (resume-safe), and ends by handing
  you exactly one command: `vela sign`.
- `/vela:sign-prep` — pre-flight: queue depth, how many answers are already
  saved, binary-pin status. Ends the same way: `vela sign`, yours to run.
- `/vela:land` — receipt authoring. Drafts the `vela.receipt.v1` from the work
  in context, shows it, confirms, lands it as your agent identity, and reports
  the route (policy-admitted or deferred to the sign queue).

## Skill

`vela-frontier` teaches any session working in a `.vela/` repository the loop,
the receipt contract, the landing routes, and the custody rules. The same
skill text is emitted into frontier repos by `vela agents sync` (as
`.claude/skills/vela-frontier/SKILL.md`), so a repo teaches the same rules
whether or not the plugin is installed.

## MCP

`.mcp.json` starts `vela serve . --profile read-only` — the frontier's read
surface as tools. Read-only is the point: the write edge stays on the CLI,
where identity and policy are enforced.

## The custody line

Agents draft; verifiers check; humans sign. The plugin records your verdicts
only after you give them, the terminal ceremony re-renders everything
independently and takes the one confirm and one key read, and no model sits in
any trust path. If a command here ever appears to offer you a shortcut around
that, it is a bug — file it.
