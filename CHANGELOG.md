# Changelog

## 0.17.0 - 2026-04-26

The agent-surface release. Forced by probing `vela serve` against Will's
v0.14 frontier — the HTTP/MCP surface hadn't been exercised since the
v0.11→v0.16 substrate work shipped, and two real gaps surfaced: (1)
`/api/events` accepted `?kind=` and `?target=` query params silently
without filtering, and (2) the MCP tool registry had no way to fetch the
event history of a single finding (the natural agent question after seeing
a `flags.superseded = true` finding: "what changed and when?").

### HTTP

- **`/api/events?kind=<kind>&target=<vf_id>`** — server-side filters now
  applied. Before v0.17 the params were silently ignored and the full
  event log was returned. Filters apply BEFORE pagination so cursor-based
  pagination (`?since=<vev_id>&limit=N`) works on the filtered view.
  Response body grows a `filtered_total` field alongside the existing
  `count` (returned slice) and `log_total` (full log size).

### MCP

- **`get_finding_history` tool** — returns the chronological event log
  for one finding (asserted, reviewed, caveated, noted, confidence-revised,
  superseded, retracted), sorted ascending by timestamp. The natural
  agent path: see a finding flagged `superseded`, call this tool, walk
  the supersedes chain via `payload.new_finding_id` on the
  `finding.superseded` event. Brings the MCP tool count to 10. Validated
  by `vela serve --check-tools`.

### Validation

- `vela serve --check-tools` now exercises `get_finding_history` against
  the first finding in the loaded frontier; passes 10/10 against Will's
  v0.14 frontier.

### Versioning

- Workspace `0.16.0 → 0.17.0`.
- `vela --version → 0.17.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. The MCP tool
  registry is a runtime contract, not a schema.

## 0.16.0 - 2026-04-26

The supersede-aware composition release. Closes the two frictions surfaced
by the v0.15 Patel dogfood:
1. `vela link add` accepted a cross-frontier `contradicts` link to a
   `flags.superseded = true` finding silently — Patel could be contradicting
   wording that's already been refined.
2. The hub's `/depends-on` endpoint shipped in v0.15 had no Workbench
   surface; users had to `curl` to see who referenced their frontier.

### CLI

- **`vela link add` cross-frontier target-status check.** When the link
  target is `vf_<id>@vfr_<id>`, the substrate fetches the dep's frontier
  from its declared locator (HTTPS, ~15s timeout) and inspects the target
  finding's `flags.superseded`. If `true`, prints a one-line warning
  (`warn · cross-frontier target … has flags.superseded = true. You may
  be linking to outdated wording. …`) suggesting `pull --transitive` to
  inspect the supersedes chain. The link is still recorded — this is a
  best-effort review hint, not a hard refusal. `--no-check-target` skips
  the network fetch (CI / offline use). Failure to fetch is silent.

### Workbench

- **Referenced-by panel on `/workbench`.** When loaded with `?vfr=…`,
  fetches `/entries/{vfr}/depends-on` from the hub and renders one row
  per dependent (vfr_id, name, owner_actor_id, publish date) with
  click-through to the dependent's own /workbench view. Hidden when the
  hub returns zero dependents or is unreachable (fail-quiet).

### What this unblocks

- The bidirectional view of cross-frontier composition is now visible
  not just queryable. A visitor on Will's frontier sees who in the world
  references it; click-through navigates the network.
- Publishers writing cross-frontier `contradicts` / `extends` / `depends`
  links get a same-shell warning when their target's wording has been
  refined out from under them.

### Versioning

- Workspace `0.15.0 → 0.16.0`.
- `vela --version → 0.16.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. The hub
  endpoint shipped at v0.15 unchanged.

## 0.15.0 - 2026-04-26

