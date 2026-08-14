# Vela ecosystem architecture

- Status: current as of 2026-08-09
- Binding decision: ADR 0039, `docs/adr/0039-repository-authority-and-derived-frontiers.md`
- Governing simplification: the 2026-08-08 ideal ecosystem and architecture memo

This document states the ecosystem structure once. Where a layer exists, the
path is given. Where a layer does not exist, it is named as future with the
gate that would open it. No layer is named that names nothing.

## 1. The four boundaries

ADR 0039 split one overloaded word into four boundaries and one projection.

| Boundary | What it bounds | Identifier |
| --- | --- | --- |
| Repository | authority: Git repository, trust root, canonical history, Standing | `repository_id`, canonical RFC 9562 UUIDv4 |
| Source | provenance: exact observations of external systems, never governed | `source_id`, prefixed `source:` |
| Problem | one bounded scientific question | native source identifier |
| Frontier | derived: the unresolved state around one or more Problems; owns nothing | none, by design |
| Atlas | projection across the four above | none; a read surface |

The rule underneath all of it: **a repository exists because there is a new
authority, never because there is a new topic.**

### Where each boundary is in code

**Repository.** `crates/vela-protocol/src/objects/repository.rs`
(`RepositoryV4`), the authority history in `crates/vela-authority/`,
protocol replay in `crates/vela-protocol/`. Scientific methods and their native
runtimes remain source-owned; Core records scoped Verification results and does
not execute domain evidence. The rename landed in v0.967.0: `vfr_` and `frontier_id` are absent
from current crate code. The type parses `vela.toml`, mints a standard UUIDv4
once at genesis, and answers `vela.status.v4`.

**Source.** Declared by the source-owning integration and bound to exact native
identities. Core ships no source acquisition or lock package. Source state is
projected by `vela-web` into its derived source declarations,
`projection.source_observations`, `projection.native_records` and
`projection.release_sources` (`packages/projection-data/schema.sql`). Thirteen
sources are declared by the current Web projection in
`packages/projection-data/config/math-sources.v1.ts`; they are not declarations
inside the Math Repository. Adapters live in
`packages/projection-data/src/source-adapters/` (fifteen files, including
`formal-conjectures.ts`, `oeis.ts`, `physlib.ts`, `openai-ten-proofs.ts` and
`vibemathed.ts`).

The containment property is enforced in SQL, not prose.
`projection.repository_source_bindings` carries
`CHECK (binding_kind = 'admission' OR local_standing_effect = 'none')`. A
reference or snapshot of an external source cannot carry Standing.

**Problem.** No protocol object. No projection table. The surface at
`apps/problems/src/app/repositories/[slug]/problems/` is derived, in
`packages/projection-data/src/index.ts`, from `projection.native_records` scoped
through `release_sources`, with the Claim a `LEFT JOIN LATERAL` and the declared
status, formalization, prize and subject tags read from the Source record's
metadata. It used to be an inner join from `observatory.graph_nodes` to
`observatory.claims` with prize and tags regex-extracted from the Claim's
assertion string, which is the defect ADR 0039 named; §6 records the repair.
This paragraph described the defect in the present tense for as long as §6
described its replacement, and this paragraph is the one a reader reaches first.

**Frontier.** Derived, and no longer minted in the protocol. In `vela-web`,
`registry.ts` pins one slug, `math`, against `repository_id`
`8138c6da-46c4-47ee-b493-5bbfbec09b1e`. The compact Vela 0.975.1 Math genesis
is current. No `vfr_` or `vrepo_` identity survives in the current reader.
Canonical projection joins key on `repository_id`, and the slug is a
presentation fact that lives only in the registry, where a URL handle meets a
protocol identity.

**Atlas.** The replaceable Problems projection, `apps/problems/` in `vela-web`. There is no
separate Atlas application, no Atlas compiler, no per-view ontology. Every
projection row is root-bound. `packages/projection-data/schema.sql` declares 19
tables, of which 16 carry `row_root`; the other three — `releases`,
`schema_migrations` and `current_release` — are the projection's own bookkeeping
and hold no projected row. Of the 16, all but three also carry `release_root`,
and those three are `source_declarations`, `source_observations` and
`native_records`, which are content-addressed and reach a release through
`release_sources`. The count of tables and the count of root-bound rows were
stated as one number here, which made the bookkeeping tables look like state.

## 2. Is there a math hub?

**No hub. There is one repository.**

`vela-science/math` is a Repository under one authority. It is not a domain
library, not a package namespace, not an index of other people's mathematics,
and not a second Mathlib. Nothing in it is a hub for anything outside it.

