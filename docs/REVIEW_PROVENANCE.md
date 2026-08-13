# Review provenance for attributed Verification

Status: current implementation memo
Date: 2026-08-13

## Decision

Vela permits Verification by people, AI models, organizations, and
deterministic software. Every such result is evidence. None is acceptance.
Only an authorized, attributed Decision and its canonical Event can change
Standing. A Decision performer may be human or agent; performer kind is
provenance, not a quality or authority rank.

The existing `vela.verification-record.v2` remains the canonical carrier. It
already binds the signer, exact subject, method path and root, scope, outcome,
independence disclosure, output artifacts, and time. This memo adds a single
portable profile for the method bytes: `vela.review-method.v1`.

The profile answers a question the Verification Record deliberately does not:
who or what actually performed the review?

The four reviewer kinds are peers. The profile records provenance. Evidence
earns weight from its named method, exact inputs, independence and shared
dependencies, retained outputs, scope, outcome, and limitations. Reviewer kind
grants neither a quality rank nor Repository authority.

For example:

- `AI model · GPT-5.6 Sol · OpenAI · gpt-5.6-sol`;
- `Human · William Blair · human:william-blair`;
- `Deterministic tool · Lean · lean · 4.22.0`; or
- `Organization · Example Lab · org:example-lab`.

The Verification Record identity remains the actor that attested and signed
the observation. For an AI review, that actor is the agent or service holding
the signing key; the model does not falsely claim key custody. For a human
review, the human actor may attest directly. The review-method profile binds
the performer and the attesting actor together without collapsing them.

## Why this shape

Entire provides a useful product precedent. It retains per-agent sessions and
checkpoints, and surfaces the agent, model, runtime, tools, token or resource use,
elapsed time, file changes, human account, and exact commit when those facts are
available. Its review flow keeps named reviewers separate before a named judge
consolidates their reports.

Vela adopts visible provenance and separately retained reviewers. Session and
checkpoint details remain links to source-owned activity evidence. Protocol 1
adds no fields for them. A judge, panel, or synthesis model produces another
attributed review. Consolidation preserves its inputs and creates no Decision
or Standing. Vela does not adopt “final verdict” as an authority concept.

References:

- <https://docs.entire.io/learn/review-and-recap-agent-work>
- <https://docs.entire.io/cli-reference/review>
- <https://docs.entire.io/guides/sessions/overview>

## Research basis

Reviewer kinds can overlap materially and fail differently. Controlled studies
report useful language-model feedback and improved coverage in some settings,
while model-as-judge work finds systematic position, verbosity, and self-
preference biases. Prompt-injection research further shows that reviewed content
can influence a model reviewer. Human and organizational review also vary with
expertise, conflicts, procedure, and access to the exact basis. These findings
favor separate rooted observations, explicit performer identity, disclosed
methods, and evidence-specific adjudication over a kind hierarchy or blended
score.

The policy also follows the broader risk-management direction in NIST's
Generative AI Profile: document provenance and limitations, evaluate in the
deployment context, monitor failures, and keep accountable human governance.
Nature Portfolio's editorial policy provides a useful disclosure boundary in a
different domain: AI assistance must be transparent and humans remain
accountable for the scientific and editorial act.

Research and policy references:

- Liang et al., *Can large language models provide useful feedback on research
  papers?* <https://arxiv.org/abs/2310.01783>
- *The Impact of AI Feedback on Research Peer Review*, randomized ICLR study:
  <https://arxiv.org/abs/2504.09737>
- Zheng et al., *Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena*:
  <https://arxiv.org/abs/2306.05685>
- NIST, *Artificial Intelligence Risk Management Framework: Generative
  Artificial Intelligence Profile*: <https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.600-1.pdf>
- Nature Portfolio, *Artificial intelligence*: <https://www.nature.com/nature-portfolio/editorial-policies/ai>

## Contract

`vela.review-method.v1` is a closed, canonical JSON document with:

- a stable profile and property;
- the review question;
- one reviewer descriptor: `human`, `ai_model`, `organization`, or
  `deterministic_tool`;
- a human-readable name plus stable identifier, provider, and version where
  those facts exist;
- the exact Vela actor that attests the result;
- the procedure and required output;
- explicit statements of what the review does not establish.

The profile does not contain an outcome. Outcomes belong to individual signed
Verification Records. It does not contain a Decision, Standing, confidence
score, or acceptance recommendation.

When `vela verification record` receives a method declaring this schema, it
must fail closed unless:

1. the file is canonical JSON and satisfies the closed profile;
2. `--profile` equals the profile in the file;
3. the observed property equals the property in the file;
4. `--as` equals the profile's `attested_by_actor_id`; and
5. the Verification Record repeats every method-level nonclaim.

`vela verification record --output <path>` retains each tracked, clean review
report as a content-addressed Artifact in the same transaction and binds its
digest through `output_artifact_ids`. A report cannot be supplied from dirty or
untracked bytes. This keeps model findings, human notes, and synthesis outputs
auditable without putting prose or model transcripts in the canonical
Verification schema.

Other retained verifier methods remain valid. They project as legacy
provenance until migrated or superseded; readers must not invent a model or
person from an opaque profile name.

## Product treatment

Normal Problem and Proposal views lead with one plain-language provenance
line:

> AI review by GPT-5.6 Sol · recorded by agent:codex-review

or:

> Human review by William Blair

Outcome remains beside that line on the Verification axis. Exact performer
identifier, provider, version, profile, actor, record ID, record root, method
root, independence, and nonclaims are available in the same record disclosure.
The UI never labels an AI synthesis as a human review and never labels either
as accepted.

Unknown, malformed, or unsupported review profiles fail closed in projection.
A Verification whose method is not a review profile remains visible with its
signed actor and method profile, explicitly labeled as legacy provenance.

## Review composition policy

1. Select reviewer methods for the property and risk, not for a preferred kind.
2. Retain each human, AI-model, organization, or deterministic-tool review as a
   separate attributed observation.
3. Preserve disagreements, unsupported claims, and shared dependencies instead
   of averaging them away.
4. Attribute a judge, panel, or synthesis as another review and retain the input
   review roots it considered.
5. Evaluate weight from method fitness, exact basis, independence, outputs,
   scope, and limitations.
6. Keep the authorized Repository Decision separate from every review, and
   retain the Decision performer's human or agent identity and session reference.

A repository may require any bounded combination of reviewer methods. It must
not relabel one reviewer kind as another, treat review count as authority, or
infer that a human review is independent merely because a person performed it.

## Ecosystem ownership

- **Vela Core** publishes and validates the review-method profile and enforces
  its binding when authoring a Verification Record.
- **Math and other source repositories** retain exact reviewer profiles,
  prompts or instructions, inputs, outputs, and model or human attribution.
- **Vela Web projection** resolves the retained method bytes at the exact
  projected commit, checks their root, and emits typed reviewer provenance.
- **problems.science** shows the provenance summary before hashes and exposes
  the exact record without changing scientific authority.
- **Hosted activity** may link to review records and Decision provenance but
  cannot manufacture a Vela Verification, impersonate a reviewer or Decision
  performer, hold Repository authority credentials, or change Standing itself.

## Acceptance gates

- Human and AI fixtures are distinguishable in canonical bytes and UI copy.
- Model provider, identifier, and known version survive projection without a
  display-name heuristic.
- Unknown fields, mismatched actors, property drift, profile drift, missing
  nonclaims, noncanonical bytes, and method-root drift refuse.
- Multiple reviewers remain multiple Verification Records.
- A synthesis reviewer is attributed like any other reviewer and has no
  authority effect.
- Signed-out public reads, keyboard navigation, narrow screens, zoom,
  forced-colors, reduced motion, and print preserve reviewer kind and outcome.
- No surface says that review, pass, consensus, merge, or publication is a
  Decision or Standing.
