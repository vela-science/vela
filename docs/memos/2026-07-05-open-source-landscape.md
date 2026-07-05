# Open-source landscape memo for Vela and Constellate

Date: 2026-07-05  
Audience: Vela and Constellate maintainers  
Status: Strategic memo, not an ADR

## Takeaway

The relevant open-source landscape is not one market. It is a stack: agent runtimes, agent observability, scientific workflow engines, data/versioning tools, lineage packages, and supply-chain attestations.

Vela should not try to win all of those categories. The durable control point is narrower and deeper: **scientific state transitions**. External systems should generate source activity. Vela should turn that activity into receipts, proposals, verifier attachments, packs, signed events, and replayed frontier state.

This matches the current repo thesis:

- `README.md`: Vela is version control for scientific state, with a gate on what counts as verified.
- `docs/CLI.md`: agents propose, verifiers reproduce, humans accept, git publishes.
- `docs/adr/0001-frontier-as-git-repo.md`: Git is the substrate. Vela is the protocol.
- `docs/THEORY.md`: papers, datasets, lab logs, benchmark outputs, agent traces, and reviews are source activity until they become proposals, diffs, accepted events, and replayed state.

## Landscape map

| Layer | Representative projects | What they own | Vela response |
|---|---|---|---|
| Agent execution | Shepherd, LangGraph, OpenHands/SWE-agent-style systems | Task execution, sandboxing, checkpoints, fork/replay, tool calls | Ingest traces as receipts. Do not become the runtime. |
| Observability and evals | OpenTelemetry, Phoenix, Langfuse, MLflow, Inspect AI | Traces, spans, model calls, datasets, experiments, eval reports | Treat as telemetry and evaluator evidence, not verified science. |
| Scientific workflows | Nextflow, Snakemake, REANA, Renku | Portable pipelines, containers, HPC/cloud execution | Attach workflow runs to claims. Preserve replay recipes and outputs. |
| Data/model versioning | DVC, DataLad, lakeFS-like systems, MLflow registry | Dataset/model identity, remote storage, experiment history | Reference content-addressed artifacts. Let data tools store bytes. |
| Lineage and packaging | OpenLineage, Marquez, RO-Crate, ReproZip, WorkflowHub | Job/run/dataset lineage, exchange packages, replay bundles | Map into Vela provenance and export back out. |
| Supply-chain integrity | Sigstore, in-toto, SLSA, Software Heritage SWHID | Artifact signing, provenance, durable IDs, build integrity | Borrow attestation patterns. Keep scientific verification separate. |

## Highest-signal finding: Shepherd

Shepherd is the closest conceptual adjacency. It treats agent execution as a Git-like, reversible, forkable, replayable trace with retained outputs before workspace acceptance. That is directly relevant because scientific agent work cannot be trusted as chat prose. It needs a replayable receipt, explicit artifact deltas, claim binding, and reviewable evidence.

But Shepherd is lower in the stack. It answers: what did the agent do, and can we inspect, fork, or replay it? Vela answers: what scientific state change, if any, is admissible under verifier and human-key policy?

Recommendation: build `shepherd -> vrc` as the first trace adapter. Do not adopt Shepherd as Vela's spine yet. Use it as a design pressure test.

Minimal spike:

```text
Shepherd run -> trace + retained output -> Vela receipt -> proposal preview -> gate check
```

The expected result is a list of what Shepherd gives for free and what Vela still needs: claim digest binding, verifier attachments, independence declarations, adversarial probes, review policy, and frontier state semantics.

## Other project implications

### LangGraph

Useful for durable agent workflows, checkpointing, time travel, and human-in-the-loop execution. Vela should ingest checkpoints and run metadata, not compete with the orchestration layer.

### Phoenix, Langfuse, MLflow, and OpenTelemetry

These are strong observability and experiment systems. Vela should import trace ids, span links, run ids, parameters, artifacts, metrics, and evaluation outputs. It should not become a generic dashboard. A trace is source activity. It is not a verified claim.

### Inspect AI

Inspect's shape maps well to verifier attachments: dataset, solver, tools, sandbox, scorer, log. Treat Inspect as a likely first evaluator adapter.

### Nextflow, Snakemake, REANA, and Renku

These already own much of reproducible scientific workflow execution. Vela should attach their runs to claims with workflow version, input artifacts, parameters, environment, backend, outputs, logs, and replay command.

### DVC and DataLad

These solve large data and dataset checkout better than Vela should. Support their identifiers and metadata as artifact references. Vela should bind claims to data versions and verifier results, not store all scientific bytes itself.

