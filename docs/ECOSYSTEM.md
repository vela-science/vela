# Vela ecosystem architecture

- Status: current as of 2026-08-07
- Binding decision: ADR 0039, `docs/adr/0039-repository-authority-and-derived-frontiers.md`
- Supersedes: the ecosystem sections of `docs/ARCHITECTURE.md` §"Source and
  repository ownership" and §"Rust ecosystem comparison", which still describe
  the four-Frontier topology and a package layer this document closes

This document states the ecosystem structure once. Where a layer exists, the
path is given. Where a layer does not exist, it is named as future with the
gate that would open it. No layer is named that names nothing.

## 1. The four boundaries

ADR 0039 split one overloaded word into four boundaries and one projection.

| Boundary | What it bounds | Identifier |
| --- | --- | --- |
| Repository | authority: Git repository, trust root, canonical history, Standing | `repository_id`, `^vrepo_[0-9a-f]{32}$` |
| Source | provenance: exact observations of external systems, never governed | `source_id`, prefixed `source:` |
| Problem | one bounded scientific question | native source identifier |
| Frontier | derived: the unresolved state around one or more Problems; owns nothing | none, by design |
| Atlas | projection across the four above | none; a read surface |

The rule underneath all of it: **a repository exists because there is a new
authority, never because there is a new topic.**

### Where each boundary is in code

**Repository.** `crates/vela-protocol/src/objects/repository.rs`
(`RepositoryV4`), the authority history in `crates/vela-authority/`,
replay in `crates/vela-verify/`. The rename landed in v0.967.0: `vfr_` and
`frontier_id` are at zero in `crates/`, against 76 `vrepo_` and 392
`repository_id`. The type parses `vela.toml`, mints `vrepo_<32 hex>`, and
answers `vela.status.v4`.

**Source.** Declared per repository in `sources.yaml` and locked in
`sources.lock.json`; the shared lock tooling is
`packages/vela-source-manifest/` (a Python package with `pyproject.toml` and
`uv.lock`). Projected by `vela-web` into `observatory.source_declarations`,
`observatory.source_observations`, `observatory.native_records` and
`observatory.release_sources` (`packages/frontier-data/schema.sql`). Thirteen
sources are declared in
`packages/frontier-data/config/math-sources.v1.ts`, which is the same thirteen
§2 says `math` declares. Adapters live in
`packages/frontier-data/src/source-adapters/` (fifteen files, including
`formal-conjectures.ts`, `oeis.ts`, `physlib.ts`, `openai-ten-proofs.ts` and
`vibemathed.ts`).

The containment property is enforced in SQL, not prose.
`observatory.frontier_source_bindings` carries
`CHECK (binding_kind = 'admission' OR local_standing_effect = 'none')`. A
reference or snapshot of an external source cannot carry Standing.

**Problem.** No protocol object. No projection table. The surface at
`apps/observatory/src/app/frontiers/[slug]/problems/` is derived, in
`packages/frontier-data/src/index.ts`, from `observatory.native_records` scoped
through `release_sources`, with the Claim a `LEFT JOIN LATERAL` and the declared
status, formalization, prize and subject tags read from the Source record's
metadata. It used to be an inner join from `observatory.graph_nodes` to
`observatory.claims` with prize and tags regex-extracted from the Claim's
assertion string, which is the defect ADR 0039 named; §6 records the repair.
This paragraph described the defect in the present tense for as long as §6
described its replacement, and this paragraph is the one a reader reaches first.

**Frontier.** Derived, and no longer minted in the protocol. In `vela-web`,
`registry.ts` pins one slug, `math`, against `repository_id`
`vrepo_8b32ff6fa11cdb5fa0bb8a043c7d6941` and validates it as `^vrepo_[0-9a-f]{32}$`; no `vfr_`
identity survives there. The keying is finished too: all thirteen projection
tables key on `repository_id`, and the slug is a presentation fact that lives
only in the registry, where a URL handle meets a protocol identity. It was a
root migration rather than a rename — `rooted()` hashes `canonicalJson(row)`
including keys, so every renamed column moved a `row_root` — and the release it
produced was rebuilt from an empty database and compared root for root.

**Atlas.** The Observatory, `apps/observatory/` in `vela-web`. There is no
separate Atlas application, no Atlas compiler, no per-view ontology. Every
projection row is root-bound. `packages/frontier-data/schema.sql` declares 20
tables, of which 17 carry `row_root`; the other three — `releases`,
`schema_migrations` and `current_release` — are the projection's own bookkeeping
and hold no projected row. Of the 17, all but three also carry `release_root`,
and those three are `source_declarations`, `source_observations` and
`native_records`, which are content-addressed and reach a release through
`release_sources`. The count of tables and the count of root-bound rows were
stated as one number here, which made the bookkeeping tables look like state.

