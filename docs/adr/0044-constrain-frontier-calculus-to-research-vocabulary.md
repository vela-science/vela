# ADR 0044: Constrain Frontier Calculus to research vocabulary

- Status: Accepted, 2026-08-11
- Supersedes: ADR 0017 for current layer vocabulary and implementation
  disposition
- Protocol, schema, JSON, command-surface, and authority effect: None
- Documentation effect: current research vocabulary only

## Context

ADR 0017 deferred Frontier Algebra and Discovery Calculus as research-only
layers. Neither gained an implementation or maintained consumer. ADR 0039
later made Frontier an identifier-free derived query, while the current
architecture uses three conceptual graphs, four clocks, and concrete read
mechanisms. Keeping the old layer names in controlled terminology implies a
product surface that does not exist.

The warning in ADR 0017 remains useful: representation shape does not
establish support, path multiplicity does not establish independent evidence,
and a deterministic score is not scientific confidence.

## Decision

Retire `Frontier Algebra`, `Discovery Calculus`, and `Lens` from current Vela
vocabulary. Do not rebase them under new names. Retain `Frontier Calculus` only
as a research-program label for formalizing support, provenance, correction,
transfer and obligations. It is not a Vela layer, product surface, kernel
dependency, protocol object or reserved extension point.

Historical ADR bodies remain historical evidence. Shipped mechanisms retain
concrete names and contracts. Source-local experiments may describe their own
models without creating a Vela layer.

A future shared analysis layer requires a new decision backed by two
maintained consumers, exact independent agreement, and net deletion of
duplicated implementation.

## Consequences

No canonical bytes, schemas, JSON fields, parser command, Decision rule, Event,
Standing, or stored Web projection bytes change. Any downstream legacy
display or read-model field remains owned by that reader and must not be
interpreted as a Vela object.
