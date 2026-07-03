# Design Handoff — vela-hub

Developer implementation spec for the **hub** (hub.constellate.science): the
read-only registry / trust substrate that serves content-negotiated HTML from
[`crates/vela-hub/src/html.rs`](../crates/vela-hub/src/html.rs), styled by the
embedded [`web/styles/{tokens,workbench,site}.css`](../web/styles/).

The hub and vela-site share **one** design language. The canonical, full spec —
color/type/spacing/motion tokens, component state matrices, a11y — is
**`vela-site/HANDOFF.md`** (the design source of truth). This file covers only
what is hub-specific: its HTML page inventory, the `HUB_STYLES` surfaces, and the
**reconciliation deltas** needed to bring the hub back in line with canonical.

For hub *doctrine* (endpoints, philosophy, transparency log), see
[`HUB.md`](HUB.md) — don't restate it here.

---

## 1. What the hub renders

Not a SPA. Each route content-negotiates: `Accept: text/html` → a rendered page,
otherwise JSON. HTML is assembled by `render_*_html()` in `src/html.rs` (~3.2k
lines); all CSS/fonts/SVG are `include_str!`/`include_bytes!` compiled into the
binary (single-binary deploy, no runtime asset dir).

### HTML page inventory

| Route | Page | Surface |
|---|---|---|
| `GET /` | Banner / root | endpoint list, publish + verify instructions |
| `GET /entries` | Registry index | `.fr-table` of live frontiers |
| `GET /entries/{vfr}` | Entry detail | `.fd` two-col: findings + metadata |
| `GET /entries/{vfr}/findings/{vf}` | Finding detail | `.fd-claim` + `.fd-cond` + `.fd-dial` |
| `GET /entries/{vfr}/packs/{pack}` | Pack review | member proposals, evidence diffs, verdict |
| `GET /entries/{vfr}/reproduce` | Verify page | `.tm-paper` reproduction commands |
| `GET /entries/{vfr}/proof` | Proof packet | browsable packet (HTML or JSON) |
| `GET /producers/{pubkey}` | Producer ledger | signed objects grouped by frontier |

