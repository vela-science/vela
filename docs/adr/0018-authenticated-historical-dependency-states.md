# ADR 0018: Authenticated historical dependency states

- Status: Proposed
- Protocol effect: no new event kind, reducer transition, canonical dependency
  object, signature algorithm, or authority rule
- Candidate release: Vela `v0.915.0`
- Scientific authority effect: None
- Entry gate: reproduced Erdős to Formal migration failure under Vela `0.914`

## Context

ADR 0016 made a repository boundary the signed, non-scientific authentication
point for stable Frontier identity and exact dependency pins. Vela `0.914`
correctly derives an `ExactFrontierDependencyV1` at the signed boundary's Git
anchor. The first real Erdős migration preview exposed one narrower case that
the released resolver cannot express.

Erdős immutably pins this historical Formal Conjectures state:

```text
commit:   a143c351f8488e0c621598307e248373d9dc3374
tree:     093e84c03a722e5367812a6e6240b1c28042f969
snapshot: sha256:48ec4e84bb4640fa54023db58d7eabc6a713a46b053b6ccc3050414ab18520ec
```

The reviewed Formal migration candidate anchors thirteen descendant commits
later:

```text
commit:   4e6f040aa204f0dcdf26b4b5c39779cef03fbefc
tree:     5b001592ce7e3d56ac039872b2dfcf9bb5a27e65
snapshot: sha256:45fa712bd6d9a8d4c8514a7cba107e7f814f2c1368805abd577e762ccb6123a4
identity: sha256:1832b08fb8a4a9afcbdfcd0b7d9743e7949efa795cc3133ce33087a7ed8b08c0
```

The historical commit is an exact ancestor of the later anchor. Vela `0.914`
still rejects the Erdős preview because it requires the dependency boundary's
anchor snapshot to equal the legacy pin. That equality is sufficient, but it
is not necessary when one independently pinned later boundary can authenticate
an exact retained ancestor.

Creating a second first Formal boundary at the historical commit would create
competing administrator roots. Retargeting Erdős to current Formal would change
its immutable scientific context. Rewriting or splicing Git history is
forbidden. The missing operation is a stricter read derivation, not another
authority object.

The first real regression also exposed two operational preconditions that the
synthetic fixtures did not:

- Erdős had 32 content-addressed local artifact objects whose exact bytes were
  already retained under tracked `witnesses/` paths but not at the canonical
  artifact locators named by its immutable records. Commit
  `5ac839c385b92d62c9f323fa6eff26beb5fd4f5a` restores those identical bytes at
  the named paths without changing any event, artifact record, or scientific
  root.
- the historical Formal manifest carries the retired, non-scientific
  `carina.kernel` field. Historical replay recognizes that one closed legacy
  field as inert migration input; Profile v1 still rejects it, and migration
  drops it.

Neither repair grants dependency authentication. They make the exact historical
bytes available so the signed boundary proof can be evaluated.

## Decision

Clarify the existing `vela.exact-frontier-dependency.v1` semantics:

```text
frontier_id + identity_root
    identify one authenticated Frontier repository

git_commit + git_tree + scientific_state_root
    identify one exact state of that repository
```

The exact state may be the authenticating boundary anchor, as in Vela `0.914`,
or an exact retained ancestor of the first temporalization boundary. Add one
read-only derivation equivalent to:

```text
derive_exact_dependency_at_temporalized_ancestor(
  repository,
  selected_boundary_leaf,
  consumer_trust_anchor,
  historical_commit,
  expected_legacy_snapshot
) -> ExactFrontierDependencyV1
```

No wire schema changes. `vela.frontier-dependency-migration.v1` already binds:

- the exact legacy descriptor and snapshot pin;
- the local repository resolution hint;
- the selected signed boundary's full content root;
- an independent consumer trust anchor; and
- the complete expected `ExactFrontierDependencyV1`, including historical
  commit, tree, identity, and scientific-state roots.

Every security value is rederived. A repository path remains a local retrieval
hint and is excluded from `dependency_root`.

### 1. Select and verify the authenticating chain

Before resolving a historical state, Vela must:

1. validate the complete repository-boundary event set;
2. require exactly one valid boundary leaf and require the selected boundary to
   be that leaf;
3. verify the complete chain from its first administrator boundary to that
   leaf;
4. require an independently supplied trust anchor for the first administrator
   boundary and key;
5. rederive every signed Git/Vela anchor, signature, actor-registry fact,
   retained-object manifest, and chain-continuity fact required by ADR 0016;
   and
6. require the chain root to be `temporalize_existing`.

The historical state inherits only the stable identity authenticated by that
first temporalization boundary. A later dependency-update leaf cannot
retroactively supply a different dependency context to the earlier state.

### 2. Resolve an exact ancestor through hardened Git

The requested historical commit must:

