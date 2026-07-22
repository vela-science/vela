# ADR 0016: Frontier Repository Profile v1 and legacy identity migration

- Status: Proposed
- Protocol effect: one signed non-scientific boundary event, versioned state
  root, closed repository profile, and derived-root contracts
- Candidate release: Vela `v0.914.0`
- Claim-acceptance authority effect: None; adds a repository-administration
  authorization and continuity rule

## Context

ADR 0001 correctly made a Frontier an ordinary Git repository. ADR 0010 kept
initialization minimal and domain-neutral. Released Vela packages a Frontier
with:

```text
vela.frontier_manifest.v0.1
vela.frontier_repo.v0.1
vela.frontier_lock.v0.1
```

The broad architecture remains:

```text
Git stores and collaborates.
Vela interprets and governs scientific state.
Domain tools produce evidence.
Readers project replayed state.
```

The v0.1 profile does not uphold that separation. Exact tests against the
released implementation and the four maintained Frontiers reproduced these
defects:

1. `vela init --scope <question>` writes the question to `SCOPE.md` but leaves
   `frontier.yaml.scope.question` empty.
2. The loader accepts and ignores wrong manifest schema, layout, mode,
   visibility, reducer, path, and policy-location values.
3. Human-editable manifest `name`, `description`, `frontier_id`, and
   `dependencies.frontiers_v2` override loaded Project state and therefore
   change `snapshot_hash` without an authority event.
4. Strict verification initially detects stale generated files, but ordinary
   `vela frontier materialize` regenerates the lock and proof and then reports
   strict success over the unsigned metadata change.
5. Scope, license, maintainers, legacy dependency lists, and most policy fields
   are unrooted or inert.
6. `.vela/config.toml` mixes reducer seed metadata with shared runtime settings.
   A normal Vela state save rewrites the project section and can erase a
   previously valid `[work]` or other runtime setting.
7. All four maintained Frontiers have zero `frontier.created` events. Their
   current IDs therefore cannot be validated against genesis.
8. Three of four maintained `vela.target-index.v1` files are stale against
   current state, yet `vela next` still returns their offers without exposing
   that fact in its compact contract.

These are protocol-boundary defects, not merely documentation debt. A
regenerable lock containing `manifest_root` cannot distinguish an authorized
legacy identity or dependency migration from an attacker rewriting the
manifest and regenerating the lock. A proper v1 must separate repository
metadata from scientific state and bind the legacy boundary once.

## Decision

Introduce Repository Profile v1 with:

```text
vela.frontier-profile.v1
vela.frontier_repo.v1
vela.frontier_lock.v1
vela.frontier_repo_proof.v1
vela.frontier-identity.v1
vela.frontier-repository-boundary.v1
vela.target-index.v2
```

The smallest protocol addition is a signed, non-scientific
`frontier.repository_bound` event. Its purpose is only to bind stable Frontier
identity, exact dependency pins, and the old-to-new root boundary. The profile
is deliberately not its authority object. The event cannot add, accept,
reject, supersede, correct, or retract a finding.

### 1. One repository, one authority boundary

One ordinary Git repository contains one canonical `.vela/` history by
default. Split a Frontier when authority, correction policy, confidentiality,
stable namespace, or steward group diverges—not merely because file count
grows.

A portfolio, workspace, or Atlas may pin several exact Frontier roots. It is a
derived collection unless it owns separately bounded claims and signed events.
Nested canonical `.vela/` histories must not be presented as one Frontier.

### 2. Closed non-authoritative profile

`frontier.yaml` becomes the human-editable Repository Profile:

```yaml
schema: vela.frontier-profile.v1
frontier_id: vfr_...
name: Bounded human-readable name
summary: One concise description
scope:
  question: Which bounded scientific question does this Frontier maintain?
  includes: []
  excludes: []
maintainers: []
license:
  content: CC-BY-4.0
  code: Apache-2.0
  data: varies
```

