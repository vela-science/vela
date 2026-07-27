# Interoperability and the narrow waist

Status: current candidate boundary through Profile v1, 2026-07-26.

Vela interoperates by preserving a small public boundary, not by making every
scientific tool adopt Vela's internal model. A producer can keep its own Git
repository, language, verifier, notebook, graph, or publication system. It
crosses the boundary with ordinary immutable references and an authenticated
Submission v1. A Vela Frontier then issues a Registration Record, records the
resulting Proposal and Verification Records, and retains any later
authority-bearing event without giving the producer authority to decide it.

## Contract classes

| Class | Contracts | Prelaunch stability intent |
| --- | --- | --- |
| Current candidate | Submission v1, Registration Record v1, Verification Record v1, canonical Event JSON and replay, content-addressed Artifacts, Profile v1 identity/boundary contracts, Scientific State Root v2, and documented CLI JSON | Versioned, bounded, and conformance-tested before release. |
| Historical replay | Receipt v1 JSON and canonicalization, landing records, and policy-era events | Read-only compatibility. Unknown namespaced Receipt v1 extensions survive parsing and canonical re-emission; no current writer emits them. |
| Derived work | Target Index v2 candidate/seal, `vela.offer.v1`, and retained `vela.target-task-binding.v1` | Exact and fail-closed for Profile v1 work, but non-authoritative and replaceable by another domain candidate generator. |
| Experimental | Review Packet presentation and packet decision view | Useful and covered by fixtures, but may change before evidence from independent producers and consumers. |
| Internal | Private Attempt scratch, transaction journals, adapter result JSON, caches, Rust modules | Replaceable implementation detail. Producers must not author or depend on it. |

The packet file `decisions/decision-view.json` is a derived offline view. It
embeds the exact signed decision event or policy certificate and points back to
`events/events.json`. The canonical event remains authority. When binding
evidence is unavailable, export reports that absence instead of inventing a
root or rewriting history.

## Producer contract

The supported adapter shape is:

```text
pinned inputs -> sandboxed verifier -> bounded adapter-private result
-> authenticated SubmissionBuilder -> shared registration service
```

An independently shipped producer skips the adapter-private result and emits a
complete Submission v1. A verifier emits a separate, authenticated Verification
Record. An adapter may not mutate accepted Events, Proposals, Decisions,
publication, policy, or Attempt state directly.

Historical Receipt extension preservation remains pinned by:

```bash
cargo test -p vela-protocol rich_unknown_extensions_round_trip_losslessly_and_bind_root
```

## Portable transport and offline use

Vela uses standard Git repositories and bundles for transport. It does not
define a Vela archive format. A recipient can verify a bundle, clone without a
network, replay the event log, rebuild derived review material, inspect opaque
restricted references, and continue from the accepted parent root.

```bash
git bundle verify frontier.bundle
git clone frontier.bundle offline-frontier
vela check offline-frontier --strict --json
vela frontier materialize offline-frontier
vela reproduce offline-frontier
```

A Profile v1 bundle must include the complete Git anchor history. Each
consumer separately installs the independently reviewed first-administrator
boundary pin; repository bytes cannot create that trust decision for the
recipient. A shallow bundle that omits the anchor is unavailable, not valid by
assertion.

The manual portability checklist, including incremental bundles and failure on
missing prerequisites, is in
[EXIT_AND_EXPORT_DRILL.md](EXIT_AND_EXPORT_DRILL.md).

## Boundary examples

- External Lean is an optional producer adapter. It pins source and toolchain,
  executes in a fail-closed sandbox, and emits an authenticated Submission
  whose producer check remains distinct from an independent Verification
  Record.
- Diderot is a very early exploratory project, not a Vela partner, robust
  external producer, compatibility target, or architectural validation. A
  future experiment may carry one of its certificates as attributed external
  evidence through Submission v1, preserving issuer, scope, caveats, and
  reviewer labor. Diderot-specific formats and availability are not Vela
  conformance or release dependencies, and a certificate is never mapped
  directly to Vela acceptance.
- RO-Crate and SWHID are export and immutable-locator conventions. They do not
  replace Vela event authority.
- OpenResearch, OCI, CloudEvents, Hugging Face cards, graph tools, and wiki
  tools remain adapters or documented sketches until a real consumer requires
  a stable mapping.

## Derived consumers

Graphs, wikis, search indexes, embeddings, and SQLite caches are disposable
projections. They must cite their Git and event roots, label deterministic and
inferred relations differently, expose stale state, and be deletable without
changing replay or decision roots. No derived consumer gets a new kernel kind,
custom Git ref, hosted-only API, or authority role merely for convenience.

## What conformance proves

Conformance proves deterministic parsing, canonical bytes, bounded inputs,
replay agreement, extension preservation, and the stated offline workflow. It
does not prove scientific truth, semantic faithfulness, producer independence,
institutional neutrality, or human acceptance. Those claims require their own
evidence and authority.