Its current compact genesis has UUID
`8138c6da-46c4-47ee-b493-5bbfbec09b1e`, origin
`vro_be55672495053325`, genesis generation 1, and replays from a clean clone at
commit `08a0e6d327e1ae9937ab2e0e5002192815eac69a`. It holds two current accepted
Claims, three authenticated Submissions, three scoped Verifications, and three
accepted Proposal transitions. One retained predecessor Claim projects current
Standing `superseded`; no rejected or pending Proposal remains.

**New nouns added: zero.** `vela-science/math` is an instance of Repository,
which already exists.

The reasoning is the ADR 0039 rule read forward. Four repositories existed
because there were four topics; they named the same maintainer and the same
decision model, so four trust roots bought four Standing universes that could
not see each other. One authority gets one repository. That repository is named
for its subject because it has to be named something, and the name is not the
justification. If a second mathematics authority ever appears, with a different
maintainer set and a different decision model, that is what earns a second
repository.

This closed a documented conflict rather than dodging it. ADR 0038 §119 lists
"a `vela-math` repository" under rejected alternatives, and `docs/ROADMAP.md`
and `docs/CAMPAIGN.md` once carried the same line. ADR 0039's supersession
clause names that prohibition in all three places and supersedes it "only in
the narrow sense stated below". Its §11 states the sense: what those documents
reject is a Vela-owned mathematics *library* and a second canonical database,
and that rejection stands. A single mathematics *authority* is a different
object, so the prohibition is restated rather than lifted. ROADMAP and CAMPAIGN
now say so in their own words, ROADMAP pointing at ADR 0039 §11 where the old
line was. ADR 0038 keeps its wording, which is what a superseded ADR is for.

## 3. Is there a registry?

Two different things get called a registry. One exists. One is not a
destination.

### The Math Source Registry: exists

It is a source-owned Web projection over explicit adapters, occurrence
snapshots, and native records. It is not a Repository object and the current
Math genesis intentionally carries no `sources.yaml` or source-manifest
runtime. Its schema and read surface live with the replaceable Problems
projection, not with Vela Core or Repository authority.

**New nouns added: zero.** Source is already one of the four boundaries. The
registry is a view of it, and a view is not a noun.

It observes. It never governs. A source binding is exact only to the degree its
source-owned adapter and retained occurrence evidence establish; it creates no
Verification, Decision, Event, or Standing.

### The Vela Package Registry: not a destination

Vela does not build a package manager and does not build a hosted package
registry. The 2026-08-06 position is that distribution runs through native
registries (§5), and that a discovery index may index but must never become a
second package manager.

**New nouns a package layer would add: at least six** — Package, Package
Registry, package coordinate, package manifest, package lock, package impact
notice. Each needs a schema, a lifecycle and a surface, and none of them is the
thing Vela is for. That is why the answer is no rather than later.

The experiment was actually run and it failed its own gate.
`research/lean-replay-contract-evidence/qualification.json` records the
candidate consumed by two independent readers at pinned commits, with
`"linux_network_disabled_native_replay": false`, `"net_deletion": false`,
`"level_1_promotion": false` — 1,017 lines added and 0 deleted. Two gates
failed, not one; the network-sandbox gate is independent of the consumers. Both
of the named consumers are now frozen repositories, so the experiment cannot be
re-run as designed. It is closed as answered in the negative rather than carried
as open backlog, and the candidate package itself has left the tree — Git
history retains it. The qualification record stays because it is the answer.

`docs/ARCHITECTURE.md` now states the same closed decision: native package
ecosystems own resolution and publication; the two-consumer and net-deletion
gate governs ordinary code extraction rather than opening a roadmap to a Vela
package manager, package repository, or hosted registry.

## 4. Repository topology

### Live

| Repository | Sole responsibility | Status |
| --- | --- | --- |
| `vela-science/vela` | Protocol, CLI, conformance readers and fixtures, releases, architecture | exists |
| `vela-science/vela-web` | Editorial site and root-bound Problems projection | exists |
| `vela-science/.github` | Organization profile, reusable workflows, security policy | exists |
| `vela-science/math` | The one live mathematics authority, fresh genesis | exists |

`vela-science/math` exists with a compact Vela 0.975.1 genesis, UUID
`8138c6da-46c4-47ee-b493-5bbfbec09b1e`, two current accepted Claims, three
Submissions, three Verification Records, and three accepted Proposal
transitions. Strict replay at
`08a0e6d327e1ae9937ab2e0e5002192815eac69a` yields Repository root
`sha256:3e2236510923277c1e363d2d28c3d84d86a1d698bafd576b79308b18ae0cf0d2`.

The current genesis uses one DSSE transport implementation, the closed
authorization model, genesis-only origin, and RFC 9562 UUIDv4 Repository
identity. The pre-Coherence lineage remains reachable only as an ordinary Git
rollback tag during the release window and contributes no Standing to the
current genesis.

