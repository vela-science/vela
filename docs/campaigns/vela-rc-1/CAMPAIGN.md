# VELA-RC-1 campaign charter

Recorded: 2026-08-26, America/Toronto.

## Objective

Determine whether the currently qualified Vela Protocol 1 implementation is
ready to become credible external infrastructure that a technically capable
third party can install, understand, use, inspect, and trust without
campaign-specific knowledge.

The terminal verdict is exactly one of:

- `RELEASE READY`;
- `RELEASE READY WITH EXPLICIT LIMITATIONS`;
- `HOLD — FIXABLE RELEASE BLOCKERS`; or
- `DO NOT RELEASE CURRENT CANDIDATE`.

Even a ready verdict means `READY FOR USER AUTHORIZATION`; it is not authority
to release.

## Frozen scope

The candidate is the exact tree recorded in [BASELINE.md](BASELINE.md). The
foundational top-down search remains `CLOSED`. This campaign must not reopen
CUT, Inheritance, ACQUIRE, VELA-COMPOSE, cumulative handoff, or biomedical
research. It must not expand Protocol 1 merely because release qualification
exposes friction.

Public claims are limited to the demonstrated boundary: Vela is a protocol and
toolchain for governed, replayable scientific-state transitions. It records
what was proposed, what checked it, what authority decided, what changed, and
how current Standing can be reconstructed. It does not establish scientific
truth, cumulative-intelligence advantage, autonomous discovery, physical
reruns, or improved agent performance.

## Gates

| Gate | Requirement | Initial state |
| --- | --- | --- |
| G1 | Semantic integrity | `IN PROGRESS` |
| G2 | Reproducible installation and conformance | `IN PROGRESS` |
| G3 | Legibility and first-user usability | `BLOCKED ON G1/G2` |
| G4 | External workflow independence and two examples | `BLOCKED ON G1/G2` |
| G5 | Product and release integrity | `BLOCKED ON G3/G4` |
| R7 | Blind external-user simulation | `BLOCKED ON R1-R4 PASS` |

The release blockers B1-B10 from the campaign instruction are binding.
Semantic ambiguity, replay failure, hidden campaign dependency, authority
ambiguity, fail-open artifact handling, documentation divergence, clean-install
failure, a domain-specific example fork, semantic UI/CLI misrepresentation, or
untraceable release bytes requires `HOLD`.

## Lane order

S0 owns the candidate, gates, decisions, integration, and final verdict. R1
audits protocol semantics and the conformance matrix. R2 performs clean-install
and replay qualification. Only after R1 and R2 pass may R3 and R4 change or add
release-facing material. R5 and R6 follow. R7 runs only after R1-R4 pass.

Workers may report blockers but may not tag, publish, push, bump versions,
contact external parties, or broaden the scientific or protocol scope.
