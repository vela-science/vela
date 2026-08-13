# ADR 0046: Capability-based, attributed Decisions

- Status: Accepted
- Date: 2026-08-13
- Supersedes: the human-only Decision restriction in ADR 0032
- Protocol effect: Decision performer provenance
- Authority effect: agents may hold the reviewer role; governance remains human-only

## Context

Vela correctly separates evidence from authority: Verification reports a scoped
check, while a Decision changes Repository Standing. It incorrectly made actor
kind part of that authority rule. A human could decide and an agent could not,
even when both used the same exact Proposal, read set, policy, Repository
authority signature, and replay checks.

Actor kind is provenance, not a quality rank. Human and agent decisions can each
be careful or poor. The useful distinction is who or what acted, under which
source-owned session or checkpoint, and which Repository capability authorized
the transition.

Entire's Git-native agent checkpoints are a useful product precedent: agent
session provenance is retained beside a change without pretending the agent is
the Git authorization mechanism. Vela applies that separation to scientific
state.

## Decision

`vela review accept` and `vela review reject` admit either a `human:` or
`agent:` performer. `--as` names the performer; `VELA_ACTOR_ID` is the
non-interactive equivalent. `--session-ref` or `VELA_SESSION_REF` may bind a
source-owned session, checkpoint, or run reference. Vela stores the performer
class, actor id, optional session reference, and the distinct authority
principal in the rooted Decision plan and every resulting Event.

Repository authority remains capability-based and fail closed:

- the exact current Proposal, Claim, Submission, Verification set, Repository
  root, policy root, event-log root, action, reason, performer, and session
  reference are bound before signing;
- the retained authorization model must grant the exact principal the reviewer
  role;
- the Repository authority key signs the covering transaction;
- stale entry roots, changed inputs, missing evidence, or unauthorized
  principals refuse before Standing changes; and
- replay derives the same Standing from the attributed Events.

Human and agent principals may hold the reviewer role. Authority initialization,
rotation, closure, policy, membership, recovery, and destructive governance
remain human-only. Reviewer kind does not imply independence, correctness, or
scientific weight.

For compatibility, omitting performer attribution records the existing local
operator identity as `human`. Automated agents should set an explicit
`agent:<name>` identity. A declared actor id proves what the authority signer
attested; it does not prove that a particular model provider operated the key.
Model, method, inputs, outputs, and limitations remain separately rooted review
provenance.

## Consequences

The scientific loop is no longer bottlenecked on a human actor class. A
Repository may authorize an agent to decide routine, policy-ready Proposals or
retain human performers for selected lanes. Both produce the same canonical
Decision and replay semantics, while public readers can distinguish the
performer without guessing from names.

Verification still does not become acceptance. An agent pass, a human pass,
consensus, CI, merge, and publication remain evidence until an authorized
Decision occurs. The change removes actor hierarchy, not the exact transition
boundary.

## Acceptance

- An attributed agent can accept an exact protocol-ready Proposal end to end.
- Agent id, class, and session reference survive Decision output, Events,
  clean-clone replay, and `why`.
- Actor or session substitution changes the Decision plan root.
- Event performer drift, missing provenance, wrong authority principal, stale
  entry root, and unauthorized agent membership refuse.
- Historical human Decisions replay byte-for-byte.
- Current CLI and product copy describe authorized attributed actors rather
  than a human-only lane.
