# Brand

One page, one canon. Voice, color, type, asset, motif. Read this before you
write docs, edit CLI output, or place the mark on a surface.

---

## 1. mood

An astrolabe laid on warm vellum under a desk lamp. Hairlines, not borders.
Serif for meaning, sans for chrome, mono for instrument readout. One cool
accent (`#3B5BDB`) reserved for live state. Engraved ticks along every section
head and along the left rim of every surface. The alidade — a 1px signal-blue
line — marks "here, now."

Vela is an instrument, not a dashboard.

## 2. voice

- **Sentence case** for headings, buttons, commands, captions. Never title-case.
- **Lowercase command names.** `vela seal`, never `Vela Seal`.
- **UPPERCASE + tracking** only for instrument labels and figure tags
  (`§1 CONDITIONS`, `PROOF PACKET`, `SUMMARY`).
- **Bounded claims.** State the condition, the assay, where the claim stops.
- **Declarative.** Describe what a thing is, not what it will do for you.
- **Concrete before abstract.** The first sentence of any doc grounds the
  reader; abstractions follow.
- **No hype, no emoji.** The one decorative separator is `·`.
- **Restraint.** Trust the reader. Don't explain what good naming already says.

### Banned words

`unlock`, `supercharge`, `AI-powered`, `revolutionize`, `blazing`, `seamless`
(outside strict technical use), `powerful`, `next-generation`.

### Anti-patterns

- Unlock insights with AI-powered intelligence.
- Vela turns your lab into a science operating system.

### Preferred patterns

- `vela seal --audit` re-runs the audit and stamps a proof packet.
- A reviewed correction becomes canonical history. The inherited frontier
  changes. The prior proof packet is marked stale.

## 3. color

### Paper and ink

| Token | Hex | Use |
|---|---|---|
| `--paper-0` | `#F4F0E8` | Page — warm off-white, astrolabe vellum. Never pure white. |
| `--paper-1` | `#EFEAE0` | Raised surface. |
| `--paper-2` | `#E7E1D4` | Sunken / inset. |
| `--paper-edge` | `#DCD4C3` | Folded edge. |
| `--ink-0` | `#15181E` | Strongest ink. Page titles, active chrome. |
| `--ink-1` | `#1B1F27` | Body ink. |
| `--ink-2` | `#3A4151` | Secondary text. |
| `--ink-3` | `#6B7386` | Tertiary / caption / tick labels. |
| `--ink-4` | `#9199A8` | Placeholder. Never pure black. |

Hairlines are ink-with-alpha, never grey:

| Token | Alpha | Use |
|---|---|---|
| `--rule-1` | 8% | Faintest tick. |
| `--rule-2` | 14% | Standard hairline. |
| `--rule-3` | 24% | Emphasis rule. |
| `--rule-ink` | 88% | Engraved line. |

### The single accent

| Token | Hex | Use |
|---|---|---|
| `--signal` | `#3B5BDB` | Live state only. The alidade. The current cursor. The active running-audit marker. One signal star at a focal point. |

Reserved. Not for button fills, not for decoration, not for icon bodies.

### Review-state palette

All state colors derive from ink, not a fresh palette. They feel engraved, not
traffic-light.

| Semantic | Name | Hex | In CLI |
|---|---|---|---|
| replicated, ok | moss | `#3F6B4E` | `style::ok("ok")` |
| contested, warn | brass | `#8A6A1F` | `style::warn("contested")` |
| gap, stale | dust | `#7A6F5C` | `style::stale("stale")` |
| retracted, lost | madder | `#8A3A3A` | `style::lost("lost")` |
| live | signal | `#3B5BDB` | `style::live("running")` |

State chips are engraved pills: a dot, a lowercase label, the state color. No
filled bubbles. Never `.green()`/`.red()` from the `colored` crate directly.

## 4. type

Three families, three jobs.