## 2. Is there a math hub?

**No hub. There is one repository.**

`vela-science/math` is a Repository under one authority. It is not a domain
library, not a package namespace, not an index of other people's mathematics,
and not a second Mathlib. Nothing in it is a hub for anything outside it.

It exists as of 2026-08-09: `vrepo_8b32ff6fa11cdb5fa0bb8a043c7d6941`, origin
`vro_3cfb63bdb525a407`, genesis generation 1, signed, and it replays from a
clean clone. It declares thirteen Sources and holds one accepted Claim, which
entered the only way a Claim can — through a Decision.

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

It is the projection of Source declarations. Thirteen sources in
`packages/frontier-data/config/math-sources.v1.ts`, adapters in
`packages/frontier-data/src/source-adapters/`, tables
`observatory.source_declarations` / `source_observations` / `native_records` /
`release_sources` / `frontier_source_bindings`, read surface under
`apps/observatory/src/app/sources/`.

**New nouns added: zero.** Source is already one of the four boundaries. The
registry is a view of it, and a view is not a noun.

It observes. It never governs. Each Frontier owns its own `sources.yaml` and,
when the registry and the repository disagree, the repository is right
(`packages/frontier-data/src/source-declarations.ts`).

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
re-run as designed. It should be closed as answered in the negative rather than
carried as open backlog.

Consequently these lines in `docs/ARCHITECTURE.md` are dead and should be
deleted rather than reworded:

- `:279-281` reusable package sources move to a `vela-science/vela-packages`
  repository;
- `:282-283` package inspection, validation and acquisition become `vela`
  subcommands (this directly contradicts "no custom package manager");
- `:284-285` the Vela Web deployment serves a sparse package index.

The five-level ladder in the same file survives only as a description of what
would have to become true before the question could be reopened. Levels 3
(hosted registry) and 4 (federated registry) are **not destinations**.

## 4. Repository topology

### Live

| Repository | Sole responsibility | Status |
| --- | --- | --- |
| `vela-science/vela` | Protocol, CLI, conformance readers and fixtures, releases, architecture | exists |
| `vela-science/vela-web` | Editorial site and read-only Observatory | exists |
| `vela-science/.github` | Organization profile, reusable workflows, security policy | exists |
| `vela-science/math` | The one live mathematics authority, fresh genesis | exists |

`vela-science/math` exists and holds a signed genesis. The epoch rename this
paragraph named as its blocker is done, so the genesis was written by a binary
that mints `vrepo_` and writes `vela.toml`, which is what the repository
carries. What its authority record still holds from before the rename is the
Cedar entity `Frontier`, the `frontier_administrator` role and the StateTarget
type `frontier`, all inside a valid signature; retiring those spellings is a
policy-bundle rotation on a live authority rather than a wording change.

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
| Python tools | PyPI | `uv.lock`, `uv run --locked` | `packages/vela-source-manifest/` (pyproject.toml + uv.lock) |
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

### Obligation is vocabulary without an object

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

**Obligation.** Less far along than this section used to say, and the error was
worth more than the gap. The rooted wire identity is real:
`crates/vela-edge/src/analysis/correction_impact.rs:122` declares
`RepairObligation`, and the root is computed over an `ObligationPreimage`
carrying `schema: "vela.correction-repair-obligation.v1"`. Nothing consumes it.

This section previously claimed a shipped UI surface and named
`DecisionInboxNextObligation` as the consumer. That is a different object
sharing a word. `DecisionInboxNextObligation`
(`crates/vela-cli/src/decision_inbox.rs:150`) is three prose strings, `now`,
`if_accept` and `if_reject`, that the decision inbox writes for a reader; it
reaches `vela-web` as `next_obligation` inside `decision_packet` and is rendered
by `decision-boundary.tsx`, which documents that it renders on nothing today
because every Proposal in the current release is terminal. None of its three
fields appears in `RepairObligation`, and nothing about it is rooted.

What has since changed is the reach, not the consumer. `vela correction impact`
shipped in 0.969.0 and runs the derivation over the accepted claim index of a
real repository, so the sentence this section used to carry — that no CLI verb
reaches it — is no longer true; §7 states the current position. The rooted
Obligation still has no consumer. Its readers are
`crates/vela-edge/tests/correction_impact.rs`,
`crates/vela-cli/tests/correction_impact.rs` and
`conformance/verify_correction_impact.py`, and the first and last of those run
over synthetic fixtures.