### Frozen

`vela-science/erdos-frontier`, `vela-science/sidon-frontier`,
`vela-science/quantum-codes-frontier`,
`vela-science/formal-conjectures-frontier`.

All four are archived on GitHub and read-only. Preserved exactly as historical
Vela repositories. No further architecture work inside them beyond integrity
fixes. `formal-conjectures-frontier` is
additionally dissolved: all 18 of its Claims are Erdős problems and both its
declared sources are already declared by `erdos-frontier`.

**The current binary cannot read any of them, and says so.** Their profiles are
`vela.frontier-profile.v1` in `frontier.toml` declaring `frontier_id`, so they
carry neither the `vela.toml` nor the `.vela/origin.json` and
`.vela/repository.json` pair every read path requires. `require_initialized_repo`
in `crates/vela-cli/src/ui.rs` refuses them by name —
`repository_predecessor_layout`, "this Vela release verifies only current
repository origins" — and points at the pinned historical release instead.
`v0.966.4` is that release. Held shut by
`current_replay_refuses_retired_repositories_before_parsing_them` in
`crates/vela-cli/tests/genesis.rs`.

Even reached directly, the profile would not parse:
`RepositoryProfileV1` carries `#[serde(deny_unknown_fields)]` and a
required `repository_id`
(`crates/vela-protocol/src/objects/repository.rs:45-69`). ADR 0039 §8
explains why no alias was available: `RepositoryV4::parse` re-serializes
and compares bytes, so a `#[serde(alias)]` would still fail the canonical-bytes
check.

That is a deliberate cost, not an oversight, and it is a real one — the
accepted Claims those four hold (ADR 0039 records 2,782 as the Observatory
reported them) are retained and unreadable by the tool that wrote them.
Re-admitting valuable state through ordinary Submission → Verification →
Decision remains the sanctioned path, with the old roots kept as provenance.

This section previously went further and called that re-admission "the
cross-repository transfer experiment", with the frozen repositories standing in
as the second authority. That is ADR 0039 §9, which the same ADR's same-day
amendment withdrew. Authority containment — RQ3, evidence level 2, and gate B8
of `WHITEPAPER_CONTRACT.md` — needs two live authorities and there is one. It is
future work pending a second, and the amendment accepts that as a loss rather
than mitigating it.

### The rule for adding a repository

A repository is created when, and only when, **there is a new authority**: a
different maintainer set, a different decision model, a different correction
policy, or a confidentiality boundary that cannot be expressed inside an
existing one.

A repository is never created because of: a new topic, a new corpus, a new
paper, a new data source, a new verifier implementation, a new model, a new
campaign, a new branch, or one important problem.

For the three non-scientific repositories the additional test is an independent
deployment lifecycle with its own release cadence. `vela` and `vela-web` pass
it; `.github` passes it because organization policy outlives any one repository.

## 5. Packaging and distribution

Vela publishes through the native registry for each artifact type. There is no
Vela-format distribution channel.

| Artifact | Channel | Lock / pin | Exists today |
| --- | --- | --- | --- |
| Rust libraries | crates.io | `Cargo.lock`, `--locked` on every command | Cargo.lock present; nothing published yet |
| Rust toolchain | rustup | `rust-toolchain.toml` | yes |
| Source-owned Python tools | Their source repository | `uv.lock`, `uv run --locked` | A current integration that actually uses them |
| TypeScript | npm, `@vela-science/*` | `bun.lock` (`vela-web` uses `bun@1.3.12`) | none currently published; `@vela-science/protocol@0.1.0` was published and then removed |
| Lean | Lake, pinned to an immutable Git revision | `lake-manifest.json` | in authority repositories only |
| Binaries | GitHub Releases | SHA-256 checksums, SPDX SBOM, release manifest (signed out of band before publication), build provenance attestation | `scripts/release.sh`, called by `.github/workflows/release.yml`; two targets, `vela-linux-x86_64` and `vela-macos-aarch64` |
| Containers | GHCR | image digest | none |
| Vela-format contracts | none | none | empty set |

The last row is the point. A Vela-format package would be a contract that no
native registry can carry. On current evidence that set is empty, which is why
there is nothing to distribute and no registry to build.

Three notes on what is not in the table. Windows distribution is not restored.
Linux ARM64 is not built. The release manifest now exists, and binds a
different set than this paragraph used to promise: `scripts/release.sh` emits
one `release-manifest.json` per built bundle under
`vela.release-bundle-manifest.v1`, binding the source commit and tree, the Vela
version and tag, the toolchain channel and the exact rustc, the target triple,
the build command, the binary digest, and every asset and SBOM digest. One
manifest per bundle rather than one per release: a cross-target manifest would
have to be assembled in a job holding both targets and signed by whatever key
that job could reach, which is the provider coupling the manifest exists to
remove. It is signed with `ssh-keygen -Y sign` under a distribution identity separate
from the repository-authority key, by an operator rather than in CI for the same
reason — `scripts/sign-published-release.sh` signs the bytes CI published and
then publishes the draft, so the release is immutable and signed from the moment
it is visible. Per-asset attestation is unchanged and stays provider-bound:
`actions/attest-build-provenance` is OIDC-bound to a GitHub identity and has no
neutral equivalent.

