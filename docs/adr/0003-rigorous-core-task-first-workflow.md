# ADR 0003: Preserve the rigorous core behind a task-first workflow

- Status: Accepted 2026-07-13. Approved by the user for implementation.
- Scope: Vela substrate CLI, protocol plumbing, MCP exposure, and derived review
  projections.
- Implementation state: The technical scaffold was implemented for Vela
  `0.759.0` on 2026-07-14 and its macOS verifier-cleanup race was hardened in
  patch `0.759.1`. The human-ceremony, independent-producer, and
  independent-consumer acceptance gates below remain open. This ADR is an
  engineering decision record, not a signed scientific event or a substitute
  for a Vela policy decision.
- Amendment: 2026-07-14 records the provisional causal policy-head contract
  required to make the existing signed Permit lane replay-safe across policy
  activation, rotation, and revocation.

## Executive decision

Vela will keep its current authority boundary and make the ordinary workflow
task-first. Producers should submit evidence without writing protocol JSON by
hand. Reviewers should see one complete, plain-language Decision Brief. The
human key should authorize the exact immutable decision that was displayed,
with any change to the base state, evidence, policy, or semantic effect forcing
a new review.

This is not a thinner trust model. It is a clerk layer over the existing event
log, reducer, verifier, policy, and key-custody system. The protocol stays
explicit in JSON and audit views while the default path leads with the user's
job.

Vela's stable narrow waist remains content-addressed evidence, proposed
transitions, and authority-bearing events. ADR 0003 adds no authority object,
no scientific event kind, no second receipt format, and no new artifact
family. The causal policy-head amendment adds one versioned governance
proposal contract over the existing `StateProposal` and signed
`review.accepted` primitives; it does not add a policy-head object, signature
family, or event kind. Decision views, staging plans, work sessions,
transaction journals, and adapters are replaceable implementation above or
below that waist.

The ecosystem unit is a bounded frontier loop, not a monolithic science
platform: accepted root, ranked target, private search, selected verification,
receipt landing, scoped decision, replay, and downstream inheritance. The
reference kit and conformance commons make that loop easy to join; Git hosts,
proof libraries, workbenches, courses, wikis, and publication systems remain
independent complementors.

## Research and design precedents

The research question was narrow: how do mature systems reduce ceremony while
preserving informed authorization, stale-state protection, role separation,
and forensic detail?

