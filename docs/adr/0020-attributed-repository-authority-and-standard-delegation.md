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

### 7A. Proposed amendment: Agent Campaigns and a Decision Inbox

Status: **Proposed, design-only, 2026-07-30. Nothing in this amendment is
implemented or accepted.**

This amendment responds to a concrete dogfood failure: a multi-hour or
multi-day agent run cannot stop for human verification or
repository-authority signing every few minutes. Repeated prompts do not improve
judgment. They train the user to approve mechanically and hide the small number
of consequences that require real authority.

The product rule is:

> Authorize bounded execution once. Append evidence continuously. Interrupt
> only when consequence changes. Commit reviewed Decisions through the existing
> exact authority transaction.

The researcher flow is:

```text
choose Frontier and Target
  -> authorize one bounded local campaign
  -> agents run and append authenticated evidence without authority prompts
  -> inspect the few pending Proposals that require judgment
  -> commit one exact set of reviewed Decisions
  -> replay Standing from retained canonical bytes
```

The invariant does not change:

- evidence is not a verdict;
- Verification is not acceptance;
- an agent cannot accept, reject, or broaden its own authority;
- accepted transitions replay from retained bytes; and
- corrections append instead of replacing history.

#### Existing seam and the actual defect

Most of the required substrate already exists:

- `vela.attempt.v3` privately binds one Target, actor, task contract, exact
  repository read set, and expiry while creating no Event, authority record, or
  Standing;
- `canopus.activity.v0` and `canopus.run.v2` retain non-authoritative activity,
  artifacts, verifier results, failures, and clean-clone reproduction;
- Submission and Verification Record signatures authenticate their exact
  producer or verifier bytes without becoming scientific authority;
- the Frontier transaction journal already provides serialization,
  postcondition verification, crash recovery, and idempotent publication;
- `AuthorityTransactionRequest` already accepts multiple Event and object
  drafts, one exact read set, multiple semantic approvals, and one recoverable
  commit; and
- the current Decision path already rederives Proposal, Claim, Submission,
  Verification set, policy, authority head, binary, reason, and action under
  the write barrier before authentication or signing.

The missing seam is not another candidate or approval protocol. The current
Submission and Verification writers route routine, non-standing evidence
through a repository-authority transaction. That makes the repository signer
approve evidence retention even though producer or verifier authentication
already identifies the author and accepted-state delta remains zero. This is
the source of the repeated ceremony.

The smallest repair has four parts:

1. extend the private Attempt into a controller-owned campaign authorization;
2. add one narrow non-standing evidence transaction;
3. derive a private Decision Inbox from real pending Proposals; and
4. plan several ordinary Decisions for one existing authority transaction.

No new candidate, review-outbox, review-selection, review-batch, campaign,
resume, or canonical lease schema is justified by the current evidence.

#### Private campaign authorization

The campaign authorization is private controller state, not a scientific
object, repository-authority object, Event, capability token, or source of
Standing. It evolves the existing Attempt boundary rather than layering a
second delegation system beside it.

Its exact rooted state binds:

```text
campaign ID and predecessor, if any
Frontier and exact Target IDs and roots
starting repository and packet roots
agent or workload principal
controller and runner build identities
allowed operations and Artifact classes
readable and writable roots
tool, network, sandbox, and publication constraints
wall-clock, model-call, token, compute, spend, storage, and parallelism budgets
issued_at, not_before, expires_at, and revocation state
consequence ceiling: evidence_only | pending_review
```

One explicit local start action creates it after the user inspects the scope.
The local OS session may authenticate that action, but neither a human
scientific key nor the repository-authority key is read. The authorization is
stored outside worker-writable roots. The worker receives only an opaque
campaign identity and the restricted runtime capabilities that the controller
can enforce.

The authorization permits:

- inspect exact source and Frontier state;
- create isolated baseline and child experiment worktrees;
- run allowed tools and verifiers;
- append receipts, failures, Artifacts, Submissions, Proposals, and
  Verification Records through the evidence transaction;
- pause, resume, and accept steering inside the exact scope; and
- request a broader consequence through the Decision Inbox.

It never permits:

- accept, reject, correct, retract, or supersede scientific Standing;
- change policy, schema, membership, quorum, or repository authority;
- publish outside the approved boundary;
- perform a destructive or high-impact action;
- increase budget, tools, network, writable roots, or risk;
- access a human or repository-authority key; or
- create, renew, widen, revoke, or approve its own authorization.

