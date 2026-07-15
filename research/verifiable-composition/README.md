# Verifiable composition experiment

This directory contains the experiment-only implementation for ADR 0004. It
tests whether current Vela and Git objects can express an exact scientific
dependency before any public protocol object or command is proposed.

The current run class is `internal_fixture`. Unassigned independent roles,
project-authored code, fixture keys, and Codex subagents do not count as an
outside producer, independent reader, human decision, or ecosystem adoption.

Hard boundaries:

- no human key access, signing, acceptance, or automatic child-truth update;
- no live Hub, registry, or hosted-service dependency;
- no paid API or credential access;
- no public Vela wire object or crate-wide public type; and
- no benchmark or foundation claim from internal fixture results.

The frozen Phase 0 inputs live in `registration/`. Phase 1 reference code and
vectors must remain removable without changing Vela replay or authority.

The focused Phase 1 test uses a **shape-compatible synthetic aggregate**, not a
canonical current-schema frontier. Its selected VerifierAttachment is parsed by
the current Rust `vela gate check` path when the local binary is available; the
gate intentionally returns `needs_verification` (G1), not `verified`.

## Phase 1 exact-checkout candidate

`reference/composition.py` reads only regular-file bytes from one full,
already-local Git commit. It rejects branches, abbreviated object names,
symlinks or submodules selected as inputs, unknown paths, oversized objects, and
root mismatches. An
encoded observation is a **structural candidate**, never a verified dependency
or authority result.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 reference/composition.py encode \
  --repo /path/to/local/repo \
  --commit <full-40-or-64-hex-commit> \
  --selection selection.json

PYTHONDONTWRITEBYTECODE=1 python3 reference/composition.py resolve \
  --repo /path/to/local/repo \
  --observation observation.json \
  --frontier-path path/inside/repo \
  --premise-path inputs/exact-premise.json
```

The encode command wraps the schema object under `.observation` beside the
explicit `structural_candidate` status; pass that nested object, not the wrapper,
as `observation.json` to `resolve`.

The resolver deliberately does not invoke `vela check --strict`: that whole-
frontier command does not return the named decision's signature/authority
verdict or the recomputed normative snapshot root, so running it would add a
partial green signal without closing either blocker. The resolver currently
always rejects or returns `unresolvable:authority_snapshot_porcelain_missing`.
That result also carries `derived_view_not_canonical_state`: Phase 1 reads the
committed aggregate `frontier.json` but does not replay canonical `.vela` state,
so even self-consistent aggregate bytes are only candidate provenance.

Focused offline checks:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 check_phase0.py
PYTHONDONTWRITEBYTECODE=1 python3 check_observation_vectors.py
PYTHONDONTWRITEBYTECODE=1 python3 check_receipt_binding.py
PYTHONDONTWRITEBYTECODE=1 python3 check_phase1_resolver.py
```

`check_observation_vectors.py` requires Python `jsonschema` and invokes its
Draft 2020-12 metaschema validator. The focused experiment fails with a clear
message when that package is absent; the hosted core gate does not acquire or
depend on it.

See `current-object-gap-report.md` for exact root algorithms and the classified
porcelain, semantics, and representability gaps.

The base observation under `vectors/` is deliberately a shape-only placeholder:
its roots and signature have valid encodings but do not resolve to the graph
case, a Git object, a Vela event, or an authority decision. The binding check
proves only that Receipt v1 preserves and commits those bytes. Actual root
derivation, signature verification, and current-object resolution remain Phase
1 gates and must not be inferred from the parser vectors.
