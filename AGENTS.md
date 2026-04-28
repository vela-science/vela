# Vela

Vela is a scientific knowledge navigation system: a substrate where
findings, evidence, provenance, contradictions, and gaps live as
structured state that humans and AI can inspect, correct, serve, and
export. The protocol underneath is content-addressed and replayable.
Surfaces (CLI, site, hub, workbench) are renderings of that state.

For the canonical voice and visual doctrine, read:

- `docs/PRODUCT.md`: what Vela is, who uses it, voice, taglines, Akari
- `docs/DESIGN.md`: design tokens, color, type, components, motion
- `.impeccable.md`: condensed design context (loads fast)
- `docs/CORE_DOCTRINE.md`: what the protocol layer promises

## Current product shape

Shipped and stable:

- substrate primitives: compile, check, review, caveat, revise,
  reject, retract, and history as canonical state transitions
- finding bundles with evidence, conditions, entities, confidence,
  provenance, and typed links
- replications, datasets, and code artifacts as first-class
  content-addressed objects (v0.32 / v0.33)
- predictions and resolutions: calibration ledger (v0.34)
- consensus aggregation queries (v0.35)
- Pearl-ladder causal reasoning: identifiability audit (v0.40),
  back-door + front-door pairwise effects (v0.44), counterfactuals
  via twin-network propagation (v0.45)
- cross-frontier bridges as content-addressed objects (v0.46)
- daily-driver session entry (`vela`) and reorganized help (v0.47)
- localhost workbench (`vela workbench`): Rust + axum, single
  binary, read+write against the cwd's `.vela/` repo (v0.48)
- federation peer registry and hub-aware sync (v0.39 / v0.41)
- registry hub at vela-hub.fly.dev: dumb transport for signed
  manifests
- MCP / HTTP serve modes
- proof packet export

Not yet shipped (in roadmap, do not present as available):

- Vela Desktop (Tauri): native desktop app shape sketched in
  `docs/PRODUCT.md`. Replaces the localhost workbench as the
  daily driver. Plan target v0.51.
- Cross-institutional hub federation gossip: beyond peer-aware
  sync.
- Multi-actor joint signatures.
- Causal level-3 over networks of frontiers.

Do not present roadmap items as shipped. Do not call Vela a "full
science operating system" or "GitHub for science."

## Architecture

Architecture follows this layered shape:

- **Frontier state:** finding bundles, typed links, provenance, confidence, review events, state transitions, signatures
- **Signal layer:** proof readiness, review queues, candidate gaps, candidate bridges, candidate tensions, observer rerankings
- **Review loop:** bootstrap, check, review, search, inspect, proof, serve, benchmark
- **Network later:** compare, merge, institutional sharing, broader federation

Public-facing protocol terms must stay Git-native:

- Findings are versioned as frontier state artifacts (for example `frontier.json`).
- Frontier correction history is represented as signed, reviewable events.
- Shared work should happen with normal Git primitives (branches, commits, diffs).
- Internal Rust identifiers may still use `Project`, but public names must say `frontier`.

## Vocabulary

- **Frontier:** a bounded, reviewable body of structured scientific state. Content-addressed as `vfr_<hash>`.
- **Finding bundle:** one assertion with evidence, conditions, entities, confidence, provenance, and links. Content-addressed as `vf_<hash>`. The assertion is a field; the finding bundle is the durable object.
- **Source:** the paper, dataset, note, protocol, file, or record a finding came from.
- **Evidence:** the specific span, row, table, measurement, or excerpt supporting a finding.
- **Observation:** a conceptual distinction between what was reported and the finding bundle Vela stores; not a first-class object.
- **Replication:** a first-class `vrep_<hash>` record of an attempt to reproduce a finding.
- **Bridge:** a content-addressed `vbr_<hash>` cross-frontier hypothesis: an entity that links findings in two frontiers (v0.46). Status: derived → confirmed → refuted.
- **Mechanism:** an optional structural causal annotation on a `depends` / `supports` link. One of `linear`, `monotonic`, `threshold`, `saturating`, `unknown`. Enables level-3 counterfactual queries.
- **Prediction / Resolution:** content-addressed `vpred_<hash>` and `vres_<hash>` records of a forward-dated claim and its outcome. Drives the calibration ledger.
- **Constellation:** a named, inspectable map of related findings, evidence, gaps, and trails around a scientific question. Not just a visual cluster.
- **Trail:** a navigable provenance, reasoning, or decision path. Always implies inspectability.
- **Gap:** a first-class absence (unknown, missing experiment, unresolved contradiction).
- **Candidate gap / candidate bridge:** an *automated* signal worth review, not a guaranteed target.
- **Retraction impact:** simulated impact over declared dependency links.
- **Prior-art check:** PubMed search as a rough signal, not proof of novelty.
- **Observer policy:** policy-weighted reranking, not definitive disagreement.
- **Akari:** Vela's small lantern-spirit companion. Appears in onboarding, empty states, gentle assistant moments. Never in dense expert review.

