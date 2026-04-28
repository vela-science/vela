---
version: alpha
name: Vela
description: Calm scientific knowledge navigation system with Japanese restraint, nautical-celestial motifs, and provenance-first product UI.
colors:
  primary: "#0B1320"
  on-primary: "#F7F4EC"
  secondary: "#132334"
  on-secondary: "#E7ECF1"
  tertiary: "#C8A45D"
  on-tertiary: "#0B1320"
  background: "#F7F4EC"
  on-background: "#101820"
  surface: "#FBF9F3"
  on-surface: "#162234"
  surface-dark: "#08111C"
  on-surface-dark: "#E8EEF4"
  panel-dark: "#0D1B2A"
  panel-dark-muted: "#142538"
  border: "#D9D4C8"
  border-dark: "#243548"
  muted: "#8A95A1"
  muted-dark: "#93A1AF"
  mist: "#E8EDF1"
  stone: "#D8D2C6"
  sand: "#EFE8DA"
  gold: "#C8A45D"
  gold-soft: "#E7D4A4"
  seafoam: "#68B7AA"
  blue: "#6F92B8"
  purple: "#8D82B8"
  success: "#68B7AA"
  warning: "#D6A84F"
  error: "#B86A61"
  contradiction: "#B86A61"
  focus-ring: "#D4B56A"
typography:
  display-xl:
    fontFamily: "Lyon Display, Canela, Source Serif 4, Georgia, serif"
    fontSize: "48px"
    fontWeight: 500
    lineHeight: "56px"
    letterSpacing: "-0.03em"
  display-lg:
    fontFamily: "Lyon Display, Canela, Source Serif 4, Georgia, serif"
    fontSize: "36px"
    fontWeight: 500
    lineHeight: "44px"
    letterSpacing: "-0.025em"
  h1:
    fontFamily: "Lyon Display, Canela, Source Serif 4, Georgia, serif"
    fontSize: "32px"
    fontWeight: 500
    lineHeight: "40px"
    letterSpacing: "-0.02em"
  h2:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    lineHeight: "30px"
    letterSpacing: "-0.015em"
  h3:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 600
    lineHeight: "24px"
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "22px"
    letterSpacing: "-0.005em"
  body-sm:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "20px"
    letterSpacing: "-0.003em"
  label:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: "16px"
    letterSpacing: "0.08em"
  caption:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: "16px"
    letterSpacing: "0.01em"
  mono:
    fontFamily: "JetBrains Mono, IBM Plex Mono, ui-monospace, SFMono-Regular, monospace"
    fontSize: "12px"
    fontWeight: 450
    lineHeight: "18px"
    letterSpacing: "-0.01em"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  xxl: "32px"
  xxxl: "48px"
  section: "64px"
rounded:
  xs: "4px"
  sm: "6px"
  md: "10px"
  lg: "14px"
  xl: "20px"
  full: "999px"
motion:
  ease: "cubic-bezier(0.2, 0.6, 0.2, 1)"
  ease-spring: "cubic-bezier(0.34, 1.56, 0.64, 1)"
  dur-1: "120ms"
  dur-2: "240ms"
  dur-3: "360ms"
  dur-4: "480ms"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.sm}"
    padding: "10px 14px"
    typography: "{typography.body-sm}"
  button-primary-hover:
    backgroundColor: "{colors.secondary}"
    textColor: "{colors.on-secondary}"
    rounded: "{rounded.sm}"
    padding: "10px 14px"
  button-accent:
    backgroundColor: "{colors.tertiary}"
    textColor: "{colors.on-tertiary}"
    rounded: "{rounded.sm}"
    padding: "10px 14px"
    typography: "{typography.body-sm}"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.sm}"
    padding: "9px 13px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
    typography: "{typography.body-sm}"
  input-dark:
    backgroundColor: "{colors.panel-dark}"
    textColor: "{colors.on-surface-dark}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
    typography: "{typography.body-sm}"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.lg}"
    padding: "16px"
  card-dark:
    backgroundColor: "{colors.panel-dark}"
    textColor: "{colors.on-surface-dark}"
    rounded: "{rounded.lg}"
    padding: "16px"
  sidebar-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.on-surface-dark}"
    rounded: "{rounded.lg}"
    padding: "12px"
  tag:
    backgroundColor: "{colors.mist}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.full}"
    padding: "4px 8px"
    typography: "{typography.caption}"
  tag-high-confidence:
    backgroundColor: "#E6F2EF"
    textColor: "#2E746B"
    rounded: "{rounded.full}"
    padding: "4px 8px"
  tag-contradiction:
    backgroundColor: "#F5E4E1"
    textColor: "#9C554E"
    rounded: "{rounded.full}"
    padding: "4px 8px"
