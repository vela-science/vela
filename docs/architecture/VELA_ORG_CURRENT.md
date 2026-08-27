# Vela organization: current architecture

Status: canonical observed-state map for VELA-ORG-1
Observation cutoff: 2026-08-26

Supersession note (2026-08-27): the release-only facts in this dated inventory
are superseded by the signed, immutable Vela `0.977.6` release: annotated tag
object `4a562d4529f6a329d938fc427bc73c4cbff90767`, commit
`9ac8e7730bfb63a3b8eb1d2e1d91081c3e703c59`, tree
`1332713f627ac73c235e4f9a7afe206499717154`, and Protocol 1 manifest root
`sha256:bf1ef68165bccbc4d2e8a854f78c70448cc7de771bac23329f7a8ca115303f56`.
The `0.977.5` identities below remain immutable historical state. This note does
not re-audit or rewrite the remaining 2026-08-26 topology, deployment,
downstream-version, provider, or scientific-authority observations.

This document records the observed organization, product, deployment, provider,
and scientific-authority topology. It is authoritative for that dated
inventory, not for facts that were not observable and not for future state.
Facts that the inventory did not establish are marked **UNKNOWN**.

[Vela Protocol 1](../PROTOCOL.md) remains normative for scientific objects,
replay, and authority. [Vela architecture](../ARCHITECTURE.md) defines the
component model, and [repository ownership boundaries](../REPOSITORY_BOUNDARIES.md)
define placement. Where the older architecture document describes a different
organization topology, this dated map supersedes only that topology; it does
not change protocol or component semantics.

## Immutable release boundary

The released boundary is Vela `0.977.5` and Protocol `1`:

| Item | Exact identity |
| --- | --- |
| Annotated tag object | `0afe844862186cbf01a4ba91c4e6ad2129a8fcbc` |
| Release commit | `9cf13af9fd687db88e562842fd6dd641e10bae6a` |
| Release tree | `5863c283ad3a3efb76d365e5936544923851fb4a` |
| Protocol manifest root | `sha256:d3af662374c2940329016ffdeccdc406f30a5cf412c4b0b565ee5ee58e223af5` |
| Observed Core `main` | `4e85644d9a2f21b202d13f451c78d851aca04af6` |

Core `main` is one documentation-index commit beyond the release record. The
release tag, commit, tree, and manifest root are immutable historical state.

## GitHub repository inventory

The live `vela-science` organization contains exactly 16 observed repositories:
13 public and 3 private. Package visibility is **UNKNOWN** because the audit
identity did not have `read:packages`.

| Repository | Visibility and state | Classification | Observed role |
| --- | --- | --- | --- |
| `vela` | public, active | CORE | Protocol 1, CLI, schemas, conformance, replay, and releases |
| `problems` | public, active | PRODUCT | Canonical Problems and WebMCP source; source of the `problems.science` production deployment |
| `vela-workbench` | public, active | PRODUCT | Local execution, evidence preparation, and explicit authority handoff |
| `vela-web` | private, active | PRODUCT, TRANSITIONAL | Canonical source of the current `vela.space`; also retains predecessor Problems source for rollback and history |
| `math` | public, active | SCIENTIFIC REPOSITORY | Canonical reference mathematics authority Repository |
| `lean-correspondence` | public, active | REFERENCE / EXAMPLE | Non-authoritative exact relationship receipts |
| `.github` | public, active | REFERENCE / EXAMPLE | Organization profile and shared support |
| `erdos-frontier` | public, archived | ARCHIVE | Predecessor custody; a historical Pages surface remains live |
| `formal-conjectures-frontier` | public, archived | ARCHIVE | Predecessor proof-state custody |
| `prover-lane-frontier` | public, archived | ARCHIVE | Historical formal-proof frontier |
| `quantum-codes-frontier` | public, archived | ARCHIVE | Predecessor custody |
| `sidon-frontier` | public, archived | ARCHIVE | Predecessor custody |
| `vela-frontiers` | public, archived | ARCHIVE | Obsolete pre-consolidation registry and mirror |
| `vela-internal` | private, archived | ARCHIVE | Decomposed integration repository |
| `vela-research-harness` | public, archived | ARCHIVE | Canopus predecessor retained after current machinery moved into Core |
| `vela-site` | private, archived | ARCHIVE | Superseded reader absorbed into `vela-web` |