**What it lacks is not a schema.** Publishing
`vela.correction-repair-obligation.v1` into `schemas/` would not be the neutral
act of documenting bytes that already exist. `schemas/` is defined
mechanically as the kernel's generated wire surface: every file is produced by
`wire_schema::published()` in `vela-protocol`, and
`crates/vela-protocol/tests/wire_schemas.rs` asserts the directory holds that
set exactly. `vela-protocol` cannot see `vela-edge` — §8's dependencies point
one way — so a file in `schemas/` first requires moving the type into the
kernel, and the kernel is where a canonical protocol object is defined. That is
the promotion the standing rule withholds, arrived at through a directory
rather than through a decision.

Two further reasons. The preimage never travels: it exists to be hashed, and
the document that does travel is `vela.correction-impact-projection.v1`, which
carries `RepairObligation` inline. No other object publishes a schema for its
preimage. And the shape is already held by something stronger than a schema —
`conformance/fixtures/correction/` pins the literal `obligation_root` that the
Rust and the Python must each reproduce, which catches field names, order,
values and canonicalization where a schema would catch names and types. A third
statement of one shape, held by nothing, is the defect this document names
elsewhere, not a fix for it.

**The gap is a caller.** Correction impact is the highest-value mechanism
already built and still unreachable (§7). It needs a verb that feeds it a real
repository, not a schema, a projection table and a route for output no run has
ever produced.

**Obligation cannot say "unattributable".** `discharge_condition` is a required
`String` taken from the affected Claim's `repair_condition`, and
`correction_impact.rs:345` fails the entire projection with
`repair_condition_missing_for_affected_claim` when it is absent. A Claim that is
genuinely affected but for which no one can state what would repair it is
therefore not recorded as unattributable; it stops the projection being produced
at all. A protocol that cannot name an unattributable failure will overclaim
attribution for every failure it does report. Adding the state is a root change
— the preimage is hashed including keys, so a new field moves every
`obligation_root`, including the one frozen in
`conformance/fixtures/correction/diamond-expected.json` — and it belongs with
the caller, in one deliberate change, rather than ahead of it.

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

What the corpus does not yet have is Standing, and that is the point rather than
a gap: a Claim in `math` arrives by Decision on evidence, one at a time. The
manifest bears that out from the other side of the boundary — `math` holds zero
accepted Claims, zero Proposals and zero Submissions — so the 1,217 are a read
projection over Source observations and nothing there has been adjudicated.

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
(`packages/frontier-data/scripts/prune-releases.mjs` uses `LIMIT 3`). Use the
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
  missing, leaving §7 below naming six against its five. `frontier_id` → `repository_id`,
  `vfr_` → `vrepo_`, `frontier.toml` → `vela.toml`, `--frontier` → `--repo` and
  `vela.status.v3` → `v4` all landed: each is at zero across `crates/`,
  `schemas/`, `packages/` and the top level of `docs/`, and the occurrences that
  remain are deliberate references to the retired spelling — the retired-path
  predicate, `docs/ROOTS.md` stating that a prefix is never reused, and the
  tests that hold the new wording in place. This list previously called all five
  "not done" while §1 of this same document said the rename landed in v0.967.0,
  and it stated two of them as `vela.toml` → `vela.toml` and `--repo` → `--repo`
  after a sweep rewrote both sides of the arrow.

  `crates/vela-protocol/src/epoch1/` and the read-only `vela history <path>`
  are absent because they were withdrawn, not because they are outstanding.
  This list carried them as "additions the ADR asked for" by reading ADR 0039
  §8 without its amendment: the same-day amendment withdraws §8, §9 and §10 and
  records that `epoch1/` "was built, verified against all four checkouts, and
  then deleted". Both are correctly absent and must stay absent, which is what
  `scripts/ecosystem-status.py` declares.
- ~~One relation alias is inert and cannot be removed here.~~ Resolved:
  `opposes` → `contradicts` is withdrawn rather than aliased. It aliased
  nothing — the fixture recorded `"retained_uses": 0` and said it "was declared
  in PROTOCOL.md and written into no record" — and a near-miss table is for
  spellings a retained record holds. It now has the disposition `revises` and
  `retracts` already had. Removing it required editing the fixture that pins it,
  which nothing outside this repository reads. `depends_on` → `depends` stays:
  it reads the same way in the fixture but is live, because
  `correction_impact.rs` classifies edges by the derived-graph rendering ADR
  0004 gave it.
