# Vela glossary

Vela is the open protocol for replayable, authority-scoped, correction-aware
scientific state transitions. This glossary is an informative map of current
Vela terminology. Protocol 1, its normative schemas, and the conformance
vectors it marks normative remain authoritative when wording differs.

**Protocol 1.** The normative interoperability selection for canonical
objects, roots, signatures, replay, and authority-scoped state transitions.

**Repository.** One ordinary Git repository for a bounded scientific scope and
the local authority boundary. A Repository retains canonical records,
authority history, and the inputs from which Standing is replayed. It is not a
global truth ledger.

**Repository authority.** The service identity and retained authorization
model that admit an exact Repository write. Repository authority records an
authenticated and authorized write; it does not supply scientific judgment.

**Submission.** Authenticated producer input proposing one bounded change. A
Submission grants no verification or Decision authority and changes no
Standing.

**Verification Record.** One scoped verifier observation over exact retained
inputs. A Verification Record may pass or fail, but it never accepts a Claim.

**Decision.** An attributed accept or reject action admitted through
Repository authority. Only an authorized Decision admits the Event that can
change Standing.

**Event.** The canonical admitted transition retained in Repository history.

**Standing.** The deterministic result of replaying valid admitted Events. It
is derived from canonical Repository state rather than asserted by a website,
index, check, signature, or Git merge.

**Projection.** A root-bound, rebuildable read model over exact retained state.
It carries no authority and cannot change Standing.

**Frontier.** A derived query over current Standing in one or more
Repositories, usually selecting unresolved or actionable items. A Frontier has
no persistent governed identity, owns no records or authority, and is not a
Repository.

**External activity and control.** Controllers, agents, attempts, runs,
campaigns, schedulers, workflows, notebooks, and workbenches remain owned by
their source systems. They may produce evidence or a Submission; Vela Core
does not define universal objects for them or infer authority from them.

**Speculative research.** Papers, benchmarks, exploratory reducers, and
unevaluated product hypotheses. They may motivate experiments but do not alter
Protocol 1, Repository authority, or Standing.