The schema is closed at every mapping. It contains discovery, onboarding, and
stewardship metadata only. It does not seed or override replayed Project state.
Changing name, summary, scope, maintainers, or license changes the profile and
Git roots, not the scientific-state root or standing.
`maintainers` is contact/discovery metadata; it never grants signing,
acceptance, policy, or migration authority. Those permissions continue to come
only from verified actor and policy state.

V1 removes v0.1 fields that duplicate fixed or separately bound behavior:

- `layout` and `mode`;
- `visibility`;
- `vela.reducer`;
- configurable `paths`;
- review and proof policy paths;
- `dependencies.frontiers`, `packages`, and `adapters`; and
- `dependencies.frontiers_v2`.

Repository paths are fixed by `vela.frontier_repo.v1`. Exact tool pins belong in
`vela.lock`. Exact scientific dependencies belong in the bound dependency
record below. Host packages and workbench adapters remain removable edge
integrations rather than manifest vocabulary.

Vela normalizes the parsed profile as canonical JSON and computes:

```text
profile_root = sha256(canonical_json(vela.frontier-profile.v1))
```

YAML whitespace, comments, key order, quoting, and final newlines do not change
the root. Semantic field changes do. The lock and proof repeat the current
profile root so exact repository metadata is inspectable. It is not a
scientific or authority root.

Parsing is deliberately narrower than general YAML. Duplicate mapping keys,
anchors, aliases, merge keys, and explicit YAML tags are rejected before
deserialization. Every text scalar must already be Unicode NFC and may contain
no Unicode control character other than line feed. Vela never silently
normalizes a parsed value. Duplicate list values are compared after this
validation, and one scope statement cannot appear in both `includes` and
`excludes`. These rules make the same semantic profile root portable across
implementations without inventing a Vela-specific document language.

The profile's `frontier_id` is a checked assertion about the bound identity,
not the identity source. A mismatch is a strict repository-profile blocker and
prevents canonical writes, but it cannot replace the identity derived from
`frontier.created` or the latest valid repository boundary.

### 3. Stable identity and dependency roots

Vela computes a full `identity_root` from a closed
`vela.frontier-identity.v1` preimage. The preimage is never supplied as a bare
trusted hash. It contains:

```text
schema
frontier_id
origin: genesis | legacy_boundary
origin_commitment
legacy_identity_preimage_root | null
```

For a new Frontier, `origin_commitment` is the full SHA-256 of the ordinary
canonical `frontier.created` event preimage, not the truncated `vev_` or
`vfr_` display handle. For a migrated legacy Frontier it is the canonical root
of the exact legacy identity preimage plus the anchored Git commit/tree and
event-log root/count. `legacy_identity_preimage_root` repeats the exact v0.1
fallback metadata commitment in that case. This avoids any self-reference to
the boundary event that carries `identity_root`. Implementations recompute the
identity root and reject a supplied value that disagrees.

Concretely, the legacy `origin_commitment` is the canonical root of
`{schema:"vela.legacy-frontier-origin.v1", frontier_id,
legacy_identity_preimage_root, git_object_format, anchor_git_commit,
anchor_git_tree, anchor_event_log_root, anchor_event_count}`. `identity_root`
is then the canonical root of the closed identity record above. Every root
preimage is domain-separated by its schema.

Exact cross-Frontier dependencies move out of human-editable profile metadata.
The current dependency list is carried inside the latest valid
`frontier.repository_bound` payload. Each closed entry contains:

```text
frontier_id
identity_root
scientific_state_root
git_object_format
git_commit
git_tree
locator
```

The locator is retained for retrieval but is never a security identity.
Implementations canonicalize the entries, sort by `(frontier_id,
identity_root)`, reject duplicates, and compute:

```text
dependency_root = sha256(canonical_json(exact_dependency_list))
```

Short roots, mutable tags, missing Git objects, or a locator whose resolved
bytes disagree with the exact roots are invalid. A Frontier with no exact
dependencies uses the canonical root of the empty list.

The current Erdős dependency on the historical Formal Conjectures snapshot is
preserved as an exact historical commitment. Its retired `vela.hub` source and
old locator remain only in the anchored v0.1 bytes. Migration translates them
to a v1 Git identity and root record only after proving both descriptions
resolve to the same retained content; ambiguity blocks migration. A later
retarget to Formal v1 requires a new signed boundary event.