No repository is an evidence-backed delete candidate. Archive means retained
custody, not dispensability. No repository history, signed state, or archived
surface is authorized for deletion or rewriting by this map.

## Active deployment and DNS provenance

| Public name | Provider project | Canonical source | Deployed revision | Observed state |
| --- | --- | --- | --- | --- |
| `problems.science` | Vercel `problems` | `vela-science/problems` | `532241ba5db565e9ee35e13cbd7eff76393f6475` | Exact and current |
| `vela.space` | Vercel `vela-web-www` | `vela-science/vela-web` | `04741101bddf01c95a7e60145ab970f45b0ab30a` | Exact deployment, behind current source, and still linking Vela `0.977.2` |
| `app.vela.space` | no live project | none | none | Dangling CNAME; returns `DEPLOYMENT_NOT_FOUND` |
| `erdos.constellate.science` | GitHub Pages | archived `vela-science/erdos-frontier` | observed prefix `dba5f249...`; full SHA **UNKNOWN** | Live historical surface |

The first two rows are the active product deployments with exact provenance.
The latter two are still externally visible and therefore remain in the map,
but neither is an active product deployment. The old Vercel
`vela-web-problems` project remains as rollback infrastructure and retains the
same ten production variable names as the live Problems project; the values
and whether all names are still required are **UNKNOWN**.

## Authority, data, and provider boundaries

| System | Canonical responsibility | Explicit non-authority boundary |
| --- | --- | --- |
| A named Vela Repository in Git | Its own source locks, records, admitted Events, Decisions, replay state, and Standing | One Repository cannot decide for another; publication and merge do not change Standing |
| Vela Core | Protocol 1, canonical bytes and roots, replay, schemas, conformance, CLI contracts, and authority evaluation | It does not own scientific Decisions, hosted work, projections, or provider state |
| `vela-science/math` | Reference mathematics authority at repository root `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4` | Formal Conjectures statements, proof implementations, and correspondence evidence keep their native owners |
| Neon `lingering-meadow-20929365` / provider `vela-observatory-projection` | Operational PostgreSQL infrastructure | Neon is not a source of scientific Standing or Repository authority |
| `vela_projection.projection` | 23-table disposable, reconstructible scientific read model | A projection release never becomes canonical scientific state |
| `vela_activity.activity` | 21-table canonical hosted Problems identity, workspace, attempt, evidence, discussion, draft, and audit state, including application write functions | Canonical hosted product state is not scientific authority |
| `vela_observatory.observatory` | Legacy derived data | No evidence of current production use; application ownership is historical |
| WorkOS | Hosted account authentication | It grants no Vela Repository authority; exact dashboard policy is **UNKNOWN** |
| Entire | Optional activity provenance | Its absence falls back to ordinary Git and does not affect replay |
| Vercel, GitHub, WebMCP | Deployment, Git custody, and browser integration respectively | None can infer or mutate Standing merely by serving, merging, or invoking hosted actions |

Vercel, Neon, WorkOS, GitHub, Entire, and WebMCP are operational ecosystem
dependencies. None belongs to Vela Core's semantic dependency surface.

The current projection release root is
`sha256:c9d14c459c518937e758918b5897dc3b22f1a55f07739afe99502f5b046c907a`.
It was generated with Vela `0.977.3` and binds 15 sources and 6,598 native
records. Exact reconstruction requires a private authenticated source-adapter
asset retained in `vela-web`, so public `problems` is not yet self-contained
for release reconstruction.

## Product boundaries

### Problems and WebMCP

