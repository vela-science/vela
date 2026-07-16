# Verifiable composition experiment

This directory contains the bounded implementation and evidence for ADR 0004.
It tests whether current Vela and Git objects can express an exact scientific
dependency before any truth-bearing dependency object or command is proposed.

The current run class is `internal_fixture`. Unassigned independent roles,
project-authored code, fixture keys, and Codex subagents do not count as an
outside producer, independent reader, human decision, or ecosystem adoption.

Hard boundaries:

- no human key access, signing, acceptance, or automatic child-truth update;
- no live Hub, registry, or hosted-service dependency;
- no paid API or credential access;
- no truth-bearing dependency wire object, status reducer, or automatic graph;
- no benchmark or foundation claim from internal fixture results.

## Phase 1B: one derived fact manifest, two removable profiles

The current Phase 1B candidate keeps one exact, canonical fact manifest and
derives three read-only Vela consumers from it:

- a dependency-standing resolver;
- a correction-aware CI projection; and
- an accepted-state context pack.

An independently written Reader C agrees on status and roots. A separate
Git/DSSE/in-toto/`science.lock` wrapper carries the same manifest and is
explicitly allowed to win. Neither representation is accepted state.

The standing model deliberately permits only one later truth-relevant change
event per delivered manifest. A correction, decision revocation, and verifier
revocation may all occur over time, but combined changes must arrive as
separate exact manifests. This removes precedence ambiguity instead of adding a
more complicated multi-event reducer.

Focused checks:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 check_fact_manifest_projections.py
PYTHONDONTWRITEBYTECODE=1 python3 check_offline_bundle_inspection.py
PYTHONDONTWRITEBYTECODE=1 python3 check_projection_deletion.py
PYTHONDONTWRITEBYTECODE=1 python3 generate_standards_baseline.py
PYTHONDONTWRITEBYTECODE=1 python3 check_standards_baseline.py
```

These currently cover 54 hostile fact-manifest cases, seven CLI cases across
three consumers, independent Reader C parity, offline same/descendant/stale/fork
delivery, projection deletion, and 13 standards-wrapper vectors. They are
internal engineering evidence only.

The frozen Phase 0 inputs and v0.800.12 outcome remain unchanged. Phase 1A is a
separately registered v0.800.13 experiment. Its current custody fixture is the
exact committed `examples/erdos-formalization` Git tree: accepted state comes
from its split `.vela` directory, produced through ordinary Vela paths, rather
than a hand-authored aggregate. The small decision-inspection fixture uses only
the explicitly published fixed test-key seed and ordinary protocol proposal,
DecisionPlan binding, and signing helpers. It is not a human decision.

The old `check_phase1_resolver.py` synthetic-aggregate probe is retained only as
the frozen negative baseline that exposed the gap. It is not the current Phase
1A custody test and is not evidence about canonical replay.

## Phase 1 exact-checkout candidate

`reference/exact_checkout.py` reads only regular-file bytes from one full,
already-local Git commit. It rejects branches, abbreviated object names,
symlinks or submodules, path escape, unknown paths, oversized objects, and root
mismatches. Phase 1A materializes only the registered frontier subtree into an
isolated temporary directory, records the runner digest, and invokes the frozen
release offline:

```bash
vela check . --strict --json
vela proof verify . --json
```

Canonical custody verifies only when Git identity, replay, proof, visible-view,
and lock roots all agree. Mutating only `frontier.json`, mutually fabricating
derived views, or changing/deleting/duplicating canonical `.vela` material
fails closed.

The encoder remains available for the earlier structural observation probe:

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

Canonical custody is intentionally not a named-decision verdict. The current
unreleased working tree exposes a pure read-only `inspect_named_decision` seam
that accepts one complete event ID, one full event-content root, and optional
canonical DecisionPlan preimage bytes. It rederives the event ID/root, requires
exactly one historically authorized registered reviewer or steward, verifies
the Ed25519 signature, checks proposal/applied-event linkage, and reconstructs
the base event-log, proposal, authority, and semantic-event commitments. It has
no path, key, socket, clock, registry, or write parameter.

Released `v0.800.13` decisions retain only the DecisionPlan root. When the
preimage is absent, the exact result is
`unresolvable:decision_preimage_unavailable`; no frontier-wide green result is
used as a shortcut. `verified:decision_evidence_bound` means only that this
named human decision and its retained evidence agree. It is not a dependency or
scientific-truth verdict.

The `v0.800.14` working-tree target retains the exact canonical seven-field
preimage for new decisions at
`records/decision-evidence/decision-root/<decision-root-hex>.json`. The
decision transaction installs those bytes as `CanonicalEvidence` alongside its
authority writes in one `FrontierTxn`. The path key is the same domain-separated
root carried by the signed event. The evidence file creates no event and
supplies no authority.
Deleting it leaves reducer replay, the event-log root, and signatures unchanged;
a caller with no other preimage copy then receives the root-only unresolvable
result.

The pure inspector still receives preimage bytes from its caller. A public
reader that discovers this conventional path, batch inspection, independent
handoff, and any dependency-status projection remain open work. Existing
`v0.800.13` decisions do not gain bytes retroactively.

Focused offline checks:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 check_phase0.py
PYTHONDONTWRITEBYTECODE=1 python3 check_observation_vectors.py
PYTHONDONTWRITEBYTECODE=1 python3 check_receipt_binding.py
REGISTERED_VELA_SOURCE="$(mktemp -d)"
git archive b3076f8935a38ecaef252e7f062648794cc7cd07 | \
  tar -x -C "$REGISTERED_VELA_SOURCE"
cargo build --offline --locked \
  --manifest-path "$REGISTERED_VELA_SOURCE/Cargo.toml" \
  --target-dir "$REGISTERED_VELA_SOURCE/target" \
  -p vela-cli --bin vela
PYTHONDONTWRITEBYTECODE=1 python3 check_phase1_canonical.py --self-test \
  --vela "$REGISTERED_VELA_SOURCE/target/debug/vela"
cargo test --locked -p vela-protocol decision_inspection -- --nocapture
cargo test --locked -p vela-cli decision_plan -- --nocapture
cargo clippy --locked -p vela-protocol --lib -- -D warnings
```