### 4. Signed repository-boundary event

Define event kind:

```text
frontier.repository_bound
```

with payload schema:

```text
vela.frontier-repository-boundary.v1
```

and fields:

```text
mode: temporalize_existing | update_dependencies
frontier_id
identity_root
observed_profile_root
dependency_root
dependencies
previous_boundary_event_root | null
legacy_identity_preimage_root | null
administrator_actor_id
administrator_public_key
administrator_algorithm: ed25519
trust_mode: tofu | previous_boundary
git_object_format: sha1 | sha256
anchor_git_commit
anchor_git_tree
anchor_event_log_root
anchor_event_count
anchor_snapshot_root
anchor_snapshot_schema
anchor_proposal_root
anchor_actor_registry_root
anchor_artifact_registry_root
anchor_canonical_store_root
```

Every `*_root` is `sha256:<64 lowercase hex>`. Git object length must match the
declared repository object format. `previous_boundary_event_root` is the full
SHA-256 of the preceding boundary event's ordinary canonical preimage, not its
truncated event ID. The legacy anchor is always the pre-boundary Git commit and
event prefix; no field may refer to the commit that will contain the event
itself.

The remaining anchor roots have one meaning:

- `anchor_proposal_root` is the released proposal aggregate-root algorithm;
- `anchor_actor_registry_root` is SHA-256 of the exact anchored
  `.vela/actors.json` bytes, or the canonical empty registry when absent;
- `anchor_artifact_registry_root` is the canonical JSON root of the anchored
  protocol artifact records; and
- `anchor_canonical_store_root` is the root of
  `vela.retained-object-manifest.v1`.

The retained-object manifest is a canonical JSON list of sorted entries
`{path, git_mode, size, sha256}`. It covers every tracked regular file under
the canonical event, proposal, finding, artifact, actor, and policy stores,
every retained Receipt, and every tracked file reached by an exact Receipt,
artifact, or signed-policy locator. Paths must be normalized UTF-8 repository
paths. Symlinks, submodules, traversal, platform case-fold collisions, Unicode
normalization collisions, duplicate paths, and missing locator targets fail
closed. This includes legacy Sidon artifact blobs and Formal's Receipt-bound
Lean file without making their old directories the v1 layout for new writes.

The ordinary canonical event signature binds the payload and signer. The event
targets the exact Frontier, uses null before/after scientific finding hashes,
and is reducer-neutral except for repository identity and dependency boundary
state. `identity_root` and `dependency_root` are recomputed from the closed
payload before the signature can grant validity.

The event core is fixed:

```text
target: {type: frontier, id: <frontier_id>}
actor: {type: human, id: <administrator_actor_id>}
before_hash: sha256:null
after_hash: sha256:null
signature: ordinary Ed25519 event signature
```

The anchored actor registry must contain exactly one record matching the
administrator actor, public key, and algorithm. That record must be active in
the anchored causal state; changing the event timestamp cannot bypass a
revocation already present at the anchor. Actor naming or registration alone
grants no repository authority.

Modes:

- `temporalize_existing` binds one exact legacy Git/Vela membership set. Every old event,
  proposal, Receipt, registration, policy, decision, finding, and artifact byte
  remains byte-identical; the boundary event is appended. Its trust mode is
  `tofu`, its previous boundary is null, and consumers must pin the resulting
  boundary root or release tag out of band.
- `update_dependencies` must chain the previous boundary event root and must
  preserve `identity_root`, administrator actor, and administrator public key.
  Its trust mode is `previous_boundary`. A Frontier identity is immutable;
  changing it creates a different Frontier. Display-only profile edits do not
  require an event. V1 deliberately has no administrator-key rotation; that
  requires a separately reviewed recovery contract.

A new v1 Frontier does not emit a self-referential boundary. Its first
`frontier.created` event establishes identity and the empty dependency root.
The event preimage never contains a Git commit that contains itself. The first
later dependency update anchors the exact pre-update Git commit, tree, event
log, and state roots, then appends one `frontier.repository_bound` event.