| System | Useful precedent | Vela consequence |
| --- | --- | --- |
| [Git object model and plumbing/porcelain](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain) | Git puts a VCS interface over a content-addressable store and exposes a small object model plus low-level commands that scripts can compose. Its polished porcelain can evolve without replacing the store. | Keep the replay and content-addressed kernel small. Make task-first CLI and Decision Brief porcelain over shared deterministic functions, not a second state model. |
| [Git alternate indexes](https://git-scm.com/docs/git#Documentation/git.txt-codeGITINDEXFILEcode), [tree construction](https://git-scm.com/docs/git-write-tree), and [safe ref updates](https://git-scm.com/docs/git-update-ref) | Git separates the caller's mutable index, immutable tree and commit objects, and compare-and-swap ref movement. Its plumbing is explicitly intended for alternative porcelain and stable scripting. | Build a Vela publication from one explicit resolved path set without consuming the caller's unrelated staged work. Bind it to a named ref and expected full Git commit OID, inspect the candidate tree before moving the ref, and fail on concurrent Git movement instead of silently merging. |
| [CodeGraph](https://github.com/colbymchenry/codegraph) | A local Tree-sitter and SQLite index turns a repository into a queryable symbol graph, incrementally follows file changes, marks the brief stale window explicitly, and exposes one high-leverage MCP query by default. | Treat semantic search, impact analysis, and agent context as removable derived indexes keyed to source and Vela roots. Show freshness and source spans. Do not make the local graph a new authority store. |
| [Graphify](https://github.com/Graphify-Labs/graphify) | A multimodal graph can combine code, schemas, documents, and media, distinguish extracted, inferred, and ambiguous edges, and serve several agent clients. Its team workflow also illustrates the cost of committing and union-merging generated graph state. | Preserve evidence class and inference confidence in derived views, but regenerate graphs from canonical inputs. Never auto-merge an inferred graph into accepted state or require a committed graph cache for replay. |
| [DeepWiki](https://docs.devin.ai/work-with-devin/deepwiki) and [LLM Wiki](https://github.com/lucasastorian/llmwiki) | Repository wikis and self-maintaining knowledge bases generate navigable prose, diagrams, links, and agent context from source. The local LLM Wiki pattern keeps generated Markdown inspectable and its search cache explicitly rebuildable. | Let wikis consume Vela read contracts and cite the exact Git tree, event-log root, and evidence sources they summarize. Generated prose is a projection or proposal, not accepted scientific truth. |
| [The UNIX Time-Sharing System](https://onlinelibrary.wiley.com/doi/abs/10.1002/j.1538-7305.1978.tb02136.x) | Compatible file, device, and inter-process I/O let many languages and small subsystems compose without one program owning the whole environment. | Use ordinary files, streams, digests, exit status, and versioned JSON at adapter boundaries. Do not require a Vela-native notebook, language, compute service, or presenter. |
| [Internet architectural principles](https://www.rfc-editor.org/info/rfc1958/) and [the simplicity principle](https://www.rfc-editor.org/rfc/rfc3439.html) | A small spanning set, a hardware-independent inter-networking layer, end-to-end responsibility, modularity, and a minimalist waist supported unplanned applications at global scale. | Put domain intelligence and presentation at the edges. The waist carries references, evidence claims, proposed transitions, scoped authority, and replay, not a universal model of every science. |
| [Linux userspace ABI discipline](https://www.kernel.org/doc/html/latest/admin-guide/abi.html) | Stable external interfaces are distinguished from testing and internal interfaces, allowing the implementation to change while applications retain a dependable contract. | Version and test Receipt v1, event replay, CLI JSON, and packet projections. Mark adapters experimental until real users depend on them; do not freeze internal Rust module boundaries as protocol. |
| [Stripe idempotent requests](https://docs.stripe.com/api/idempotent_requests) and [Payment Intents](https://docs.stripe.com/payments/payment-intents) | Stripe makes a difficult regulated workflow usable through safe retries, one lifecycle-bearing intent per customer session, consistent errors, testing, and versioned APIs. The object retains failed attempts without permitting duplicate charges. | Make the common path one intent and one result, make retries exact, keep failed scientific attempts inspectable when deliberately deposited, and invest in fixtures and error quality. Do not copy Stripe's centralized authority model: Vela must remain forkable, offline, and plural in authority. |
| [Amazon S3 object model](https://docs.aws.amazon.com/AmazonS3/latest/userguide/Welcome.html) | A small object interface became a substrate for unplanned applications. Its explicit boundary is equally useful: an update is atomic at one key, not across arbitrary keys, and a locator or ETag is not always an immutable content identity. | Keep content digest separate from retrieval locator, make one frontier the transaction unit, and do not invent distributed atomicity merely because one command can address several repositories. |
| [OCI image and distribution specifications](https://github.com/opencontainers/image-spec) | A content-neutral registry protocol composes media type, size, digest, manifests, subjects, and referrers while runtimes remain separate. Multiple tools share the format without one engine owning execution. | Reuse digest, descriptor, and subject-style references at adapter boundaries. Let registries carry scientific artifacts when useful, but keep Vela acceptance and replay independent of any registry. |
| [OpenTelemetry](https://opentelemetry.io/docs/) | A vendor-neutral instrumentation contract lets many runtimes emit correlated traces, metrics, and logs to replaceable backends. It succeeds by standardizing signals and context rather than one observability product. | Treat workbench and agent telemetry as upstream activity. Import only digest-bound, decision-relevant evidence and provenance; do not turn Vela into a trace warehouse. |
| [CloudEvents](https://github.com/cloudevents/spec/blob/main/cloudevents/spec.md) | A small event envelope separates core context, extensions, encodings, and transport bindings so events can cross vendors without one broker owning semantics. | Offer CloudEvents as an export adapter when demanded. Do not replace canonical Vela events with a carrier that supplies neither scientific authority nor integrity. |
| [OAuth 2.0](https://www.rfc-editor.org/info/rfc6749/) and its [security best current practice](https://www.rfc-editor.org/info/rfc9700/) | Separating resource owner, client, authorization server, and resource server enabled scoped delegation, while years of security hardening show the cost of excess flexibility and ambiguous binding. | Keep authentication, tool authorization, evidence, and scientific acceptance as four distinct facts. Prefer few profiles, least privilege, exact transaction binding, and fail-closed defaults. |
| [Hugging Face Hub repositories](https://huggingface.co/docs/hub/en/repositories) and [model cards](https://huggingface.co/docs/hub/model-cards) | Domain infrastructure grew by keeping models and datasets in Git repositories and layering discoverability, cards, evaluations, and APIs over the workflow people already used. | Let tools generate receipt metadata as a byproduct and keep source in its original repository. Treat cards as attributed producer claims, never as a verifier or authority verdict. |
| [SQLite](https://www.sqlite.org/about.html) | A serverless, zero-configuration, transactional library with a stable cross-platform file format gives immediate local value and survives application churn. Its documentation is also explicit about where a client-server database is the better tool. | Preserve clone-and-run, offline, no-daemon use and long-lived readable bytes. Choose infrastructure by scope instead of growing a hosted control plane to solve local state. SQLite may serve private indexes, but it does not replace canonical event replay. |
| [Kubernetes custom-resource guidance](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/) | Declarative resources plus controllers are powerful, but Kubernetes explicitly warns against using custom resources for high-volume application or monitoring data and notes that cross-object transactions are not the model. | Reconciliation is a useful analogy for materialized views. CRD proliferation is the warning: high-volume activity stays outside the authority API, and new Vela kinds need demonstrated replay or interoperability value. |
| [W3C Secure Payment Confirmation](https://www.w3.org/TR/secure-payment-confirmation/) | Authentication alone does not prove that transaction details were shown. SPC defines the details a trusted surface presents and includes transaction data in signed client data. | The signing key must bind the exact reviewed decision, not merely prove that the reviewer was present. |
| [NIST SP 800-63B, authentication intent](https://pages.nist.gov/800-63-4/sp800-63b.html#authentication-intent) | An explicit response establishes intent to authenticate. It does not by itself establish informed consent to an arbitrary semantic change. | One confirmation and one key read are necessary, but the reviewed effect must also be exact and inspectable. |
| [Terraform saved plans](https://developer.hashicorp.com/terraform/tutorials/cli/plan) and [stale-plan handling](https://developer.hashicorp.com/terraform/enterprise/workspaces/run/cli) | A saved plan separates preview from application, applies only the planned changes, and becomes unusable when its base state is stale. | Prepare a content-addressed decision manifest, render from it, and refuse to sign if its inputs no longer match. |
| [GitHub pull request reviews](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/about-pull-request-reviews) and [merge queues](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue) | Reviews can be dismissed after new commits, and queued work is tested again against the current base before merge. | Proposal, head, policy, verifier, or effect drift invalidates review. Revalidation may confirm the same decision but may not silently change it. |
| [The Update Framework roles](https://theupdateframework.io/docs/metadata/) | Separate roles and threshold rules limit the damage of one compromised key. | Evidence production, verification, policy, human judgment, and publication remain distinct capabilities. |
| [in-toto layouts and links](https://in-toto.io/docs/getting-started/) | The owner defines the expected supply-chain steps while functionaries produce signed evidence about what ran. | A producer receipt and verifier run are evidence for an owner-defined acceptance rule, never acceptance by themselves. |
| [Sigstore's security model](https://docs.sigstore.dev/about/security/) | A valid identity, signature, and transparency entry prove provenance properties, not that an artifact is correct or desirable. | Signing and publication must not be presented as scientific verification or quality judgment. |
| [Command Line Interface Guidelines](https://clig.dev/) | Human output, stable structured output, dry runs, actionable errors, and safe non-interactive behavior are separate interface contracts. | Every machine mode emits one complete object with no prose. Human mode explains the state and gives one exact next action. |
| [GOV.UK Check answers](https://design-system.service.gov.uk/patterns/check-answers/) | A compact summary supports correction before submission, with details available where they affect the choice. | The final confirmation shows the complete decision set and lets the reviewer edit an item without restarting the ceremony. |
| [Hypothesis Evolution Protocol](https://arxiv.org/abs/2607.09195) | An explicit hypothesis-test-evidence-belief loop with persistent hypothesis IDs and append-only evolution records changed agent behavior in a small exploratory study. The agent still assigned its own belief scores and assessed evidence validity; independent validation remains future work. | Preserve attempt, hypothesis, evidence, and refinement lineage in the activity plane and Decision Brief. Never promote model belief, a probability threshold, or model-assessed validity into Vela authority. |
| [OpenResearch](https://openresearch.sh/) | An arXiv paper can seed an agent-built minimal reproduction in ordinary compute, with projects exposing experiment graphs, runs, and reports. | Treat reproduction systems as producers and adapters. Content-address their environment, inputs, commands, outputs, and reports in a Vela receipt rather than rebuilding their compute plane. |
| [Proving, fast and slow](https://isabeldahlgren.github.io/proving-fast-and-slow/) | A scenario of AI-native mathematics centers shared repositories for canon, preliminaries, open problems, Lean artifacts, many specialized agents, and multiple human-facing presentation forms. | Make Vela the interoperable state and authority layer beneath article machines, repositories, proof assistants, and presenters. Do not require them to become one Vela-native authoring environment. |
| [Lean's ecosystem roadmap](https://lean-lang.org/fro/roadmap/y3/), [mathlib contribution guidance](https://leanprover-community.github.io/contribute/index.html), and [Reservoir](https://reservoir.lean-lang.org/) | Lean compounds through a small proof-checking and language foundation, a stable standard library, a large community library, Lake packages, an indexing and build service, IDEs, automation, benchmarks, and independent projects. Mathlib explicitly sends fast-moving or out-of-scope theories to standalone repositories while preserving a path to upstream durable definitions. | Aim for the same shape: a small Vela kernel, a conformance commons, ordinary Git frontiers, reusable verifier packages, and independent tools. Let frontier work move quickly without forcing every domain object into core; upstream only contracts proven by repeated use. |
| [Zooniverse](https://www.zooniverse.org/about) | People without specialized backgrounds contribute to real research by choosing a project, learning one bounded classification task, submitting work, and discussing surprising cases with the research team. Multiple independent contributions are combined, while project approval, analysis, and publication remain distinct roles. | Make the first Vela target bounded, authentic, teachable, and status-clear. Preserve contributor credit and independent replication, but distinguish proof of participation from verification and acceptance. Community and mentoring may surround the frontier without becoming protocol authority. |
| [LeanDojo v2](https://leandojo.org/leandojo.html) | Repository tracing turns Git-hosted Lean projects into proof-state, premise, and training data; a programmatic interaction layer lets many agents observe states and test tactics against the same proof environment. Its derived datasets and retrieval machinery can evolve independently of Lean's trusted kernel. | Let AI training, search trees, proof-state traces, and retrieval indexes remain high-volume activity and derived data. A Vela adapter should deposit selected results and exact roots at the boundary, not import the entire inner loop or make an agent framework authoritative. |
| [Formal Conjectures](https://github.com/google-deepmind/formal-conjectures) | A living Git repository of open Lean statements tracks tagged mathlib releases, keeps potentially upstreamable definitions in a library-shaped area, and publishes immutable benchmark snapshots whose version changes when statements or formalizations change. It also states that a checked formal statement can still misrepresent the source conjecture. | Separate a fast-moving frontier from immutable evaluation snapshots. Pin language and library roots, preserve corrections as new versions, and expose semantic-faithfulness review separately from kernel checking. Mature reusable definitions should flow to their domain library rather than becoming Vela-owned. |
| [AlphaEvolve](https://deepmind.google/blog/alphaevolve-a-gemini-powered-coding-agent-for-designing-advanced-algorithms/) and [AlphaProof](https://deepmind.google/blog/ai-solves-imo-problems-at-silver-medal-level/) | Evaluator-driven systems can run a rapid propose, execute, score, select loop over program databases and proof environments. Their strongest domains have cheap systematic checks, yet formal problem translation and final integration still require scarce human and domain judgment. | Optimize Vela around the boundary of a fast evaluator loop: pin the task and verifier, retain selected positive and informative negative checkpoints, route narrow exact classes by signed policy, and reserve people for semantic fidelity and exceptions. Do not append every search mutation to the authority log. |
| [Diderot](https://projectdiderot.com/about) | An open-source mathematics preprint server requires authorship transparency and attaches human-issued certificates for tool disclosure, proof review, formal verification, and citation review. Its formal certificate separates machine checking from human judgments of statement and argument faithfulness. | Treat Diderot as a publication and attestation peer. Import or export papers, disclosures, repositories, comparator results, and scoped certificates with their issuer roles intact; do not collapse a Diderot certificate into Vela acceptance or rebuild its preprint server. |
| [Leiden Declaration on Artificial Intelligence and Mathematics](https://leidendeclaration.ai/) | The IMU-endorsed declaration emphasizes tool and resource disclosure, attribution, independent verification, human responsibility for correctness, open science, community review, and public computational infrastructure. It warns that plausible automated arguments and formalizations can still be wrong or semantically unfaithful. | Make provenance, licensing, tool disclosure, references, formal versus informal scope, and human accountability inspectable. Keep significance, correctness, authorship norms, and ethical judgment outside automated Vela scoring. |
| [RO-Crate](https://www.researchobject.org/ro-crate/specification.html) and [Software Heritage identifiers](https://www.softwareheritage.org/software-hash-identifier-swhid/) | Existing standards already describe research bundles and give archived source content-derived persistent references. | Support them as import, export, and immutable-content-locator adapters. Do not replace canonical Vela serialization, object IDs, or event replay with a general metadata graph. |
| [Principles of Open Scholarly Infrastructure](https://openscholarlyinfrastructure.org/) | Durable shared infrastructure needs stakeholder governance, transparent operations, open source, open standards, data access within legal and ethical limits, preservation, and a credible exit path. | Put governance, sustainability, preservation, and a living-will plan on the project roadmap. Keep them out of scientific event semantics, but treat proprietary lock-in as an architectural risk. |

The HEP preprint is useful product and protocol-design evidence, not a trust
anchor. Its experiment used three runs per condition on three materials tasks,
and its code was not yet available with the preprint. The durable lesson is
that explicit, inspectable state changes how agents work. Its self-reported
belief probabilities and model-led evidence judgments are exactly the kind of
claims Vela must keep on the activity and evidence side of the boundary.

### Internal research record

Recent Constellate memos were treated as design input, not canon. Their claims
were checked against the live repository where they affected this decision.

| Memo | Durable idea used here | ADR response |
| --- | --- | --- |
| *Scientific Frontier Briefs: Full Vela Synthesis* | Vela already has a narrow authority waist. Evidence Diff should be the flagship review object, pending state must be portable, and sensitive activity should be referenced rather than copied into public history. | Build the clerk layer around the existing kernel, make Decision Brief the shared review projection, and define explicit evidence storage modes. |
| *Vela, Agentic Git, and the Entire Cursor Origin* | Review a state transition, not a transcript; bind it to its base and evaluators; keep operation recovery separate from append-only accepted history; keep privacy and status axes explicit. | An internal Decision Plan binds base, evidence, policy, and effect. A private operation journal handles crashes without becoming scientific authority. |
| *Vela Human Governance Memo* | Strong protocol can reduce visible ceremony. Review cards should explain the exact judgment, evidence, change, and consequence; batches must be homogeneous and fully shown. | Use progressive disclosure, no blind batch action, and one exact final set before key access. Older passkey and keyless ideas remain outside this ADR. |
| *Receipt v1 Policy Options* and *Receipt Article / Machines Memo* | A receipt should be produced by the system, not performed by the producer. Evidence, identity, verifier output, and acceptance authority must remain separate, and new schema should answer a demonstrated need. | Give the supported authoring path a small private input while preserving the released Receipt v1 wire shape. Build the complete valid receipt behind CLI flags and adapters. Do not publish a second draft wire format, loosen v1 under the same schema identifier, or fabricate identities, verdicts, or attestations. |
| *Vela Science State Memo* and *Verifier Coverage Night Findings* | A readable lifecycle must not flatten verification, acceptance scope, provenance, or known coverage gaps. | Use subject-scoped work and decision journeys while retaining independent verification, authority, and publication axes. |
| *Science and Education / Vikings Summer Camp* (2025-09-14) and the local compiled memo wiki | Scientific access has three coupled constraints: education, tools, and community. The motivating experience is learning by doing real projects, with templates, mentors, and contribution roles, while AI removes mechanical work and lets small human teams direct many agents. | Treat an understandable, bounded frontier contribution as the adoption unit. `next`, `work`, examples, and frontier kits should help a newcomer make a real pending contribution without granting authority. Keep discovery, teaching, mentorship, and community as complementor surfaces rather than turning the trust kernel into a social or education platform. |
| *Oak St: Open Science Protocol* (2025-08-10), *Scientific Knowledge Protocol* (2026-04-10), and *Unified Vision* | The broad ecosystem separates durable state, distribution and community, and domain runtimes. Massive agent search benefits from cheap early evaluators, but the Vela wedge is the state and governed-transition layer rather than the whole scientific operating system. | Keep high-volume search in runtimes, carry selected checkpoints through one receipt waist, and let Git hosts, registries, wikis, archives, and publication systems provide network surfaces. The layer map is an interoperability map, not a Vela build list. |
| *Science Ecosystem* Star Notes (2026-04-19) | The notes pair “keep simple and primitive” with the challenge “is this all just a wrapper around Git?” They also distinguish an LLM wiki from the deeper requirement that knowledge compound. | State the edge precisely: Git records byte history; Vela binds a proposed scientific change to evidence, a selected verifier, scoped authority, replayed frontier state, and correction lineage. Keep the LLM wiki source-linked, freshness-labeled, and disposable. |

Together, the external research and internal record point to thirteen rules:

1. Keep evidence, authority, accepted state, and transport separate.
2. Display a semantic effect before raw protocol detail.
3. Authorize an immutable prepared decision, not a live query whose result can
   change between review and signature.
4. Treat stale state as a reason to review again, not as an inconvenience to
   bypass.
5. Reveal detail progressively: Decision, Evidence, then Audit.
6. Make ceremony proportional to risk and batch only a coherent, fully shown
   set.
7. Give humans a concise terminal surface and machines one versioned, complete
   JSON projection.
8. Measure usability with outside users because a workflow that invites
   bypasses is a security defect.
9. Preserve failed attempts and hypothesis evolution as provenance, while
   keeping agent belief and verifier evidence distinct from authorization.
10. Add a stable public primitive only when existing objects and relations
    cannot express the need and independent producers and consumers have
    demonstrated it.
11. Put platform and domain intelligence in adapters or typed facets, not in
    the authority kernel.
12. Design the queue, verifier edge, and renderer for mostly machine-produced,
    potentially hostile input rather than assuming scarce, well-behaved
    submissions.
13. Optimize for the first meaningful contribution and the next inherited
    iteration, not merely for protocol completeness or submission volume.

### Adoption and interoperability constraint

Vela is scaffolding over the scientific tools people already use, not a
replacement for them. Git continues to store and transport immutable bytes.
Ordinary programming languages, notebooks, proof assistants, containers,
workflow engines, repositories, arXiv-like archives, and domain instruments
continue to produce and present work. Vela adds a small shared vocabulary for
activity, evidence, proposals, policy, decisions, and replayed state where
those systems currently disagree or lose context.

This is also the answer to the network-effect problem. Adoption may begin with
one repository, one adapter, or one receipt and must provide value without
moving the author's source, rewriting accepted history, changing the compute
or publication platform, or requiring collaborators to use a hosted Vela
service. Every integration should prefer a content-addressed reference and a
thin adapter over a new execution engine or file format. A proposed protocol
extension needs a demonstrated outside consumer; otherwise it stays in the
activity metadata or adapter layer.

The long-term opportunity is larger than review ergonomics: a common state
layer lets specialized article machines communicate claims, tests, artifacts,
counterevidence, and lineage to other humans and machines. ADR 0003 supplies
the reliable handoff and authority boundary for that ecosystem. It does not
attempt to define the entire future scientific object model.

Democratization here means widening the ability to learn, inspect, reproduce,
challenge, and contribute. It does not mean that a novice, an agent, or a
platform can erase domain expertise, laboratory access, biosafety, dual-use
controls, clinical responsibility, or the authority of the community that
maintains a canon. Vela's useful asymmetry is that contribution can be cheap
while acceptance stays scoped and accountable. A middle-school student, an AI
lab, and a senior mathematician can all land evidence through the same public
waist; the frontier's verifier and signed authority still determine what that
evidence changes.

### Modern convergence and tailwinds

Several independent shifts now make this layer more timely:

- [Claude Science](https://www.anthropic.com/news/claude-science-ai-workbench)
  combines agents, scientific tools, local or HPC compute, auditable artifacts,
  reviewer agents, and persistent sessions. [GPT-Rosalind](https://openai.com/index/introducing-gpt-rosalind/)
  connects domain models to Codex, APIs, and more than 50 scientific tools and
  data sources. These products make the workbench and its private trajectory a
  valuable surface, which increases the need for accepted state that can leave
  one vendor.
- [OpenResearch](https://openresearch.sh/) and containerized reproduction make
  more papers executable. Formal kernels, deterministic data access, and
  domain simulators provide cheap verifiers for a growing subset of work. The
  missing link is a portable record of exactly what ran, what it established,
  and what a scoped authority decided.
- Content-addressed registries, Git, OCI, RO-Crate, Software Heritage, and
  vendor-neutral telemetry reduce the cost of referencing heterogeneous
  artifacts without mirroring every upstream system.
- Code graphs, generated repository wikis, local semantic indexes, and MCP
  make previously implicit structure cheap for agents to query. This increases
  the value of one stable, source-linked read contract and the danger of
  mistaking a stale or inferred projection for canonical state.
- Long-running agents and specialized article machines make activity abundant
  while qualified review remains scarce. A deterministic state and exception
  layer becomes more valuable as generation cost falls.
- Institutions increasingly need governed context, identity, data access,
  audit, and export. Those generic services can supply lower layers; Vela can
  remain focused on scientific transitions and scoped inheritance.
- Diderot, the Leiden Declaration, and POSI point in the same social direction:
  disclose tools and roles, preserve human responsibility and review, use open
  infrastructure, and keep an exit path from any one provider.

The counterwinds are equally important. Workbench vendors can add provenance;
generic governance systems can own admission; rich semantic systems have
historically suffered negative network effects; and a reviewer queue can
become a graveyard. Vela only earns a role if one outside producer can emit a
useful receipt with almost no extra work, one reviewer understands the proposed
change faster than from raw files, and an independent consumer gains value from
the accepted root.

### Clean-slate test and network-aware path

If science were designed now, with abundant computation and machine agents, a
paper would be one projection of a continuously updated record rather than the
only durable unit. Claims, attempts, tests, evidence, dependencies,
counterevidence, corrections, and scoped decisions would remain addressable.
Verification would rerun when an input changed. Humans would spend attention on
semantic fidelity, significance, exceptions, and responsibility rather than
reconstructing provenance from prose and folders.

That clean-slate model is a direction test, not a migration plan. The
network-aware path starts with a repository, notebook, proof, container,
preprint, or lab export that already exists. Vela references its bytes, records
the proposed scientific transition, and exposes derived state without taking
over the producer's authoring, compute, collaboration, or publication system.
One user in one Git repository must receive value before any collaborator,
hosted service, or platform adapter is required.

### Load test: mostly machine-produced science

ADR 0003 treats "99% of output is AI-produced" as a stress case, not a numeric
forecast. When generation is cheap, the scarce resources become independent
verification, semantic review, deduplication, correction, consolidation, and
human attention. Vela therefore optimizes for:

- deterministic checks before human review;
- signed Permit policy for narrow, repeatedly validated classes and typed
  Defer for real exceptions;
- exact-retry deduplication without discarding independent replication;
- explicit replication versus methodological diversity;
- negative-result and correction retention;
- bounded queues, payloads, rendering, and resource use;
- reviewer credit as provenance, without turning prestige into authority; and
- downstream reuse and correction latency rather than raw receipt volume.

Producer text, artifact names, logs, locators, and imported metadata are
untrusted input. Review never automatically fetches a locator. Human rendering
escapes terminal controls, bounds every list and text field, makes truncation
explicit, and preserves a digest-addressed full form for audit. Admission
limits bound JSON depth, byte size, artifact count, decompression, verifier
work, and per-actor queue pressure. Priority may use policy-defined impact,
staleness, uncertainty, reviewer capability, and expected loss, but not model
confidence or prestige as truth signals.

This waist should permit applications Vela does not implement: reproduce-on-
arrival bots, dependency and correction watchers, article-machine handoffs,
alternative paper, site, and graph renderers, reviewer-credit views, and scoped
training or benchmark corpora. Each should be buildable from a clone, ordinary
content references, and stable read contracts without a new kernel kind.

### Ecosystem shape: kernel, commons, frontier kits, and complementors

Git, Linux, and Lean did not become ecosystems by absorbing every useful tool.
They made a small foundation dependable, exposed boundaries that independent
builders could rely on, and let libraries, packages, hosts, editors, and
workflows compete above it. Vela should follow that shape.

| Layer | Responsibility | Stability rule |
| --- | --- | --- |
| Existing substrate | Git stores and transports bytes; Lean and other languages check domain artifacts; containers, workflow engines, object stores, notebooks, and archives execute or present work. | Vela composes these systems and records exact roots. It does not replace them. |
| Vela kernel | Receipt v1, content references, proposals, signed-policy or human-signed events, deterministic replay, correction, and scoped inheritance. | Small, portable, backward-compatible, and sufficient from a clone. No model, hosted service, graph, or package index is in the trust path. |
| Frontier kit | An ordinary Git tree containing a bounded problem, pinned dependencies, selected verifier instructions, task templates, examples, and domain guidance assembled from existing Vela contracts. | Packaging convention, not a new protocol object. A kit can fork, move, or disappear without changing receipt or event meaning. |
| Complementors | Article machines, theorem provers, lab and workflow systems, OpenResearch, Diderot, graph and wiki tools, renderers, correction watchers, training pipelines, and hosted collaboration. | Replaceable producers and consumers. They use released read and write contracts and never acquire authority by integration. |
| Commons | Conformance fixtures, replay corpora, verifier honesty cases, immutable benchmark snapshots, interoperability examples, security advisories, governance, and preservation. | Openly forkable and independently runnable. A hosted hub may index the commons, but it may not become required for replay, acceptance, or export. |

The unique edge over Git is narrow and testable. Git answers which bytes and
trees changed and how they are related. Vela answers which scientific change
was proposed against which accepted root, which evidence and selected verifier
supported it, which scoped authority permitted or decided it, what replayed
frontier state followed, and which descendants must be reconsidered after a
correction. Everything that does not need that contract should remain ordinary
Git data or an external tool.

The released ecosystem contracts are deliberately few and carry explicit
stability labels:

1. canonical Receipt v1 and typed content references;
2. signed event, policy-certificate, correction, and replay semantics;
3. versioned CLI JSON and the testing-stage Decision Brief read contract; and
4. ordinary Git frontier layout, exact-root export, bundle recovery, and the
   conformance corpus that proves all of the above.

Pinned input, sandboxed execution, bounded output, and private receipt
construction are the required shape of Vela-owned verifier adapters, not a
second public wire format. An independently shipped producer or verifier emits
canonical Receipt v1. Any bounded runner result used inside a first-party
adapter is private and versioned with that adapter.

Internal Rust modules, local session storage, telemetry, graph schemas, wiki
formats, ranking strategies, user interfaces, and hosted APIs are not stable
ecosystem contracts. They should be easy to replace until independent users
prove otherwise.

The division of labor is equally important. Frontier maintainers define the
bounded question, selected evaluators, and authority policy. Verifier authors
state exactly what a check establishes and omits. Producers create evidence.
Reviewers spend judgment on semantic faithfulness, significance, conflicts,
and exceptions. Library maintainers absorb mature reusable definitions and
methods. Adapter and renderer authors create new views and workflows. Vela
preserves their attribution and inputs; it does not turn reputation into a
truth score.

Discovery should begin as a read-only index over signed frontier metadata,
ordinary repositories, verifier descriptors, and conformance results. It is
not a central plugin marketplace. Installing a verifier or adapter means
running untrusted code, so any later package experience needs pinned source,
license and maintainer identity, declared capabilities, sandboxing, reproducible
build evidence, and an exit path. It should be designed only after multiple
independent packages expose a repeated problem.

### Frontier flywheel: optimize the closed loop, not the ledger

The product measure is time to the next trustworthy iteration. Vela should not
record every token, tactic, mutation, or failed process from a high-volume
search system. It should make the durable crossings between loops cheap and
exact.

| Loop | Typical cadence | System of work | What crosses into durable shared state |
| --- | --- | --- | --- |
| Search | Seconds to hours | A proof agent, article machine, simulator, notebook, lab system, or evolutionary program database proposes and evaluates many candidates. | Nothing by default. Private traces and caches remain with the producer. |
| Deposit | Minutes to hours | `next` selects a target, `work` pins the task and lease, a frozen verifier runs, and `land` normalizes the result. | One Receipt v1 with exact roots, result, method, caveats, and selected positive or deliberately informative negative evidence. |
| Decision | Immediate under a narrow signed Permit, otherwise hours to days | Deterministic policy routes the proposal. A human reviews one Decision Brief only for Defer or an explicit human-owned lane. | One exact authority-bearing event or signed-policy certificate. Deny leaves no canonical submission delta. |
| Inheritance | Immediately after accepted state changes | Replay materializes the new root, recomputes ranked tasks, and downstream tools consume it. Correction watchers re-evaluate affected children. | A child cites the accepted root and adds a substantive result, creating the next iteration. |

The reference loop is therefore:

```text
accepted root -> next -> work -> search and verify -> land
              -> Permit or Defer -> sign only when human judgment is required
              -> replay and materialize -> recompute next -> downstream child
```

A sharp frontier has one bounded target, one exact base and dependency set, one
selected evaluation path, explicit success and informative-failure outputs,
one scoped authority policy, and a clear rule for deriving the next work. These
facts should be assembled from existing task, verifier, receipt, policy, and
frontier fields. They do not justify a new `FrontierKit`, `Loop`, or `Benchmark`
authority object.

Lean illustrates both halves of this design. Kernel checking makes proof
verification cheap enough for tight inner loops, while statement translation,
library design, generality, and maintainability remain scarce review work.
Mathlib, standalone packages, Reservoir, Formal Conjectures, and AI prover
frameworks can all evolve at different rates because they share pinned
language and library roots. Vela should record which root and check supported a
transition, preserve formalization and correction lineage, and let mature
definitions flow upstream. It should not own the proof search, the theorem
library, or the meaning of the mathematical statement.

This ecosystem is working when independent producers and consumers shorten the
loop without a kernel change: a new verifier can serve several frontiers; a
new workbench can deposit the same receipt; a correction reaches dependent
work; a result becomes a reusable library contribution; an offline fork still
works; and a tool the Vela maintainers did not design can build on an accepted
root. Package count, generated output, and receipt volume are not ecosystem
success metrics.

### Primitive budget

| Category | ADR 0003 budget |
| --- | --- |
| New truth-bearing primitive or authority kind | Zero new object, event kind, or signature family. The causal policy-head amendment below is a versioned relation over existing `StateProposal` and `review.accepted` primitives and is not yet a stable ecosystem primitive. |
| New scientific or coordination event kind | Zero |
| Receipt wire formats | One: backward-compatible Receipt v1 |
| New artifact family | Zero: extend or reference existing artifacts |
| New stable read projection | At most one testing-stage `DecisionBrief` JSON contract |
| Private mechanisms | One operation journal, `ReceiptBuilder`/`DecisionFacts`/`DecisionPlan`, one shared state `FrontierTxn`, and a separate Git publication transaction; names and layouts are not protocol |
| Domain integrations | External adapters and conformance fixtures only |

A future stable public primitive must satisfy all five tests:

1. Existing objects plus a typed relation or derived view cannot express it.
2. At least two independent producers and two independent consumers need it.
3. It belongs in replay, authority, or the interoperability waist.
4. Its canonical semantics can evolve backward-compatibly.
5. It removes more concepts than it adds.

### Amendment: causally select the policy allowed to Permit

The signed policy file was necessary but not sufficient authority for an
unsigned policy-lane event. An `AcceptancePolicy` and its signature prove that
a human authorized those exact policy bytes. They do not prove which signed
policy was active at a particular causal point in the frontier, whether a
newer policy superseded it, or whether it had been revoked before a later event
was appended. The mutable `.vela/policies/active.json` selector cannot supply
that missing fact: copying an old signed policy into that path, or choosing an
earlier self-asserted event timestamp, must not reopen a retired Permit lane.

Git order, file modification time, the event's unsigned timestamp, and the
policy signature cannot prove the missing fact. Git remains transport rather
than scientific authority, and an unsigned policy-lane event can be fully
readdressed after changing its own timestamp and payload. A new
`policy.activated` event or policy-head object would duplicate primitives Vela
already has. The smallest sufficient addition is a closed, typed governance
proposal whose human acceptance has canonical signature, identity,
content-addressing, and replay semantics.

`governance.policy_head` is a `StateProposal` targeting this frontier with
`target.type = "governance"` and payload schema `vela.policy-head.v1`. The
payload contains:

- `action`: `activate`, `rotate`, or `revoke`;
- `policy_id`: the selected `vap_` policy for activate or rotate, and absent
  for revoke;
- `prior_head_event_id`: absent only for the first activation and otherwise
  the exact preceding head event;
- `expected_parent_event_log_root` and sorted, unique `parent_event_ids`, which
  together commit the exact canonical replay prefix preceding the decision;
  and
- a strictly increasing head-chain `epoch`.

The head-chain epoch counts governance transitions. It is independent of
`AcceptancePolicy.epoch`, which versions the policy's own rule lineage. Vela
does not require those two numbers to match and does not infer causal
activation from the policy's internal epoch.

The proposal does not become authority by existing. Acceptance requires a
registered, non-revoked `reviewer:` or `steward:` Ed25519 actor and a real
human event signature. It emits the existing signed
`review.accepted` event targeting the proposal. That review event is the
proposal's applied event; no second domain event and no new event kind are
emitted. Agent, CI, MCP, keyless, and custody-only paths cannot accept a policy
head. A head also does not validate or embed policy rules: every Permit still
requires the retained policy bytes, content address, human policy signature,
signer authority, and deterministic evaluation to verify.

The supported `vela policy sign` and `vela policy revoke` ceremonies use the
existing single-frontier `FrontierTxn` write edge. After the recovery barrier,
the command reloads the selected policy, actor registry, and current head,
rejects drift from the policy shown to the reviewer, samples one fixed time,
then invokes the key loader exactly once. That one in-memory key signs the
policy envelope when opening and the existing `review.accepted` head event;
revocation is likewise a real keyed review. Policy snapshots, active signature
creation or deletion, the revocation marker, proposal, and review event share
one recoverable plan. A stable intent ID resumes a prepared or committed plan
from its public journal bytes without reading the key or clock again. Private
key bytes are never journaled.

Replay derives one linear head chain and fails closed on any ambiguity:

1. Epoch 1 is `activate`, names one `vap_` policy, and has no prior head.
2. When epoch n selects a policy, epoch n+1 is `rotate` or `revoke`, names
   epoch n's exact signed head event, and includes that event in its exact
   causal prefix. Rotate must select a different signed `vap_` policy. Revoke
   carries no policy ID and closes the Permit lane while that Revoke remains
   the current head.
3. Revoke is not permanently terminal. Its only valid successor is a causally
   linked epoch n+1 `rotate` with a new signed policy ID that the chain has
   never revoked. The Rotate must parent the Revoke and cannot resurrect the
   `vap_` policy that Revoke closed.
4. Every head proposal commits every event in the canonical replay prefix
   before its signed review event, excluding the review itself. The sorted ID
   list proves exact membership; `expected_parent_event_log_root` commits the
   same event contents in the protocol's stable event-ID order. Canonical
   `(timestamp, id)` replay order separately defines which events precede the
   review. Missing or extra parents, a stale root, a parent after the review,
   duplicate IDs, a fork, a gap, a repeated epoch, or an invalid reviewer
   signature invalidates the chain. Filesystem enumeration and incidental
   in-memory vector order define neither the prefix nor its root.
5. Every new `vela.policy-lane.v2` event names and causally parents the exact
   head event and epoch whose policy it used. The head must select the same
   policy, precede the lane, and still be the current non-revoked head when the
   event is staged.
6. Rotation or revocation preserves only old-lane events already present in
   the successor head's exact causal prefix. A backdated or fully readdressed
   old-policy event appended after supersession is not grandfathered and fails
   strict replay.

Historical schema-less policy-lane bytes are a compatibility case, not a way
to create new Permit authority. Only the first signed Activate head is the
migration checkpoint: it may retain an exact historical event already present
in its causal prefix. Strict replay may then recognize that exact immutable
event under a typed historical audit result, but that path exposes no live
verified policy and cannot stage or apply a new Permit. A later Rotate or
Revoke cannot retroactively bless schema-less bytes appended after the first
checkpoint, even though its exact prefix necessarily contains them.

Before the first Activate confirmation, the reference ceremony enumerates the
exact schema-less lane event IDs that would enter this audit-only checkpoint.
It validates each frozen lane, binds the sorted ID set into the private
recoverable ceremony intent, and rederives the same set under the frontier
transaction barrier before reading the clock or key. A malformed lane or any
display-to-sign set drift refuses the ceremony. Subsequent Rotate and Revoke
ceremonies checkpoint no schema-less lanes.

For new policy-lane events, every later use in this ADR of “signed Permit
policy” is shorthand for the conjunction of two proofs: the policy snapshot
and policy signature verify, and the same policy is selected by the current
valid signed policy head. Neither proof substitutes for the other. The files
`.vela/policies/active.json` and `active.sig.json` remain local inputs.
`active.json` selects candidate policy bytes, while the signature record helps
prove that a human signed those bytes. Neither file proves causal activation,
rotation, or revocation. Changing them without a matching signed head cannot
create a Permit; a missing, mismatched, forked, or revoked head closes the
lane. A valid successor Rotate after Revoke reopens it only for the new,
never-revoked policy named by that Rotate.

The policy head supplies causal selection, not trustworthy wall-clock time.
This amendment does not by itself enable finite-expiry policies to emit
unsigned Permit events; that requires a separately reviewed, authority-signed
causal time design. Reference-generated policies therefore use the explicit
`9999-12-31T23:59:59Z` sentinel and remain valid only while selected by the
signed head chain: a signed Rotate or Revoke is their operational validity
boundary. An imported or hand-authored finite policy remains valid for
Defer/Deny and human routing, but the CLI labels its Permit rules as
human-routed rather than claiming the lane can auto-admit them.

#### Producers, consumers, and maturity

| Role | Current implementation |
| --- | --- |
| Producer | The reference proposal path constructs `governance.policy_head`; the existing terminal human acceptance path emits its signed `review.accepted`. This is one first-party producer. |
| Permit consumer | Policy-route staging and application require the verified active policy ID and head-chain epoch to match the current non-revoked signed head. The policy's internal epoch remains a separate rule-version field. |
| Replay consumer | Strict replay derives the chain and verifies every new policy-lane event, plus exact historical checkpoint membership. |
| Operational consumer | Status and policy-selection porcelain may read `active.json`, but only to explain or select bytes; it cannot derive authority from that file. |

These are useful separate code paths, but they are not independent ecosystem
implementations. The two-independent-producer and two-independent-consumer
criterion is not met as of this amendment. `vela.policy-head.v1` is therefore
a protocol-local security and compatibility contract for the already existing
Permit lane, not a promoted stable ecosystem primitive. Once accepted v1 bytes
exist, their replay meaning is permanent: later evolution must use a new
schema and preserve v1 verification. Any broader public abstraction, alternate
producer, or generalized governance API remains blocked until the ADR's
producer-and-consumer evidence threshold is satisfied.

Open infrastructure also requires non-code work. Before any hosted Vela
service becomes relied upon, the project will publish an honest POSI
self-assessment and gaps, archive its open specifications and conformance
fixtures, run an offline fork and export drill, document public and sensitive
data transfer policy, and define the human governance trigger for a second
steward. A living will and patent non-assertion position remain explicit human
decisions rather than protocol fields.

## Current Vela evidence

### What is already right

The substrate already contains most of the clerk layer this ADR needs:

- `Artifact`, Receipt v1, `ActivityRecord`, and `ActivityEnvelope` already
  cover generic content references and non-authoritative activity.
- `StateProposal`, signed `StateEvent`, signed-policy certificates, and replay
  already cover candidate and authority-bearing transitions.
- Signed provenance already has generic `input_refs`; the attempt lease already
  supports same-owner refresh and a zero-second TTL.
- `vela submit` builds and lands a receipt internally for the supported witness
  path.
- `vela land --claim --artifact --caveat` avoids a receipt file for a basic
  landing.
- `vela sign` is resumable, uses one final confirmation and one key read, and
  self-materializes and publishes signed decisions.
- The sign queue is claim-first and already exposes artifacts, caveats,
  verifier runs, policy explanations, semantic pack operations, and blast
  radius when the source data is available.
- `vela policy suggest` compresses recurring asks without granting authority or
  signing anything.
- Errors use typed hints and default help already leads with the ordinary loop.
- Git is transport. Replayed authority-bearing Vela events remain the
  scientific authority: either directly human-signed or policy-certified under
  a verified, previously human-signed Permit policy.
- Verification state, acceptance scope, and publication state are distinct.

ADR 0003 reuses these parts. It does not introduce a second review command, a
TUI, or a hosted authority service.

### Defects and friction found in the live implementation

The audit found correctness problems before it found presentation problems.
They define the implementation order.

1. **A denied landing can leave state behind.** `workflow::land` writes an
   activity record and pending proposal before evaluating the signed policy.
   The Deny branch returns an error after those writes, despite the documented
   promise that Deny lands nothing.
2. **Machine output is not isolated.** `vela land --json` can receive prose
   printed by publication before the final JSON object. That violates the
   documented one-object contract.
3. **Review material is not reliably portable.** Generic landing mints root
   `records/` files, while publication stages only `.vela`, `frontier.json`,
   `vela.lock`, and `proof`. Those records and generic artifact paths can be
   absent from the published commit. Frontiers with decommitted stores add a
   second topology that a hard-coded `.vela` path cannot serve.
4. **Receipt implementations disagree.** The published Receipt v1 schema is
   rich, while the Rust landing type and inline `vela land` path accept a much
   smaller shape. Active examples do not all validate against the published
   schema.
5. **Receipt evidence is flattened away.** A record-derived proposal keeps a
   prose condition with the record ID, receipt digest, caveats, and artifact
   count, but not the structured artifact hashes and verifier runs the sign
   queue promises to show.
6. **Policy context is derived more than once.** Landing derives replayability,
   independence, method integrity, and claim class. The sign queue, policy
   testing, and suggestions often use a default context instead, so the same
   proposal can be described differently at different points.
7. **Bulk acceptance can include unseen items.** Capital `A` marks later
   Decision items accepted without rendering their individual previews. The
   final list shows verdict, ID, and title, not the omitted evidence or effect.
8. **There is no canonical Decision Brief.** Pack proposals receive a richer
   semantic preview than ordinary receipt proposals. `diff`, `sign`, status,
   campaign reports, and future site views do not consume one shared derived
   object.
9. **Review is not bound to one exact decision root.** Event signatures cover
   the resulting event content, and the existing accept preimage binds the
   proposal and base event-log root, but the interactive display is assembled from live
   state. It does not bind the full reviewed effect, critical evidence, policy
   inputs, and ordered action set to the bytes rechecked before key access.
10. **The agent lifecycle is disconnected.** `next` and `work` create an offer
    and local session, but `land` does not name the session and does not close
    it. `work --drop` removes local files while leaving the canonical lease
    until its TTL expires.
11. **The external Lean path is repository-local and under-sandboxed.**
    `reproduce-external` is an argument intercept that searches for
    campaign-owned Python scripts. A release binary outside this workspace
    cannot perform the documented path, and command help behaves like a usage
    error. The current driver copies producer `.lean` files and runs Lake over
    them, so claims that it does not execute external code are too strong: Lean
    elaboration can execute metaprograms and IO commands.
12. **Publication language and detection are incomplete.** A local commit can
    be printed as published and then described as not pushed. Status detects
    uncommitted store files but not a commit that is ahead of its remote.
13. **CLI and MCP landings differ.** The MCP land path does not materialize,
    commit, or publish like the CLI path, and the broad action schema cannot
    express action-specific required fields.
14. **The MCP custody rule contradicts itself.** The charter says acceptance
    stays off MCP, but the maintainer profile exposes a finalizing `decide`
    tool.
15. **The larger submit path is not one transaction.** `submit` can land, then
    fail while registering an artifact, applying the exact lane, materializing,
    or publishing. The resulting state may not contain the evidence or gate
    result that the command promised.
16. **Claim-text idempotency can discard new science.** Landing treats an
    existing `(claim, type)` as already landed before retaining a new receipt.
    An independent replication with the same claim can therefore disappear
    instead of becoming new evidence.
17. **Publication consumes the caller's shared Git index.** The live path
    stages Vela paths, checks the entire index, and runs an unscoped Git
    commit. Pre-existing staged work can therefore enter a Vela decision
    commit. If the Vela path set has no delta, unrelated staged work can be the
    only content committed under a Vela decision message.

These are not arguments to weaken the boundary. They are reasons to make the
boundary smaller, more truthful, and easier to inspect.

### Relation to the supplied Git-native architecture note

The note has the right central sentence: Git preserves bytes and history; Vela
interprets those bytes as scientific objects and governs accepted scientific
state. The live system already implements the strongest parts of that model:

- Git transport is not scientific authority.
- Signed accepted events are append-only, while replayed frontier state is
  revisable.
- Receipts and verifier runs remain evidence.
- Semantic Evidence Diff and graph-impact projections go beyond byte diffs.
- The event graph and scientific provenance graph are distinct from the Git
  object graph.
- Derived views, search, packets, and future UI surfaces can be rebuilt.

ADR 0003 does not copy several shortcuts from the note:

- A Git commit is not always one scientific proposal. One commit may carry
  several Vela objects, and one proposal may cite bytes from several commits or
  repositories.
- A pull request is a useful collaboration space, not the canonical review
  object. Review must remain possible offline and outside one hosting provider.
- A merge does not become acceptance. It transports bytes that may still be
  pending, rejected, or already accepted by a signed event.
- A receipt does not contain the human decision as though evidence and
  authority were one object. The decision cites the receipt and remains a
  separately signed event.
- Git object IDs are useful transport references, but Vela review evidence uses
  an explicitly typed content digest independent of filenames, repository
  layout, or hosting provider. Stable scientific identity, where a domain needs
  it, is a separate concern.
- Stable identity is not added to every protocol object merely because it is
  convenient in a diagram. ADR 0002 keeps content-addressed proposal, record,
  attachment, and event IDs until a real consumer proves a need that back-pointer
  chains cannot serve.

The missing piece is therefore not another semantic layer. It is the usable
clerk layer specified below: durable evidence in, one semantic decision out,
and exact binding at the human key.

### Git primitive audit and the emerging semantic-index wave

Vela's architectural use of Git is mostly right, but its publication
implementation is not yet Git-native enough. The right split is:

| Graph | Canonical role | May carry authority? | Rebuild rule |
| --- | --- | --- | --- |
| Git object graph | Immutable bytes, directory trees, commits, refs, and transport history | No scientific authority | Native Git object traversal |
| Vela event graph | Proposals, authority-bearing events, challenges, corrections, and replay order | Yes, only through the existing signed decision or signed policy path | Replay canonical Vela events |
| Scientific evidence graph | Claims, tests, artifacts, provenance, verifier runs, and typed lineage | Evidence only | Derive from receipts, artifacts, and events |
| Semantic index or wiki graph | Search, impact, summaries, diagrams, inferred relations, and agent context | No | Delete and rebuild from named source roots |

This separation answers the question raised by CodeGraph, Graphify, DeepWiki,
LLM Wiki, and related systems. They reveal a real new application layer: a
machine can navigate a large body of work through compact, continuously
updated semantic views. They do not justify making an inferred graph, vector
index, or generated wiki the scientific ledger. Vela should give those tools a
dependable source contract and let several implementations compete.

Every derived semantic view must therefore identify:

- the full Git commit or tree object ID, including object format, for every
  repository it read;
- the Vela event-log root and reducer version it read, when applicable;
- the extractor, indexer, or renderer version;
- the worktree or checkout identity when the cache follows uncommitted files;
- source spans or content references for displayed claims;
- whether each relation is `deterministic`, `source_asserted`, `heuristic`, or
  `model_inferred`; and
- freshness as `current`, `stale`, or `unknown`.

The view is safe to remove. A stale view may help navigation if visibly marked,
but it may not supply an unmarked decision fact or become a signed authority
input. If a generated wiki makes a new scientific assertion, that assertion
enters Vela as attributed proposal evidence through the existing Receipt v1
path.

Caches are per worktree and source-root tuple, never one mutable database
silently shared across linked worktrees or branches. Signing reconstructs
`DecisionFacts` from canonical receipt bytes and replay at the locked head; it
does not read a semantic cache or generated wiki. Existing detailed read
plumbing remains callable, but agents receive one default aggregate offer and
context surface through `next` and the Decision Brief rather than a fleet of
graph-specific MCP tools.

Adjacent systems sharpen the same boundary. [SCIP](https://github.com/sourcegraph/scip)
uses a language-neutral index with exact source ranges for definitions and
references, while heuristic search remains a lower-assurance fallback.
[GitHub Copilot Spaces](https://docs.github.com/en/copilot/concepts/context/spaces)
keeps GitHub sources current by following the latest default branch, which is
helpful for orientation and specifically wrong for a reproducible decision
unless the review also fixes an exact commit. [Entire](https://github.com/entireio/cli)
stores high-volume agent checkpoints away from the ordinary code branch,
which is a useful activity-plane pattern but not a home for canonical Vela
events. These are adapter and projection lessons, not reasons to standardize a
universal scientific graph schema.

Vela should use more of Git's existing spanning set before inventing protocol:

| Git primitive | Decision |
| --- | --- |
| Explicit path-scoped commit | Fix the shared-index defect first. Resolve the complete Vela-owned publication path set through the storage locator and construct the commit from exactly that set without writing the caller's index. Path-scoped porcelain is useful for manual recovery, but `git commit --only` may reconstruct entries from the worktree after validation and move the ref before post-commit inspection, so it is not the atomic publication mechanism. |
| Alternate index | Seed a temporary `GIT_INDEX_FILE` from the expected full Git commit OID. Insert only raw canonical blobs for the resolved Vela paths, represent deletions explicitly, and discard the index on every exit. Before ref success, the caller's real index is read only to reject overlapping staged Vela edits and prove it stayed logically identical. After a successful move of the current checked-out ref, only Vela-owned stage-zero entries may be reconciled to that commit; an un-checked-out target never changes the caller index. This is private publication plumbing, not a Vela object. |
| Trees and ref compare-and-swap | Use `read-tree`, raw `hash-object --no-filters`, `update-index --cacheinfo`, and `write-tree` to construct and validate the exact tree before publication. Use `commit-tree` to create an unreachable commit object, then move the named branch only with `git update-ref <ref> <new> <expected-git-commit-oid>`. A failed comparison leaves the branch unchanged and never invokes merge, rebase, reset, or checkout. |
| Object inspection | Before moving the ref, inspect the candidate tree and commit with `diff-tree`, `ls-tree`, and `cat-file`: every changed path is allowlisted, every expected durable object is present, and each authority blob equals its canonical bytes. A normal repository tree contains unrelated paths inherited from its parent; the commit diff, not the complete tree, must exclude unrelated changes. Git's object ID is transport identity; Vela's typed SHA-256 remains the cross-repository evidence identity. |
| Attributes | Canonical event and review bytes use a repository-owned line-ending and filter contract. Before moving the ref, compare each raw candidate authority blob with the canonical bytes so a higher-precedence local attribute or filter override fails visibly. Authority paths do not use keyword expansion or semantic auto-merge. Large witness transport may use LFS only when the Vela digest, availability, and clean-clone behavior remain explicit. |
| Hooks and signed Git commits | Hooks are local convenience and may improve ergonomics, but they are bypassable and cannot enforce Vela authority. Every atomic-publication Git subprocess runs in a sanitized environment with `core.hooksPath` fixed to a known empty temporary directory. Worktree, index, commit, and reference-transaction hooks are therefore not invoked: they can mutate inputs or external state after validation, which Vela cannot roll back. The publication commit is an unsigned operational wrapper unless a separate non-scientific Git-signing design is approved. Git commit or tag signatures may attest publication provenance but never replace a scoped Vela decision. |
| Bundles | Use [`git bundle`](https://git-scm.com/docs/git-bundle) for the offline exit and fork drill. A verified bundle can carry ordinary Git objects and refs without a server; Vela replay and digest checks still establish the scientific view. |
| Commit graph-style caches | Git's commit-graph and multi-pack indexes are precedents for derived acceleration, not new truth. A future local Vela SQLite or graph cache is allowed only when measurements require it, is keyed to its exact source roots, exposes staleness, and can be deleted without loss. |
| Notes, replace refs, and custom refs | Do not put decisions or required review facts in Git notes, replace refs, or a new `refs/vela/*` namespace. They are mutable overlays with extra fetch, push, display, and merge behavior. They may be reconsidered for an optional cache only after an outside consumer proves value. |
| Partial clone and sparse checkout | Defer them as scale optimizations. Git's promisor model permits referenced objects to be absent and fetched later, while Vela's current trust path promises offline replay and complete public review bytes. Decision-critical bytes remain locally present until a separate measured design preserves that promise. |

This is the next useful layer over Git: Git remains excellent at immutable
bytes, trees, history, transfer, and local branching; Vela adds a narrowly
scoped scientific transition and authority vocabulary that Git intentionally
does not understand; code graphs, wikis, article machines, and sites remain
replaceable consumers. Vela does not become a second source-control system, a
universal graph database, or an AI-authored encyclopedia.

## Decision

### 1. Preserve the authority invariants

The following do not change:

- Only a human-signed decision or a previously human-signed Permit policy may
  admit truth-bearing state.
- No model, MCP agent, browser UI, publication hook, receipt, verifier, or Git
  actor receives a human signing key.
- Policy defaults never imply Permit. Deny precedence and the deterministic
  engine gate remain.
- Pending proposals never mutate accepted state.
- Accepted public state derives only from replayed authority-bearing events:
  directly human-signed decisions or events carrying a verified certificate
  from a previously human-signed Permit policy.
- Acceptance always means accepted under a named frontier, policy digest,
  evidence profile, authority, and scope. It is never a claim of universal or
  context-free truth.
- Canonical serialization, content addressing, signature verification,
  base-state binding, append-only correction, and deterministic replay remain.
- Receipts and verifier runs are evidence, not verdicts.
- Verification, acceptance scope, workflow lifecycle, and publication remain
  separate axes even when the default view leads with a simpler journey.
- Existing object ID semantics remain. ADR 0002 still governs stable identity
  and revision digests.

### 2. Organize the default surface around three jobs

The product has three jobs, not three new protocol layers:

| Job | Default surface | Safe result |
| --- | --- | --- |
| Submit evidence | `next`, `work`, `reproduce`, `submit`, `land` | A durable receipt and proposal, routed by signed policy, with no manual protocol JSON on supported paths. |
| Understand a proposed change | `status`, `diff`, `sign --preview` | The same deterministic Decision Brief in human or JSON form, with no mutation and no key access. |
| Decide and publish | `sign` | A reviewed internal Decision Plan is revalidated, signed once, materialized, committed, and pushed or left with an exact recovery command. |

There will be no new top-level `vela review` command. `sign` remains the only
human decision entry. `sign --preview` is a read-only view of what a later sign
session would prepare.

There will also be no top-level `vela graph`, `vela wiki`, or fleet of
graph-specific MCP tools. Detailed read plumbing stays available, while `next`
and the Decision Brief remain the two aggregate surfaces for agent work and
human review. External indexes may accelerate how an adapter finds context,
but their cache output is never an input to signing or policy.

### 3. Make submission one recoverable transaction

`land`, `submit`, MCP landing, and supported adapters call one submission
service. Its state transaction includes receipt normalization, evidence
registration, activity and proposal creation, policy and engine-gate results,
an optional Permit event, and work-session close. Materialization and Git
publication happen after that state commit and report their own outcome.

Submission and human decision installation reuse one private, single-frontier
`FrontierTxn` mechanism. It contains an `expected_event_log_root`, staged
writes, the complete canonical delta, a durable commit marker, and recovery
state. It is not a protocol object and does not enter replay. The later Git
publication transaction separately carries `target_refname` and
`expected_git_commit_oid`; neither value is allowed to stand in for the event-
log root. ADR 0003 does not create separate submission and decision transaction
frameworks or a cross-frontier coordinator.

`FrontierTxn` and the separate Git publication transaction reuse one private
operation-journal encoding, fsync discipline, failpoint harness, and recovery
runner. They remain separate transaction domains because a scientific state
commit can succeed while publication fails. The shared journal is internal
durability plumbing, not a new event, receipt, or ecosystem contract.

The service uses the frontier's storage manifest and path resolver rather than
assuming a committed `.vela/` directory. It acquires an exclusive frontier
write lock, or an equivalent compare-and-swap event-log-root lease, before final
validation and holds it through the state commit. It then:

1. parses and validates the versioned producer input;
2. normalizes a canonical receipt and computes its digest;
3. resolves the independent storage, disclosure, locator-integrity, and
   availability facts of each existing artifact reference;
4. stages the receipt, safe evidence material, activity record, proposal,
   policy context, gate result, and possible Permit event without changing the
   canonical store;
5. evaluates policy and the engine gate against the staged view;
6. rechecks the locked event-log root and every staged digest;
7. writes and fsyncs a durable transaction journal containing the complete
   deterministic delta, then records a commit marker;
8. installs every canonical file idempotently, materializes derived views, and
   attempts the requested Git publication.

The contract is not fictitious multi-file filesystem atomicity:

- Before the durable commit marker, Deny, invalid input, a stale event-log
  root, or any
  interruption leaves zero canonical Vela or Git delta.
- After the commit marker, the submission is committed. A crash or
  materialization failure is a typed, recoverable state; replaying the journal
  idempotently finishes the exact delta. It is not rolled back or mislabeled as
  Deny.
- Publication failure never changes the scientific outcome. It returns the
  retained commit, the exact recovery command, and an honest publication
  state.

`already_landed` is valid only for a byte-identical normalized receipt digest
and the same submission identity. The same `(claim, type)` with different
evidence is a new activity record and receipt. Vela may link it to the existing
finding, but it must retain the replication and recompute independence, policy,
and effect.

Permit still admits only through the verified signed policy. Defer still lands
pending. Deny means no commit marker and zero durable Vela state.

### 4. Normalize receipts once and retain review material safely

The CLI, MCP server, adapters, and tests use one Rust normalization and
validation path.

- Receipt v1 keeps the complete required field set of the already published v1
  schema. ADR 0003 does not make those fields optional under the same schema
  identifier: an older conforming validator must accept every new v1 receipt.
  The supported authoring input is smaller: claim, claim type, artifact or
  content references, caveats, and explicit producer context. A private
  builder expands that input into the complete current-valid wire shape.
- Supported CLI flags and adapters feed a private `ReceiptBuilder`; they do not
  publish a second receipt-draft wire format. File-based Receipt v1 remains the
  interoperability boundary.
- The normalizer fills only mechanical fields from an explicit context. It
  never invents a caveat, verifier outcome, independence claim, source, or
  authority claim.
- Where Receipt v1 requires a state that has not been established, the builder
  uses only a truthful neutral, empty, pending, or not-assessed representation
  already admitted by v1, and derives mechanical provenance from explicit
  context. It never fabricates OIDC identity, attestation, verifier success, or
  an acceptor merely to satisfy a schema. If v1 cannot express an unknown fact
  truthfully, the authoring path asks for it or stops; changing the required
  wire shape needs a separately reviewed compatibility decision or Receipt v2.
- The stored Receipt v1 validates against the published schema and shared
  semantic checks.
- The canonical normalized receipt and activity record are stored through the
  frontier's resolved committed-store topology.
- Existing `Artifact` and receipt reference fields carry orthogonal facts:
  storage is embedded or referenced; disclosure is public or restricted;
  locator integrity is immutable, mutable, or unknown; and availability is
  available, unavailable, or unknown. A restricted artifact may also have an
  immutable locator.
- Public bytes use an approved public digest. Restricted bytes use either a
  sealed, domain-separated commitment with its opening held outside Git, or an
  opaque custodian reference when even public equality would leak information.
  A raw public hash of low-entropy restricted content is forbidden. The
  commitment scheme, custodian, disclosure tier, access procedure, and safe
  summary remain public; authorized reveal verifies the opening.
- Secret environment values, credentials, proprietary source, classified
  evidence, and other restricted payloads are never copied into Git merely to
  make a packet complete.
- The raw canonical receipt is the evidence source of truth. `ActivityRecord`
  is a deterministic local index or compatibility object that points to its
  root. Proposal payloads cite receipt, record, and evidence roots rather than
  copying a second semantic receipt into the proposal. They do not rely on a
  prose condition for reconstruction.
- A receipt may preserve an attributed producer claim, predicted observable,
  test, result, evidentiary direction, insufficient evidence, counterevidence,
  and attempt lineage. Producer-reported confidence, belief, `supports`,
  `refutes`, or evidence-validity labels remain namespaced assertions. They
  cannot satisfy a verifier, increase policy assurance, trigger Permit, or
  mutate accepted state.
- Normalization preserves content-addressed distillation material when present:
  artifact digest, intended audience, rubric, known gaps, inheritance note, and
  contributor-role attribution. A policy may require a distillation for a
  named scope, but only the later authority event accepts that distillation.
- A legacy proposal remains readable. Missing legacy material is reported as a
  typed `missing` entry, never silently omitted or guessed.

A fresh clone must reconstruct the same safe Decision Brief and all public
review bytes without the producer's working directory. Restricted material is
represented by its public commitment or opaque reference, disclosure tier,
access requirement, and typed availability. It is never silently treated as
present.

### 5. Derive one small Decision Brief from shared facts

One private, pure `DecisionFacts` builder reads the proposal, canonical receipt
and record roots, existing artifacts, verifier state, semantic diff, existing
policy context, challenge relations, and downstream graph. Status, next,
Decision Brief, and packet rendering reuse those leaf facts. ADR 0003 does not
add a versioned `WorkflowProjection`, lifecycle schema, or second state model.

`DecisionBrief` is the only new testing-stage read contract. Every JSON object
declares `schema: vela.decision-brief.testing.v1` and `stability: testing`, and
the repository carries a schema plus canonical golden fixtures. It is a
deterministic projection, not durable scientific state, a verdict, or an
authority object. Its required core is intentionally small:

```text
change
  subject and fixed base
  claim, before and after, exact requested action

basis
  primary evidence roots and check state
  main caveat and attributed producer interpretation

impact
  downstream effect, reversibility or correction path
  critical warnings

authority
  named frontier, policy route, acceptance scope
  why a human is needed and allowed actions

audit
  proposal root, decision-facts root, and raw references
```

Only decision-critical absence enters the core as a typed missing fact. Richer
information appears in deterministic typed facets when present or required by
the proposal's assurance profile:

- verifier and gate matrix, including exact inputs and pass, fail, or missing;
- formal versus informal semantic fidelity, quantifiers, conditions, axioms,
  and comparator definitions;
- evidence lineage, recomputed versus trusted inputs, failure domains,
  replication, and methodological diversity;
- predicted observable, performed test, result, counterevidence, and HEP-style
  producer beliefs, all attributed;
- distillation, audience, rubric, known gaps, contributor roles, disclosures,
  external certificates, and reviewer credit;
- challenge, correction, supersession, and adjudication state; and
- operational publication status and recovery action.

Human output shows the core plus any critical facet. JSON includes every
available typed facet and raw reference. A pre-answer brief never contains an
accept or reject event intent because no such decision exists yet. A generic
`signed` label is forbidden because origin, authorization, execution,
transparency, and endorsement signatures mean different things.

Decision Brief bytes are never hashed into scientific authorization. Its
domain-separated `decision_facts_root` is a testing-stage projection-consistency
digest over ordered typed leaf facts, not an authority input. The exact fact
roots consumed by the decision provide stale-state binding, so a testing
renderer or schema can evolve without becoming a signing protocol.

The same Rust projection powers `diff`, optional `sign --preview`, interactive
`sign`, the pending part of `status`, MCP orientation, reports, and derived
packet views. A later site or plugin may consume the same contract after the
terminal and JSON surfaces survive outside use.

Verification, acceptance scope, work or lease state, contestation, and
publication remain separate axes in their existing models. Human copy may lead
with the relevant task, but no combined green badge or boolean `trusted` field
is permitted. Any actor may land a challenge or counter-attestation as
evidence. One authority cannot revoke another authority's decision; it can
issue a scoped correction, supersession, or contrary decision whose relation
remains visible.

### 6. Bind signing to one internal Decision Plan

`sign --preview` renders only a read-only Decision Brief. It neither guesses a
verdict nor persists a plan. During `sign`, Vela renders the brief, records the
human's explicit answers, and only then builds a private `DecisionPlan` from one
fixed `expected_event_log_root`. Any edit rebuilds the plan and rerenders every
changed decision-critical field before confirmation.

The canonical `decision_root` hashes a domain-separated, versioned internal
preimage. It covers:

- frontier identity and current event-log head;
- ordered proposal roots and the explicit human action and reason for each;
- canonical receipt, evidence, verifier, existing `PolicyContext`, evaluator
  result, authority, semantic-effect, and downstream-impact roots actually
  consumed by the decision;
- reviewer identity and the current authorization facts checked by the
  existing custody gate; and
- every semantic field of the exact unsigned event cores, including final
  timestamps, with only the audit-reference slot, event ID, and signature
  zeroed.

The preimage records its version, ordered proposal/action/reason tuples, each
exact consumed fact root, the policy-input root, and the ordered zeroed event
cores. It does not redundantly include `decision_facts_root`, or hash
`DecisionBrief` JSON, terminal layout, or renderer bytes. Cross-implementation
fixtures freeze the canonical preimage and digest without creating a new replay
object or public manifest namespace.

Publication intent and renderer version are not scientific authorization
inputs. A Git push option cannot stale a scientific review, and UI code does
not become authority metadata. Renderer changes invalidate resumable local
session state when necessary and remain covered by renderer tests.

`DecisionPlan` is private transaction plumbing, not `PreparedDecisionV1`, a new
protocol object, a new ID family, or a special review-manifest namespace. If an
audit snapshot is retained, Vela stores its canonical bytes as an ordinary
content blob and cites its digest through the existing signed
`Provenance.input_refs`. The root is inserted before event IDs and signatures
are computed. The existing canonical event signature therefore covers the
reference. The detached acceptance path extends its existing versioned
`accept_preimage_bytes` with the same `decision_root`; it does not introduce a
second signature protocol.

The successful order for one frontier is exact:

```text
render briefs -> explicit answers -> build plan -> render final set -> confirm
-> acquire the frontier lock
-> reload and rederive every decision-critical dependency while locked
-> reproduce the same decision_root and run the existing engine gate
-> read the resolved human signer once
-> finalize and sign the exact event cores in memory
-> commit through FrontierTxn
-> materialize and attempt publication
```

Any change to the base event-log root, proposal, canonical receipt, decision-critical
evidence or availability, verifier snapshot, policy context or evaluator
result, reviewer authorization, semantic effect, or ordered action set aborts
before key access. Vela prints what changed and starts a fresh review. It never
patches a confirmed plan.

ADR 0003 makes one frontier the atomic authority and recovery unit. A command
working across frontiers processes and reports independent transactions; it
does not imply distributed all-or-nothing acceptance. A genuinely atomic
cross-frontier scientific act requires a demonstrated use case and a separate
ADR.

### 7. Keep the ceremony small and remove blind batching

The successful human ceremony remains:

1. Render every item or a complete deterministic set-level brief.
2. Record accept, reject, skip, retraction, or other applicable human answers.
3. Build and show the full final set with claim, effect, critical warning, and
   chosen action.
4. Allow one item to be edited or the set to be reset.
5. Ask once for final confirmation.
6. Acquire the frontier lock and reproduce the `decision_root`.
7. Read or activate the resolved human signer once.
8. Sign the exact event intents, journal, save, materialize, commit, and attempt
   push.

Skip remains the absence of a decision. Skipped proposals stay pending, are
listed as retained in the final summary, and are excluded from the Decision
Plan and signed event set.

Capital `A` may not accept unseen items. It can return only after Vela has
rendered the complete coherent set-level brief for those items.
High-risk operations such as retraction, policy or governance changes, trust
path changes, and broad multi-finding effects are isolated from routine
batches.

A coherent batch also shares claim class, assurance profile, policy route,
required checks, reviewer capability, and risk tier; has no escalation trigger;
and stays within a signed-policy size bound. Every item remains individually
excludable. Status, diff, preview, export, reproduce, and verification are
categorically key-free. Citation or endorsement is a separate explicit act,
never a side effect of reading.

Mixed human-signed actions on one frontier are one transaction, not a sequence
in which rejections land before acceptances are confirmed. Skip is excluded
because it is no decision. Any action that changes a field after the final set
was shown forces a new set and a new confirmation.

Hardware-backed raw Ed25519 remains a compatible future signer, as described in
`docs/HARDWARE_SIGNING_PROPOSAL.md`. Passkeys, OIDC, and keyless signatures do
not become acceptance authority through this ADR.

### 8. Derive policy context in one place

One pure builder constructs the existing `PolicyContext` from the proposal and
its durable receipt root, artifacts, verifier state, gate result, and graph
impact. Landing, sign queue filtering, status, policy testing, policy
suggestions, CLI, and MCP use the same context and existing evaluator result.
One canonical helper digests exactly the fields the live policy language
consumes.

Decision Brief copy may explain assurance profile, semantic ambiguity,
reversibility, contestation, dependency impact, unresolved inputs, typed Defer
reason, exact human judgment requested, consequence of error, and required
reviewer capability. Those explanatory facts do not expand `PolicyContext`
unless an actual signed rule consumes them. High-risk semantic work receives
an independent assessment before any machine recommendation could be shown.
ADR 0003 does not otherwise add machine recommendations to the Decision Brief.

If required evidence is missing, the context becomes more conservative. It
never upgrades assurance, independence, replayability, method integrity, or
credential validity from an absent field.

### 9. Connect the producer lifecycle

The supported producer path becomes:

```text
vela next <frontier> --json
vela work <target> --as agent:<name> --json
<produce and verify>
vela land --work <target> --claim <claim> --artifact <path-or-ref> \
  --caveat <text> --as agent:<name> --json
```

`work` keeps one typed internal session record in ignored scratch; the producer
does not edit receipt JSON on the default path. `land --work` accepts
concise claim, artifact, caveat, source, and optional verifier-result facts, or
loads those facts mechanically from a supported adapter. It normalizes and
lands the receipt, binds the target and attempt into provenance, and closes the
session only after a committed Defer or Permit transaction. File-based receipt
import remains an advanced interoperability path. A failed or denied landing
leaves the session open with an exact repair action.

The offer and internal task contract expose the existing task's objective,
exact base, target, constraints, allowed and forbidden actions, required
outputs and checks, completion conditions, escalation path, and authority
ceiling. Their digest is retained in attempt and receipt provenance when
available. This composes existing objects; it does not add a new truth-bearing
task object.

`work --drop` reuses the existing signed `attempt.claimed` lease update with a
zero-second TTL, a non-empty release reason, and strict same-actor, key, target,
and prior-lease checks. The reducer already treats lease expiry as read-time
coordination, so no `attempt.released` kind, release object, or lifecycle schema
is needed. Another agent may claim immediately after the signed update commits;
only then may local scratch be removed. Deleting scratch alone is not reported
as release.

For supported verifier adapters, the user does not author receipt JSON:

```text
vela reproduce-external <repo> <commit> <declaration> \
  --land-work <target> --as agent:<name> --json
```

The external Lean command becomes a normal parsed CLI command with real help.
Its frozen driver and Receipt v1 builder are versioned with the substrate and
available to an installed release binary. The binary may rely on documented
system prerequisites such as Git, Python, Lean, and a supported sandbox backend,
but not on the campaign checkout's `scripts/` directory.

External Lean elaboration is untrusted code execution. Repository fetch and
pin verification happen outside the execution sandbox. Elaboration then runs
with pinned, read-only source and toolchains; no network; an empty temporary
home; no inherited credentials, SSH agent, tokens, or user configuration; an
allowlist of executables and writable paths; and CPU, memory, disk, process, and
wall-time limits. Vela fails closed if no supported sandbox is available. The
receipt records the sandbox backend, limits, source and toolchain pins, and any
blocked capability attempt. It never claims `external_code_execution: false`.

`vela submit` remains the one-command path where an adapter can safely verify,
normalize, land, and bind evidence. New adapters must implement the same receipt
builder and transactional landing contract, not bespoke state mutation.

Useful failure is retained only deliberately. A producer may land a scoped
negative or inconclusive receipt that records the attempted method, environment,
failure mode, counterevidence, and what was ruled out. Incidental files from an
invalid or denied submission are still removed before the commit marker.

### 10. Make publication truthful and recoverable

Publication returns a structured `PublicationOutcome`; it does not print. The
outer command renders it exactly once.

Publication also behaves as a scoped Git transaction:

- resolve the complete durable path set from the frontier storage topology;
- decide whether there is a Vela delta by comparing only that path set;
- construct a candidate commit from only that path set in a temporary index;
- preserve unrelated staged and unstaged work;
- refuse overlapping staged or unstaged edits to Vela-owned paths unless they
  are the exact preimage or output of the current state transaction;
- verify before publication that the candidate commit diff contains no changed
  path outside the allowlist and every authority blob equals canonical bytes;
- update `target_refname` from one `expected_git_commit_oid` with compare and
  swap, failing on concurrent Git movement; and
- leave the caller's branch and entire index unchanged when construction,
  inspection, or compare and swap fails.

The implementation uses a temporary `GIT_INDEX_FILE` seeded from the expected
Git commit. It inserts raw canonical blobs with `hash-object --no-filters` and
`update-index --cacheinfo`, writes and inspects the candidate tree, creates an
unreferenced commit with `commit-tree`, and only then moves the named branch
with `update-ref <ref> <new> <expected>`. The private operation journal fixes
the commit message, author, committer, and timestamp and retains the candidate
OID, so crash recovery reuses the same commit object. It never runs `git add`,
`git commit`, merge, rebase, reset, or checkout against the caller's worktree.
A compare-and-swap loss may leave an unreachable object for normal Git garbage
collection, but no visible branch or index delta.

Publication distinguishes three ref states before constructing objects. If the
target is the branch checked out in the caller's worktree, a successful ref
move aligns only the resolved Vela stage-zero entries in that index with the
candidate tree. Otherwise the new paths would appear as artificial staged
reversions and unstaged additions. If the target is an allowed local branch
not checked out in any worktree, publication moves it without touching the
caller's index or worktree. A target checked out in another linked worktree is
rejected before object construction.

Every unrelated logical index entry and worktree byte remains unchanged.
Because Git has no atomic transaction spanning the current checked-out ref and
its worktree index, the private journal records the post-ref boundary only for
that case. It also records the exact worktree identity, checked-out ref, original
Vela stage-zero index entries, and expected Vela worktree output. A crash or
index-lock failure returns `committed_local` and one idempotent recovery command.
Recovery writes only when the ref still equals the candidate, the same worktree
still has that ref checked out, and every journaled Vela index entry and
worktree byte is unchanged. Any drift returns a manual-repair state with zero
index writes; it never overwrites newly staged or edited Vela work. A
non-checked-out target needs no index reconciliation.

Candidate commit reuse is limited to crash recovery while the target still
equals the recorded expected Git OID, or to finishing post-ref index recovery
when it already equals the recorded candidate OID. Losing the ref compare and
swap returns a distinct stale-publication result; a later attempt must resolve
and inspect a new parent and construct a new candidate rather than retrying the
old commit forever.

- `uncommitted`: Vela state changed but no publication commit is reachable from
  the target ref. An unreachable candidate object may await Git garbage collection.
- `stale`: another writer moved the target ref. Vela leaves the winning ref and
  caller index unchanged, reports expected and actual OIDs, and requires a new
  publication plan against the new parent.
- `committed_local`: a local commit contains the complete durable review or
  signed-decision material and is ahead of its remote.
- `pushed`: the remote contains the commit.
- `unknown`: Vela cannot prove remote state and says why.

Only `pushed` is described as published. A push failure does not erase a valid
signed event. Human and JSON output identify the retained commit and give one
exact recovery command.

CLI and MCP landing use the same workflow service and return the same state and
publication object. MCP may default publication off, but that difference is an
explicit option rather than an omitted code path.

Canonical Vela paths ship with and test a repository-owned `.gitattributes`
contract. Publication checks effective attributes and compares each candidate
blob with the canonical bytes, so a higher-precedence local clean or smudge
filter, keyword substitution, working-tree encoding, or other transformation
aborts. Raw blob construction does not rely on worktree conversion. Semantic
merge drivers are not used for authority paths.

Every Git subprocess in this plumbing uses a sanitized environment and a known
empty `core.hooksPath`. It therefore runs no repository worktree, index,
commit, or reference-transaction hook; their post-validation mutation and
arbitrary side effects cannot be included in Vela's rollback promise. The Git
commit is an unsigned operational publication wrapper. Replay, signature
checks, and conformance remain the authority gate. A future requirement for
Git commit signing or hook integration needs a separate design and must never
reuse or read the human scientific signing key.

### 11. Keep finalization off MCP

No MCP profile may solicit a human verdict, read a human key, create a human
signature, or finalize a truth-bearing proposal. The current `decide` exposure
is removed from the maintainer profile.

The `maintainer` MCP profile becomes a deprecated warning alias for the
nonfinalizing `draft` profile. It does not retain a hidden capability difference.
Callers that need broader read or producer tools must request those explicit
capabilities; no profile name implies human authority.

If a future integration needs to relay an already signed event, it must be a
separately named, signature-verifying transport tool with no key access and no
ability to alter signed bytes. That relay requires its own threat-model review.

### 12. Enforce human and machine output contracts

Every mutating service returns data. Rendering happens only at the command or
MCP boundary.

- `--json` writes exactly one JSON object to stdout and no prose.
- Diagnostics go to stderr in a stable error envelope.
- Non-interactive invocation never enters a prompt loop.
- Human errors state what failed, what was retained, and one safe next action.
- `--dry-run` or `--preview` performs the real parsing, normalization,
  projection, policy derivation, and gate checks without persistent state.
- JSON additions are additive within a schema version. Removals or semantic
  changes require a versioned projection.
- Human renderers remove or visibly escape control characters, bidirectional
  overrides, hyperlinks, and terminal escape sequences from untrusted fields.
- Every import and render path has explicit byte, depth, string, artifact,
  archive-expansion, and list limits. Truncation is labeled and retains the
  digest of the complete object.
- Review does not fetch remote locators or execute producer content. Retrieval
  and verification are separate explicit, sandboxed actions.
- Queue reads are paginated and bounded. Admission can apply policy-owned
  per-actor and per-frontier backpressure without converting volume or rank
  into scientific authority.

### 13. Make the frontier loop the ecosystem seam

The first ecosystem artifact is a reference frontier kit, not a Vela app
store. A kit is an ordinary cloneable Git tree and documentation convention
assembled from existing fields. It contains:

- a bounded scientific question and current accepted root;
- ranked contribution targets with prerequisites and completion conditions;
- pinned domain dependencies, environment, and selected verifier descriptor;
- examples of valid positive, negative, correction, and inconclusive receipts;
- the scoped policy and authority ceiling that explain whether a result can be
  permitted automatically or must wait; and
- exact `next`, `work`, reproduce, `land`, inspect, and replay commands.

The task-first surface turns that kit into one short loop. `next` explains why
a target matters, what a contributor needs, what proves completion, and what
the contributor is not authorized to do. `work` prepares local scratch and the
lease. The contributor's preferred tool performs as many private iterations as
needed. The frozen verifier and `land` select one durable checkpoint. Signed
policy routes exact low-risk lanes; the Decision Brief sends only real
exceptions to a person. Replay installs the new root and recomputes the next
targets.

This is also the education seam. A tutorial should culminate in a real pending
contribution against a live or immutable training frontier, not an imitation
dashboard. Beginner and expert views may differ in explanation, examples, and
suggested targets, but they share the same receipt, verifier, policy, and
authority semantics. A learner can make useful evidence cheap to contribute
without being asked to understand event IDs or being allowed to decide what
enters canon.

Vela will publish conformance fixtures, verifier honesty cases, benchmark
snapshots, reference adapters, and source-linked documentation so independent
builders can create workbenches, courses, package indexes, article machines,
or hosted collaboration without a private API. A read-only hub may help people
discover those repositories and their current compatibility. A hub, course,
social layer, agent runtime, or package installer is not required to clone,
verify, land, decide, replay, fork, or export a frontier.

The implementation may document this convention, exercise the same adapter on
two pinned Lean sources, and prove one independent consumer. It does not add `FrontierKit` to the
protocol, create a central marketplace, standardize an inner-loop trace, or
make education metadata decision-critical. The proof of ecosystem value is a
shorter accepted-root-to-substantive-child loop and a useful complementor built
without a kernel change.

## Non-goals

ADR 0003 does not:

- weaken signature, replay, policy, or reducer checks;
- replace Git, programming languages, notebooks, proof assistants, containers,
  repositories, archives, or domain execution systems;
- collapse lifecycle, verification, acceptance, and publication into one state;
- add a model recommendation or automated scientific verdict;
- make a receipt, CI result, Git merge, signature, or transparency entry equal
  acceptance;
- introduce a hosted Vela authority, public transparency log, passkey authority,
  OIDC authority, or keyless human acceptance;
- add stable IDs to proposals, records, attachments, or events;
- add a universal first-class hypothesis, experiment, or article-machine object;
- build a social network, course platform, agent organization runtime, or
  central verifier marketplace;
- make a generated wiki, semantic index, vector store, or knowledge graph a
  canonical authority store;
- require Git notes, replace refs, custom Vela refs, committed graph caches, or
  a new pack and clone protocol;
- add a public receipt-draft schema, Workflow Projection schema, prepared-
  decision protocol, new artifact family, or attempt-release event;
- add distributed cross-frontier transaction semantics;
- add platform-specific OpenResearch, HEP, or Diderot kinds or mapping code to
  the authority kernel;
- add a top-level review command or a second signing surface;
- add browser or MCP signing;
- build the site projection before the CLI and JSON projection survive outside
  use;
- migrate or rewrite existing accepted events.

## Options considered

### Keep the protocol and improve documentation only

Rejected. The quickstart can be shortened, but documentation cannot make a
denied landing leave zero delta, restore missing review material, make JSON parseable, or
bind the displayed effect to the signature.

### Replace the terminal ceremony with a web app or TUI

Rejected for this phase. It creates another trusted renderer and a tempting key
path before the underlying review projection is complete. A later UI should
consume the same Decision Brief and keep signing in the terminal.

### Build a Vela-native compute, authoring, or publication platform

Rejected. OpenResearch, Git repositories, notebooks, Lean, containers,
arXiv-like archives, and specialized article machines already own useful parts
of that workflow. Vela should ingest their durable outputs through adapters and
give them a shared state and authorization boundary. Reimplementing them would
increase adoption cost, concentrate failure modes, and compete with the
ecosystem this protocol is meant to connect.

### Collapse all status into Draft, Pending, and Accepted

Rejected. Those words are useful as a journey projection, but using them as the
data model would hide whether a claim was verified, how broadly it was
accepted, and whether its signed state was pushed.

### Sign the receipt or proposal directly

Rejected. The human is authorizing the semantic state transition, not endorsing
every producer assertion in a receipt. Existing domain events remain the
authority. The internal Decision Plan binds those event intents to the reviewed
facts and base without becoming another authority object.

### Add stronger authenticators first

Deferred. Hardware custody improves resistance to key extraction, but it does
not prove that the right semantic change was displayed. Clear decision binding
comes first; raw-Ed25519 hardware signing can follow without changing replay.

## Migration and delivery order

The July 11 synthesis remains the sequencing constraint: close conformance
parity, portable pending inheritance, an outside first write, a human decision,
and independent downstream consumption before adding speculative profiles or
kernel kinds. ADR 0003 is delivered as six vertical slices:

1. Freeze the cold producer and reviewer baseline; fix JSON purity, truthful
   publication results through an isolated candidate-tree transaction,
   shared-index contamination, hostile rendering, and the MCP custody
   contradiction.
2. Create one correct single-frontier write edge with `FrontierTxn`, Receipt v1,
   existing artifacts, portable review roots, exact retry semantics, and Deny
   with zero committed delta.
3. Create one `DecisionFacts` path and a minimal Decision Brief for an ordinary
   receipt proposal; reuse it in diff, preview, sign, and status, then cold-test
   reviewers immediately.
4. Bind exact decisions with the internal Decision Plan, existing signing
   seams, single-frontier recovery, and no blind batch action.
5. Connect work-session porcelain, signed zero-TTL lease release, one installed
   external Lean adapter, and the schema-free reference frontier kit; exercise
   the adapter on two pinned Lean sources, then cold-test an outside producer
   and an isolated learning path.
6. Prove clean-clone portability, offline replay, compatibility, one human
   ceremony, verified Git-bundle exit, and independent downstream use. Add only
   real adapter fixtures and POSI exit/governance work supported by that
   evidence. A graph or wiki consumer must remain derived, commit-bound, and
   removable. Record the full accepted-root-to-substantive-child loop and every
   repair rather than substituting output volume for ecosystem proof.

Cold runs are milestone gates, not one final usability test. Repeat them after
the minimal portable landing slice, after the shared Decision Brief, after
Decision Plan binding, and after the connected producer path. Repeated
failure to understand a field or complete a step is a reason to simplify the
scaffold before adding another surface.

Each trust-path change runs the full conformance union. Existing frontiers must
replay byte-identically. The optional signed decision-root reference is
exercised by a new fixture; old fixture bytes and IDs do not change.

### Implementation evidence, 2026-07-14

The six slices above now have a release-shaped technical scaffold. The work was
landed incrementally so each trust-boundary change remains reviewable:

- `53e69c1` closes JSON-output and key-custody boundary leaks.
- `453c6a5` makes receipt landing one recoverable, single-frontier write.
- `c89db89` derives one bounded Decision Brief from canonical decision facts.
- `a51c8a5` binds the displayed decision to exact event bytes and stale-state
  checks.
- `93a575c` connects task-first work sessions to installed verification and
  signed lease release.
- `e193edc` proves portable review consumption, Git-native exit, and removable
  derived consumers without adding authority to those consumers.

Release verification covers the complete Rust workspace and lint surface; the
Python, JavaScript, Rust, and decision-binding cross-implementation vectors;
hostile and installed external-Lean fixtures; clean-clone and offline Git-bundle
replay; the Diderot relay fixtures; the Atlas derived consumer; documentation
and voice checks; and the registered 70-gate conformance union. These checks
establish compatibility, replay, bounded rendering, safe retries, custody
separation, and portable consumption. They do not establish scientific
acceptance or ecosystem adoption.

The following program gates intentionally remain open:

- No real human key ceremony was performed as part of implementation or
  release. Test ceremonies use fixture keys only.
- The Decision Brief contract remains `testing` until two independent
  producers and two independent consumers provide conformance evidence.
- The recorded producer path is an isolated engineering fixture, not a cold
  outside-producer usability trial.
- The substantive-child and correction-watcher exercises prove the interfaces,
  but do not yet satisfy the criterion that a different independent producer
  consume a human-accepted outside result.
- The POSI governance, institutional stewardship, sustainability, and hosted
  service commitments remain organizational work even where export, archive,
  and fork mechanics are implemented.

Accordingly, the `0.759` release train is the implementation release for the
clerk layer, not a claim that the acceptance program or the outside-producer
goal is complete. Patch `0.759.1` changes only fail-closed sandbox cleanup; it
does not expand the authority surface.

## Acceptance metrics

The program is complete only when all of these hold:

- A first-time producer reaches one valid pending receipt in 5 minutes at the
  median and 10 minutes at the 90th percentile after documented prerequisites.
- The exact-witness path takes one Vela command. The external Lean path takes at
  most two Vela commands after prerequisites.
- Supported paths require no hand-authored protocol JSON and no maintainer
  repair during the measured run.
- Deny, invalid input, stale event-log roots, Git ref races, and every
  interruption before a commit marker produce zero durable Vela or Git delta. Every tested interruption
  after a marker recovers the exact committed delta idempotently.
- Two receipts with the same claim and type but different evidence both remain
  inspectable; only a byte-identical receipt retry is deduplicated.
- Every newly emitted Receipt v1 validates with the pre-ADR v1 validity rules;
  the existing required wire fields remain required and unknown namespaced
  extensions survive round-trip.
- A clean clone reconstructs the same safe Decision Brief and every public
  review byte. Restricted evidence retains the same sealed commitment or opaque
  custodian reference, disclosure tier, access requirement, and typed
  availability without leaking payload equality when that is sensitive.
- Every decision-critical Decision Brief field is populated or carries an
  explicit typed reason for absence. Optional facets do not emit decorative
  missing values.
- The Decision Brief contract remains testing until two independent producers
  and two independent consumers are named with conformance evidence; one
  outside consumer cannot stabilize it. Every brief declares its testing
  schema and stability, and canonical golden fixtures cover the required core.
- Supported producer paths preserve an explicit claim, predicted observable or
  typed `not_applicable`, test, result, evidence reference, and caveat chain.
  Contrary and insufficient evidence remains visible.
- Landing, sign, status, policy test, and policy suggest derive identical policy
  context for the same proposal.
- Five out of five test reviewers see every item before it can enter a batch;
  at least four can accurately state the proposed change, evidence, caveat,
  authority source, and consequence without opening raw JSON.
- The ceremony has one final confirmation and one key read.
- No Decision Brief contains a preselected verdict or model recommendation.
- Every tested change to the base, proposal, evidence, verifier snapshot,
  policy context or evaluator result, reviewer authorization, or semantic
  effect changes the `decision_root` and invalidates the review before key
  access.
- Changing the signed decision-root input reference changes the event ID and
  fails signature verification; changing Git publication options does not
  change scientific authorization bytes.
- A failed push retains the signed decision and gives a verified one-command
  recovery path.
- Publication with unrelated staged and unstaged files changes only the
  resolved Vela path set in the candidate commit diff and leaves those unrelated
  changes in their original state. When the target is the current branch, Vela
  paths are clean against the new tip while unrelated index entries retain
  their logical state. An un-checked-out target leaves the caller's entire index
  untouched; a target checked out in another linked worktree is rejected before
  object construction. If the Vela path set has no delta, no commit is created
  even when the caller's index is nonempty.
- Repository hooks are not invoked. A concurrent Git ref update leaves the
  winning branch and caller index unchanged and returns a stale-publication
  result that requires replanning against the new parent. Publication never
  merges, rebases, resets, retries the stale candidate indefinitely, or silently
  rewrites unrelated work.
- A crash or index-lock failure after the ref moves reports the retained local
  commit and recovers only the Vela index entries idempotently when the
  journaled worktree, checkout, index entries, and worktree bytes still match.
  Any drift refuses recovery with zero writes; recovery does not move the ref
  again, overwrite new Vela work, or rewrite unrelated entries.
- Canonical authority paths pass cross-platform attribute tests. Any clean or
  smudge filter, keyword expansion, line-ending, or local attribute override
  that would change an authority blob is detected and rejected before
  commit; semantic merge drivers are not used for those paths.
- CLI and MCP JSON parse as exactly one object and agree on the landing result.
- Agent signing and MCP-finalization violations remain zero.
- Receipt import rejects or safely bounds oversized strings, deep JSON,
  excessive artifacts, archive bombs, malicious locators, terminal controls,
  and untrusted hyperlinks. Review never fetches or executes them implicitly.
- A stress fixture with at least 10,000 machine-generated submissions remains
  paginated and bounded, applies explicit backpressure, preserves exact retry
  idempotency and independent evidence, and routes only real exceptions to
  human attention.
- Queue depth and age, reviewer minutes, correction latency, verifier
  diversity, replication-versus-duplication, and independent downstream use
  are measured. Receipt volume alone is not a success metric.
- A malicious external Lean fixture cannot read host credentials, reach the
  network, write outside its temporary output, or exceed configured resources;
  the command fails closed without a supported sandbox.
- One existing Git-and-container or Git-and-Lean project integrates without
  moving its source, changing its authoring language, or adopting a hosted Vela
  service.
- From a clean clone, a new contributor identifies the bounded question,
  accepted root, next target, selected verifier, authority ceiling, and first
  command from the reference frontier kit in under two minutes.
- The installed Lean adapter and task-first contract run unchanged on two
  pinned Lean repositories or frontiers. Reports distinguish formal kernel
  checking from statement faithfulness, significance, and library quality.
- One isolated training frontier ends in a real pending Receipt v1 through the
  same verifier and authority semantics as the expert path; it never grants or
  simulates acceptance.
- Accepted-root-to-substantive-child, target-to-verifier, receipt-to-route,
  Defer-to-decision, and correction-propagation latency are measured. Mature
  reusable definitions record whether they were proposed to the upstream
  domain library.
- Unknown namespaced Receipt v1 extensions survive normalize, land, export, and
  re-import byte-for-byte or with a documented canonical equivalence.
- A reproduce-on-arrival bot or correction watcher is implemented outside the
  kernel from a clone and stable read contracts, proving generativity without a
  platform-specific event kind.
- One graph or wiki consumer binds every result to an exact Git and Vela source
  root, labels deterministic versus inferred relations, reports stale roots,
  and can lose its entire cache without losing accepted meaning. Signing is
  identical with that cache present, stale, or deleted.
- The POSI self-assessment, specification archive, data-export policy, and
  verified Git-bundle fork drill exist before a hosted service becomes a
  dependency.
- Core, frontier, trust-invariant, cross-implementation, and selected formal
  suites report zero failures.
- Every new `vela.policy-lane.v2` Permit names the matching current signed
  policy-head event and epoch. Strict replay rejects a missing or mismatched
  head, fork, gap, stale causal prefix, pre-activation lane, and old-policy
  event appended after rotation or revocation. An exact historical lane passes
  only when the first signed Activate checkpoint already contains its event ID;
  `active.json` alone never authorizes a write. A current Revoke closes Permit;
  only its causally linked successor Rotate with a new, never-revoked policy ID
  may reopen the lane.
- The first activation displays and transaction-binds every schema-less lane
  it retains for audit compatibility; malformed or changed checkpoint sets
  refuse before clock or key access. Generated Permit policies use causal
  Rotate/Revoke validity rather than a misleading finite wall-clock window.
- Any new Permit rule first passes historical replay and shadow mode, begins
  with complete audit, has sentinel cases and a kill switch, and reports false
  Permit, false Defer, semantic-fidelity error, reviewer minutes, and
  replication-versus-diversity rates before its audit sample may shrink.
- The existing outside-producer goal closes: one independent producer lands a
  genuinely rederived, non-vacuous result through a strong verifier, a human
  decides it, and a different independent producer consumes its durable receipt
  to build a substantive child from the accepted root. Raw receipt, project, or
  verifier counts do not satisfy this criterion.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A clerk layer accidentally becomes a second authority model | Keep projections derived, content-addressed, and reducer-irrelevant. Only verified human-signed or signed-policy-certified events carry authority. |
| Decision-root metadata changes event IDs | Add only the optional existing-provenance input reference to newly created events, add cross-implementation fixtures, and do not rewrite old events. |
| Public evidence commitments leak sensitive equality | Use approved public digests only when disclosure is safe, sealed commitments when later reveal is intended, and opaque custodian references when equality is sensitive. Never commit restricted payloads or openings. |
| Transaction machinery becomes distributed infrastructure | Keep `FrontierTxn` private and single-frontier. Report independent cross-frontier completion honestly and require a separate ADR for distributed atomicity. |
| Batching recreates approval fatigue | Require a coherent grouping rule, a complete set-level brief, visible high-risk separation, and measured reviewer comprehension. |
| Portable evidence increases repository size | Deduplicate blobs by digest, enforce size limits, and support locators only when their immutability and retrieval policy are explicit. |
| Receipt compatibility breaks existing producers | Keep one backward-compatible Receipt v1, normalize once, preserve namespaced extensions, and use named profiles only after a real producer and reviewer need them. |
| Publication checks depend on unavailable remotes | Report `unknown` with evidence and never infer `pushed` from a local commit. |
| Publication corrupts or commits the user's work | Scope both delta detection and commit construction to the resolved Vela path set, preserve the caller index, inspect the candidate tree before publication, and fail on overlapping staged paths or Git ref drift. |
| A graph or wiki projection is mistaken for current truth | Root every view in exact source versions, label relation provenance and staleness, recompute signing facts from canonical replay, and make every cache disposable. |
| External Lean exfiltrates credentials or mutates the host | Treat elaboration as untrusted code, require the documented sandbox profile, remove network and credentials, mount inputs read-only, bound resources, and fail closed when isolation is unavailable. |
| Machine volume or hostile text overwhelms review | Bound parsing, rendering, queues, retrieval, decompression, and verifier work; escape terminal input; paginate; apply policy-owned backpressure; and measure review debt. |
| A new Vela layer creates prohibitive network effects | Integrate through Git, ordinary files, content digests, and thin adapters; prove value on one repository without forcing a new authoring, compute, or publication platform. |
| Ecosystem ambition turns Vela into a monolithic science platform | Keep the state, runtime, and network ownership boundaries explicit. Ship a frontier-kit convention and conformance commons; leave workbenches, education, community, package discovery, and publication to replaceable complementors. |
| Democratization is mistaken for removal of expertise or safety controls | Make evidence contribution cheap while keeping verifier scope, biosafety and dual-use constraints, semantic review, clinical responsibility, and canon authority explicit and domain-owned. |
| A teaching surface creates a weaker parallel trust model | End training in an isolated real pending receipt through the same verifier, policy, and authority path; never simulate acceptance or issue a truth-looking badge. |
| The implementation adds more concepts to user output | Keep Decision Plan, digests, and protocol IDs in Audit. Default output uses the task, claim, effect, caveat, and next action. |

## Revisit triggers

Revisit this decision if:

- outside testing shows the terminal Decision Brief cannot support accurate
  human review;
- a second independent signer requires threshold or role-separated authority;
- repository size makes committed review material impractical under measured
  workloads;
- two independent semantic-index producers and two independent consumers need
  a relation contract that Receipt v1, events, and source anchors cannot carry;
- measured clone cost requires partial-clone or sparse-checkout support without
  weakening offline availability of decision-critical bytes;
- a consumer needs cross-revision proposal identity that ADR 0002 chains cannot
  provide;
- two independent policy-governance producers and two independent consumers
  need a stable policy-head interoperability contract;
- a hardware signer requires a new signature envelope rather than raw Ed25519;
- a site or plugin can demonstrate the same exact display-to-sign binding
  without placing a browser or model in key custody.

Until one of those triggers occurs, Vela should accept complexity only in
private mechanisms when it removes more complexity from the kernel and user
workflow. Otherwise it should not build the mechanism.
