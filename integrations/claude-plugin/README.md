# The Vela plugin for Claude Code

Trusted scientific state, driven from your agent. The plugin puts the Vela
loop — `next → start → submit` — inside Claude Code. Authority remains outside
the agent: a retained Era-0 policy may authorize a narrow Permit class, while
other results wait for an exact human or repository-authority transition. No
plugin command signs, approves, stores verdicts, touches a human key, or
suggests a decision.

Requires the `vela` binary on PATH (https://github.com/vela-science/vela)
and a repository with a `.vela/` directory.

## Install

Local, today:

```bash
claude --plugin-dir /path/to/vela/integrations/claude-plugin
```

Marketplace, later: once published, `claude plugin install vela`.

## Commands

- `/vela:status` — one-screen dashboard: frontier integrity, pending-review
  records, top next targets, and unpublished-state warnings.
- `/vela:next` — the offer. Ranked targets rendered conversationally; pick one
  and it opens one typed private work session, reports its exact lease and
  task contract, and summarizes the briefing. The agent never edits or stages
  `session.json`.
- `/vela:review` — read-only proposal inspection. Lists compact pending
  records, opens one exact Review Packet at a time, and keeps proposal,
  verifier evidence, and terminal authority distinct. It writes no answer or
  session file and never enters an authority path.
- `/vela:submit` — Submission authoring. Builds `vela.submission.v1` from a
  bounded claim, exact artifacts, caveats, and the selected Attempt, registers
  it as the agent identity, and reports the route and accepted-state delta.
  It does not ask a human to confirm producer-authored evidence as a trust
  step.

## Producer path

`/vela:next` opens the Attempt. `/vela:submit <target>` selects it explicitly;
with no target, the CLI infers only when the current actor owns exactly one
active Attempt. The normal path is equivalent to:

```bash
vela start <target> --as agent:<name> --json

vela submit --attempt <vat_id> \
  --claim "<bounded result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --as agent:<name> \
  --json
```

Vela builds canonical Submission v1 from the typed Attempt. Successful intake
returns the Registration Record, Proposal, route, and explicit
`accepted_state_changed` value. The ordinary path is pending review with no
accepted-state change. Refusal, invalid input, or an identity mismatch
preserves the Attempt and returns a repair action. A foreign producer supplies
one complete `vela.submission.v1`; plugin Attempts do not hand-author protocol
JSON.

Release abandoned work through the owner-checked command:

```bash
vela start <target> --drop --reason "<why the attempt stopped>" \
  --as agent:<name> --json
```

Vela commits a signed same-owner zero-TTL lease update before it removes
private scratch. Deleting `.vela/work/` does not release a lease.

## Skill

`vela-frontier` teaches any session working in a `.vela/` repository the loop,
the Submission contract, registration routes, and custody rules. The same
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

## Execution

The skill calls Vela's exact CLI contracts directly. Optional execution
harnesses such as Canopus may produce bounded evidence, but they do not become
Vela plugins, writers, or authority surfaces. Successful ordinary intake
creates a pending Proposal and no accepted-state change.

## Session brief

`hooks/hooks.json` runs `scripts/session-brief.sh` on SessionStart. Inside a
frontier (a `.vela/` directory at or above the cwd, discovered the way git
finds `.git`), it emits a few lines of session context: the frontier name, a
one-line state summary, the pending-review count, and the top `next` target.
Anywhere else — or on any error — it exits silently; a broken hook must never
break a session. `/vela:status` is the full render when you want the whole
picture.

## The custody line

Agents draft; verifiers check; accountable principals authorize. A retained
Era-0 policy can authorize a bounded Permit; later authority is recorded by
Vela's exact repository-authority transition. The plugin only reads proposals
and producer state. It never records a verdict or sits in the trust path.
Report any command that offers a shortcut.