For a maintained legacy Frontier, `temporalize_existing` requires a registered
human identity using the existing protected signer. Operationally the command
requires a protected human backend; protocol verification relies on the exact
signature, anchored registry record, boundary chain, and the consumer's
out-of-band pin rather than pretending the open actor registry created a root
of trust. The migration command may prepare and invoke the exact request, but a
model cannot approve the OS card or read the key. This is a rare
repository-administration ceremony, not an everyday scientific decision. The
prompt names the Frontier, profile version, dependency summary, anchor roots,
reason, administrator key, trust mode, and boundary plan root.

The first boundary is always explicit TOFU unless a future protocol names and
verifies a stronger prior trust source. It cannot manufacture historical trust
and must be labeled `integrity_verified_tofu`, never `trusted`, by readers.

`observed_profile_root` proves which human-facing profile was presented at
the ceremony. It is historical audit data. Strict verification does not
require the current editable profile to equal it.

### 5. Scientific-state root v2

The current `snapshot_hash` covers display and operational Project metadata.
Profile v1 introduces `scientific_state_root_v2` as the canonical root of this
closed component-root record:

```text
schema: vela.scientific-state.v2
identity_root
dependency_root
findings_root
sources_root
evidence_atoms_root
condition_records_root
review_events_root
confidence_updates_root
artifacts_root
released_diff_packs_root
verdict_conflicts_root
contradictions_root
verifier_attachments_root
attempts_root
attempt_resolutions_root
transfers_root
endorsements_root
statement_attestations_root
anchor_links_root
statement_registrations_root
```

Each component root is the SHA-256 of its protocol-defined canonical JSON
collection, including the canonical empty collection. The closed record makes
field inclusion reviewable and prevents a future `Project` field from silently
entering or leaving the security identity through Serde reflection.

It excludes:

- display name, summary, scope prose, maintainers, and licenses;
- compiler label, compiled timestamp, counters, and UI statistics;
- Git host, branch, locator-only metadata, and runtime settings;
- events, signatures, proposals, actor registry, active leases, private
  coordination, proof packets, and projections, each of which has its own root
  or explicitly non-scientific role.

In particular, graphs, provenance circuits, support/opposition projections,
Belnap or graded lenses, assurance summaries, information scores, and action
rankings never enter this Kernel root. Exact repository dependencies are
context pins only; they imply no transfer, support, opposition, or standing.
Those questions belong to the read-only Frontier Algebra and Discovery
Calculus boundaries in ADR 0017.

The existing `snapshot_hash` remains available as `legacy_snapshot_root` when
reading v0.1 histories and is recorded in the migration boundary. V1 JSON
contracts use `scientific_state_root`; compatibility output may expose
`legacy_snapshot_root` explicitly but must not call both values `snapshot`.

This root split changes no historical event preimage. The boundary event
records the exact old root and the v1 lock records the new one. A signed policy
or protected decision that historically bound an old snapshot continues to
verify against its historical bytes.

### 6. Runtime settings split

V1 removes reducer seed metadata and runtime preferences from
`.vela/config.toml`.

- Shared, allowlisted Frontier preferences move to `.vela/settings.toml`.
- User preferences remain in `~/.vela/config.toml`.
- Vela state saves, work, land, materialize, and recovery never rewrite
  settings.
- V0.1 remains read-compatible. Its config writer must preserve unknown
  legacy sections so a normal command cannot erase settings before migration.
- The migrated identity and dependency state comes from the signed boundary,
  not from either settings file.

The v1 settings file is closed and begins with
`schema = "vela.frontier-settings.v1"`. Its initial allowlist is the existing
Frontier-scoped preference set: `publish.git_push` (narrowing only),
`work.lease_ttl_seconds`, and `mcp.profile`. Credentials, key paths, tokens,
commands, hooks, network endpoints,
policy, actor, verifier, dependency, and accepted-state fields are forbidden.
Unknown keys fail settings validation but do not change replayed state.
Precedence remains command flag, allowlisted environment override, Frontier
settings, user config, then built-in default. `.vela/config.toml` is absent
from an active v1 repository after migration.