1. be one full object ID in the repository's declared object format;
2. resolve exactly to that commit object with replacement refs, graft-like
   redirects, inherited `GIT_*` variables, hooks, attributes, external diffs,
   submodule recursion, prompts, and mutable global/system configuration
   disabled;
3. be an ancestor of the temporalization boundary payload's signed
   `anchor_git_commit`, not merely an ancestor of current `HEAD`;
4. have all required Git objects available locally; a shallow or partial view
   that cannot prove the state fails closed; and
5. rederive its exact tree from immutable Git objects.

No timestamp, branch, tag, remote, locator, label, or short digest participates
in eligibility.

### 3. Prove historical Vela-state retention

Vela materializes the historical Project from the exact Git tree and requires:

1. successful deterministic load and replay;
2. the same Frontier ID as the signed temporalization boundary;
3. the same legacy identity preimage root bound by that boundary;
4. every historical event canonical preimage to remain a member of the
   temporalization-anchor event set;
5. every historical event that already carried a signature to retain a valid
   signature, and no changed preimage to hide behind the same display ID;
6. every historical proposal's immutable identity and producer provenance to
   remain present at the temporalization anchor;
7. every historical retained event, proposal, Receipt, actor, policy,
   artifact, evidence object, and referenced canonical file to remain present
   at the temporalization anchor with the same path, Git mode, size, and
   SHA-256; and
8. the historical legacy snapshot root to equal the immutable child pin.

The first implementation additionally requires both the historical Project
and the temporalization boundary to have the canonical empty dependency set.
This holds for the reproduced Formal/Erdős vector and avoids inventing
recursive historical-dependency semantics. A non-empty case remains blocked
until separately specified and tested.

### 4. Derive the existing exact dependency record

Vela computes the historical `scientific_state_root_v2` from:

- the exact historical Project bytes;
- the stable `identity_root` authenticated by the temporalization boundary;
  and
- the canonical empty dependency root authenticated by that boundary.

It then returns the existing `ExactFrontierDependencyV1` using the historical
commit and tree. The result must equal the complete expected record in the
migration input byte-for-byte. A mismatch in any field blocks migration.

Boundary-anchor equality remains the unchanged Vela `0.914` path. The ancestor
path is used only when the expected exact commit differs from the verified
boundary anchor.

## Migration and replay semantics

- Preview is key-free, zero-write, and binds the exact dependency input bytes,
  selected boundary, trust anchor, historical state, binary, and resulting
  migration plan root.
- Apply rederives the complete dependency proof immediately before any
  protected signing prompt. Under the transaction barrier, Vela then rechecks
  the migrating Frontier read set and the exact signed plan. External
  dependency repositories are retrieval inputs, not transaction participants:
  drift during the approval prompt cannot change the signed dependency bytes,
  but later strict resolution may report that those exact objects are no
  longer locally available.
- The eventual Erdős migration writes the ordinary ADR 0016
  `frontier.repository_bound` event with the existing exact dependency record.
  It adds no Formal event and rewrites no historical byte.
- Vela `0.914` continues replaying that canonical event and dependency record
  because their wire semantics are unchanged. It cannot prepare the historical
  migration and may conservatively report the dependency unavailable.
- Missing history can be supplied by an exact Git checkout or bundle. Vela does
  not fetch automatically.

This authentication proves repository continuity only. It does not
retroactively attribute unsigned Formal events to the boundary administrator,
turn a dependency pin into evidence or a transfer edge, or imply acceptance of
any scientific assertion. Those distinctions also preserve the Kernel,
Frontier Algebra, and Discovery Calculus boundary in ADR 0017.

## Strict and non-strict behavior

An unavailable or invalid historical state grants no dependency resolution.

- Strict migration remains blocked until every exact check succeeds.
- Non-strict inspection may report the unresolved dependency but must not
  substitute current state, another commit, or a weaker identity.
- Backdated timestamps have no effect.
- A shallow clone, missing object, fork, wrong root, or ambiguous boundary is
  an error, not a warning-based exemption.
- Existing Frontiers and exact boundary-anchor dependencies replay unchanged.

Diagnostics must distinguish unavailable history, non-ancestry, tree mismatch,
snapshot mismatch, retention mismatch, dependency-context drift, and failed
authentication. Their human-readable wording is not a protocol surface and no
new persisted strict-signal vocabulary is introduced.

## Adversarial cases

Conformance must fail closed when:

- the historical commit is a sibling, fork, or descendant of the signed
  temporalization anchor;
- it is only an ancestor of current `HEAD`;
- the object is missing, shallow, replaced, grafted, abbreviated, or reached
  through hostile Git configuration;
- another repository copies plausible `.vela/` bytes or Frontier metadata;
- the commit, tree, object format, Frontier ID, legacy identity preimage, or
  legacy snapshot is wrong;
- a historical event is deleted, changed, stripped of a signature, corrupted,
  or replaced under a colliding display handle;
