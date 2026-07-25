# Hub and derived-projection retirement plan

> **Status:** Completed on 2026-07-25. The hosted service was previously
> sunset and archived; the unpublished compatibility crate is now removed
> after a complete consumer trace. See
> [`VELA_HUB_COMPATIBILITY_RETIREMENT_2026-07-25.md`](../reports/VELA_HUB_COMPATIBILITY_RETIREMENT_2026-07-25.md).

## Goal

Remove obsolete hosted Hub and duplicated product projection code while
preserving the smallest deterministic read contracts that released Vela,
frontier fixtures, Canopus, and Vela Web actually consume.

The connected product story is:

```text
produce -> preserve -> check -> decide -> reuse
```

- Canopus or another tool may **produce** bounded work.
- Canonical frontier Git repositories **preserve** it.
- Vela **checks** replay, evidence, roots, and strict signals.
- Existing Vela policy or protected human review **decides** standing.
- Vela Web and other optional readers support **reuse**.

The Hub is not an authority step and must not be replaced with another hosted
Vela service. The Observatory's normalized Neon schema is one rebuildable read
cache owned by Vela Web. ADR 0015's possible Erdős knowledge adapter is another
optional reader and is neither a dependency nor a destination for Hub code.

## Non-goals

This plan does not change an event, Receipt, proposal, signature, policy,
accepted-state rule, canonical frontier byte, or authority path. It does not
create a public API product, graph database, package registry, ontology engine,
or Vela Knowledge service. It does not move Web presentation or database logic
into the substrate.

## Preconditions

Before deletion:

1. Reconcile the exact released Vela tag and identify which Hub components
   shipped in it.
2. Rebuild the current Web projection from clean, exact frontier checkouts and
   verify its public roots and corpus counts.
3. Inventory live deployments, DNS, Fly applications, persistent volumes,
   databases, credentials, clients, packages, and release artifacts.
4. Classify every candidate as `protocol survival`, `Web reader`, `historical
   evidence`, or `unused`.

Absence from a local `rg` result is not proof that a released consumer does not
exist. Do not combine this retirement with an unrelated Vela release already
in flight.

## Task 1: Produce the consumer trace

Add `docs/reports/HUB_PROJECTION_CONSUMER_TRACE_2026-07-22.md` and trace:

```text
crates/vela-hub
any standalone Hub/indexer binary
analysis/atlas.rs
frontier_graph.rs
Hub and projection environment variables
Fly, Docker, and deployment files
docs/HUB.md
release, package, and conformance entries
parent, Web, Canopus, and frontier consumers
```

For every schema, public Rust item, CLI JSON field, fixture, workflow,
deployment, and external consumer, record exact path/commit, observed purpose,
release status, replacement owner, and delete/retain decision.

The report must answer four questions plainly:

1. Does any current client require the hosted Hub?
2. Does Hub retain unique state not present in canonical Git?
3. Which exact Vela read contracts does Web consume?
4. Would deleting a type alter replay, strict signals, current CLI output, or a
   released fixture?

## Task 2: Freeze the substrate survival contract

Add `docs/reports/DERIVED_ANALYSIS_SURVIVAL_CONTRACT.md` and focused fixtures
for behavior that must remain in Vela:

- parse and replay canonical frontier objects;
- derive deterministic standing and strict signals;
- verify exact roots;
- retain typed relations required by a current CLI contract or conformance
  fixture; and
- emit any released machine-readable field that a traced consumer still uses.

For each retained item, name input bytes, output schema/root, public API, test,
and consumer. UI labels, HTML, HTTP routing, database tables, graph layout,
search ranking, source scheduling, and hosted refresh logic are not protocol
survival behavior.

## Task 3: Remove unused indexing code first

If an indexer or source reader is unshipped and has no traced consumer, delete
it and record the source commit in the report. Do not migrate it merely because
it exists.

If Web consumes a small part, prefer a stable released Vela JSON surface or a
surviving library function. Move only the missing read adapter into Web's sole
projection package and prove parity for:

```text
Git commit and tree
event, snapshot, proposal, registry, and artifact roots
finding and proposal identities
strict signal codes and counts
relations required by the current Web projection
```

Do not copy a whole Hub binary, database layer, scheduler, or HTTP API under a
new name. Do not move domain vocabulary into Vela. Any experimental
standards-based export follows ADR 0015 and remains optional.

## Task 4: Retire the hosted Hub safely

Only after the trace proves there is no live dependency:

1. Export and hash unique persistent service state.
2. Prove every authoritative or scientifically relevant byte exists in a
   canonical frontier or a documented archive.
3. Disable writes before removing reads.
4. Publish a concise machine-readable sunset response naming canonical Git
   frontiers and the read-only Observatory.
5. Remove Fly applications, volumes, secrets, DNS, Docker/deployment files,
   and service documentation after the declared rollback window.
6. Preserve historical tags, release artifacts, and a note naming the last
   release and final source commit.

If unique state cannot be classified or archived, stop. Do not treat a service
database as canonical merely to complete cleanup.

## Task 5: Reduce the remaining analysis code

For every type or function under protocol graph/analysis modules, require all
four answers to be yes:

1. A current replay, verification, CLI, or fixture consumer needs it.
2. It reads only canonical retained inputs.
3. Its output is independent of UI, layout, database, and hosted-service
   concerns.
4. It does not infer scientific standing, identity, or authority beyond Vela's
   existing reducer and policy rules.

Delete unconsumed generic graph abstractions. Keep domain-specific reader
classification in Web or the optional ADR 0015 adapter, not Vela. Keep
workbench activity adapters in Canopus.

## Task 6: Remove the third projector

The private integration repository may invoke exact released Vela and Web
projection verification. It must not maintain copied frontier graphs, a Hub
wrapper, a parallel database, or another source-selection implementation.

Remove copied projection scripts only after their unique outputs and consumers
are inventoried. Preserve divergent signed history as an archive, never by
splicing it into a current frontier.

## Task 7: Verify the narrow waist

Run focused checks first:

```bash
cargo test -p vela-protocol frontier_graph
cargo test -p vela-protocol analysis
cargo test -p vela-cli --test product_09 compact_contract
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
cargo metadata --no-deps --format-version 1
git diff --check
```

Then run the ordinary core suite only if focused checks pass:

```bash
./scripts/full-conformance.sh --suite core --mode=ci
```

Run the frontier suite only when a surviving frontier-source contract changes.
Do not run external Lean, Diderot, live-network, or an unrelated full release
union for this retirement slice.

Inspect packaged crates and ordinary CLI help. No retired Hub command, hidden
HTTP server, database driver, deployment file, or stale environment variable
may remain in the ordinary product. Web projection checks must independently
prove exact source roots and clean rebuild.

## Release rule

Removing a crate or binary that appeared in a published Vela release is a
bounded pre-1.0 product-surface change and receives its own release notes,
version, focused compatibility checks, and parent pin. Unshipped, unconsumed
code may be removed without pretending it was a released migration.

Do not assign a version until the consumer trace establishes the actual
published surface. Do not append this cleanup to another release after that
release's candidate scope has frozen.

## Stop conditions

Stop on:

- changed canonical replay, strict-signal drift, or missing Web inputs;
- an unclassified external consumer;
- unique service state without a verified archive;
- a replacement hosted service, public API product, or second active
  projection implementation;
- domain ontology or Web presentation entering the protocol;
- pressure to accept ADR 0015 merely to provide a destination for old Hub code;
  or
- pressure to keep dead code solely because it once appeared in a plan.
