# Math-first, framework-neutral evidence campaign

## Objective

Prove or falsify Vela's value through one bounded campaign:

```text
complete and replay a real Submission -> Verification -> Decision loop
compare native Codex, Canopus, and removable orchestration under matched budgets
retain only components that improve verified work or reviewer efficiency
earn one standards-compatible computational transfer
```

The campaign does not assume that Vela, Canopus, an orchestration framework, or
a graph-shaped reader is useful. Negative and null results are valid when they
are rooted, replayable, and honestly scoped.

## Product thesis

Vela is an open scientific-state substrate: version control for what is
claimed, submitted, verified, decided, corrected, and ready to do next.

```text
inspect -> attempt -> submit -> verify -> decide -> continue
```

Producers produce. Verifiers report. Authorized reviewers decide. Events
record. Replay derives Standing. Readers explain.

The longer-range hypothesis is a federated merge and inheritance layer for
science. This campaign does not treat that hypothesis as established. It asks
whether a fresh producer or reader can safely continue from retained state
after the original session, runtime, and read database are removed.

Vela does not replace Git, workbenches, formal systems, workflow engines,
artifact stores, or domain databases. It reuses them and adds only the
scientific transition boundary they do not share.

The familiar product analogy is Git plus a protected pull-request workflow,
with Terraform's preview-versus-apply separation: a candidate scientific
change is inspectable and checkable before an authorized Decision changes
Standing. The analogy stops at the authority boundary. A Git merge, signature,
registration, workflow completion, or verifier pass is not a scientific
Decision.

The product control point is the transition from abundant candidate activity
to exact state that another human or agent may safely inherit. Vela does not
need to own the workbench, model, trace store, database, or publication venue
to own that transition.

## Performance functions

The north star is genuine reusable scientific progress per scarce human
judgment. Three measures stay separate:

1. **Execution lift:** verifier-passing bounded artifacts per all-in cost and
   expert-minute.
2. **State lift:** correct dispositions, correction comprehension, and exact
   replay per reviewer-minute.
3. **Inheritance lift:** time to first useful downstream action, duplicated
   work, and correction awareness after producer substitution.

Run count, graph size, generated Claims, and workflow completion are not proxy
success metrics. Verification is evidence, never acceptance.

The campaign tests four claims independently: Canopus execution lift, Vela
state lift, combined-system efficiency, and cold adoption. Evidence for one
does not establish another.

## Architecture under test

```text
optional producer
  -> bounded Artifact and Claim
  -> independent frozen verifier
  -> Vela Submission and Verification records in canonical Git
  -> one authorized human Decision
  -> current Standing and next bounded action
  -> disposable read projection
```

Only canonical Frontier Git, Vela's replay rules, and the authorized Decision
boundary are durable product architecture. Canopus, evaluator bridges, traces,
Observatory projections, orchestration frameworks, and external-format
adapters must remain removable.

The activity and state planes remain deliberately separate:

```text
models, tools, sessions, attempts, traces, artifacts
                     |
               bounded Submission
                     v
Claim -> Verification -> Proposal -> Decision -> Event -> Standing
```

A rich work episode may be retained as evidence. It is never promoted wholesale
into the portable scientific-state contract.

The repository topology is:

```text
vela                    product monorepo and release owner
vela-web                read-only web product
erdos-frontier          canonical Frontier
formal-conjectures-frontier
quantum-codes-frontier
sidon-frontier
.github                 organization profile and shared GitHub configuration
```

The former `vela-research-harness`, `vela-internal`, `vela-site`,
`vela-frontiers`, and `prover-lane-frontier` repositories are archived. No
replacement integration repository exists.

## Current evidence

| Surface | Current evidence |
| --- | --- |
| Vela product | `0.940.9`; product packages and conformance are green at `13a404bfe7b7f91d900f0e6bad3a4dc0dc0d6342` |
| TypeScript protocol | `@vela-science/protocol@0.1.0`, owned by this monorepo |
| Canopus | `@vela-science/canopus@0.8.0`, owned by `packages/canopus` with unsquashed source history |
| Erdős Frontier | strict replay passes at `6a2b20a4623c0aa3ec667e65452c7aae6210b306` |
| Formal Frontier | strict replay passes at `d5f5355de3588a9a558ee9505e2960e7d138acaf` |
| Sidon Frontier | strict replay passes at `410fd680e1d27a185617b7cf06cd940ef0016369` |
| Quantum Frontier | strict replay passes at `790f255c394b7900e7a1e36a740406d16b783165` |
| Web | read-only clean-clone checks and projection-bound build pass at `bdb10ed92edba5ea1a0e75d44bfb457e0b806f5f`; GitHub runner allocation is externally blocked by the organization billing state |

