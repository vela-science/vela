# Vela benchmarks

This directory contains Vela-owned evaluation definitions that test a named
product or protocol claim. It is source, not scientific Standing and not a
runtime-data store.

The ownership boundary is strict:

- this directory tracks benchmark instructions, exact contracts, thin fixture
  compilers, deterministic Vela-specific checks, and compact frozen plans;
- [`erdos-264-proof-repair`](erdos-264-proof-repair/README.md) is the one
  Decision-gated native Lean episode; Harbor executes one matched pair and a
  separate verifier checks the exact scientific artifact;
- `jobs/` holds ignored Harbor jobs, trials, trajectories, generated fixtures,
  temporary binaries, container state, and debugging output;
- `paper/artifacts/` holds only compact, immutable result summaries and roots
  cited by the white paper;
- Frontier repositories hold their own domain evidence, Vela records, Targets,
  and Decision history. A benchmark may bind an exact Frontier commit or bundle
  but may not use a Frontier as its harness store;
- credentials remain in their native stores and are never copied into Git.

Do not create a separate benchmark repository until at least two independent
benchmark families, external task authors or consumers, or independently
versioned Harbor publication requires a separate lifecycle. A cookbook is a
different user-facing collection of recipes and is not a benchmark home.