---

# Design

The design system. Read with `docs/PRODUCT.md` (which says what Vela
is) as the prelude. This file says what Vela looks like and how to
build it.

## Overview

Vela is a calm scientific knowledge navigation system. The interface
should feel like a disciplined research instrument: precise enough
for scientists, quiet enough for deep work, and beautiful enough to
make complex knowledge feel navigable.

The design language combines:

- Linear-like product clarity and density
- Codex-like technical seriousness
- Japanese restraint, negative space, and quiet asymmetry
- scientific editorial rigor
- nautical and celestial navigation motifs
- subtle warmth from paper, ink, brass, and reflected light

The product should not feel like a generic dashboard, AI copilot,
social app, or sci-fi command center. It should feel like the place
where scientific state becomes visible and correctable.

Primary mood words:

- calm
- precise
- luminous
- navigable
- correctable
- archival
- serious

## Visual world

The visual system feels like:

- Linear if it studied epistemology and Japanese architecture
- a scientific observatory interface, not a spaceship cockpit
- a *Nature Reviews* figure brought into a modern desktop app
- a nautical chart for knowledge
- an archive illuminated by stars reflected in water

Vela's world combines:

- scientific editorial rigor
- Japanese restraint
- nautical navigation
- astronomical mapping
- quiet software precision
- warm archival materiality

Two registers, one composition seen at different times of day:

**Atlas register (dark, default for navigation).** Deep navy ground,
borrowed-gold accents, calm-sea horizon, warm ink on dark glass.
Default for the desktop app, homepage hero, graph and bridge views,
dashboard. *The user is looking up.*

**Reading register (light, default for prose).** Warm rice paper,
deep brown-black text, hairline rules. Default for claim detail,
docs, essays, exported artifacts, hub registry. *The user is looking
down.*

Both registers share borrowed-gold as the single accent (kintsugi
seam, focal star, active route), the same type families, the same
tick motif. They are the same room photographed twice.

Dark mode is the flagship mode for map, provenance, and deep-focus
scientific navigation. Light mode is the flagship mode for reading,
editing, settings, exported artifacts, and long-form review.

## Core visual metaphors

### Sails

Meaning: intention, direction, vessel, movement through uncertainty.

Usage:

- logo mark
- splash screen
- brand artifacts
- occasional empty states

Avoid making sails too literal, recreational, or maritime-tourist.

### Constellations

Meaning: pattern, relationship, meaning created from points.

Usage:

- knowledge map
- graph views
- linked findings
- visual identity

Avoid decorative star fields that do not encode meaning.

### Reflections

Meaning: borrowed light, evidence made visible, truth seen indirectly
through careful observation.

Usage:

- hero imagery
- dark-mode map depth
- brand motifs
- transition between sky and water (state and provenance)

Avoid glossy water clichés.

### Horizon

Meaning: orientation, frontier, direction, humility before
complexity.

Usage:

- overview surfaces
- onboarding
- landscape pages
- large brand moments

Avoid generic sunset imagery.

### Trails

Meaning: provenance, route, reasoning, history.

Usage:

- trail UI
- provenance timelines
- command and action surfaces

