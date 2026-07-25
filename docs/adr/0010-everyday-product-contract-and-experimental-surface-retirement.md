# ADR 0010: Everyday product contract and experimental surface retirement

- Status: Accepted
- Accepted: 2026-07-16 at the Vela `v0.900.0` release gate
- Target release: Vela `v0.900.0`
- Protocol effect: none
- Authority effect: none
- Acceptance gate: the `v0.900.0` release commit must pass the focused product
  tests, old-frontier replay, core conformance, frontier conformance, and the
  deterministic release union.

## Context

Vela `v0.800.23` preserves immutable event history, verifies Receipt v1,
separates agent work from human decisions, and repairs completed operation
journals after derived-view regeneration. The core binary also exposes years of
experiments as peer commands. A new operator sees producer work, review queues,
Hub administration, Atlas projections, proof tooling, campaign adapters, and
old aliases in one list.

The output contracts carry the same accumulation. Erdős `status --json`
included Decision Briefs and testing metrics. `next` mixed review work with
producer targets. `work` embedded a tracked packet body. Fresh `init` installed
MCP, CI, proof projections, and editor adapters before the operator had named a
bounded question.

Vela needs a product boundary that a producer, reviewer, or reader can learn in
one session. Existing event, proposal, Receipt, registration, and policy bytes
must replay without conversion.

## Decision

Vela `v0.900.0` makes a breaking pre-1.0 CLI cut. It changes commands and
derived projections. It adds no event kind, object schema, signature rule,
accepted-state rule, or authority service.

The release has four product laws:

1. The default command list teaches the daily path.
2. Machine output binds exact roots and stays small.
3. Migration rewrites derived product files while preserving canonical bytes.
4. Human keys remain outside producer, verifier, migration, and reader paths.

ADRs 0006 through 0009 remain Proposed. Vela will not promote their candidate
primitives during this release.

## Stable command surface

Default help exposes these commands:

```text
init status next work land review sign check reproduce log doctor migrate
```

Setup and noun commands remain available:

```text
finding artifact frontier policy actor id agents config
```

Advanced help contains:

```text
gate proof ci serve
```

The core binary retires these surfaces:

| Retired surface | 0.9 replacement |
| --- | --- |
| `proposals` | `review list`, `review show`, `review preview`, `review export` |
| `diff vpr_*` | `review preview` |
| frontier-to-frontier `diff` | `frontier diff` |
| `state` | `finding show --view record` or `--view standing` |
| `credit` | `finding show --view evidence` or `--view attribution` |
| `publication` | `frontier recover-publication` |
| `hub` | `vela serve` for local reads or an optional external reader |
| `foundry` | parent scripts or Canopus profiles |
| `atlas` | read-only projections outside the core CLI |
| `reproduce-external` | a named Canopus verifier profile |

A retired command exits with usage status and one replacement. Vela does not
execute a compatibility alias because an alias can preserve an obsolete
contract after users believe they migrated.

The unpublished `vela-hub` compatibility crate was subsequently removed from
the breaking `0.930` source train after consumer tracing found no current
client, deployment, release binary, published crate, or unique canonical
state. Historical tags retain its implementation.

## Compact read contracts

These schemas project current state. They do not enter the canonical store.

### `vela.status.v1`

`vela status <frontier> --json` returns:

- frontier ID and name;
- Git commit, tree, and checkout cleanliness;
- full event, snapshot, proposal, and actor-registry roots;
- replay result and strict blockers grouped by code;
- event, finding, open-work, and pending-review counts;
- policy state and permit readiness; and
- one next action.

Erdős output must remain at most 16 KiB. Human output must remain at most 40
lines. The default projection contains no Decision Brief, packet body, review
pressure report, or `.testing.v1` metrics.

### `vela.offer.v1`

`vela next <frontier> --limit 1 --json` returns producer targets with:

```text
rank
target_id
title
objective
packet path and full root
verifier profile
lease state
next command
```

The response must remain at most 8 KiB for one Erdős target. `next` excludes
review items. Reviewers use `review list`.

### `vela.work.v1`

`vela work <target> --json` returns the session ID, starting roots, exact target
and packet reference, bounded completion contract, verifier profile, and
landing command. The tracked packet holds the full briefing. The JSON response
must remain at most 16 KiB without the packet.