## 6. Known gaps

Stated plainly. Each of these is either false in the documentation today or
absent from the code.

### Repair Obligation is a projection output, not a protocol object

**Problem.** Resolved 2026-08-07, and resolved as ADR 0039 intends rather than
by adding an object. Problem has no protocol object and should not have one: it
is derived and owns nothing. What was wrong was where the derivation started.
It was synthesized by joining `graph_nodes` to `claims` — an inner join, so a
question could not exist until this authority had already answered it — and its
prize and tags were regex-parsed out of the Claim's assertion string.

It is now anchored on `observatory.native_records` scoped through
`release_sources`, with the Claim a `LEFT JOIN LATERAL`, and its declared
status, formalization, prize and subject tags read from the Source record's
metadata, where upstream publishes them. The live projection carries 1,217
Problems against zero Claims, which is a shape the previous derivation could
not express at all.

**Repair Obligation.** Less far along than this section used to say, and the
error was worth more than the gap. The rooted projection identity is real:
`crates/vela-cli/src/correction_impact/reducer.rs` declares
`RepairObligation`, and the root is computed over an `ObligationPreimage`
carrying `schema: "vela.correction-repair-obligation.v1"`. The CLI exposes it
inline in the correction-impact projection; no downstream product consumes it
as an action contract.

This section previously claimed a shipped UI surface and named
`DecisionInboxNextObligation` as the consumer. That is a different object
sharing a word. `DecisionInboxNextObligation`
(`crates/vela-cli/src/decision_inbox.rs:150`) is three prose strings, `now`,
`if_accept` and `if_reject`, that the decision inbox writes for a reader; it
reaches `vela-web` as `next_obligation` inside `decision_packet` and is rendered
by `decision-boundary.tsx`, which documents that it renders on nothing today
because every Proposal in the current release is terminal. None of its three
fields appears in `RepairObligation`, and nothing about it is rooted.

What has since changed is both the reach and the immediate reader.
`vela correction impact` shipped in 0.969.0 and runs the derivation over the
accepted Claim index of a real Repository. The CLI consumes each
`RepairObligation` as inline projection data, augments its JSON with
`condition_source`, and renders the obligation count and source. No downstream
product consumes it as an action contract. The Rust tests and clean-room reader
remain `crates/vela-cli/src/correction_impact/reducer_tests.rs`,
`crates/vela-cli/tests/correction_impact.rs` and
`conformance/verify_correction_impact.py`; the first and last run over
synthetic fixtures.

**What it lacks is not a schema.** Publishing
`vela.correction-repair-obligation.v1` into `schemas/` would not be the neutral
act of documenting bytes that already exist. `schemas/` is defined
mechanically as the kernel's generated wire surface: every file is produced by
`wire_schema::published()` in `vela-protocol`, and
`crates/vela-protocol/tests/wire_schemas.rs` asserts the directory holds that
set exactly. `vela-protocol` cannot see the CLI-owned reducer — §8's
dependencies point one way — so a file in `schemas/` first requires moving the
type into the kernel, and the kernel is where a canonical protocol object is
defined. That is the promotion the standing rule withholds, arrived at through
a directory rather than through a decision.

Two further reasons. The preimage never travels: it exists to be hashed, and
the document that does travel is `vela.correction-impact-projection.v1`, which
carries `RepairObligation` inline. No other object publishes a schema for its
preimage. And the shape is already held by something stronger than a schema —
`conformance/fixtures/correction/` pins the literal `obligation_root` that the
Rust and the Python must each reproduce, which catches field names, order,
values and canonicalization where a schema would catch names and types. A third
statement of one shape, held by nothing, is the defect this document names
elsewhere, not a fix for it.

**The remaining gap is authored consequential input, not a caller.**
`vela correction impact` is the shipped caller and runs the existing
derivation over a verified Repository. The current writer authors only
`corrects` or `supersedes`; it cannot author `depends` or `supports`, so a
current producer-built Repository has no consequential edge to traverse and
correctly returns an empty cascade. ADR 0043 freezes a separate, noncanonical
`requires` experiment; it does not fill this protocol gap or demonstrate a
current-Repository cascade.

