# .impeccable.md

Persistent design context across sessions, populated by the
`/teach-impeccable` workflow. The long-form canon lives in
`docs/PRODUCT.md` and `docs/DESIGN.md`. This file exists so any
future session loads the design context fast without re-deriving it.

When the canon shifts, update the section below in place.

## Design Context

### Users

Vela serves three audiences working over scientific evidence.

1. **Frontier scientist.** Inspects evidence, compares claims, finds
   contradictions, identifies gaps, decides what experiment or
   analysis comes next. Needs fast orientation in unfamiliar fields,
   inspectable provenance, calibrated uncertainty, strong literature
   and dataset grounding, a clear distinction between settled
   findings and speculative trails. Does not need another dashboard,
   AI summaries without evidence, or inflated certainty.
2. **Research operator** (program lead, lab builder, translational
   strategist, scientific founder). Maps a frontier, allocates
   attention, evaluates opportunities, coordinates work. Needs
   landscape views, decision trails, portfolio maps, gap analysis,
   exportable evidence packages. Does not need toy visualizations or
   abstract diagrams that can't drive decisions.
3. **AI research agent.** Uses Vela as a structured substrate for
   reasoning. Needs stable object identities, typed relations,
   append-only provenance, confidence and scope fields, correction
   propagation, deterministic materialized views, machine-readable
   evidence bundles. Does not need prose-only memory or arbitrary
   graph schemas.

The atomic unit is the **finding**, not the paper.

### Brand Personality

Three-word personality: **calm, precise, luminous.**

Five pillars:

- **Calm** — we remove noise. Restraint over decoration.
- **Precise** — we respect evidence. Bounded claims. Hairlines.
- **Luminous** — we borrow light. We do not generate truth; we make
  seeing legible.
- **Navigable** — we reveal pathways. Provenance is a trail you can
  walk.
- **Correctable** — we learn and improve. Kintsugi: the seam is
  celebrated.

Emotional center: quiet confidence.
Strategic center: provenance as infrastructure.

Vela is calm, serious, exact, spacious, observant, trustworthy,
quietly beautiful, technically competent, intellectually honest,
human without being casual.

Vela is not loud, cute-first, mystical, corporate, academic-stuffy,
sterile, over-explained, trendy, hype-driven, or sci-fi theatrical.

Voice rules: short sentences for UI, direct verbs, concrete nouns,
evidence-aware language, humble uncertainty. Sentence case for
headings and buttons. Lowercase command names. UPPERCASE only for
instrument labels. The `·` is the only decorative separator. The
em-dash is restricted as an LLM-shaped writing tell.

Banned words include `unlock`, `supercharge`, `AI-powered`,
`revolutionize`, `seamless` (outside strict technical use),
`powerful`, `next-generation`, `cutting-edge`, `world-class`,
`paradigm`, `disruptive`, `effortlessly`, `10x`, `game-changing`.

Taglines:

- Primary — *Navigate knowledge. Reflect truth.*
- Secondary — *Borrowed light. Shared discovery.*
- Japanese — *借りた光、共有された発見*

### Aesthetic Direction

**Vela is a constellation, a sail, and borrowed light.**

The visual world combines scientific editorial rigor, Japanese
restraint, nautical navigation, astronomical mapping, quiet software
precision, and warm archival materiality. It feels like:

- Linear if it studied epistemology and Japanese architecture
- a scientific observatory interface, not a spaceship cockpit
- a *Nature Reviews* figure brought into a modern desktop app
- a nautical chart for knowledge
- an archive illuminated by stars reflected in water

**One register, light only — the cream paper of Borrowed Light.**
The instrument and the essay share one ground. As of the
2026 Borrowed Light port, dark mode is removed; the workbench is
the same paper as the marketing site and the claim detail and the
exported artifacts.