Trails always imply inspectability, not just movement.

### Borrowed light

Meaning: knowledge does not generate truth; it reflects evidence. The
moon does not generate light; it reflects the sun.

Usage:

- the gold accent itself (the carried light)
- attribution chrome on the assistant and on Akari
- export-and-share artifacts

## Color

The palette is built around deep ocean ink, rice-paper surfaces,
misted borders, and restrained gold.

### Atlas register (dark, navigation default)

| Token | Hex | Use |
|---|---|---|
| `surface-dark` | `#08111C` | Deepest dark surface — page ground, behind-everything. |
| `primary` | `#0B1320` | Midnight ink — primary buttons in light mode, dark headers. |
| `panel-dark` | `#0D1B2A` | Cards and panels on dark. |
| `secondary` | `#132334` | Deep sea — sidebar, layered surfaces, hover ground. |
| `panel-dark-muted` | `#142538` | Sunken inputs, table rows. |
| `border-dark` | `#243548` | Borders and dividers on dark. |
| `on-surface-dark` | `#E8EEF4` | Body text on dark. |
| `on-secondary` | `#E7ECF1` | Body text on deep sea. |
| `muted-dark` | `#93A1AF` | Secondary text on dark. |

### Reading register (light, prose default)

| Token | Hex | Use |
|---|---|---|
| `background` | `#F7F4EC` | Rice paper — page ground. Never pure white. |
| `surface` | `#FBF9F3` | Paper — cards, sheets, reading surfaces. |
| `mist` | `#E8EDF1` | Misted border / divider. Cool grey-blue for atmospheric depth. |
| `stone` | `#D8D2C6` | Stone — heavier dividers, inactive controls. |
| `sand` | `#EFE8DA` | Warm subdued surface — tag backgrounds, hover wells. |
| `border` | `#D9D4C8` | Standard border on light. |
| `on-background` | `#101820` | Body ink. |
| `on-surface` | `#162234` | Body ink on raised surface. |
| `muted` | `#8A95A1` | Secondary text. |

### The single accent — borrowed gold

| Token | Hex | Use |
|---|---|---|
| `tertiary` / `gold` | `#C8A45D` | The carried light. Active findings, focal star, primary route, command-palette accent, key call-to-action. |
| `gold-soft` | `#E7D4A4` | Hover and lit-edge derivative. |
| `focus-ring` | `#D4B56A` | Focus outline. |

Gold is meaning, not decoration. Gold is borrowed light, focus, and
active meaning. Do not make every CTA gold. The whole composition
has at most one lantern-lit element at a time.

### State

| Semantic | Token | Hex |
|---|---|---|
| Verified, live, ok | `seafoam` / `success` | `#68B7AA` |
| Warning | `warning` | `#D6A84F` |
| Contradiction, error | `error` / `contradiction` | `#B86A61` |
| Inferred relation | `blue` | `#6F92B8` |
| Speculative trail | `purple` | `#8D82B8` |

State chips are engraved pills: a dot, a lowercase label, the state
color. Never filled bubbles. Never `.green()` / `.red()` from the
`colored` crate directly — use the helpers in
`crates/vela-protocol/src/cli_style.rs`.

### Contrast rules

- Do not place thin gray text on dark navy.
- Do not use gold for long body copy.
- Do not create low-contrast prestige UI.
- Pure black and pure white are banned. Always warm.

## Typography

Vela typography balances editorial authority with interface clarity.

### Display and editorial headings

`Lyon Display` is the canonical display face. Fallback chain:
`Canela` → `Source Serif 4` → `Georgia` → `serif`. The serif feels
literary and scientific, not fashion-editorial.

Use display typography sparingly in product UI. It works best for:

- finding titles
- collection titles
- major empty states
- launch and brand surfaces
- long-form essays
- focal numbers in claim detail (the 0.92 confidence, etc.)

### Product interface