The Phase 1A checker requires an explicit runner built from registered commit
`b3076f8935a38ecaef252e7f062648794cc7cd07`. It rejects any executable whose
reported version is not exactly `vela 0.800.13`; the mutable current workspace
binary is never an implicit experiment input. `--offline` keeps the rebuild
network-free and therefore requires the locked Cargo dependencies in the local
cache.

The Phase 1A checker validates preregistration, including negative tests for an
unregistered metric, vector, or arm. It then runs all registered vectors. The
Rust test reads the same decision-vector file, so every classification is
pinned across the two implementations. Results and the minimal promotion
decision live under `results/`; no dependency object, status reducer, graph,
wiki, or cache is promoted.

The Phase 1A result remains pinned to released `v0.800.13`. The separate
`results/retained-decision-evidence-implementation-2026-07-15.json` record
names the unreleased `v0.800.14` target and the focused transaction checks; it
does not rewrite the registered experiment.

`check_observation_vectors.py` requires Python `jsonschema` and invokes its
Draft 2020-12 metaschema validator. The focused experiment fails with a clear
message when that package is absent; the hosted core gate does not acquire or
depend on it.

See `current-object-gap-report.md` for exact root algorithms and the classified
porcelain, semantics, and representability gaps.

The original base observation under `vectors/` remains a shape-only placeholder:
its roots and signature have valid encodings but do not resolve to the graph
case, a Git object, a Vela event, or an authority decision. The binding check
proves only that Receipt v1 preserves and commits those bytes. Root derivation,
signature verification, and current-object resolution must not be inferred
from the parser vectors; the separate registered Phase 1A internal fixture
establishes only the bounded checks described above.
