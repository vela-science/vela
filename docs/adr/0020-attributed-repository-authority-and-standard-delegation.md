# ADR 0020: Attributed repository authority and standard delegation

- Status: Proposed
- Target release: Vela `v0.930.0`
- Protocol effect: new authority keyset, policy bundle, authority record, and
  transaction-linked event forms
- Product effect: replace personal Vela key setup and `vela-signer` with
  ordinary authentication, scoped delegation, and one repository-authority
  transaction edge
- Scientific authority effect: preserve human or governed scientific judgment;
  change how that judgment is authenticated, authorized, and recorded
- Evidence update: the real repository-authority Decision loop is complete at
  Erdős commit `80606bdccb51fa86524111a1a61876bb08e45d79`.
- Remaining acceptance gate: retain Proposed status until an uncoached fresh
  user comprehends and safely completes the released Decision path.
- Compatibility: all Vela `0.915` history remains byte-identical and
  read-verifiable; only the new authority model has a live writer after
  migration
- Entry gate: source audit of the released `0.915.1` authority stack, repeated
  first-party usability failure, and the decision-grade authority architecture
  review dated 2026-07-24

## Context

Vela protects the right invariant through the wrong identity and signing
model.

The invariant is:

```text
evidence is not a verdict
verification is not acceptance
an authorized decision changes standing
the exact transition is replayable
corrections append
```

That invariant is Vela's core contribution and remains unchanged.

The released implementation additionally makes Vela responsible for:

- long-lived human Ed25519 identities;
- per-Frontier actor-key registration;
- raw seed custody through a custom cross-platform helper;
- OS authentication and prompt adapters;
- signed local signer sessions;
- binary and helper digest rebind ceremonies;
- manually echoed Decision Plan roots and timestamps;
- three generations of a custom AcceptancePolicy language; and
- long-lived agent credentials used as policy inputs.

The implementation is rigorous, but it is not a non-exportable signing
boundary. The protected helper retrieves a raw Ed25519 seed into process memory
before signing. It pays the product and platform cost of a bespoke custody
system without delivering the lifecycle, recovery, institutional membership,
or hardware-backed guarantees of a mature identity system.

AcceptancePolicy v0.1 through v0.3 closed real authority gaps. Their evolution
also demonstrates that Vela is becoming an authorization-language vendor.
Exact verifier, packet, result-contract, replayability, and producer-credential
constraints are legitimate authorization inputs. They do not justify a fourth
custom policy generation.

The current Decision Plan mechanism is valuable internal transaction plumbing.
Asking a person to copy its digest and timestamp is not. A person should decide
the scientific meaning shown in a contextual review surface. The transaction
edge should rederive and bind the exact bytes.

The target is not “just trust a database” and it is not “make GitHub the
authority.” The target is portable process authority:

```text
authenticated principal
  + current role or scoped delegation
  + restricted Cedar decision
  + semantic approval when consequence requires it
  + final read-set recheck
  + repository-authority signature
  -> attributable scientific authority
```

## Decision

Adopt attributed repository authority for every new write after either an
explicit historical migration boundary or a fresh structural initialization.

### 1. Keep the scientific-state kernel

Retain:

- canonical JSON and full SHA-256 identities;
- Receipt v1 as the producer/evidence boundary;
- deterministic verifier attachments;
- proposal objects;
- append-only events and corrections;
- deterministic replay and clean-clone verification;
- the recoverable transaction journal;
- exact intent and read-set binding;
- human judgment for exceptional scientific decisions;
- prior human authorization for automatic policy lanes; and
- structural prevention of agent self-approval.

Git remains transport, lineage, and distributed backup. Git publication,
artifact provenance, deterministic verification, policy authorization, and
scientific standing remain separate facts.

### 2. Replace personal signing with one authority transaction

Introduce three canonical object families and one new event form:

```text
vela.authority-keyset.v1
vela.policy-bundle.v1
vela.authority-record.v1
vela.event.v1
```

A transaction may create multiple canonical objects and events. One authority
record covers the complete transaction:

- operation and transaction ID;
- exact internal `intent_digest`;
- previous authority-record digest and sequence;
- before and after event-log roots;
- every generated event ID;
- a canonical object-delta manifest;
- authenticated principal and attribution snapshot;
- delegation or workload claims;
- Cedar request, bundle, entity snapshot, outcome, diagnostics, and determining
  policies;
- semantic approval and quorum records when required;
- final read-set and execution identity;
- authority key ID; and
- one repository-authority signature.

The signature means:

> The repository authority attests that this exact transaction passed the
> recorded authentication, authorization, approval, verification, quorum, and
> final-state checks.

It does not mean that a personal key signed every event.

### 3. Use DSSE as the authority envelope

The canonical `vela.authority-record.v1` bytes are wrapped in one DSSE envelope
with payload type:

```text
application/vnd.vela.authority-record.v1+json
```

DSSE authenticates both payload type and payload and avoids inventing another
signature envelope. The authority-record payload remains canonical JSON for
Vela content addressing. DSSE key management is intentionally out of scope;
the authority keyset supplies the Frontier-specific verification history.

An authority record ID is derived from its unsigned canonical payload. The
DSSE envelope, signature, and authority key ID are verified before the record
may enter the authority log.

### 4. Separate human identity from authority keys

`vela.principal.v1` uses an exact namespaced issuer-subject identifier:

```text
local:<device-id>|uid:<uid>
oidc:<issuer>|<subject>
orcid:<issuer>|<subject>
```

A human principal ID must equal one retained local, OIDC, or ORCID account
link. It is never inferred from email, display name, affiliation, a GitHub
handle, or an unlinked ORCID value. Display name and affiliation are readable
snapshots only. Current account links and governed role bindings determine
authorization. Revocation appends to the account-link history rather than
silently reassigning a principal.

Non-human principal namespaces are explicit:

```text
agent:<provider>:<agent-id>:<run-id>
workload:<provider>:<workload-id>
service:<service-id>
institution:<institution-id>
```

Human authentication is replaceable:

- solo local mode uses the existing OS login session;
- online workbenches may use passkeys/WebAuthn or OIDC;
- institutions may use SSO and managed lifecycle;
- disconnected high-assurance mode may use a standard hardware-backed direct
  signer over the same authority-record payload.

Authentication proves control of a session or authenticator. Cedar determines
authorization. A semantic action supplies transaction approval. None alone
implies scientific correctness.

### 5. Make the repository authority a service identity

`vela.authority-keyset.v1` records:

```text
frontier_id
generation
threshold
keys[]
  key_id
  algorithm
  public_key
  valid_from_sequence
  valid_through_sequence
  purpose
previous_keyset_root
activation_record_root
```

The repository authority is not a person and never appears as the scientific
reviewer. Its private key may be held by a standard provider:

- SSH agent for the first solo local implementation;
- PKCS#11 token;
- cloud KMS or HSM;
- GitHub App authority process; or
- institutional authority service.

The provider signs only an already validated authority-record payload. It
receives no proposal prose, model output, repository file paths, or policy
authoring context beyond the digest and domain-separated payload required to
sign.

### 6. Replace AcceptancePolicy with restricted Cedar

Freeze AcceptancePolicy v0.1 through v0.3 as legacy replay inputs. There will
be no AcceptancePolicy v0.4 and no new legacy policy writer after migration.

`vela.policy-bundle.v1` binds:

```text
manifest
schema.cedarschema
policies.cedar
entities.json
tests/
engine name and exact version
restricted Vela profile version
previous bundle digest
activation authority record
human-readable authority summary
```

The first implementation pins `cedar-policy` exactly and disables its
top-level default features. Cedar core currently still compiles its built-in
extension implementations, so Vela additionally rejects extension
constructors and extension values before evaluation; compiled availability is
not protocol availability. The restricted Vela profile permits no network
access, file I/O, runtime policy fetch, model decision, nondeterministic score,
or registered extension value.

Closed policy and authority-record types remain in dependency-light
`vela-protocol`. The Cedar runtime lives in the narrow `vela-authority` crate.
Protocol consumers that only replay or inspect canonical records therefore do
not inherit the policy engine or its dependency graph.

Every policy and request is schema-validated before evaluation. Cedar's
authorization API skips an individual policy that errors; Vela therefore
applies the stricter application rule:

```text
any validation error
or any evaluation diagnostic
or any unknown action/entity/attribute
=> no automatic authorization
```

Cedar returns Allow or Deny. Vela preserves its domain routing:

- structural invalidity or an applicable `forbid` is `Deny`;
- valid proposal creation without authorization to auto-admit is `Defer`;
- one applicable `permit`, no forbid, no diagnostics, and all deterministic
  gates passing is `Permit`.

Quorum counting remains a small deterministic Vela state machine. Cedar
determines whether a principal may cast a decision; it does not implement a
temporal voting protocol.

### 7. Use short-lived capabilities for agents and workloads

Agents and workloads are non-human principals. They use short-lived,
audience-, Frontier-, resource-, action-, and expiry-bound capabilities.

The first capability profile is the closed, content-addressed
`vela.capability-grant.v1`. It records:

```text
issuer
subject
subject class
current actor
actor chain
parent capability full root
delegation depth and maximum depth
audience
frontier
actions
resource bindings
exact execution bindings
consequence ceiling
issued_at
not_before
expires_at
token_id
revocation reference
```

The profile permits only `agent` and `workload` subjects, the exact
`vela.repository-authority.v1` audience, full resource and execution roots,
and a maximum lifetime of 24 hours. Delegation depth is at most one. A child
must name the full parent root and may only narrow action, resource, execution,
time, and consequence scope. Expiry and revocation are evaluated at the
authority record's observation time, so a backdated operation cannot revive a
grant.