### 7. Path ownership

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/events/`, retained policy pairs, actor registry | Canonical protocol | Change only through released Vela transitions |
| `.vela/proposals/`, `.vela/findings/`, `.vela/artifacts/` | Retained or reducer-owned protocol records | Never hand-edit or decommit before separate event-sourcing proof |
| `.vela/settings.toml` | Allowlisted operational preferences | Never scientific state; preserve across writes |
| `records/receipts/sha256/` and retained evidence bytes | Exact retained evidence | Preserve by full digest |
| `frontier.yaml` | Human Repository Profile | Edit deliberately; validate and re-materialize |
| `frontier.json` | Generated read projection | Delete or regenerate; never hand-edit |
| `vela.lock` | Generated exact root/tool witness | Delete or regenerate; never hand-edit |
| `proof/` | Generated Vela replay and integrity packet | Reserved for generated output, subject to legacy exceptions |
| `README.md`, `SCOPE.md` | Frontier Card and expanded scope | Human onboarding; no duplicate machine front matter |
| `VELA.md` | Canonical agent charter | Generates tool adapters explicitly |
| `targets.json`, target packets | Optional derived work projection | Rooted, freshness-checked, and non-authoritative |
| domain-native files | Scientific source and evidence | Stable identity paths, not mutable status folders |
| `.vela/work/`, operation journals | Local-only coordination and recovery | Never publish as scientific state |

Historical retained bytes are not deleted to make the layout prettier:

- Sidon's tracked `.vela/artifact-blobs/` remain public retained artifacts
  until an exact digest-locator migration preserves every reference.
- Formal Conjectures' Receipt-bound `proof/erdos505-test-dim-one-proof.lean`
  remains resolvable at its historical path. New formal work uses
  `formal/lean/`; retiring the old locator requires a separately verified
  digest-based compatibility rule and never rewrites the Receipt.

### 8. Minimal initialization

`vela init <path> --name <name> --scope <question>` creates a v1 profile and a
real `frontier.created` event whose closed payload supplies the identity
preimage and binds the canonical empty dependency root. The supplied name and
scope are written once into the profile and rendered into the initial Frontier
Card and `SCOPE.md`.

The v1 genesis payload is closed:

```text
schema: vela.frontier-created.v1
name_at_creation
creator
profile_schema: vela.frontier-profile.v1
dependency_root: <canonical empty dependency list root>
created_at
```

The event targets `{type: frontier, id: <name_at_creation>}`, uses null
before/after hashes, and remains an unsigned structural genesis. Its full
canonical preimage root establishes TOFU identity; it grants no human signing
or claim-acceptance authority.

Initialization still creates no domain directory matrix, target catalogue,
proof packet, CI adapter, MCP file, editor configuration, host registration,
or executable hook. Optional Frontier Kits remain non-protocol conventions
applied afterward.

### 9. Target index v2

`vela.target-index.v2` keeps the useful v1 work interface and adds exact:

```text
source Git commit and tree
event-log root
scientific-state root
proposal root
identity root
dependency root
index root
packet schema, path, size, and SHA-256
generated-by Vela version
```

`vela next` and `vela work` fail closed on stale roots. Human output explains
that the catalogue needs regeneration; JSON reports configured, available,
returned, and stale counts plus one repair command. A stale target remains
inspectable by exact ID but is not offered or leased.

The indexed Git commit/tree is the exact input revision projected, never the
later commit that stores the generated index. It must be an ancestor of the
inspected checkout and its tree must resolve exactly. A later documentation or
profile-only commit does not stale the index when every security-bearing root,
input-path root, and packet byte still matches. A source, state, dependency,
proposal, packet, or identity change does. `index_root` is computed with its
own field omitted. Target IDs are unique; packet paths are normalized relative
paths and may not traverse or resolve through symlinks.

The closed stale codes are:

```text
target_index_source_unavailable
target_index_source_not_ancestor
target_index_event_root_mismatch
target_index_state_root_mismatch
target_index_proposal_root_mismatch
target_index_identity_root_mismatch
target_index_dependency_root_mismatch
target_index_input_root_mismatch
target_index_packet_mismatch
target_index_duplicate_target
target_index_invalid_path
```

Graph position or structural advice never replaces canonical work ranking.
Domain packet schemas remain owned by their Frontiers.

The index may record the observed `profile_root` for display audit, but profile
drift alone does not invalidate a work offer. Scientific state, dependencies,
proposals, packets, and source identity do.

## Migration

Add a two-phase protected migration:

```bash
vela migrate <frontier> --to frontier-repo-v1 --check \
  --profile <candidate-profile.yaml> --as <human-actor> --reason <text> --json
