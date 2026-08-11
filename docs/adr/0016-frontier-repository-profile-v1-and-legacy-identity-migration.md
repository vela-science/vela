# ADR 0016: Frontier Repository Profile v1 and legacy identity migration

- Status: Accepted — 2026-07-23
- Target-surface disposition: any Vela Target Index or `next`/`start` language
  below is historical; commit `719cbc77` retired that core surface on 2026-08-10.
- Operational settings portion superseded by accepted ADR 0031; retained as
  release history
- Protocol effect: one signed non-scientific boundary event, versioned state
  root, closed repository profile, and derived-root contracts
- First released in: Vela `v0.914.0`
- Evidence gate: deterministic `full` CI union recorded 41 PASS, 1 explicit
  external-custody reconciliation WARN, 7 intentionally excluded external/live
  SKIP, and 0 FAIL among 49 registered gates
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
```

Implementations canonicalize the entries, sort by `(frontier_id,
identity_root)`, reject duplicates, and compute:

```text
dependency_root = sha256(canonical_json(exact_dependency_list))
```

Retrieval locators are deliberately absent from the closed entry and therefore
from `dependency_root`. A Git remote, bundle path, object-store URL, or other
retrieval hint belongs in ordinary non-authoritative repository configuration
or a consumer lock. Changing that hint cannot change scientific or dependency
identity. When a consumer resolves a hint, the obtained Git objects and Vela
state must match every full root in the entry; otherwise the dependency is
unavailable or invalid and grants no standing. Short roots, mutable tags, and
missing or mismatched Git objects are invalid. A Frontier with no exact
dependencies uses the canonical root of the empty list.

The current Erdős dependency on the historical Formal Conjectures snapshot is
preserved as an exact historical commitment. Its retired `vela.hub` source and
old locator remain only in the anchored v0.1 bytes. Migration translates them
to a v1 Git identity and root record, plus a separate retrieval hint if one is
still useful, only after proving both descriptions resolve to the same retained
content. The hint is not copied into the dependency security preimage;
ambiguity blocks migration. A later retarget to Formal v1 requires a new
signed boundary event.

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
previous_identity_event_root | null
legacy_identity_preimage_root | null
administrator_actor_id
administrator_public_key
administrator_algorithm: ed25519
trust_mode: tofu | genesis | previous_boundary
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
declared repository object format. `previous_identity_event_root` is the full
SHA-256 content root of the exact preceding identity event: the canonical
`frontier.created` event for the first dependency update of a new v1 Frontier,
or the preceding valid `frontier.repository_bound` event thereafter. It is
never a truncated event ID. The legacy anchor is always the pre-boundary Git
commit and event prefix; no field may refer to the commit that will contain the
event itself.

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
the canonical event, proposal, finding, artifact, and actor stores; every
content-addressed `.vela/artifact-blobs/` object; every immutable
`.vela/policies/<vap_>.json` and `<vap_>.sig.json` pair; every retained
Receipt; and every tracked file reached by an exact Receipt or artifact
locator. Mutable policy-head pointers `active.json` and `active.sig.json` are
deliberately excluded and remain governed by their event and signature
continuity rules. Paths must be normalized UTF-8 repository paths. Symlinks,
submodules, traversal, platform case-fold collisions, Unicode normalization
collisions, duplicate paths, and missing locator targets fail closed. This
includes legacy Sidon artifact blobs and Formal's Receipt-bound Lean file
without making their old directories the v1 layout for new writes.

The ordinary canonical event signature binds the payload and signer. The event
targets the exact Frontier, uses null before/after scientific finding hashes,
and is reducer-neutral except for repository identity and dependency boundary
state. `dependency_root` is recomputed from the closed payload before the
signature can grant validity. A `temporalize_existing` identity root is
recomputed from the legacy preimage and anchor fields carried by that payload.
An `update_dependencies` identity root is verified by continuity with the
previous identity event, because the update does not repeat the original
identity preimage. For `trust_mode: genesis`, implementations rederive the
Frontier ID and identity root from the exact `frontier.created` event. For
`trust_mode: previous_boundary`, they preserve the identity and administrator
fields from the preceding valid boundary.

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
  `tofu`, its previous identity event is null, and consumers must pin the
  resulting boundary root or release tag out of band.
- The first `update_dependencies` event for a new v1 Frontier has trust mode
  `genesis` and sets `previous_identity_event_root` to the full content root of
  its exact `frontier.created` event. The verifier rederives the Frontier ID and
  `identity_root` from that genesis event; an arbitrary supplied origin
  commitment cannot establish identity.
- Every later `update_dependencies` event has trust mode `previous_boundary`,
  sets `previous_identity_event_root` to the full content root of the preceding
  valid boundary, preserves `identity_root`, legacy origin (if any),
  administrator actor, public key, and algorithm, and strictly increases
  `anchor_event_count`. The new anchor must contain that preceding boundary in
  its exact event prefix. A Frontier identity is immutable; changing it creates
  a different Frontier. Display-only profile edits do not require an event. V1
  deliberately has no administrator-key rotation; that requires a separately
  reviewed recovery contract.

Vela `0.914.0` validates this continuation shape for deterministic replay but
does not ship porcelain that creates a later `previous_boundary` update. Its
maintained writers are limited to the first protected genesis boundary and one
legacy temporalization boundary. The dependency set established by that first
boundary is therefore immutable through ordinary `0.914.0` commands. A later
dependency-update writer needs its own two-phase protected plan, transaction
and adversarial conformance evidence; recognizing the wire shape is not a
license for callers to hand-author it.

A new v1 Frontier does not emit a self-referential boundary. Its first
`frontier.created` event establishes identity and the empty dependency root.
The event preimage never contains a Git commit that contains itself. The first
later dependency update anchors the exact pre-update Git commit, tree, event
log, and state roots, chains directly to the genesis content root with
`trust_mode: genesis`, then appends one `frontier.repository_bound` event.

The first administrator boundary of any Frontier requires a registered human
identity using the existing protected signer. Operationally the command
requires a protected human backend; protocol verification relies on the exact
signature, anchored registry record, boundary chain, and the consumer's
out-of-band pin rather than pretending either an open actor registry or an
unsigned structural genesis created a root of administrator trust. The command
may prepare and invoke the exact request, but a model cannot approve the OS
card or read the key. This is a rare repository-administration ceremony, not
an everyday scientific decision. The prompt names the Frontier, profile
version, dependency summary, anchor roots, reason, administrator key, trust
mode, and boundary plan root.

The first legacy migration boundary is explicit TOFU unless a future protocol
names and verifies a stronger prior trust source. A first native
`trust_mode: genesis` boundary proves exact continuity with the structural
genesis but does not authenticate the administrator: the v1 genesis is
unsigned and deliberately key-free. Both cases therefore require an
out-of-band pin to the first administrator boundary. Neither can manufacture
historical trust, and readers label the resulting assurance
`integrity_verified_pinned`, not universally `trusted`.

`observed_profile_root` proves which human-facing profile was presented at
the ceremony. It is historical audit data. Strict verification does not
require the current editable profile to equal it.

The actor registry is byte-stable throughout a Profile v1 boundary chain in
this ADR. The historical `key.revoke` event remains replayable but audit-only;
it is not a hidden registry mutation and no retired Hub table supplies missing
authority. Key rotation, registry extension, and administrator recovery need a
separate repository-local governance contract. Until then, a changed registry
fails closed rather than being accepted through a later self-signed boundary.

#### Consumer trust anchor for administrator boundaries

The first boundary cannot establish its own external administrator trust
merely by including a public key in repository bytes. This is true even when
the boundary chains to a native v1 genesis: two forks of the same unsigned
genesis could otherwise bootstrap different administrator keys while retaining
the same Frontier identity. Once any administrator boundary exists, each
consumer therefore carries one closed, public, local pin at:

```text
~/.vela/trust/frontiers/<frontier_id>.json
```

```json
{
  "schema": "vela.repository-trust-anchor.v1",
  "frontier_id": "vfr_...",
  "identity_root": "sha256:...",
  "boundary_content_root": "sha256:...",
  "administrator_actor_id": "reviewer:...",
  "administrator_public_key": "<64 lowercase hex>"
}
```

This is a non-protocol consumer trust record, not a Frontier object, actor
registration, policy, signature, or scientific-state input. It contains no
secret. The file is a regular non-symlink file, installed atomically below a
user-owned `0700` directory with mode `0600` where the platform supports Unix
modes. Vela accepts no environment, repository-profile, settings, lockfile,
remote-URL, or mutable-tag override for it and never creates it from a
repository on first use.

The protected first-boundary plan binds the exact candidate record and its
canonical root. After the signed boundary transaction crosses its durable
commit marker, Vela installs that already-confirmed record and reads it back;
it does not reconstruct the pin from newly mutable repository fields. Failure
to install the local pin does not roll back committed canonical history. It
reports the exact recovery action and leaves later canonical writes blocked.

A second consumer selects the independently reviewed first-boundary content
root with a key-free preview and an exact, time-bound confirmation:

```bash
vela frontier trust pin <frontier> \
  --boundary-root <sha256:...> --json