Runtime bearer credentials are never committed. The authority record retains
only `vela.verified-capability-claim.v1` fields and the grant identity, never a
JWT, CWT, OAuth token, refresh token, cookie, or provider assertion. OAuth
token exchange, OIDC, SciTokens, GitHub App identity, and local credentials are
replaceable adapters, not Vela protocol dependencies. DPoP or SPIFFE is added
only for a reproduced network or institutional workload threat.

An agent or workload can never obtain:

```text
authority_migrate
authority_revoke
authority_rotate
bulk_correct
destroy
membership_manage
policy_activate
policy_revoke
policy_rotate
quorum_manage
recovery_approve
review_accept
review_reject
```

Those actions are absent from the capability action enum and are also rejected
by the shared application invariant before Cedar evaluation. The same closed
list is used by `vela-protocol` and `vela-authority`, so policy text cannot
manufacture an agent or workload decision path.

### 7A. Proposed amendment: scoped Agent Campaigns and batched semantic review

Status: **Proposed, design-only, 2026-07-30.**

This amendment responds to a concrete dogfood failure: a multi-hour or
multi-day agent run cannot stop for human verification or repository-signing
prompts every few minutes. That interaction is not safer. It trains the user
to approve mechanically, hides the few decisions that matter, and makes the
scientific workflow materially worse than ordinary Codex, Claude Code, or
research-workbench use.

The product rule is:

> Approve bounded execution once. Observe everything. Interrupt only for a
> change in consequence. Apply reviewed scientific decisions in one exact
> authority transaction.

The cold-researcher flow is one coherent path:

```text
choose Frontier and Target
  -> authorize one bounded campaign
  -> agents run and append evidence without interruption
  -> inspect the few meaningful outbox entries
  -> apply one reviewed batch
  -> see the exact new Standing and next obligation
```

The north-star dogfood case is one agent working for twelve hours after one
campaign authorization and returning one or a few consequential review items,
with no intervening human prompt unless the approved scope, budget, risk, or
consequence changes.

The invariant does not change:

- evidence is not a verdict;
- Verification is not acceptance;
- an agent cannot accept, reject, or broaden its own authority;
- accepted transitions replay from retained bytes; and
- corrections append instead of replacing history.

#### Existing seam

Most of the required substrate already exists:

- `vela.attempt.v3` binds one Target, actor, task contract, exact repository
  read set, and expiry while creating no Event, authority record, or Standing;
- `canopus.activity.v0` is an append-only, hash-linked run log, and
  `canopus.run.v2` is explicitly non-authoritative;
- Submission and Verification Record signatures already authenticate their
  exact producer or verifier bytes without becoming scientific authority;
- `AuthorityTransactionRequest` already accepts multiple Event and object
  drafts, one exact read set, multiple semantic approvals, and one recoverable
  journaled commit; and
- the current Decision path already rederives the Proposal, Claim, Submission,
  Verification set, policy, authority head, binary, reason, and action under
  the write barrier before authentication or signing.

The gap is product composition, not a second authority system. The generic
`vela.capability-grant.v1` types exist in protocol fixtures, but no current
Submission, Verification, or Decision writer uses them and the four active
Frontiers retain no such canonical object. `AuthorityRecordV1` nevertheless
has a typed reader for `VerifiedCapabilityClaimV1`, so implementation must not
delete the retained v1 read shape.

Before implementation, repeat that inventory. If it remains empty, section 7's
generic capability proposal is superseded for current writers by the narrower
execution lease below. Keep the v1 reader and conformance fixtures, remove the
unused generic writer, and do not add an `AuthorityRecord` version merely to
carry campaign activity. Campaign execution creates no authority record after
lease issuance and before a reviewed Decision.

#### Execution lease

Add one non-scientific, repository-authority object:

```text
vela.execution-lease.v1
  lease_id and full root
  Frontier ID
  exact Target IDs and roots
  starting repository, packet, and execution roots
  approved campaign-plan root
  agent or workload principal and public identity
  isolated readable and writable roots
  allowed operations
  allowed Artifact types
  tool, network, sandbox, and publication constraints
  wall-clock, model-call, token, compute, spend, storage, and parallelism budgets
  issued_at, not_before, and expires_at
  optional predecessor-lease root
  revocation reference
  consequence ceiling: evidence_only | pending_review
```

One human-only Cedar action, `execution_lease_issue`, may issue the lease
through an object-only authority transaction. Acceptance of this amendment
must add it to the closed structural human-only action list and Cedar schema
and policy. Its `SemanticApprovalV1.intent_digest` binds the full exact lease
root, including the approved campaign-plan root. The normal `start` flow
should expose that action; this amendment does not justify another top-level
CLI command.

The lease authorizes only the execution plane:

- inspect exact source and Frontier state;
- create isolated baseline and child experiment worktrees;
- run allowed tools and verifiers;
- append run receipts and failures;
- freeze allowed Artifacts;
- draft Submissions and Verification Records; and
- emit escalation intents that Vela deterministically classifies for the
  review outbox.

It never authorizes:

- a canonical scientific Decision;
- policy, schema, membership, quorum, or repository-authority change;
- publication outside the approved local evidence store;
- destructive or high-impact action;
- a budget or risk increase;
- access to a human or repository-authority key; or
- creation, renewal, widening, or approval of its own lease.

The execution controller may refresh short-lived provider credentials inside
the exact active lease without human interaction. Those credentials and bearer
bytes are runtime state, not canonical Vela objects. Refresh must fail after
expiry, exhaustion, or revocation. Scope, budget, risk, publication, or
consequence expansion produces a review item; it never silently widens the
lease.

One campaign controller is the only budget allocator for a lease. Parallel
branches reserve, commit, or release bounded allocations through one
hash-linked local ledger protected by the existing local work lock. A restarted
controller replays and reconciles that ledger before issuing another
allocation. An engine that cannot route a claimed budget dimension through the
controller may not advertise or use that dimension as enforced. This avoids
ceremonial budget fields that no component can actually police.

The controller is a thin authorization, metering, and evidence adapter around
Codex, OpenResearch, Canopus, or another native runner. It does not schedule
research, replace the runner's durable execution, or normalize its complete
internal state into Vela.

