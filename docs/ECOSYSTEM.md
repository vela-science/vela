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
| Repository | authority: Git repository, trust root, canonical history, Standing | `repository_id`, `^vrepo_[0-9a-f]{16}$` |
| Source | provenance: exact observations of external systems, never governed | `source_id`, prefixed `source:` |
| Problem | one bounded scientific question | native source identifier |
| Frontier | derived: the unresolved state around one or more Problems; owns nothing | none, by design |
| Atlas | projection across the four above | none; a read surface |

The rule underneath all of it: **a repository exists because there is a new
authority, never because there is a new topic.**

### Where each boundary is in code

**Repository.** `crates/vela-protocol/src/objects/current_repository.rs`
(`CurrentRepositoryV4`), the authority history in `crates/vela-authority/`,
replay in `crates/vela-verify/`. The rename landed in v0.967.0: `vfr_` and
`frontier_id` are at zero in `crates/`, against 76 `vrepo_` and 392
`repository_id`. The type parses `vela.toml`, mints `vrepo_<16 hex>`, and
answers `vela.status.v4`.

**Source.** Declared per repository in `sources.yaml` and locked in
`sources.lock.json`; the shared lock tooling is
`packages/vela-source-manifest/` (a Python package with `pyproject.toml` and
`uv.lock`). Projected by `vela-web` into `observatory.source_declarations`,
`observatory.source_observations`, `observatory.native_records` and
`observatory.release_sources` (`packages/frontier-data/schema.sql`). Thirteen
sources are declared in
`packages/frontier-data/config/math-sources.v1.ts`. Adapters live in
`packages/frontier-data/src/source-adapters/` (twelve files, including
`formal-conjectures.ts`, `oeis.ts`, `physlib.ts`, `openai-ten-proofs.ts`).

The containment property is enforced in SQL, not prose.
`observatory.frontier_source_bindings` carries
`CHECK (binding_kind = 'admission' OR local_standing_effect = 'none')`. A
reference or snapshot of an external source cannot carry Standing.

**Problem.** No protocol object. No projection table. The surface at
`apps/observatory/src/app/frontiers/[slug]/problems/` is synthesized by joining
`observatory.graph_nodes` to `observatory.claims` on
`f.claim_id = n.content ->> 'claim_id'`, with prize and tags regex-extracted
from the Claim's own assertion string
(`packages/frontier-data/src/index.ts:1860-1897`). This is the defect ADR 0039
is correcting, and it is load-bearing: see §6.

**Frontier.** Derived, and no longer minted in the protocol. It survives as a
minted identity only in `vela-web`, whose `registry.ts` still pins four slugs
and four `vfr_` ids and whose eleven projection tables are still keyed
`(release_root, frontier_slug, …)`. That is the remaining epoch work, and it is
a root migration rather than a rename: `rooted()` hashes `canonicalJson(row)`
including keys, so every renamed column moves a `row_root`.

**Atlas.** The Observatory, `apps/observatory/` in `vela-web`. There is no
separate Atlas application, no Atlas compiler, no per-view ontology. Every
projection row is root-bound; 21 tables in `packages/frontier-data/schema.sql`
carry `row_root`, and all but three also carry `release_root`. The three
exceptions are `source_declarations`, `source_observations` and
`native_records`, which are content-addressed and reach a release through
`release_sources`.

## 2. Is there a math hub?

**No hub. There is one repository.**

`vela-science/math` is a Repository under one authority. It is not a domain
library, not a package namespace, not an index of other people's mathematics,
and not a second Mathlib. Nothing in it is a hub for anything outside it.

It exists as of 2026-08-07: `vrepo_56d3fdfcd34ff5c3`, origin
`vro_2e75a5b77102842f`, genesis generation 1, signed, and it replays from a
clean clone. It declares twelve Sources and holds zero Claims, which is the
intended starting state — the corpus returns as observations, and a Claim
enters only through a Decision.

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

This closes a documented conflict rather than dodging it.
`docs/ROADMAP.md:51`, `docs/CAMPAIGN.md:44` and `docs/adr/0038:119` each list
"a `vela-math` repository" under what will not be built. ADR 0039's supersession
clause names only 0038's topology, not its prohibition, so the conflict is
ADR-to-ADR and is currently unresolved in writing. The resolution is the
distinction above: those documents were rejecting a Vela-owned mathematics
*library*, which is still rejected. A single mathematics *authority* is a
different object. ADR 0039 should carry a supersession clause saying so;
until it does, `docs/ROADMAP.md`, `docs/CAMPAIGN.md` and ADR 0038 read as
prohibiting what ADR 0039 creates.

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
`research/lean-replay-contract-evidence/qualification.v1.json` records the
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