**Repair-condition provenance is explicit.** The reducer fails closed with
`repair_condition_missing_for_affected_claim` when direct input omits a
discharge condition. The Repository caller supplies either the Claim's
namespaced `vela.correction.repair_condition` or one fixed default and reports
`condition_source` as `declared` or `protocol_default`. The caller therefore
does not present its default as producer-authored. This supplies no dependency
relation and changes no Standing.

### 1,217 open Problems were stored as accepted Claims

Resolved 2026-08-07. The Erdős catalogue had been bulk-imported as Standing: a
catalogue row presented as adjudicated scientific state. This was the largest
single defect in the ecosystem and the reason for the reset.

Reclassifying the corpus was not a safe change to the map but a rewrite of it,
and the warning stood: the Problem product's join key *was* `observatory.claims`
and its prize and tags came out of the Claim's assertion string, so the corpus
could not stop being Claims without the surface losing both in the same commit.
The replacement was built first. The catalogue is now acquired as Source
observations from the pinned upstream registry, the ledger reads it directly,
and the live release publishes 1,217 Problems with no Claim behind any of them.

The catalogue corpus has no Standing, and that is the point rather than a gap:
a Claim in `math` arrives by Decision on evidence, one at a time. Math now holds
one accepted Claim, three Proposals and three Submissions, none of which turns
the 1,217 catalogue entries into Claims. They remain a read projection over
Source observations and nothing in that catalogue was bulk-adjudicated.

**On the numbers, one honest correction.** ADR 0039's own measured table reads:

| Repository | Claims | with any evidence | Submissions | Verification Records |
| --- | ---: | ---: | ---: | ---: |
| erdos | 2785 | 23 | 13 | 17 |
| sidon | 40 | 22 | 0 | 0 |
| formal-conjectures | 18 | 7 | 4 | 5 |
| quantum-codes | 6 | 6 | 1 | 2 |

That is 2,849 Claims and 58 with evidence across four repositories, with 24
Verification Records. A figure of 8,532 accepted Claims with 162 carrying
evidence has been circulating. It is roughly three times the ADR's count and
does not reproduce from this repository; the likely cause is a count unfiltered
by `release_root` against a projection that retains three activated releases
(`packages/observatory-data/scripts/prune-releases.mjs` uses `LIMIT 3`). Use the
ADR's numbers. Re-measure with an explicit `release_root` filter before any of
these figures is published.

The 1,217 figure is a corpus size — the number of problems in the Erdős
catalogue — not a count of misfiled Claims. Both facts are bad; they are
different facts.

### Protocol and tooling gaps

- **The epoch rename is done.** Five Event
  kinds are `claim.*` and `target.claimed`
  (`crates/vela-protocol/src/kernel/events.rs`) and six vocabulary terms are
  documented as retired (`docs/TERMINOLOGY.md`) — the five ADR 0039 §5 retired
  plus Registration Record, which ADR 0033 retired and that table had been
  missing, leaving §7 below naming six against its five. The current runtime,
  schemas, configuration, and CLI use Repository identity, `vela.toml`,
  `--repo`, and `vela.status.v4`; no compatibility reader, writer, alias, or
  translation path remains. Historical ADRs and `docs/ROOTS.md` retain the old
  identifiers only to bind archived bytes and prevent prefix reuse. Tests guard
  the live CLI wording, not the absence of deleted source.

  `crates/vela-protocol/src/epoch1/` and the read-only `vela history <path>`
  are absent because they were withdrawn, not because they are outstanding.
  This list carried them as "additions the ADR asked for" by reading ADR 0039
  §8 without its amendment: the same-day amendment withdraws §8, §9 and §10 and
  records that `epoch1/` "was built, verified against all four checkouts, and
  then deleted". Both are correctly absent; Git history records the deletion.
- ~~One relation alias is inert and cannot be removed here.~~ Resolved:
  `opposes` → `contradicts` is withdrawn rather than aliased. It aliased
  nothing — the fixture recorded `"retained_uses": 0` and said it "was declared
  in PROTOCOL.md and written into no record" — and a near-miss table is for
  spellings a retained record holds. It now has the disposition `revises` and
  `retracts` already had. Removing it required editing the fixture that pins it,
  which nothing outside this repository reads. `depends_on` → `depends` was
  kept at the time because `correction_impact.rs` was said to classify edges by
  the derived-graph rendering ADR 0004 gave it. That was wrong about the code:
  `RULE_FOR_RETAINED_KIND` matches its left column, which is the spelling a
  record retains — `depends` — and emits `depends_on` as the rule kind
  independently of any alias. The alias narrowed nothing, no record in any
  repository holds it, and it is withdrawn too. The near-miss table is gone with
  it.