The local-first, model-agnostic
[OpenScience workbench](https://github.com/synthetic-sciences/openscience) is
another representative execution plane: native sessions, files, runs, tools,
and provenance remain in the workbench, while a thin adapter may bind exact
candidate evidence into Vela. Vela does not copy its agent runtime, connector
catalog, editor, terminal, compute layer, or project graph.

The first profile permits at most seven days because that matches the bounded
long-run lifecycle already used by research workbenches. A Frontier policy may
set a shorter maximum. Longer work requires a new reviewed lease rather than
an unbounded credential. The unused generic capability reader retains its
historical 24-hour validation rule; it does not constrain the new lease.

Lease v1 has no renewal or in-place extension. Continued work requires a new
human-issued lease that names the predecessor root and exact remaining plan.

Revocation appends a separately rooted, non-scientific revocation object
covered by repository authority. It does not mutate or delete the original
lease or invalidate evidence produced while the lease was active.

#### Campaign bundle and experiment graph

Add one portable, non-authoritative activity-plane manifest:

```text
vela.agent-campaign.v1
  campaign ID and root
  execution-lease root
  approved campaign-plan root
  exact controller and runner build identities
  append-only steering directive roots
  baseline source commit and tree
  experiment nodes[]
    node ID and parent root
    isolated worktree identity
    exact diff root
    Run and activity-log roots
    evaluation and Artifact roots
    verifier outputs
    draft Submission and Verification roots
    budget consumption
    status: starting | running | completed | failed | cancelled
  hidden-failure count and roots
  selected candidate, if any
  sealed_at
```

This is a bundle, not another canonical scientific graph. Native workbenches
retain their exact logs and formats; the manifest binds those bytes instead of
normalizing every tool event into Vela. Canopus may emit it, but Canopus is not
required for replay or authority.

[OpenResearch](https://openresearch.sh/docs/experiment-flow) is the execution
reference: a baseline has child experiments, each child has its own isolated
branch or worktree, run state, evaluation output, and exact code diff. Its run
contract also demonstrates that a bounded run may last up to seven days and
emit a structured evaluation artifact. Vela borrows that observable
baseline-to-child shape, not OpenResearch's storage, scheduler, or authority.

Campaign activity appends continuously without human approval. A sealed bundle
is immutable. A repair or continuation creates a child bundle that names the
parent root. Missing campaign bytes may make a proposed review incomplete, but
they can never make accepted-state replay depend on Canopus, OpenResearch, a
checkpoint database, or a network service.

Raw prompts, private traces, credentials, and undisclosed research-process
content remain local by default and are excluded from portable bundles.
Exporting them for evaluation or model training requires a separate explicit
consent and custody policy. Rooted metadata must not become a pretext for
centralizing private workbench history.

Before lease issuance, the user may edit the bounded campaign plan. During
execution, pause and resume do not cancel or reauthorize the campaign, and
steering inside the existing scope appends a non-authoritative directive.
Changing Target, writable roots, tools, network, budget, risk, publication, or
consequence ceiling requires a new reviewed lease or escalation. Reusable
plans are unsigned templates only; a later campaign still requires its own
fresh exact lease.

This adapts the useful interaction boundary from
[Magentic-UI](https://www.microsoft.com/en-us/research/blog/magentic-ui-an-experimental-human-centered-web-agent/):
co-plan before work, observe and steer while it runs, pause without destroying
state, guard consequential actions, and show parallel-task status. Vela does
not import Magentic-UI's agent architecture or treat a learned plan as
authority.

#### Candidate sealing and non-standing writes

The campaign path does not call the current repository-authority Submission or
Verification writer after every result. Instead it stages a closed candidate
graph:

```text
producer-signed Submission draft
  -> deterministic Claim and Proposal draft
  -> verifier-signed Verification Record draft over those exact roots
  -> sealed candidate root
  -> derived outbox entry
```

These bytes are append-only campaign evidence under producer or verifier
authentication. They create no repository-authority transaction, canonical
Proposal, Event, or Standing, and therefore require no human or
repository-authority prompt. The UI calls them candidates, not
`pending_review`.

The sealed candidate root binds every draft byte, Artifact, source commit and
tree, verifier environment and limitation, campaign node, and evidence-set
root. Later activity does not mutate or stale it. A better result creates a
new candidate root and explicitly supersedes the old candidate in the
campaign.

At an authorized batch commit, the planner deterministically derives and
installs the ordinary Submission, Registration Record, Claim, Proposal, and
Verification Record objects together with each ordinary Decision Event in the
same authority transaction. Existing direct `submit` and single-Proposal
review remain compatibility paths. This candidate-sealing path is what removes
per-step signing from a long-running campaign without granting an agent
repository authority.

#### Review outbox

Add one local, root-bound derived projection:

```text
vela.review-outbox.v1
  classifier rules and version
  Frontier and repository roots
  Standing, policy, keyset, and authority heads
  entries[]
    entry ID derived from the full entry root
    sealed candidate and campaign roots
    exact draft and evidence roots
    consequence class and requested disposition
    designated authority domain and principal rule
    prepared_at and expires_at
    semantic diff and next-Obligation roots
```

Each entry has its own full root over those inputs. The outbox index root is the
ordered root of its entries and classifier. Ongoing campaign activity cannot
change an entry because review begins only from a sealed candidate. A later
candidate, changed repository head, or changed policy creates a new entry and
marks the prior one stale rather than mutating it.

The outbox contains only consequence-bearing entries:

1. accept, reject, retract an accepted Claim, correct, or supersede canonical
   scientific state;
2. contradict current accepted state;
3. change policy, schema, membership, quorum, or repository authority;
4. publish externally;
5. perform a destructive or otherwise high-impact action; or
6. increase approved budget, tools, network access, writable roots, or risk.

Routine tool calls, checkpoints, progress messages, run receipts, Artifact
freezes, verifier passes or failures, drafts, and experiment branches never
enter the outbox. They remain visible in the activity timeline.

The interaction should follow the useful part of
[LangGraph interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts)
and the open-source
[Agent Inbox example](https://github.com/langchain-ai/agent-inbox-langgraphjs-example):
persist exact work before the interruption, surface a queue of items with
enough context to decide, and support accept, reject, revise, or defer.
Vela differs in three load-bearing ways:

- the agent does not choose which scientific actions are safe to auto-approve;
- editing evidence is forbidden—an edit derives a new intent and root; and
- resuming an execution thread is not a scientific Decision.

The current Agent Inbox is useful prior art, not Vela's product model. It
groups generic interruptions by agent thread and submits one response shape
back to the runtime. Vela instead groups by Frontier, Target, and Claim or
Proposal lineage, and maps every disposition to an exact outbox entry ID and
full root. The response protocol must never rely on array position. This
follows the stronger correlation rule in the
[Agent Protocol batch-resume contract](https://github.com/langchain-ai/agent-protocol/blob/0ff7cd3962e8b4b3e347b76203be7dfeba003928/streaming/protocol.cddl),
where every response names its interrupt ID and one batch resumes atomically.
The source review at Agent Inbox commit
[`081b2a30`](https://github.com/langchain-ai/agent-inbox/tree/081b2a30409304fa04bfcf7b01d035853b846ecd)
also confirms that its primary queue is runtime-status centric—All,
Interrupted, Idle, Busy, and Error—and may offer `Ignore Thread` as a fallback.
Those are useful Cockpit states, not Decision Inbox dispositions. A malformed
or unreadable Vela decision request fails closed and remains an explicit
repair obligation; it cannot become ignorable merely because the UI cannot
render its expected schema.

Other agent products expose “always approve” for a tool or the remainder of a
run. Vela must not. The only reusable autonomy grant is the exact, expiring,
budgeted execution lease. No inbox action may create a persistent tool
approval, wildcard, remembered answer, or classifier exception. A repeated
consequence either remains inside the existing lease or receives a newly
rooted review item.

The local review UI or CLI groups entries by campaign and consequence class.
Each row shows:

- action and exact target;
- why human judgment is required now;
- proposed scientific or external effect;
- decisive evidence and verifier scope;
- caveats and explicit nonclaims;
- current conflicts or missing checks; and
- age, expiry, and budget impact.

Selecting an entry opens the exact scientific diff and evidence chain.
Multi-select is enabled only for one Frontier and one authority domain. The
available consequence dispositions are Accept, Reject, and Request revision;
Save for later and Snooze are local triage actions rather than dispositions.
The public Observatory remains credential-free and read-only. It may display
committed Decision and retained campaign evidence, but unresolved outbox state
stays in the private local reviewer surface.

Review validity binds the exact candidate, source, and evidence bytes. A
changed source hash is visibly stale and cannot inherit a prior review. A
reviewer may still export a draft packet for inspection, but overriding a
non-authoritative reviewer finding requires an attributed reason and never
bypasses a deterministic verifier, policy, or Standing check. This borrows the
useful exact-byte review discipline of local scientific workbenches without
importing their project graph or provenance store as Vela authority.

The primary grouping is Frontier -> Target -> Proposal or Claim lineage, not
agent thread, tool, or verifier. Multiple Verification Records for one
Proposal appear under one decision. Competing Proposals remain separately
actionable. The UI may deduplicate identical presentation, but it must retain
and disclose every distinct underlying root.

Inbox submission uses a keyed request, never a positional response list:

```text
vela.review-selection.v1
  inbox_snapshot_root
  selected_entry_ids[]
  decisions
    <entry_id>
      expected_entry_root
      disposition
      reason
```

The selected-entry set may be a strict subset of the Inbox. Every selected
entry requires exactly one keyed decision; omission inside that set is invalid.
Unselected entries remain pending and do not block the selected subset.
Validation rejects unknown or duplicate IDs, wrong roots, extra decisions, and
positional substitutions before deriving the deterministic canonical batch
order.

Every entry answers five questions before internal machinery:

1. What changed?
2. What exact evidence supports or limits it?
3. What decision is requested?
4. What consequence would that decision have?
5. What Obligation or Target follows?

The queue adopts the useful parts of GitHub review and notification patterns:
filter and group unresolved work, collect pending dispositions before one
submission, keep approve and request-changes distinct, and invalidate review
when the reviewed diff changes. A stale entry is automatically deselected,
shows which exact inputs changed, and requires re-review. It is never silently
carried into a batch.

The queue also adopts the narrow useful interaction contract of
[Linear's Inbox](https://linear.app/docs/inbox): keyboard traversal, quick
filtering, explicit read state, and Snooze as a reminder action rather than a
domain disposition. Saved filters are URL-addressable so a reviewer may return
to or share one exact local view. Unlike an ordinary notification inbox, Vela
does not let archive, mark-read, delete, or snooze remove a still-required
scientific or authority obligation from the unresolved set.

Every committed effect follows
[Slack's agent-design guidance](https://docs.slack.dev/concepts/agent-design/)
to make action provenance visible. The result names the human who decided, the
repository authority that signed, and the agent or verifier that produced the
underlying work. It never compresses those roles into “Vela approved” or
implies that the requester acted with the human's identity. Rejection,
staleness, and failed commitment always show the exact next recovery action.

The interaction has two surfaces, not a raw agent outbox or stream of modal
prompts:

- **Campaign Cockpit** shows the approved and current plan, active baseline and
  child branches, live status, retained failures, Artifacts, verifier scope,
  budget consumption, steering history, and next checkpoint without asking
  for a decision; and
- **Decision Inbox** shows only unresolved outbox entries, grouped by
  consequence and campaign, with one persistent batch action bar.

##### Narrow adoption from Buzz

A source review of
[Block Buzz at commit `bd0bff24`](https://github.com/block/buzz/tree/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c)
adds three useful implementation disciplines without changing Vela's product
or authority boundary.

First, the Cockpit activity projection uses Buzz's
[verb, object, outcome](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/VISION_ACTIVITY.md#the-governing-frame-verb-object-outcome)
frame. Each visible row answers what happened, to which exact object, and with
what outcome before exposing raw details. The row also binds its campaign,
experiment-node, Run or run-start, and source activity roots. Routine reads,
heartbeats, and transport chatter remain available in the raw activity log but
are coalesced or suppressed from the primary view. This is a disposable
presentation projection over retained evidence, not a new event or Standing
model.

Second, in-scope steering is exact and positively acknowledged. Buzz's ACP
adapter binds its native steer to the
[current expected run ID](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/crates/buzz-acp/src/pool.rs#L310-L330)
and treats only
[recognized outcomes](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/crates/buzz-acp/src/acp.rs#L1529-L1633)
as delivery. A Vela campaign steering directive therefore binds:

```text
campaign, lease, experiment-node, and active-run identities
expected last activity or checkpoint root
issuer, issued_at, and exact directive body root
```

The controller appends a correlated acknowledgment naming the directive root,
the run that received it, and one closed outcome:

```text
applied_in_run | started_successor_run | rejected | completed_before_delivery
```

Only the first two outcomes count as delivery. Missing, unknown, mismatched, or
transport-only success remains visibly unacknowledged and must not be reported
as applied. The acknowledgment changes neither lease scope nor Standing. A
directive that would change Target, budget, tools, writable roots, network,
risk, publication, or consequence instead creates a new reviewed escalation.

Third, outbox creation and disposition use the useful storage guards in Buzz's
workflow approval path without adopting that workflow engine. Buzz records a
pending approval with an expiry and designated approver, then uses a
[`pending` compare-and-swap](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/crates/buzz-db/src/workflow.rs#L1050-L1120)
so concurrent grant and deny cannot both succeed. Its command path also checks
pending state, expiry, and the named approver before atomically committing the
command event and status change. Vela applies a stricter root-bound form:

- sealing a candidate and durably creating its outbox entry is one journaled
  projection operation, or an equivalent atomic transaction;
- the full entry root is the deduplication key, so replay or restart cannot
  create a second logical item for the same candidate, classifier, and read
  heads;
- a disposition may consume only a `pending`, unexpired entry whose exact
  designated human authority and authority domain match the authenticated
  principal;
- the transition from `pending` to `batched` uses compare-and-swap over the
  exact entry root and Inbox snapshot root; and
- final batch commitment rederives every selected entry root and the complete
  transaction read set immediately before confirmation and again under the
  repository-authority write barrier.

Local read, save, snooze, selection, and draft-reason state remains a separate
triage sidecar. It never participates in the pending compare-and-swap, extends
an expiry, changes the designated authority, or becomes a scientific
disposition.

The boundaries are equally important. Vela does **not** adopt Buzz's chat,
channel, forge, Nostr relay, or
[one identity and event substrate](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/README.md#why-buzz-is-better).
Agents do not receive human authority parity merely because both produce
attributed activity. Native runners and workbenches retain their own logs;
Vela retains separate execution evidence, Verification, human or governed
Decision, Event, and Standing planes.

Buzz is prior art for these seams, not proof of a finished approval system. At
the reviewed commit its own architecture says the approval schema, database,
API, and UI exist while the executor
[still fails rather than persisting and resuming an approval gate](https://github.com/block/buzz/blob/bd0bff24bfd2cffa2b3b3a995f7628af5e460a5c/ARCHITECTURE.md#L548-L553).
Vela may not cite Buzz as release evidence and may not import its workflow or
approval engine.

A blocked experiment branch may appear in Decisions while independent branches
continue in the Campaign Cockpit. The product may notify once when the first
unresolved entry appears and once before lease expiry; it must not notify on
routine progress, every verifier completion, or credential refresh.

Routine receipts, checkpoints, and verifier completions roll into a bounded
informational digest in the Cockpit. They never appear as Decision Inbox rows.
Pause, resume, or in-scope guidance is available from the Cockpit and appends
to the steering log without changing the lease.

Inbox triage state is a versioned local sidecar keyed by reviewer ID, entry ID,
and exact entry root. It may contain only read state, saved state, snooze time,
follow preference, draft disposition, and selected batch. `Snooze` may carry an
explicit wake time or state condition but cannot extend beyond a real deadline
or lease expiry. `Request revision` appends reviewer guidance and requires a
new candidate root; `Dismiss` removes only a local reminder for an abandoned
candidate. None changes the deterministic outbox projection, campaign roots,
authority read set, or Standing. Any changed entry root creates a successor
that does not inherit selection, draft reason, dismissal, or snooze. “Follow”
belongs to Cockpit notification preferences, not to scientific disposition.

The UI says `Save for later` or `Snooze`, not `Defer`, because Defer could be
mistaken for a scientific disposition. Done, Dismiss, Unfollow, and Snooze
change reminder presentation only. They cannot remove a required Decision,
correction, policy action, or lease-expiry condition from the authoritative
unresolved view.

The durable interaction model follows the narrow useful boundary in the
[OpenAI Agents SDK human-in-the-loop flow](https://openai.github.io/openai-agents-python/human_in_the_loop/):
approval policy attaches to action classes, ordinary trusted actions continue,
multiple pending sensitive actions may coexist, and serialized run state can
resume later. Vela does not make tool approval scientific authority; it uses
that durability only for the execution plane.

Persisted resumable state uses an activity-plane envelope:

```text
vela.agent-campaign-resume.v1
  resume schema and serializer or adapter version
  checkpoint ID and full root
  campaign and lease roots
  controller and runner build identities
  agent-definition root
  tool-schema and policy roots
  model and configuration root
  pending interruption IDs and payload roots
  last activity root
```

It excludes bearer credentials, human or repository-authority material,
private prompts, raw provider session secrets, and serialized sticky approval.
If any bound input drifts, the campaign remains inspectable and exportable but
does not silently resume under changed semantics. Lease-root change, expiry, or
revocation also invalidates any pending runtime approval.

The local UI uses a semantic list or table with native selection controls,
visible keyboard focus, and an equivalent non-visual reading order. Opening
detail preserves and restores focus. Status changes are announced, and a
destructive confirmation initially focuses the least destructive action.
These are interaction requirements, not justification for a custom component
library.

Workflow state is distinct from scientific disposition:

```text
candidate active -> superseded
outbox item pending -> saved | snoozed | revision_requested | dismissed | batched | stale
Proposal unregistered -> pending_review only after canonical registration
Proposal pending_review -> accepted | rejected through an authorized Decision
```

Save, Snooze, Dismiss, and Follow affect only the local review workflow. They
never masquerade as a canonical disposition, delete a candidate, or hide a
mandatory unresolved obligation.

#### Batched review plan

Add one retained, non-standing review object:

```text
vela.review-batch.v1
  batch ID and full root
  Frontier ID
  authority domain
  ordered entries[]
    outbox entry, campaign, and sealed-candidate roots
    draft or retained Proposal, Claim, Submission, Verification-set, and Artifact roots
    planned canonical roots when the candidate is not yet registered
    explicit requested action
    exact human reason and limits
    proposed per-entry state diff
    underlying policy evaluations
  repository, authority, keyset, and policy heads
  complete transaction read-set root
  conflict and deterministic ordering proof
  prepared_at and expires_at
```

The object records exactly what the human reviewed and is installed as a
non-standing object by the same authority transaction that applies its
ordinary Decision Events. It is not a prerequisite canonical write. Partial
selection, revision, changed reason, changed evidence, or changed action
derives a new batch root; it never mutates the old packet.

The v1 batch planner handles only exact Accept or Reject dispositions for
sealed campaign candidates or already registered pending Proposals. Correction,
retraction, supersession, policy, authority, publication, budget, and
destructive actions remain individually actionable in the same Decision Inbox
but use their existing dedicated planners until separate atomic semantics and
conformance are earned.

One human-only Cedar action, `review_batch_commit`, authorizes commitment of
one batch. Acceptance of this amendment must add that action to the closed
structural human-only action list and Cedar schema and policy. Agent and
workload denial is structural before policy evaluation. The planner must also
evaluate every underlying `review_accept` or `review_reject` action for the
same principal and retain those deterministic evaluations. A batch is invalid
if it:

- spans Frontiers;
- mixes scientific review with policy, schema, authority, budget, publication,
  or destructive effects;
- repeats a Proposal;
- has overlapping or order-dependent writes without one unique topological
  order;
- omits a failed or blocking Verification;
- changes a reason or limitation after review; or
- is stale against any read-set input.

External publication and destructive effects may appear in the outbox, but
they are never co-committed with scientific state. Their side effects are not
atomically replayable Git state and therefore require their own explicit,
idempotent executor after authorization.

`review_batch_commit` is an internal transaction action. It does not revive a
generic public `review apply` command or erase the direct Accept, Reject,
Request revision, correct, retract, or supersede disposition reviewed for each
item. The visible commit control names the exact selection, such as `Accept 2
and reject 1`, rather than `Approve all`.

Immediately before confirmation, the planner rebases the complete candidate
set onto current Standing and reruns every required check. This adopts the
load-bearing property of
[GitHub's merge queue](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/merging-a-pull-request-with-a-merge-queue):
a once-valid change cannot enter shared state merely because its original
checks passed. If the current heads change during or after review, the affected
entry is deselected and must be reviewed again.

#### One authority transaction

`Commit reviewed decisions` performs one local OS authentication and one
repository-authority signature:

1. acquire the existing repository-authority write barrier;
2. rederive every retained item, policy evaluation, state diff, dependency,
   read-set input, and batch root;
3. simulate the complete ordered transition;
4. reject any stale, conflicting, ineligible, or partially verified item
   before authentication, signing, or journaling;
5. build the ordinary Submission, Registration, Claim, Proposal,
   Verification, per-Proposal Decision Event, and canonical object postimages;
6. execute the existing multi-event, multi-object authority transaction; and
7. replay the resulting history and Standing before publication.

One authority record covers every ordinary Event and object delta exactly once.
The batch adds no new Standing rule. A one-entry batch must be byte-for-byte
equivalent in scientific effect to the current single-Proposal path.

Cancellation, authentication failure, authorization failure, signer failure,
read-set drift, crash before commit, or any invalid item writes no canonical
byte. A committed crash recovers all-or-none from the existing journal without
another human approval or signature.

#### State model

The planes remain separate:

```text
lease       proposed -> active -> exhausted | expired | revoked
campaign    open -> sealed -> superseded by child
candidate   open -> sealed -> superseded | selected
run         starting -> running -> completed | failed | cancelled
outbox item pending -> saved | snoozed | revision_requested | dismissed | batched | stale
batch       prepared -> applied | rejected | expired | stale
Standing    unchanged until one authorized batch is applied
```

A branch blocked on one consequence-bearing item pauses at that boundary.
Independent experiments may continue inside the same active lease. A passing
verifier may change campaign evidence and outbox context; it never changes
Standing.

#### Migration and deletion

Do not rewrite existing Attempts, Runs, Submissions, Verification Records,
Decisions, Events, or authority records.

- Existing single-Proposal review remains the one-entry compatibility path.
- Existing Run v2 records remain immutable campaign-node inputs.
- Lease expiry or revocation blocks future work but preserves prior evidence.
- Accepted-state replay never requires the campaign bundle or review UI.
- Corrections and review revisions append new bundles, batches, Decisions, and
  Events.

Before implementing this amendment:

1. repeat the four-Frontier inventory for retained capability objects;
2. if none exist, delete only the generic unused capability issuance and
   writer surface instead of layering the lease beside it; retain the v1
   types, reader, and conformance required by `AuthorityRecordV1`;
3. retain only verification code required by an actual retained record;
4. remove per-step human approval and repository signing from execution
   receipts, progress, checkpoints, verifier runs, and draft creation;
5. remove copied roots, timestamps, fingerprints, saved answers, and approval
   session files from the campaign path; and
6. keep a prompt only for lease issuance or expansion and meaningful outbox
   decisions.

This amendment does not add a scheduler, hosted authority, second writer,
checkpoint database, work graph, public mutation API, or mandatory Canopus
dependency.

#### Conformance

The entry gate is a twelve-hour dogfood trace showing no human prompt between
lease issuance and a real consequence-bearing outbox item. A shorter run may
exercise conformance but cannot satisfy the product-compression gate.

Focused conformance must prove:

- exact Target, root, principal, operation, Artifact-type, writable-root,
  tool, network, budget, time, and consequence enforcement;
- parallel budget reservation, commit, release, crash replay, and exhaustion
  through one controller without overspend;
- expiry, exhaustion, and revocation stop future work without invalidating
  prior evidence;
- no agent or workload can issue, widen, renew, approve, accept, reject,
  publish, destroy, or change policy through the lease;
- runtime credential refresh inside an active lease needs no semantic approval
  and fails outside it;
- campaign entries detect truncation, reordering, substitution, hidden failed
  children, missing parents, cycles, changed diffs, and changed evaluations;
- pause, resume, and in-scope steering append without reauthorization, while
  scope-expanding plan changes fail into the Decision Inbox;
- every steering directive binds the exact campaign, lease, experiment node,
  active run, and prior activity root; only a correlated recognized outcome
  counts as delivery, while missing, unknown, mismatched, or
  completed-before-delivery acknowledgments remain visibly unapplied;
- producer- and verifier-authenticated candidate sealing creates no repository
  transaction, repository-authority signature, canonical Proposal, Event, or
  Standing;
- a crash before, during, or after candidate sealing and outbox projection
  leaves either no durable entry or exactly one entry under its full root;
  restart deterministically repairs the projection without duplicating,
  dropping, or silently changing an entry;
- one candidate commit installs the same canonical scientific effect as the
  current direct Submission, Verification, and single-Proposal review path;
- Submission and Verification remain accepted-state delta zero;
- the outbox excludes routine activity and includes every escalation class;
- outbox entry roots change on classifier, head, sealed-candidate, requested
  action, evidence, or semantic-diff drift, but not on unrelated continuing
  campaign activity;
- every selected review response maps an explicit entry ID to its full root;
  queue reorder cannot misapply it; unknown, duplicate, wrong-root, extra,
  omitted-selected, or positionally substituted responses fail;
- only a pending, unexpired entry acted on by its designated human authority
  may compare-and-swap into a batch; expiry, wrong authority domain, wrong
  principal, stale Inbox root, concurrent disposition, or lost race writes
  nothing;
- a selected subset commits while unselected entries remain pending;
- save, snooze, revision-request, dismissal, and notification preferences
  change no deterministic outbox entry or Standing, cannot suppress a
  mandatory unresolved obligation, and do not carry to a successor root;
- no persistent, wildcard, tool-wide, “always approve,” saved-answer, or
  classifier-exception path exists outside a newly reviewed execution lease;
- resumable campaign state fails closed on serializer, controller, runner,
  agent, tool, policy, model, configuration, interruption, lease, or
  activity-root drift and contains no credential or authority material;
- lease-root change, expiry, or revocation rejects serialized sticky approval;
- batch roots and ordering are deterministic;
- mixed Frontier or authority domains, duplicate Proposals, ambiguous order,
  and write conflicts fail closed;
- stale Proposal, Claim, Submission, Verification set, policy, keyset,
  authority head, binary, or read set fails before any provider prompt;
- current-Standing rebase and every required check pass immediately before
  review confirmation and again under the write barrier;
- `review_batch_commit`, `review_accept`, and `review_reject` remain
  structurally unavailable to agent and workload principals;
- cancellation and authentication, authorization, or signer failure write
  nothing;
- exact retry is idempotent and committed recovery installs all or none
  without reauthentication or resigning;
- restart after a steering acknowledgment, outbox enqueue, batch preparation,
  or committed batch reconstructs the same visible state and never converts
  an unacknowledged directive, triage action, verifier result, or partial
  journal into authority;
- batch Standing equals valid sequential application in the same canonical
  order while one authority record covers every Event exactly once; and
- a clean clone replays accepted state with the campaign system, workbench,
  runtime credentials, and network absent.

Implementation order is deliberately narrow:

1. prove one staged candidate can derive and atomically install the current
   Submission, Verification, Proposal, and single-Proposal Decision effect;
2. add candidate sealing, the root-bound outbox, and one-entry equivalence;
3. add atomic homogeneous multi-Proposal review and current-Standing rebase;
4. add the execution lease, budget controller, campaign bundle, and Cockpit;
   and
5. dogfood one real twelve-hour campaign before changing the daily product
   surface or accepting this amendment.

### 8. Keep exact intent binding; remove root ceremonies

Rename Decision Plan concepts internally:

```text
intent_digest
transaction_plan
approval_intent
expected_before_root
```

The intent binds the exact canonical machine inputs and the exact sanitized
semantic summary shown to the user. The client holds an opaque short-lived
intent handle. The user never copies:

- a hash;
- timestamp;
- key fingerprint;
- helper digest; or
- policy schema identifier.

Routine policy-covered work requires no prompt. An exceptional decision
requires one semantic action. Policy, membership, recovery, authority rotation,
and high-impact bulk changes require one semantic review plus step-up or quorum
when the active policy requires it.

Cancellation, stale state, authentication failure, authorization failure,
quorum failure, signer failure, or read-set drift writes no canonical byte.

### 9. Link events without a hash cycle

`vela.event.v1` replaces per-event authority signatures with:

```text
transaction_id
attribution principal and authority mode
exact event content
```

Vela derives the complete event set and resulting event-log root first. It then
builds and signs the authority record covering those event IDs and roots.
Events, retained objects, the authority record, and required policy/key
snapshots install atomically.

The one legacy migration bridge remains under `.vela/events/`. Every
post-migration `vela.event.v1` is stored separately under
`.vela/authority/events/`, with its covering DSSE envelope under
`.vela/authority/records/`. The separation is load-bearing: the legacy event
loader continues to accept only `StateEvent` bytes, while dual-history replay
reads the Era-1 event and record stores explicitly.

Strict replay requires every post-migration canonical mutation to be covered
exactly once by a valid authority record. Gaps, overlaps, duplicate coverage,
sequence reuse, wrong transaction IDs, wrong before/after roots, or authority
forks fail closed.

The record's execution write-set root commits to the semantic transition:
transaction ID, before and after authority-event-log roots, sorted event IDs,
and exact object deltas. The covering DSSE envelope is excluded from that
semantic root to avoid a self-referential hash cycle. The existing recoverable
repository transaction still commits to and verifies every exact event and
envelope postimage before installation.

### 10. Preserve two verification eras and one live writer

Verification recognizes:

```text
Era 0: legacy per-event signatures and AcceptancePolicy certificates
Era 1: transaction authority records and Cedar bundles
```

Only Era 1 has a live writer after a Frontier migrates. Era 0 remains a
read-only verification obligation. A post-migration legacy authority event is
a strict blocker; it is not silently interpreted under the old rules.

Offline replay needs no live IdP, passkey service, Git host, KMS, policy
service, Fulcio, or Rekor lookup. It validates retained authentication claims
as claims attested by the repository authority at transaction time. It does
not claim to reauthenticate the biological person offline.

### 11. Cross the boundary once without rewriting history

Each active Frontier receives one explicit continuity bridge:

```text
vela.authority-model-migration.v1
frontier_id
legacy event-log root
legacy actor-registry root
legacy active-policy head and store-manifest roots
new authority-keyset digest
new policy-bundle digest
new principal ID
new minimum writer version
migration reason
```

The current authorized human key signs this one migration event. Authority
record sequence 1, signed by the new repository authority, covers that event
and begins the new chain.

No historical event is re-signed or rewritten. If the legacy key is
unavailable, Vela creates no fictional continuity. The only fallback is an
explicit trust reset with a new out-of-band trust anchor and a read-only legacy
lineage.

Fresh Profile v1 repositories do not pretend to migrate. They use the closed
`vela.authority-initialization.v1` payload in one
`authority.initialized` `vela.event.v1`. The event is valid only over exactly
one unsigned structural `frontier.created` event, an empty actor registry, no
authority history, and the bound initial keyset and Cedar bundle. The selected
OpenSSH-agent key signs the covering sequence-1 authority record. This proves
repository-key possession but grants no scientific standing; consumers still
pin the resulting full authority root through an independent distribution
path. The pin is the minimal public local
`vela.authority-trust-anchor.v1 {frontier_id,
first_authority_record_root}` record, installed directly with
`vela authority trust pin`. It is separate from the ADR 0016
repository-boundary anchor, duplicates no keyset or policy fields, performs no
semantic ceremony, and grants no authority. The fresh path cannot run over
historical or established state.

## Migration and release plan

The implementation sequence is normative:

1. add dual-read schemas, validation, and authority-record verification;
2. translate AcceptancePolicy v0.1-v0.3 to Cedar;
3. shadow-evaluate every retained and hostile policy vector;
4. require Cedar to be identical or stricter and never introduce a new Permit;
5. implement the authority transaction edge and one standard SSH-agent
   provider;
6. migrate a disposable fixture Frontier;
7. migrate one low-risk active Frontier;
8. migrate Formal, Quantum, Sidon, and Erdős only after the earlier gates pass;
9. reject every live legacy writer after its Frontier marker changes;
10. delete `vela-signer`, identity-v2 custody, signer sessions, rebind flows,
    AcceptancePolicy authoring, and long-lived agent onboarding;
11. retain only the minimum Era-0 verifier; and
12. release Vela `v0.930.0` only after every active Frontier and clean-clone
    verifier passes.

ADRs 0011 through 0014 remain historical records of the problems they solved.
ADR 0020 supersedes their writer and product decisions only after the new
migration marker is active. Vela `0.915.1` remains the exact Era-0 replay
baseline.

## Strict and non-strict behavior

Strict mode blocks:

- an invalid or missing migration bridge;
- an unknown authority writer marker;
- a missing, malformed, forked, or non-contiguous authority record;
- an invalid DSSE envelope or authority signature;
- an authority key outside its sequence range;
- missing or duplicate event coverage;
- wrong before/after roots or transaction IDs;
- a missing, unvalidated, or altered Cedar bundle;
- policy or evaluation diagnostics on an automatic path;
- a stale or unauthorized principal, role, capability, or quorum;
- a post-migration legacy write; and
- any retained-object or canonical-history mutation.

Non-strict mode reports the same defects and grants no standing, Permit,
exemption, or fallback. Unknown authority is never interpreted as legacy
authority.

## Adversarial cases

Conformance must reject:

- an agent represented as a human through a changed type or string prefix;
- an agent approving or broadening its own capability;
- an expired, wrong-audience, wrong-Frontier, wrong-resource, replayed, or
  over-delegated capability;
- a changed policy, entity snapshot, schema, evaluator version, request,
  semantic summary, reviewer set, event, object delta, or state root;
- a Cedar `permit` attempting to shadow an unconditional `forbid`;
- a skipped Cedar evaluation error being mistaken for safe authorization;
- a stale role or recovered account used contrary to the active bundle;
- an authority record replayed at another sequence or Frontier;
- signature verification under an inactive, revoked, or substituted key;
- an uncovered, multiply covered, or reordered event;
- an authority-log fork or rollback;
- a Git merge, CODEOWNERS approval, Sigstore attestation, or verifier pass being
  treated as scientific acceptance;
- historical re-signing presented as continuity; and
- a live network dependency during clean-clone replay.

## Exact conformance contract

Focused implementation gates:

```bash
cargo test -p vela-protocol authority_record
cargo test -p vela-authority cedar_profile
cargo test -p vela-protocol legacy_policy_translation
cargo test -p vela-cli authority_transaction
cargo test -p vela-cli --test authority_initialization
cargo test -p vela-protocol --test authority_era_fixtures
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

Required vectors include:

- authority-record canonicalization, ID, DSSE domain separation, and signature;
- keyset rotation, revocation, sequence windows, fork, and rollback;
- event gap, overlap, duplicate coverage, wrong transaction, and wrong roots;
- schema and request validation;
- Cedar default deny, forbid precedence, and Vela deny-on-diagnostic behavior;
- exact v0.1, v0.2, and v0.3 policy translation;
- no-new-Permit shadow evaluation over every retained policy decision;
- agent/workload capability scope, expiry, audience, and delegation depth;
- semantic-summary substitution and transaction TOCTOU;
- cancellation and authentication/authorization/signer failures with zero
  writes;
- byte-identical Era-0 replay;
- one exact migration bridge and rejection of later legacy writes;
- one exact fresh initialization and rejection of non-genesis, non-empty,
  duplicate, or substituted inputs;
- clean-clone network-disabled replay of both eras; and
- unchanged accepted scientific state from authority migration alone.

The full deterministic release union runs once at the actual `v0.930.0`
boundary. External Lean, Diderot, and live-network suites remain excluded
unless a selected migration fixture directly requires them.

### Implementation evidence, 2026-07-24

The read-only candidate now implements the closed bridge payload and dual
history verifier. It preserves the ordinary Era-0 event-log commitment through
authority-record sequence 1. Subsequent mixed-history commitments use
`vela.authority-event-log.v1`, binding that frozen legacy root and the sorted
full roots of covered Era-1 events.

Focused fixtures prove:

- byte-identical legacy-only replay remains valid;
- the bridge requires an exact unrevoked actor-registry key and legacy event
  signature;
- sequence 1 covers only the bridge and binds the new keyset, policy bundle,
  principal, semantic approval, and event object root;
- later transactions have exact unique event coverage and attribution;
- extra legacy events, missing or duplicate coverage, transaction
  substitution, wrong roots, chain forks, registry drift, signature
  tampering, policy substitution, and unknown bridge fields fail closed; and
- the reducer treats the bridge as non-scientific.

This evidence does not accept the ADR, enable a live Era-1 writer, or migrate
a Frontier.

The CLI-unreachable sequence-1 candidate now implements the corresponding
writer boundary. It verifies the already legacy-signed bridge against the
exact held Era-0 history and initial manifests before runtime authentication
or repository signing. It then prepares one recoverable transaction covering
the bridge, initial full-root keyset, initial full-root policy bundle, and
sequence-1 DSSE record. Cancellation, signer refusal, legacy-signature drift,
runtime-policy byte substitution, and marker-time membership drift install no
canonical postimage. A committed partial install recovers and exact retry
requires neither another authentication nor another signature. The
application-level machine-authority forbid list explicitly includes
`authority_model_migrate`.

This completes only the sequence-1 installation sub-gate. The read-side
rotation law and its CLI-unreachable writer are now implemented and
adversarially tested. Emergency close, the full disposable Frontier drill,
active migration, CLI exposure, and ADR acceptance remain open.

Phase 1 now also has a committed
`vela.authority-history-conformance.v1` vector at
`sha256:5a609f00f97f9bda79ffceb77f34edfdc4b1ad3c1252f28b844b45b0d1f23806`.
Sequence 1 binds the full canonical signed migration-event root as its semantic
intent and contains exactly three initial object deltas: the bridge event, the
initial authority keyset snapshot, and the initial restricted policy snapshot.
The fixture was deliberately regenerated before any Era-1 writer release to
place post-migration events under `.vela/authority/events/` rather than the
legacy `StateEvent` directory.
An independent Python path rederives both event forms, legacy and mixed roots,
the actor-registry and policy inputs, the legacy Ed25519 signature, both DSSE
authority-record signatures, sequence and threshold rules, clean pinned Cedar
authorization, transaction coverage, attribution, and object deltas. Six
historical/DSSE hostile vectors and four authentication-observation hostile
vectors fail closed. The same vector passes from the exact detached Git commit
in a clean clone with network access denied. Phase 1 is therefore complete.
This advances the candidate only to the runtime authentication and
authority-transaction gates; it does not authorize a writer or substitute for
the deterministic release union at an actual release boundary.

The vector was deliberately regenerated before any writer release when the
provisional arbitrary authentication strings were replaced by the closed
`vela.authentication-observation.v1` contract. The observation binds exact
principal class and issuer-subject, closed method and assurance, a full
non-secret session root, authentication/observation/expiry times,
presence/verification facts, recovery context, and revocation reference.
Runtime cookies, bearer tokens, assertions, and raw session identifiers remain
outside canonical history.

Phase 3 now has a committed `vela.principal-capability-conformance.v1` vector
at
`sha256:67bf660a0733bbc7579a883e8cc2e1b9ae09843e6ecee856794e2c07f1f5ef2d`.
The Rust and independent Python paths rederive the exact human
issuer-subject identity, agent grant, attenuated workload child, and
bearer-free verified claim. Eight hostile cases reject email-based identity
inference, human capability subjects, human-only actions, lifetimes over 24
hours, delegation broadening, parent substitution, bearer-token retention,
and revocation. The application-level human-action forbid now consumes the
same protocol-owned list used by the capability contract.

The candidate now also implements a source-level runtime preflight behind an
injectable authentication adapter. It validates exact principal, observation
time, expiry, and a passed live revocation set; derives the reserved Cedar
authentication context; and exposes no filesystem or signer capability.
Caller principal or reserved-context substitution fails before adapter
invocation. Seven focused tests prove bearer-free local observation,
cancellation, identity/expiry/revocation failure, recovery-visible policy,
derived context, fail-closed Cedar behavior, and zero-write sentinels.

This evidence does not implement an OS or identity-provider session, issue a
runtime token, enable a live authority writer, accept this ADR, or migrate a
Frontier.

The disposable Phase 4 writer core now composes the verified dual-history
snapshot, runtime authentication preflight, Cedar result, semantic approval,
event construction, authority-record signing, DSSE verification, and the
existing recoverable frontier transaction. It has no CLI route or production
signer provider. Six focused tests prove one exact offline-replayable Era-1
transaction, transaction identity binding to read-set and binary-pin changes,
zero journal or canonical bytes after authentication cancellation or signer
failure, history and policy substitution rejection before signing, stale-read
rejection before the commit marker, and committed partial-install recovery
without reauthentication or resigning. The test signer uses only a
deterministic fixture key.

This advances Phase 4 to the provider and broader object-delta gates. It does
not authorize a live writer, accept the ADR, migrate a Frontier, or access any
human signing key.

The next disposable slice now qualifies the first repository-authority
provider against a real ephemeral OpenSSH agent. The provider implements only
the standard request-identities and sign-request messages, selects exactly one
plain Ed25519 identity matching the full keyset public key, signs the exact
DSSE pre-authentication encoding, and verifies the returned signature locally.
It rejects certificates, security-key identities, other algorithms, missing
or duplicate matching identities, malformed responses, wrong payload types,
and key substitution. It reads no private-key file and adds no new
cryptographic dependency graph; the three provider tests use only an
ephemeral fixture key loaded into a disposable `ssh-agent`.

The writer now also binds the complete on-disk authority history rather than
trusting a caller-supplied verified snapshot. Exact direct-directory manifests
cover legacy events, Era-1 events, and authority envelopes, while individual
bindings cover every expected canonical byte and the legacy actor registry.
Missing, extra, altered, mode-drifted, or symlinked history fails before
authentication or signing, and marker-time directory changes abort before any
canonical postimage. This closes the stale-snapshot sibling-fork path.

Eleven writer tests now additionally cover create, update, and delete deltas
across authority, public-review, and canonical-evidence classes; automatic
object read-set binding; no-op and non-canonical object refusal; post-signing
object drift; broader partial-install recovery; missing history; extra history;
and stale-history fork refusal. Authority-record validation also rejects any
object delta whose two roots are equal.

This evidence completes the provider and broader object-delta sub-gates. The
following installation and rotation slices close canonical keyset and
policy-snapshot installation, exact retry, and non-cyclic rotation. Phase 4
remains open for a CLI-unreachable disposable Frontier exercise; the later
terminal-close slice closes emergency revocation. It does not authorize a live
writer, accept this ADR, migrate an active Frontier, or access any human
signing key.

The following bounded writer slice now derives content-addressed keyset and
policy-manifest paths from the verified history. Missing snapshots are covered
by the same authority record and recoverable journal as the events and other
objects; an existing snapshot must match its canonical bytes. Exact
direct-directory bindings reject store membership changes before the marker.
The runtime Cedar schema, policies, and entities must independently rederive
the roots retained by the policy bundle, closing a policy-byte substitution
gap.

Completed authority operations also retain the full typed result in the
verified journal. An exact retry supplies that complete result and receives it
back only after the marker, blobs, postimages, and event commitment verify.
This path has no authentication adapter or signer argument. A changed
transaction ID, record root, event set, read-set root, or write-set root is not
a retry.

Thirteen writer tests now cover these additions. The later sequence-1 and
rotation slices close initial installation and rotation. Phase 4 remains open
for the full offline Frontier drill; the later terminal-close slice closes
emergency revocation. No live writer, CLI route, active Frontier, or human key
is involved.

The next read-side slice defines rotation without a hash cycle. A rotation
record is verified under the currently active keyset and policy, covers one
new immutable full-root keyset and/or policy snapshot, and carries the exact
`authority_rotate` and/or `policy_rotate` semantic approval. The next keyset
must advance generation by one, link the exact current keyset root, and bind
the authority-record root immediately preceding the rotation transaction.
The next policy bundle must link the exact current policy root. Both become
active only on the following record.

The verifier indexes the retained stores by full root and rejects duplicate
roots, wrong Frontiers, unactivated retained generations, snapshot path/root
substitution, missing rotation approval, skipped generation, wrong prior
chain head, policy substitution, and use of the old key after activation.
Keysets also reject duplicate public-key material under different key IDs,
preventing one Ed25519 key from satisfying a multi-key threshold through
aliases. A four-record protocol test proves old-authority rotation followed by
a new-authority transaction.

The CLI-unreachable writer now installs one exact keyset or policy transition
through the same recoverable transaction barrier. It refuses a combined
keyset-and-policy transition so every write has one semantic action. Wrong
transition links, missing rotation approval, and an active-snapshot/history
mismatch fail before authentication or signing. The completed candidate is
replayed against the retained stores before any journal is prepared. Focused
writer tests prove exact full-root installation, policy rotation, zero-sign
refusal, offline replay, and an ordinary post-rotation decision signed by the
new key. This closes the rotation writer sub-gate only.

The following emergency-close slice adds no recovery authority. A terminal
successor keyset is represented by the optional `closed: true` field, zero
threshold, and no keys. Open and historical v1 keysets omit the field, so
their bytes and roots remain unchanged; older binaries fail closed on the new
terminal object. The covering record is signed under the current authority,
requires `authority_close`, and covers exactly one `authority.closed` event
and the terminal snapshot. The event's closed payload binds the exact last
trusted record, keyset, policy, incident, and reason. Replay rejects any later
record.

The writer validates the close shape before authentication or signing, replays
the terminal candidate before journaling, and installs it through the existing
recoverable barrier. A focused fixture proves missing approval has zero signer
access, exact terminal installation, retained offline replay, unchanged
historical roots, and fail-closed attempted continuation. Agents and workloads
are structurally forbidden from `authority_close`.

This closes emergency revocation only. It deliberately does not invent a
break-glass signer or let a compromised or lost key silently manufacture
continuity. A future lineage after total authority loss is a trust reset
requiring a new out-of-band anchor, not recovery of this history.

The final CLI-unreachable Phase 4 drill composes all writer paths in one
disposable Frontier:

1. install the exact legacy-signed bridge and sequence-1 record;
2. append one ordinary Era-1 decision;
3. rotate to generation 2 under the old repository key;
4. append one ordinary decision under the new key;
5. append the terminal close under the new key;
6. commit only canonical authority bytes to a local Git repository;
7. clone with local object reuse disabled; and
8. replay all retained events, snapshots, and five records from that clean
   clone without any signer or authentication provider.

The final replay reports four Era-1 events, the terminal keyset, unchanged
policy, and closed authority. This closes the disposable Frontier writer gate.
It does not accept this ADR, migrate an active Frontier, or remove Era-0
replay.

The subsequent `0.930.0-rc.2` source slice exposes only sequence 1 through
`vela authority migrate`. The preview is key-free and write-free. Apply
rederives the exact plan under the recovery barrier, asks the closed helper to
sign only the legacy continuity event after fresh user presence, authenticates
one exact local issuer-subject principal, and asks a matching Ed25519 identity
in the standard OpenSSH agent to sign the covering repository record.

The first composed Git fixture found two defects that isolated canonical
fixtures did not:

- historical event files may have valid non-canonical JSON formatting, so the
  migration now compares typed content while binding and preserving exact
  existing file bytes; and
- the initial human Cedar rule must name the one migrated principal and type
  review actions against proposals rather than the Frontier.

The corrected fixture proves stable plan derivation, substitution sensitivity,
dirty and pre-existing-authority refusal, cancellation before custody access,
one exact protected signature, recoverable installation, and clean-clone
sequence-1 replay. No active Frontier or human credential was used. ADR 0020
remains Proposed.

The next safe deletion slice removes the obsolete Claude plugin authority
workflow before the final active migration. `/vela:sign-prep`, saved
`.vela/sign-session.json` answers, binary-pin preflight, and every plugin
invocation of legacy batch `vela sign` are gone. The surviving plugin has only
producer commands plus read-only `review list`, `show`, and `preview`.
`conformance/check-retired-surface.sh` prevents the retired workflow from
returning. A real Erdős session-hook smoke reads the current 2,770 findings,
reproduced replay, strict-blocked state, and 15 pending proposals without
entering a signing path. No protocol, Frontier, proposal, Receipt, event,
policy, or scientific-state byte changes. The helper and protected identity
remain solely for the final sequence-1 continuity signature and are still
deleted only after Erdős migrates.

The same slice removes stale batch-signing instructions from active Rust
product surfaces. Integrity repair, artifact retirement, MCP refusal,
policy-lane, and scientific-diff output now identify the exact protected
`vela review decide` path. A focused prelaunch regression prevents those
surfaces from routing users back to batch `vela sign`, while historical
parsers and immutable replay fixtures remain intact.

The four active migration ceremonies then completed without rewriting their
pre-boundary histories or scientific roots. A post-migration Erdős writer
audit found a critical product gap before final deletion: the key-free legacy
`review decide` preview still advertised a second-phase legacy signature even
though `authority.model_migrated` had made that writer invalid. The same
generation of producer, administrator, actor-registry, and first-boundary
writers would have reached the repository write gate only when mutation began.

The repaired boundary now rejects every legacy canonical write intent as soon
as the migration marker is present. Decision and historical `sign` previews
run the same read-only era preflight, so they fail before a lock, journal,
helper, authentication prompt, or key read. The production write gate repeats
the check under its normal barrier. Focused fixtures cover all four legacy
intents plus the preview path, and the real migrated Erdős checkout rejects
both `review decide` and `work` with an unchanged Git tree and operation
journal digest.

The next implementation slice makes routine Era-1 work usable without
reopening personal signing. Exact Cedar source bytes are retained by content
root under:

```text
.vela/authority/policy-material/schema/<digest>.cedarschema
.vela/authority/policy-material/policies/<digest>.cedar
.vela/authority/policy-material/entities/<digest>.json
```

The first ordinary Era-1 transaction backfills the sequence-1 material while
retaining every historical bundle. Missing, altered, partial, symlinked, or
unactivated source fails closed. Sequence-1 bundles emitted before source
retention remain reconstructible byte-for-byte.

`vela work` now enters the repository-authority writer. An agent proves its
exact identity with the ordinary signed lease event it already creates; the
record retains only a five-minute, bearer-free
`agent_event_signature` observation. The active Cedar bundle must explicitly
authorize `work_claim` for an exact Frontier and that authentication method.
The lease event is stored in the Era-1 event log and covered by a DSSE
authority record; no post-migration legacy event is appended.

`vela land` uses the same writer without pretending that verifier success is
scientific authority. Vela builds the existing signed activity record, verifies that its
actor and public key match the Receipt identity binding and active lease, and
retains a five-minute, bearer-free `agent_record_signature` observation.
The Cedar action is `receipt_land`. The authority record covers the exact
pending proposal, Receipt, activity record, review material, and retained
artifacts as an object-only transaction. It appends no scientific event and
leaves the accepted event root unchanged. The proposal therefore remains
pending until a separate authorized scientific decision.

The object-only transaction path is covered independently of the landing
workflow. A no-op caller intent fails before authentication, repository
signing, or journaling even when retained policy material needs backfill. A
committed partial install recovers from its journal without reauthentication
or another signature. Offline history verification proves the authority
sequence advances while the event root does not, and rejects a re-signed
record whose after-event root was changed.

For migrated Frontiers whose sequence-1 bundle predated routine work, the
historical `authority enable-work` ceremony installed one root-bound successor
bundle. That one-time writer is now retired. Its retained policy grants claim,
refresh, release, and Receipt-bound pending submission only. Agents still
cannot obtain review, scientific acceptance, policy administration,
membership, recovery, or key rotation.

The real Erdős Frontier exercised the routine-work activation and coordination
path on 2026-07-26. One protected semantic approval produced policy record
`var_5d26ad19af679006` and activated bundle
`sha256:298e66a4c72b9504f12794eed63fa6f5f9e783c3abde2760d6bd8da494eca521`
at Git commit `9f7c7540e76404985ddb19ebcfbeb5589e7e7b8a`. The retained Cedar source
permits only exact `work_claim` with `agent_event_signature` and exact
`receipt_land` with `agent_record_signature`.

Without another human prompt, existing producer `agent:canopus-local` claimed
and released the first available ranked target, `erdos:124`, through authority
records `var_336bfaa0c37b35d7` and `var_6846d617407a052c`. The qualification is
published at commit `678f8436edf5db7656093fd8e091ae59c34b53a4`.
A fresh clone replayed all seven authority records. The legacy event root
remained
`sha256:d35b11555988458d28a971b0c882c6f42c27e0d4ca47df3080bc9872d51c7096`,
the scientific root remained
`sha256:540d4967071425f77c693e61f62053208b07d67667490dcb9eeef62ec3f1d316`,
the private work session was removed, and all pre-existing strict debt
remained visible.

The exceptional-decision slice replaces migrated rejection's dead legacy-key
route with one exact provider-authenticated repository transaction.
The Decision Plan binds the proposal ID and full root, Decision Brief and typed
binding roots, action, reason, principal, authority head, policy root, and
binary identity. The exact command is the semantic human action. The local
operating-system session authenticates the principal, restricted Cedar
authorizes `review_reject`, and the standard OpenSSH-agent repository authority
signs the covering record. There is no helper, Vela human key, copied root, or
copied timestamp. The transaction installs one
`review.rejected` Era-1 event and the matching proposal postimage while using
null before/after scientific roots. Dual-log proposal parity and terminal
review projection now consume already-verified repository-authority events.

Focused tests prove exact proposal-root binding, policy non-rotation,
principal and repository-key mismatch refusal, dual-log rejection parity,
unchanged legacy terminal decisions, and zero writes on preflight or provider
failure.

The next slice closes the acceptance hash cycle without changing
`vela.event.v1`. Every verified Era-1 event deterministically recovers the
ordinary unsigned `StateEvent` identity of its shared semantic fields.
Repository attribution and `transaction_id` remain covered by the stored
Era-1 event ID, while `review.accepted.payload.applied_event_id` names that
transaction-independent semantic identity. The authority record covers both
stored events, their full roots, the dual-log root, and exact proposal and
scientific-object postimages.

Focused protocol and transaction fixtures now prove:

- one repository-authority acceptance derives at least one scientific domain
  event plus exactly one explicit `review.accepted`;
- neither event is duplicated under `.vela/events/`;
- the exact materialized finding and proposal projection replays from the
  verified dual log;
- a missing or ambiguous applied semantic event fails proposal parity;
- a blocked Decision Brief fails before any provider prompt; and
- the recoverable authority writer covers both events and the proposal
  postimage in one DSSE transaction.

The retained Erdős vertical slice then exercised routine work again with
Vela `0.930.0-rc.12`. Proposal `vpr_d94b6b3bbe4c80ed` binds Receipt root
`sha256:816c7a1c3b355706eeb24aa30755b0d83d83489084dff64ce401ce80b0b26f5b`,
artifact root
`sha256:b369f29c3dfe777401375eeb47f682d20d89ca0387deaf600d791c01f98da9c0`,
and verifier root
`sha256:02a8e6504e78b3109cf02f5d1bf092d1242a666b19b21ec84c119414470ca536`.
It routed `Defer`, changed no accepted event, replayed from a clean clone, and
preserved all 1,592 pre-existing strict blockers.

The source candidate now deletes the custom `vela-signer` crate and binary,
identity-v2 custody, signer sessions, binary/helper pins, OS prompt adapters,
actor and boundary bootstrap writers, migration writers, and their packaging.
The release graph contains one product binary and six crates. Era-0 history
still replays through the protocol verifier, but no current command can mint a
new Era-0 signature or migration.

This completes the source contraction and implementation gate, not the live
exceptional-decision qualification. No active proposal has yet completed an
accept or reject transaction through the final local-OS-session plus OpenSSH
repository-authority path. ADR 0020 therefore remains Proposed until that
human-run qualification, clean-clone replay, and the remaining acceptance
gates pass.

### Fresh-authority evidence, 2026-07-27

The candidate now exposes one advanced standard-provider setup command:

```text
vela authority init <frontier> [--key <full-fingerprint>] --reason <text>
```

It selects exactly one plain Ed25519 identity from the normal OpenSSH agent,
binds the local OS principal, writes the fresh `authority.initialized` event,
initial keyset, current routine-work Cedar bundle and material, and sequence-1
DSSE record through the existing recovery journal. It reads no private-key
file and invokes no Vela signer helper or personal identity. A disposable
Profile v1 Frontier initializes and passes strict replay; repeated
initialization fails closed. Historical authority-transaction and migration
vectors remain unchanged.

## Alternatives rejected

### Preserve or improve the current helper

Rejected. Better dialogs, longer sessions, Secure Enclave support, or another
custody backend leave Vela responsible for personal identity, recovery,
cross-platform prompts, and a growing signer protocol.

### Put each personal key behind SSH agent, PKCS#11, or KMS

Rejected as the default model. It improves key custody but preserves the false
product model that every event is a personal signing act. Standard providers
belong behind the repository authority instead.

### Make GitHub the authority

Rejected. GitHub is a useful authentication, workload, review, and publication
adapter. Its accounts, rules, audit logs, and availability are not the portable
scientific authority root.

### Use WebAuthn assertions as event signatures

Rejected. WebAuthn strongly authenticates an RP-scoped challenge. It does not
provide Vela's general portable event-signature semantics or prove scientific
comprehension.

### Use Sigstore or in-toto as scientific standing

Rejected. They are appropriate for build and execution provenance. Provenance
does not decide scientific standing.

### Build AcceptancePolicy v0.4

Rejected. Cedar supplies a bounded, schema-validated, default-deny,
forbid-overrides-permit authorization language with an embedded Rust
implementation. Vela retains only its consequence routing and quorum state
machine.

### Delete portable signatures

Rejected. A compromised Git host or hosted database must not be the only
historical authority. One repository-authority signature per exact transaction
is the smallest justified cryptographic core.

## Acceptance gates

ADR 0020 may become Accepted only when:

- every current and hostile AcceptancePolicy decision is identical or stricter
  under the pinned Cedar bundle;
- no translated case introduces a new automatic Permit;
- one disposable and one active Frontier use the new writer;
- a clean fresh Frontier reaches routine `start -> submit -> verification`
  through `authority init` without a migration or personal-key ceremony;
- an independent reader validates the closed authority trust-anchor schema,
  rejects a substituted sequence-1 root, and selects the exact first record
  without trusting repository-controlled bytes;
- clean-clone replay validates both eras without network access;
- every post-migration event has unique valid authority-record coverage;
- routine agent work produces zero prompts;
- one exceptional decision requires one semantic action and no key, hash,
  timestamp, fingerprint, or helper interaction;
- account recovery and authority-key rotation preserve historical verification;
- agent self-expansion, stale approval, token replay, policy broadening, and
  authority-fork fixtures fail closed;
- all active Frontiers migrate without rewriting pre-boundary history;
- `vela-signer`, live legacy writers, identity-v2 custody, signer sessions,
  rebind ceremonies, and AcceptancePolicy authoring are deleted; and
- a blinded user can distinguish evidence, verification, authorization,
  acceptance, publication, and correction.