vela migrate <frontier> --to frontier-repo-v1 --apply \
  --profile <candidate-profile.yaml> --as <human-actor> --reason <text> \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

Without confirmation, the command is key-free and returns the exact migration
plan, boundary payload, before/after root families, touched files, signer,
reason, trust mode, and plan root. The candidate profile is read-only input and
lets the user resolve missing legacy description or scope without dirtying the
source checkout. With a matching non-expired confirmation, the command
revalidates the binary, Git ancestry, all roots, registry, signer, read set,
candidate profile, and expiry before the protected OS prompt.

Cancellation, timeout, authentication failure, root drift, or stale input
writes no event, profile, generated view, journal commit marker, or Git commit.

### Field and state translation

1. Freeze the exact v0.1 Git commit/tree, event root/count, legacy snapshot,
   proposal, actor-registry, artifact-registry, and canonical-store roots.
2. Freeze the full legacy identity preimage, including current Frontier ID and
   reducer seed fields, rather than relying on the truncated ID alone.
3. Preserve the exact current dependency list in sorted canonical form.
4. Build the v1 profile from current human metadata. Empty or ambiguous name,
   summary, or scope requires an explicit user edit in the preview; Vela never
   invents scientific scope.
5. Move allowlisted runtime sections from `.vela/config.toml` to
   `.vela/settings.toml` without loss. Unknown sections block migration.
6. Remove inert/fixed v0.1 manifest fields only after verifying their exact
   historical values and recording them in the plan. Divergence is reported,
   not silently normalized.
7. Append one valid `frontier.repository_bound` event after protected approval.
8. Render v1 `frontier.yaml`, `.vela/settings.toml`, canonical boundary event,
   `frontier.json`, lock, proof packet, and target index from the exact
   candidate. Remove active `.vela/config.toml` only after settings parity.
9. Re-run strict verification and clean-clone replay before committing.

The protected transaction touches only those named protocol/profile/settings
and derived paths. README, SCOPE, VELA, AGENTS, CLAUDE, host templates, and
domain-directory cleanup are reported as a separate Git-only follow-up. They
must not enlarge the signed administration delta.

### Root and byte semantics

Migration preserves byte-for-byte:

- every pre-boundary canonical event;
- every proposal and Receipt;
- actor registrations and signed policy pairs;
- accepted finding and reducer-owned remnant bytes;
- retained artifacts and evidence; and
- every historical signature.

These roots remain unchanged:

- proposal root;
- actor-registry root;
- artifact-registry root; and
- the roots of every retained historical object store.

These values intentionally change or are added:

- event-log root and count, because one non-scientific boundary event appends;
- Git commit/tree;
- profile and identity roots;
- dependency root;
- scientific-state root v2;
- lock and proof roots; and
- target-index root.

The boundary records the exact old event and snapshot roots so old replay and
historical references remain auditable. Migration never fabricates or reorders
`frontier.created` in an existing history.

Historical Decision Plans, pending proposals, withdrawal capabilities,
Receipt producer-context roots, active or expired policies, actor-registration
boundaries, and policy-admission certificates continue to verify against the
root schema they actually signed. Migration does not substitute
`scientific_state_root_v2` for an old `snapshot_hash`. A still-actionable
object receives an explicit compatibility resolution from its historical root
to the exact boundary anchor; if that proof is absent, it is reported stale
rather than silently refreshed. The current Erdős regression vector contains
13 pending proposals and must prove that each remains byte-identical and has
the same standing after migration.

