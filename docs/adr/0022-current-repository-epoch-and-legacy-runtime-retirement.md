# ADR 0022: Current repository epoch and legacy runtime retirement

- Status: Superseded by accepted ADR 0027
- Target-surface disposition: any Vela Target Index or `next`/`start` language
  below is historical; commit `719cbc77` retired that core surface on 2026-08-10.
- Target release: Vela `v0.940.0`
- Protocol effect: one current-only repository epoch, Claim Record v1,
  Proposal v1, and a signed predecessor boundary
- Product effect: current repositories no longer carry or parse Era-0 events,
  Finding bundles, Receipt-era proposals, actor registries, or AcceptancePolicy
- Authority effect: repository authority signs one exact epoch transition;
  scientific judgment and standing do not change
- Compatibility: historical bytes remain in an immutable Git predecessor and
  source archive; the current binary verifies the boundary but does not parse
  the retired protocol
- Entry gate: satisfied; Quantum Codes, Formal Conjectures, Sidon, and Erdős
  replay as published current epochs with exact signed predecessor boundaries

> Historical decision. ADR 0027 compacted these repositories into the sole
> current `vela.repository-origin.v1` / `vela.repository.v3` boundary and
> removed this epoch reader from the active runtime.

## Context

Vela has completed the product-language transition to:

```text
Target -> Attempt -> Submission -> Registration Record
       -> Proposal -> Verification Record -> Decision -> Event -> Standing
```

The active repositories nevertheless still depend on three historical
representations:

1. accepted scientific state is stored as `vf_` Finding records in a
   `finding-bundle/v0.10.0` snapshot;
2. Proposal files use `vela.proposal.v0.1`, including terminal proposals whose
   producer package was a historical Receipt; and
3. repository-authority replay still requires the active repository to retain
   and parse the complete Era-0 event, actor, policy, and signing history.

Dual-read compatibility was correct while external users or unmigrated
repositories could exist. There are currently no external users, all four
active Frontiers are controlled, and immutable Git predecessors already
preserve the old bytes. Keeping both eras in every daily binary now enlarges
the trust base, CLI, tests, reader projection, and explanation burden without
adding current scientific capability.

Deleting old parsers in place is not safe. Old signatures authenticated old
bytes and old semantics. They cannot be reissued as Submission, Claim Record,
Proposal, or repository-authority signatures. The safe contraction is a
signed repository epoch transition that imports the exact standing state,
retains current authenticated objects, and binds an independently retrievable
predecessor.

## Decision

### 1. Start one current-only repository epoch

Every migrated Frontier has:

```text
frontier.yaml                         vela.frontier-profile.v2
.vela/repository.json                 vela.repository.v2
.vela/epoch.json                      vela.repository-epoch.v1
.vela/authority/events/               current authority events only
.vela/authority/records/              current authority records only
.vela/authority/keysets/              current keysets only
.vela/authority/policies/             current Cedar bundles only
records/claims/sha256/                 vela.claim-record.v1
records/submissions/sha256/            vela.submission.v1
records/registrations/sha256/          vela.registration-record.v1
records/verifications/sha256/          vela.verification-record.v1
records/proposals/sha256/              vela.proposal.v1
records/artifacts/sha256/              retained content-addressed evidence
targets.json                           vela.target-index.v3, when fresh
```

The active repository does not contain:

```text
.vela/events/
.vela/actors.json
.vela/policies/
.vela/findings/
.vela/proposals/
records/receipts/
legacy finding-bundle snapshots
legacy proposal or decision-evidence mirrors
```

The Git predecessor and source archive retain those bytes.

### 2. Bind the predecessor exactly

`vela.repository-epoch.v1` is canonical JSON with:

```text
frontier_id
epoch
predecessor_remote
predecessor_tag
predecessor_commit
predecessor_tree
predecessor_profile_schema
predecessor_event_log_root
predecessor_scientific_state_root
predecessor_compatibility_snapshot_root
predecessor_proposal_state_root
predecessor_actor_registry_root
predecessor_artifact_registry_root
predecessor_authority_head_root
predecessor_authority_event_log_root
predecessor_git_object_manifest_root
archive_bundle_sha256
imported_claim_set_root
retained_current_object_set_root
archived_object_index_root
equivalence_report_root
reason
```

The predecessor Git object manifest is a canonical, path-sorted list of every
tracked entry at the exact predecessor commit:

