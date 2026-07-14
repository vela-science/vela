# Interoperability and the narrow waist

Status: released contracts and testing projections, 2026-07-14.

Vela interoperates by preserving a small public boundary, not by making every
scientific tool adopt Vela's internal model. A producer can keep its own Git
repository, language, verifier, notebook, graph, or publication system. It
crosses the boundary with ordinary immutable references and a complete Receipt
v1. A Vela frontier then records the resulting proposal and any later
authority-bearing event without giving the producer authority to decide it.

## Contract classes

| Class | Contracts | Compatibility promise |
| --- | --- | --- |
| Released | Receipt v1 JSON and canonicalization, canonical event JSON and replay, content-addressed artifacts, CLI JSON documented as released | Versioned, bounded, conformance-tested. Unknown namespaced Receipt v1 extensions survive import and export. |
| Testing | Decision Brief, `next` task contract, packet decision view | Useful and covered by fixtures, but may change before evidence from two independent producers and two independent consumers. |
| Internal | Work sessions, transaction journals, adapter result JSON, caches, Rust modules | Replaceable implementation detail. Producers must not author or depend on it. |

The packet file `decisions/decision-view.json` is a derived offline view. It
embeds the exact signed decision event or policy certificate and points back to
`events/events.json`. The canonical event remains authority. Older events that
predate decision-root binding report `unavailable_legacy_event`; export does not
invent a root or rewrite history.

## Producer contract

The supported adapter shape is:

```text
pinned inputs -> sandboxed verifier -> bounded adapter-private result
-> private ReceiptBuilder -> shared land service
```

An independently shipped producer skips the adapter-private result and emits a
complete Receipt v1. Vela intentionally does not define a second generic
verifier-result schema. An adapter may not mutate accepted events, proposals,
publication, policy, or work-session state directly.

The default conformance fixture adds an unknown
`x:foreign-receipt-conformance` object to a current-valid receipt, lands it
pending through the real service, reads the durable exported receipt, and
re-imports it into a second clean frontier. Both imports must preserve the same
canonical receipt root and every unknown extension byte.

```bash
python3 scripts/cross_impl_conformance.py
```

## Portable transport and offline use

Vela uses standard Git repositories and bundles for transport. It does not
define a Vela archive format. A recipient can verify a bundle, clone without a
network, replay the event log, rebuild derived review material, inspect opaque
restricted references, and continue from the accepted parent root.

```bash
git bundle verify frontier.bundle
git clone frontier.bundle offline-frontier
vela frontier materialize offline-frontier
vela reproduce offline-frontier
```

The tested procedure, including incremental bundles and failure on missing
prerequisites, is in [EXIT_AND_EXPORT_DRILL.md](EXIT_AND_EXPORT_DRILL.md).

## Existing adapters

- External Lean is an installed Vela command. It pins source and toolchain,
  executes in a fail-closed sandbox, builds Receipt v1 privately, and uses the
  shared landing service.
- Diderot certificates remain attributed external evidence. Issuer,
  certificate type, disclosure, dates, faithfulness scope, caveats, and known
  reviewer labor are preserved; a certificate is never mapped directly to
  Vela acceptance.
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