- ~~One retired term is still wire.~~ Resolved in the final pre-release wire
  cut: `provenance.source_attempt`, its bespoke `vat_` identifier, and the
  `--source-attempt` flag are deleted. `provenance.source_run` is the one
  optional external workbench-run identity, `--source-run` authors it, and the
  duplicate-execution guards compare it. Vela neither mints a run identity nor
  owns the workbench runtime.
- ~~The authority architecture has not been challenged against gittuf.~~
  Resolved by `docs/GITTUF_AUTHORITY_DELETION_SPIKE.md`: gittuf v0.15.0
  protected and independently verified the same fixture's Git ref transitions,
  including rejection of an unauthorized RSL signer, while Vela independently
  completed Submission → Verification → Decision → replay. The combined path
  deleted zero Vela lines, added a second root/policy lifecycle and custom-ref
  fetch, and could not replace any scientific authority check. The selected
  current architecture remains the one closed native evaluator; gittuf stays an
  optional external publication check.
- ~~One repository has to re-genesis before it can be read.~~ Resolved by the
  single Vela 0.972.1 Math genesis and explicit re-admission. The current binary
  strictly replays the UUIDv4 Repository and its one accepted Claim; the
  retained 0.971.0 predecessor remains readable with its pinned binary and does
  not carry Standing forward.
- ~~`serde_yaml_ng` is a `serde_yaml` fork.~~ Resolved: the sole YAML consumer,
  the dev-only GitHub Action contract test, now uses maintained pure-Rust
  `serde-saphyr` 1.0.1. No runtime protocol path parses YAML.
- ~~License fields are not SPDX.~~ Resolved: all three Repository Profile
  license values are parsed as SPDX license expressions by `spdx` 0.13.5.
  Initialization and the documented example use SPDX's `NOASSERTION` value
  where the data license is not known; free-form `"varies"` fails closed.
- ~~Four published schemas are missing.~~ Resolved: `schemas/` holds twelve
  generated documents. Repository Profile, authorization request,
  authorization evaluation, and `vela.error.v1` now come from the live Rust
  types, participate in the drift gate, and have positive and negative
  independent conformance cases.
- ~~Canonicalization vectors run in two languages, not three.~~ Resolved:
  `conformance/canonical-hashing.json` declares Rust, Python, and JavaScript.
  The clean-room JavaScript reader independently checks RFC 8785 UTF-16 key
  ordering, ECMAScript number serialization, canonical UTF-8 bytes, and exact
  SHA-256 roots.
- **The portable TypeScript waist was removed, not deferred.**
  `@vela-science/protocol@0.1.0` was published (ADR 0024) and removed.
  `docs/THEORY.md` claimed
  "The TypeScript package and language-neutral vectors check the portable
  producer boundary" and, in a second place this list never named, "independent
  Python and JavaScript readers". The deletion decision still stands: there is
  no portable TypeScript package. The current conformance surface is two
  readers, two clean-room emitters, and language-neutral vectors.
- ~~No CodeQL, Scorecard, CODEOWNERS, or private disclosure route.~~ Resolved.
  GitHub CodeQL default setup is configured for Actions, JavaScript/TypeScript,
  Python, and Rust; `.github/workflows/scorecard.yml` publishes pinned OpenSSF
  Scorecard SARIF; `.github/CODEOWNERS` routes review to the one current
  maintainer; and `SECURITY.md` uses the enabled private vulnerability-reporting
  surface instead of a personal address. The conformance emitter's
  `cryptography` pin is 50.0.0, above every version implicated by the six
  Dependabot advisories open against 46.0.5. `proptest` and `cargo-fuzz` are not
  adopted as ornamental dependencies: exhaustive shape enumeration, negative
  wire cases, frozen corpora, strict replay fixtures, and independent readers
  already target the concrete parser and canonicalization risks. Add either
  only with a failing property or fuzz corpus it uniquely owns.

### Documentation contradictions to resolve

Line numbers are not cited below: they drifted off their subjects once already
while the contradictions themselves stood. Each item quotes the wording to
search for instead.

- ~~`docs/ARCHITECTURE.md` binds the Atlas to "the four declared" Frontiers in
  one place and to the Observatory in another.~~ Resolved: both read the one
  live mathematics authority, which is what `vela-science/math` is.
- ~~`docs/TERMINOLOGY.md` scopes the Math Atlas to "three maintained
  mathematical Frontiers" and later reports that "All four Frontiers hold every
  indexed Claim", and calls the Atlas current in one section and future in
  another.~~ Resolved: the Math Atlas is scoped to `vela-science/math`, the four
  are named as the repositories ADR 0039 archived, and the analysis-table row is
  "Federated Atlas", which is the future one.
- ~~`docs/TERMINOLOGY.md` derives `superseded` from a `finding.superseded`
  Event, after the same file retires `finding.*`.~~ Resolved: it reads
  "`superseded` follows a `claim.superseded` Event".
