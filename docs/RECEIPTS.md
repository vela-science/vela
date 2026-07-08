# Receipts - the boundary where activity becomes admissible

A **receipt** is how external activity enters Vela. It is not a transcript and
not a verdict. It is a content-addressed, claim-aware evidence packet that
`vela land` turns into a pending proposal, which a signed policy or a human then
routes into frontier state.

```
source activity → receipt (vrc_) → proposal → policy/gate → signed event → frontier state
```

Everything before the receipt is production of evidence (an agent run, a
workflow, an eval, a proof). Everything after it is coordination over accepted,
deferred, or contested scientific state. The receipt is the one control point in
between, and the only input the landing path reads.

A receipt does **not** imply verification. It can carry evidence and suggest
verifier attachments, but it never sets gate status. That stays with the
verifier attachments (`vva_`) and the human key.

## The shape: `vela.receipt.v1`

The schema is versioned at
[`docs/schemas/vela.receipt.v1.schema.json`](schemas/vela.receipt.v1.schema.json).
External producers should validate against that file before handing a receipt to
`vela land`. The tiny dependency-free emitter in
[`tools/receipt-v0`](../tools/receipt-v0) is the reference cold-start tool for
strangers: copy it outside this repo and run `python3 -m vela_receipt_v0`.

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

- **claim** - the exact assertion. Landing dedups by `(claim, type)`: a
  byte-identical re-land is a retry (exit 5), never a fork.
- **type** - classifies the finding itself (`computational`, `theoretical`,
  `negative`, …), and drives the policy `claim_class` (`receipt_<type>`).
- **caveats** - the honesty requirement. An empty list is rejected. State what
  the evidence does not establish so the receipt cannot over-claim.
- **artifacts** - hashed (sha256) at land time. `kind` defaults to `witness`.
- **verifier_runs** - evidence from a source-system verifier. A single `pass`
  raises the landing's assurance signal; it does not, alone, verify the claim.
- **replayability** - see below.
- **conditions / verification_requirements / state_diff** - the Carina-facing
  typed handoff fields. They state what assumptions travel with the receipt,
  what verifier or review work is still required, and what state change the
  producer believes the proposal would make. Landing records them as provenance;
  acceptance still requires the signed policy or a human key.
- **status** - the producer may only emit `draft` or `emitted` status with
  `authority: producer`. `landed_pending`, `accepted`, `rejected`, and
  `superseded` are Vela-side or human-key statuses and must not be forged by an
  emitter.

## Reference emitter and validator

From outside this workspace:

```bash
python3 -m vela_receipt_v0 emit \
  --claim "the scoped claim" \
  --artifact witnesses/w.json:witness \
  --caveat "Pending Vela landing and human acceptance." \
  --replayability exact \
  --out receipt.json

python3 -m vela_receipt_v0 validate receipt.json
```

The conformance gate runs
[`scripts/stranger-first-write.sh`](../scripts/stranger-first-write.sh): it
copies only the emitter into a temp directory, emits a receipt, replays the
Sidon artifact with `vela reproduce`, lands into a temp copy of the Sidon
frontier as `agent:scripted-stranger`, and asserts the proposal remains
`pending_review`.

## Replayability - honesty about re-execution

A large share of agent and hosted-model work is not exactly reproducible: model
weights, provider routing, hidden prompts, and retrieval layers drift. A receipt
must say so rather than pretend. The field is a closed set:

| class | meaning |
|---|---|
| `exact` | same bytes, same frozen verifier - deterministically re-runnable |
| `bounded` | deterministic code path; the external service version is only partly pinned |
| `approximate` | same prompt/model label can be re-run, but provider behavior may vary |
| `unavailable` | cannot be re-run, but the payloads are hash-bound and auditable |
| `unknown` | insufficient replay metadata (the default for a receipt that omits the field) |

A value outside this set is rejected at land time. Absence defaults to `unknown`,
so every pre-v0.748 receipt lands unchanged.

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

1. **Content identity** - artifacts are content-addressed (sha256) or bound to a
   durable external id (DOI, SWHID, OCI digest).
2. **Claim awareness** - bound to a claim (for dedup) and, via the attachment
   layer, to a claim digest (for verifier binding).
3. **Replay disclosure** - the receipt states honestly what can be re-run.
4. **Redaction honesty** - sensitive payloads are hash-bound and governed by
   access tiers (`Public` | `Restricted` | `Classified`); the public shell
   carries only safe fields.
5. **Verifier separation** - receipts suggest verifier attachments; they do not
   set gate status.
6. **Human-acceptance separation** - an agent or system can produce a receipt,
   but accepted frontier state requires the key ceremony (`vela sign`) or a
   human-signed policy admitting the lane.

## The layer above: verifier attachments (`vva_`)

Gate status is derived from **verifier attachments**, a separate post-receipt
object bound to a finding by id. Each carries a `claim_digest` (it checked the
exact claim text), `independent_of` declarations, `adversarial_probes`, and a
`method_integrity` verdict. Two matched independent attachments plus a surviving
adversarial probe are what move a claim toward `Verified` - never a receipt or a
single score on its own. A lone attachment is evidence ("attested by X"), not a
reproduction.

The **Inspect-AI adapter** is the first external attachment source: an Inspect
eval log becomes a `vva_` bound to a claim digest, defaulted to
`method_integrity: unattested` (an LLM-driven scorer is evidence, not a frozen
exact verifier), so it cannot flip the gate to `Verified` alone. Not every
Inspect score is admissible verification; the frontier policy decides.

## Adapter roadmap (documented, mostly deferred)

The receipt boundary is intentionally the same for every source. One adapter is
built (Inspect → verifier attachment). The rest are documented and follow a real
producer, not built on spec:

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
