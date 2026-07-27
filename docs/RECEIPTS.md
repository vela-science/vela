# Receipts - the boundary where activity becomes admissible

> **Historical interoperability contract.** Receipt v1 and `vela land` are
> replay-only in the current product era. Current producers create
> `vela.submission.v1` with `vela submit`; Vela returns a Registration Record
> and a pending Proposal. The word Receipt is reserved for a future
> Vela-issued inclusion proof. This document preserves the exact historical
> schema and its replay semantics; it is not current producer guidance.

A **receipt** is how external activity enters Vela. It is not a transcript and
not a verdict. It is a content-addressed, claim-aware evidence packet that
`vela land` turns into a pending proposal, which a signed policy or a human then
routes into frontier state.

```text
source activity -> Receipt v1 bytes (sha256:<64>)
                -> durable ActivityRecord (vrc_) + proposal
                -> policy route -> Deny | Defer | signed-policy Permit
                -> one recoverable frontier transaction -> derived state
```

The raw Receipt v1 root and the `vrc_` identifier are deliberately different.
The former is the full canonical Receipt byte root and names
`records/receipts/sha256/<digest>.json`; the latter identifies the durable
landing record that points at those bytes. Consumers must not substitute one
identity for the other.

Everything before the receipt is production of evidence (an agent run, a
workflow, an eval, a proof). Everything after it is coordination over accepted,
deferred, or contested scientific state. The receipt is the one control point in
between, and the only input the landing path reads.

A receipt does **not** imply verification. It can carry evidence and state
verification requirements, but it never authors an attachment or sets gate status. That stays with the
verifier attachments (`vva_`) and the human key.

## The shape: `vela.receipt.v1`

The schema is versioned at
[`docs/schemas/vela.receipt.v1.schema.json`](schemas/vela.receipt.v1.schema.json).
External producers should validate against that file before handing a receipt to
`vela land`. The dependency-free production core and its command harness live
in [`crates/vela-cli/resources`](../crates/vela-cli/resources) and are embedded
byte-for-byte in the installed CLI. Copy `receipt_v1.py`,
`vela_receipt_v1.py`, and `receipt_json.py` together for a cold-start emitter;
there is no checkout-only second implementation.

The schema in this prelaunch `0.800` candidate is canonical. There is no
second frozen pre-ADR schema copy or promise to preserve an unpublished
validity set. Parsers reject duplicate object names before schema validation,
including escaped names and names in the decoded DSSE payload.

```jsonc
{
  "schema": "vela.receipt.v1",
  "claim": "the specific finding this run supports",   // required, non-empty
  "type": "computational",                              // computational|theoretical|…
  "replayability": "exact",                             // exact|bounded|approximate|unavailable|unknown
  "artifacts": [{"path": "witnesses/w.json", "kind": "witness"}],
  "caveats": ["what this does NOT establish"],          // required, non-empty
  "verifier_runs": [{"method": "…", "outcome": "pass", "log": "…", "solver": "…"}],
  "conditions": ["assumptions or scope limits"],
  "verification_requirements": ["what Vela must re-run before acceptance"],
  "state_diff": {"effect": "pending proposal only"},
  "environment": { /* extension points - see below */ },
  "provenance":  { /* generated_by, submitter, emitted_at */ },
  "status": {"kind": "emitted", "authority": "producer"}
}
```

- **claim** - the exact assertion. Retry identity is the client operation ID
  plus the normalized Receipt root. Reusing the same operation ID with changed
  input is an error. The same claim and type with different evidence is a new
  retained receipt, related to the earlier finding rather than erased as a
  duplicate.
- **type** - classifies the finding itself (`computational`, `theoretical`,
  `negative`, …), and drives the policy `claim_class` (`receipt_<type>`).
- **caveats** - the honesty requirement. An empty list is rejected. State what
  the evidence does not establish so the receipt cannot over-claim.