- **One retired term is still wire.** ADR 0039 §5 retired `Attempt`, and
  `provenance.source_attempt` with the `vat_` prefix was added afterwards and is
  published in `schemas/submission.schema.json`. The product surface says
  "workbench run", which is what `docs/TERMINOLOGY.md` prescribes, but the field
  and the prefix cannot follow without a schema version, so the retired spelling
  is load-bearing on the wire. This is the same shape as
  `integrity.replay: "verified"`: a token a prose sweep must not take.
- **DSSE is not the common waist.** Authority records use DSSE; Submission,
  Verification Record and Proposal Withdrawal still sign a bespoke zeroed-field
  preimage (`crates/vela-protocol/src/objects/submission_v1.rs`). ADR 0035
  remains Proposed. This blocks any external producer and any 1.0 freeze. The
  `vela-science/math` genesis was the cheapest moment to cut v2 payloads and it
  has been written, so that window is closed: math now has one signed authority
  record and a producer history to migrate rather than none.
- **Cedar is not removed.** The closed evaluator exists
  (`evaluate_authorization_v1` in `crates/vela-authority/src/lib.rs`) and the
  parity corpus exists
  (`conformance/fixtures/epoch1/authorization-profile-parity.json`), but the
  evaluator is called only from tests and `cedar-policy = "=4.11.2"` is still a
  workspace dependency of the active writer.
- **`serde_yaml_ng` is a `serde_yaml` fork.** `Cargo.toml:43`. The standards
  audit named forks as the option to avoid, and a fork is what was adopted. It
  is dev-only today (`crates/vela-protocol/tests/action_contracts.rs`), which
  bounds the exposure but does not resolve the decision.
- **License fields are not SPDX.** `repository.rs:105-119` validates
  the three license fields as bounded NFC text only. The documented example in
  `docs/REPOSITORY_PROFILE.md:52` is `data = "varies"`, which is not an
  SPDX expression.
- **Four published schemas are missing.** `schemas/` holds eight: Submission,
  Verification Record, Proposal Withdrawal, Claim Record, Proposal, repository
  origin, the DSSE authority envelope and `vela.status.v4`. Claim Record was on
  this list as unpublished after it had been published. Still unpublished: the
  authority request and decision payloads, the repository profile, and a
  `vela.error.v1` CLI error envelope. The generator already exists
  (`crates/vela-protocol/src/wire_schema.rs` with a blessing test), so this is
  scope, not mechanism.
- **Canonicalization vectors run in two languages, not three.**
  `conformance/canonical-hashing.json` declares exactly two conforming
  implementations (Rust and Python); `conformance/readers/` contains only
  `python`. The two emitters, `conformance/emitters/javascript.mjs` and
  `conformance/emitters/python.py`, emit submission and verification objects
  and neither reads the vector corpus.
- **The portable TypeScript waist was removed, not deferred.**
  `@vela-science/protocol@0.1.0` was published (ADR 0024) and
  `packages/` now holds only `vela-source-manifest`. `docs/THEORY.md` claimed
  "The TypeScript package and language-neutral vectors check the portable
  producer boundary" and, in a second place this list never named, "independent
  Python and JavaScript readers". Both now say what is there: one Python reader,
  two clean-room emitters, and vectors that run in Rust and Python.
- **No proptest, no cargo-fuzz, no CodeQL, no Scorecard, no CODEOWNERS.**
  `.github/workflows/` holds `conformance.yml`, `release.yml` and
  `ecosystem-status.yml` only.
  `SECURITY.md:5` routes disclosure to a personal Gmail address.

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
- ADR 0017 is "Deferred — research only" and forbids implementing a Frontier
  calculus. Its two layer names, Frontier Algebra and Discovery Calculus, are
  live rows in the analysis table of `docs/TERMINOLOGY.md`, and
  `frontier_algebra_atom` is a permanently-null field in a `vela-web` projection
  fixture (`packages/frontier-data/tests/support/semantic-correction.ts`).
  Supersede 0017 or rename its layers; a reserved field that is never filled is
  worse than an absent one. `Lens` is a third row of the same table and a third
  entry of §7's delete list, and belongs to the same ruling: the rows are not
  removed here because which way they go is 0017's to decide, not a sweep's.

### One open governance question

ADR 0039 changes the identifier, five Event kinds, a status schema version, and
quarantines the object types. Under the constitutional-core rule those changes
require a reproduced gap; the justification offered is an ontology argument plus
a corpus census. Either the amendment rule is restated to admit ontology
corrections, or ADR 0039 should say why it does not bind a pre-1.0 epoch change.
Leaving both standing means the change-control rule is the first thing the reset
broke. This document does not resolve it.