### Completed real loop

The first campaign loop is terminal and replayed:

- Run `run_55604e5b-4290-4201-bd5f-becae6a0e40d`;
- Submission `vsb_be4ef74c7c4857c9`, root
  `sha256:9bea49924e30670d3b4a08c29059b949bcec3161c448680ff2c364e731424dc7`;
- Claim `vcl_d65fa34573c0a57dff8959ed6b4227999cc03e07fad5f2be9bef184677a7ef8a`;
- Verification `vvr_1974ed5d3e3a72c3`, root
  `sha256:946b09e8dc50dd41bf1fda6733cc04b9ce2faa516e54845c006a90418a5641cb`;
- accepted Decision event `vev_27bf8b3635f8f747`, root
  `sha256:3cae525ba7fb95f4960cf0be3fa86de03660f7308ae3e27fd2e2123caa4dd915`;
- exact bounded range `10429201..10429400`;
- accepted-event delta remained zero until the authorized Decision;
- the Decision is explicitly bounded and does not resolve Erdős problem 1056.

The current Erdős repository has 2,771 accepted Claims, one accepted review,
one rejected review, no pending review, strict replay, and zero blockers.

## Next gate: registered framework-neutral evaluation

The non-normative `canopus.evaluation-plan.v1` freezes tasks, arms, versions,
roots, budgets, retries, scorers, exclusions, custody rules, and publication
policy before usable model output.

Registration binds and rehashes the source snapshot, task packet, verifier,
arm executable, trusted arm wrapper, dependency lock, environment, and each of
the three distinct performance scorers. The runner accepts provider usage only
through its bounded supervisor control channel; a worker-created control file
fails closed. Every registered outcome remains visible; an ordinary failed
process does not truncate the remaining matched cells, and a hard stop must
list every registered cell that was not run.

### Stage A

- Tasks: the next uncovered Erdős range and the first qualifying pinned
  scientific-computing task.
- Arms: native Codex, native Codex with the same packet and verifier, and the
  current Canopus engine.
- Repetitions: two fresh sessions per task and arm.
- Maximum: 12 model calls.

The canonical coverage audit confirms `10429401..10429600` as the first
uncovered contiguous Erdős range. Its exact Frontier packet root is
`sha256:517c16cc9c59d7f91aeaea4287e0ce49000c7545199e86ea632c0a2e91faf30b`;
the answer-free evaluation packet root is
`sha256:51f20cf69e9f1453b9cb58adbc28ad640653f5eeb26adb44c6b3eb34dd5c94ff`.
The existing independent search verifier was rebuilt with those exact bounds
as a deterministic static Linux amd64 binary at root
`sha256:68f64c3dc4bc55e98927f65ba509e5c571944239337864bbf631546ac259cdf4`.
The registered Bun wrapper invokes it only inside the exact pinned,
network-denied, read-only, capability-dropped verifier image. It accepted the
same preflight artifact twice. Its preflight artifact root is
`sha256:baec250e336105634fec8a7ce529c38e85a0bc7085004fecf70023ac5fd0113e`.
The packet contains none of those result bytes.

The first canonically ordered CORE-Bench candidate, `capsule-0201225`, is
ineligible: the retained code is GPL-3.0 rather than permissively licensed, and
the capsule depends on an archived external container image. Its exclusion is
part of the registered selection audit. `capsule-0220918` is ineligible because
both registered evaluation commands bind TensorFlow directly to GPU devices.
The following candidate, `capsule-0238624`, has MIT code and CC0 data but is
also ineligible because its frozen environment and reproduction command
require CUDA, `tensorflow-gpu`, and an NVIDIA runtime. `capsule-0325493`
(`sha256:16e4b41cfce6e94d22c21978a085d3d58274ae8720242b18ae80bce618f51d57`)
is ineligible because its code is GPL-3.0 and its data is CC BY-NC 4.0.
`capsule-0396930`
(`sha256:f472a05b447c7b286936c356c42511c2e2f31cbb971e4e0c685532a92d59c142`)
has MIT code and a CPU-only frozen command, but its data is CC BY-NC-SA 4.0
and therefore also fails the permissive-data rule. `capsule-0940461` has MIT
code and CC0 data, but its source notebook contains retained answer output and
its exact archived amd64 image did not advance beyond kernel initialization in
two network-denied Apple-silicon preflights, including a single-threaded retry.
It therefore fails the no-answer-leakage/current-platform completion gate.

