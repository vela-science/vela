# Vela ecosystem convergence addendum, 2026-08-13

## Status

This addendum governs the remaining internal work from these exact revisions:

| Repository | Revision | Relevant state |
| --- | --- | --- |
| Vela Core | `0264d0d34f35441468419a0a8fadb8df05d45c85` | signed release `v0.974.2` |
| Vela Math | `f09e93ef7c78d2e0ab58b9b7b7ec4d9e4ff27a5d` | strict replay; two accepted Claims; empty Decision Inbox |
| Vela Web | `ceda91f1c9de9c25770194391249385e09aacada` | current Campaign and Work Offer projection |
| Formal Conjectures audit fork | `96eeecf40bc06ddc8bae6d106f461d4fd774858a` | bounded audit and Comparator prototype |

The source program memo, `VELA_ECOSYSTEM_CONVERGENCE_EXECUTION_MEMO_2026-08-12.md`,
is retained as a historical planning input with SHA-256
`4dfef11f56497fe029204919e810dcfb9d8a9597a767681bd17155c57f1f6fda`.
This addendum replaces its stale performer restrictions, records work completed
since its evidence cutoff, and narrows the unfinished critical path. It does not
change Protocol 1.

## Performer and authority rule

A reviewer or scientific Decision performer may be a human or an agent. Actor
class is provenance, not a quality rank. Each review retains the performer
identity and class, method, exact inputs, session or checkpoint when available,
scope, outcome, limitations, and shared dependencies. Consolidating several
reviews is another attributed review, not anonymous consensus.

Repository authority, current roots, policy, signature, and replay govern a
Decision. Producer or verifier status does not grant that capability. A
Decision records the performer's own identity and session reference. Governance
actions remain human-only under [ADR 0046](adr/0046-attributed-actor-decisions.md).

The following statements in the source memo are superseded:

- `human Decision`, `human-authorized Decision`, and equivalent scientific
  admission constraints become `authorized, attributed Decision`;
- a required `human reviewer` becomes a reviewer meeting the declared method,
  capability, and independence policy;
- an actor class may be required by law, safety policy, sponsor policy, or
  governance, but not as an undocumented proxy for review quality;
- model review is not advisory because it is performed by a model. Its authority
  effect depends on the record and Repository policy, as it does for human
  review.

Verification remains separate from Decision. A review pass, Git merge, test,
signature, agent count, or human count does not change Standing.

## Evaluation disposition

`EVAL-01` now has two separate evidence lanes.

### EVAL-01A: attributed agent feasibility

This lane is complete for one declared performer configuration. Vela Math
retains 60 fresh task contexts at commit
`f09e93ef7c78d2e0ab58b9b7b7ec4d9e4ff27a5d`. The performer provenance is
OpenAI `gpt-5.6-sol`, Codex CLI `0.145.0`, high reasoning effort. Senders and
receivers used different fresh task contexts while sharing the same model,
provider, runtime, runner, operator account, and frozen ground truth. This is
task-context separation, not institutional, provider, or model-family
independence.

The completed results root is
`sha256:7c2a05601a19a3bcdb9ae646262077787c75ead9557cee44246d6e2308354ab0`.
The first failed execution is also retained. It produced no model outputs
because the output schemas were rejected before inference.

The measured result is mixed:

| Question | Result | Evidence |
| --- | --- | --- |
| H2, does the audit improve initial review? | supported | treatment/control elapsed ratio `0.7561`; 90% dyad-slot interval `0.6950` to `0.8323`; correct verdicts `13/15` versus `11/15`; consequential issues `6/6` versus `5/6` |
| H5, does the audit improve receiver continuation? | not supported | ratio `0.9241`; 90% interval `0.8004` to `1.0773`; correct verdicts tied at `12/15`; missing provenance fields in `15/15` sessions in both conditions |

Treatment used more input tokens in both roles. No authority-boundary violation
was observed. These results compare two interfaces for one model and five
fixtures. They do not rank humans and agents, establish general reviewer
quality, or change Formal Conjectures or Vela authority state.

The interface disposition is `revise`. Retain the audit for initial review.
Before another evaluation, replace the receiver's large inherited packet with
a bounded handoff containing the exact verdict, consequential findings,
evidence locators, unresolved questions, authority status, and next action.
Measure the new handoff against the present treatment. Do not add a kernel
object, review panel, or new authority step.