- **artifacts** - public local bytes are copied to a content-addressed canonical
  path. A remote public artifact needs an immutable locator, digest, and size.
  Restricted material is represented by a `custodian:` or `opaque:` reference
  and must not publish an equality-revealing digest or payload bytes.
- **verifier_runs** - mechanical provenance reported by the producer. A
  reported `pass` remains A0 and cannot raise durable assurance. Only separate
  retained verifier attachments, evaluated by the gate, can do that.
- **replayability** - see below.
- **conditions / verification_requirements / state_diff** - typed handoff
  fields. They state what assumptions travel with the receipt,
  what verifier or review work is still required, and what state change the
  producer believes the proposal would make. Landing records them as provenance;
  acceptance still requires the signed policy or a human key.
- **status** - the producer may only emit `draft` or `emitted` status with
  `authority: producer`. `landed_pending`, `accepted`, `rejected`, and
  `superseded` are Vela-side or human-key statuses and must not be forged by an
  emitter.

A producer-reported verifier run remains `producer_reported`. The producer
receipt uses `acceptance_scope: hypothesis_only`, leaves artifact and claim
assessment `not_assessed`, and names no acceptor. A later verifier or policy
decision can add authority through its own signed object.

## Whole-receipt binding

Emitters compute canonical JSON for the complete top-level receipt without
`attestation`. They place its lowercase SHA-256 digest at
`attestation.statement.predicate["vela:receipt_body"].sha256`. Consumers check
that root and compare the statement's subject, machine, acceptance,
distillation, lineage, contributors, signature identities, and provenance with
the receipt body. A missing, malformed, or stale binding fails validation.

## Exact execution binding

AcceptancePolicy v0.2 and v0.3 can narrow Permit to one frozen producer
contract using this optional Receipt v1 environment extension:

```json
{
  "vela:execution_binding": {
    "schema": "vela.execution-binding.v1",
    "packet_root": "sha256:<64 lowercase hex>",
    "profile_root": "sha256:<64 lowercase hex>",
    "verifier_capsule_root": "sha256:<64 lowercase hex>",
    "result_contract_root": "sha256:<64 lowercase hex>"
  }
}
```

The shape is closed and whole-body-bound. It names no mutable tag, verifier
service, target alias, authority, or verdict. A missing field, extra field,
short digest, uppercase digest, altered Receipt, wrong packet/profile/capsule,
or wrong result contract cannot satisfy an exact v0.2/v0.3 Permit rule. Policy
v0.1 ignores the extension for routing, preserving historical replay.

Policy v0.3 additionally commits to the full SHA-256 root of the Receipt's
self-signed `vela.identity_binding.v0.1`, canonically encoded with
`binding_id` and `signature` cleared. The readable `vib_` prefix is not an
authorization digest. The protected policy path must resolve every allowed
root back to complete retained Receipt bytes before it can show a human card.
Self-signing alone grants nothing, and actor-registry membership does not
bypass the v0.3 list.

A v0.2 or v0.3 exact positive lane may also rederive A2 from one retained public
artifact whose kind is exactly `vela-witness`, whose byte digest matches the
Receipt, and whose Vela-native verifier and claim-fidelity checks both pass.
This is not a producer verdict or a second Receipt extension. The retained
artifact bytes are re-read during strict replay; missing files, symlinks,
digest drift, duplicate `vela-witness` descriptors, invalid constructions,
wrong dimensions, inflated bounds, and equality/optimality claims fail closed.
Policy v0.1 never receives this floor.

The native flag-authoring path accepts the same four fields as
`--packet-root`, `--profile-root`, `--verifier-capsule-root`, and
`--result-contract-root`. They are required together, cannot accompany an
imported Receipt file, and enter the operation preimage before Vela builds the
whole-Receipt binding. External producers may emit the closed extension in
their own complete Receipt v1 instead.

## Landing is one write edge