The first qualifying task is `capsule-1108125`
(`sha256:95240472124f26b33ab40a35dad435b27bc4b42f9b6dbc52d6d02248d72d8371`).
Its code is MIT, its data is CC0, and the exact archived image
`sha256:503117b1e393779705fd34c2dbcabfb04fbd65d755887c13137566205418630a`
completed two network-denied, capability-dropped, CPU-bounded replays in 10
and 9 seconds. The three requested means and requested scree direction were
stable. An unrelated Monte Carlo output and three PNG container bytes differed
between replays, so the scorer must bind only the registered scientific result
contract and must not claim whole-output byte equivalence. The model-visible
source bundle must exclude the retained `results/` directory and the
CORE-Bench answer records.

The deterministic answer-safe packet projects exactly 14 allowlisted code and
data files and has root
`sha256:21df72869c39f4d116a5a44760d9105f10400acf17e2946e92176192cc003a2f`.
The independent verifier reproduced the registered result contract with
artifact root
`sha256:cd684ba40c64a445d6ba2e119571dfd8cc85f3a6bd86a0a5174b2d923772ed84`
and verifier-result root
`sha256:36f31e928a854c13bbcaf8d2589e7e952460f08385ce29788509acb4105baba2`.
These are preflight evidence, not model output or scientific acceptance. Stage
A is registered at plan root
`sha256:b62608a4da6f5dfd534eecc980c7019313e12234c3ffef40532170cd2ce947b7`
from monorepo commit `f986031`. The plan binds 33 files and all 12 assignments,
including the three trusted arm wrappers, both task packets and verifiers, the
scorer files, locks, exact executables, resources, and environment manifest.

Both frozen verifier entrypoints replayed their preflight artifacts from the
registration directory. The hostile verifier fixture observed no network,
writable root, writable input, writable artifact, writable capsule, or visible
host home. The hostile native-worker fixture at exact Codex root
`sha256:1da3f4e0e96028b8a771814293c3033dafd1971f943f6c7e79b0897fe705f590`
observed a working shell but no readable source authentication, runtime
authentication, unrelated repository, host canary, process authentication, or
outside-workspace write. Its event-stream root is
`sha256:6d6cac4f41d6bd59fdd8e3b46596783e6b8423ffeca9c5cd95effe22a5e14809`.

Two earlier registrations stopped before model execution after preflight
exposed an incompatible direct Linux verifier and then unbundled verifier
imports. Their immutable directories retain explicit stop records and do not
count as campaign evidence. A completed arm process and a passing task
verifier remain separate rooted outcomes.

The original 100,000-token registration then stopped its first native cell
after 113,985 observed tokens, no candidate ingestion, and no verifier run.
Its run root is
`sha256:c68d8ea8bab6ade8014baa6920e8cc7e06ff1599b22f0ad30173fe448d3e73ab`
and its report root is
`sha256:c878bfde27443a646ee7f188c7b710288fd68d73ec44083fbd1283c8a28cf7a2`.
The user explicitly authorized a higher budget. A root-linked amendment at
plan root
`sha256:bff9d87e2d340ac7dfd18eada1df6d02613a23f4b558f17d1b84bef4bc9c97a5`
registered fresh `A2-*` assignments with a 200,000-token ceiling. Its first
native cell reached 224,005 observed tokens, so the post-run budget guard
rejected the result before candidate ingestion or verifier execution. That
stopped run root is
`sha256:345df46d92ebd86d4219f36df041bbbcd8e0ede4c759e2bbf0425c9f04da26af`;
its report root is
`sha256:559c37f77697f59881efeebd9da06b5fa8ab55bd56cc8df8090c95de2479ba8e`.