vela frontier trust pin <frontier> \
  --boundary-root <sha256:...> \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

The preview locates that full root in the exact current chain, derives the
closed public trust record from the verified event and identity, and binds it
with the current repository roots and observation time. Apply rederives those
facts rather than accepting an asserted anchor file, then installs the exact
record atomically. Replacing a different existing pin is a separately reviewed
future action; there is no `--force`, automatic trust-on-first-use, mutable
tag, or repository-local fallback. A genesis-rooted Profile v1 repository
with no administrator boundary needs no pin, but no administrator authority is
inferred in that state.

Hosted CI may transport the independently selected full boundary root through
protected repository or organization configuration and invoke this same exact
two-phase command in an ephemeral account. That is an explicit consumer
configuration input, not a Vela environment fallback: the proposed checkout
cannot select it, Vela still derives the public key and complete record from
the uniquely matching signed event, and a missing or wrong root fails closed.
The released action never reads a root from Frontier bytes and never accepts an
arbitrary trust-anchor path.

Boundary validity is an event-set property, not a timestamp heuristic. Before
replay may accept repository identity or dependency state, implementations
validate every known `frontier.repository_bound` event in the supplied event
set. Each event must have the fixed core, closed payload, canonical ID, and a
valid ordinary signature under its named administrator key. The identity-event
graph must contain exactly one linear chain for the Frontier: a legacy TOFU
root or the exact `frontier.created` root, followed by zero or more valid
updates. Missing parents, duplicate content roots, forks, cycles, mode/trust
mismatches, changed immutable fields, and a non-increasing boundary anchor
count all fail closed. Event timestamps never create membership or repair a
broken chain.