## CLI

Examples should use:

```bash
vela compile ./papers --output frontier.json
vela check frontier.json --strict --json
vela normalize frontier.json --out frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' frontier.json)
vela review frontier.normalized.json "$FINDING_ID" --status contested --reason "Mouse-only evidence" --reviewer reviewer:demo --apply
vela history frontier.normalized.json "$FINDING_ID"
vela proof frontier.normalized.json --out proof-packet
vela stats frontier.normalized.json
vela search "LRP1 RAGE amyloid" --source frontier.json
vela tensions frontier.json --both-high
vela gaps rank frontier.json --top 5
vela serve frontier.normalized.json
```

Legacy naming note: avoid pre-frontier command, file, route, and MCP-tool names.

## Conservative reasoning loop

When investigating a frontier, keep this loop:

1. Call `frontier_stats`.
2. Search relevant findings with `search_findings`.
3. Inspect important findings with `get_finding`.
4. Review candidate gaps with `list_gaps`.
5. Review candidate bridges with `find_bridges`.
6. Run `check_pubmed` only as a rough prior-art check.
7. Inspect contested claims with `list_contradictions`.
8. Use `propagate_retraction` only as simulated impact over dependency links.
9. Compare `apply_observer` rerankings as policy-weighted views.
10. Summarize conclusions with finding IDs (`vf_xxx`) and explicit caveats.

Evidence ranking is a heuristic: meta-analysis > RCT > cohort > case-control > case-report > in-vitro. Do not overstate automated contradiction, novelty, bridge, gap, or observer outputs.

## Voice (CLI output, code, comments, docs, error messages)

Coding agents writing any user-facing string follow the voice rules
in `docs/PRODUCT.md`. The short version:

- **Sentence case.** Lowercase command names (`vela seal`, never
  `Vela Seal`). UPPERCASE + tracking only for instrument labels and
  figure tags (`§1 CONDITIONS`, `PROOF PACKET`).
- **Bounded, declarative claims.** State the scope. Don't oversell.
- **Concrete before abstract.** First sentence grounds the reader.
- **No hype, no emoji.** The `·` is the only decorative separator.

### Banned words

`unlock`, `supercharge`, `AI-powered`, `revolutionize`, `blazing`,
`seamless` (outside strict technical use), `powerful`,
`next-generation`, `cutting-edge`, `world-class`, `state-of-the-art`,
`paradigm`, `disruptive`, `seamlessly`, `effortlessly`, `10x`,
`game-changing`.

### Banned punctuation

The em-dash (`—`) is a distinguishing tell of LLM-shaped writing.
Reach for the period or the colon first. The em-dash is allowed only
when no other punctuation works.

### CLI output specifically

- ANSI gated on `stdout.is_terminal()` and `NO_COLOR` being unset.
- Style helpers in `crates/vela-protocol/src/cli_style.rs`. Call
  them; don't use `colored`'s `.green()` / `.red()` directly.
- Banners are a dim mono eyebrow + tick row, never `===` or `---`.
- Gold appears on confirmed-bridge chips and the focal-finding
  marker. Seafoam on the `live` and `running` chip only.

## Design (any UI work: site, workbench, future Vela Desktop)

When touching a user-facing surface, follow `docs/DESIGN.md`. Quick
load:

