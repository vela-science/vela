# Review provenance for human and AI verification

Status: current implementation memo
Date: 2026-08-13

## Decision

Vela permits Verification by people, AI models, organizations, and
deterministic software. Every such result is evidence. None is acceptance.
Only an authorized human Decision and its canonical Event can change Standing.

The existing `vela.verification-record.v2` remains the canonical carrier. It
already binds the signer, exact subject, method path and root, scope, outcome,
independence disclosure, output artifacts, and time. This memo adds a single
portable profile for the method bytes: `vela.review-method.v1`.

The profile answers a question the Verification Record deliberately does not:
who or what actually performed the review?

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

Entire provides a useful product precedent. Its session header shows agent
surface, model, human account, elapsed time, checkpoints, file changes, and
token use before exposing the transcript. Its commit view keeps the human
author, AI co-author, checkpoint, and exact commit together. Its review command
runs named reviewer agents separately and then uses a named judge to
consolidate their reports.

Vela adopts the visible attribution and separate reviewer records. It does not
adopt the phrase “final verdict” as an authority concept. A judge or synthesis
model may produce another scoped Verification, but it cannot create a Decision
or Standing.

References:

- <https://docs.entire.io/learn/review-and-recap-agent-work>
- <https://docs.entire.io/cli-reference/review>
- <https://docs.entire.io/guides/sessions/overview>

## Research basis

AI review is useful as a scalable second reader, not as an anonymous oracle.
Controlled studies report that language-model feedback can overlap materially
with human review and can improve reviewer coverage, while also showing
different error patterns and continued value from expert adjudication. Separate
work on model-as-judge systems finds systematic position, verbosity, and
self-preference biases. Prompt-injection research further shows that content
being reviewed can attempt to influence a model reviewer. These findings favor
separate rooted observations, explicit performer identity, disclosed methods,
and policy-based escalation over one blended score.

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

## Review policy

AI-first review is the default operating path for scalable screening:

1. run one or more exact, separately attributed AI or deterministic reviews;
2. preserve disagreements and unsupported claims instead of averaging them
   away;
3. escalate to a human when policy requires human judgment, reviewers
   disagree materially, source fidelity remains uncertain, the evidence is
   incomplete, the impact is high, or the proposed Decision would exceed the
   declared automated scope;
4. record the human review separately; and
5. keep the authorized human Decision separate from all review records.

Human review is therefore selective, not universal. A repository may admit a
Proposal under its own policy with one authorized human Decision, but it must
not relabel AI evidence as human evidence or treat review count as authority.

## Ecosystem ownership

- **Vela Core** publishes and validates the review-method profile and enforces
  its binding when authoring a Verification Record.
- **Math and other source repositories** retain exact reviewer profiles,
  prompts or instructions, inputs, outputs, and model or human attribution.
- **Vela Web projection** resolves the retained method bytes at the exact
  projected commit, checks their root, and emits typed reviewer provenance.
- **problems.science** shows the provenance summary before hashes and exposes
  the exact record without changing scientific authority.
- **Hosted activity** may link to review records but cannot manufacture a Vela
  Verification, sign for a reviewer, decide, or change Standing.

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
