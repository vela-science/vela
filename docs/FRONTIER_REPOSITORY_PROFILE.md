# Frontier repository profile

This document defines the current `vela.frontier-profile.v2` repository
contract. The profile describes one bounded scientific Frontier. It grants no
authority and contains no accepted state.

## One repository, one boundary

One ordinary Git repository contains one Vela repository manifest and one
append-only repository-authority history. Keep material together while scope,
authority, correction policy, confidentiality, namespace, and stewardship are
shared. Split a Frontier when one of those boundaries changes materially.

Generated databases, graphs, sites, and search indexes are disposable readers.
They never become canonical because they are convenient to query.

## Keep the three `.vela` boundaries separate

The Vela implementation repository is source code, conformance fixtures,
bounded benchmark definitions, and publication evidence. It is not a
Frontier, must not contain a root `.vela/`, and must not be used as a convenient
home for scientific records.

A Frontier-local `.vela/` is repository control state for that one scientific
boundary. Canonical identity and authority bytes are tracked; private work,
temporary candidates, and recovery journals are ignored. Domain artifacts and
scientific records stay in the Frontier repository rather than moving into the
Vela implementation repository.

User-local `~/.vela/` is private machine state: configuration, identity
custody, and local execution output. Nothing there is canonical scientific
state. Repositories and readers must not depend on it for replay.

## Profile v2

`frontier.yaml` is closed, human-readable metadata:

```yaml
schema: vela.frontier-profile.v2
frontier_id: vfr_0123456789abcdef
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

The schema rejects unknown fields, duplicate keys, aliases, merge keys,
explicit tags, non-NFC text, and disallowed control characters.

```text
profile_root = sha256(canonical_json(profile))
```

YAML comments, whitespace, quoting, key order, and final newlines do not change
the root. Maintainers are descriptive and receive no review or repository
authority from the profile.

## Repository origin

`.vela/origin.json` has one closed schema: `vela.repository-origin.v1`.
It binds the `frontier_id`, Profile root, generation, initial object-set root,
and full origin identity carried by `vela.repository.v4`. Unknown or
substituted origins fail closed.

A native `vela init` writes Profile v2 and scaffolding only. `status`
identifies the repository as `authority_uninitialized`. One
`vela authority init` transaction installs the genesis, manifest, keyset,
policy, sequence-1 authority event and record, then creates the initial
unsigned Git commit.

The four compacted pre-release repositories retain one exact predecessor block
inside their origin. It is provenance, not an alternate active schema. There
is no current migration command.

## Current canonical layout

```text
frontier.yaml
.vela/origin.json
.vela/repository.json
.vela/authority/events/
.vela/authority/records/
.vela/authority/keysets/
.vela/authority/policies/
.vela/authority/policy-material/
records/claims/sha256/
records/submissions/sha256/
records/verifications/sha256/
records/proposals/sha256/
records/artifacts/sha256/
targets.json                       optional derived work index
```

The repository manifest binds every active canonical object by full root.
Claim Standing is derived from the manifest and verified Decision history.
`targets.json` is a disposable, root-bound work projection; it never defines
Standing or authority.

The active current layout does not use:

```text
.vela/events/
.vela/actors.json
.vela/findings/
.vela/proposals/
.vela/policies/
frontier.json
vela.lock
proof/
records/receipts/
```

Historical Git commits and predecessor tags retain old bytes. They are not
valid templates for new repositories.

## Runtime behavior

Frontier repositories do not configure the operator. Runtime behavior stays
explicit and process-local:

- Vela commits its exact local delta; ordinary Git owns network publication;
- `NO_COLOR` disables terminal color;
- `--quiet` or `VELA_ADVICE=0` suppresses advice.

Credentials, keys, commands, hooks, network endpoints, verifier declarations,
dependencies, policy, actors, and accepted-state settings never belong in a
checked-in runtime configuration file.

## Path ownership

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/origin.json`, `.vela/repository.json` | Canonical repository identity | Origin is immutable; manifest changes only through a Vela transaction |
| `.vela/authority/` | Canonical authentication history | Append through repository authority only |
| `records/**/sha256/` | Canonical content-addressed objects | Never hand-edit or rename |
| `frontier.yaml` | Descriptive profile | Edit deliberately; any root change must be governed before canonical writes continue |
| `targets.json` and packets | Derived work projection | Generate directly and freshness-check; never treat as Standing |
| domain-native files | Source and evidence | Keep stable, reviewable identities |
| `.vela/operation-journals/`, `.vela/work/` | Recovery/private coordination | Never scientific state |
| `README.md`, `SCOPE.md`, `VELA.md` | Human and agent guidance | Keep aligned with the current product |

## Verification

```bash
vela check . --json
vela status . --json
```

Verification checks:

- Profile, origin, and repository roots agree;
- every indexed canonical object exists at its exact content path;
- repository-authority records form one valid chain;
- every authority event is covered exactly once;
- active keyset and Cedar policy roots match the manifest;
- Claim and Proposal Standing is deterministic;
- the optional Target Index, its declared inputs, and packets exactly match
  tracked `HEAD` bytes and the current repository root; and
- rejected active legacy paths are absent.

`vela check` fails until native authority initialization completes.
Git publication transports bytes; it does not create scientific acceptance.
