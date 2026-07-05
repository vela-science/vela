# Open-source landscape memo for Vela and Constellate

Date: 2026-07-05  
Audience: Vela and Constellate maintainers  
Status: Strategic memo, not an ADR

## Executive summary

The relevant open-source landscape is not a single competitor set. It is a stack of adjacent substrates:

1. Agent execution runtimes and coding agents.
2. Agent observability and evaluation systems.
3. Scientific workflow and reproducibility engines.
4. Data, model, and lineage versioning tools.
5. Software supply-chain attestation and durable content ID standards.

Vela should not try to win all five categories. That would dilute the protocol. The durable control point is narrower and more important: **scientific state transitions**. Other systems should generate source activity. Vela should turn source activity into receipts, proposals, verifier attachments, packs, accepted events, and replayed frontier state.

The current repo language is already pointed in the right direction:

- Vela is "version control for scientific state, with a gate on what counts as verified." See `README.md`.
- The CLI sentence is: "agents propose, verifiers reproduce, humans accept, git publishes." See `docs/CLI.md`.
- The repo-native ADR says: "Git is the substrate. Vela is the protocol." See `docs/adr/0001-frontier-as-git-repo.md`.
- The formal theory frames papers, datasets, lab logs, benchmark outputs, agent traces, and reviews as source activity until they become proposals, diffs, accepted events, and replayed state. See `docs/THEORY.md`.

That framing should be preserved. Vela should become the canonical state protocol for scientific evidence, not another agent framework, telemetry backend, notebook system, workflow engine, or model registry.