### `vela.review.v1`

`vela review list --json` returns paginated proposal summaries ordered by
`created_at` newest-first, then full proposal ID. Every row includes its exact
`created_at`. `review show` returns one full Decision Brief. List output cannot
embed the full queue of briefs.

## Initialization and diagnosis

`vela init <path> --name <name> --scope <question>` creates:

```text
.vela canonical skeleton
README.md
SCOPE.md
frontier.yaml
frontier.json
vela.lock
.gitignore
.gitattributes
VELA.md
versioned Git safety hooks
```

It creates no proof packet, MCP configuration, CI workflow, review-pressure
metrics, or optional editor integration. JSON mode refuses an omitted name or
scope. Human mode asks for each missing value.

`doctor` reports blockers and one next action. `doctor --all` adds tool
inventory, setup diagnostics, and optional integration advice.

## Migration contract

Vela adds:

```bash
vela migrate <frontier> --to 0.900 --check --json
vela migrate <frontier> --to 0.900 --apply --json
```

`--check` builds an isolated clean preview and lists each file whose bytes
would change. `--apply` copies the verified projection into the source
checkout with atomic file replacement.

Migration refuses:

- tracked changes;
- unrelated untracked files;
- an active or incomplete operation journal;
- a checkout behind or forked from its upstream;
- a replay failure;
- any canonical-root change; or
- a preview that touches a path outside the closed derived-output set.

Migration permits ignored completed journals after the transaction barrier
verifies their markers, stored blobs, event membership, roots, and durable
postimages.

The 0.9 migration removes retired manifest fields such as `carina`, updates the
reducer label, and regenerates these derived files when their bytes differ:

```text
frontier.yaml
frontier.json
vela.lock
proof/latest.json
proof/events.manifest.jsonl
proof/replay.trace.jsonl
proof/freshness.md
proof/hashes.json
```

Migration records before and after values for:

```text
Git commit and tree
event-log root
snapshot root
proposal root
actor-registry root
artifact root
canonical-store root
```

The event, proposal, Receipt, registration, artifact, and signed-policy stores
remain byte-identical. The command reports a stale proof pointer and artifact
reference debt as debt. It does not repair either one.

Migration clean means the canonical roots match and reducer replay succeeds.
Strict scientific blockers can remain after a clean migration.

## Compatibility

Vela `v0.900.0` replays all released canonical event fixtures. The release
keeps `v0.800.23` binaries and tags available for historical command replay.
Old scripts that call retired commands receive a migration hint and must adopt
the new interface.

The compact schemas begin at `v1`. A reader must reject an unknown schema or a
missing full root. Human formatting has no compatibility guarantee.

## Authority boundary

This ADR changes product navigation and derived files. Agents keep the existing
permissions to inspect, reproduce, register evidence, claim work, and land a
Receipt that policy routes to `Deferred` or `pending_review`.

Only a human key holder may accept, reject, apply, or finalize a truth-bearing
proposal. `migrate`, `status`, `next`, `work`, `review`, Canopus, Hub, Atlas,
and the site cannot read or invoke that key. Git publication records bytes; it
does not make a scientific decision.

## Release gate

The release accepts this ADR only after the exact release commit passes:

```bash
cargo test -p vela-cli compact_contract
cargo test -p vela-cli migration
cargo test -p vela-cli init_minimal
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
./scripts/full-conformance.sh --suite core --mode=ci
./scripts/full-conformance.sh --suite frontier --mode=ci
```

The maintainer runs the deterministic full release union once at the release
boundary. The gate also migrates clean clones of Sidon,
formal-conjectures-frontier, and quantum-codes-frontier and compares canonical
roots before and after.

The release stops on a canonical-history rewrite, false replay or strict pass,
root drift, authority confusion, key exposure, or a migration that hides known
scientific debt.

## Consequences

Operators lose command aliases and several experimental wrappers. They gain a
smaller default path, bounded JSON, a root-preserving migration preview, and a
separate review queue.

Canopus owns bounded producer orchestration after this cut. The read-only site
consumes the compact projections. Hub remains a separate read-only binary.
Research code can stay in Git history or a named profile without occupying the
core product surface.