This pure event-set validation does not pretend to prove facts absent from the
event bytes. A repository-context verifier must additionally prove Git
ancestry and object availability, exact anchored roots and retained bytes, and
that the named administrator record was active at the anchor. Both layers are
required before a boundary grants continuity or enables a canonical write.
Merely recognizing the event kind is never an unconditional reducer no-op.

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
`findings_root` commits to a closed scientific finding projection. It includes
assertion, evidence, conditions, confidence, provenance, flags, annotations,
attachments, version identity, and creation/update identity, while excluding
mutable graph `links` and the read-side `access_tier`. Those excluded fields
change navigation or disclosure, not what the finding scientifically says.

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
The work TTL is an integer from 1 through 31,536,000 seconds. The same upper
bound applies to non-release `attempt.claimed` events; zero remains only the
compare-and-swap release form. Writers and readers use checked timestamp
arithmetic, so an invalid historical value is never a panic or a perpetually
live lease.
Unknown keys fail settings validation but do not change replayed state.
Precedence is safety-aware: command flags and allowlisted environment
overrides are first; an explicit user preference then beats an ordinary
Frontier convention; the Frontier convention beats the built-in default.
The exception is a narrowing-only value: checked-in
`publish.git_push = "off"` may override a wider user preference but can never
turn publishing on. This prevents a cloned Frontier from changing an explicit
user `mcp.profile = "read-only"` to `draft`. `.vela/config.toml` is absent from
an active v1 repository after migration.