| Family | Role | Where |
|---|---|---|
| Source Serif 4 | meaning | page titles, claims, reading prose, figure captions |
| Inter Tight | chrome | UI body, labels, buttons, metadata rows |
| JetBrains Mono | instrument readout | IDs, indices, tick labels, terminal output, numerics |

Scale is a major third, anchored at 16px: `11 · 13 · 16 · 18 · 22 · 32 · 48 · 72`.

- Eyebrow is always 11px mono with `.14em` tracking.
- Page title is 32–72px serif with `-0.015em` tracking.
- Mono numerics always use `tabular-nums`.

## 5. assets

Canonical copies live at `assets/brand/`:

| File | Use |
|---|---|
| `vela-logo-mark.svg` | The mark — three concentric rings, one alidade, one signal star. |
| `vela-logo-wordmark.svg` | The name set in the brand typeface. |
| `favicon.svg` | 16×16 derivative of the mark. |
| `rete.svg` | The larger astrolabe motif. Full-bleed section openers, proof seals. |
| `og-image.png` | GitHub social preview. |

### Asset rules

- Don't recolor the mark. The alidade is `#1B1F27`; the signal star is
  `#3B5BDB`. Both are load-bearing.
- Don't rotate the alidade or nudge the star — proportions carry the
  instrument metaphor.
- Don't fill the mark with a gradient. Don't add a drop shadow. Don't set it
  on pure white — use `#F4F0E8` or darker.
- Don't place the mark inside a rounded SaaS card. Radii in Vela are 0 or 2px.

## 6. tick motif

Every section head and every rim carries engraved ticks. The tick row is a 1px
repeating `linear-gradient` at `--rule-3` opacity. In the CLI, the tick row is
`·` characters dimmed. The tick motif is the signature — if a surface has it,
the surface belongs to Vela.

## 7. hover, focus, motion

- **Links:** `--ink-1` → `--ink-0`, plus a 1px underline on a hairline.
- **Buttons:** `--paper-0` → `--paper-1`; border `--rule-3` → `--rule-ink`.
- **Focus:** `outline: 2px solid var(--signal); outline-offset: 2px;`
- **Easing:** `cubic-bezier(0.2, 0.6, 0.2, 1)`.
- **Duration:** 120ms hover, 200ms state, 360ms entry. No bounce. No
  slide-from-off-screen. Honor `prefers-reduced-motion`.

## 8. CLI output

The CLI output is a first-class surface and obeys the same rules.

- All ANSI is gated on `stdout` being a terminal and `NO_COLOR` being unset.
- Style helpers live in `crates/vela-protocol/src/cli_style.rs`. Call them;
  don't use `colored`'s `.green()` / `.red()` directly.
- Banners are a dim mono eyebrow + tick row, never `===` or `---`.
- `·` is the only decorative separator between fields.
- Signal blue appears only on the progress-bar current position and the
  `live` / `running` chip.

## 9. web surface

The landing page at `web/index.html` is the canonical public surface. It is
static, GitHub Pages deployable, and uses only `web/styles/tokens.css` +
`web/styles/site.css`. No tracking, no analytics, no framework bundle.

To preview locally with the same layout Pages deploys:

```bash
./scripts/serve-web.sh
open http://localhost:8000/
```

Automatic GitHub Pages deploy: push to `main` triggers
`.github/workflows/pages.yml`, which stages `web/` + `assets/brand/` and
publishes to the repo's Pages environment. Enable Pages once in
**Settings → Pages → GitHub Actions** for the first run.

## 10. the workbench previews

The HTMLs under `web/previews/` are the design-system's proposal for a
future Vela product surface — Frontier, Finding, Terminal, Proof. They are
not wired to data and are not shipping v0 product. Every preview page carries
a banner saying so.

Vela v0 is a CLI protocol. Any public messaging that implies v0 has a GUI is
wrong.

---

*If you find yourself reaching for a color that is not in this file, a
separator that is not `·`, or a word that is on the ban list — stop. The
instrument has one voice.*