CLI flags, file import, MCP, and adapters converge on the same strict Receipt
v1 parser and landing service. The service prepares the Receipt bytes, safe
artifact projection, durable landing record, proposal, exact `PolicyContext`,
gate result, policy route, and materialized views before it crosses the commit
marker. The proposal retains one typed evidence span for each explicit Receipt
artifact and points it into the canonical Receipt; no span infers a verifier
pass, independence, or acceptance.

Atlas source adapters follow this boundary too: they emit their catalogue or
graph output as Receipt v1 artifacts and call `vela land`; they never mint
findings, anchors, or canonical events directly.

- **Deny** returns before the marker and leaves no canonical Vela or Git delta.
- **Defer** installs the pending proposal. This is the ordinary producer
  outcome and is success, not a failed or half-accepted submission.
- **Permit** installs accepted state only through a verified certificate from a
  previously human-signed policy. A producer or MCP caller cannot create that
  authority.

The operation journal is private recovery state, not a protocol object. Before
its marker, failure is discardable. After the marker, recovery installs the
exact stored bytes idempotently. Git publication is a separate exact-path
transaction after the scientific transaction; a failed push cannot change the
route or authorization bytes.

## Reference emitter and validator

From outside this workspace:

```bash
python3 receipt_v1.py emit \
  --claim "the scoped claim" \
  --artifact witnesses/w.json:witness \
  --caveat "Pending Vela landing and human acceptance." \
  --replayability exact \
  --out receipt.json

python3 receipt_v1.py validate receipt.json
```

Receipt v1 is now a historical read-only interoperability object. Its parser,
canonicalization, bounds, and cross-implementation vectors remain under
`scripts/cross_impl_conformance.py` and the protocol Receipt tests. The retired
`work -> land -> sign` integration harness remains recoverable from Git
history but is not a current writer contract. Current producer intake uses
Submission v1 and is qualified separately.

## Replayability - honesty about re-execution

A large share of agent and hosted-model work is not exactly reproducible: model
weights, provider routing, hidden prompts, and retrieval layers drift. A receipt
must say so rather than pretend. The field is a closed set:

| class | meaning |
|---|---|
| `exact` | same bytes, same frozen verifier - deterministically re-runnable |
| `bounded` | deterministic code path; the external service version is only partly pinned |
| `approximate` | same prompt/model label can be re-run, but provider behavior may vary |
| `unavailable` | cannot be re-run; retained public bytes or immutable locators remain auditable, while restricted material may expose only an opaque custodian reference |
| `unknown` | insufficient replay metadata (the default for a receipt that omits the field) |

A value outside this set or a missing value is rejected at land time. The
neutral emitter writes `unknown` when the producer does not choose a more
specific class.

The class reaches the signed policy as `PolicyContext.replayability`. It is a
**policy lever, not a gate lever**: the gate (G1-G5, derived from verifier
attachments) is independent of it, but a policy may require `exact` before it
auto-admits a serious claim class - the honest expression of "a non-replayable
run should not satisfy a serious verification lane by itself." A policy that
never reads the field is unaffected; `unknown` is the cautious default, and the
evaluator is monotonic on it (it can only defer, never over-admit).

## Extension points (`environment` / `provenance`)

The `environment` and `provenance` objects are open by design. They are carried
for provenance consumers (the hub, exporters, adapters) and are not branched on
by the landing path. They are the stable home for external-run metadata, so the
receipt schema does not churn as adapters arrive:

- `environment.source` - `{system, run_id, source_uri, exported_at}` (which
  external system produced the run: shepherd, langgraph, inspect, nextflow, …).
- `environment.trace_refs` - `{otel_trace_id, shepherd_trace_id, …}`.
- `environment.lineage_refs` - `{swhid/dvc/datalad/ro_crate}`
  for durable code/data identity beyond a git URL.
- `environment.independence_basis` - `{method_family, solver_identity,
  code_lineage, dataset_lineage, model_lineage, shared_dependencies,
  declared_independent_of, known_couplings}`. Independence is inspectable and
  refutable, not asserted; the gate's `independent_of` on verifier attachments is
  the enforced counterpart.