Both stopped plans and runs remain campaign evidence. They are not retried or
overwritten. The second root-linked amendment registered fresh `A3-*`
assignments at a bounded 300,000-token ceiling. Its first native cell completed
provider output at 511,024 observed tokens, so the post-run budget guard again
rejected the result before candidate ingestion or verifier execution. Its
stopped run root is
`sha256:c764d57b1de22216afb46a334c1dcd085de692c7fc442203e38f5483aeb35758`;
its report root is
`sha256:3fc5cb5716e137e4cb10496e9073685e315b37599497473f936ddd2e3ba9df8c`.

The three stopped provider outputs show that a tight post-run ceiling prevents
diagnosis because usage is known only after Codex exits and before Canopus
ingests the candidate. A final root-linked `A4-*` amendment may therefore use
a 2,000,000-token safety ceiling per assignment and a 24,000,000-token total
ceiling. Actual observed usage, not the ceiling, remains the efficiency score.
All task facts, model, tools, verifiers, custody, retry, scorer, and publication
rules remain unchanged. The cumulative campaign remains below its
32-model-call ceiling.

### Stage B

Run only after a safe Stage A. Compare plain TypeScript, stateless LangGraph,
and the OpenAI Agents SDK on the same frozen tasks, with two repetitions per
arm and task. Maximum: 12 model calls.

### Stage C

Run only if Stage B produces a candidate winner. Compare that candidate, the
stronger registered native control, and plain TypeScript on held-out tasks.
Maximum: 12 model calls.

An orchestration framework is retained only if it has no hard-gate failure,
wins both task classes, repeats on held-out tasks, and improves the primary
efficiency measure by at least 20 percent over native Codex and plain
TypeScript. Otherwise preserve the result and delete the integration.

## Subsequent gates

1. Run one named mechanically checkable Formal Conjectures mission with a
   frozen Lean statement, toolchain, dependencies, axioms policy, packet, and
   verifier.
2. Compare the `status -> show -> why` read path against Git plus the same
   structured evidence and verifier.
3. Treat the exact Proposal/Scientific Diff and `why` explanation as the first
   product surfaces under test; add a scientific-state comparison surface only
   if a frozen cold-use test
   materially reduces evidence-location or correction time.
4. After measured math value, package one public CPU-only computational
   replication with a source-local adapter and a root-bound RO-Crate export.
5. Require a second genuinely different external format before proposing
   shared adapter infrastructure.

## Product evidence gates

The first reader contract is not a graph or dashboard. It is an exact,
human-readable answer to:

```text
What is claimed?
What changed?
What exact evidence and scoped checks bear on it?
Who decided?
Why does it have this Standing?
What remains safe to do next?
```

Treat the Proposal Scientific Diff and `vela why` as the highest-leverage
product surfaces. Test them against Git plus the same evidence and verifier
before expanding the Observatory. A new read surface must reduce
evidence-location, correction, or continuation time by at least 20 percent in a
frozen cold-use comparison. A new write surface must close a reproduced
authority or workflow gap that the current CLI cannot close safely.

For every proposed feature, record:

1. which transition from activity to inherited state it improves;
2. whether it is canonical or disposable;
3. which existing standard or tool already owns adjacent behavior;
4. the matched test that could falsify its value; and
5. the deletion rule if it does not improve verified output or expert leverage.

Failure deletes or narrows the feature. It does not create a new abstraction.

## Invariants and stop conditions

- Canonical Git Frontiers remain the scientific source of truth.
- Vela owns protocol, replay, Standing, repository authority, and Decisions.
- Canopus is optional and removable.
- The Observatory and Neon remain disposable read projections.
- Agents never invoke a Decision, access repository-authority credentials, or
  enter an authentication or signing trust path.
- No second writer, hosted authority, scheduler, universal ontology, mandatory
  framework, or canonical database is added.
- Component compatibility is generated from release manifests and conformance,
  not maintained by copying version literals across repositories. Exact Runs
  still pin every binary and digest.
- Hidden failures, benchmark leakage, post-output plan changes, unexplained
  root drift, verifier-as-acceptance language, or non-reproducible evidence stop
  the campaign.

Failure to demonstrate lift causes simplification and deletion, not another
architecture layer.