```text
path
git_mode
git_object_type
git_object_id
byte_length
sha256
```

The archive bundle is operational redundancy whose exact bytes are also bound
by `archive_bundle_sha256`. Protocol identity does not rely on a bundle parser:
the remote, tag, commit, tree, canonical roots, and object-manifest root are
sufficient to verify the predecessor, while the exact bundle is a convenient
offline carrier.

The transition uses the existing non-scientific `authority.initialized` event
and a fresh sequence-1 repository-authority record. No second epoch event kind
is added. The initialization payload binds the predecessor event-log and actor
roots plus the current keyset, Cedar bundle, principal, version floor, and
reason. The covering record's intent digest binds the exact upgrade plan, and
its object delta binds the epoch object, repository manifest, Profile v2, new
Claim set, retained current objects, retired Era-0 paths, keyset, Cedar
material, and exact transaction postimages. The authority event retains null
scientific before/after hashes: repository authentication does not itself
change scientific standing. The equivalence report separately proves that the
predecessor and imported accepted sets represent the same assertions,
conditions, provenance, evidence references, correction/supersession
relations, and standing.

The first authority-record root becomes the new independently distributed
trust anchor. The predecessor authority head remains evidence, not a live
writer.

Current replay starts at that sequence-1 record. The verifier takes the exact
predecessor event-log and actor-registry roots only from the canonical epoch
object, requires the initialization event to bind both roots, then verifies
every retained current authority event and contiguous DSSE record. Supplying
retained Era-0 bytes together with archived predecessor roots is invalid.
Wrong or partial roots, a second initialization, uncovered current events,
record gaps, forks, or unactivated keyset/policy snapshots fail closed.
Sequence 1 may retain an already content-addressed current keyset and policy
without pretending the epoch created those bytes; its signed event, authority
heads, and complete object delta still cover the exact epoch transition.

### 3. Migrate meaning, not signatures

For every accepted historical Finding, the migration creates one
`vela.claim-record.v1`:

```text
claim_id                         vcl_
version
assertion
conditions
evidence
provenance
relations
created_at
source:
  era
  object_id
  object_root
  predecessor_commit
```

The Claim Record receives a new content-derived identity. The source block
preserves the old `vf_` identity and full object root. No historical signature
is copied into a new authentication field.

Corrections and supersessions are converted to relations between full
`vcl_<64hex>` Claim content identities. The Claim identity preimage contains
the revision, assertion, conditions, Evidence references, and provenance;
relation metadata and import provenance remain in the full canonical record
root. This avoids recursive roots and remains unambiguous even when a
scientific graph contains a cycle. A missing, ambiguous, dangling, or
standing-changing mapping blocks migration.

Current authenticated Submission, Registration Record, and Verification Record
bytes are retained byte-identically. A current pending Proposal is rebuilt as
`vela.proposal.v1` only when its exact Submission and Registration Record are
valid. Its migration block names the old Proposal ID and root. Verification
Records remain separate observations.

Historical Receipt-backed, unauthenticated, terminal, abandoned, or otherwise
non-current Proposals are not made live again. They are listed in the archived
object index with exact status, source root, related object roots, and
predecessor location. The same rule applies to terminal legacy Decisions,
actor records, policies, and events.

Only Artifacts reachable from imported Claims or retained current objects
remain in the active repository. Every other tracked Artifact remains
retrievable from the predecessor.

Target Index v2 is also a predecessor object because its standing read set
contains Era-0 event, proposal, identity, dependency, and Profile roots. A
fresh v2 index is converted to event-free `vela.target-index.v3`, which retains
the exact source/input/packet bindings and replaces that legacy read set with:

```text
repository:
  epoch_id
  repository_root
```

The Target Index remains derived, non-authoritative, and deletable. An index
that is stale at the predecessor is archived rather than blessed by the epoch
transition. Its domain generator must produce a new closed candidate after
migration; Vela never carries stale ranking forward as apparently current
work.

`vela start` in a current epoch creates `vela.attempt.v2` only under the
ignored `.vela/work/` authoring directory. The Attempt closes over the exact
repository root, epoch, Target Index v3, packet, source, Git commit/tree, and
task contract. It creates no lease Event, repository record, authority
transaction, or key read. Local expiry and one target-scoped filesystem lock
prevent two local producers from silently sharing the same private Attempt;
they are coordination mechanics, not global authority or scientific state.