- immutable proposal or Receipt provenance changes;
- a retained artifact, policy, actor, evidence object, path, mode, size, or
  digest changes;
- the historical or temporalization dependency context is non-empty;
- the boundary is unsigned, forked, cyclic, non-leaf, wrongly pinned, or
  signed by the wrong administrator key;
- supplied exact output values disagree with rederivation; or
- the repository, input, binary, or roots drift before the apply preview is
  rederived; or the migrating Frontier read set drifts before commit.

## Exact conformance contract

Focused implementation checks:

```bash
cargo test -p vela-edge authenticated_ancestor_dependency
cargo test -p vela-edge git_read::tests
cargo test -p vela-cli --lib cli::migration::tests
cargo test -p vela-protocol retired_carina_field_is_readable_only_in_legacy_manifests
cargo test -p vela-protocol --test frontier_repository_bound
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

Required named cases and their current implementation status:

```text
authenticated_ancestor_dependency_derives_exact_v1_pin                 implemented
authenticated_ancestor_dependency_uses_temporalization_identity       implemented
authenticated_ancestor_dependency_rejects_missing_or_forked_history   implemented
authenticated_ancestor_dependency_rejects_snapshot_mismatch           implemented
authenticated_ancestor_dependency_rejects_nonreplayable_history       implemented
authenticated_ancestor_dependency_rejects_event_signature_or_proposal_loss implemented
authenticated_ancestor_dependency_rejects_retained_object_loss_or_mutation implemented
authenticated_ancestor_dependency_rejects_nonempty_dependency_context implemented
migration_historical_dependency_preview_uses_authenticated_ancestor   implemented
dirt_check_drains_large_path_and_hash_streams_without_deadlock         implemented
retired_carina_field_is_readable_only_in_legacy_manifests              implemented
exact_dependency_pin_is_derived_from_the_boundary_anchor               existing 0.914 compatibility
migration_frontier_repo_v1_injected_signer_applies_one_exact_transaction existing transaction rederivation
migration_cancellation_zero_writes_and_crash_recovers                  existing zero-write recovery
erdos_formal_historical_dependency_read_only_regression                pre-ceremony branch passed; post-boundary branch is release gate
```

The focused synthetic tests mutate the expected Git tree and consumer trust
anchor and require both failures before any source write. The existing
migration transaction tests continue to cover late rederivation, cancellation,
drift, and recovery for the same plan executor.

The real ignored vector is bound to Erdős `main` commit
`e79feaeddf2d4c68ce395d2e7daec1e7fae41702`, all 1,217 target packets, both
canonical `.vela` stores, and external candidate files under
`~/Desktop/Constellate/Archives/vela-0.914-migration-previews-2026-07-23/`.
Before a Formal ceremony it now passes by requiring the anticipated all-zero
boundary root to resolve to zero events, independently sealing the complete
1,217-target candidate as Target Index v2, and proving zero writes plus
unchanged Erdős roots and the exact 1,511/81 strict-blocker distribution. The
post-ceremony branch requires exactly one Formal repository boundary, selects
that event by its full content root, and checks that its signed payload names
the reviewed `4e6f040a…` commit, `5b001592…` tree, and
`sha256:45fa712b…` snapshot before the historical dependency preview may pass.

ADR acceptance and Vela `0.915.0` remain blocked. After the human creates
Formal's first protected temporalization boundary, the same external
dependency file must replace only its two boundary-root fields with the real
content root. The unchanged test must then take its positive branch and prove
that the signed later boundary authenticates the retained historical state
without changing the pin, creating a competing Formal boundary, or rewriting
history.

## Alternatives rejected

### Competing boundary at the historical commit

Rejected. It creates ambiguous first administrator authority and a forked
repository identity.

### Retarget Erdős to current Formal

Rejected. It changes immutable scientific context rather than authenticating
the context Erdős already names.

### Rewrite or splice history

Rejected. It destroys exact Git/Vela continuity and is unnecessary.

### Git ancestry or snapshot equality alone

Rejected. Ancestry alone does not prove Vela replay, identity, retained bytes,
or historical state. Snapshot equality alone omits Git and trust identity.

### A new dependency schema, historical-state event, or second signature

Rejected. The existing dependency record and migration input already contain
the necessary exact fields. Another object or ceremony adds authority surface
without increasing assurance.

### Mutable locators, timestamps, or short identifiers

Rejected. They permit substitution or backdating and cannot prove exact
membership.

## Consequences

The change is deliberately small: one stricter read derivation and one
migration branch. It closes a reproduced, immutable-history integration gap
without weakening verification, changing scientific authority, or promoting
the broader dependency-standing proposal in ADR 0009.

Because this extends the trust semantics deliberately absent from Vela
`0.914`, it requires a reviewed ADR and candidate `v0.915.0`, even though the
canonical wire format remains unchanged.