`Inter` carries the majority of UI: navigation, tables, metadata,
controls, sidebars, cards, labels.

### Data and provenance

`JetBrains Mono` (with `IBM Plex Mono` fallback) for:

- finding IDs
- source IDs
- hashes
- object references
- timestamps
- confidence values
- event logs
- code-like provenance snippets

### Type behavior

- Small caps labels for sections, with controlled letter spacing.
- Sentence case for buttons and navigation labels. Never title case.
- Tabular numerals for confidence, counts, dates, and metrics.
- Prefer fewer type sizes with stronger hierarchy.
- Avoid generic giant SaaS headlines in product surfaces.

### Japanese fallback

Japanese text — the borrowed-light tagline and any Japanese terms in
docs — uses the system Japanese serif fallback:
`"Hiragino Mincho ProN", "Yu Mincho", "Noto Serif JP", serif`.

## Layout

### Grid

Use an 8px base grid. Most spacing comes from 4 / 8 / 12 / 16 / 24 /
32 / 48 / 64.

### Density

Vela is dense enough for expert work but never cramped.

Default product density:

- sidebar width: 220 to 260px
- right inspector width: 320 to 420px
- content max width for reading: 720 to 900px
- map canvases: flexible, with semantic zoom and side panels

### Composition

A stable three-zone product layout:

1. **Navigation** — persistent left sidebar
2. **Work surface** — map, finding, trail, or source view
3. **Inspector** — findings, provenance, links, metadata, or assistant

### Sidebar order

1. Overview
2. Map
3. Findings
4. Trails
5. Sources
6. Collections
7. Notes
8. Alerts
9. Settings

### Command palette

The command palette is a first-class interface, not a shortcut
afterthought. It is the fastest way to move through scientific
state.

Common commands:

- Find recent high-confidence findings
- Trace provenance for selected finding
- Compare methods across datasets
- Show contradictions
- Create trail from selection
- Import source
- Explain confidence
- Export evidence package

Styling: dark by default, high contrast, search-first, grouped
commands, keyboard hints, recent objects, AI assistant actions
clearly labeled.

### Maps and graphs

Constellation maps must encode meaning. They are not decorative node
fields.

Use:

- semantic zoom
- labels only at appropriate zoom levels
- clear node legends
- typed edge styles
- clustering
- selected-focus mode
- calm animation

Avoid:

- hundreds of unlabeled points
- random particle effects
- glowing neural-network visuals
- graph spaghetti

## Elevation and depth

Vela uses depth subtly.

### Light mode

- 1px borders
- low-opacity shadows
- soft paper surfaces
- gentle contrast between background and panels

Avoid heavy shadows. Light-mode depth feels like layered paper, not
floating cards.

### Dark mode

- tonal layering instead of heavy shadow
- thin borders in deep slate
- faint inner highlights on focused panels
- subtle gold or seafoam focus rings

Dark panels do not become glassmorphism. Transparency is minimal and
purposeful.

### Elevation scale

| Level | Use |
|---|---|
| 0 | Page background, map canvas |
| 1 | Cards, table rows, sidebar groups |
| 2 | Inspectors, floating panels, command palette |
| 3 | Modals, popovers, destructive confirmations |

Elevation signals interaction and containment, not decoration.

## Shapes

Vela shapes are softened, not bubbly.

| Element | Radius |
|---|---|
| Small controls (buttons, inputs) | 6px |
| Cards and panels | 10 to 14px |
| Modals | 16 to 20px |
| Pills and tags | full radius |
| App icon | rounded square, 20 to 24% corner radius |

Avoid:

- extreme rounded corners everywhere
- sharp sci-fi polygons
- excessive circular badges
- decorative cut corners

The logo mark may use sharper sail and star geometry, but product UI
stays calm and tactile.

## Components

### Button

Quiet and legible.

Primary actions use midnight ink in light mode and paper-on-ink
inversion in dark mode. Accent gold is reserved for the most
important action on a surface. Do not make every CTA gold.

