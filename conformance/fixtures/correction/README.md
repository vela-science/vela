# Correction-impact conformance

This directory contains a language-neutral, non-authoritative experiment. It
is not a Vela protocol schema and does not change Standing.

## Closed input

`vela.correction-impact-input.v1` binds:

- one exact predecessor and successor Claim;
- a bounded, explicitly complete Claim set;
- a bounded, explicitly complete relation set;
- full Claim and relation roots;
- one closed relation rule for each relation kind; and
- a repair discharge condition for any Claim that can require repair.

The v1 experiment recognizes three meanings:

| Relation | Direction | Consequence |
| --- | --- | --- |
| `depends_on` | source requires target | target loss requires source repair |
| `supports` | target is one support route for source | retain every independent surviving route; repair only when no route survives |
| `discovery` | source was discovered through target | no scientific consequence |

Other meanings fail closed. A reader cannot reinterpret `discovery` as a
dependency or collapse all relations into a generic edge.

## Deterministic projection

Given predecessor `A`:

1. mark `A` unavailable as a support/dependency input while retaining its
   identity;
2. repeatedly propagate hard dependencies in lexical Claim order until both
   classifications and causal relation sets reach a fixed point;
3. group support routes by source Claim;
4. classify a source as `route_changed` when a route through unavailable state
   is lost and at least one independent route survives;
5. classify it as `repair_required` when no support route survives or a hard
   dependency is affected;
6. emit one root-bound repair Obligation for each `repair_required` Claim; and
7. sort Claims and routes by full identifier before canonical encoding.

If either completeness flag is false, readers emit an `incomplete` projection
with explicit diagnostics and no affected or unaffected set. Unknown
relations, missing endpoints, stale or shortened roots, duplicate identities,
semantic rule substitution, and exceeded bounds are errors.

## Frozen synthetic vector

- canonical input root:
  `sha256:68a5094a5a98d60ab1d34c11c5306a202ea44d126f6dc95f33e20d31b5b1f8da`;
- expected projection root:
  `sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6`;
- Rust reader: `vela_edge::correction_impact`;
- clean-room reader: `conformance/verify_correction_impact.py`.

Run both:

```bash
cargo test -p vela-edge --test correction_impact
python3 conformance/verify_correction_impact.py
```

The diamond is synthetic. Agreement establishes implementation readiness,
not a real scientific correction, independent organizational reproduction,
user value, federation, or a protocol breakthrough.