`vela submit` in a current epoch is an object-only repository-authority
transaction. It retains the exact authenticated Submission and
content-addressed Artifact bytes, derives one current Claim Record when the
request adds or revises a Claim, creates one pending Proposal and Registration
Record, and advances `.vela/repository.json`. A retraction Proposal targets
the exact accepted Claim without minting a replacement Claim. New Claims enter
only `pending_claims`; `accepted_claims` and scientific Standing do not
change. The Registration Record's event-log before/after fields bind the
unchanged current authority-event root, not an Era-0 scientific event log.
The transaction covers the new repository root and rebinds the derived Target
Index to it. A retained private Attempt is deleted only after the covered
transaction installs and verifies.

`vela verification import` follows the same object-only boundary. A new
Verification Record must bind one exact current pending Proposal, its exact
Submission and Claim, and only Artifact IDs already present in the current
repository. Intake adds the Verification Record and next repository manifest,
rebinds the derived Target Index, and changes no Claim standing or scientific
Event. Byte-identical Verification Records imported before the epoch remain
immutable predecessor-scoped observations: they are retained only when their
old Proposal and Claim identities map uniquely through the current Proposal's
and Claim Record's `imported_from` blocks. They do not become signatures over
the replacement objects, and the live writer cannot use that migration-only
mapping for a new record.

`vela review accept` and `vela review reject` in a current epoch read only the
current repository and current authority history. The Decision Plan binds the
exact repository, Proposal, Claim, Submission, ordered Verification Record
set, principal, action, reason, observation time, authority-event head, and
policy root. Acceptance is unavailable if any exact Verification Record fails
or errors, or if any Submission verification requirement lacks an independent
passing record that declares separation from the producer. Verifier success
does not decide the Proposal; it is only a prerequisite for the human semantic
command.

Rejection removes an add or revision Claim from `pending_claims` and leaves
accepted standing unchanged. Rejected withdrawal leaves the accepted Claim
unchanged. Acceptance moves an add Claim from pending to accepted, replaces
exactly one accepted predecessor for a revision, or removes exactly one
accepted Claim for a withdrawal. The covering repository-authority transaction
updates `.vela/repository.json`, rebinds Target Index v3, and appends either
one `review.rejected` event or one scientific domain event plus a linked
`review.accepted` event. No Era-0 reducer, Project snapshot, human Vela key,
copied confirmation root, or legacy Decision object participates.

Current `status`, `review list`, and `review show` derive Proposal standing
from those covered current authority events. Proposal bytes remain retained
after a terminal Decision; add/revision Claim files and withdrawn Claim files
remain content-addressed audit evidence even when they are no longer indexed
as active standing. More than one terminal Decision, a Decision over an
unknown Proposal, a missing or later applied event, a mismatched scientific
transition, or disagreement between Decision standing and the active Claim
indexes blocks repository verification.

Current `show`, `claim show`, `why`, and `log` use the same verified boundary.
They resolve active Claims from the repository indexes and terminal Claims
through their retained Proposal and Decision chain; expose current Submission,
Registration, Verification, Artifact, Proposal, Claim, and authority Event
records by exact full identity; and report both semantic and covering
authority-event identity. They do not load the retired Era-0 Project, event
store, proposal store, or actor registry. Missing bytes, root drift, an
unresolvable Claim, or an invalid authority chain fails closed instead of
falling back to historical replay.

### 4. One fail-closed migration command

The advanced command is:

```bash
vela repository upgrade <frontier> \
  --to current \
  --archive-dir <directory> \
  --reason <text> \
  --json
```

Without `--confirm-root`, it is key-free and emits
`vela.repository-upgrade-plan.v1`. The plan includes every removed, retained,
and created path; exact predecessor and successor roots; object counts; the
equivalence report; required archive tag; expected authority root; and a
single plan root.

Application requires:

```bash
vela repository upgrade <frontier> \
  --to current \
  --archive-dir <directory> \
  --reason <text> \
  --confirm-root sha256:... \
  --json
```

The executor rederives the plan, creates an isolated detached worktree at the
predecessor, locks the Git and Vela read set, verifies the loaded OpenSSH
repository-authority identity, creates and verifies the predecessor tag and
source archive, requests exactly one repository-authority signature, installs
the new epoch through the recoverable transaction barrier, makes one unsigned
descendant commit, atomically pushes the exact branch and tag refs, and
verifies a clean clone before advancing the operator checkout. Cancellation or
any drift writes no canonical postimage to the source Frontier. A signed
candidate that cannot be published is preserved as an isolated recovery
worktree rather than silently discarded or partially copied.