Preserved exactly as historical Vela repositories. No further architecture work
inside them beyond integrity fixes. `formal-conjectures-frontier` is
additionally dissolved: all 18 of its Claims are Erdős problems and both its
declared sources are already declared by `erdos-frontier`.

The frozen repositories are not dead weight. They are genuinely distinct trust
roots with real signed history, so re-admitting state from them into
`vela-science/math` through Submission → Verification → Decision *is* the
cross-repository transfer experiment. The migration and the experiment are one
activity.

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
| Lean | Lake, pinned to an immutable Git revision | `lake-manifest.json` | in Frontier repositories only |
| Binaries | GitHub Releases | SHA-256 checksums, SPDX SBOM, build provenance attestation | `.github/workflows/release.yml`; two targets, `vela-linux-x86_64` and `vela-macos-aarch64` |
| Containers | GHCR | image digest | none |
| Vela-format contracts | none | none | empty set |

The last row is the point. A Vela-format package would be a contract that no
native registry can carry. On current evidence that set is empty, which is why
there is nothing to distribute and no registry to build.

Three notes on what is not in the table. Windows distribution is not restored.
Linux ARM64 is not built. A single attested `release-manifest.json` binding the
source commit, tree, Vela version, Rust version, target triples and every asset
SHA-256 does not exist; per-asset attestation already does
(`actions/attest-build-provenance` is pinned at `release.yml:106`), so this is a
small addition rather than a new capability.

## 6. Known gaps

Stated plainly. Each of these is either false in the documentation today or
absent from the code.

### Problem and Obligation are vocabulary without objects

**Problem.** No protocol object, no projection table. It is synthesized in SQL
by joining `graph_nodes` to `claims` and regex-parsing the Claim's assertion
text for prize and tags. ADR 0039 promotes Problem to a boundary; nothing
implements it. There is no ADR that decides what a Problem object is.

**Obligation.** Further along than it looks, and still not a first-class object.
It has a rooted wire identity:
`crates/vela-edge/src/analysis/correction_impact.rs:107` declares
`RepairObligation`, and the root is computed over an `ObligationPreimage`
carrying `schema: "vela.correction-repair-obligation.v1"`. It has a shipped UI
surface: `DecisionInboxNextObligation` in
`crates/vela-cli/src/decision_inbox.rs`, mirrored at
`packages/frontier-data/src/index.ts:325` and rendered by
`apps/observatory/src/components/vela/decision-boundary.tsx:46`. What it lacks
is a JSON Schema in `schemas/` (which holds eight), a projection table, and a
route. Unlike Problem, Obligation does not lack a decision — ADR 0039 §4 and §6
name it twice. It lacks an implementation of a decision that exists.

### 1,217 open Problems are stored as accepted Claims

The Erdős catalogue was bulk-imported as Standing. A catalogue row was being
presented as adjudicated scientific state. This is the largest single defect in
the ecosystem and the reason for the reset.

Reclassifying the corpus is not a safe change to the map, it is a rewrite of it.
The Problem product's join key *is* `observatory.claims`, and its prize and tags
come out of the Claim's assertion string. If the corpus stops being Claims, the
Problem surface loses both in the same commit. Plan the replacement before the
reclassification, not after.

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

- **The epoch rename is done except for two absent additions.** Five Event
  kinds are `claim.*` and `target.claimed`
  (`crates/vela-protocol/src/kernel/events.rs`) and five vocabulary terms are
  documented as retired (`docs/TERMINOLOGY.md`). `frontier_id` → `repository_id`,
  `vfr_` → `vrepo_`, `frontier.toml` → `vela.toml`, `--frontier` → `--repo` and
  `vela.status.v3` → `v4` all landed: each is at zero across `crates/`,
  `schemas/`, `packages/` and the top level of `docs/`, and the occurrences that
  remain are deliberate references to the retired spelling — the retired-path
  predicate, `docs/ROOTS.md` stating that a prefix is never reused, and the
  tests that hold the new wording in place. This list previously called all five
  "not done" while §1 of this same document said the rename landed in v0.967.0,
  and it stated two of them as `vela.toml` → `vela.toml` and `--repo` → `--repo`
  after a sweep rewrote both sides of the arrow.

  Still absent: `crates/vela-protocol/src/epoch1/` and the read-only
  `vela history <path>`. Both are additions the ADR asked for, not leftovers of
  a half-executed rename.
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
  (`conformance/fixtures/epoch1/authorization-profile-parity-v1.json`), but the
  evaluator is called only from tests and `cedar-policy = "=4.11.2"` is still a
  workspace dependency of the active writer.