- **Reading + instrument** — `paper-0 #F8F2E5` (aged cream), `paper-1
  #FCF8EE` (raised), `paper-deep #ECE0C7` (sunken accent). Body ink
  `#232B3A`, secondary `#3A4555`, tertiary `#4A5568`. Hairlines are
  ink-with-alpha at 6–22%, never grey.

The single accent is **borrowed gold (`#C9A227`)** — kintsugi seam,
focal star, active route, live indicator. Gold is meaning, not
decoration. The composition has at most one lantern-lit element at a
time.

State palette (editorial, never traffic-light):

- `cinnabar #B5443A` — retraction, contradiction, correction
- `moss #59634E` — replicated, verified, ok
- `brass #8A6A1F` — contested, dissent
- `winter #8FA7B7` — inferred, distant, speculative

**Typography (Borrowed Light stack):**

- Display: **Cormorant Garamond** italic (fallback chain: EB Garamond
  → Iowan Old Style → Georgia). For finding titles, hero, focal
  numbers, essays.
- Body: **EB Garamond** (fallback Palatino → Georgia). For prose,
  long-form reading, claim cards, evidence quotes.
- UI: **Inter Tight** (fallback Inter → system sans). For navigation,
  tables, metadata, controls.
- Mono: **JetBrains Mono** (fallback IBM Plex Mono). For IDs, hashes,
  provenance, code, timestamps, kickers.
- Body size: 14px for instrument UI, 1.05rem (≈17px) for prose.

**Shapes:** softened, not bubbly. Buttons 6px, cards 10–14px, modals
16–20px, tags full radius. App icon 20–24% corner.

**Inspirations:** Linear (clarity, density, command palette), Codex
(restrained dark surfaces), Japanese editorial minimalism (`ma`,
asymmetry), scientific journals (*Nature Reviews* figures), nautical
instruments, astronomical maps. Cajal-style precision, Kawase-style
atmospheric warmth, Studio Ghibli patient devotion.

**Akari** — Vela's small lantern-spirit companion. Appears in
onboarding, empty states, gentle assistant moments. Never in dense
expert review or high-stakes uncertainty. Calm, observant, short
copy, never cute-first. Akari is not the brand; Akari is a helper
within the brand world.

**Anti-references — do not drift toward:**

- Linear's saturated purple (we share their restraint, not the
  startup-aesthetic)
- Notion's friendly informality, illustrated empty states
- Glassmorphism, neon, web3 / crypto graphics
- "AI brain" imagery, floating 3D blobs
- Stock photography, decorative graph spaghetti
- Card radii outside the designated scale
- Pure black or pure white (always warm)
- Em-dash overuse, hype words, LLM-shaped writing
- Mascot-first / Duolingo engagement pressure for Akari

### Design Principles

1. **Show the state, then the proof.** Every claim is one click from
   evidence, method, confidence basis, provenance, contradictions,
   correction history.
2. **Progressive disclosure over maximal display.** Default views
   orient. Detail views satisfy scrutiny.
3. **Confidence is not decoration.** Tied to evidence quality,
   replication, recency, method fit, sample relevance, contradiction
   state.
4. **Contradictions and gaps are first-class.** Dignified, not errors.
   The map includes its own absences.
5. **Calm before impressive, precise before poetic.** When forced to
   choose, choose clarity over beauty and evidence over metaphor.
6. **Agents are participants, not magicians.** AI actions are
   visible, attributable, reviewable, and reversible.

## Canonical references

- `docs/PRODUCT.md` — product doctrine: users, primitives, surfaces,
  voice, taglines, messaging, Akari, quality bar.
- `docs/DESIGN.md` — design system: tokens (YAML frontmatter),
  visual world, color, type, layout, components, motion, anti-
  references.
- `web/styles/tokens.css` — implementation of the tokens. (Note: at
  the time of writing, this file still uses the prior token values
  and needs to be updated to match `docs/DESIGN.md`.)
- `assets/brand/` — canonical mark, wordmark, favicon, OG image.

When in doubt, those files are canonical. This summary exists to load
context fast; the long form has the actual rules.