### OpenLineage, RO-Crate, ReproZip, and SWHID

OpenLineage gives a useful job/run/dataset vocabulary. RO-Crate gives a useful exchange package. ReproZip can preserve messy legacy computations. SWHID gives durable software identifiers. Vela should interoperate with these rather than replace them.

### Sigstore, in-toto, and SLSA

These are useful models for signing, provenance, transparency, and policy levels. They should influence Vela attestations. But artifact authenticity is not scientific verification. A signed result can still be wrong.

## Proposed primitive: execution receipt

The repo already has `vela record` and `vrc_` packets. Sharpen that boundary into a universal ingestion primitive for external systems.

Minimal fields:

```text
receipt_version
source_system              # shepherd, langgraph, nextflow, snakemake, mlflow, phoenix, langfuse, inspect, manual
source_run_id / source_uri
actor_refs                 # human, agent, org, key ids where available
claim_bindings             # claim digest or proposed claim text plus context
input_artifacts            # data, code, prompts, configs, protocols
output_artifacts           # results, plots, tables, notebooks, proofs
workspace_diff_refs        # git diff, patch, retained output, artifact delta
trace_refs                 # OTel, Shepherd, Langfuse, Phoenix, etc.
workflow_refs              # Nextflow, Snakemake, REANA, Renku, etc.
environment_refs           # container, lockfile, package set, hardware, verifier version
model_call_summary         # provider/model/tool summary, payload hashes, redaction policy
replay_recipe              # command, workflow entrypoint, proof verifier, or reproduction path
lineage_refs               # OpenLineage, DVC, DataLad, RO-Crate, SWHID
verifier_attachment_hooks
signature_refs
```

Rule: a receipt can be admitted to the log by signature and content address. A claim becomes verified only through the gate.

## 90-day recommendation

1. Write `docs/RECEIPTS.md` as the implementation-facing spec.
2. Add receipt schema fixtures and conformance vectors: complete, redacted/hash-bound, unmatched claim digest, missing replay recipe, external trace reference only.
3. Build first adapters: Shepherd trace, OpenTelemetry trace reference, Inspect result attachment, Nextflow or Snakemake run attachment, DVC or DataLad artifact reference.
4. Demo one full loop:

```text
agent/workflow run -> receipt -> proposal -> verifier attachment -> gate -> human accept -> git push -> hub index
```

The demo claim should be modest. The point is not that an agent solved science. The point is that agentic scientific work became reviewable, reproducible, and gated frontier state.

## Non-goals

- Do not build a generic agent runtime.
- Do not build an LLM observability dashboard.
- Do not treat LLM-as-judge outputs as verification by default.
- Do not let the hub become canonical authority again.
- Do not store transcripts as if they were receipts.
- Do not collapse software supply-chain integrity into scientific truth.

## Recommended decision

Adopt this product boundary:

```text
source activity -> Vela receipt/proposal -> verifier gate -> signed event -> frontier state -> Constellate index/network
```

In CLI terms, the next move is some version of:

```text
vela record --from <external-run>
```

or:

```text
vela receipt ingest <source>
```

The exact command can wait. The boundary should be decided now.

## Sources studied

Internal: `README.md`, `docs/CLI.md`, `docs/adr/0001-frontier-as-git-repo.md`, `docs/THEORY.md`.

External checked on 2026-07-05: [Shepherd](https://github.com/shepherd-agents/shepherd), [Shepherd paper](https://arxiv.org/abs/2605.10913), [LangGraph persistence](https://langchain-ai.github.io/langgraph/concepts/persistence/), [OpenTelemetry](https://opentelemetry.io/docs/), [Phoenix](https://arize.com/docs/phoenix), [Langfuse](https://langfuse.com/docs), [MLflow](https://mlflow.org/docs/latest/), [Inspect AI](https://inspect.aisi.org.uk/), [Nextflow](https://www.nextflow.io/docs/latest/), [Snakemake](https://snakemake.readthedocs.io/), [REANA](https://docs.reana.io/), [Renku](https://renku.readthedocs.io/), [DVC](https://dvc.org/doc), [DataLad](https://handbook.datalad.org/), [OpenLineage](https://openlineage.io/docs/), [RO-Crate](https://www.researchobject.org/ro-crate/), [ReproZip](https://www.reprozip.org/), [Software Heritage SWHID](https://docs.softwareheritage.org/devel/swh-model/persistent-identifiers.html), [Sigstore](https://www.sigstore.dev/), [in-toto](https://in-toto.io/), [SLSA](https://slsa.dev/).
