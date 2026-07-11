# Vela CLI style guide

The conventions the CLI holds itself to, and the tests that enforce them.
Written down so they stop regressing. Grounded in gh and clig.dev; where
Vela already had a rule (`docs/CLI.md`), this codifies it.

## Output contract (`ui.rs`)

One module speaks for every porcelain verb. A dispatch arm calls
`ui::set_mode(command, json)` once; everything downstream inherits it.

- **Header**: `VELA · CMD · subject (note)` over a tick row, via `ui::header`.
- **Body**: an aligned key-value block, `·`-separated. Never `===` / `---`.
- **Errors**: `ui::fail_with(kind, message, hint)` →
  human: `err · <message>` then optional `  hint: <next command>`;
  `--json`: one `{ok:false, command, error:{kind,message,hint}}` object.
- **Exit codes**: `0` ok · `1` domain failure · `2` usage · `3` not found ·
  `4` custody/permission · `5` already exists. The code always tells the truth.
- **Advice**: the `hint:` line names the next command; `VELA_ADVICE=0` /
  `--quiet` silence hints without touching the message (git's `advice.*`).

## `--json` universality

Every porcelain verb takes `--json` and emits `{ok, command, …}`. No prose
ever leaks into a `--json` stream. JSON mode is non-interactive: a verb
that would prompt (`policy sign`, `policy revoke`) requires `--yes` under
`--json`. Off-menu utilities (`completions`, `serve`, `init`, `doctor`) are
the documented exceptions. Pinned by `every_visible_command_offers_json`.

## Color (`cli_style.rs`)

- Palette: `moss` ok · `brass` contested · `dust` stale · `madder` lost ·
  `signal` blue (reserved for live state only).
- `cli_style::init()` disables color when stdout is not a TTY, `NO_COLOR`
  is set, or `ui.color=never`. Call it before any print — including
  pre-dispatch intercepts (`lib.rs`).
- Tables pad **raw, then color** — ANSI in a `{:>width}` breaks alignment.

## Interactivity

- Prompt only on a terminal. `ui::is_interactive()` = stdin+stdout are TTYs
  and `VELA_NO_INPUT` is unset; `ui::ensure_can_prompt(what, hint)` refuses
  with exit 2 rather than hang or assume "no". Pinned by
  `prompts_refuse_piped_stdin`.
- Shared input lives in `cli/prompt.rs` (`read_line`, `confirm`). Scriptable
  alternatives always exist (`--yes`, `sign … --yes` / `--batch`).
- **No TUI.** `docs/CLI.md`: "the interactivity of this era belongs to the
  agent, and the pen belongs to you." No raw-mode arrow-key selectors,
  especially not in the signing path. A picker, if ever needed outside the
  ceremony, is a small numeric `select_one` — no dep.

## Help

Lead with EXAMPLES. Each command's block is an `after_long_help` const in
`cli/help_text.rs`; a `SEE ALSO` names sibling verbs. A new verb ships its
const in the same edit that adds it to the surface. Pinned by
`every_visible_command_has_examples`.

## Grammar

The surface is a deliberate hybrid, not drift:

- **Flat loop verbs** for daily cadence: `next · work · land · sign ·
  status · log · diff · check · reproduce · proof · credit · init · doctor
  · serve`.
- **Noun-verb** for everything else: `finding <verb>`, `frontier <verb>`,
  `policy <verb>`, `config <verb>`, `id <verb>`, …
- **One human decision entry** (`sign`, the ceremony) and **one plumbing
  entry** (`proposals accept|reject`, which `sign` drives). `finding note`
  annotates; it does not decide.
- No new top-level verb without a deliberate `V0738_VISIBLE` edit. Growth
  is a decision, not a drift (pinned both directions).

## Tables & progress

- `cli/table.rs` computes column widths from content, pads-then-colors, and
  truncates the widest column only on a TTY — piped output stays full-width
  and byte-stable for scripts.
- The spinner (`cli/progress.rs`) is stderr-only and TTY-gated; it finishes
  as one plain line when not interactive.

## Dependencies

No new crate without written justification. `progress.rs`, `prompt.rs`, and
`table.rs` are the exemplars: a small hand-rolled helper beats a framework,
and keeps the trust-critical paths auditable. No TUI framework, ever, in
the signing path.

## Config layering (`config/settings.rs`)

Effective value = flag > `VELA_*` env > frontier `.vela/config.toml`
(allowlisted keys only) > user `~/.vela/config.toml` > built-in default.
The frontier file is the committed/team layer; `~/.vela` is the personal
override — the same split Entire spells `settings.json` / `settings.local.json`.
