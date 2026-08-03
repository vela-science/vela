# ADR 0039: Multi-case human Result Dossier qualification

- Status: Accepted
- Accepted: 2026-08-03
- Protocol effect: none
- Authority effect: none
- Product effect: replaces repeated same-model timing with real-reviewer proof

## Context

The Erdős 264 Dossier is exact but not publicly qualified. Four model sessions
per arm were instrumentation, not independent reviewers, and three iterations
on one case cannot prove reuse. A fourth text compression pass would increase
researcher degrees of freedom without establishing external usability.

## Decision

Keep the current declaration removable and inactive. Generalize only after a
second completed case exposes shared machinery. Use Erdős 730 as case two and
Astra/Erdős 183 as case three if their scientific packets reach explicit human
Decision or documented deferral.

Before recruiting anyone, freeze:

- the cases, exact Frontier commits, repository roots, and release candidate;
- an information-matched flat presentation for every case;
- eight common recovery questions plus case-specific source-fidelity fields;
- a case-blocked randomization and sample-size/sensitivity calculation;
- timing start/stop rules, exclusions, scoring, and missing-data treatment;
- the complete authority-error, caveat, dependency, and nonclaim rubric; and
- a positive, neutral, and negative reporting template.

The minimum release gate remains:

- zero authority or Standing errors in the Dossier arm;
- exact recovery of all common fields;
- no omitted failure, caveat, source discrepancy, or shared dependency;
- at least 20% lower preregistered median time than the matched flat arm;
- no case with a directionally adverse material error rate; and
- deterministic HTML, JSON, and print output from one release root.

Public messaging may describe an exact, correction-aware case record only
after the gate passes. Reviewer-efficiency evidence must name the design,
sample, cases, and uncertainty. Adoption, general productivity, and causal
Vela lift remain nonclaims.

## Rejected alternatives

- **Run more Erdős 264 model sessions.** Rejected as repeated instrumentation
  on one episode.
- **Lower the gate to 19%.** Rejected because the threshold was frozen before
  observation.
- **Release because content was perfect.** Rejected because the registered
  product claim included time.
- **Add a Dossier protocol object.** Rejected because the projection has no
  scientific authority and remains removable.