There is no `--yes`, wildcard, batch, source-key, compatibility alias, or
in-place partial mode.

### 5. Strict and non-strict behavior

Current-epoch strict verification blocks:

- a missing, duplicate, malformed, unsigned, or untrusted epoch boundary;
- wrong predecessor remote, tag, commit, tree, root, or object-manifest root;
- a missing or mismatched Claim import;
- semantic equivalence drift;
- a copied historical signature presented as current authentication;
- a retained current object whose canonical bytes, links, or authentication
  fail;
- an archived Proposal made live without a valid current Submission and
  Registration Record;
- missing reachable Artifacts;
- a legacy path or schema in the active repository;
- a Target Index v2 in the active repository, or a Target Index v3 whose
  source, input, packet, epoch, or repository binding drifts;
- a current Attempt that retains an event-rooted task binding or does not bind
  the exact current repository, Target Index, packet, and Git read set;
- a forked, rolled-back, incomplete, or wrongly rooted current authority
  chain; and
- any current event or Decision that targets an archived-only object.

Non-strict mode reports the same defects and grants no standing, authority,
legacy fallback, or partial import.

### 6. Compatibility and replay

The current Vela binary replays only Profile v2/current-epoch repositories.
Encountering Profile v1 returns a bounded diagnostic naming:

1. the exact last Vela release that replays it;
2. `vela repository upgrade`; and
3. the requirement for a separately pinned repository-authority trust anchor.

Historical replay remains available through immutable Git tags, release
artifacts, conformance bundles, and the source archive. It is not linked into
the current release graph.

The current repository preserves Git ancestry: the epoch commit is a normal
descendant of the predecessor. No force-push or history rewrite is allowed.

## Migration order

The controlled order is:

1. a disposable mixed-era fixture;
2. Quantum Codes;
3. Formal Conjectures;
4. Sidon;
5. Erdős.

Each Frontier must pass preview, archive verification, semantic equivalence,
apply, strict replay, clean-clone replay, remote tag verification, and new
trust-anchor installation before the next begins.

After all four pass, Vela removes the retired parser, reducer, writer,
projection, CLI, fixture, and dependency paths. Vela Web rebuilds its read
model from only current objects. Canopus changes only if a retained replay
fixture imports a retired type.

## Conformance contract

Focused tests must cover:

- deterministic predecessor object-manifest and archive roots;
- exact Claim Record conversion and stable `vf_` to full `vcl_<64hex>` mapping;
- full assertion, condition, evidence, provenance, relation, and standing
  equivalence;
- current Submission/Registration/Verification byte retention;
- exact current Proposal v1 reconstruction;
- terminal and Receipt-backed Proposal archival;
- reachable Artifact closure and unreferenced Artifact archival;
- missing, extra, mutated, duplicated, reordered, cyclic, or ambiguous inputs;
- Git dirt, remote mismatch, non-ancestor head, unpushed predecessor, tag
  substitution, active journal, root drift, and archive write failure;
- wrong key, wrong trust anchor, signer refusal, plan drift, and cancellation
  with zero canonical writes;
- recovery before and after signature with exactly-once commit semantics;
- clean-clone current replay with the predecessor unavailable;
- optional historical audit with the exact predecessor available;
- rejection of every retired path and schema in Profile v2; and
- exact Quantum, Formal, Sidon, and Erdős migration vectors.

Focused commands:

```bash
cargo test -p vela-protocol repository_epoch
cargo test -p vela-cli repository_upgrade
cargo test -p vela-cli current_only_replay
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

The deterministic full release union runs once at the actual `v0.940.0`
boundary. External Lean, Diderot, live-network, and unrelated suites remain
excluded.

## Consequences

Vela loses same-binary replay of historical eras and gains one understandable
current protocol, one authority chain, one repository layout, and one reader
model. Historical audit becomes explicit and versioned rather than an
ever-growing branch inside every daily code path.

This is acceptable before external adoption because all active repositories
are controlled and every predecessor remains immutable and independently
retrievable. It would not be an acceptable silent upgrade after a public
compatibility promise.
