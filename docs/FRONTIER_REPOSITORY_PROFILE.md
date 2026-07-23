# Frontier Repository Profile

This document explains the released `vela.frontier-profile.v1` and
`vela.frontier_repo.v1` repository contract. It adds no authority of its own:
Git stores and transports exact bytes, Vela replays and governs scientific
state, domain tools produce evidence, and read systems project verified state.

Profile v0.1 remains readable for historical replay and migration. In Vela
0.914, it is no longer a second canonical writer: every canonical v0.1 write
fails with `frontier_profile_upgrade_required`.

## One repository, one boundary

One ordinary Git repository contains one canonical `.vela/` history by
default. Keep content together while authority, correction policy,
confidentiality, stable namespace, source cadence, and steward group remain
shared. Split a Frontier when one of those boundaries changes materially, not
merely because the repository becomes large.

A workspace or portfolio may pin several exact Frontier roots. It is a
non-authoritative collection unless it owns separately bounded claims and
signed events. Never present nested canonical `.vela/` histories as one
Frontier.

## Closed, non-authoritative profile

`frontier.yaml` is human-facing repository metadata:

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

The schema is closed at every mapping. Vela rejects duplicate YAML keys,
anchors, aliases, merge keys, explicit tags, non-NFC text, and disallowed
control characters. It parses the YAML and computes
`profile_root = sha256(canonical_json(profile))`; comments, whitespace, key
order, quoting, and final newlines therefore do not affect that root.

The profile supplies discovery, scope, and stewardship metadata only.
Maintainers do not gain signing or acceptance authority. Changing display
metadata changes the profile and Git roots, but not scientific state or
standing. `frontier_id` is checked against identity derived independently from
the event history; it cannot overwrite that identity.

Profile v1 fixes repository paths and removes the v0.1 `layout`, `mode`,
`visibility`, reducer, configurable-path, proof-policy, review-policy, and
dependency fields. Tool pins belong in `vela.lock`. Exact scientific
dependencies come from the signed repository boundary. Runtime preferences
belong in `.vela/settings.toml`.

## Runtime settings

The checked-in settings file is also closed:

```toml
schema = "vela.frontier-settings.v1"

[publish]
git_push = "off"

[work]
lease_ttl_seconds = 86400

[mcp]
profile = "read-only"
```

Only `publish.git_push = "off"`, a work lease TTL from 1 through 31,536,000
seconds, and `mcp.profile = "read-only" | "draft"` are accepted. Credentials,
keys, commands, hooks, network endpoints, verifier declarations,
dependencies, policy, actors, and accepted-state settings are forbidden.

Effective precedence is flag, then allowlisted environment override, then user
configuration, then the checked-in Frontier convention, then the built-in
default. Safety can only narrow: checked-in `publish.git_push = "off"` may
disable a wider user preference, and a user `mcp.profile = "read-only"` cannot
be widened by a cloned repository.

Profile v0.1 `.vela/config.toml` remains readable until migration. A current
Profile v1 repository has no active `.vela/config.toml`.

## Identity, dependencies, and administrator trust

Profile metadata is not the Frontier identity. Vela derives:

- a full `identity_root` from the exact Profile v1 `frontier.created` event or
  the signed legacy migration boundary;
- a `dependency_root` over sorted, closed dependency records containing each
  dependency's Frontier ID, identity root, scientific-state root, Git object
  format, commit, and tree; and
- a `scientific_state_root` from the closed
  `vela.scientific-state.v2` component record.

Remote URLs and mutable refs are retrieval hints, never dependency identity.
Every resolved dependency must match all full roots in its signed record.
The identity fields name one authenticated repository; the scientific-state
and Git fields name one exact state within it. An exact pin is repository
context only. It cannot become evidence, a transfer edge, scientific standing,
or acceptance without a separately retained class-specific object and the
ordinary authority path.

Vela `0.914.0` migration resolves only the signed dependency-boundary anchor.
The Proposed ADR 0018 candidate additionally permits one exact historical
commit when it is rederived as a byte-retained ancestor of the independently
pinned first temporalization anchor. That candidate changes no wire record and
currently requires the historical and temporalization states to have the
canonical empty dependency context. It remains unreleased until its focused
adversarial fixtures and the real Formal/Erdős read-only vector pass.