The controller is the sole allocator for enforceable campaign budgets.
Parallel branches reserve, commit, or release allocations through one
hash-linked local ledger and the existing local work lock. A restarted
controller replays the ledger before allocating again. A dimension the
controller cannot enforce must be labelled observed rather than enforced.

Runtime credential refresh inside the unchanged active scope requires no human
prompt. It fails after expiry, exhaustion, or revocation. Widening any bound
input creates a new explicit authorization request and pauses only the affected
branch. Completed evidence remains valid after the campaign expires or is
revoked.

Native runners retain their own durable execution. The controller is a thin
authorization, metering, and evidence adapter around Codex, Claude Code,
Canopus, OpenResearch, or another runner. Vela does not acquire a scheduler,
checkpoint database, universal work graph, tool trace, or model runtime.

#### Evidence transactions

Add one internal `EvidenceTransaction` path that reuses the Frontier
transaction journal and write barrier without invoking repository authority.
It appends ordinary canonical evidence under producer or verifier
authentication. It is not an Event, authority record, Decision, or alternative
source of Standing.

Every evidence transaction binds:

```text
actor and authentication class
active campaign-authorization root, when used by a campaign
exact object drafts and closed write classes
complete repository read set
repository before root
binary identity and recorded time
deterministic transaction root
```

The write surface is a closed allowlist:

- producer-authenticated Artifact, Submission, Registration Record, Claim, and
  pending Proposal objects and their exact repository references;
- verifier-authenticated Verification Records and their exact repository
  references; and
- deterministic, non-authoritative derived indexes covered by the journal.

An evidence transaction may create or extend `pending_review`. It may not:

- append a scientific or authority Event;
- change an accepted Claim or scientific Standing;
- write a Decision, policy, schema, membership, quorum, keyset, authority
  record, authority head, or repository trust anchor;
- delete or rewrite retained canonical evidence;
- write an unknown object kind, path, or class; or
- infer acceptance from a producer result or verifier pass.

The producer or verifier signature, registered actor, exact subject roots,
campaign scope when present, repository read set, and postimage are verified
before the journal is prepared. The transaction rechecks its repository root
under the barrier. Crash recovery installs all postimages or none, and exact
retry returns the retained result without another key read.

This keeps evidence canonical as it is produced. It avoids a parallel
draft-candidate lifecycle and avoids combining initial registration with a
later scientific Decision. A passing Verification may strengthen a pending
Proposal; it never changes Standing.

#### Campaign Cockpit

The Campaign Cockpit is a local projection over native runner state, Attempt
or campaign authorization, retained Runs, Artifacts, Submissions,
Verifications, and failures. It is not a canonical graph.