### EVAL-01H: frozen human study

The existing human study remains frozen and uncollected. Its population,
custody, consent, and estimand stay separate from EVAL-01A. It may be run when a
real comparative question and willing participants justify the cost. It does
not block internally controlled engineering, and its observations must never be
silently pooled with the agent lane.

For the source memo's completion contract, the required internal EVAL-01
dataset, method, results, limits, and interface disposition are satisfied by
EVAL-01A. EVAL-01H is a conditional external evidence lane.

## Work-package disposition

| Package | Disposition | Evidence or limit |
| --- | --- | --- |
| `CORE-01` | complete | Vela `v0.974.2` is signed and published; current docs and ADR 0046 carry attributed human/agent review and Decision semantics. |
| `FC-01` through `FC-07` | internally complete | The fork at `96eeecf4` retains the audit schemas, deterministic generator, five cases, projections, thin skill, and Comparator packet. Upstream adoption remains `FC-08`, external. |
| `MATH-00` through `MATH-03` | complete for their stated internal exits | Math imports the audit, retains clean and failure cases, and roots the Erdős 321 correction slice at `sha256:e43ca42426ca54c55703baaee351657015019fae36e7e627f6cda0d44b22d513`. |
| `WEB-01` through `WEB-04` | implemented and deployed | Target-bound activity, exact execution lineage, FC projection, and bounded draft selection are present. Authenticated interaction coverage remains a release-quality task, not a missing protocol interface. |
| `EVAL-01` | complete internally through EVAL-01A | Exact agent dataset and `revise` disposition recorded above. EVAL-01H remains conditional and separate. |
| `PILOT-01` | partially complete | Exact source execution, review provenance, Submission, Verification, Decision, replay, Work Offer, and Campaign evidence exist. Preserve and finish a concise hosted stale-root and successor-path record rather than rebuilding the scientific case. |
| `PILOT-02` | incomplete | The prior/current replay, bounded correction slice, and cold-reader report exist. The terminal-to-fixed-variant repair obligation is still open and has no Verification or Decision. |
| `PILOT-03`, `FC-08`, `EXT-02` | external | Keep exact handoffs and outcomes. Do not describe pending or declined external work as internal failure or adoption. |
| `CLOSE-01` | incomplete | It depends on the three remaining internal items below and durable external dispositions. |

## Remaining internal critical path

Only work that changes the completion result stays on the critical path:

1. **Receiver handoff revision.** Build and measure one smaller provenance-safe
   handoff. A result that still crosses parity is a valid `retain_source_locally`
   or `retire` decision, not a reason to add process.
2. **PILOT-02 repair disposition.** Construct and kernel-check the declared
   real-log to `Nat.log` bridges for the bounded terminal-to-fixed comparison,
   or produce exact evidence that the bridge is unsupported under the retained
   statements. Submit, verify, decide under an authorized attributed performer,
   replay, and remap every relation in the declared slice.
3. **Hosted lineage evidence.** Retain one authenticated current-root success
   and one deliberate stale-root refusal across Target, Approach, Attempt,
   Artifact, and draft. This is evidence for the implemented Web path, not a new
   workflow feature.
4. **Closeout.** Pin final repository revisions and roots, record external
   package dispositions, rerun the cross-layer conformance union, and publish
   the final handoff.

The first two tasks may proceed independently. The hosted lineage record must
use the deployed binding-aware application and must not change scientific
Standing.

## Campaign and institutional coordination

The Levers for Progress analysis is adopted only at the product layer. A
Campaign is a source-owned coordination wrapper around immutable Work Offers,
review requirements, resource facts, and program disposition. Scientific,
program, and deployment Decisions retain distinct domains and provenance. No
domain implies another.

The current Math Campaign is enough to test that shape. Do not add Campaign,
Mechanism, Resource, or Repository to primary navigation, create a global
registry, or add payment and procurement machinery. Extract a reusable
Mechanism Profile only after a second materially different Campaign shows a
stable shared contract.

## Closure rule

Internal completion means the remaining bounded interfaces have an exact
result and disposition, every current repository replays, every Web projection
rebuilds from pinned source, and each external package has a durable handoff or
recorded outcome. It does not mean Formal Conjectures adoption, independent
scientific validation, federation, funding, or measured scientific lift.

If a result is negative, retain it and make the corresponding interface
decision. Do not keep a work package open merely to seek a favorable result.