Repository administration begins with a signed, non-scientific
`frontier.repository_bound` event. It binds the exact identity, dependency
set, administrator, Git anchor, event prefix, actor registry, proposal set,
artifact registry, and retained canonical bytes. It cannot accept, reject,
correct, supersede, or retract a finding.

A boundary chain must be one valid linear chain. Vela rejects missing parents,
forks, cycles, duplicate roots, rollback-shaped anchors, changed identity or
administrator fields, invalid signatures, missing Git objects, non-ancestor
anchors, altered retained bytes, and registry drift. Event timestamps never
establish boundary membership.

The first administrator boundary cannot authenticate itself from repository
bytes alone. Each consumer pins its full content root and administrator key
through a public, local, out-of-band record:

```text
~/.vela/trust/frontiers/<frontier_id>.json
```

Vela never derives that pin automatically from the checkout, an environment
variable, a profile, a settings file, a remote URL, or a mutable tag. It stores
no secret. On Unix, the trust directory and file are required to have
user-private modes.

For a native Profile v1 Frontier:

```bash
vela id protect --json
vela actor add . --json
vela frontier bind . --reason "establish the first administrator" --json
vela frontier bind . --reason "establish the first administrator" \
  --confirm-root sha256:... --confirm-at <RFC3339> --json
```

`actor add` is a one-shot protected possession proof that replaces only the
canonical empty registry with the exact configured human actor. It is not a
general actor-registry editor. Agent identities and plaintext file identities
cannot bootstrap repository administration; the later boundary requires that
human to be a `reviewer:` or `steward:`. The successful result is left as an
exact uncommitted delta; commit it before `frontier bind`.

`frontier bind` is also two phase. Preview is key-free. Apply rederives the
same plan before asking the protected OS signer for one exact approval. It
appends one boundary event and installs the confirmed local pin, but does not
stage, commit, or push Git.

Vela `0.914.0` intentionally has no porcelain for a later dependency update.
The dependency set established by this first boundary is immutable through
ordinary commands until a separately reviewed protected-update workflow ships.

Another consumer installs the independently reviewed first boundary:

```bash
vela frontier trust pin . --boundary-root sha256:... --json
vela frontier trust pin . --boundary-root sha256:... \
  --confirm-root sha256:... --confirm-at <RFC3339> --json
```

There is no `--force`, automatic TOFU, or repository-local fallback.