- ~~Two taglines ship at once.~~ Resolved: everything now says "version
  control for scientific state", which is what the binary prints, and
  `the_documented_tagline_is_the_one_the_binary_prints` in
  `crates/vela-protocol/tests/cli_release_contract.rs` holds the six documents
  to the binary rather than to each other.
- ~~`docs/ROADMAP.md` says `create -> submit -> verify -> decide -> replay`.~~
  Resolved: it says `init -> …`, which is the verb the CLI has.
- ~~ADR 0017 left Frontier Algebra, Discovery Calculus, and Lens in current
  vocabulary despite deferring their implementation.~~ Resolved by ADR 0044:
  those names are historical research labels, not current Vela layers,
  objects, fields, or reserved extension points. `Frontier Calculus` remains
  only the constrained research-program label the canonical framing assigns
  it. Concrete shipped mechanisms retain concrete names such as `correction
  impact`; no replacement layer is introduced. The downstream
  `frontier_algebra_atom` field is a Web-local legacy read-model concern and is
  not changed here or rewritten in stored projection bytes.

### Change-control boundary

Resolved. The reproduced-gap rule forbids a new subsystem without evidence; it
does not forbid deleting or consolidating a misleading pre-1.0 surface. ADR
0039 also rested on observed defects: the same scientific territory was split
along incompatible axes, four repositories represented one authority, and
catalogue rows were presented as accepted Standing. It made one explicit
pre-1.0 epoch cut, preserved predecessor history, and added no new authority
path or subsystem. No amendment exception is required.

## 7. What is novel, and what is borrowed

### Borrowed. Name it after the thing it already is

| Vela term | What it is |
| --- | --- |
| Repository | a Git repository under a named authority |
| Problem | a bounded issue or milestone |
| Obligation | an explicit open requirement |
| Submission | a signed patch |
| Proposal | a pull request |
| Verification Record | a check run |
| Decision | the merge ruling |
| Event | a commit to an append-only authority log |
| Standing | what an event-sourced reducer derives |
| Atlas | a read projection |
| Source | an upstream observed at a pinned revision |

Nothing structural is invented here by design. Native systems keep sovereignty:
Lean is the prover, Cargo/uv/Lake resolve dependencies, Git carries ancestry and
distribution, GitHub carries releases and attestation.

### Novel. Worth going all in on

Four terms, and three shipped mechanisms.

**Claim.** A scientific assertion whose Standing is repository-local and only
ever moves on an attributed, authorized Decision.

**Evidence as a typed role.** Not a file attached to a record; a role a rooted
object plays with respect to a Claim, which can be lost when the object it
depends on is corrected.

**Obligation.** What is missing before a Claim can stand, transfer or be
repaired. Rooted at `vela.correction-repair-obligation.v1`; still not a
first-class object (§6).

**The four axes that never collapse.** Claim standing, Verification outcome,
Proposal status, repository integrity. `vela-web`'s state glyph separates two of
them visually today (ring = standing, core = verification). Source-specific
acquisition remains outside Core.

The three mechanisms that are built and are the defensible part:

1. **Verification is not acceptance.** A Verification Record states a property,
   its inputs, method, environment, outcome and an explicit
   `does_not_establish` list (`crates/vela-protocol/src/objects/verification_record.rs:55-60`,
   with at least one limitation required at `:242-250`). It changes no Standing.
   Only an attributed, authorized Decision does.
2. **Correction deterministically partitions declared support edges.**
   `crates/vela-cli/src/correction_impact/reducer.rs` partitions
   `lost_support_routes` from `surviving_support_routes` and emits repair
   obligations. Those field names describe the reducer's treatment of declared
   edges; they do not establish route grouping, sufficiency, shared-premise
   separation or scientific independence. Fixtures at
   `conformance/fixtures/correction/` run on every CI run through
   `conformance/verify.py`. `vela correction impact` reaches the reducer over
   the accepted claim index of a real repository, and shipped in 0.969.0.

   Driving one correction end to end to get there found two things worth
   stating plainly. First, a defect: accepting a Claim that corrects an
   accepted Claim retires the predecessor, and until the loader learned that,
   the repository stopped loading — `status`, `claims`, `replay`, `why` and
   `review list` all failed on a repository that had done nothing but accept a
   correction. `crates/vela-cli/tests/correction_impact.rs` holds that shut.
   Second, a gap that is not fixed: the projection traverses `depends` and
   `supports` claim-to-claim edges, and **the write path authors neither**.
   Every such edge in the corpus was written by the epoch-1 ingest. A
   repository built with today's CLI therefore has no edge to traverse and
   correctly reports an empty cascade. ADR 0043 freezes a noncanonical,
   source-owned `requires` profile and matched baseline before considering a
   signed field. The synthetic experiment is not a producer-authored
   Repository cascade and moves no Standing.