The first release does not claim native Windows support for the two
exact-preimage repository-file writers: `.vela/settings.toml` mutation and
Target Index v2 sealing. Native Windows remains read/check/reproduce capable,
but these writes fail before temporary creation. The release may lift this
restriction only after a Windows-native implementation proves reparse-safe
handle binding, relative temporary creation, atomic no-clobber create, atomic
exchange with displaced-preimage verification and rollback, installed-byte
readback, directory durability, and deterministic hostile race/crash tests.
Microsoft's documented
[`ReplaceFileW`](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew)
surface is path-based, while the documented Win32
[`FILE_RENAME_INFO`](https://learn.microsoft.com/windows/win32/api/winbase/ns-winbase-file_rename_info)
contract requires a null `RootDirectory` and provides replacement rather than
exchange. Combining them is not an acceptable approximation. WSL2 is
supported for these operations only when the checkout resides on its Linux
filesystem.

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

- Sidon's tracked `.vela/artifact-blobs/` and Erdős's 32 restored
  content-addressed blobs remain public retained artifacts until an exact
  digest-locator migration preserves every reference.
- Formal Conjectures' Receipt-bound `proof/erdos505-test-dim-one-proof.lean`
  remains resolvable at its historical path. New formal work uses
  `formal/lean/`; retiring the old locator requires a separately verified
  digest-based compatibility rule and never rewrites the Receipt.

Proposal files are included in every retained-object manifest. Their logical
content and content-derived ID are immutable, while terminal status and
decision fields are a checked projection of explicit `review.*` or
`proposal.withdrawn` events. The central write barrier therefore verifies
proposal identity, uniqueness, decision parity, and withdrawal authorization
before granting a write. It does not compare an entire proposal file
byte-for-byte with an old boundary anchor, because that would incorrectly
forbid a legitimate event-derived decision projection. Historical accepts
that predate explicit `review.accepted` events retain their released display
metadata; their terminal standing and applied domain-event pointer remain
checked, while current explicit review events bind the complete decision
projection.

Some exact legacy proposal files also predate the current logical-ID preimage.
A verified pinned temporalization boundary may retain that finite conflict set
as `anchored_immutable_unauthenticated` compatibility debt. The conflicts grant
no proposal authentication or authority. Every conflicted proposal is frozen
byte-for-byte at the anchor; adding a conflict, changing one of those bytes, or
losing the exact Git boundary or consumer pin fails closed. Native Profile v1
repositories receive no such compatibility classification.

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
canonical preimage root establishes stable structural identity; it grants no
external trust, administrator authenticity, human signing, or claim-acceptance
authority.

Historical `frontier.created` payloads remain valid replay input, but they do
not establish Profile v1 identity and cannot serve as the parent of a
`trust_mode: genesis` repository boundary. A legacy repository that happens to
contain the old event shape still uses the protected `temporalize_existing`
path. This prevents an old, open structural event from bypassing the explicit
legacy trust boundary.

Initialization still creates no domain directory matrix, target catalogue,
proof packet, CI adapter, MCP file, editor configuration, host registration,
or executable hook. Optional Frontier Kits remain non-protocol conventions
applied afterward.

### 9. Target index v2

`vela.target-index.v2` keeps the useful v1 work interface but makes a work
offer an exact, derived projection rather than a best-effort hint. Its wire
shape is the following closed JSON object; every nested object is closed too:

```json
{
  "schema": "vela.target-index.v2",
  "frontier_id": "vfr_...",
  "source": {
    "git_object_format": "sha1",
    "git_commit": "<full lowercase Git object ID>",
    "git_tree": "<full lowercase Git object ID>"
  },
  "inputs": {
    "schema": "vela.target-index-input-manifest.v1",
    "input_root": "sha256:<64 lowercase hex>",
    "entries": [
      {
        "path": "domain/source.json",
        "git_mode": "100644",
        "size": 123,
        "sha256": "<64 lowercase hex>"
      }
    ]
  },
  "roots": {
    "event_log_root": "sha256:<64 lowercase hex>",
    "event_count": 42,
    "nonlease_event_log_root": "sha256:<64 lowercase hex>",
    "scientific_state_root": "sha256:<64 lowercase hex>",
    "proposal_root": "sha256:<64 lowercase hex>",
    "identity_root": "sha256:<64 lowercase hex>",
    "dependency_root": "sha256:<64 lowercase hex>",
    "observed_profile_root": "sha256:<64 lowercase hex>"
  },
  "claim_boundary": {
    "derived": true,
    "authoritative": false,
    "deletable": true
  },
  "generated_by": {
    "program": "vela",
    "version": "0.914.0"
  },
  "targets": [
    {
      "id": "erdos:1056",
      "title": "Erdos 1056",
      "why": "Why this bounded target is ranked now.",
      "state": "open",
      "rank": 17619056,
      "objective": "Produce one decision-relevant artifact.",
      "labels": ["erdos", "upstream-open"],
      "packet": {
        "schema": "erdos-frontier.problem-work.v1",
        "path": "site/problems/1056.json",
        "size": 456,
        "sha256": "sha256:<64 lowercase hex>"
      }
    }
  ],
  "index_root": "sha256:<64 lowercase hex>"
}
```

`git_object_format` is exactly `sha1` or `sha256`; commit and tree lengths must
match it. All sizes, counts, and ranks are JSON integers in
`0..=9007199254740991`. Target `state` is exactly `open`, `paused`, `blocked`,
`done`, or `retired`. Text follows the Profile v1 NFC and control-character
rules. Labels are sorted, unique, non-empty strings. Targets are sorted by
ascending `(rank, id)` and target IDs are unique. Alternate ordering is
invalid rather than silently normalized.

The existing bounded external-target ID grammar remains the one grammar for
candidate IDs and leases. Canonical index and candidate JSON are at most 4
MiB, an index contains at most 16,384 targets, a target contains at most 64
labels, and a packet is at most 1 MiB. ID, title, reason, objective, label,
packet-schema, and packet-path byte limits remain respectively 256, 512,
2,048, 4,096, 128, 256, and 1,024. Packet bytes must be one JSON object whose
top-level `schema` exactly equals `packet.schema`. These bounds are protocol
conformance limits for v2, not implementation suggestions.

`claim_boundary` has the three fixed values shown above. The broader,
descriptive v1 variants are not carried forward. `generated_by.program` is
fixed to `vela`; `version` is the semver of the Vela binary that sealed the
index. A later Vela version does not by itself stale a valid v2 index. The
tracked `targets.json` bytes are exactly the canonical JSON serialization of
this object, with no BOM, insignificant whitespace, or trailing newline. A
parseable hand-edited representation is `target_index_schema_invalid`.

#### Input and index roots

The input manifest is a complete declaration by the Frontier-owned candidate
generator of every tracked file whose bytes influenced target membership,
state, rank, description, labels, or packet bytes. It is not a claim by Vela
that an arbitrary domain generator disclosed its complete read set. Vela does
prove that every declared byte is exact.

Entries use the same closed `{path, git_mode, size, sha256}` record and
portable-path rules as `vela.retained-object-manifest.v1`. Only Git modes
`100644` and `100755` are valid. Entries are sorted by validated path in UTF-8
byte order. Duplicate paths, portable case-fold or Unicode collisions,
symlinks, submodules, backslashes, absolute paths, `.` or `..` components, and
non-NFC or control-bearing paths are invalid. `targets.json`, the candidate
file, and every packet output path are forbidden as input paths, which removes
the index and its outputs from their own source preimage.

The two derived roots are exactly:

```text
input_root = sha256(canonical_json({
  "schema": "vela.target-index-input-manifest.v1",
  "entries": <entries>
}))

index_root = sha256(canonical_json(<complete vela.target-index.v2 object
                                    with only index_root omitted>))
```

The `sha256` inside each input entry is a bare lowercase digest, matching the
retained-object manifest. Every named root and packet digest uses the
`sha256:` prefix. Implementations reject an incorrect supplied root; they do
not repair it in memory.

The source commit is the exact revision from which the candidate generator
read its declared inputs. Vela resolves every input entry from that commit's
tree, not from working-tree bytes. The source commit must exist locally, be an
ancestor of the inspected `HEAD`, and resolve to the exact declared tree. The
source tree must not contain the exact sealed `targets.json` blob; the later
commit that first stores the sealed index therefore cannot be its own source.
Shallow history or a missing object is unavailable, not trusted by assertion.

At offer and lease time, `targets.json` and every referenced packet must be
tracked regular files in `HEAD`, and their working-tree bytes must equal their
`HEAD` blobs. An untracked, staged-only, dirty, symlinked, or submodule-backed
index or packet grants no work. Unrelated worktree policy remains the concern
of the calling command; it cannot relax these exact-file checks. `next`
validates every open entry's packet before counting it available; `work`
revalidates its selected packet at the transaction edge.

`roots.event_log_root` and `event_count` bind the exact event prefix observed
at seal time. The current log must still contain that byte-identical prefix.
`nonlease_event_log_root` is the existing event-set commitment that excludes
only valid `attempt.claimed` coordination events. A later lease may extend the
full event log without staling every other offer, but any changed prefix or
any later event kind other than `attempt.claimed` changes the non-lease root
and stales the index. The remaining roots are rederived from the validated
Repository Profile v1 state, never trusted from `frontier.yaml`.

A later documentation or profile-only commit remains fresh when the source
commit is still an ancestor, all declared source entries and packet bytes
match, and every security-bearing root matches. `observed_profile_root` is
reported as audit context; its drift alone is not stale. Scientific state,
non-lease events, dependencies, proposals, packets, source inputs, or identity
drift is stale.

#### Candidate ownership and Vela sealing

Domain tools own target semantics and packet schemas. They emit a closed
`vela.target-index-candidate.v1` at the ignored conventional path
`.vela/tmp/target-index-candidate.json` unless the user supplies another path:

```json
{
  "schema": "vela.target-index-candidate.v1",
  "frontier_id": "vfr_...",
  "source": {
    "git_commit": "<full lowercase Git object ID>",
    "input_paths": ["domain/source.json"]
  },
  "targets": [
    {
      "id": "erdos:1056",
      "title": "Erdos 1056",
      "why": "Why this bounded target is ranked now.",
      "state": "open",
      "rank": 17619056,
      "objective": "Produce one decision-relevant artifact.",
      "labels": ["erdos", "upstream-open"],
      "packet": {
        "schema": "erdos-frontier.problem-work.v1",
        "path": "site/problems/1056.json"
      }
    }
  ]
}
```

The candidate `source` mapping and every target/packet mapping are closed.
`input_paths` are sorted, unique, validated by the input-path rules above, and
may be empty for a genuinely self-contained candidate. Candidate targets use
the same ordering, grammar, text limits, states, ranks, and labels as the
sealed index; only seal-owned Git format/tree, input entries/root, repository
roots, packet size/digest, `generated_by`, and `index_root` are absent.

Vela owns only the seal. It derives the Git format and tree, materializes the
input manifest from Git, rederives the repository roots, validates the closed
candidate, reads each candidate packet once, fills packet size and digest,
writes its own version, and computes `index_root`. It never invents, reranks,
or silently repins candidate semantics.

The interface is two phase:

```bash
vela target-index repair <frontier> --json
vela target-index seal <frontier> --candidate <candidate.json> --check --json
vela target-index seal <frontier> --candidate <candidate.json> --apply --json
vela target-index inspect <frontier> [<target-id>] --json
```

`repair` is read-only. It emits `vela.target-index-repair.v1` with every stale
code, changed declared path, the conventional candidate path, and the exact
`seal --check` argv; it never runs a domain generator or updates roots.
`seal --check` returns the complete proposed v2 bytes, root, read set, and
touched path without writing. `seal --apply` atomically writes only
`targets.json`, refuses dirt outside the candidate, declared packet outputs,
and existing target index, and neither stages nor commits. Consequently
`next` and `work` remain unavailable until the sealed index and packets are
committed and match `HEAD` exactly.

#### Inspection, offer, and retained task binding

`vela next` and `vela work` use one validated index assessment and fail closed
on stale or invalid entries. `work` revalidates the index, packet, repository
roots, and transaction read set inside the same recovery barrier immediately
before appending a lease. Failure writes no session, event, journal marker, or
Git commit. There is no `--force`, non-strict, or legacy-v1 bypass.

`vela target-index inspect` may show a stale entry by exact full target ID. It
labels the entry stale and unactionable and returns its exact codes. It returns
packet content only when the bound packet still matches; otherwise it returns
metadata and the mismatch. Inspection never turns the entry into an offer or
lease. V1 indexes remain readable for historical inspection but grant no v1
Profile work.

For a parseable v2 index, JSON availability has exact integer fields:

```text
configured = number of entries whose state is open
stale      = open entries excluded by a global or entry-local freshness error
leased     = fresh open entries excluded by a live lease
available  = configured - stale - leased
returned   = offers actually returned after the caller's limit
```

The contract also contains `repair_command` exactly
`vela target-index repair <frontier> --json`. A structurally unreadable index
returns a closed error contract and no offers; it does not guess counts.

One successful claim creates this closed `vela.target-task-binding.v1`:

```json
{
  "schema": "vela.target-task-binding.v1",
  "frontier_id": "vfr_...",
  "target_id": "erdos:1056",
  "target_index_root": "sha256:<64 lowercase hex>",
  "source": {
    "git_object_format": "sha1",
    "git_commit": "<full lowercase Git object ID>",
    "git_tree": "<full lowercase Git object ID>"
  },
  "input_root": "sha256:<64 lowercase hex>",
  "packet": {
    "schema": "erdos-frontier.problem-work.v1",
    "path": "site/problems/1056.json",
    "size": 456,
    "sha256": "sha256:<64 lowercase hex>"
  },
  "index_roots": {
    "event_log_root": "sha256:<64 lowercase hex>",
    "event_count": 42,
    "nonlease_event_log_root": "sha256:<64 lowercase hex>",
    "scientific_state_root": "sha256:<64 lowercase hex>",
    "proposal_root": "sha256:<64 lowercase hex>",
    "identity_root": "sha256:<64 lowercase hex>",
    "dependency_root": "sha256:<64 lowercase hex>"
  },
  "claim_read_set": {
    "event_log_root": "sha256:<64 lowercase hex>",
    "event_count": 45,
    "git_object_format": "sha1",
    "git_commit": "<full lowercase Git object ID>",
    "git_tree": "<full lowercase Git object ID>"
  },
  "binding_root": "sha256:<64 lowercase hex>"
}
```

`binding_root` is SHA-256 of canonical JSON for the complete binding with only
`binding_root` omitted. `claim_read_set` is the exact `HEAD` and event prefix
immediately before the lease append; it may be later than the index's sealed
prefix only by valid leases. The private work session carries the exact
binding and root. Landing copies the same record byte-for-byte into the
Receipt v1 environment extension `vela:target_task_binding`, so deleting the
private session cannot erase which offer and packet produced the Receipt. The
Receipt whole-body binding covers the extension. A later valid index change
does not rewrite this historical binding or automatically invalidate evidence
already produced; landing still revalidates the bound bytes and reports
current drift.

The closed stale/error codes are:

```text
target_index_schema_invalid
target_index_frontier_mismatch
target_index_source_unavailable
target_index_source_not_ancestor
target_index_source_tree_mismatch
target_index_source_self_reference
target_index_event_root_mismatch
target_index_state_root_mismatch
target_index_proposal_root_mismatch
target_index_identity_root_mismatch
target_index_dependency_root_mismatch
target_index_input_root_mismatch
target_index_index_root_mismatch
target_index_packet_mismatch
target_index_output_not_tracked
target_index_duplicate_target
target_index_invalid_path
target_index_invalid_target
```

Schema, frontier, source, index/root, input-manifest, ordering, and duplicate
failures are global and mark every parseable open entry stale. Packet, tracked
output, path, and target failures are entry-local when they identify exactly
one target; otherwise they are global. Codes are sorted and deduplicated in
JSON. No implementation may downgrade one of these failures to an advisory.

Graph position or structural advice never replaces canonical work ranking.
Deleting the index removes only catalogue convenience. It cannot change
scientific state or authority.

## Migration

Add a two-phase protected migration:

```bash
vela migrate <frontier> --to frontier-repo-v1 --check \
  --profile <candidate-profile.yaml> \
  --target-candidate <target-index-candidate.json> \
  [--dependency-input <dependency-migration.json>] \
  --as <human-actor> --reason <text> --json
vela migrate <frontier> --to frontier-repo-v1 --apply \
  --profile <candidate-profile.yaml> \
  --target-candidate <target-index-candidate.json> \
  [--dependency-input <dependency-migration.json>] \
  --as <human-actor> --reason <text> \
  --confirm-root <sha256:...> --confirm-at <RFC3339> --json
```

Without confirmation, the command is key-free and returns the exact migration
plan, boundary payload, before/after root families, touched files, signer,
reason, trust mode, and plan root. The candidate profile is read-only input and
lets the user resolve missing legacy description or scope without dirtying the
source checkout. The required target candidate is likewise external and is
sealed as Target Index v2 inside the same transaction; migration never
requires an otherwise-illegal pre-migration write. A Frontier with legacy
dependencies additionally supplies one closed `vela.frontier-dependency-
migration.v1` input. Each entry reproduces the full legacy descriptor, names
one exact dependency repository and boundary, supplies its independently
reviewed consumer trust anchor, and binds the resulting full Profile v1
dependency record. Mutable locators remain hints and never enter the exact
dependency security preimage. With a matching non-expired confirmation, the command
revalidates the binary, Git ancestry, all roots, registry, signer, read set,
candidate profile, dependency resolutions, target candidate, and expiry before
the protected OS prompt.

Cancellation, timeout, authentication failure, root drift, or stale input
writes no event, profile, generated view, journal commit marker, or Git commit.

### Field and state translation

1. Freeze the exact v0.1 Git commit/tree, event root/count, legacy snapshot,
   proposal, actor-registry, artifact-registry, and canonical-store roots.
2. Freeze the full legacy identity preimage, including current Frontier ID and
   reducer seed fields, rather than relying on the truncated ID alone.
3. Preserve the exact current dependency list in sorted canonical form. Every
   legacy Frontier ID and pinned snapshot must match the verified immutable
   anchor of the supplied dependency boundary. The exact Profile v1 identity,
   scientific-state, Git commit, and Git tree are rederived from that anchor;
   supplied values are never trusted by assertion. A missing full legacy pin,
   ambiguous entry, dirty dependency checkout, wrong external trust anchor, or
   mismatched resolution blocks migration.
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

The key-free preview has two explicit root families. `roots_before` contains
the exact raw retained-store root and all exact pre-boundary roots.
`semantic_after` contains the event/count, legacy snapshot, proposal, actor,
artifact, profile, identity, dependency, and scientific-state roots that are
computable from the unsigned event core. The preview separately reports:

```text
signed_store_root_state: pending_protected_signature
```

It does not use a nullable or fabricated canonical-store root. The raw
post-migration retained-store root includes the canonical signed event bytes
and therefore cannot exist before the late protected key read. After user
approval, the executor supplies that exact raw root and every exact postimage
in the execution result, verifies them before the transaction commit marker,
and fails closed on any difference.

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
   Erdős to Formal v1 would require a later, separately signed dependency
   update. That writer is explicitly deferred beyond `0.914.0`; the migration
   must not claim or hand-author the retarget.

Divergent parent copies and immutable fixtures remain v0.1 unless selected by
their own owner. Histories are never unioned or spliced.

## Strict and non-strict behavior

For v1:

- wrong or unknown profile fields are strict blockers;
- a missing, duplicate, unsigned, invalid, unverifiable, forked, cyclic,
  chain-broken, rollback-shaped, or non-ancestor boundary is a strict blocker;
- the full boundary event set is validated before replay accepts repository
  identity or dependency state; a recognized event kind grants no exemption;
- identity, dependency, anchor Git, event, legacy snapshot, proposal,
  registry, artifact, or canonical-store mismatch is a strict blocker;
- an unsigned dependency substitution or stale target index grants no work or
  landing path;
- non-strict inspection reports the same problems but never treats an invalid
  boundary as an exemption;
- materialize validates but never creates or repairs a boundary event;
- work, land, policy routing, protected decisions, and migration refuse an
  invalid boundary before writing; and
- every canonical writer refuses a mismatched proposal ID, duplicate proposal,
  event-less terminal status, explicit review-decision field mismatch,
  orphan decision event, or invalid Receipt-bound withdrawal before creating a
  transaction journal; and
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
- a hand-edited proposal status or explicit decision projection, and a deleted
  proposal still targeted by a retained decision event;
- legacy snapshot, proposal, actor, artifact, or canonical-store mismatch;
- duplicate boundary events and previous-root chain breaks;
- conflicting signed children from one previous boundary;
- a first v1 dependency update that does not chain to the exact
  `frontier.created` content root;
- a missing identity-event parent, cycle, duplicate content root, or boundary
  whose anchor event count does not advance;
- invalid signer, signature, registry state, or protected confirmation;
- profile changes hidden behind YAML aliases, duplicate keys, unknown fields,
  non-canonical scalars, or Unicode normalization traps;
- identity substitution using a copied `.vela/` directory;
- actor-registry replacement followed by a self-signed boundary;
- a missing, malformed, symlinked, wrong-Frontier, wrong-identity,
  wrong-boundary, wrong-actor, or wrong-key consumer trust anchor;
- repository bytes, environment variables, settings, or a mutable tag
  attempting to create or replace the consumer trust anchor;
- a backdated boundary after an anchored revocation;
- dependency substitution using a mutable tag, short digest, mismatched Git
  object, or a retrieval hint presented as identity;
- display metadata incorrectly changing scientific-state root;
- runtime settings erased by work, land, materialize, or recovery;
- a stale target index offered as current work;
- a target index whose source object is missing, is not an ancestor, resolves
  to another tree, or already contains the exact sealed index;
- an omitted, reordered, dirty, untracked, symlinked, submodule-backed,
  case-colliding, or Unicode-colliding target input or packet;
- an index-root preimage that omits any field other than `index_root`, or an
  input-root preimage that includes its own supplied root;
- a candidate that supplies security roots, packet digests, Vela version, or
  another seal-owned field;
- a lease-only event suffix that incorrectly stales all other work, and a
  non-lease event suffix that incorrectly leaves work available;
- exact-ID inspection accidentally enabling a stale lease, a stale work claim
  writing coordination state, or two reads combining different index bytes;
- a private work session deletion erasing the exact target binding from its
  landed Receipt;
- `repair` silently refreshing roots without a new domain candidate, or
  `seal --check` writing, staging, or committing files;
- a generated file hand edit that happens to parse;
- a native Windows settings or Target Index mutation accidentally falling back
  to a path-based replace, creating a temporary, or touching the destination;
- a symlink, submodule, path traversal, case collision, or Unicode-normalized
  path collision in retained objects or target packets;
- a Receipt-bound legacy path removed or redirected to different bytes; and
- migration from a dirty, forked, recovery-incomplete, or root-drifting
  checkout.

Every invalid case fails before authority, profile, canonical history, or Git
mutation.

## Exact conformance tests

The executable contract is organized by test binary or exact Rust module
filter. These are test selectors, not aspirational capability labels: every
selector below must list and execute at least one test.

| Contract surface | Executable selector |
| --- | --- |
| Closed Profile v1, canonical root, NFC, genesis identity | `cargo test -p vela-protocol --test frontier_profile_v1` |
| Profile loading, v0.1 replay, profile-only edits, symlink refusal | `cargo test -p vela-protocol --test frontier_profile_loader_v1` |
| Boundary wire shape, chains, identity, dependencies, retained manifest | `cargo test -p vela-protocol --test frontier_repository_bound` |
| Repository anchor, TOFU pins, forks, registry replacement, retained bytes | `cargo test -p vela-edge --lib analysis::frontier_repository::tests` |
| Write gate, proposal parity, settings, trust-store custody | `cargo test -p vela-edge --lib analysis::repository_write::tests` |
| Minimal initialization and Profile v1 genesis | `cargo test -p vela-cli --test init_minimal` |
| Profile/settings behavior through ordinary CLI workflows | `cargo test -p vela-cli --test frontier_settings_v1` |
| Protected legacy migration and exact recovery | `cargo test -p vela-cli --lib cli::migration::tests` |
| Target Index CLI sealing, inspection, work binding, and v1 retirement | `cargo test -p vela-cli --test target_index_cli` |
| Target Index roots, Git assessment, staleness, and task binding | `cargo test -p vela-edge --lib analysis::target_index::tests` |
| Receipt target-task binding | `cargo test -p vela-protocol target_task_binding` |
| Portable reducer behavior | `cargo test -p vela-protocol --test cross_impl_reducer_fixtures` and `python3 conformance/verify.py` |

The migration module must retain these exact named assertions:

```text
migration_frontier_repo_v1_preview_is_zero_writes_and_root_binds_inputs
migration_preserves_all_preboundary_canonical_bytes
migration_cancellation_zero_writes_and_crash_recovers
pending_proposals_and_policy_history_survive_root_v2_without_substitution
consumer_pin_drift_before_migration_commit_is_zero_canonical_writes
```

`migration_preserves_all_preboundary_canonical_bytes` covers the original
event set, pending proposal, content-addressed Receipt, actor registry,
immutable policy pair, and Receipt-linked evidence bytes. The crash test exits
after the durable transaction marker and proves journal-only recovery installs
the exact postimages without another signer call. The root-v2 compatibility
test proves pending standing and historical snapshot references are not
substituted with `scientific_state_root_v2`.

The current read-only Erdős vector is pinned separately at Git commit
`e79feaeddf2d4c68ce395d2e7daec1e7fae41702`: 13 pending proposal files,
proposal root
`sha256:e69b38037814f2e8ca826942cfc50ab370993889be2913cac1c0b3e77711160f`,
and pending-byte root
`sha256:9e7c6cc1de996f34621291c8c5b9378e67d991b44b4989f7d43174a2f771f044`.
It is an external immutable regression input, not a copied corpus fixture.

The canonical cross-implementation root for the Profile A fixture in
`crates/vela-protocol/tests/frontier_profile_v1.rs` is:

```text
sha256:26f4cd0e61408c17b7e9f979ea8dca809a6c5ec0cbd5f22e6114ffdf68e1f1aa
```

The target-index fixture must likewise check in the complete candidate,
source-input manifest, packet, sealed v2 index, target-task binding, and fixed
expected `input_root`, `index_root`, and binding root. Tests read those fixed
expected strings; they may not compute the expected value with the function
under test. The Git fixture has commit A containing the declared inputs,
commit B containing the sealed index and packets, commit C changing only
profile or documentation bytes, and commit D appending one valid
`attempt.claimed` event. A second branch replaces D with one non-lease event.
The expected behavior is A unavailable as an offer before B, B/C/D fresh, and
the non-lease branch stale. A shallow clone missing A fails closed.

Focused commands:

```bash
cargo test -p vela-protocol --test frontier_profile_v1
cargo test -p vela-protocol --test frontier_profile_loader_v1
cargo test -p vela-protocol --test frontier_repository_bound
cargo test -p vela-protocol target_task_binding
cargo test -p vela-cli --test init_minimal
cargo test -p vela-cli --test frontier_settings_v1
cargo test -p vela-cli --lib cli::migration::tests
cargo test -p vela-cli --test target_index_cli
cargo test -p vela-edge --lib analysis::frontier_repository::tests
cargo test -p vela-edge --lib analysis::repository_write::tests
cargo test -p vela-edge --lib analysis::target_index::tests
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

When the exact local Erdős checkout is present, run the additional read-only
vector without copying or mutating it:

```bash
VELA_ERDOS_REGRESSION_FRONTIER=/path/to/erdos-frontier \
  cargo test -p vela-cli erdos_13_pending_proposals_read_only_regression_vector -- --ignored
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
