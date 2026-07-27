# Frontier repository profile

This document defines the current `vela.frontier-profile.v1` and
`vela.frontier_repo.v1` repository contract. It grants no authority by itself.
Git stores and transports exact bytes, Vela replays and governs scientific
state, domain tools produce evidence, and read systems project verified state.

Profile v0.1 remains readable for historical replay. It has no canonical writer
in the current candidate.

## One repository, one boundary

One ordinary Git repository contains one canonical `.vela/` history. Keep
content together while authority, correction policy, confidentiality, stable
namespace, source cadence, and steward group remain shared. Split a Frontier
when one of those boundaries changes materially, not because a repository
becomes large.

A workspace may pin several exact Frontier roots. It is a non-authoritative
collection unless it owns separately bounded claims and authority records.
Never present nested canonical `.vela/` histories as one Frontier.

## Closed descriptive profile

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

The schema is closed. Vela rejects duplicate YAML keys, anchors, aliases, merge
keys, explicit tags, non-NFC text, and disallowed control characters.

```text
profile_root = sha256(canonical_json(profile))
```

Comments, whitespace, quoting, key order, and final newlines therefore do not
affect the root.

The profile supplies discovery, scope, and stewardship metadata only.
Maintainers do not gain review or acceptance authority. `frontier_id` is
checked against identity derived independently from event history.

Tool pins belong in `vela.lock`. Exact scientific dependencies come from the
retained repository boundary. Runtime preferences belong in
`.vela/settings.toml`.

## Runtime settings

The checked-in settings file is closed:

```toml
schema = "vela.frontier-settings.v1"

[publish]
git_push = "off"

[work]
lease_ttl_seconds = 86400

[mcp]
profile = "read-only"
```

Only these allowlisted preferences are accepted. Credentials, keys, commands,
hooks, network endpoints, verifier declarations, dependencies, policy, actors,
and accepted-state settings are forbidden.

Effective precedence is flag, allowlisted environment override, user
configuration, checked-in Frontier convention, then built-in default. Safety
only narrows: repository `git_push = "off"` can disable a wider user
preference, and a user read-only MCP preference cannot be widened by a cloned
repository.

## Identity, dependencies, and trust

Profile metadata is not Frontier identity. Vela derives:

- `identity_root` from the exact creation or legacy boundary history;
- `dependency_root` over sorted full-root dependency records;
- `scientific_state_root` from the closed scientific-state component record;
- repository-authority continuity from the authority keyset, policy bundle,
  event coverage, and authority-record chain; and
- consumer trust from an independently installed first-boundary anchor.

Remote URLs and mutable refs are retrieval hints, never dependency identity.
An exact dependency pin is repository context only. It is not evidence,
standing, or acceptance.

Historical `frontier.repository_bound` events bind the exact identity,
dependency set, Git anchor, event prefix, actor registry, proposal set,
artifact registry, and retained canonical bytes. They cannot accept, reject,
correct, supersede, or retract a Claim.

The boundary chain must be linear. Vela rejects missing parents, forks, cycles,
duplicate roots, rollback-shaped anchors, changed identity fields, invalid
signatures, missing Git objects, non-ancestor anchors, altered retained bytes,
and registry drift. Event timestamps never establish membership.

Each consumer pins the full first-boundary root through a local, out-of-band
record:

```text
~/.vela/trust/frontiers/<frontier_id>.json
```

Install it with:

```bash
vela frontier trust pin . --boundary-root sha256:... --json
```

The trust pin stores no secret and changes no Frontier history. Vela never
derives it automatically from the checkout, environment, remote URL, branch,
tag, or mutable service.

New repository-boundary, actor-registry, dependency, and migration writers are
retired. Current administration uses attributed principals, restricted Cedar,
and repository-authority transactions as documented in
[SIGNING.md](SIGNING.md).

## Path ownership

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/events/`, `.vela/authority/`, `.vela/actors.json`, retained policies | Canonical protocol | Change only through released Vela operations |
| `.vela/proposals/`, `.vela/findings/`, `.vela/artifacts/` | Retained or reducer-owned protocol | Never hand-edit |
| `records/receipts/sha256/` and retained evidence | Exact evidence | Preserve by full digest |
| `frontier.yaml` | Descriptive repository profile | Edit deliberately, then validate |
| `.vela/settings.toml` | Allowlisted runtime preferences | Never scientific state |
| `frontier.json` | Generated read projection | Regenerate; never hand-edit |
| `vela.lock` | Generated root and tool witness | Regenerate; never hand-edit |
| `proof/` | Generated replay packet | Reserved for Vela, subject to retained legacy exceptions |
| `README.md`, `SCOPE.md` | Human onboarding | Keep consistent with the profile |
| `VELA.md` | Canonical agent charter | Generate adapters explicitly |
| target index and packets | Derived work projection | Seal and freshness-check; never treat as standing |
| domain-native files | Scientific source and evidence | Use stable identity paths |
| `.vela/work/`, operation journals | Private coordination and recovery | Never publish as scientific state |

Formal Conjectures, Sidon, and Erdős retain a small number of immutable legacy
path exceptions. Their exact bytes remain replayable; they are not precedents
for new writes.

## Target Index

The Target Index is an optional work bridge. Domain tools own target meaning
and packet schemas. Vela owns only a closed seal over exact Git inputs,
repository roots, packet bytes, and target ordering.

`next` and `start` fail closed on a stale or invalid index. A successful start
retains an exact target-task binding in the private Attempt and eventual
Submission. Deleting the index removes a catalogue convenience and changes no
scientific state or authority. Graph position and structural advice never
replace canonical producer ranking.

## Verification

`vela check . --strict` validates:

- the closed profile and settings;
- exact replay and projection parity;
- boundary and authority-history continuity;
- Git anchor, ancestry, and object availability;
- retained canonical bytes;
- actor, proposal, and artifact registries; and
- the consumer trust anchor.

Non-strict checking reports the same defects but is diagnostic only. Every
canonical writer refuses invalid repository context before creating a
transaction journal.

Profile v0.1 histories remain inspectable, replayable, and reproducible. Use
Vela `0.915.1` for exact historical command replay. Do not relabel an old
checkout, copy a v1 profile over it, or hand-author a continuation event.

## Serving

`vela serve .` exposes read-only MCP over stdio. `--profile draft` adds only
nonfinalizing producer work. Optional loopback HTTP uses the same selected
profile.

The server has no human decision, signing, policy-administration, or
accepted-state operation. It ignores caller-asserted HTTP actor names and
returns public-tier data. A networked authenticated service requires a
separately designed boundary.