## Invariants

1. **Content identity** - public artifacts are content-addressed (sha256) or
   bound to a durable external id (DOI, SWHID, OCI digest). Restricted,
   low-entropy material may instead use an opaque custodian reference.
2. **Claim awareness** - bound to a claim for relation and review, not text
   deduplication, and via the attachment layer to a claim digest for verifier
   binding.
3. **Replay disclosure** - the receipt states honestly what can be re-run.
4. **Redaction honesty** - sensitive payloads, equality-revealing digests, and
   commitment openings stay outside public Git. The public shell carries a
   safe digest, sealed commitment, or opaque custodian reference according to
   disclosure risk and access tier (`Public` | `Restricted` | `Classified`).
5. **Verifier separation** - receipts suggest verifier attachments; they do not
   set gate status.
6. **Human-acceptance separation** - an agent or system can produce a receipt,
   but accepted frontier state requires a protected human `review accept`
   or `review reject`
   approval or a human-signed policy admitting the lane.

## Status vocabularies - three layers, not one enum

Three status vocabularies coexist around a receipt, and they answer
different questions. Collapsing them into one field is the single-green-badge
failure; keep the projection explicit:

| Layer | Vocabulary | Question it answers |
|---|---|---|
| Gate (`vva_` attachments) | `NeedsVerification` / `Verified` / `Refuted` | Did named checks, derived from immutable inputs, pass? |
| Acceptance (`acceptance.acceptance_scope`) | `machine_verified` / `human_seen` / `locally_accepted` / `frontier_accepted` / `canon_accepted` / `hypothesis_only` / `retracted` / `superseded` | What standing did an accountable steward grant, and how far does it travel? |
| Status events (`status.kind`) | the event ladder (`draft` … `accepted`, plus supersede/withdraw/revoke/challenge/deprecate/restore) | What happened to this receipt over time? |

A receipt can be gate-`Verified` and only `locally_accepted`; a
`frontier_accepted` claim can later carry a `revokes` status event without
its historical gate result changing. Unifying these into one enum is
deferred deliberately (the same wire-risk class as the EventKind
unification): the Rust read layer is `vela-protocol`'s `objects::receipt_v1`
(`AcceptanceScope`), and the gate vocabulary stays
`analysis::verifier_attachment::GateStatus`.

## The layer above: verifier attachments (`vva_`)

Gate status is derived from **verifier attachments**, a separate post-receipt
object bound to a finding by id. Each carries a `claim_digest` (it checked the
exact claim text), `independent_of` declarations, `adversarial_probes`, and a
`method_integrity` verdict. Two matched independent attachments plus a surviving
adversarial probe are what move a claim toward `Verified` - never a receipt or a
single score on its own. A lone attachment is evidence ("attested by X"), not a
reproduction.

External evaluation logs, including Inspect-AI output, are producer activity.
They enter Vela only through Receipt v1 and `vela land`; there is no separate
attachment writer. A producer-reported evaluation remains provenance, not a
frozen exact-verifier result, and cannot flip the gate to `Verified` alone.

## Adapter roadmap (documented, mostly deferred)

The receipt boundary is intentionally the same for every source. Optional
producer adapters may shape source-native activity into Receipt v1; none is a
second frontier writer. Candidate integrations follow real producers, not spec:

- **Shepherd / OpenTelemetry** - agent execution trace → receipt `trace_refs`.
- **Nextflow / Snakemake** - workflow run → receipt (inputs, outputs, env,
  replay command).
- **DVC / DataLad** - content-addressed dataset/model → artifact + lineage refs.
- **Phoenix / Langfuse / MLflow** - trace/eval importers.
- **RO-Crate** - export a receipt / proof / frontier slice as an archival package
  (the exporter already emits `ro-crate-metadata.jsonld`).

The rule holds across all of them: external systems produce activity; Vela
canonicalizes it into receipts; verifiers and human keys decide what becomes
state.