[OpenResearch](https://openresearch.sh/docs/experiment-flow) provides the
useful execution shape: one baseline, isolated child experiments, observable
run state, evaluation output, and exact diffs. Vela adopts that shape for
inspection but not OpenResearch's scheduler, storage, or authority.

The Cockpit shows:

- Frontier, Target, authorized scope, expiry, and remaining budgets;
- a baseline and child-experiment tree;
- selected-run diff, evaluation, Artifacts, verifier scope, and failures;
- coalesced `verb -> object -> outcome` activity with exact retained roots;
- pause, resume, graceful stop after the current atomic operation, emergency
  revoke, and in-scope steering; and
- separate `Needs steering` and `Pending scientific decisions` counts.

Its private states are:

```text
queued | running | needs_input | paused | exhausted | expired
revoked | stopped | failed | complete
```

`needs_input` is limited to execution steering that remains inside the
authorized scope. A Proposal-based scientific Decision appears in the Decision
Inbox instead. A budget, tool, network, writable-root, publication, destructive
action, or risk escalation remains a native controller interruption beside the
affected branch unless an existing typed authority planner already derives
complete exact input for it. Stop, expiry, or revocation preserves every
completed receipt, Artifact, failure, and verifier result.

A steering directive binds the campaign, experiment, active run, prior
activity root, issuer, time, and exact instruction. A correlated
acknowledgement records `applied_in_run`, `started_successor_run`, `rejected`,
or `completed_before_delivery`; only the first two count as delivery. Steering
cannot widen campaign scope or change Standing.

The product notifies once for a new scientific consequence, once when a blocked
Proposal becomes decision-ready or its exact consequence materially changes,
and once near campaign expiry. Notifications coalesce per campaign. Routine
receipts, progress, verifier completion without a readiness transition,
credential refresh, and checkpoints remain visible but never create prompts.

Durable interruption belongs to the runner. The useful pattern from
[LangGraph interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts)
is to persist state before interruption, resume by stable identity, and make
pre-interrupt side effects idempotent. Vela does not serialize private prompts,
credentials, human keys, repository-authority material, or sticky approval into
a new protocol object.

#### Derived Decision Inbox

The Decision Inbox is a private, deterministic projection of real pending
Proposals, their Submissions and Verification Records, current Standing,
policy, keyset, and authority heads. It has no retained outbox schema and no
independent lifecycle. V1 contains Proposal-based scientific Decisions only.
An existing typed policy or authority planner may later expose an exact rooted
input in the same inspection surface, but the Inbox never invents a generic
high-impact-action record to make unlike actions look batchable.

“Outbox” and “Inbox” name two perspectives on the same consequence
projection. The agent or Campaign Cockpit may say that one consequence is
waiting in its outbox. The reviewer sees that exact rooted entry in the
Decision Inbox. There is no transport queue, copied review object, or second
status model between them.

The v1 projection contains only meaningful scientific consequences:

1. accept or reject a pending Claim;
2. accept or reject a correction, retraction, or supersession Proposal; or
3. resolve a Proposal that identifies a contradiction with accepted state.

Policy, schema, membership, quorum, repository authority, external
publication, destructive action, and execution-scope escalation retain their
existing dedicated planner or controller surface. They do not become Inbox
rows merely because they require a human.

Routine tool calls, progress, receipts, Artifact freezes, verifier passes or
failures, and experiment branches never become Inbox rows. A malformed or
unreadable decision input fails closed and remains a repair obligation.

The Inbox groups by Frontier, Target, and Proposal lineage, not by agent
thread. Each row shows:

- requested action and exact target;
- one-line semantic change and why judgment is required;
- decisive evidence, verifier result and scope, caveats, and nonclaims;
- conflicts, missing checks, consequence class, age, and expiry; and
- the exact root that will become stale if any reviewed input changes.

Detail answers five questions: what changed, what evidence and limits support
it, what Decision is requested, what consequence follows, and what obligation
comes next. It shows the semantic diff and evidence chain, with exact roots one
disclosure away.

The interaction contract is deliberately smaller than a generic agent
approval dashboard:

```text
collection
  -> filter by Frontier, Proposal kind, and readiness
  -> inspect one rooted semantic diff and evidence chain
  -> explicitly Stage accept or Stage reject with an exact reason
  -> inspect the complete staged set and merged read set
  -> commit the named consequences once
```

The collection uses a deterministic order and makes urgency, expiry, blocked
checks, and stale inputs explicit. Search order, age, campaign ownership, and
agent identity never imply scientific priority or authority. A pending entry
shows how long it has waited, but campaign expiry stops future execution rather
than silently expiring the Proposal.

There is no default disposition, preselected Accept, or `Select all`. Opening a
row, pressing Enter, or invoking the primary command inspects it; none stages
or commits a Decision. This deliberately does not copy the conventional
primary-action behavior of a
[Raycast action panel](https://manual.raycast.com/action-panel). Staged rows say
`Staged — not applied`. The final action names the exact counts and targets it
will commit.

The Cockpit never embeds an Accept or Reject control in a run transcript. It
shows the blocked branch, requested consequence, current entry root, and one
link into the Inbox. Independent in-scope branches remain runnable. The Inbox
may show campaign and experiment lineage as context, but its primary identity
is the Proposal and exact scientific change.

After commitment, an entry leaves the Inbox. Its former deep link resolves or
redirects to the ordinary canonical Decision and Event transcript. Vela does
not retain a second “resolved card” solely to preserve UI history. If any
Proposal, Claim, Submission, Verification set, policy, keyset, authority head,
binary, or read set changes before commitment, the staged action and draft
reason are cleared. The open detail does not close and focus does not move: it
becomes a read-only stale comparison, a pre-existing polite status region
announces `Evidence changed; staged decision cleared`, and an explicit
`Inspect current root` action opens the successor input.

The first UI uses native table, list, and article structure with ordinary
controls and bounded pagination. Following the
[WAI listbox pattern](https://www.w3.org/WAI/ARIA/apg/patterns/listbox/), it does
not use `role=listbox` for rich rows containing links, buttons, selection
controls, and structured evidence. An ARIA grid is allowed only if its complete
keyboard contract is implemented.
The UI supports keyboard selection, filtering, inspection, cancellation, and
focus restoration; announces result counts, committed Decisions, readiness
changes, and drift failures; and pairs every status color with text and shape.
Mobile uses stacked articles and a modal detail surface that follows the
[WAI modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/):
visible close, inert background, logical initial focus, and focus restoration.
It never hides evidence behind hover, flattens structured evidence into
`aria-describedby`, or clips long Claims, reasons, or roots.

Filters, Proposal selection, and the exact entry root are URL-addressable.
Back and forward restore the same view, adapting the useful shareable-view
property of [Linear filters](https://linear.app/docs/filters) without making
saved views canonical state. A deep link to a stale root shows what changed and
links to the current root rather than silently swapping content.

The scientific dispositions are Accept and Reject, each with an attributed
reason. V1 omits Request revision, Save, and Snooze. Revisions travel through
the existing coordination channel and append a successor Proposal; dogfood
must demonstrate a real triage problem before local Save or Snooze state is
added. `Dismiss`, `Done`, `Ignore`, `Always approve`, wildcard approval,
remembered answers, and classifier exceptions do not exist.

Local staging is a sidecar keyed by reviewer, Proposal ID, and exact entry
root. It may contain only read state, draft reason, and staged disposition. It
does not affect the deterministic projection, deadline, authority, pending
Proposal, read set, or Standing, and it never carries to a successor root. A
changed entry root clears the draft and staged disposition for that entry.

This keeps the useful queue interactions from
[LangChain Agent Inbox](https://github.com/langchain-ai/agent-inbox/tree/081b2a30409304fa04bfcf7b01d035853b846ecd)
without adopting its thread-centric, editable, or ignorable interruption
semantics. LangChain's current frontend guidance usefully requires exact
action context, persistent interruption state, visible wait time, attributed
decision logs, and an explicit final submission for multiple pending actions.
Vela applies those interaction properties to rooted Proposals, not arbitrary
tool calls. It keeps durable, partially resolvable action state from the
[OpenAI Agents SDK human-in-the-loop flow](https://openai.github.io/openai-agents-js/guides/human-in-the-loop/)
without adopting persistent `alwaysApprove` policy or serializing secrets.
It treats GitHub's
[pending review](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/reviewing-proposed-changes-in-a-pull-request)
as useful prior art for collecting comments before one review, and its
[notification triage](https://docs.github.com/en/subscriptions-and-notifications/how-tos/viewing-and-triaging-notifications/triaging-a-single-notification)
as evidence that local notification triage is presentation state rather than
scientific disposition. Vela intentionally omits it from v1.

The public Observatory remains read-only and credential-free. It may display
committed Decisions and retained campaign evidence; unresolved Inbox and
staging state remain local to the reviewer.

#### Ephemeral batch planner

Batching is a planner composition over existing per-Proposal Decision
semantics, not a retained review object or new authority action.

The planner accepts a keyed selection:

```text
Inbox snapshot root
selected Proposal IDs and expected roots
for each Proposal:
  action: accept | reject
  exact reason
```

Selection may include only pending Proposals in one Frontier and one authority
domain. Every selected Proposal has exactly one keyed action and reason.
Unknown, duplicate, omitted-selected, extra, wrong-root, or positional
responses fail before a canonical plan is derived. Unselected Proposals remain
pending. The reviewer may commit any nonempty valid subset; partially staged or
unresolved entries remain in the Inbox without pausing unrelated execution.

For each selected Proposal, the planner reuses the existing single-Proposal
prepare and reducer logic. It rederives the Claim, Submission, complete
Verification set, current Standing, policy evaluation, proposed state delta,
and exact read set. It then:

1. acquires the existing repository-authority write barrier;
2. rederives every selected item and merges their read sets;
3. rejects duplicate Proposals, write conflicts, ambiguous order, stale
   inputs, ineligible actions, and failed deterministic checks;
4. simulates the complete canonical order;
5. requests one local OS authentication and repository-authority signature;
6. submits the ordinary per-Proposal Decision and scientific Event drafts in
   one existing multi-event, multi-object authority transaction; and
7. replays the resulting history and Standing before publication.

The visible action names the exact consequence, such as `Accept 2 and reject
1`; it never says `Approve all`. A changed input automatically deselects the
item. Correction, retraction, supersession, policy, authority, publication,
budget, and destructive actions retain their dedicated planners until their
atomic composition is separately justified.

The transaction's existing Authority Record, semantic approvals, Event IDs,
object delta, authorization context, and exact read/write roots are the
durable record of what was committed. An ephemeral batch-plan root may be
shown and confirmed, but no `vela.review-batch.v1` object is retained unless a
conformance test demonstrates an audit or replay gap that those existing bytes
cannot close.

A one-entry batch has the same scientific effect and retained ordinary
Decision bytes as the direct one-Proposal path. Cancellation, authentication
failure, authorization failure, signer failure, drift, or invalid selection
writes nothing. A committed crash recovers all or none from the existing
journal without reauthentication or resigning.

#### State model

The planes stay separate:

```text
campaign authorization  active -> exhausted | expired | revoked
runtime pause            false <-> true
run branch               queued -> running -> completed | failed | cancelled
Proposal                 pending_review -> accepted | rejected
Inbox projection         ready | blocked | stale
local staging            unstaged | staged_accept | staged_reject
ephemeral batch plan     prepared -> committing -> applied | stale | failed
Standing                 unchanged until an authorized Decision is applied
```

The derived entry, local staging, and ephemeral planner are separate state
machines. An applied Decision has no applied Inbox state: the row leaves the
collection and its deep link resolves to canonical history. Revision guidance
uses the ordinary coordination channel and waits for a successor Proposal
rather than mutating the current Proposal. An Inbox row blocked on evidence
pauses only the affected run branch; independent in-scope branches may
continue. A verifier result may change evidence and make an Inbox entry stale
or ready, but it never changes Standing.

#### Rejected design alternatives

The superseded design-only draft proposed canonical execution leases, retained
campaign and resume manifests, sealed unregistered candidates, retained
review-outbox and review-batch objects, and new Cedar actions for lease and
batch commitment. The source audit rejected that composition because it:

- preserved repository-authority signing on the routine evidence path;
- created a second candidate lifecycle beside ordinary pending Proposals;
- combined initial evidence registration with a later scientific Decision;
- duplicated native runner durability and the existing authority transaction;
  and
- introduced retained schemas before a replay or audit gap demonstrated need.

The useful evidence from that draft remains incorporated here: bounded
execution, visible failures, exact steering acknowledgements, consequence-only
interruptions, root-bound review, stale-item invalidation, one exact commit,
and clean-clone replay. Git history preserves the complete superseded design.

#### Migration and replay

This amendment does not rewrite existing Attempts, Runs, Submissions,
Verification Records, Proposals, Decisions, Events, authority records, or
Standing.

- Existing Attempt v3 records remain readable private authoring state.
- Existing Run v2 records remain immutable campaign inputs.
- Existing authority-signed Submission and Verification transactions replay
  unchanged.
- New evidence transactions write the same ordinary current object schemas;
  they add no accepted-state rule.
- Existing single-Proposal review remains the direct one-entry path.
- Campaign expiry or revocation blocks future work but preserves prior
  evidence.
- Accepted-state replay never requires the campaign controller, Cockpit,
  Inbox, local staging sidecar, runner, credentials, network, or batch planner.
- Corrections and review revisions append ordinary successor records.

Before implementation, inventory retained generic capability objects. If none
exist, remove only unused current writer surfaces rather than layering campaign
authorization beside them; preserve any reader and conformance fixture needed
for retained history.

This amendment adds no scheduler, hosted authority, second writer, checkpoint
database, work graph, public mutation API, mandatory Canopus dependency, or new
scientific Event kind.

#### Conformance and product gate

Focused conformance must prove:

- campaign authorization binds exact Frontier, Target, roots, actor,
  operations, Artifact classes, writable roots, tools, network, budget, time,
  and consequence ceiling;
- unenforceable budget dimensions are labelled observed, and parallel
  reservation, commit, release, crash replay, and exhaustion cannot overspend
  an enforced dimension;
- expiry, exhaustion, and revocation stop future work without invalidating
  completed evidence;
- no agent or workload can issue, widen, renew, revoke, approve, accept,
  reject, publish, destroy, or change policy through campaign authorization;
- runtime credential refresh inside unchanged scope needs no semantic prompt
  and fails outside it;
- pause, resume, and in-scope steering append without reauthorization, while
  scope expansion pauses the affected branch and creates a consequence item;
- steering acknowledgement is correlated to exact campaign, run, prior
  activity, and directive roots, and unknown or transport-only success is not
  reported as applied;
- an evidence transaction verifies actor, signature, campaign scope when
  present, exact subjects, repository root, complete read set, object delta,
  and postimages;
- the evidence allowlist rejects unknown object kinds, paths, classes,
  deletions, and rewrites;
- evidence transactions cannot change accepted Claims, Standing, Event log,
  policy, schema, membership, quorum, keyset, authority record, authority head,
  or trust anchor;
- Submission and Verification remain accepted-state delta zero, and verifier
  pass never renders or replays as acceptance;
- concurrent evidence and Decision writes serialize through the Frontier
  barrier, and stale inputs fail closed;
- crash before, during, or after evidence publication leaves no partial state,
  while exact retry is idempotent;
- the derived Inbox includes every Proposal-based scientific Decision and
  excludes routine activity, execution-scope escalation, publication,
  destructive action, and policy or authority work without an existing typed
  exact planner input;
- the Cockpit `Pending scientific decisions` count and Inbox collection derive
  the same exact entry roots, while the Cockpit exposes no Accept or Reject
  action;
- `Needs steering` remains distinct from `Pending scientific decisions`, and
  execution-scope escalation remains beside the blocked branch;
- campaign expiry stops future execution without expiring, deciding, or
  hiding a pending Proposal;
- changed Proposal, Claim, Submission, Verification set, policy, keyset,
  authority head, binary, or read set changes the derived entry and clears
  local staging and draft reason without closing detail or moving focus;
- root drift is announced through a polite status region and the stale deep
  link exposes the exact change plus an explicit current-root action;
- read state, draft reason, and staging change no Proposal, deterministic
  Inbox entry, deadline, authority input, or Standing;
- no Dismiss, Done, Ignore, wildcard, persistent, tool-wide, remembered-answer,
  classifier-exception, `always approve`, default disposition, preselected
  Accept, or Select-all path exists;
- opening a row, pressing Enter, or invoking the primary action only inspects;
  it never stages or commits;
- staged rows say `Staged — not applied`, and the final action names exact
  counts and targets;
- keyboard-only and mobile review can inspect every decisive field, stage and
  clear an action, cancel, recover focus, and reach exact roots without
  horizontal trapping or color-only state;
- rich review rows use native table, list, or article semantics rather than a
  listbox, and modal mobile detail provides a visible close, inert background,
  logical initial focus, and focus restoration;
- URL-backed filters, Proposal selection, and entry roots survive refresh and
  browser history navigation without silently replacing stale content;
- notifications fire for a new consequence and for the transition to
  decision-ready or a material exact-root change, but not for routine verifier
  activity;
- exact keyed selection prevents queue reorder, duplicate, omitted-selected,
  extra, wrong-root, or positional substitution;
- committing one valid selected subset leaves every unselected entry pending
  and independently actionable;
- mixed Frontier or authority domains, duplicate Proposals, write conflicts,
  ambiguous order, stale inputs, and ineligible actions fail before any OS
  prompt;
- every selected item independently passes the current one-Proposal prepare,
  policy, Verification, and reducer checks;
- the complete current-Standing simulation passes before confirmation and
  again under the write barrier;
- agents and workloads remain structurally unable to call the underlying
  Decision actions;
- cancellation and authentication, authorization, or signer failure write
  nothing;
- one-entry batch effect and retained Decision bytes equal the direct path;
- multi-entry Standing equals valid sequential application in the one
  deterministic order while one authority record covers every Event and
  object delta exactly once;
- committed crash recovery installs all or none without reauthentication or
  resigning;
- a resolved Inbox entry renders from the ordinary canonical Decision and
  Event transcript without a retained resolved-card or review-batch object;
  and
- a clean clone replays accepted state with campaign state, Cockpit, Inbox,
  local staging, runner, credentials, and network absent.

The product gate is one real twelve-hour dogfood trace showing:

- one bounded campaign authorization;
- no human or repository-authority prompt during ordinary evidence work;
- continuous retained receipts, failures, Artifacts, Submissions, and
  Verifications;
- only meaningful consequence items in the Decision Inbox; and
- one exact reviewed Decision commit or a user cancellation with zero
  scientific mutation.

Implementation order is deliberately narrow:

1. prove an evidence transaction can append one current Submission and one
   current Verification with accepted-state delta zero and no
   repository-authority key read;
2. prove old and new evidence paths replay to the same ordinary object and
   proposal semantics;
3. derive the Inbox from real pending Proposals and add local staging;
4. prove one-entry and homogeneous multi-Proposal batch equivalence using the
   existing authority transaction;
5. evolve Attempt into the private campaign authorization and add the
   Cockpit; and
6. complete the dogfood gate before accepting this amendment or changing the
   default daily workflow.

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
