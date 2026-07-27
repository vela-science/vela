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

`.vela/epoch.json` has one of two closed schemas:

- `vela.repository-genesis.v1` for a repository created directly by the
  current product;
- `vela.repository-epoch.v1` for one of the signed predecessor boundaries
  produced before the migration writer was retired.

Both bind the same `frontier_id`, `epoch_id`, and full `epoch_root` carried by
`.vela/repository.json`. Unknown or substituted origins fail closed.

A native `vela init` writes Profile v2 and scaffolding only. `status` and
`doctor` identify the repository as `authority_uninitialized`. One
`vela authority init` transaction installs the genesis, manifest, keyset,
policy, sequence-1 authority event and record, then creates the initial
unsigned Git commit. No Era-0 event log or actor registry is invented.

Existing predecessor epochs remain exact read-only origin objects. There is no
command that creates another migration epoch.

## Current canonical layout

```text
frontier.yaml
.vela/epoch.json
.vela/repository.json
.vela/authority/events/
.vela/authority/records/
.vela/authority/keysets/
.vela/authority/policies/
.vela/authority/policy-material/
records/claims/sha256/
records/submissions/sha256/
records/registrations/sha256/
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

## Runtime settings

`.vela/settings.toml` contains allowlisted local/product preferences only:

```toml
schema = "vela.frontier-settings.v1"
```

Credentials, keys, commands, hooks, network endpoints, verifier declarations,
dependencies, policy, actors, and accepted-state settings are forbidden.
Safety preferences may narrow behavior but may not widen repository authority.

## Path ownership

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/epoch.json`, `.vela/repository.json` | Canonical repository identity | Change only through a released Vela transaction |
| `.vela/authority/` | Canonical authentication history | Append through repository authority only |
| `records/**/sha256/` | Canonical content-addressed objects | Never hand-edit or rename |
| `frontier.yaml` | Descriptive profile | Edit deliberately; any root change must be governed before canonical writes continue |
| `.vela/settings.toml` | Runtime preference | Never scientific state |
| `targets.json` and packets | Derived work projection | Seal and freshness-check; never treat as Standing |
| domain-native files | Source and evidence | Keep stable, reviewable identities |
| `.vela/operation-journals/`, `.vela/work/` | Recovery/private coordination | Never scientific state |
| `README.md`, `SCOPE.md`, `VELA.md` | Human and agent guidance | Keep aligned with the current product |

## Verification

```bash
vela repository verify . --json
vela status . --json
vela doctor . --all --json
```

Verification checks:

- Profile, origin, and repository roots agree;
- every indexed canonical object exists at its exact content path;
- repository-authority records form one valid chain;
- every authority event is covered exactly once;
- active keyset and Cedar policy roots match the manifest;
- Claim and Proposal Standing is deterministic;
- optional Target Index inputs match the current repository root; and
- rejected active legacy paths are absent.

`repository verify` fails until native authority initialization completes.
Git publication transports bytes; it does not create scientific acceptance.