3. **Projections cannot silently go stale.** Every projection row is
   root-bound. The disclosure contract is partial: `projector_version`, lens
   identity and truncation rules are not built.

### Invented and unbuilt. Delete rather than re-base

Executable Frontier Model, Frontier Algebra, Discovery Calculus, Verified
Frontier Learning, FrontierBench, possible worlds,
distinction partitions, capabilities, the Frontier Inheritance Effect,
long-horizon transition credit, Constellation, Lens, Translation
Studio, Atlas-as-application, release modes, risk tiers, sealed commitments.

None of these has an implementation. The Frontier-named entries no longer have
an authority identity, and the rest never earned a maintained product surface;
re-basing any of them would preserve an unproved tower.

`Frontier Calculus` remains usable only as a research-program label for
formalizing support, provenance, correction, transfer and obligations. It is
not a Vela layer, kernel dependency, product surface or implementation
commitment.

"Capsules" was on this list and does not belong: the verifier capsule is ADR
0013's, it is built, and `vela submit --verifier-capsule-root` binds one
(`crates/vela-protocol/src/objects/execution_binding.rs:22`). `docs/PROTOCOL.md`
and `docs/VERIFICATION.md` describe it as current, correctly. It also failed the
test the paragraph above states, since it is not named after `Frontier`.

Already retired and not to be reintroduced: Finding, Frontier Commit, Review
Packet, Frontier map, Attempt (ADR 0039 §5), and Registration Record (ADR
0033).

## 8. Layering, and what must never depend on what

```text
  kernel        crates/vela-protocol
                  ↑ canonical objects, roots, Events, replay, Standing
  authorization crates/vela-authority
                  ↑ restricted authorization and repository service signing
  durability    crates/vela-repository
                  ↑ policy-neutral transactions and recovery; no Decision
                    or Standing semantics
  operator      crates/vela-cli
                  ↑ 16 verbs: replay status projection claims log verification
                    correction integration recover authority init review show
                    why submit completions
  readers       conformance/readers/python, conformance/readers/javascript,
                conformance/emitters/javascript.mjs, conformance/emitters/python.py
                  ↑ independent implementations of the same bytes
  projection    vela-web/packages/observatory-data  (20 tables; the 17 that hold
                    projected rows are root-bound)
  surfaces      vela-web/apps/observatory, vela-web/apps/www
```

The CLI also owns its sole-consumer derived correction reducer and small process
adapters. The compile-time graph is not one vertical product stack.
`vela-authority` and `vela-repository` each depend on `vela-protocol`, and
`vela-cli` composes those current boundaries. The clean-room
readers implement the public bytes independently, and Web consumes committed
roots. No reverse dependency is authorized. Concretely:

- **No non-kernel implementation defines Standing.** Not the CLI, not a reader,
  not the correction reducer, not the projection, not a surface, not a package,
  not a Source, not an agent, not a benchmark, not an authorization Allow. Only
  protocol admission of an attributed human or agent Decision records an Event from
  which replay derives Standing.
- **`vela` must not depend on `vela-web`.** One documented leak:
  `crates/vela-cli/tests/wording_contract.rs:11` records that `vela-web` pins a
  literal, which is knowledge of a downstream consumer inside the protocol
  repository. Direction of the pin is correct (`vela-web` pins
  `vela-science/vela@26e7afa2`, `v0.972.1`); the test comment is a soft reverse
  coupling and should be stated as a note, not enforced as a contract.
- **The projection must never hold a repository-authority credential** and must
  never be the source of a fact the repository does not already contain.
- **A surface may show the command an authorized operator would run. It may
  never run it.** The Observatory's read-only boundary is enforced by
  `vela-web/scripts/read-only-boundary.mjs`. It permits exactly one
  mutating Server Action, `signOutAccount`, pinned to a single line of its own
  body, plus three identity routes. Scientific state has no write path from a
  browser; identity does.
- **A Source may be referenced or snapshotted freely and admitted only through a
  Decision.** Enforced in SQL, §1.
- **CLI-owned derived analysis is optional.** Removing the correction reducer
  or process adapters must not affect replay of any repository.
- **Canonical replay must not depend on package, analysis, independent-reader,
  Web, Source, agent, benchmark or projection state.** The CLI invokes kernel
  validation over canonical Repository bytes; the durability runtime supplies
  write and recovery mechanics, not alternate scientific semantics.

The former violation is closed. The route under
`apps/observatory/src/app/repositories/` keeps `math` only as a route and
presentation handle; `packages/observatory-data/src/registry.ts` maps it to the
Repository UUID. Canonical rows and joins use `repository_id`, while
`repository_slugs` fields describe declared route coverage and confer no
protocol identity or authority.
