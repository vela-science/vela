# The Vela plugin for Claude Code

Trusted scientific state, driven from your agent. The plugin puts the Vela
loop — `next → work → land` — inside Claude Code. Human keys remain outside
the agent: a signed policy may authorize a narrow Permit class, while every
other result waits for the human's `vela sign` ceremony. No plugin command runs
that ceremony, touches a human key, or suggests a verdict.

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
  and it opens one typed private work session, reports its exact lease and
  task contract, and summarizes the briefing. The agent never edits or stages
  `session.json`.
- `/vela:review` — the triage room. Walks the sign queue one item at a time,
  shows each claim with its evidence, prior, and caveats, records the verdicts
  you dictate into `.vela/sign-session.json` (resume-safe), and ends by handing
  you exactly one command: `vela sign`.
- `/vela:sign-prep` — pre-flight: queue depth, how many answers are already
  saved, binary-pin status. Ends the same way: `vela sign`, yours to run.
- `/vela:land` — receipt authoring. Builds Receipt v1 from claim, type,
  replayability, artifact, and caveat flags bound to the exact work session,
  lands it as the agent identity, and reports the route. It does not ask a
  human to confirm producer-authored evidence as a trust step.

## Producer path

`/vela:next` opens the session. `/vela:land <target>` selects it explicitly;
with no target, the CLI infers only when the current actor owns exactly one
active session. The normal path is equivalent to:

```bash
vela work <target> --as agent:<name> --json

vela land --work <target> \
  --claim "<bounded result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --as agent:<name> \
  --json
```

Vela builds canonical Receipt v1 from the typed session. A committed Permit or
Defer closes only `session.json` after installation. Deny, invalid input, or a
key mismatch preserves the session and returns a repair action. File-based
`vela land receipt.json` remains available for canonical Receipt v1 emitted by
a foreign or stateless producer; plugin sessions do not hand-author it.

Release abandoned work through the owner-checked command:

```bash
vela work <target> --drop --reason "<why work stopped>" \
  --as agent:<name> --json
```

Vela commits a signed same-owner zero-TTL lease update before it removes
private scratch. Deleting `.vela/work/` does not release a lease.

## Skill

`vela-frontier` teaches any session working in a `.vela/` repository the loop,
the receipt contract, the landing routes, and the custody rules. The same
skill text is emitted into frontier repos by `vela agents sync` (as
`.claude/skills/vela-frontier/SKILL.md` and
`.agents/skills/vela-frontier/SKILL.md`), so a repo teaches the same rules
whether or not the plugin is installed.

## Codex

Codex discovers skills at `.agents/skills/` (repo) and `~/.agents/skills/`
(user) — the open agent-skills standard, same SKILL.md format Claude Code
reads. `vela agents sync` emits the identical `vela-frontier` skill to both
roots, byte-for-byte (a test pins the parity), so a frontier teaches Claude
Code and Codex the same rules from one source. Invoke it explicitly with
`$vela-frontier`, or let it trigger implicitly whenever the work touches a
`.vela/` repository. Codex custom prompts are deliberately not used: they are
deprecated upstream in favor of skills.

## MCP

`.mcp.json` starts `vela serve . --profile draft` with
`VELA_ACTOR_ID=agent:claude`. The draft profile is the read surface plus the
drafting writes — `propose` (signed proposals that land pending review) and
`work` (claim, land a foreign Receipt v1, signed drop, or deposit an attempt).
The plugin's session-built default uses the CLI flag surface above. Both paths
call the same landing service and signed policy evaluator. Permit carries the
human-signed policy certificate; Defer parks the proposal in the human sign
queue; Deny commits no landing. Nothing on MCP finalizes a human decision. The
`decide` tool is absent from the registry and every profile.

## Session brief

`hooks/hooks.json` runs `scripts/session-brief.sh` on SessionStart. Inside a
frontier (a `.vela/` directory at or above the cwd, discovered the way git
finds `.git`), it emits a few lines of session context: the frontier name, a
one-line state summary, the sign-queue depth, and the top `next` target.
Anywhere else — or on any error — it exits silently; a broken hook must never
break a session. `/vela:status` is the full render when you want the whole
picture.

## The custody line

Agents draft; verifiers check; human keys authorize. A prior signed policy can
authorize a bounded Permit; a direct judgment uses the terminal ceremony. The
plugin records verdicts only after a human gives them, and the ceremony
re-renders the exact decision before one confirmation and one key read. No
model sits in that trust path. Report any command that offers a shortcut.
