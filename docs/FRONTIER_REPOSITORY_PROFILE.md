# Frontier Repository Profile

This document explains how Vela packages one replayable Frontier in ordinary
Git. It consolidates the released `vela.frontier_repo.v0.1` contract; it does
not create a new protocol object or make a host integration authoritative.

The proposed replacement is
[ADR 0016](adr/0016-frontier-repository-profile-v1-and-legacy-identity-migration.md).
Until that ADR is accepted and released, `v0.1` remains the implemented
profile.

## One sentence

> Git stores and collaborates; Vela interprets and governs scientific state;
> domain tools produce evidence; readers project replayed state.

## Four layers

### State protocol

Receipt, finding, proposal, event, actor, policy, signature, verifier,
canonicalization, replay, correction, and standing semantics form Vela's
narrow protocol waist. Only an already authorized signed policy route or a
protected human decision admits a truth-bearing transition.

### Repository profile

The profile locates retained protocol bytes, retained evidence, generated
views, human documentation, and local-only state inside one ordinary Git
repository. One repository contains one Frontier authority and correction
boundary by default.

### Frontier Kits

README sections, domain directories, issue forms, pull-request templates,
CODEOWNERS examples, devcontainers, and verifier skeletons are optional
conventions applied after minimal initialization. They are not protocol and
must not copy a live Frontier identity or accepted history.

### Host and tool integrations

GitHub, Vercel, Hugging Face, Entire, Codex, Claude Code, Canopus, Neon, and
other systems may produce, transport, inspect, or project exact bytes. They do
not gain authority by integration and must remain removable without changing
replay or standing.

## Released v0.1 markers

A current Frontier declares:

```yaml
schema: vela.frontier_manifest.v0.1
layout: vela.frontier_repo.v0.1
mode: split
```

The repository name, host, and branch are locators. The replayed Frontier ID,
full roots, exact Git commit/tree, and retained signatures carry identity and
integrity according to their defined roles.

## Path ownership

| Path | Class | Editing rule |
| --- | --- | --- |
| `.vela/events/`, actor registry, retained policy pairs | Canonical protocol | Change only through released Vela operations |
| `.vela/proposals/`, `.vela/findings/`, `.vela/artifacts/` | Retained or reducer-owned protocol records | Never hand-edit |
| `records/receipts/sha256/` and retained evidence | Exact evidence | Preserve by full digest |
| `frontier.yaml` | Human repository manifest | Edit deliberately, then validate and materialize |
| `frontier.json` | Generated current view | Regenerate; never hand-edit |
| `vela.lock` | Generated reproducibility witness | Regenerate; never hand-edit |
| `proof/` | Generated Vela replay and integrity packet | Reserved for Vela; regenerate |
| `README.md` | Frontier Card | Human onboarding |
| `SCOPE.md` | Expanded scientific boundary | Human-authored, consistent with the manifest |
| `VELA.md` | Canonical agent charter | Edit here; generate adapters explicitly |
| `targets.json` and target packets | Optional derived work projection | Root and validate; never treat as standing |
| domain-native files | Scientific source and evidence | Govern with domain tools and stable identity paths |
| `.vela/work/`, operation journals | Local coordination and recovery | Never publish as scientific state |

Root `proof/` is Vela's integrity packet. Mathematical proofs belong in a
domain path such as `formal/`, `theorems/`, or `lean/`.

Two maintained repositories contain immutable legacy exceptions. Formal
Conjectures has a Receipt-bound Lean file under `proof/`; Sidon has tracked
public artifact blobs under `.vela/artifact-blobs/`. They remain addressable at
their exact historical paths until a digest-locator compatibility migration is
proved. They are not precedents for new v1 writes.

## Repository boundary

Keep content in one Frontier when it shares authority, correction policy,
confidentiality, namespace, source cadence, and steward group. Split when one
of those changes materially. File count alone is not a reason to create a new
authority history.

A portfolio or workspace can pin several exact Frontier roots. It is a
derived collection unless it owns its own bounded claims and signed events.
Do not nest several canonical `.vela/` directories under one repository and
call the root one Frontier.

## Stable paths and scale

Paths identify objects; they should not encode mutable state. Prefer
`problems/000646/` plus projected standing over `open/646/` and `solved/646/`.

Use derived indexes, deterministic prefix sharding, partial graph
neighborhoods, and digest-addressed artifact storage before splitting a
scientifically coherent Frontier. Event-store sharding or another layout
version requires benchmark evidence and conformance fixtures.

## Existing target index

`vela.target-index.v1` is the optional generic work bridge documented in
[TARGET_INDEX.md](TARGET_INDEX.md). It is already used by the maintained
Erdős, Sidon, Formal Conjectures, and Quantum Codes Frontiers. Vela validates
its closed shared fields and exact packet roots, but the released CLI does not
fail closed when the index's source state becomes stale. Three of the four
maintained indexes currently demonstrate that gap. Domain packet schemas
remain domain-owned.

Deleting the index removes a work-catalog convenience and changes no accepted
state. Graph position and structural advice never replace canonical producer
ranking.

## Host integrations

- A Git push publishes bytes; it does not accept a claim.
- A pull request, CODEOWNERS approval, or required check coordinates Git
  review; it does not replace a Vela decision.
- A preview renders one exact candidate commit and roots; deployment success
  does not establish standing.
- A mirror carries exact artifacts or discovery metadata; it is not a second
  writable authority.
- A process checkpoint may be referenced as provenance; raw agent history is
  not accepted evidence unless selected bytes enter the ordinary Receipt path.
- A webhook may request refresh; the consumer must fetch and verify the exact
  commit independently.

## Current limitation and proposed migration

The released v0.1 loader does not fully enforce its declared manifest version,
and initialization does not copy the required `--scope` value into the
manifest. Some manifest fields affect the materialized snapshot while others
are Git-only metadata. ADR 0016 proposes a closed non-authoritative v1 profile,
a full identity root, exact dependency roots, a signed legacy repository
boundary, scientific-state root v2, separate runtime settings, and stale-safe
target indexes.

That migration preserves every pre-boundary event, proposal, Receipt,
registration, policy, finding, artifact, and signature byte. It intentionally
appends one non-scientific boundary event, so the event-log and Git roots
change while the old roots remain anchored and auditable. This is a protocol
migration, not a relabeling or a regenerable-lock exemption.

Do not relabel a repository `v1` or hand-edit generated files before that
decision is implemented and released.
