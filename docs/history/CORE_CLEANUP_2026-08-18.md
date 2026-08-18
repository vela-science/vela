# Core cleanup and repository inventory, 2026-08-18

> Historical maintenance record. This file records one repository audit and
> cleanup. It does not define Protocol semantics, release compatibility, or
> downstream authority.

Baseline: clean `main` at
`da3bf30bb22b5ebea90e0181ec9a9fdebf933a00`, equal to `origin/main` after
fetch.

## Retained repository

| Surface | Owner and mechanical consumer | Source status | Protocol or release role | Decision |
| --- | --- | --- | --- | --- |
| `.github/workflows/conformance.yml` | GitHub Actions | source | full Core and object-waist gate | KEEP |
| `.github/workflows/release.yml`, `.github/release/*` | release workflow and `scripts/release.sh` | source | archives, SBOM checks, bundle smoke tests, provenance transport | KEEP |
| `.github/workflows/scorecard.yml` | GitHub Actions | source | repository security reporting | KEEP |
| `.github/CODEOWNERS`, `.github/dependabot.yml` | GitHub review routing and dependency updater | source config | repository maintenance, no runtime role | KEEP |
| `crates/vela-protocol` | workspace and the other three crates | source | Protocol 1 objects, schemas, roots, replay types | KEEP |
| `crates/vela-authority` | CLI Decision path | source | restricted authorization evaluation | KEEP |
| `crates/vela-repository` | CLI transactions and recovery | source | policy-neutral durable writes | KEEP |
| `crates/vela-cli` | shipped `vela` binary | source | the complete operator loop and stable JSON contracts | KEEP |
| `schemas/*.schema.json` | `wire_schema::published()`, schema tests, Protocol manifest, downstream readers | generated and tracked | closed current wire and read schemas | KEEP |
| `conformance/protocol-1.json` | `verify_protocol_1.py` | generated and tracked | digest-bound Protocol selection | KEEP and REGENERATE |
| `conformance/current-objects/*` | current-object verifier | source vectors | positive v3 objects and retired-format refusal cases | KEEP |
| `conformance/fixtures/authority/*` | authority-chain verifier and Rust conformance tests | source vectors | current authority and replay chain | KEEP |
| `conformance/fixtures/correction/*` | correction-impact verifier | source vectors | derived projection behavior | KEEP |
| `conformance/emitters/*`, `conformance/readers/*` | conformance driver and architecture test | independent source | cross-language canonical-byte checks | KEEP |
| other `conformance/*.py`, `*.mjs`, `*.sh`, locks | `verify.py`, `check-core.sh`, workflows | source and locks | protocol, schema, reference-flow, release, and install gates | KEEP |
| `conformance/pyproject.toml`, `uv.lock`, `.python-version`, `README.md` | uv, Ruff, CI, and operators | source config, lock, and guidance | reproducible independent verifier environment | KEEP |
| `examples/README.md`, `computational-science`, `correction-inheritance`, `formal-math`, `review-methods` | Protocol manifest and reference-flow verifier | source fixtures | three reference flows and review method examples | KEEP |
| `scripts/release.sh`, `release_manifest.py`, `sign-published-release.sh` | release workflow, release tests, operator docs | source | one release entrypoint, manifest generation, signing/publication | KEEP |
| `install.sh` | release bundles, release tests, README | source | signed manifest installation | KEEP; repair quickstart link |
| `allowed_signers` | installer and release-signature tests | source trust root | out-of-band release identity | KEEP |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml` | Cargo, CI, release script | source and generated lock | workspace, toolchain, dependency policy | KEEP |
| `.gitattributes`, `.gitignore` | Git and release/archive tests | source config | line endings and excluded outputs | KEEP |
| `AGENTS.md`, `CLAUDE.md` | repository coding agents | source guidance | contributor boundary, no runtime effect | KEEP |
| `README.md`, `SECURITY.md`, `CITATION.cff`, `CHANGELOG.md` | users, security reporters, citation tools, release history | source records | current entrypoints and published history | KEEP and CONSOLIDATE |
| `LICENSE*` | source and release packages | source legal text | dual-license and trademark boundary | KEEP |
| `assets/brand/vela-readme-hero.jpg`, `assets/brand/LICENSE` | README and brand license link | source asset and license | presentation only | KEEP |
| `paper/vela.md`, render and artifact scripts, tests, bibliography and source manifest | paper README and direct commands | source | non-normative working paper | KEEP |
| `paper/artifacts/cost`, `heldout-selection`, `map-target-loop`, `transfer` | exact citations and commands in `paper/vela.md` | historical evidence | reproducibility inputs, no runtime role | KEEP |
| `paper/artifacts/erdos-424`, `formal-505` | paper artifact commands | historical test evidence | source-artifact verification | KEEP |
| `paper/artifacts/lean-replay-package-qualification` | this dated audit and its own record | historical negative evidence | failed extraction stop condition | MOVE from `research/` |
| Git tags and published releases | reproducibility readers and release records | immutable history | binaries and source for repositories of their era | KEEP |

The four Cargo workspace members are the only tracked crate directories. Rust
module reachability, Cargo manifests, clippy, and workspace tests cover their
source. The schema test compares every tracked schema with
`wire_schema::published()` and rejects orphans. The Protocol manifest selects
every example and current conformance fixture by path and digest.

## Documentation classification

The current set is:

- product overview and install: `README.md`, `docs/README.md`,
  `docs/QUICKSTART.md`;
- Protocol and implementation: `docs/PROTOCOL.md`,
  `docs/ARCHITECTURE.md`, `docs/CLI.md`, `docs/REPOSITORY_PROFILE.md`,
  `docs/ROOTS.md`;
- authority and trust: `docs/SIGNING.md`, `docs/VERIFICATION.md`,
  `docs/THREAT_MODEL.md`, `docs/REPOSITORY_BOUNDARIES.md`;
- interoperability and operations: `docs/INTEROPERABILITY.md`,
  `docs/CONTINUITY.md`, `docs/PUBLISHING.md`, `docs/RELEASES.md`,
  `docs/interop/scientific-state-profile.md`;
- integrations: the three files under `docs/integrations/`.

All 47 files `docs/adr/0001-*.md` through `0047-*.md` are
historical-but-useful decision records. `docs/adr/README.md` lists each one
and now states that current contracts override retired ADR surfaces.

The files listed by `docs/history/README.md` are historical-but-useful
migration, qualification, portability, governance, and rejected-design
records. Four completed but still useful documents moved there with dated
historical labels: the Submission v3 migration, portable-waist campaign,
external-validation program, and gittuf deletion spike.

## Deleted source and records

The cleanup deletes these unconsumed surfaces instead of keeping aliases,
fallbacks, or archive wrappers:

- duplicated quickstarts and current explanations:
  `docs/AGENT_QUICKSTART.md`, `PRODUCER_QUICKSTART.md`, `ECOSYSTEM.md`,
  `TERMINOLOGY.md`, `THEORY.md`, `PROTOCOL_ADOPTION.md`, and
  `REVIEW_PROVENANCE.md`;
- completed planning and benchmark scaffolding:
  `docs/CAMPAIGN.md`, `ROADMAP.md`, `BREAKTHROUGH_BENCHMARK.md`, and
  `WHITEPAPER_CONTRACT.md`;
- three obsolete dated ecosystem/campaign ledgers under `docs/history/`;
- 17 paper artifact directories for retired frontier campaigns, dossier
  usability runs, dependency-profile observations, product-compression runs,
  proof-repair trials, removability, and state-lift;
- the unused `assets/brand/vela-logo-wordmark.svg`.

Git history preserves all deleted material. No build manifest, test runner,
release workflow, schema manifest, package, current paper citation, or current
downstream pin consumes it.

## Retired-format truth

Current Core contains no Submission v2 reader or writer, execution-binding
type, epoch-1 loader, or compatibility branch.

Intentional old-version references have these consumers:

| Reference | Exact current consumer | Reason retained |
| --- | --- | --- |
| `vela.submission.v2` and v2 media type | Rust intake/unit tests plus `verify_current_objects.py` and `verify_wire_schemas.py` | prove current intake and schemas refuse retired bytes |
| `execution_binding` and `vela.execution-binding.v1` | the same Rust and independent Python refusal tests | prove unknown retired fields fail closed |
| `frontier.toml`, `vela.frontier-profile.v1`, epoch-1 release `v0.966.4` | `docs/CONTINUITY.md` | explicit non-default recovery guidance for four archived repositories |
| `vfr_` | `docs/ROOTS.md` and retained paper artifacts | interpret historical typed identifiers without adding a reader |
| epoch-1 relation observations | CLI, interoperability, and source comments | explain why the current writer produces no dependency cascade; no loader exists |
| Math rollback ref in the authority fixture | immutable current conformance Event bytes | exact signed caveat; Core does not follow or read the branch |
| older version strings in ADRs, history, paper evidence, and `CHANGELOG.md` | dated reproducibility records | preserve decisions and published evidence |

## Release hygiene

GitHub release `v0.974.0` was still a draft, unpublished, and had ten assets
with zero downloads. Code search across the organization and local repositories
found no tag, URL, or asset consumer. The audit deleted GitHub draft release
object `370133148` only. It retained annotated tag object
`a5e7394dae59c870d72690a90818c58cc31371bc` and peeled commit
`f95510a5351ba808f54d456e488a95a9044be250`.

GitHub cannot restore the deleted draft object or its uploaded assets. The tag
allows an operator to recreate a new draft and rebuild or re-upload the
artifacts if evidence later requires it. No published signed release or tag
changed.

## Local Git and cache audit

The audit found only generated ignored outputs: Rust `target/`, the locked
conformance virtual environment, Ruff caches, and Python bytecode created by
the verification runs. `git clean -ndX` named those paths and no ignored
secret, release asset, or source file; the cleanup removed that exact set after
the final test run. `git worktree prune --dry-run --verbose` reported no stale
worktree. Review corrections left five unreachable staging blobs; `git gc
--prune=now` removed them. The final object store has one 62.75 MiB pack, no
loose objects or garbage, and no object reported by `git fsck --full
--no-reflogs --unreachable`.

The audit removed empty local remnants for retired `vela-edge`,
`vela-verify`, `vela-source-manifest`, epoch-1 fixtures, deleted paper
artifacts, and the old `research/` location. The detached Codex worktree at
`/Users/williamblair/.codex/worktrees/7659/vela` resolves to the baseline
commit and remains a live worktree, so the audit retained it.

## Downstream observations and handoff

Read-only inspection found `vela-web` clean and equal to `origin/main` at
`5ab7d4f5`. Its governed generator and copied schema provenance still pin
signed `v0.977.0`. A reviewed Web deployment should update every release
config, installer helper, source-inspection constant, threat-model statement,
and fixture to signed `v0.977.1`; regenerate both projection-data and
activity-data schema provenance; then run the package tests and deploy checks.
The v0.977.1 manifests bind commit
`0e057c0debcff775a3deb56150ceaccfd4707b41`, Linux binary digest
`sha256:3c25344f2a636a803d82fd7cf663e5638778d1121198301f478ff3dcc18f0270`,
Linux archive digest
`sha256:a8a6c74c7694ea64b69b70d412c90506bc681de137b15c72416b3e7b2f7abf56`,
macOS binary digest
`sha256:a4f5594b2777b265f6d58296cc8e9efd85d0a72c82b49c0fce4805438ed46948`,
and macOS archive digest
`sha256:c4e591c8683754ac0e310912b5227a697213bd4812e836ac88bef40430d9e7a6`.

Read-only inspection found `math` clean and equal to `origin/main` at
`f4672aa`. Its current signed Erdős 94 records and migration document still
cite `rollback/submission-v2-coh-00` at `508b39a`. The Math owner must submit
a compact current-state Claim revision, retain scoped independent Verification,
and make an authorized Decision that replaces the transient branch caveat with
a stable historical locator if recovery still needs one. The owner should then
update `README.md` and `MIGRATION.md`, verify strict replay, and delete the
rollback branch only after no current signed record or document depends on it.
This Core cleanup performs no Math scientific or authority write.

## Change size and verification

Against the baseline, the cleanup changes 183 files with 421 inserted lines
and 19,167 deleted lines, a net deletion of 18,746 lines. The tracked tree
shrinks from 7,766,193 to 6,869,444 bytes, a net deletion of 896,749 bytes, and
from 550 to 407 files.

The following local gates passed:

- `cargo fmt --all -- --check`;
- focused documentation, wording, repository-argument, and review-admission
  integration tests;
- all 28 tests across the six retained paper evidence suites;
- `uv run --project conformance --locked ./conformance/check-core.sh`,
  including the ordinary and crash-recovery workspace unions;
- `cargo clippy --locked --workspace --all-targets -- -D warnings`;
- `git diff --check` and a relative-link check over all non-ADR Markdown.

The remaining broken relative links occur only inside immutable ADR bodies.
They point to paths that existed when those decisions were written. The audit
did not rewrite ADR text toward current state.