- **Two registers.** Atlas (dark, `surface-dark #08111C` ground,
  default for navigation: site homepage, graph views, dashboard,
  desktop app). Reading (light, `background #F7F4EC` ground,
  default for prose: claim detail, docs, hub registry).
- **Single accent.** Borrowed gold `#C8A45D`. Reserved for meaning
  (focal finding, active route, key CTA, kintsugi seam). Never
  decoration. Never every CTA.
- **Type.** Lyon Display (fallback chain: Canela → Source Serif 4 →
  Georgia → serif) for editorial headings and focal numbers. Inter
  for product UI. JetBrains Mono for IDs and provenance.
- **Soft radii.** Buttons 6px, cards 10–14px, modals 16–20px, tags
  full radius. Not 0 or 2px (that was the previous canon).
- **Body 14px.** Denser than typical SaaS: Linear-density.
- **Motion.** Default ease `cubic-bezier(0.2, 0.6, 0.2, 1)`.
  Durations 120 / 240 / 360 / 480 ms. No bounce. No spring physics.
  Honor `prefers-reduced-motion`.
- **Tick motif.** Engraved 1px gradient at border opacity is the
  signature of every section head and rim. If a Vela surface lacks
  the tick row, ask why.
- **Anti-references.** No purple-gradient SaaS. No glassmorphism. No
  neon. No floating 3D blobs. No stock photography. No decorative
  graph spaghetti. No card radii outside the scale. No pure black
  or pure white.

Do not invent new tokens. If the value you need is missing, propose
an addition to `docs/DESIGN.md` and wait for confirmation.

## Repository map

- `crates/vela-protocol/`: core frontier protocol and runnable `vela` binary
- `crates/vela-hub/`: Rust + axum hub: dumb transport for signed manifests, federation peer registry
- `crates/vela-cli/`: thin CLI wrapper over `vela-protocol`
- `crates/vela-scientist/`: agent-inbox compilers (scout, notes, code, datasets, etc.)
- `frontiers/`: checked-in compiled sample frontier artifacts
- `projects/`: full `.vela/` repos for active frontiers (e.g. `bbb-flagship`)
- `examples/`: tiny first-use fixtures, including the paper-folder workflow
- `schema/`: finding-bundle JSON schema
- `demo/`: demo scripts
- `docs/`: product, design, architecture, protocol, MCP, proof, vocabulary
- `web/styles/`: shared design tokens (`tokens.css`) used by site, hub, workbench
- `assets/brand/`: canonical mark, wordmark, favicon, OG image
- `site/`: Astro static site at vela-site.fly.dev
- `.impeccable.md`: persistent design context for AI agents

Inherited coding-agent code, archive docs, runtime scaffolds, and
reference research not aligned with current product shape stay out of
the release path unless intentionally reintroduced under the current
PRODUCT.md doctrine.

## Doctrine

The four canon files, in order of precedence on conflict:

1. `docs/PRODUCT.md`: what Vela is, voice, taglines, Akari, quality bar.
2. `docs/DESIGN.md`: design system: tokens, components, motion, anti-references.
3. `docs/CORE_DOCTRINE.md`: protocol-layer claims and v0 surface scope.
4. `.impeccable.md`: condensed design context (summary, not source of truth).

Plus the contracts:

- `docs/PROOF.md`: proof packet contract.
- `docs/BENCHMARKS.md`: benchmark contract.
- `docs/MCP_SETUP.md`: serving contract.

Operational rules:

- Keep public examples centered on candidate bootstrap, `check`,
  state-transition review, `proof`, and `serve`.
- Preserve the compounding loop: use should write better state back
  into the frontier rather than create sidecar memory.
- When the doctrine conflicts with a quick fix, the doctrine wins.
  When the doctrine itself looks wrong, propose the change to the
  doc, don't bypass it in code.

## Environment

- Root CLI env: `.env`
- Do not commit real secrets.

## Verification

Use focused gates for this repo:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p vela-protocol
./target/release/vela stats frontiers/bbb-alzheimer.json
./target/release/vela check frontiers/bbb-alzheimer.json
./target/release/vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-proof-packet
./tests/test-local-corpus-workflow.sh
tests/test-http-server.sh
tests/test-mcp-server.sh
./demo/run-bbb-proof.sh
./scripts/release-check.sh
```