## 7. What is novel, and what is borrowed

### Borrowed. Name it after the thing it already is

| Vela term | What it is |
| --- | --- |
| Repository | a Git repository under a named authority |
| Target | an issue |
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
ever moves on an attributed human Decision.

**Evidence as a typed role.** Not a file attached to a record; a role a rooted
object plays with respect to a Claim, which can be lost when the object it
depends on is corrected.

**Obligation.** What is missing before a Claim can stand, transfer or be
repaired. Rooted at `vela.correction-repair-obligation.v1`; still not a
first-class object (§6).

**The four axes that never collapse.** Claim standing, Verification outcome,
Proposal status, repository integrity. `vela-web`'s state glyph separates two of
them visually today (ring = standing, core = verification). The path this
sentence used to cite resolved inside *this* repository, where it has never
existed; `packages/` here holds `vela-source-manifest` alone.

The three mechanisms that are built and are the defensible part:

1. **Verification is not acceptance.** A Verification Record states a property,
   its inputs, method, environment, outcome and an explicit
   `does_not_establish` list (`crates/vela-protocol/src/objects/verification_record.rs:55-60`,
   with at least one limitation required at `:242-250`). It changes no Standing.
   Only a human Decision does.
2. **Correction preserves independent support routes.**
   `crates/vela-edge/src/analysis/correction_impact.rs` partitions
   `lost_support_routes` from `surviving_support_routes` and emits repair
   obligations. Fixtures at `conformance/fixtures/correction/` run on every CI
   run through `conformance/verify.py`. `vela correction impact` reaches it over
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
   correctly reports an empty cascade. Closing that means giving the signed
   Submission schema a place to declare dependencies, which is a protocol
   change with an ADR, not a CLI change.
3. **Projections cannot silently go stale.** Every projection row is
   root-bound. The disclosure contract is partial: `projector_version`, lens
   identity and truncation rules are not built.

### Invented and unbuilt. Delete rather than re-base

Executable Frontier Model, Frontier Algebra, Discovery Calculus, Frontier
Calculus, Verified Frontier Learning, FrontierBench, possible worlds,
distinction partitions, capabilities, the Frontier Inheritance Effect,
long-horizon transition credit, Constellation, Lens, Translation
Studio, Atlas-as-application, release modes, risk tiers, sealed commitments.

None of these has an implementation. All of them are named after `Frontier`,
which no longer has an identity, so re-basing the names on the derived noun
would preserve a tower whose foundation was removed.

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
  kernel        crates/vela-protocol, vela-authority, vela-verify
                  ↑ objects, roots, signatures, authority, replay, Standing
  operator      crates/vela-cli
                  ↑ 16 verbs: replay status claims log verification reproduce
                    correction authority init review show why next start submit
                    completions
  readers       conformance/readers/python, conformance/emitters/javascript.mjs,
                conformance/emitters/python.py
                  ↑ independent implementations of the same bytes
  analysis      crates/vela-edge
                  ↑ correction impact, target index; read-only, never required
                    for replay
  projection    vela-web/packages/frontier-data  (20 tables; the 17 that hold
                    projected rows are root-bound)
  surfaces      vela-web/apps/observatory, vela-web/apps/www
```

Dependencies point up the list only. Concretely:

- **Nothing above the kernel may change Standing.** Not the CLI, not a reader,
  not `vela-edge`, not the projection, not a surface, not a package, not a
  Source, not an agent, not a benchmark, not an authorization Allow. Only an
  attributed human Decision, recorded as an Event in the repository's own
  history.
- **`vela` must not depend on `vela-web`.** One documented leak:
  `crates/vela-cli/tests/wording_contract.rs:11` records that `vela-web` pins a
  literal, which is knowledge of a downstream consumer inside the protocol
  repository. Direction of the pin is correct (`vela-web` pins
  `vela-science/vela@b202e3bc`, `v0.968.1`); the test comment is a soft reverse
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
- **`vela-edge` is optional.** Deleting it must not affect replay of any
  repository.
- **No layer may be required to replay.** If a repository cannot be replayed
  from a clean clone with the kernel and the CLI alone, the layering has been
  violated.

One surface currently violates this. The Frontier Directory at
`apps/observatory/src/app/frontiers/` routes by topic slug, and twelve
projection tables carry `frontier_slug` in their primary key. Under ADR 0039 a
Frontier is derived and
has no identifier, and topic is the one thing a Repository must never encode.
That surface needs re-deriving against Problem, not renaming.