## Path ownership

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/events/`, `.vela/actors.json`, retained policy pairs | Canonical protocol | Change only through released Vela operations |
| `.vela/proposals/`, `.vela/findings/`, `.vela/artifacts/` | Retained or reducer-owned protocol | Never hand-edit |
| `records/receipts/sha256/` and retained evidence | Exact evidence | Preserve by full digest |
| `frontier.yaml` | Human repository profile | Edit deliberately, then validate |
| `.vela/settings.toml` | Allowlisted operational preferences | Never scientific state; preserve across writes |
| `frontier.json` | Generated read projection | Regenerate; never hand-edit |
| `vela.lock` | Generated root and tool witness | Regenerate; never hand-edit |
| `proof/` | Generated replay and integrity packet | Reserved for Vela, subject to documented legacy exceptions |
| `README.md`, `SCOPE.md` | Human onboarding | Keep consistent with the profile |
| `VELA.md` | Canonical agent charter | Generate adapters explicitly |
| `targets.json` and target packets | Derived work projection | Seal and freshness-check; never treat as standing |
| domain-native files | Scientific source and evidence | Use stable identity paths |
| `.vela/work/`, operation journals | Local coordination and recovery | Never publish as scientific state |

Three maintained histories contain immutable legacy exceptions: Formal
Conjectures has a Receipt-bound Lean file under `proof/`; Sidon has tracked
artifact blobs under `.vela/artifact-blobs/`; and Erdős `main` restores 32
content-addressed blobs whose immutable records already named those exact
locators. Migration preserves their exact bytes; none of these paths is a
precedent for new writes.

## Target Index v2

`vela.target-index.v2` is the optional generic work bridge documented in
[TARGET_INDEX.md](TARGET_INDEX.md). Domain tools own target meaning and packet
schemas. Vela owns only a closed seal over exact Git inputs, repository roots,
packet bytes, and target ordering.

`next` and `work` fail closed on stale or invalid v2 indexes. A successful
claim retains a `vela.target-task-binding.v1` in the private session and copies
the same record into Receipt v1 at landing. Deleting `targets.json` removes a
catalogue convenience and changes no scientific state or authority. Graph
position and structural advice never replace canonical producer ranking.

## Verification and compatibility

For Profile v1, `vela check . --strict` validates the closed profile and
settings, exact replay and parity, the complete boundary chain, Git
anchor/ancestry and object availability, retained bytes, actor registry, and
the consumer's external pin whenever an administrator boundary exists.

Non-strict checking reports the same typed repository-context defects and
keeps the context invalid. It never turns an invalid boundary into an
exemption. Strict mode makes those defects blocking. Every canonical writer,
including work, land, policy administration, protected decisions, and
materialization, refuses invalid Profile v1 repository context before creating
a transaction journal. Legacy migration uses its own complete v0.1
Git/replay/retained-byte gate before it may append the first boundary.

Profile v0.1 histories remain inspectable, replayable, reproducible, and
migratable. Profile v1 events are an intentional protocol-version boundary for
older binaries.

## Native Windows mutation boundary

Profile v1 reading, strict checking, replay, and reproduction remain supported
by the native Windows binary. The two repository-file writers that require an
exact present-or-absent preimage—`.vela/settings.toml` updates and Target Index
v2 sealing—fail closed on native Windows in this release.

This is a security boundary, not a missing compatibility alias. The maintained
Unix edge pins the repository and parent directories, creates and flushes a
temporary relative to the pinned parent, atomically performs no-clobber create
or exchange, verifies the displaced exact preimage, reads back the installed
bytes, and flushes the parent. The native Windows path may be enabled only
after one implementation proves the equivalent properties:

1. every repository component is opened without following a reparse point and
   is retained by stable file identity;
2. temporary creation is relative to the pinned parent and cannot write
   outside the repository after a path swap;
3. absent installation is atomic and no-clobber;
4. existing installation atomically retains the displaced file so its exact
   bytes and identity can be checked and restored before deletion;
5. the replacement and containing directory receive an explicit durability
   barrier; and
6. native Windows race and crash tests cover root, parent, leaf, reparse-point,
   and replacement substitution.

Microsoft documents
[`ReplaceFileW`](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew)
as accepting path names. Its documented Win32
[`FILE_RENAME_INFO`](https://learn.microsoft.com/windows/win32/api/winbase/ns-winbase-file_rename_info)
contract requires a null `RootDirectory`, and `FileRenameInfoEx` offers
replace/no-replace rather than exchange. Combining those APIs without the
properties above would create a TOCTOU or unverified clobber gap, so Vela does
not ship a partial writer. Operators may use WSL2 with a WSL-owned
Linux-filesystem checkout, or another supported Unix host, for these
mutations.

## Serving

`vela serve .` defaults to the read-only MCP surface over stdio;
`--profile draft` adds only the nonfinalizing work tool. `vela serve . --http
3741` exposes the same selected tool profile and REST reads on
`127.0.0.1:3741` only. The HTTP reader has no authenticated request identity,
ignores caller-asserted actor names, and returns public-tier data only. Neither
profile exposes signing or a human decision. A networked or authenticated
service needs a separately designed boundary; changing the bind address is not
a supported shortcut.

## Migration

Use the protected, two-phase migration documented in [CLI.md](CLI.md).
Migration preserves every pre-boundary event, proposal, Receipt,
registration, policy, finding, artifact, evidence object, and signature byte.
It appends exactly one signed non-scientific boundary event, so Git and
event-log roots intentionally change. Proposal, actor-registry,
artifact-registry, and retained historical object roots do not.

Never relabel a v0.1 checkout, hand-edit its generated views, or copy a v1
profile over it. Preview the exact migration, review its candidate profile,
dependency resolutions, target-index candidate, touched paths, and root
families, then apply the matching protected plan. A historical dependency
selection is eligible only through the Proposed ADR 0018 ancestor proof;
branch names, tags, timestamps, short IDs, current-state substitution, and Git
ancestry without retained Vela-byte verification remain invalid.
