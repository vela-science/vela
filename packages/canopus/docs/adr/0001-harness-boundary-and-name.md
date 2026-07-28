# ADR 0001: Canopus is a replaceable Vela research harness

- Status: accepted
- Date: 2026-07-15
- Decision owner: William Blair

## Context

Vela already supplies the durable boundary: content-addressed objects, replay,
policy routing, human-key custody, and Git publication. Research systems still
need a practical loop for choosing a bounded mission, running specialized
workers, preserving failures, verifying exact bytes, repairing attempts, and
turning the result into Receipt v1.

Older projects used the Canopus name for several broader protocol,
stewardship, and runtime designs. Those designs remain historical. On
2026-07-15 the decision owner explicitly reactivated the name for the narrow
orchestration product defined here.

## Decision

The product is **Canopus**. Its descriptive repository name is **Vela Research
Harness**, its repository is `vela-research-harness`, and its CLI is `canopus`.

Canopus may:

- select and claim work through released `vela next` and `vela work`;
- prepare an exact, isolated Git checkout with explicit budgets;
- run replaceable producer, adversary, verifier, and fidelity roles;
- freeze content-addressed artifacts and preserve null or failed attempts;
- map verified candidate facts into Receipt v1;
- call `vela land` as an `agent:` actor and report the returned route;
- render removable, read-only projections of its run records.

Canopus may not:

- invoke `vela sign`, make a human accept/reject decision, or read a human key;
- forge accepted Vela events or hand-edit derived Vela views; `vela land` may
  mechanically apply an already signed Permit policy and must report that
  effect exactly;
- duplicate Vela policy, signature, replay, or authority logic;
- treat a model, worker, verifier, cache, graph, or activity store as truth;
- branch substantive downstream work from a merely pending claim.

The implementation is one strict TypeScript package on Node LTS. It starts
with subprocess adapters for the released Vela and Codex CLIs, JSON Schemas for
missions and candidates, local append-only activity records, clean worker and
verifier lanes, and no hosted control plane.

## Durable identities

Every mission binds full Git commit and tree identifiers plus full Vela event
log and snapshot roots. Every artifact is hashed from bounded regular-file
bytes. A Canopus run ID is useful for operations but is never a replacement for
those roots.

Before any Vela mutation, Canopus rechecks the roots it observed. After
landing, it records the returned receipt, proposal, operation, publication,
and accepted-state effect exactly as Vela reports them. Pending does not mean
accepted.

## Trust assumptions

The operator controls the host and supplies immutable local Vela and Codex
binaries whose SHA-256 values are registered in the mission or benchmark.
Canopus detects version or digest drift but does not defend against a
privileged host replacing a binary between verification and execution. The
source Git object database is trusted to serve the named commit/tree; the
checkout rejects symlinks, hard links, dirt, and root mismatches before it is
sealed. Vela's transaction and publication barriers own concurrent scientific
writes in the isolated landing clone.

The Codex process needs provider network access. Its model-facing lane has no
shell, patch, browser, app, MCP, memory, or computer-use tools and the outer
Seatbelt profile restricts host reads, but the registered mission and Vela
briefing are sent to the provider. They must not contain secrets.

## Deletion test

The architecture is conforming only if all Canopus code, activity records,
queues, and projections can be deleted while:

1. the frontier replays to the same accepted Vela root;
2. committed receipts and artifacts remain addressable through Git/Vela;
3. a clean reader can reproduce the verifier result from published bytes;
4. no accepted outcome depends on a Canopus database or service.

## Consequences

Canopus can iterate quickly without entering Vela's trust path. Engines and
role prompts can change without a protocol migration. The cost is that local
run status is intentionally weaker than Vela state. Defer stops at the review
queue; Permit is only the result of authority Vela already held, never
authority created by Canopus.

Queues, SQLite, a web console, remote workers, and additional model providers
are deferred until the bounded single-worker loop demonstrates a concrete need.
Derived graphs or wikis may be added only as rebuildable projections.