The closest conceptual adjacency found is [Shepherd](https://github.com/shepherd-agents/shepherd): a Git-like, reversible, forkable, replayable execution trace substrate for agents. Shepherd is important because it points at the same control point from below: agent work must become inspectable execution history. But it is generic agent infrastructure and early-stage. The right Vela response is not to adopt Shepherd as the spine. The right response is to build a Shepherd adapter and use it as a design pressure test for Vela receipts.

## Strategic conclusion

Vela should own this boundary:

```text
source activity -> Vela receipt/proposal -> verifier gate -> signed event -> frontier state -> Constellate index/network
```

Where source activity includes:

```text
agent traces
workflow runs
notebooks
benchmarks
datasets
proofs
human reviews
lab logs
model evaluations
external repository changes
```

The key product discipline is this:

> Capture broadly. Verify narrowly. Accept only by human key. Publish through git. Index through Constellate.

This lets Vela benefit from the whole open-source ecosystem without becoming hostage to any one runtime.

## Landscape map

| Layer | Representative projects | What they own | Vela response |
|---|---|---|---|
| Agent execution control | Shepherd, LangGraph, OpenHands-style coding agents, SWE-agent-style coding agents | Task execution, sandboxing, checkpoints, fork/replay, tool calls | Ingest traces as receipts. Do not become the runtime. |
| Agent observability and evals | OpenTelemetry, Phoenix, Langfuse, MLflow, Inspect AI | Traces, spans, model calls, datasets, experiments, eval reports | Treat as telemetry and evaluator evidence. Do not treat dashboard scores as verified science. |
| Scientific workflows | Nextflow, Snakemake, REANA, Renku | Portable computational pipelines, containers, HPC/cloud execution | Attach workflow runs to claims. Preserve replay recipes and outputs. |
| Data and model versioning | DVC, DataLad, lakeFS-like object-store versioning, MLflow model registry | Dataset/model identity, remote storage, experiment history | Reference content-addressed artifacts. Let data tools store bytes. Vela stores scientific meaning and state transitions. |
| Lineage and research packaging | OpenLineage, Marquez, RO-Crate, WorkflowHub | Dataset/job/run lineage, exchange packages, workflow metadata | Map lineage facets and crates into Vela provenance objects and export back out. |
| Supply-chain attestation | Sigstore, in-toto, SLSA, Software Heritage SWHID | Artifact identity, signing, provenance, build integrity | Reuse mental model and possibly identifiers. Do not collapse scientific verification into software integrity. |

## Project notes and implications

### Shepherd

**What it is:** A runtime substrate for agent work. It records agent runs as durable, inspectable traces and supports retained outputs before accepting them into the workspace. The paper describes a typed event log with Git-like fork/replay semantics and argues that meta-agents need structured execution state rather than flat transcripts.

**Why it matters:** Shepherd is the strongest open-source signal that agent work is moving from chat transcripts to execution histories. That is directly relevant to Vela because scientific agent work cannot be trusted as prose. It needs replayable receipts, changed artifacts, explicit claims, and reviewable deltas.

**Risk to Vela:** Low direct competition, high conceptual adjacency. Shepherd is lower in the stack. It answers: "What did the agent do, and can we replay or fork it?" Vela answers: "What scientific state change, if any, is now admissible?"

**Recommended action:** Build `shepherd -> vrc` as the first trace adapter. A minimal spike should:

1. Run a toy scientific task through Shepherd.
2. Export the execution trace, workspace diff, retained output, and final claim.
3. Convert those into a Vela record/proposal packet.
4. Run `vela gate check` with mock or real verifier attachments.
5. Document what Shepherd gives for free and what science-specific metadata Vela still requires.

Do not depend on Shepherd as core infrastructure yet. Treat it as an adapter target and design adversary.

### LangGraph

**What it is:** A durable agent workflow graph framework with persistence, checkpointing, human-in-the-loop interruption, and time travel semantics.

**Why it matters:** LangGraph is likely to be used by teams building long-running scientific agents. Its checkpoints and stores can become source material for receipts.

**Risk to Vela:** Low. It is an orchestration layer. It does not define scientific state, verification gates, review policy, or frontier semantics.

**Recommended action:** Define a generic agent-run receipt schema that can ingest LangGraph checkpoints, inputs, outputs, and tool-call traces without depending on LangGraph internals.

### OpenHands, SWE-agent-like systems, and coding agents

**What they are:** Software engineering agents that operate over repositories, issues, tests, shells, browsers, and sandboxes.

**Why they matter:** They are the likely interface through which agents will propose changes to frontier repos, verifier code, examples, and scientific artifacts.

**Risk to Vela:** Medium at the workflow edge, low at the protocol core. These systems can produce plausible patches and tests, but they do not solve the scientific acceptance problem.

**Recommended action:** Make Vela first-class agent plumbing:

- Keep `--json` everywhere.
- Keep MCP tools small and typed.
- Treat agent patches as proposals, never accepted state.
- Make failure records cheap and signed when useful.
- Add examples for coding-agent loops: issue -> attempt -> receipt -> proposal -> human sign.

### OpenTelemetry

**What it is:** Vendor-neutral observability standards and tooling for traces, metrics, and logs.

**Why it matters:** Agent and workflow systems are converging on trace data. Vela should not invent a private tracing vocabulary unless the scientific state layer requires it.

**Risk to Vela:** Low. OpenTelemetry is a substrate for operational telemetry, not evidence admission.

**Recommended action:** Align Vela receipt trace references with OpenTelemetry concepts where possible: trace id, span id, attributes, events, links, status. Store only what is needed for scientific provenance and reproducibility. Keep sensitive payloads redacted but hash-bound.

### Phoenix and Langfuse

**What they are:** Open-source LLM observability and evaluation platforms. They record traces, sessions, model calls, retrieval, tool use, prompts, datasets, experiments, and human or model-based evaluations.

**Why they matter:** They are likely to be used by teams debugging scientific agents. They can generate valuable source activity and evaluator metadata.

**Risk to Vela:** Medium if Vela drifts into dashboard observability. Low if Vela keeps the state-machine boundary.

**Recommended action:** Do not build a Phoenix/Langfuse clone. Build importers for their trace/export formats and let their dashboards remain dashboards. Vela should say whether a claim is `needs_verification`, `verified`, `refuted`, or contested under frontier policy. It should not expose "trace quality" as if it were scientific truth.

### MLflow

**What it is:** A widely used experiment tracking, model lifecycle, and increasingly LLM/agent evaluation platform.

**Why it matters:** Many scientific and ML teams already use MLflow to organize runs, parameters, metrics, artifacts, model versions, traces, and evaluations.

**Risk to Vela:** Medium for user confusion. MLflow tracks experiments and models. Vela tracks scientific state transitions and verification gates.

**Recommended action:** Build `mlflow run -> Vela attachment` rather than a competing experiment tracker. MLflow run ids, artifact URIs, parameters, metrics, and model versions should be evidence references, not frontier state by themselves.

### Inspect AI

**What it is:** An open-source framework for LLM evaluations built around datasets, solvers, tools, scorers, sandboxing, and logs.

**Why it matters:** Its task shape maps well to Vela verifier attachments: dataset, solver, scorer, environment, score, logs.

**Risk to Vela:** Low. Inspect helps author and run evaluations. Vela decides how an evaluation attaches to a claim and whether it counts toward a gate.

**Recommended action:** Treat Inspect as a first evaluator adapter. A Vela verifier attachment should be able to point to an Inspect task, config, logs, and scored output, then bind the result to a claim digest.

### Nextflow and Snakemake

**What they are:** Mature scientific workflow engines for reproducible, scalable pipelines across local, HPC, cluster, and cloud environments.

**Why they matter:** They already own a large part of computational biology and scientific pipeline execution. Vela should not try to replace them.

**Risk to Vela:** Low if Vela is a state protocol. High if Vela becomes a workflow engine.

**Recommended action:** Build workflow-run attachments:

```text
workflow engine
workflow version
input dataset ids
parameter set
container/package environment
execution backend
output artifact ids
logs and report ids
replay command
claim digest binding
```

A successful workflow run is evidence. It is not automatically a verified claim.

### REANA and Renku

**What they are:** Reproducible analysis and collaborative research platforms that connect code, data, compute, containers, workflows, and people.

**Why they matter:** They represent the institutional research-platform shape Vela will be compared against.

**Risk to Vela:** Medium in product narrative. Low in protocol substance. They manage reproducible analysis environments; Vela governs scientific state and evidence transitions.

**Recommended action:** Position Constellate as complementary to research environments. A Renku or REANA project should be able to export a Vela proposal, and a Vela frontier should be able to link back to the exact project/run that produced evidence.

### DVC and DataLad

**What they are:** Git-adjacent systems for versioning large data, dataset structure, provenance, and collaboration. DVC is especially strong in ML pipelines and remote artifact storage. DataLad builds on Git and git-annex for distributed datasets and nested subdatasets.

**Why they matter:** They solve a major part of the storage problem that Vela should not own: large bytes, remote storage, dataset versions, and local checkout semantics.

**Risk to Vela:** High if Vela tries to become data storage. Low if Vela remains protocol and references content-addressed artifacts.

**Recommended action:** Support DVC/DataLad identifiers and metadata as first-class artifact references. Let them store bytes. Vela should bind claims to content-addressed data references and verifier results.

### OpenLineage and Marquez

**What they are:** Open standards and reference implementation for job, run, and dataset lineage.

**Why they matter:** Their model of jobs, runs, datasets, and facets is close to the operational lineage Vela needs to ingest from data pipelines.

**Risk to Vela:** Low. OpenLineage is operational lineage. Vela is scientific state and verification policy.

**Recommended action:** Map OpenLineage concepts into Vela provenance:

```text
OpenLineage Dataset -> Vela Dataset object
OpenLineage Job -> Vela Method or Workflow object
OpenLineage Run -> Vela Experiment or ExecutionReceipt object
OpenLineage Facet -> typed provenance attributes
```

Do not make OpenLineage the normative model for scientific claims. Use it as an import/export surface.

### RO-Crate and WorkflowHub

**What they are:** Lightweight research object packaging and workflow exchange conventions.

**Why they matter:** Vela needs to interoperate with research archives and workflow communities. RO-Crate is a plausible export package for a Vela receipt, proof packet, or accepted frontier slice.

**Risk to Vela:** Low. This is an interoperability opportunity.

**Recommended action:** Add an RO-Crate export path for evidence bundles:

```text
claim
context
artifacts
workflow
run metadata
verifier attachments
review attestations
frontier ids
content hashes
```

### ReproZip

**What it is:** A tool for packing code, data, libraries, environment, and execution options into reproducible bundles that can run elsewhere.

**Why it matters:** It is a useful lower-level capture mechanism for legacy or messy computations.

**Risk to Vela:** Low.

**Recommended action:** Treat ReproZip bundles as replay artifacts. They can support reproducibility, but they are not a substitute for claim-bound verifier attachments.

### Software Heritage SWHID

**What it is:** A persistent intrinsic identifier system for archived software artifacts, based on content identity and Merkle-DAG style ideas.

**Why it matters:** Scientific evidence must cite software and source code durably. Git commit URLs are useful, but archival identifiers are stronger for long-term scientific references.

**Risk to Vela:** Low.

**Recommended action:** Allow Vela artifact references to include SWHIDs when available. Use SWHID-style thinking for durable source references, but keep Vela ids tied to canonical typed objects and frontier events.

### Sigstore, in-toto, and SLSA

**What they are:** Open supply-chain integrity systems and standards. Sigstore focuses on signing and verification of artifacts with transparency logs. in-toto describes supply-chain steps and attestations. SLSA provides a framework for trustworthy artifact production.

**Why they matter:** They have already solved many problems around artifact identity, signing, provenance, transparency, and build integrity.

**Risk to Vela:** Medium if the team conflates supply-chain integrity with scientific verification. A signed artifact is authentic, not necessarily scientifically true.

**Recommended action:** Borrow patterns, not semantics:

- signed attestations
- transparent logs
- provenance predicates
- build/run steps
- tamper-resistant evidence
- policy levels

But preserve Vela's stronger distinction: scientific status is derived from claim-bound evidence and gate policy, not from artifact authenticity alone.

## What Vela should copy

### 1. Shepherd's retained-output discipline

An agent's output should land as a proposal, not as state. This is already aligned with Vela. The useful addition is trace-level fork/replay metadata for failed, partial, and alternative attempts.

### 2. OpenTelemetry's schema humility

Do not make every source system speak native Vela internally. Define a small, typed receipt boundary and import from common telemetry conventions.

### 3. DVC/DataLad's git-native storage stance

The ADR is right: git stores and transports; Vela judges. Use existing storage and versioning tools for bytes. Vela should bind bytes to meaning.

### 4. Inspect's dataset/solver/scorer shape

This maps cleanly to verifier attachments. It is also teachable to users.

### 5. RO-Crate's exchange pragmatism

Vela needs exportable evidence bundles that institutions can archive, cite, and inspect without running the full platform.

### 6. Sigstore/in-toto's attestation model

The Vela event chain, verifier attachments, and review attestations should feel as boring and inspectable as modern supply-chain metadata, while preserving science-specific semantics.

## What Vela should avoid

### 1. Do not become a generic agent runtime

Agent runtimes will churn. Vela's moat is not tool calling, sandboxing, or model orchestration. Vela's moat is the verified frontier.

### 2. Do not become an LLM observability dashboard

Traces are useful source activity. They are not scientific state. Phoenix, Langfuse, MLflow, and OpenTelemetry are better positioned for generic observability.

### 3. Do not treat LLM-as-judge evals as verification

They can be evidence, probes, triage, or review aids. They should not satisfy a serious scientific verification gate unless the frontier policy explicitly admits them for a narrow claim class.

### 4. Do not let the hub become authority again

The ADR's direction is correct: the hub should be an index over replayable git-native frontiers, not the source of truth.

### 5. Do not store transcripts as if they were receipts

A transcript is not a receipt. A receipt needs content-addressed inputs and outputs, code/environment references, trace metadata, claim binding, replay recipe, redaction policy, and verifier attachment hooks.

## Proposed primitive: Vela execution receipt

The repo already has `vela record` and `vrc_` packets. The recommendation is to sharpen that boundary into a universal ingestion primitive for external systems.

A receipt should minimally carry:

```text
receipt_version
source_system              # shepherd, langgraph, nextflow, snakemake, mlflow, phoenix, langfuse, manual, other
source_run_id
source_uri
actor_refs                 # human, agent, organization, key ids where available
started_at / ended_at
claim_bindings             # claim digest or proposed claim text plus context
input_artifacts            # content-addressed data, code, prompts, configs, protocols
output_artifacts           # content-addressed result files, plots, tables, notebooks, proofs
workspace_diff_refs        # git diff, patch, retained output, or artifact delta
trace_refs                 # OpenTelemetry trace id, Shepherd trace id, Langfuse trace id, etc.
workflow_refs              # Nextflow/Snakemake/REANA/Renku ids when relevant
environment_refs           # container, lockfile, package set, hardware, verifier version
model_call_summary         # provider/model/tool summary, with payload hashes and redaction policy
replay_recipe              # command, workflow entrypoint, or proof verifier invocation
lineage_refs               # OpenLineage, DataLad, DVC, SWHID, RO-Crate, etc.
redaction_policy           # what is hidden, what is hash-bound, who may inspect
verifier_attachment_hooks  # how this receipt can become evidence for a gate
signature_refs             # human, agent, system, supply-chain attestations
```

The important rule: a receipt is admitted to the log by content addressing and signatures. A claim becomes verified only by the gate.

## Proposed adapters

Prioritize adapters by strategic leverage, not by completeness.

### P0: Shepherd adapter

Reason: closest conceptual adjacency. Tests the agent-trace-to-science-receipt boundary.

Output:

```text
Shepherd trace + retained workspace output -> Vela receipt -> proposal preview -> gate check
```

### P0: OpenTelemetry trace reference

Reason: prevents Vela from inventing private observability vocabulary.

Output:

```text
trace id + span links + redacted payload hashes -> Vela receipt trace_refs
```

### P1: Inspect AI verifier attachment

Reason: maps cleanly to evaluator attachments.

Output:

```text
Inspect task log + scorer result -> verifier attachment bound to claim digest
```

### P1: Nextflow/Snakemake workflow-run attachment

Reason: high relevance to computational science.

Output:

```text
workflow run metadata + inputs + outputs + environment -> Vela evidence attachment
```

### P1: DVC/DataLad artifact reference

Reason: keeps Vela out of the data storage business.

Output:

```text
DVC/DataLad dataset version -> content-addressed artifact ref in receipt
```

### P2: RO-Crate export

Reason: useful for institutions, archives, and workflow communities.

Output:

```text
Vela receipt/proof/frontier slice -> RO-Crate package
```

### P2: Sigstore/in-toto attestation import

Reason: useful for build integrity and software provenance.

Output:

```text
artifact attestation -> Vela artifact authenticity metadata
```

## Product implications for Constellate

Constellate should not be framed as a better notebook, better workflow engine, or better agent IDE. It should be framed as the coordination layer over replayable scientific state.

The clean split:

```text
Vela: protocol, CLI, reducer, gate, receipts, verifier attachments, signed frontier state
Constellate Hub: index, search, citation, dependency graph, review surfaces, public/private overlays
Constellate App: human and agent-facing product over that state
```

This matches the ADR's hub-index direction. The hub is valuable because it makes frontier state discoverable, queryable, citeable, and reviewable. It should not be the canonical store.

## Suggested 90-day execution plan

### Phase 1: Receipt boundary

1. Write `docs/RECEIPTS.md` as the implementation-facing spec.
2. Add JSON schema fixtures for external receipts.
3. Add conformance vectors for:
   - complete receipt
   - redacted but hash-bound receipt
   - receipt with unmatched claim digest
   - receipt with missing replay recipe
   - receipt with external trace reference only
4. Decide whether this is a sharpened `vrc_` packet or a distinct receipt object. Prefer sharpening `vrc_` unless the type separation prevents confusion.

### Phase 2: First adapters

1. Shepherd trace adapter.
2. OpenTelemetry trace-reference adapter.
3. Inspect AI result attachment.
4. Nextflow or Snakemake run attachment.
5. DVC or DataLad artifact reference.

The demo should be one complete loop:

```text
agent/workflow runs -> receipt -> proposal -> verifier attachment -> gate -> human accept -> git push -> hub index
```

### Phase 3: Public demonstration

Pick one small frontier where the evidence is tractable. Good candidates:

- Sidon or another exact-verifier math frontier.
- A small benchmark frontier with deterministic evaluator attachments.
- A toy computational biology pipeline with public data and simple verifier checks.

The demo should not be "agent solves science." It should be:

> Agentic scientific work becomes reviewable, reproducible, and gated frontier state.

That is the stronger claim.

## Open questions

1. **Receipt privacy:** How much trace detail can be hash-bound without being public?
2. **Model calls:** How should Vela represent non-replayable hosted model calls?
3. **Independence:** How does the gate detect or encode independence when two receipts use the same model, data, or workflow engine?
4. **Artifact storage:** What is the canonical policy for large trace payloads: git LFS, DVC, DataLad, S3-compatible mirrors, or external archive ids?
5. **Verifier attachment language:** Should workflow/eval adapters emit general receipts first, then explicit verifier attachments later?
6. **Institutional custody:** How should labs sign receipts when the producer is an agent running inside a platform account?
7. **Export:** What is the minimum useful RO-Crate export for a frontier slice?
8. **Hub indexing:** Which receipt fields should the hub index for discovery without compromising private trace payloads?

## Recommended decision

Adopt this strategy:

> Vela is the protocol for scientific state. External runtimes produce source activity. Vela receipts canonicalize that activity. Verifiers and human keys decide what becomes frontier state. Constellate makes that state discoverable, reviewable, and compounding.

In practical terms, the next product move is not a new agent runtime. It is:

```text
vela record --from <external-run>
```

or equivalently:

```text
vela receipt ingest <source>
```

The exact command can be decided later. The boundary should be decided now.

## Sources studied

Internal Vela sources:

- `README.md`
- `docs/CLI.md`
- `docs/adr/0001-frontier-as-git-repo.md`
- `docs/THEORY.md`

External sources checked on 2026-07-05:

- [Shepherd GitHub repo](https://github.com/shepherd-agents/shepherd)
- [Shepherd paper](https://arxiv.org/abs/2605.10913)
- [LangGraph persistence docs](https://langchain-ai.github.io/langgraph/concepts/persistence/)
- [OpenTelemetry docs](https://opentelemetry.io/docs/)
- [Phoenix docs](https://arize.com/docs/phoenix)
- [Langfuse docs](https://langfuse.com/docs)
- [MLflow docs](https://mlflow.org/docs/latest/)
- [Inspect AI docs](https://inspect.aisi.org.uk/)
- [Nextflow docs](https://www.nextflow.io/docs/latest/)
- [Snakemake docs](https://snakemake.readthedocs.io/)
- [REANA docs](https://docs.reana.io/)
- [Renku docs](https://renku.readthedocs.io/)
- [DVC docs](https://dvc.org/doc)
- [DataLad handbook](https://handbook.datalad.org/)
- [OpenLineage docs](https://openlineage.io/docs/)
- [RO-Crate](https://www.researchobject.org/ro-crate/)
- [ReproZip](https://www.reprozip.org/)
- [Software Heritage SWHID documentation](https://docs.softwareheritage.org/devel/swh-model/persistent-identifiers.html)
- [Sigstore](https://www.sigstore.dev/)
- [in-toto](https://in-toto.io/)
- [SLSA](https://slsa.dev/)