**Error / non-OK surfaces:** `404` (unknown vfr) · `424 Failed Dependency`
(frontier failed verification) · finding/pack "unavailable" states. Each is a
first-class rendered page, not a bare status — carry the reason and the command to
inspect (mirror the site's "show why" rule).

---

## 2. Hub component surfaces (`HUB_STYLES`, `src/html.rs:46+`)

These are the hub-only CSS classes layered on the shared tokens. State/behavior
below; the token values they consume are defined canonically in
`vela-site/HANDOFF.md §1`.

### 2.1 Entries table — `.fr-table` (`html.rs:47–79`)

Registry of live frontiers. Columns: `idx` (vfr_id, mono, 200px) · `name`
(`--font-sans` 15px, `--ink-0`, ≤360px) · `owner` (mono pubkey) · `state` (chip,
110px) · `upd` (mono timestamp, right-aligned).

| State | Spec |
|---|---|
| Default | `thead th` mono 10px caps `0.18em`, `--ink-3`, `--rule-2` underline; rows `--rule-1` divided |
| Hover | `tbody tr:hover` → `background: var(--paper-1)`, `cursor: pointer`; id link → `--gold` |
| Focus | row link focus → shared gold ring (canonical §7) |
| Loading | server-rendered; no client skeleton (full-page response) |
| Error | 424 → the frontier row/page renders the failed-dependency state |
| Empty | zero live frontiers → banner copy, not an empty table |

### 2.2 Finding detail — `.fd*` (`html.rs:81–152`)

Two-column grid `minmax(0,1fr) 320px`, `gap: 56px`, collapses to one column at
`≤1080px` (`.fd` media query).

- `.fd-claim` — the claim headline, `clamp(1.5rem, 3.2vw, 2rem)`, `--ink-0`,
  `text-wrap: balance`. **Currently `var(--font-sans)`** → see §3 (canonical sets
  the primary assertion in Spectral, not the UI grotesque).
- `.fd-note` — annotation, italic, `--font-body`, gold left border
  (`color-mix(--gold 56%)`), ≤58ch.
- `.fd-conditions` / `.fd-cond` — `dt` mono 10px caps / `dd` mono 13px (or
  `.serif` variant `--font-sans` 14px); links get a gold underline, `--gold` on hover.
- `.fd-dial` — the metadata "gauge": gold top-rule, `__k` mono caps kicker
  (gold-mixed), `__v` value (`--font-sans` 1.15rem, or `.mono`).

### 2.3 Terminal-paper block — `.tm-paper` (`html.rs:154–169`)

Reproduction/command output. `--paper-1` bg, `--rule-2` border, `--radius-sm`,
mono 13px. `__bar` = mono caps header (gold-mixed); `__body` = `white-space: pre`,
`overflow-x: auto`. This is the hub analogue of the site's `ProofWell` — but note
the hub renders it on `--paper-1`, **not** the site's recessed `--lacquer`.

### 2.4 Endpoint list — `.hub-endpoints`

Verb (GET/POST, uppercase, gold) · path · description; hover → gold underline.

### 2.5 Workbench chips & review rail — `workbench.css`

`.wb-chip-*` = engraved state pills (live/ok/contested/lost/inferred), a gold
pulse (`wb-chip-pulse`, 3.6s) on live chips — the one motion exception on the hub.
`.wb-review-rail` = reviewer-session sidebar with gold accent. These consume the
same five-state spine as the site (`vela-site/HANDOFF.md §1.4`).

### Motion note

The hub uses the older motion aliases (`--dur-1`/`--dur-fast`, `--ease`) and a
two-easing set (`--ease-out`, `--ease-trace`) in `tokens.css`. Hover transitions
are paint-only (`background var(--dur-1)`), consistent with canonical. The
`wb-chip-pulse` loop is the one deviation from the site's "nothing loops" rule and
should be reviewed against the settle-once doctrine.

---

## 3. Reconciliation deltas — align hub → canonical

The hub shares the **identical OKLCH color spine** as the site (paper/ink/gold/
moss/brass/winter/cinnabar/stone match). The drift is **typography** (and token
naming). The hub currently violates the site's own font law
(`DESIGN.md`: Inter/Geist banned for chrome). Order: **fonts first** — highest
visible impact, lowest risk.

### Delta 1 — Fonts (do first)

The hub's three sources disagree about their own fonts:
- `web/styles/tokens.css:142` header comment: "Fraunces + Inter Tight".
- `web/styles/tokens.css:152–154`: `--font-sans: Inter, Inter Tight`; `--font-display: Source Serif 4, Fraunces`.
- `src/html.rs:43–44` comment: "Inter Tight … EB Garamond".

Canonical faces (from `vela-site/app/globals.css:162–169`): **Spectral** (serif),
**Space Grotesk** (chrome), **JetBrains Mono** (code).

| Change | File:line | From → To |
|---|---|---|
| Serif var | `web/styles/tokens.css:154` | `"Source Serif 4", …` → `"Spectral", Georgia, "Times New Roman", serif` |
| Sans var | `web/styles/tokens.css:152` | `"Inter", "Inter Tight", …` → `"Space Grotesk", system-ui, -apple-system, sans-serif` |
| Header comment | `web/styles/tokens.css:142` | rewrite to Spectral / Space Grotesk / JetBrains Mono |
| Preloads | `src/html.rs:14–15` | `inter-*.woff2` + `source-serif-4-*.woff2` → `space-grotesk-*.woff2` + `spectral-*.woff2` |
| Comment | `src/html.rs:43–44` | rewrite to match |
| Font files | `web/fonts/` | **add** `space-grotesk-latin-{400,500}` + `spectral-latin-{400,500}` woff2; remove `inter-*` + `source-serif-4-*` once unreferenced |
| `.fd-claim` | `src/html.rs:85` | `--font-sans` is correct for a UI headline, but the **finding's primary assertion** should use the serif per canonical (`.t-lede` = Spectral). Decide: keep claim in grotesk (a title) vs. Spectral (an assertion). Canonical treats the one primary assertion as Spectral. |

`web/fonts/LICENSE.md` currently ships Inter, Source Serif 4, JetBrains Mono
licenses — update to Space Grotesk (OFL) + Spectral (OFL) + JetBrains Mono.

### Delta 2 — Token naming (aliases already bridge most)

| Axis | Hub | Canonical | Action |
|---|---|---|---|
| Spacing | `--s-0..7` | `--space-1..12` | add `--space-*` (or alias `--s-*` → `--space-*`); keep both during migration |
| Surfaces | `--paper-0/1/2/-edge` | `--paper` / `-raised` / `-recessed` / `-sumi` | `tokens.css` already keeps `--paper-N`; add the semantic names as aliases |
| Ink | `--ink-0..4` | `--ink` / `-2/-3/-4` | reconcile: hub's extra `--ink-0` (hero) maps to site's masthead ink; alias `--ink-1` ↔ `--ink` |
| Gold | `--gold` | `--brass-gold` | alias one to the other |
| Motion | `--dur-fast/mid/slow`, 2 eases | `--enter`/`--leave`/`--ease` | unify to one `--ease` + the two-duration chord; keep `--dur-*` as aliases |

Because both systems already alias liberally, Delta 2 is **non-breaking** if done
as additive aliases. Delta 1 is the visible, load-bearing change.

### Delta 3 — ProofWell ground

The site's `ProofWell` sits on recessed `--lacquer` (the darkest plate). The hub's
`.tm-paper` sits on `--paper-1`. If the hub adopts the recessed treatment, use
`--lacquer` + `--ink-on` text to match the canonical reproduce well.

---

## 4. Deploy (unchanged by this handoff)

`flyctl deploy --config crates/vela-hub/fly.toml` from repo root. Assets embed at
compile time, so a font swap (Delta 1) means committing new woff2 to `web/fonts/`
and rebuilding the binary — no runtime asset change. Primary region `yyz`,
bluegreen strategy, `min_machines_running = 1` (trust substrate stays warm).
`/healthz` + `/readyz` gate readiness. Public URLs default in `main.rs`
(`DEFAULT_PUBLIC_URL`, `DEFAULT_SITE_URL`, `DEFAULT_REPO_URL`); override via
`VELA_HUB_PUBLIC_URL` / `VELA_SITE_URL` / `VELA_REPO_URL`.

---

> **Companion channels:** the visual living-spec Artifact and the `/design-sync`
> "Vela Design System" project both render the canonical tokens/components from
> `vela-site/HANDOFF.md`. When Delta 1 lands, update those so the reference stops
> showing the drifted hub type.