Public `problems` is canonical for Problems and WebMCP. Its eight browser tools
form a strict subset of hosted Problems permissions. They may read projection
and hosted Work, mutate attributed hosted Work, and prepare an unsigned
Submission v3 candidate with `authority_effect:none`. They cannot sign, issue a
Verification Record, issue a Decision or Event, or mutate Standing.

The private predecessor tip has 940 tracked paths and public Problems has 886.
Of the comparison set, 839 paths are byte-identical, 23 shared paths diverge,
78 are private-only, and 24 are public-only. Private-only `apps/www` and its
brand, UI, and projection dependency closure prevent retirement of `vela-web`.
Public source adapters still name private `vela-web` custody and require repair
after the frozen challenge gate.

### Workbench

`vela-workbench@00e96baac71c8fa5869431a79c077af4b716e258` owns local
activity and evidence-to-authority handoff. It hard-gates Vela `0.977.3` and has
no hosted CI. Qualification for `0.977.5` or a subsequent qualified patch is
blocked on clean current Math replay. The observed gap is limited to the
release pin, fixtures, documentation, and a narrow status-backed readiness
guard; no DTO, handoff schema, Protocol, or Workbench version redesign is
supported by the evidence.

### Math and formal-science sources

The current remote Math revision is
`cf6d76687b205a39e2515e9fec7087c819454d2f`. Math is the authority
Repository. Formal Conjectures owns upstream statements, `lean-proofs` owns
proof implementations, `lean-correspondence` retains non-authoritative exact
relationship evidence, and archived frontier repositories retain predecessor
authority history.

Fresh replay currently fails on two retained negative credential-scan reports
while status reports strict replay passed. This is a reader contradiction, not
evidence of incorrect Math authority. A narrow Core repair is in progress;
Math authority records are frozen.

## Version drift and transitional duplication

| Surface | Observed active binding | Drift from released Core `0.977.5` |
| --- | --- | --- |
| Core release | `0.977.5` | none |
| Projection generation and reconstruction | `0.977.3` | active integration behind |
| Workbench hard gate | `0.977.3` | active integration behind |
| Installed local binary used by the audited integration | `0.977.3` | local operator binding behind |
| Math documentation | `0.977.3` | documentation behind |
| `vela.space` documentation redirects | `0.977.2` | public documentation behind |

Historical receipts remain bound to the binaries and digests they originally
used. Only active integrations are migration candidates. A version string is
not sufficient binary identity; downstream repins must bind an accepted digest.

Transitional duplication is deliberate but temporary:

- `vela-web` retains the old Problems source and the still-canonical www source;
- Vercel retains `vela-web-problems` as rollback beside the live `problems` project;
- public source adapters depend on a private release-reconstruction asset;
- `vela_observatory.observatory` remains beside the current projection and
  activity schemas without evidence of production use;
- archived frontier and predecessor repositories retain history while current
  authority is consolidated in Math; and
- local custody includes historical clones, registered and prunable worktrees,
  research directories, two `vela-web` stashes, and a Math checkout behind its
  remote. None has been approved for cleanup.

## Licensing boundary

Tailwind Plus and shadcn.io Pro assets may be used in the end product but may
not be republished as a component registry or library. The shared UI package
must remain private, and registry or lab-catalogue files must not enter a public
extraction. Six publicly tracked and served ITF/Fontshare WOFF2 files require
written permission evidence or replacement and purge review. IBM Plex Mono
redistribution requires the full OFL and copyright notice in-tree.

## Explicit unknowns

- GitHub package visibility and inventory.
- Exact WorkOS dashboard policy.
- The full deployed commit SHA for `erdos.constellate.science` beyond the
  observed `dba5f249...` prefix.
- Whether every retained rollback environment-variable name is still needed,
  and the values of those variables.
- A rights-safe final owner and dependency closure for the extracted www
  surface; no new repository is authorized by this map.
- Written rights evidence or the final replacement choice for the six
  ITF/Fontshare files.
- User decisions on DNS, archive, rename, rollback closure, stash disposition,
  and local cleanup.