- **`serde_yaml_ng` is a `serde_yaml` fork.** `Cargo.toml:43`. The standards
  audit named forks as the option to avoid, and a fork is what was adopted. It
  is dev-only today (`crates/vela-protocol/tests/action_contracts.rs`), which
  bounds the exposure but does not resolve the decision.
- **License fields are not SPDX.** `current_repository.rs:105-119` validates
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
  `python`. `conformance/emitters/javascript.mjs` emits submission and
  verification objects but never reads the vector corpus.
- **The portable TypeScript waist was removed, not deferred.**
  `@vela-science/protocol@0.1.0` was published (ADR 0024) and
  `packages/` now holds only `vela-source-manifest`. `docs/THEORY.md:16` still
  claims "The TypeScript package and language-neutral vectors check the
  portable producer boundary." That sentence is false.
- **No proptest, no cargo-fuzz, no CodeQL, no Scorecard, no CODEOWNERS.**
  `.github/workflows/` holds `conformance.yml` and `release.yml` only.
  `SECURITY.md:5` routes disclosure to a personal Gmail address.

### Documentation contradictions to resolve

- `docs/ARCHITECTURE.md:183` says "The existing Observatory is the first-party
  Math Atlas"; `:300-301` says it is the Observatory "over the four declared
  Frontiers". The second binds the Atlas to repositories the reset freezes.
- `docs/TERMINOLOGY.md:140` says "three maintained mathematical Frontiers";
  `:269` and `:289` say "All four". `:239` still derives `superseded` from a
  `finding.superseded` Event, 120 lines after the same file retires
  `finding.*`. `:139` calls Atlas current and `:407` calls it future.
- ~~Two taglines ship at once.~~ Resolved: everything now says "version
  control for scientific state", which is what the binary prints, and
  `the_documented_tagline_is_the_one_the_binary_prints` in
  `crates/vela-protocol/tests/cli_release_contract.rs` holds the six documents
  to the binary rather than to each other.
- ~~`docs/ROADMAP.md:7` says `create -> submit -> verify -> decide -> replay`.~~
  Resolved: it says `init -> …`, which is the verb the CLI has.
- ADR 0017 is "Deferred — research only" and forbids implementing a Frontier
  calculus. Its two layer names, Frontier Algebra and Discovery Calculus, are
  live rows in `docs/TERMINOLOGY.md:396-397` and `frontier_algebra` is a
  permanently-null field in a `vela-web` projection fixture. Supersede 0017 or
  rename its layers; a reserved field that is never filled is worse than an
  absent one.

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
Proposal status, repository integrity. `packages/ui/src/components/vela/state-glyph.tsx`
separates two of them visually today (ring = standing, core = verification).

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
   run through `conformance/verify.py`. Its only caller in Rust is
   `crates/vela-edge/tests/correction_impact.rs` with a synthetic input: it has
   never run against a real repository, and no CLI verb reaches it. This is the
   highest-value thing already built and not yet reachable.
3. **Projections cannot silently go stale.** Every projection row is
   root-bound. The disclosure contract is partial: `projector_version`, lens
   identity and truncation rules are not built.

### Invented and unbuilt. Delete rather than re-base

Executable Frontier Model, Frontier Algebra, Discovery Calculus, Frontier
Calculus, Verified Frontier Learning, FrontierBench, possible worlds,
distinction partitions, capabilities, the Frontier Inheritance Effect,
long-horizon transition credit, Constellation, Lens, Capsules, Translation
Studio, Atlas-as-application, release modes, risk tiers, sealed commitments.

None of these has an implementation. All of them are named after `Frontier`,
which no longer has an identity, so re-basing the names on the derived noun
would preserve a tower whose foundation was removed.

Already retired and not to be reintroduced: Finding, Frontier Commit, Review
Packet, Frontier map, Attempt (ADR 0039 §5), and Registration Record (ADR
0033).

## 8. Layering, and what must never depend on what

```text
  kernel        crates/vela-protocol, vela-authority, vela-verify
                  ↑ objects, roots, signatures, authority, replay, Standing
  operator      crates/vela-cli
                  ↑ 15 verbs: replay status claims log verification reproduce
                    authority init review show why next start submit completions
  readers       conformance/readers/python, conformance/emitters/javascript.mjs
                  ↑ independent implementations of the same bytes
  analysis      crates/vela-edge
                  ↑ correction impact, target index; read-only, never required
                    for replay
  projection    vela-web/packages/frontier-data  (21 tables, all root-bound)
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
  `vela-science/vela@c4023f11`, `v0.967.0`); the test comment is a soft reverse
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
`apps/observatory/src/app/frontiers/` routes by topic slug and keys eleven
projection tables on `frontier_slug`. Under ADR 0039 a Frontier is derived and
has no identifier, and topic is the one thing a Repository must never encode.
That surface needs re-deriving against Problem, not renaming.