The bidirectional release. Cross-frontier composition has been one-directional
since v0.8: a dependent frontier declares + pins its deps, but the upstream
has no way to learn who is referencing it. Surfaced by another dogfood pass
(Patel persona — clinical pharmacologist publishing a Lecanemab null-result
replication that contradicts Will's frontier): the substrate let her publish
and link cleanly, but Will would have no signal that the contradiction
exists. v0.15 closes that gap at the hub layer.

### Hub

- **`GET /entries/{vfr_id}/depends-on`**. Returns the registry entries
  whose frontier declares a cross-frontier dependency on `{vfr_id}`.
  Implementation walks the latest-per-vfr view, fetches each frontier
  through the existing `fetch_frontier_cached` LRU, and filters by
  `frontier.dependencies[].vfr_id`. O(N) on cold cache, memory-only on
  warm. A future optimization would denormalize a `dependent_vfrs` JSONB
  column at POST time and back this with a SQL `?` lookup.

### CLI

- **`vela registry depends-on <vfr_id> [--from <hub>]`** — calls the
  new endpoint and prints "N frontiers depend on vfr_X" with the list.
  `--json` returns the raw `vela.depends-on.v0.1` envelope.

### What this unblocks

- The bidirectional view of cross-frontier composition. Pre-v0.15 the
  question "who is referencing my frontier?" required scraping every
  hub entry's frontier file and grep-ing for your `vfr_id`. Now it's
  one HTTP call. Validates the substrate's "this is a network, not a
  file format" claim concretely — running against the live hub right
  now, BBB Flagship (`vfr_093f7f15b6c79386`) reports 4 dependents:
  three versions of Will's Alzheimer's frontier and BBB-extension.

### Known frictions surfaced but deferred

- **No warning when `vela link add` targets a finding with
  `flags.superseded = true`.** Patel's contradicts-link to Will's
  superseded Lecanemab finding (`vf_b1f04d00abcd7476`) was accepted
  silently; the substrate doesn't currently check the dep's local cache
  for `superseded` flags at link-add time. Best-effort warning at link-add
  + a `--allow-superseded` escape would close this. Defer to v0.16.
- **Workbench "Referenced by" panel** to surface the new endpoint
  visually on `/workbench` and `/workbench/finding`. Defer to v0.16.

### Versioning

- Workspace `0.14.0 → 0.15.0`.
- `vela --version → 0.15.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Hub schema
  for the new endpoint envelope is `vela.depends-on.v0.1`.

## 0.14.0 - 2026-04-26

The supersede release. Until v0.14 every other proposal kind existed
(`add`, `review`, `note`, `caveat`, `confidence_revise`, `reject`,
`retract`) but you couldn't *change a claim's text*. The assertion text
is part of the content address; mutating it would re-derive the `vf_…`
id and orphan all events targeting the old finding. Real corrections
(Phase 4 follow-up data, refined wording, scope change) had to be stacked
as caveats — which let the original prose travel unchanged. v0.14 adds
the substrate-correct path: a *new* content-addressed finding that
explicitly supersedes the old one. Both stay queryable.

### Substrate

- **`Flags.superseded: bool`** added to `bundle.rs` (additive,
  serde-skipped when false; pre-v0.14 frontiers byte-identical).
- **`finding.supersede` proposal kind** + **`finding.superseded` event
  kind**. `apply_supersede` validates the old finding exists and is not
  already superseded, validates the new finding has a distinct content
  address, pushes the new finding, auto-injects a `supersedes` link from
  new → old, sets `flags.superseded = true` on the old finding, and emits
  a `finding.superseded` canonical event targeting the *old* finding
  (with `new_finding_id` in the event payload). Event-payload validator
  in `events.rs` extended to require `proposal_id` + `new_finding_id`
  on the new event kind.
- **`build_finding_bundle` extracted** from `build_add_finding_proposal`
  so `add_finding` and `supersede_finding` share content-addressing
  logic.

### CLI

- **`vela finding supersede <old-id>`** with `--assertion`, `--reason`,
  and the full v0.11 enrichment flag set (DOI, PMID, year, journal,
  source-authors, conditions-text, species, study-type flags). The
  command builds the new finding bundle and wraps it in a
  `finding.supersede` proposal targeting `old-id`. `--apply` accepts and
  applies in one step; without it the proposal is recorded for review.

### Workbench

- **Source registry panel on `/workbench`** — surfaces the materialized
  projection v0.13 introduced. Renders one row per `SourceRecord` with
  source-type, journal, year, and clickable DOI/PMID badges. Hidden when
  the loaded frontier has no sources (legacy frontiers, or any frontier
  before its first finding).
- **Event timeline on `/workbench/finding`** — shows the chronological
  history for the active finding: asserted, reviewed, caveated, noted,
  superseded, etc. For `finding.superseded` events, the new finding's
  vf_id renders as a click-through link that walks you to the next
  version (preserving `?vfr=…` for multi-frontier mode).

### Tests

- `proposals::tests::v0_14_supersede_creates_new_finding_and_marks_old`
- `proposals::tests::v0_14_supersede_refuses_already_superseded`
- `proposals::tests::v0_14_supersede_refuses_same_content_address`

  347 tests passing.

### Versioning

- Workspace `0.13.0 → 0.14.0`.
- `vela --version → 0.14.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — `Flags.superseded` is additive and
  serde-skipped. Pre-v0.14 frontiers replay byte-identically.

### Known gaps surfaced but deferred

- `vela ingest --paper <pdf>` — a single-paper draft path remains
  deferred. Most useful when there's a real PDF in front of a real
  publisher.

## 0.13.0 - 2026-04-26

The source-record materialization fix. v0.12 unblocked event-replay for
CLI-built frontiers; the next dogfood iteration immediately surfaced the
last lint blocker: `missing_source_record` on every finding whose
provenance derives a SourceRecord that wasn't yet in `frontier.sources`.
Pre-v0.13, the only way to populate the projections was `vela normalize
--write` — but normalize refuses on event-ful frontiers ("normalize
before proposal-backed writes"), so any frontier built via the
proposal/event flow could never reach proof-ready state.

### Substrate

- **`proposals::create_or_apply` materializes source / evidence /
  condition projections inline at apply time.** Whenever a proposal is
  applied (any of finding.add, finding.review, finding.note, …),
  `sources::materialize_project` runs and refreshes
  `frontier.sources[]`, `frontier.evidence_atoms[]`, and
  `frontier.condition_records[]` from the current finding set. Idempotent:
  when no finding state changed (caveat/note on existing findings) the
  projection re-derives the same bytes, so canonical hashes are stable.
  When a proposal isn't applied (pending review), only stats recompute —
  unchanged from pre-v0.13 behavior.

### What this unblocks

- Strict-check on CLI-built frontiers no longer flags `missing_source_record`.
  Will's Alzheimer's frontier rebuild now materializes 10 sources, 11
  evidence atoms, and 11 condition records inline as the 11 findings
  land. Proof-readiness signals shift from "missing source registry" to
  the genuine review-needed signals (entity-resolution-confidence,
  experimental-finding-without-species), which are correct things for a
  fresh manual frontier to flag.

### Tests

- `proposals::tests::v0_13_apply_materializes_source_records_inline`
  exercises the new flow: a single `finding.add` proposal applied via
  `create_or_apply(apply: true)` produces a non-empty `sources[]`,
  `evidence_atoms[]`, and `condition_records[]` in the persisted
  frontier. 344 tests passing.

### Versioning

- Workspace `0.12.0 → 0.13.0`.
- `vela --version → 0.13.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Pre-v0.13
  frontiers built via the CLI grow source/evidence/condition projections
  the next time any proposal applies; pre-v0.13 frontiers built without
  the CLI (BBB, the conformance vectors) keep whatever projection state
  they already had.

## 0.12.0 - 2026-04-25

The link-hash fix. Surfaced by attempting to dogfood the v0.11 enrichment
on Will's Alzheimer's frontier: every CLI-built frontier with `vela link
add` calls broke `vela check --strict` because the `finding_hash` included
links, but `vela link add` (shipped in v0.9) mutated links inline without
emitting a state event. The asserted-event's after_hash became stale the
moment a link landed, breaking the event-replay chain.

### Substrate

- **`finding_hash` excludes `links` from the hashed view of a finding.**
  Per Protocol §5, links are review surfaces — typed relationships
  inferred at compile or review time, not part of the finding's content
  commitment. They're mutable; state-changing events (caveat / note /
  review / revise / retract) still mutate annotations / flags / confidence,
  and those remain in the hash and chain through events properly. The
  finding's own content address (the `vf_…` ID) is unchanged — it never
  used links.

### What this unblocks

- Any frontier with CLI-added links now passes `vela check --strict`
  event-replay validation. Previously: every such frontier failed silently
  on strict because hash divergence broke the chain.

### Known gaps surfaced but deferred

- **Source-record materialization on event-ful frontiers.** `vela check
  --strict` flags `missing_source_record` for findings whose provenance
  could derive a `SourceRecord` that isn't in `frontier.sources[]`. The
  fix would normally be `vela normalize --write`, but normalize refuses
  to mutate frontiers that already have canonical events ("normalize
  before proposal-backed writes"). For finding.add to materialize source
  records inline, or normalize to gain an event-aware mode, is forced
  by the next dogfood iteration. Defer to v0.13.

### Versioning

- Workspace `0.11.0 → 0.12.0`.
- `vela --version → 0.12.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.10.0` — no schema change. Pre-v0.12
  frontiers (BBB, BBB-extension, Will's v0.10 frontier, the conformance
  vectors) all replay byte-identically; the hash semantics shift only
  for findings with non-empty links, where the substrate now treats them
  consistently with how `vela link add` was already mutating them.

## 0.11.0 - 2026-04-25

The richer-finding-add release. v0.10 fixed the CLI's biology-leaning enums.
Inspecting Will's first non-bot frontier on the public hub surfaced the next
shallow-data problem: every finding's provenance was just a citation string
(no DOI, PMID, year, journal as structured fields), every finding's
conditions were the same placeholder ("Manually added finding; requires
evidence review…"), and there was no way to refresh a stale cross-frontier
dependency pin when the upstream frontier republished. The substrate had the
fields; the CLI just didn't ask for them. v0.11 fills the gaps.

### CLI surface

- **`vela finding add` provenance flags** — `--doi`, `--pmid`, `--year`,
  `--journal`, `--url`, `--source-authors`. Each populates the corresponding
  structured `Provenance` field (`url` is new in v0.11; serde-skipped when
  None so pre-v0.11 frontiers serialise byte-identically). `--source-authors`
  takes a semicolon-separated list and writes one `Author` per name (distinct
  from `--author`, which remains the curating Vela actor). When omitted, the
  curator-as-author fallback from v0.10 still applies.
- **`vela finding add` conditions flags** — `--conditions-text`, `--species`,
  `--in-vivo`, `--in-vitro`, `--human-data`, `--clinical-trial`. Replaces the
  hardcoded "Manually added finding; requires evidence review…" placeholder
  that was on every manual finding from v0.5–v0.10. `--species` takes a
  semicolon-separated list and populates `species_verified`.
- **`vela frontier refresh-deps`** — fetches the current hub snapshot for
  every declared cross-frontier dep and re-pins. Reports per-dep
  `unchanged`, `refreshed` (with old → new), `missing` (vfr_id not on hub),
  or `unreachable`. `--dry-run` shows the diff without writing. `--from`
  defaults to https://vela-hub.fly.dev. The forcing function: BBB
  republishes weekly via CI; without refresh, your local pin goes stale
  silently.

### Substrate

- `Provenance.url` (new optional field) — generic source URL when none of
  the structured identifiers fit (preprint server URL, dataset landing
  page, talk recording). Serde-skipped when None; pre-v0.11 frontiers
  validate byte-identically.

### Tests

- Three new unit tests in `state::v0_11_finding_tests` covering provenance
  flag round-trip, conditions flag round-trip, and the back-compat
  fallback when no v0.11 flags are supplied. 343 tests passing.

### Versioning

- Workspace `0.10.0 → 0.11.0`.
- `vela --version → 0.11.0`; banner stamps bump in lockstep.
- `VELA_COMPILER_VERSION → vela/0.11.0` for new frontier scaffolds; pre-v0.11
  publisher stamps continue to validate (compiler-stamp softening from v0.9).
- Schema version stays at `v0.10.0` — `Provenance.url` is additive and
  serde-skipped, so no schema URL bump.

## 0.10.0 - 2026-04-25

The first non-bio frontier. Same dogfood mechanic as v0.9: I played a second
external publisher — a particle-astrophysics postdoc with a frontier on
direct-detection constraints on spin-independent WIMP-nucleon cross-section.
The path completed end-to-end (`vfr_dede3b473cac72cf` is on `vela-hub.fly.dev`),
but the schema's biology-leaning enums leaked badly. v0.10 closes that gap
additively — every pre-v0.10 frontier replays byte-identically.

### Substrate

- **Entity type extensions** (`crates/vela-protocol/src/bundle.rs`).
  `VALID_ENTITY_TYPES` adds `particle` (WIMPs, photons), `instrument`
  (XENONnT, JWST — capital objects that run measurements), `dataset`
  (instrument data releases, distinct from the paper that reports them),
  and `quantity` (named numerical values with units, e.g. `28 GeV/c^2`).
  Pre-v0.10 entries unchanged; `other` remains the escape valve.
- **Assertion type extensions.** `VALID_ASSERTION_TYPES` adds `measurement`
  (numerical-quantity reports) and `exclusion` (upper/lower bounds at a
  confidence level — "WIMP mass < X at 90% CL"). Pre-v0.10 entries unchanged.
- **Source type extension.** `VALID_PROVENANCE_SOURCE_TYPES` adds
  `data_release` for instrument runs, observation campaigns, and dataset
  versions that are themselves the substantive object (XENONnT SR0, Planck
  data releases, JWST observation runs).
- **Schema URL bumps `v0.8.0 → v0.10.0`** for new frontiers. The validator
  now accepts either URL (`KNOWN_SCHEMA_URLS = ["v0.8.0", "v0.10.0"]`)
  with the same publisher-claimed doctrine the v0.9 compiler-stamp softening
  established. Pre-v0.10 frontiers (BBB, BBB-extension, the v0.8
  cross-frontier conformance vector, all entries already on the public hub)
  validate byte-identically under v0.10 — no churn to content addressing.

### Conformance

- **`tests/conformance/non-bio-domain/`**: a new physics frontier that
  exercises every v0.10 enum extension on two findings (XENONnT exposure
  measurement + WIMP cross-section exclusion limit) plus a `depends` link
  between them. `expected.json` pins re-derived snapshot/event-log hashes;
  `python3 scripts/cross_impl_conformance.py tests/conformance/non-bio-domain/frontier.json`
  PASSes.

### Documentation

- **`docs/PUBLISHING.md`** enum tables updated with the v0.10 additions
  and a paragraph explaining their domain-neutral provenance.
- **`docs/PROTOCOL.md`** §5.1 (new) documents the v0.10 enum extensions
  and the back-compat schema URL pattern; spec-version stamp bumps to
  v0.10.0.

### Versioning

- Workspace version `0.9.0 → 0.10.0`.
- `vela --version → 0.10.0`; banner stamps bump in lockstep.
- `VELA_SCHEMA_URL` and `VELA_SCHEMA_VERSION` bump to `v0.10.0`/`0.10.0`;
  the validator accepts both `0.8.0` and `0.10.0` for back-compat.
- `VELA_COMPILER_VERSION` bumps to `vela/0.10.0` for new frontier scaffolds;
  pre-v0.10 publisher stamps continue to validate (v0.9 compiler-stamp
  softening was the precedent).

### What is deferred to v0.11+

- **Per-domain extension packs.** v0.10 widens the canonical enums to handle
  one second domain (physics). When ≥ 3 non-bio frontiers exist with
  divergent vocabulary needs, a `frontier.domain` declaration + per-domain
  enum extension may become forced. Until then, the additive default is
  enough.
- **Structured numerical-claim representation.** A `measurement` finding's
  substance is `value · unit · confidence_level · target_quantity`; today
  it lives in prose under `assertion.text`. A first-class `claim_numeric`
  block on the finding bundle would make claims comparable across implementations
  and queryable. Holding until ≥ 2 publishers reach for it.
- **Conditions struct extensions** (instrument live-time, exposure,
  fiducial mass, blinding regime). Pre-v0.10 conditions are bio-heavy; the
  current `text` field still holds for non-bio findings, but a domain-aware
  conditions schema is open.

## 0.9.0 - 2026-04-25

The first-publisher cleanup. v0.8 proved cross-frontier composition end-to-end
on the public hub. A dogfood run pretending to be an external publisher
("Dr. M, 4 GLP-1/Alzheimer's findings, zero prior context") surfaced 12
distinct frictions on the path from `vela-hub.fly.dev` landing → signed
manifest live. None required a substrate change. v0.9 fixes the surface so
the next *real* external publisher hits a coherent path.

### CLI surface

- **`vela frontier new <path> --name "..."`** scaffolds a publishable
  `frontier.json` stub that passes `vela check --strict` immediately. Closes
  the bootstrap gap between `vela init` (creates `.vela/` repo, not
  publishable) and `vela finding add` (requires the file to pre-exist). Use
  `frontier new` whenever the target is the hub.
- **`vela link add <frontier> --from vf_… --to vf_…[@vfr_…] --type …`**
  is the CLI on-ramp for typed links. Until v0.9, links required hand-editing
  JSON — the proposal/event flow had no link counterpart. The handler validates
  the target via `LinkRef::parse`, checks that local targets exist, refuses
  cross-frontier targets without a declared dep (with the exact `frontier
  add-dep` invocation in the error), and recomputes `stats.links` /
  `stats.link_types` so strict validation stays green.
- **CLI enums single-sourced with the validator** (`crates/vela-protocol/src/bundle.rs`).
  `VALID_ASSERTION_TYPES`, `VALID_EVIDENCE_TYPES`, `VALID_PROVENANCE_SOURCE_TYPES`,
  `VALID_LINK_TYPES`, and `VALID_ENTITY_TYPES` now live next to the bundle
  types and are imported by both `cmd_finding_add`/`cmd_link_add` and
  `validate.rs`. Invalid `--type`/`--evidence-type`/`--source-type`/entity
  values fail at add-time with the full valid set in the error message
  instead of at strict-check time after the (now content-addressed) finding
  has been written.
- **`actor`, `link`, `registry` surfaced in `vela --help`.** Pre-v0.9 these
  worked but were absent from the strict-help banner — invisible to a new
  user reading the CLI surface. Help also adds a "Publish your own frontier"
  block walking the five-command path end-to-end.
- **`vela check --json` returns per-failure detail.** `checks[].errors[]`
  carries the schema validator's `{file, message}` records; `checks[].blockers[]`
  surfaces the `{id, kind, severity, reason}` of every signal that blocks
  strict mode. Pre-v0.9, `--json` reported `failed: 4` with no per-failure
  context.

### Documentation

- **`docs/PUBLISHING.md`** — the end-to-end "first publish" walkthrough.
  Linked from the README quick start; covers scaffold → findings → optional
  cross-frontier deps + links → keypair → actor registration → publish →
  verify on hub → CI republish pattern. Includes the enum table and a
  troubleshooting section keyed off the actual error strings v0.9 emits.
- **README publishing block** at the top of the file. The pre-v0.9
  README's quick start went `compile → check → proof`; the publish path
  was buried at line 39 under HUB.md. v0.9 puts the five-command publish
  path on the front page.

### Versioning

- Workspace version `0.8.0 → 0.9.0`.
- `vela --version → 0.9.0`; banner stamps bump in lockstep.
- Schema version stays at `v0.8.0` — v0.9 ships *no* schema changes. Pre-v0.9
  frontiers (BBB, BBB-extension, the v0.8 conformance vector) replay
  byte-identically; their `vela_version` and `compiler` stamps are publisher-
  claimed and unchanged.

### What is deferred to v0.10+

- Hub-to-hub federation. Forced by ≥ 2 hubs; still 1.
- Hub-hosted frontier blobs. Locator stays where the publisher hosts it.
- Browser-side WebCrypto signing.
- Webhooks/SSE on the hub.
- Multi-frontier Workbench mode (load two frontiers simultaneously into one
  rete view). The dashed-edge-to-ghost-node treatment shipped in v0.8
  remains sufficient for the cross-frontier viewer.
- A real domain (`vela-hub.fly.dev` is sufficient).
- `vela finding rekey` for content-address repair after a hand-edit. v0.9's
  enum guard at add-time removes the most common path into that breakage;
  the cure for the rest is "delete and re-add."

## 0.8.0 - 2026-04-25

The composition run. v0.7 stood up the public hub, the deployed
Workbench, and the BBB living repo. The next substrate move forced
by the protocol's own shape was *composition*: a finding in one
frontier referencing a finding in another. v0.8 closes that gap with
the smallest substrate change that keeps the whole verification
chain — canonical-JSON, signature, snapshot pin — extending across
frontier boundaries.

### Substrate

- **Cross-frontier link targets** (`crates/vela-protocol/src/bundle.rs`).
  `Link.target` now parses as `LinkRef::Local { vf_id }` (in-frontier,
  pre-v0.8 shape) or `LinkRef::Cross { vf_id, vfr_id }` (cross-frontier,
  new). Round-trip identity via `format()`. The wire shape stays
  `String` — canonical-JSON unchanged, no schema churn.
- **`ProjectDependency` extension** (`crates/vela-protocol/src/project.rs`).
  Three new optional fields — `vfr_id`, `locator`, `pinned_snapshot_hash`
  — turn the existing compile-time dependency record into a verifiable
  cross-frontier dep declaration. `Project::cross_frontier_deps()` and
  `Project::dep_for_vfr()` helpers; serde-skipped when None so pre-v0.8
  frontiers serialize byte-identically.
- **Strict cross-frontier validation** (`crates/vela-protocol/src/validate.rs`).
  Any link target with `@vfr_…` must have a matching declared dep; any
  cross-frontier dep must declare both `locator` and `pinned_snapshot_hash`.
  Pinned-by-default — mirrors Cargo.lock / package-lock.json. Strict mode
  fails with the missing dep / missing pin named.
- **Transitive pull-and-verify** (`crates/vela-protocol/src/registry.rs`).
  `pull_transitive(registry, primary_vfr, out_dir, max_depth) -> PullResult`
  walks the dep graph BFS, fetches each dep's frontier, verifies signature
  + snapshot + event-log + that the dep's actual snapshot matches the
  dependent's pinned hash. Cycle-safe (visited-set + content-addressing).
  `vela registry pull --transitive [--depth N]` exposes it; `--depth` defaults
  to 4. Total verification — partial trust isn't a state v0.8 supports.

### CLI

- **`vela frontier add-dep / list-deps / remove-dep`** (`crates/vela-protocol/src/cli.rs`).
  New subcommand group for managing cross-frontier dependency declarations
  on a frontier file. `add-dep` writes a complete
  `vfr_id`+`locator`+`pinned_snapshot_hash` triple; `remove-dep` refuses
  if any link still references the dep.

### Surfaces

- **Hub renders cross-frontier links as click-through**
  (`crates/vela-hub/src/main.rs`). When a finding's link target parses
  as `vf_…@vfr_…` and the target's `vfr_id` matches a declared dep,
  the link becomes `<a href="/entries/{vfr}/findings/{vf}">{vf} @
  {dep_name}</a>` — italic-serif `cross-vfr` badge, navigable to the
  remote frontier's entry page. Undeclared cross-frontier targets get
  a brass `(undeclared dep)` chip.
- **Workbench rete: dashed cross-frontier edges + ghost nodes**
  (`site/src/pages/workbench/index.astro`). External `vfr_id`s appear
  as small open-square ghost nodes pinned to the canvas rim, one per
  distinct external frontier. Edges to ghosts are dashed signal-blue.
  Click a ghost to jump to the hub's `/entries/{vfr}` page.

### Conformance

- **2-frontier conformance vector** (`tests/conformance/cross-frontier/`).
  Frontier A (1 finding, no deps) + Frontier B (1 finding linking to A
  via `vf_…@vfr_…`, declares A as a dep) + `expected.json` listing every
  derived id and the resolution shape. A second implementation grades
  itself by reproducing each id and confirming the dep's snapshot pin
  matches A's actual snapshot.
- **`scripts/cross_impl_conformance.py --cross-frontier <path>`** loads
  each declared dep and checks two new properties: every cross-frontier
  link resolves to a declared dep, and every dep's `pinned_snapshot_hash`
  matches the loaded dep's actual snapshot. Verified PASS on the
  positive vector and FAIL (exit 1) on a tampered copy.

### Worked example

- **`frontiers/bbb-extension.json`** + `.github/workflows/bbb-extension-living-repo.yml`.
  A small companion frontier ("BBB Flagship · follow-up") with one
  finding that extends BBB's first finding via the v0.8 link-target
  syntax. Declares BBB as a cross-frontier dep with the v0.8 vfr_id
  and snapshot pin. Separate `reviewer:bbb-extension-bot` actor;
  weekly cron 14:30 UTC (offset from BBB's 14:00). The hub now serves
  two frontiers; `vela registry pull vfr_… --transitive --from
  https://vela-hub.fly.dev/entries` walks both end-to-end.

### Cut

- Workspace + crate versions: `0.7.0 → 0.8.0`.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  → `v0.8.0` / `0.8.0` / `vela/0.8.0`.
- `default_formula_version() → "v0.8"` (cosmetic; same scoring math).
- `frontiers/bbb-alzheimer.json` and `examples/paper-folder/expected/frontier.json`
  migrated.
- `schema/finding-bundle.v0.8.0.json` published.
- All command banners (`compile`, `bridge`, `jats`, `ingest`,
  `frontier`) → `V0.8.0`.

### Deferred to v0.9+

- Hub-to-hub federation (still needs ≥ 2 hubs).
- Hub-hosted frontier blobs. The locator stays wherever the publisher
  hosts the file.
- Browser-side WebCrypto signing.
- Webhooks / SSE on the hub.
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method.
- Tier-permitted auto-apply for state-changing kinds.
- Per-pubkey rate limits, allowlists, abuse handling.
- Multi-frontier Workbench mode (loading two frontiers into one rete
  view; v0.8 ships dashed-edge-to-ghost-node only).
- A real domain.

## 0.7.0 - 2026-04-25

The public-hub run. v0.6 left the substrate complete and gave us a
local Postgres-backed hub. v0.7 puts the hub on a public URL, opens
the publish path, and stands up the BBB living-repo workflow. "There
is somewhere visible to send a signed manifest" stops being theatre.

### Hub

- **`POST /entries`** on `crates/vela-hub`. Anyone can submit a signed
  manifest; the hub deserializes, calls
  `vela_protocol::registry::verify_entry`, and INSERTs with `ON
  CONFLICT (vfr_id, signature) DO NOTHING`. 201 fresh, 200 duplicate,
  400 tamper or schema mismatch, 500 DB error. Doctrine: the signature
  is the bind, not access control. No allowlist, no rate limit.
- **`UNIQUE (vfr_id, signature)`** on `registry_entries` carries the
  substrate's idempotency guarantee into the transport. Byte-identical
  replays dedupe at the DB layer.
- **Public deploy** at <https://vela-hub.fly.dev>. `crates/vela-hub`
  ships with `Dockerfile` + `fly.toml` + `.dockerignore`. The Fly app
  runs in the `vela-237` org behind a fresh Postgres role with
  `INSERT/SELECT` only on `registry_entries`, distinct from the dev
  sandbox. Production credential lives only in Fly secrets.

### Substrate

- **`registry::publish_remote(entry, hub_url) -> PublishResponse`**
  in `crates/vela-protocol/src/registry.rs`. POSTs canonical bytes via
  `reqwest::blocking`; surfaces `{ok, vfr_id, signed_publish_at,
  duplicate}` from the hub.
- **`vela registry publish --to https://...`** routes through
  `publish_remote`. Local file paths and `file://` URLs keep working
  byte-identically. The signing path (`sign_entry`) is unchanged
  whether the destination is a file or a hub.

### BBB living repo

- **`reviewer:bbb-bot`** registered in `frontiers/bbb-alzheimer.json`
  with `tier=auto-notes`. The bot's private key lives only in the
  `VELA_BBB_BOT_KEY` GitHub Actions secret; the local copy is wiped
  after registration, so rotation requires generating a new key, not
  reading the secret out.
- **`.github/workflows/bbb-living-repo.yml`** (Mondays 14:00 UTC,
  also `workflow_dispatch`). Builds the CLI, signs, POSTs to
  `https://vela-hub.fly.dev`, summarizes `vfr_id` + `snapshot_hash` +
  `event_log_hash` + `signed_publish_at` in the job summary.
  Recompilation lives outside CI (it would need LLM credentials and
  human review); the workflow republishes whatever's in `main` with a
  fresh `signed_publish_at`, which is enough for the "living repo"
  claim.

### Docs

- New [docs/HUB.md](docs/HUB.md): doctrine, endpoints, publish/pull
  recipes, the CI-bot pattern, self-hosting notes, operational
  hygiene around credentials.
- [docs/REGISTRY.md](docs/REGISTRY.md) updated for HTTP push.
- [README.md](README.md) names the public hub URL.
- [scripts/hub-publish.sh](scripts/hub-publish.sh) header reframed as
  optional — direct-DB path remains for backfills, but `vela registry
  publish --to https://<hub>` is preferred.

### Cut

- Workspace + crate versions: `0.6.0 → 0.7.0`.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  → `v0.7.0` / `0.7.0` / `vela/0.7.0`.
- `default_formula_version() → "v0.7"` (cosmetic; same scoring math).
- `frontiers/bbb-alzheimer.json` and
  `examples/paper-folder/expected/frontier.json` migrated.
- `schema/finding-bundle.v0.7.0.json` published.
- Command banners (`compile`, `bridge`, `jats`, `ingest`,
  `frontier`) bumped to `V0.7.0`.

### Deferred to v0.8+

- Cross-frontier links (`vf_…@vfr_…` references) — verifiable
  composition. Defer until v0.7 generates pull pressure for it.
- Hub-to-hub federation. Defer until ≥ 2 hubs exist.
- Hub-hosted frontier blobs. Locator points elsewhere; the hub is
  manifest-only.
- Browser-side WebCrypto signing. Drafts-then-CLI-signs unchanged.
- Webhooks / SSE on the hub.
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method.
- Tier-permitted auto-apply for state-changing kinds.
- Per-pubkey rate limits, allowlists, abuse handling.
- A real domain. The Fly URL is sufficient for v0.7.

## 0.6.0 - 2026-04-25

The trusted-agent run. v0.5 made the substrate writable, reviewable, and
distributable. The Sonnet-vs-Haiku stress test surfaced three concrete
friction items, all driven by real pain. v0.6 fixes them without
re-opening the sprawl problem the v0.3 focusing run closed.

### Substrate

- **Trust-tiered auto-apply** (`sign.rs`, `serve.rs`, `tool_registry.rs`).
  `ActorRecord.tier: Option<String>` registered alongside the pubkey.
  The only tier in v0.6 is `"auto-notes"`. New MCP tool
  `propose_and_apply_note` signs once and applies in one call when the
  actor's tier permits the kind. Doctrine: tiers permit review-context
  kinds only — never state-changing kinds (review, retract, revise,
  caveated). New CLI flag `vela actor add --tier auto-notes`. Halves
  the signing surface for trusted bulk-note extractors.
- **Structured note provenance** (`bundle.rs`, `events.rs`,
  `reducer.rs`, `proposals.rs`). `Annotation.provenance:
  Option<ProvenanceRef>` with `{doi?, pmid?, title?, span?}`. The
  `finding.note` and `finding.caveated` payload schema accepts an
  optional `provenance` object; at least one of doi/pmid/title must be
  set when present. Provenance threads through proposal → applied event
  → materialized annotation, so reviewers can query "show every
  annotation from PMID X" via a typed field rather than parsing prose.
- **Workbench live triage surface**
  (`web/previews/live-frontier.html`, `live-finding.html`,
  `web/scripts/workbench.js`). Two new live pages alongside the static
  brand-canon fixtures: `live-frontier.html` is a live findings table
  with client-side search/filter/scope chips, click-through to live
  detail; `live-finding.html` is a two-column triage view (full
  finding bundle on the left with linked DOI/PMID; queued-review
  sidebar on the right with accept/reject buttons that POST to
  `/api/queue` for `vela queue sign`). The Ed25519 key never enters
  the browser. `proposals.html` proposal-target IDs hyperlink to
  `live-finding.html` for triage navigation.

### Conformance + docs

- New conformance suites: `auto-apply-tier.json` (7 cases pinning the
  tier-gate predicate) and `note-provenance.json` (2 cases pinning
  the canonical preimage shape with/without provenance). Total: 47 → 56
  cases.
- New `docs/TIERS.md` — full tier model, doctrine, idempotency, and
  forward-compat semantics.
- Updated `docs/MCP.md` (tool count 17 → 18), `docs/WORKBENCH.md` (live
  pages as entry surface), `docs/PYTHON.md` (`propose_and_apply_note`
  and `provenance` examples).

### Substrate metadata

- `Cargo.toml` workspace version: 0.5.0 → 0.6.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.6.0.
- `schema/finding-bundle.v0.6.0.json` published.
- `default_formula_version()` → `"v0.6"`.
- `vela --version` → `vela 0.6.0`.
- BBB fixture and paper-folder example migrated.
- All command banners read V0.6.0.

### Deferred to v0.7+ (intentionally)

- Cross-frontier links (`vf_…@vfr_…`).
- Hosted hub at `hub.vela.science`.
- Federation peers / gossip protocol.
- Multi-frontier workspace primitive.
- Hosted Workbench (multi-user, deployed).
- Browser-side WebCrypto signing.
- HTTP / git transports for registries.
- Webhooks (pull/SSE remains sufficient).
- `vela ingest --paper <path> --propose` CLI shortcut.
- `propose_with_routing` SDK method (entity-overlap routing).
- Tier-permitted auto-apply for state-changing kinds.

The substrate is now strong enough to host these without re-deriving
the protocol. v0.6 leaves the next investment outside the substrate:
make BBB a public living repo, write the canonical essay, find the
first external writer.

## 0.5.0 - 2026-04-25

The accessible-substrate run. v0.4 hardened the kernel; v0.5 makes it
writable from anywhere a writer needs to be — by AI agents through MCP
and HTTP, by human reviewers through a Workbench wired to live state,
and by other Vela instances through a verifiable-distribution registry.

### Substrate

- **Content-addressed proposals + idempotent apply** (`proposals.rs`).
  `created_at` is no longer in the `vpr_…` preimage. Identical logical
  proposals at different timestamps deterministically produce the
  same id. `create_or_apply` is upsert-by-content-address: agent
  retries return the same proposal_id and applied_event_id, with no
  duplicate proposal or event in the frontier.
- **Read-stream API** (`serve.rs`, `tool_registry.rs`).
  `GET /api/events?since=<vev_…>&limit=<n>` and the matching MCP tool
  `list_events_since` give cursor-paginated reads over the canonical
  event log. Same surface serves agent-loop completion signals and
  public-consumer diff watching. No auth on read.
- **Write surface (MCP + HTTP)** (`serve.rs`, `tool_registry.rs`,
  `sign.rs`). Six new tools: `propose_review`, `propose_note`,
  `propose_revise_confidence`, `propose_retract`, `accept_proposal`,
  `reject_proposal`. Each requires a registered actor (Phase M from
  v0.4) and an Ed25519 signature over the canonical preimage.
  `sign::proposal_signing_bytes` and `sign::verify_action_signature`
  reuse the same canonical-JSON discipline as `vev_…`/`vpr_…` derivation.
- **Workbench: drafts + CLI signs** (`web/previews/proposals.html`,
  `crates/vela-protocol/src/queue.rs`, new `vela queue list/sign/clear`).
  `vela serve --workbench` mounts `web/` alongside the API.
  Browser POSTs unsigned decisions to `/api/queue`; `vela queue sign`
  walks the queue, signs with the actor's key, and applies. The
  Ed25519 private key never enters the browser.
- **Registry primitive: verifiable distribution**
  (`crates/vela-protocol/src/registry.rs`, new `vela registry
  add/list/publish/pull`). Flat signed manifests
  `(vfr_id, name, owner, snapshot_hash, event_log_hash, locator,
  timestamp, signature)`. Pull verifies signature plus
  snapshot_hash plus event_log_hash; any mismatch is total
  rejection. Latest-publish-wins. `file://` and bare-path
  transports; HTTP/git deferred to v0.6.

### Adoption surface

- **Python SDK** (`bindings/python/vela/__init__.py`). Single-file
  client for `vela serve --http`. `Frontier.connect()`, `list_findings`,
  `events_since` generator, signed `propose_*` methods,
  `accept`/`reject`. Reuses the canonical-JSON rule in Python so
  `vpr_…` and signature derivation are byte-identical to the Rust
  kernel.
- **Hello-world agent** (`examples/python-agent/extract_and_propose.py`).
  Paper text → optional Anthropic-API claim extraction → propose
  notes against a live frontier → events_since print-out → pointer at
  the Workbench. ~50 lines of agent code.

### Conformance + docs

- New conformance vectors: `tests/conformance/proposal-idempotency.json`,
  `tests/conformance/registry-publish-pull.json`. Total: 47 cases.
- New docs: `docs/MCP.md`, `docs/WORKBENCH.md`, `docs/REGISTRY.md`,
  `docs/PYTHON.md`. Each is the public contract for its v0.5 surface.

### Substrate metadata

- `Cargo.toml` workspace version: 0.4.0 → 0.5.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.5.0.
- `schema/finding-bundle.v0.5.0.json` published.
- `default_formula_version()` → `"v0.5"` (cosmetic; same scoring math).
- `vela --version` → `vela 0.5.0`.
- BBB fixture and paper-folder example migrated to v0.5 schema URLs
  and formula versions.
- All command banners (`compile`, `bridge`, `jats`, `ingest`, `actor`,
  `queue`, `registry`) read V0.5.0.

### Deferred to v0.6 (intentionally)

- Cross-frontier links (`vf_…@vfr_…` references). Composition is a
  separate value prop from distribution.
- Hosted hub (`hub.vela.science`). v0.5's registry is local +
  `file://` URL; managed hub is operational, not protocol.
- Federation peers / gossip protocol. Push/pull only in v0.5.
- Multi-frontier workspace primitive.
- Hosted Workbench (multi-user, deployed). Local-only Workbench in
  v0.5.
- Browser-side signing via WebCrypto. The drafts-then-CLI-signs
  model is the v0.5 doctrine.
- HTTP/git transports for registries.
- Webhooks. Pull/SSE is sufficient for v0.5.

The substrate is now strong enough to host these without re-deriving
the protocol.

## 0.4.0 - 2026-04-25

The substrate-hardening run. v0.3 made the kernel a real protocol; v0.4
makes its load-bearing claims doctrine-grounded rather than convenient.

### Substrate

- **`frontier.created` is a real `events[0]` genesis event**
  (`crates/vela-protocol/src/project.rs`).
  Every freshly compiled frontier emits a canonical event whose
  hash IS the frontier_id. `frontier_id_from_genesis(events)`
  derives `vfr_…` from the same canonical preimage shape as
  `vev_…`, so a second implementation follows one rule. Legacy
  v0.3 frontiers without a genesis event fall back to meta-derivation.
- **Canonical/derived packet split** (`packet.rs`).
  `CANONICAL_PACKET_FILES` (15) carry replay-bearing protocol state;
  `DERIVED_PACKET_ARTIFACTS` (13) ship for inspection but are
  regenerable projections. `proof-trace.checked_artifacts` requires
  canonical only — derived artifacts are validated structurally.
- **Retraction cascade as per-dependent canonical events**
  (`proposals.rs`, `events.rs`, `reducer.rs`).
  A retraction now emits one `finding.dependency_invalidated` event
  per affected dependent in BFS depth order, each carrying
  `upstream_finding_id`, `upstream_event_id`, and `depth`. A pure
  reducer reproduces post-cascade state from the event log alone —
  no hidden propagation in summary fields.
- **Registered actors and signed events under `--strict`**
  (`sign.rs`, `signals.rs`, new `vela actor add/list` CLI).
  `Project.actors` maps stable actor.ids to Ed25519 public keys.
  `--strict` emits `unsigned_registered_actor` and blocks
  strict_check whenever a registered actor writes an event without a
  verifying signature. `event_signing_bytes`, `sign_event`,
  `verify_event_signature` operate on the same canonical preimage
  shape as `vev_…` derivation.
- **Provenance authority unification** (`sources.rs`, `signals.rs`,
  `vela normalize --resync-provenance`).
  `Project.sources` is canonical; `FindingBundle.provenance` is the
  denormalized cache. `--strict` emits `provenance_drift` blockers
  when title/year disagree; `vela normalize --resync-provenance --write`
  rewrites the cache from the canonical SourceRecord.

### Doctrine

- The reducer now treats `frontier.created` as a structural anchor
  and `finding.dependency_invalidated` as a state-mutating event.
- Five new `--strict` doctrine signals: cascade events, registered-actor
  signatures, provenance drift, plus the three v0.3 signals
  (conditions_undeclared, evidence_atom_missing, agent_typed_unreviewed).
- Schema URL bumped from v0.3.0 → v0.4.0; confidence
  `formula_version` defaults to `"v0.4"`.
- `vela --version` reports `vela 0.4.0`.

### Substrate metadata

- `Cargo.toml` workspace version: 0.3.0 → 0.4.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`
  bumped to v0.4.0.
- `schema/finding-bundle.v0.4.0.json` published.
- `frontiers/bbb-alzheimer.json` and the paper-folder fixture
  migrated to v0.4 schema URLs.
- All command banners (`compile`, `bridge`, `jats`, `ingest`, etc.)
  now read V0.4.0.

### Deferred to v0.5 (intentionally)

- `vela event sign` CLI for minting signatures on existing events.
- A registry (`hub.vela.science`) and federation peers.
- Multi-frontier workspace primitive.
- Cross-frontier links and propagation across frontier boundaries.
- Constellation projection product surface.

The substrate is now strong enough to host these without re-deriving
the protocol.

## 0.3.0 - 2026-04-24

The focusing run. v0.3 turns the state kernel into a real protocol — one
that two implementations can independently produce byte-identical IDs
for, that has a separable reducer, that enforces doctrine at the kernel
level, that emits typed events, that carries a stable address primitive,
and that calls itself a coherent version on every surface.

This is the v0 the doctrine has been describing all along.

### Protocol — substrate

- **Canonical JSON hashing** (`crates/vela-protocol/src/canonical.rs`).
  Every content-addressed ID — `vf_…`, `vev_…`, `vpr_…`, snapshot hash,
  event-log hash — now derives from RFC 8785-style canonical JSON
  (lexicographic key ordering at every depth, no whitespace, validated
  finite numbers, UTF-8 strings preserved verbatim). A second
  implementation conforming to the canonical-JSON rule produces
  byte-identical hashes for the same logical content.
- **Pure separable reducer** (`crates/vela-protocol/src/reducer.rs`).
  `apply_event(state, event)` is the deterministic state-transition
  function. The reducer is callable independently of proposal
  construction, so canonical event logs can be replayed from genesis
  by any conforming implementation.
- **Per-kind event payload validation**
  (`events::validate_event_payload`). Each event kind has a normative
  payload schema; payloads that don't match are conformance failures.
  Replay reports surface them as conflicts; `vela check --strict`
  treats them as failures.
- **frontier_id as address primitive**. Every frontier carries a
  `vfr_<hash>` derived from canonical creation metadata. The same
  triple (name, compiled_at, compiler) always produces the same vfr_id.
  Legacy v0.2 frontiers derive on read.

### Protocol — semantics

- **Typed three-state review verdict.** The pre-v0.3 collapse of
  contested / needs_revision / rejected to one bit becomes
  `Flags.review_state: Option<ReviewState>` with explicit variants.
  `flags.contested` is preserved as a derived bit for v0.2 readers.
- **Confidence formula version stamp.** `ConfidenceComponents.formula_version`
  now defaults to `"v0.3"`. A second implementation can refuse to
  interpret components computed under an unknown formula version.

### Doctrine invariants enforced under --strict

- `conditions_undeclared` (line 3): a finding with empty conditions and
  no scope flag (in_vivo / in_vitro / human_data / clinical_trial), and
  not theoretical, blocks strict_check.
- `evidence_atom_missing` (line 4): every active finding must have at
  least one materialized evidence atom. Lifted from packet-validation-
  only into vela check.
- `agent_typed_unreviewed` (line 5): findings with source_type =
  model_output / expert_assertion / agent_trace require explicit
  review or gap-flag before strict acceptance. Doctrine: an agent
  trace is not truth without typed consequence.

### Pruning

- `vela ask`, `vela workspace`, `vela depend`, `vela merge` removed.
  All four were premature consumer or multi-frontier surface that
  v0.3's substrate-first focus rejects. ~2200 LOC excised.
- `flags.gap_info` and the GapStatus / GapPriority / GapNote / GapInfo
  types removed. GitHub-issue-tracker fields on a finding had no
  doctrine motivation. `vela gaps rank` (the doctrine-aligned derived
  ranking) stays.
- `crates/vela-protocol/src/gaps.rs` deleted.

### Substrate metadata

- `Cargo.toml` workspace version: 0.2.0 → 0.3.0.
- `VELA_SCHEMA_URL` / `VELA_SCHEMA_VERSION` / `VELA_COMPILER_VERSION`:
  v0.2.0 → v0.3.0.
- `vela --version` reports `vela 0.3.0`; `print_strict_help` masthead
  reads `Vela 0.3.0`.
- `frontiers/bbb-alzheimer.json` and the paper-folder fixture migrated
  to v0.3 schema URLs.
- `schema/finding-bundle.v0.3.0.json` published.

### Deferred to v0.4 (intentionally)

- Cross-frontier links and propagation across frontier boundaries.
- A registry (`hub.vela.science`) and federation peers.
- Multi-frontier workspace primitive.
- A `frontier.created` canonical event in `events[0]` and per-finding
  asserted events from compile (the genesis-event surface is in place
  as `derive_frontier_id_from_meta`; v0.4 promotes it to a proper
  event log entry).
- Identity-bound signatures required under --strict.
- Canonical/derived packet split.
- Provenance-authority unification (sources canonical, finding.provenance
  derived).
- Retraction cascade as per-dependent canonical events.

These are real next-chapter work that's enabled — not blocked — by what
landed in v0.3. The substrate is now strong enough that they can be
built without re-deriving the protocol.

## 0.2.1 - 2026-04-24

This is a design-unification pass on top of the v0.2.0 release shape. No
protocol, schema, or proof-packet format changes. CLI output, docs voice, and
brand surface now share one canon.

### Design canon

- Ships `assets/brand/` with mark, wordmark, favicon, rete motif, and OG image.
- Adds `docs/BRAND.md` as the single reference for voice, color tokens, type
  families, asset usage, and the tick motif.
- Adds `web/` with a GitHub Pages-ready static landing page at `web/index.html`
  using the design-system tokens.
- Stages the proposed post-v0 product surface as static previews under
  `web/previews/` — explicitly labeled as proposals, not shipping v0 product.

### CLI surface

- Rebuilds banners across `compile`, `stats`, `validate`, `depend`, `diff`,
  `tensions`, `serve --check-tools`, `jats`, and conformance output. Every
  banner is now a dim mono eyebrow + tick row, never `===` or `---` separators.
- Reserves signal blue for live state only. Removes `.green()` traffic-light
  coloring from numeric counts and success indicators; maps `PASS/FAIL` and
  success/failure outcomes to engraved state chips (moss / brass / dust /
  madder).
- Gates all ANSI output on `stdout` being a terminal and `NO_COLOR` being
  unset. Piped and `NO_COLOR=1` runs emit no escape codes.
- Retools the `compile` progress bar to a hairline motif (`── ` fill) and `·`
  as the separator.
- Adds `crates/vela-protocol/src/cli_style.rs` as the single routing point for
  palette, chips, eyebrow, tick row, progress-bar style, and error prefix.

### Docs voice

- Rewrites the `README.md` opener as one concrete grounding sentence before
  introducing the core vocabulary. Adds the wordmark header and a footer link
  to `docs/BRAND.md` + the landing page.
- Unifies `belief state` → `frontier state` in `docs/PROTOCOL.md` and
  `docs/CORE_DOCTRINE.md`. `docs/MATH.md` keeps `belief state` with a footnote
  linking theory-side and operational nomenclature.
- Fixes title-case h3s in this `CHANGELOG.md` to sentence case.
- Adds `scripts/voice-check.sh` and wires it into `scripts/release-check.sh`.

## 0.2.0 - 2026-04-23

This is the first strict OSS release candidate for Vela v0.

### Core release shape

- Consolidates the public product around portable frontier state for science.
- Keeps the release workflow focused on `compile`, `check`, `proof`, `serve`, and `bench`.
- Removes tangential UI, runtime, inherited coding-agent, archive, and generated artifact surfaces from the tracked release repo.
- Keeps BBB/Alzheimer as the canonical proof frontier and demo path.

### Protocol and proof

- Uses schema v0.2.0 for the checked-in release frontier.
- Exports deterministic proof packets with `proof-trace.json`.
- Validates proof traces when packet validation sees them.
- Adds canonical release asset packaging for the BBB frontier, proof packet, check report, benchmark report, manifest, and checksums.

### Benchmarking

- Promotes `vela bench` as the public benchmark command.
- Adds default BBB benchmark inputs.
- Adds thresholded pass/fail behavior for finding benchmarks.
- Documents benchmark JSON as a compatibility surface.

### Release operations

- Adds `scripts/release-check.sh` as the local release gate.
- Adds `scripts/package-release-assets.sh` for release assets.
- Adds `scripts/clean-clone-smoke.sh` for fresh-clone verification.
- Updates installer behavior and release workflow packaging.
