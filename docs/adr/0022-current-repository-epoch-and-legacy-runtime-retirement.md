# ADR 0022: Current repository epoch and legacy runtime retirement

- Status: Proposed
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
- Entry gate: all four active Frontiers replay at exact predecessor commits and
  the migration tool proves semantic equivalence before requesting a signature

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
3. repository-authority replay begins by re-verifying the complete Era-0 event,
   actor, policy, and signing history.

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