### Card

Holds structured scientific objects: finding, source, dataset, trail,
concept, alert.

A card exposes object type, title, confidence or status, tags, and
one clear next action. Avoid nested cards unless the inner card is a
distinct object with clear affordance.

### Tag

Identifies scientific type, method, status, confidence, or scope.

Examples: Genomics · Dataset · Method · High Confidence · Needs
Review · Contradiction · Inferred.

Tags are readable, compact, and semantically colored.

### Table

Tables are first-class product surfaces.

Use tables for:

- evidence lists
- source records
- datasets
- extraction events
- confidence changes
- object histories

Every table needs clear columns, row actions, filtering, sorting, and
empty states.

### Finding detail

Finding detail includes:

- title (Lyon Display)
- object ID (mono)
- status and confidence (focal stat)
- summary
- scope
- evidence table
- linked concepts (mini-graph)
- provenance trail
- contradictions
- correction history
- export and share controls

A user should never wonder why a finding exists.

### Provenance trail

Provenance reads as a path through time and evidence.

Use timelines, linked cards, or flow diagrams with explicit object
types. Every node is inspectable.

### Assistant panel

Visually subordinate to scientific state. The assistant is a guide,
not the main character.

The assistant panel may use Akari subtly, in low-risk contexts only.

### Akari

Akari is a small, calm, luminous lantern-spirit. Visual direction:

- a small lantern spirit
- a point of borrowed light
- a companion shaped by sail, star, and reflection
- minimal, calm, iconic

Akari should not feel like:

- anime mascot excess
- Duolingo-style engagement pressure
- corporate cartoon helper
- fantasy creature unrelated to Vela

Appears in onboarding, empty states, and assistant guidance. Never in
dense expert workflows or as decoration inside evidence review.

## Motion

Motion is:

- slow enough to feel calm
- fast enough to feel responsive
- purposeful
- spatially coherent
- tied to navigation, reveal, and state change

### Easing

Default ease: `cubic-bezier(0.2, 0.6, 0.2, 1)`.
Spring (rare): `cubic-bezier(0.34, 1.56, 0.64, 1)`.

### Durations

| Duration | Use |
|---|---|
| 120ms | Hover, button states |
| 240ms | Panel transitions, focus glow |
| 360ms | Entry, page transitions |
| 480ms | Constellation glow, atmospheric reveals |

No bounce. No slide-from-off-screen. No spring physics outside
exceptional cases. Honor `prefers-reduced-motion`.

### Good motion

- Constellation nodes settle into place
- Trail line draws as provenance loads
- Command palette opens instantly with subtle scale and opacity
- Confidence drift animates as a controlled change
- Akari appears only in helpful moments

### Bad motion

- Particles everywhere
- Random twinkling
- Bouncy mascot loops
- Unnecessary parallax
- Slow cinematic transitions blocking work

## Tick motif

Every section head and every rim carries engraved ticks. The tick row
is a 1px repeating `linear-gradient` at `border` opacity (light) or
`border-dark` opacity (dark). In the CLI, the tick row is `·`
characters dimmed.

The tick motif is the signature. If a surface has it, the surface
belongs to Vela. If a surface lacks it, ask why.

## Logo

The logo expresses Vela as sail, star, reflection, direction, calm
intelligence.

Preferred mark qualities:

- simple enough for favicon and app icon
- recognizable at 16px
- works in monochrome
- avoids over-rendered 3D effects
- balances celestial and nautical meaning
- sits beside a refined wordmark without competing

Asset rules:

- The line of sight is `on-background` (light) or `on-surface-dark`
  (dark).
- The focal star is always `gold` (`#C8A45D`).
- Don't recolor. Don't rotate. Don't drop-shadow. Don't gradient-fill.
- Don't place on pure white or pure black. Use `background` or
  `surface-dark`.
- Don't put the mark in a rounded SaaS card.

Avoid:

- literal sailboat clip art
- Midjourney-style over-detailing
- generic compass roses
- overly sharp sci-fi shapes
- cartoon rocket or space marks

Canonical files at `assets/brand/`:

| File | Use |
|---|---|
| `vela-logo-mark.svg` | The mark. |
| `vela-logo-wordmark.svg` | The name in Lyon Display. |
| `favicon.svg` | 16×16 derivative of the mark. |
| `rete.svg` | Fuller celestial-sphere motif. Section openers. |
| `og-image.png` | GitHub social preview. |

## Imagery

Use:

- abstract sails
- star maps
- water reflections
- scientific diagrams
- paper and ink textures
- fine grid lines
- quiet charts
- landscape horizons
- subtle archival warmth

Avoid:

- stock scientists in lab coats
- generic AI brains
- glowing neural networks
- floating 3D orbs
- neon space scenes
- dramatic fantasy landscapes
- generic startup illustrations

## CLI output

The CLI is a first-class surface and obeys the same rules.

- ANSI gated on `stdout` being a terminal and `NO_COLOR` being unset.
- Style helpers live in `crates/vela-protocol/src/cli_style.rs`. Call
  them; don't use `colored`'s `.green()` / `.red()` directly.
- Banners are a dim mono eyebrow + tick row, never `===` or `---`.
- `·` is the only decorative separator between fields.
- Gold appears on confirmed-bridge chips and the focal-finding
  marker. Seafoam on the `live` and `running` chip only.

## Hover, focus, motion

- **Links** — `on-background` → `primary`, plus a 1px underline on a
  hairline.
- **Buttons** — `surface` → `panel-dark-muted` (light); `panel-dark`
  → `secondary` (dark). Border `border` → `border-dark`.
- **Focal node hover** — soft lantern glow:
  `box-shadow: 0 0 24px rgba(200, 164, 93, 0.16)`, 240ms.
- **Focus** — `outline: 2px solid var(--focus-ring); outline-offset:
  2px;`.

## Do's and don'ts

### Do

- Use provenance as a visible design principle.
- Make every claim inspectable.
- Use gold sparingly for meaning, active routes, and focus.
- Use dark mode for maps and trails when depth helps comprehension.
- Use light mode for reading and editing when legibility matters.
- Make contradictions and gaps first-class visual states.
- Prefer progressive disclosure over dumping all information at
  once.
- Use calm animation to reveal relationships and state changes.
- Keep scientific labels exact and readable.
- Preserve negative space even in dense expert interfaces.

### Don't

- Don't make generic AI SaaS.
- Don't use purple gradients, gradient text, or neon graph visuals.
- Don't create glassmorphic card stacks.
- Don't use decorative constellations that encode nothing.
- Don't hide provenance behind vague "AI insight" language.
- Don't overuse Akari.
- Don't use gold as a generic luxury accent.
- Don't make low-contrast dark UI.
- Don't ship graph spaghetti.
- Don't call uncertain hypotheses "truth."
- Don't use hype words like `supercharge`, `revolutionize`, `unlock`.

## Visual anti-references

Do not drift toward:

- generic purple-gradient SaaS
- glassmorphic dashboard sludge
- neon cyberpunk science fiction
- "AI brain" imagery
- overdone space opera
- crypto / web3 node graphics
- floating 3D blobs
- excessive cards inside cards
- template landing-page grids
- decorative graph spaghetti
- playful mascot-first branding
- illegible thin text on dark surfaces
- fake scientific precision

## Default agent instruction

When generating Vela UI, follow this file before inventing new
styling. If a needed token is missing, extend the system minimally
and consistently. Prefer calm, precise, provenance-first design over
visual novelty. The best Vela screen makes a complex scientific state
feel inspectable, trustworthy, and navigable.

When in doubt:

- choose clarity over beauty
- choose evidence over metaphor
- choose restraint over decoration
- choose the existing token over a new one

The instrument has one voice. The composition has one accent.