### Migration order

Migrate the maintained Frontiers independently:

1. Formal Conjectures, after classifying its legacy proof-path artifact;
2. Quantum Codes;
3. Sidon, preserving its public artifact blobs; and
4. Erdős last. Freeze its existing Formal dependency before Formal migrates;
   preserve that historical content pin through Erdős migration. Retargeting
   Erdős to Formal v1 is a later, separately signed dependency update.

Divergent parent copies and immutable fixtures remain v0.1 unless selected by
their own owner. Histories are never unioned or spliced.

## Strict and non-strict behavior

For v1:

- wrong or unknown profile fields are strict blockers;
- a missing, duplicate, invalid, unverifiable, or non-ancestor boundary is a
  strict blocker;
- identity, dependency, anchor Git, event, legacy snapshot, proposal,
  registry, artifact, or canonical-store mismatch is a strict blocker;
- an unsigned dependency substitution or stale target index grants no work or
  landing path;
- non-strict inspection reports the same problems but never treats an invalid
  boundary as an exemption;
- materialize validates but never creates or repairs a boundary event;
- work, land, policy routing, protected decisions, and migration refuse an
  invalid boundary before writing; and
- missing optional Frontier Card or host files remain `doctor --all`
  advisories, not scientific blockers.

Anchor eligibility is exact Git-tree membership, never an event or object
timestamp. Every anchored event and retained object must still exist with the
same canonical preimage or byte digest. Anything absent from the anchored tree
is post-boundary even if its embedded timestamp is backdated. A shallow clone
blocks boundary verification; a complete Git bundle containing the anchor is
sufficient.

Vela continues to inspect, replay, reproduce, check, and migrate v0.1
repositories. In Vela 0.914, every canonical write to v0.1 fails with
`frontier_profile_upgrade_required`; compatibility is not an indefinite second
writer. Old fixtures remain replayable without migration. Older binaries
reject `frontier.repository_bound` and the v1 profile as an intentional
protocol-version boundary.

## Adversarial cases

Conformance must cover:

- backdated or forked boundary events;
- a shallow checkout missing the anchored history;
- wrong Git commit/tree or non-ancestor anchor;
- changed, deleted, or reordered old events;
- legacy snapshot, proposal, actor, artifact, or canonical-store mismatch;
- duplicate boundary events and previous-root chain breaks;
- conflicting signed children from one previous boundary;
- invalid signer, signature, registry state, or protected confirmation;
- profile changes hidden behind YAML aliases, duplicate keys, unknown fields,
  non-canonical scalars, or Unicode normalization traps;
- identity substitution using a copied `.vela/` directory;
- actor-registry replacement followed by a self-signed boundary;
- a backdated boundary after an anchored revocation;
- dependency substitution using a mutable tag, short digest, or changed
  locator;
- display metadata incorrectly changing scientific-state root;
- runtime settings erased by work, land, materialize, or recovery;
- a stale target index offered as current work;
- a generated file hand edit that happens to parse;
- a symlink, submodule, path traversal, case collision, or Unicode-normalized
  path collision in retained objects or target packets;
- a Receipt-bound legacy path removed or redirected to different bytes; and
- migration from a dirty, forked, recovery-incomplete, or root-drifting
  checkout.

Every invalid case fails before authority, profile, canonical history, or Git
mutation.

## Exact conformance tests

Add focused tests named:

```text
frontier_profile_v1_closed_nested_schema
frontier_profile_v1_canonical_root_ignores_yaml_formatting
frontier_profile_v1_rejects_duplicate_keys_aliases_and_tags
frontier_profile_v1_requires_nfc_and_rejects_control_text
profile_frontier_id_is_assertion_not_identity_source
scientific_state_root_v2_excludes_display_operational_metadata
profile_edit_changes_only_profile_root
dependency_substitution_requires_bound_update
dependency_bytes_rederive_dependency_root
dependency_locator_requires_exact_equivalence
legacy_identity_root_preimage_is_nonrecursive
new_frontier_created_derives_full_identity_root
frontier_repository_bound_exact_anchor
frontier_repository_bound_wrong_git_tree_event_snapshot_registry_artifact_fails
frontier_repository_bound_duplicate_and_chain_break_fail
frontier_repository_bound_recomputes_identity_and_dependency_roots
frontier_repository_bound_rejects_identity_change
frontier_created_has_no_git_self_reference
boundary_signer_matches_anchored_active_actor
malicious_registry_replacement_cannot_self_authorize_boundary
backdated_boundary_cannot_bypass_anchored_revocation
tofu_boundary_is_not_reported_trusted
anchor_membership_ignores_timestamps
retained_object_manifest_binds_signatures_receipts_and_legacy_artifacts
canonical_path_symlink_submodule_case_collision_fails
legacy_without_genesis_migrates_via_exact_boundary
materialize_never_repairs_invalid_repository_boundary
frontier_settings_survive_work_land_materialize_and_recovery
legacy_config_runtime_keys_migrate_without_loss
init_writes_scope_and_frontier_created
old_binary_rejects_repo_v1_boundary
v01_canonical_write_requires_migration
target_index_v2_stale_fails_closed
target_index_v2_exact_packet_remains_available
target_index_source_revision_is_not_self_referential
profile_only_edit_does_not_stale_target_offer
migration_preserves_all_preboundary_canonical_bytes
migration_preview_reason_signer_candidate_profile_are_root_bound
migration_cancellation_zero_writes_and_crash_recovers
pending_proposals_and_policy_history_survive_root_v2_without_substitution
strict_nonstrict_invalid_boundary_no_exemption
old_frontier_repo_v01_replays_unchanged
```

The canonical cross-implementation root for the Profile A fixture in
`crates/vela-protocol/tests/frontier_profile_v1.rs` is:

```text
sha256:26f4cd0e61408c17b7e9f979ea8dca809a6c5ec0cbd5f22e6114ffdf68e1f1aa
```

Focused commands:

```bash
cargo test -p vela-protocol frontier_profile_v1
cargo test -p vela-protocol frontier_repository_bound
cargo test -p vela-cli init_minimal
cargo test -p vela-cli migration_frontier_repo_v1
cargo test -p vela-edge target_index_v2
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

Parent integration runs core and frontier suites. External Lean, Diderot,
live-network, Web visual, and unrelated release suites remain excluded unless a
named migrated Frontier directly selects its frozen verifier.

## Alternatives rejected

### Keep v0.1 and add only `manifest_root`

Rejected. An attacker can rewrite the human manifest and regenerate the lock.
It does not protect legacy identity or exact scientific dependencies.

### Keep current snapshot semantics

Rejected. Display name, compiler labels, timestamps, and counters must not
change the scientific-state root.

### Fabricate a genesis event at the start of old histories

Rejected. It would rewrite or reorder canonical history and invalidate the
event-log root. Legacy migration appends one exact boundary instead.

### Use an unsigned anchor file

Rejected for maintained legacy Frontiers. The file and regenerable lock can be
rewritten together. A signed event reuses the existing immutable log and
signature machinery.

### Require signatures for every profile edit

Rejected. Human-facing name, summary, scope, maintainer, and license edits are
Git-profile changes, not scientific authority. Only stable identity and exact
dependency changes require a boundary update.

### Make raw YAML bytes the profile root

Rejected. Formatting and comments would create identity churn without a
semantic change.

### Create one repository per target or one monorepo for all science

Rejected. Repository boundaries follow authority and correction. Derived
indexes, sharding, and portfolios solve different problems without joining or
fragmenting custody.

### Put GitHub, Vercel, Hugging Face, Entire, or workbench adapters in v1

Rejected. They remain replaceable edges, gain no authority, and require their
own named user need and removal path.

## Consequences

The migration is larger than a YAML cleanup, but it repairs a real trust and
usability defect once. Repository metadata becomes easy to edit without
changing scientific state. Identity and dependencies become exact and
tamper-evident. Runtime settings survive normal operations. Work offers cannot
silently come from stale state. Old scientific history remains byte-identical,
and Git, Vela, domain tools, and readers retain one clear responsibility each.
