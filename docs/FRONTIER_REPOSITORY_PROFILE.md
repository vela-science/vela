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

`frontier.toml` is closed, human-readable metadata:

```toml
schema = "vela.frontier-profile.v2"
frontier_id = "vfr_0123456789abcdef"
name = "Bounded human-readable name"
summary = "One concise description"
maintainers = []

[scope]
question = "Which bounded scientific question does this Frontier maintain?"
includes = []
excludes = []

[license]
content = "CC-BY-4.0"
code = "Apache-2.0"
data = "varies"
```

The schema rejects unknown fields, duplicate keys, oversized input, non-NFC
text, and disallowed control characters.

```text
profile_root = sha256(canonical_json(profile))
```

TOML comments, whitespace, quoting, key order, and final newlines do not change
the root. Maintainers are descriptive and receive no review or repository
authority from the profile.

## Frontier identity

`vela init` draws the `frontier_id` once, at genesis, from canonical
`vela.frontier-genesis-identity.v2` bytes:

```text
schema           vela.frontier-genesis-identity.v2
name             exact trimmed profile name
scope            exact trimmed bounded question
genesis_entropy  fresh 256-bit draw from the OS CSPRNG
frontier_id      "vfr_" + sha256(canonical_json(...))[..16 hex chars]
```

The entropy is what makes the identity name one repository. A Frontier is one
independently clonable repository with a bounded scope, and two groups may
legitimately open repositories on the same question with the same wording; they
are different Frontiers and must not receive the same identity. The user-local
trust store keys on `vfr_`, so a shared identity would make one repository's
authority anchor collide with the other's.

The entropy is not retained anywhere, and the derivation is deliberately not
reproducible. A `frontier_id` is asserted once and then carried: the Profile
holds it, the origin, manifest, keyset, and Cedar policy bind it, and no reader
recomputes it from the Profile's name and scope. Retaining the nonce would not
make the identity checkable, because whoever creates the repository chooses it.

The identity is a repository handle, not a scientific commitment. It is not a
Git commit, a repository root, or any statement about Standing.

## Repository origin

`.vela/origin.json` has one closed schema: `vela.repository-origin.v1`.
It binds the `frontier_id`, Profile root, generation, initial object-set root,
and full origin identity carried by `vela.repository.v4`. Unknown or
substituted origins fail closed.

A native `vela init` writes Profile v2 and scaffolding, then installs the
genesis, manifest, keyset, policy, sequence-1 authority event and record and
creates the initial unsigned Git commit. If signing cannot complete, the exact
Profile remains as a resumable bootstrap and `status` reports
`authority_uninitialized`; rerunning `vela init` completes the same lifecycle.

The four compacted pre-release repositories retain one exact predecessor block
inside their origin. It is provenance, not an alternate active schema. There
is no current migration command.

## Current canonical layout

```text
frontier.toml
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
records/proposal-withdrawals/sha256/
records/artifacts/sha256/
targets.json                       optional derived work index
targets/                           Target packets, wherever targets.json names
                                   them; every Frontier that has any uses this
```

The repository manifest binds every active canonical object by full root.
Claim Standing is derived from the manifest and verified Decision history.
`records/proposal-withdrawals/sha256/` holds `vela.proposal-withdrawal.v1`
objects, the producer-owned closure of one pending Proposal; the manifest
carries them as their own object set and `vela replay --json` reports them as
`counts.proposal_withdrawals`. `targets.json` is a disposable, root-bound work
projection; it never defines Standing or authority.

The active current layout does not use these paths, and `vela replay` fails on
a file at any of them:

```text
.vela/events/
.vela/actors.json
.vela/findings/
.vela/proposals/
.vela/artifacts/
.vela/policies/
frontier.json
frontier.yaml
records/receipts/
records/review/
records/decision-evidence/
records/vrc_*.json
```

These two are retired but not enforced:

```text
vela.lock
proof/
```

A Frontier carrying either verifies clean today. They stay on the list because
they are still wrong, and because two published Frontiers still declare
`.gitattributes` rules for `proof/**` and `vela.lock` against paths none of
them has. Enforcing them is a change to what verification rejects rather than a
documentation change, and those dead declarations have to go first.

The check is a worktree walk over files, so it sees untracked files but not an
empty retired directory.

Historical Git commits and predecessor tags retain old bytes. They are not
valid templates for new repositories.

## Conventions the profile does not define

Every published Frontier carries files this contract has never described. They
are listed here so an author knows they exist. Vela reads none of them and
`vela replay` validates none of them.

```text
.gitignore                         scaffolded by `vela init`
.gitattributes                     scaffolded by `vela init`
.github/workflows/vela-frontier.yml
sources.yaml                       domain-native source declarations
sources.lock.json                  derived; resolved source content hashes
STATEMENT.md                       the Frontier's question in prose
technique-sheet.md                 domain method notes
witnesses/                         domain-native evidence inputs
```

Of that list only `.gitignore` and `.gitattributes` are scaffolded.
`.gitattributes` matters more than its position here suggests. Vela never opens
it, but Git does, and canonical record bytes are content-addressed: a checkout
filter, keyword expansion, working-tree encoding, or merge driver that rewrites
them breaks replay. Two of the four published Frontiers have no
`.gitattributes` at all.

The remainder emerged in the Frontiers rather than in this contract, so the
shapes vary and the coverage is partial: `sources.yaml` is in all four
repositories, `STATEMENT.md` in three, `witnesses/` in two, and
`sources.lock.json` in one, written by repository-local Python. A CI workflow
is in all four and `vela init` scaffolds none of it. Treat these as domain
conventions worth copying, not as contract.

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
| `frontier.toml` | Descriptive profile | Edit deliberately; any root change must be governed before canonical writes continue |
| `targets.json` and packets | Derived work projection | Generate directly and freshness-check; never treat as Standing |
| domain-native files | Source and evidence | Keep stable, reviewable identities |
| `.vela/operation-journals/`, `.vela/work/` | Recovery/private coordination | Never scientific state |
| `README.md`, `SCOPE.md`, `AGENTS.md` | Human and agent guidance | Keep aligned with the current product; `CLAUDE.md` may be a one-line pointer to `AGENTS.md` |
| `STATEMENT.md`, `technique-sheet.md` | Optional domain guidance | Emerged convention, not contract; Vela reads neither |
| `.gitattributes` | Byte stability | Keep canonical paths out of every checkout filter, keyword expansion, encoding, and merge driver |
| `.gitignore` | Working-tree hygiene | Track canonical identity and authority bytes; ignore journals, workspaces, and `.vela/keys/` |
| `sources.yaml`, `sources.lock.json` | Domain source declarations and derived lock | Repository-local shape; never hand-write a hash nobody computed |
| `witnesses/` and other domain-native evidence | Source and evidence | Keep stable identities; canonical evidence is the Artifact copy under `records/artifacts/sha256/` |
| `.github/workflows/` | Verification gate | Run the read-only verification verb on push and pull request; CI reports, it never accepts |

## Verification

```bash
vela replay . --json
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
- the enforced retired paths listed above are absent; the two unenforced ones
  are not checked.

All four published Frontiers gate this in
`.github/workflows/vela-frontier.yml` on every push and pull request. Each one
pins `vela-science/vela` by commit SHA and passes `frontier: .`; the action
installs the pinned release and runs `vela replay <frontier> --json`. `vela
init` does not scaffold that workflow — each Frontier wrote its own copy, so
they can and do drift to different pins.

`vela replay` fails until native authority initialization completes.
Git publication transports bytes; it does not create scientific acceptance.
