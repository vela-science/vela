# Vela ecosystem convergence addendum, 2026-08-13

## Status

This addendum governed the remaining internal work from the following entry
revisions and now records its bounded internal closeout:

| Repository | Revision | Relevant state |
| --- | --- | --- |
| Vela Core | `1e760fac4f07296b02b9b8122254588bb0e94507` | governed addendum over signed release `v0.974.2` |
| Vela Math | `b1f1a1decd565d9aa38303efaba22d2a54fdf0b8` | strict replay; correction remap, proof-discharge offer, and first bounded attempt retained |
| Vela Web | `063092cd8feff45be83c75f9403750938134442b` | `v0.436.1`; scoped Workspace and touch release |
| Formal Conjectures audit fork | `96eeecf40bc06ddc8bae6d106f461d4fd774858a` | bounded audit, source-faithful fixture, missing-tool evidence, and Comparator pilot |

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
actions remain human-only under [ADR 0046](../adr/0046-attributed-actor-decisions.md).

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
| `PILOT-01` | complete internally | Exact source execution, attributed review, Submission, Verification, Decision, replay, Work Offer, Campaign, hosted bound-lineage, and stale-anchor evidence are retained. The superseded Work Offer is closed rather than rebound again. |
| `PILOT-02` | complete internally | The bounded terminal-to-fixed question has an exact `unsupported_by_retained_basis` disposition, two attributed agent Verifications, an authorized attributed agent Decision, clean replay, and a complete relation/obligation remap. |
| `PILOT-03`, `FC-08`, `EXT-02` | external | Keep exact handoffs and outcomes. Do not describe pending or declined external work as internal failure or adoption. |
| `CLOSE-01` | complete internally | Final revisions, roots, negative results, deployment evidence, and external dispositions are pinned below. External adoption and further comparative evidence remain separate. |

## Internal critical path closure

The four internal closeout items have exact dispositions:

1. **Receiver handoff revision: adopted with a negative threshold result.** The
   compact handoff preserved all measured verdict, issue, provenance, and
   authority outcomes while using less than half the input tokens. Its elapsed
   ratio `0.9138522863671479` missed the frozen 20 percent improvement threshold,
   so the hypothesis remains unsupported. The program still adopts the compact
   receiver input on bounded utility grounds and retains the full rooted audit
   as fallback. Results root:
   `sha256:939dea44230b5f8dce0c3d948b255351caec64d0e640b7cc9ad4f6512df78f35`;
   disposition root:
   `sha256:30b6bf62eea20234b0296c7a92b882be8b6f07317a0340556e45480d02a19a8a`.
2. **PILOT-02 repair: closed as unsupported by the retained basis.** The exact
   terminal and fixed sources do not supply the cross-source port, hypotheses,
   or bridge theorem needed for either implication. Two attributed agent
   reviews passed the bounded scope, including the declared independent review.
   An authorized attributed agent Decision accepted only that negative scope;
   both scientific relations remain unresolved. Remap root:
   `sha256:b0aeb164a1cc9b5e6110186631703f9e104e0f8e9942f538848a49534ba2f8c9`;
   resulting Math Repository root:
   `sha256:ae41be4a91265d91967344459fa12583314ec05c5a0ebc74d8b0136195879511`.
3. **Hosted lineage evidence: retained and bounded.** Production already
   retained exact Target packet lineage through Approach, Attempt, Research
   Block, and unsigned draft, plus current/stale anchor refusals. The final
   audit found and corrected a cross-Problem Workspace-selection defect: a
   Workspace is now selected only by account membership and the exact
   Repository plus Problem. Migration
   `20260813_problem_scoped_workspaces` is live at
   `sha256:f10321db042198a32e7c6d77dedf83f9aa1ac52eb24ae97bc86240d297a5f6ce`.
   Authenticated production then showed Erdős 321 without the unrelated Erdős
   887 Workspace. Browser checks covered 320, 390, 768, 1024, and 1440 layouts,
   reduced motion, forced colors, print, coarse-pointer controls, and the
   programmatic main-content destination. These are hosted activity and release
   facts only; no scientific Standing changed.
4. **Closeout: recorded.** The source-controlled repos are clean at the pinned
   revisions above. Vela Web production deployment
   `dpl_9kiD38q9DMpMiF4YMg4bmdRcnRwp` is Ready from exact `v0.436.1` Git commit
   `063092cd8feff45be83c75f9403750938134442b` while preserving projection release
   `sha256:5c0df33530097a06a3be49cc26eb79fa65d3db5e9bc9aa7c89ecc646ec95256b`.
   The cross-layer conformance union is the final mechanical gate for this
   closeout record, not a substitute for external adoption or scientific lift.

The hosted evidence did not require a new scientific candidate or repeat a
Decision. Math later issued the remapped Erdős 887 proof obligation as a
separate source-owned Work Offer. Web may project that offer after it imports
the new Math revision; the offer grants no scientific or hosted authority.

## Post-closeout source work

Math commit `b1f1a1decd565d9aa38303efaba22d2a54fdf0b8` issues
`erdos:887:proof-discharge` against Formal Conjectures commit
`158727e43d3be335f902ac7ef6b9beb819e38c9d`. The packet root is
`sha256:aad2dd6288b36b5194f42800ba17eb53b1a3ab9594f711a4478c41dab0417a50`.
Agent, human, organization, and deterministic-tool performers use the same
evidence contract; each result records actor class, identity, runtime, method,
inputs, dependencies, independence, outputs, and limits.

The first attributed agent attempt ended
`not_proved_within_declared_bounds`. It retains the exact upstream source,
Lean 4.27 identity, compatible-cache disclosure, `sorryAx` audit, and the goal
left by exhaustive `aesop`. Result root:
`sha256:335089529d1afd1a53b4d2d8eee4c7fd387c44f021133af0b2feaa799793c987`.
The result makes no falsehood, impossibility, Verification, Decision, Standing,
or upstream-adoption claim. Math keeps the scientific offer open for other
methods while the convergence engineering program stays closed. Current source
index root:
`sha256:e3d8404d881720df02719db3df4ab33d5c6ce19a8a9c1c1151a381ea6ecf62ad`;
Campaign root:
`sha256:6e5742d98b061c64e3487899861929f0fe6c6546edf74b2bf21ea62363e9554f`.

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

## External package disposition

- **FC-08, upstream Formal Conjectures adoption:** the deterministic audit is
  published on the contributor branch `codex/pr-audit-v1` at
  `96eeecf40bc06ddc8bae6d106f461d4fd774858a`. No upstream PR, comment, or claim
  of maintainer adoption is part of this program closeout.
- **PILOT-03, independent external run:** retained as an optional external
  validation lane. It is not needed to establish internal software completion,
  and no unconsented participant execution is inferred from the agent studies.
- **EXT-02, second workbench or standards adapter:** deferred until a second
  live workflow presents a concrete loss-accounted integration need. Entire,
  RO-Crate, PROV, and related systems remain reference patterns, not silently
  adopted protocol dependencies.

These are durable `external_wait` or `deferred` dispositions, not successful
adoption and not internal blockers.

## Closure rule

Internal completion means the remaining bounded interfaces have an exact
result and disposition, every current repository replays, every Web projection
rebuilds from pinned source, and each external package has a durable handoff or
recorded outcome. It does not mean Formal Conjectures adoption, independent
scientific validation, federation, funding, or measured scientific lift.

If a result is negative, retain it and make the corresponding interface
decision. Do not keep a work package open merely to seek a favorable result.
