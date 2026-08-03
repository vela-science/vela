# Protocol adoption and interoperability boundary

Vela owns governed scientific-state transition. It does not replace native
execution systems, and interoperability does not transfer scientific
authority.

## Ownership matrix

| Layer | Owner | Current Vela boundary | Authority effect |
|---|---|---|---|
| Proof, computation, data, models | Native tools and repositories | Exact references, artifacts, and replay contracts | None |
| Encoding and identity | JCS, SHA-256, Ed25519; DSSE for authority records | Pinned implementations and conformance vectors | None |
| Portable structure | JSON Schema 2020-12 | Checked descriptions of current producer/verifier objects | None |
| Scientific state | Vela protocol | Claim lineage, scoped Verification, Decision, correction, Standing | Only an authorized Frontier Decision changes Standing |
| Admission policy | Each Frontier | Closed local authority profile and exact human checkpoint | Frontier-local |
| Orientation | Observatory and other read projections | Root-bound, reconstructable, SELECT-only views | None |

## Current, experimental, and absent surfaces

| Surface | State | Honest contract |
|---|---|---|
| CLI JSON reads | Current | Versioned machine views; not a universal protocol API |
| Submission, Verification Record, Withdrawal JSON | Current | Closed signed v1 objects; Rust semantic validation is authoritative |
| JSON Schema descriptions | Current documentation | Structural validation only; no signature, reference, Decision, or Standing inference |
| Authority DSSE envelope | Current | Repository-authority transport only |
| Result Dossier | Experimental read projection | Exact rooted case record; no Vela object and no authority |
| Common DSSE producer/verifier v2 | Planned | Requires a separate ADR 0035 cut and retained migration evidence |
| AuthZEN-shaped closed evaluator | Shadow | Not the writer; historical recomputation is incomplete |
| MCP or A2A edge | Absent | No support claim, server, tool, resource, or write contract |
| Package manager or Registry | Absent | Source-local experiments only after a second real consumer |
| Hosted writer or automatic Decision | Prohibited by current boundary | Requires a later explicit authority and threat-model decision |

## Non-amplification invariants

- A schema-valid object is not scientifically valid or accepted.
- A signature proves control of a key over exact bytes; it does not prove
  truth, independence, identity uniqueness, or review authority.
- A passing Verification establishes only its declared property and never
  changes Standing by itself.
- A package, adapter, tool call, read projection, model vote, or imported graph
  has `authority_effect = none`.
- Assurance labels are source-local presentation vocabulary. They do not
  overload protocol Standing.
- Only an attributed, authorized human Decision admitted by one Frontier
  changes that Frontier's Standing.

## Adoption test

Before adding an interoperability edge, record:

1. the concrete consumer and user task;
2. the exact current object or read projection being exposed;
3. preserved meaning, information loss, shared dependencies, and nonclaims;
4. the fixture and release roots;
5. the authority boundary and credential exclusions;
6. deterministic offline reconstruction; and
7. the condition under which the edge will be removed.

Prefer the existing CLI JSON or static HTTP read surface when it serves the
task. Promote an edge only after measured use or two maintained consumers
justify its maintenance cost. See [ADR 0037](adr/0037-removable-protocol-edges-and-adoption-order.md).
