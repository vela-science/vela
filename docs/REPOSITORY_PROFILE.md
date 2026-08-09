# Repository profile

This document defines the current `vela.repository-profile.v1` repository
contract. The profile describes one bounded scientific Repository. It grants no
authority and contains no accepted state.

## One repository, one boundary

One ordinary Git repository contains one Vela repository manifest and one
append-only repository-authority history. Keep material together while scope,
authority, correction policy, confidentiality, namespace, and stewardship are
shared. Split a Repository when one of those boundaries changes materially.

Generated databases, graphs, sites, and search indexes are disposable readers.
They never become canonical because they are convenient to query.

## Keep the three `.vela` boundaries separate

The Vela implementation repository is source code, conformance fixtures,
bounded benchmark definitions, and publication evidence. It is not a Vela
repository, must not contain a root `.vela/`, and must not be used as a convenient
home for scientific records.

A repository-local `.vela/` is control state for that one scientific
boundary. Canonical identity and authority bytes are tracked; private work,
temporary candidates, and recovery journals are ignored. Domain artifacts and
scientific records stay in the Vela repository rather than moving into the
Vela implementation repository.

User-local `~/.vela/` is private machine state: configuration, identity
custody, and local execution output. Nothing there is canonical scientific
state. Repositories and readers must not depend on it for replay.

## Profile v2

`vela.toml` is closed, human-readable metadata:

```toml
schema = "vela.repository-profile.v1"
repository_id = "vrepo_0123456789abcdef0123456789abcdef"
name = "Bounded human-readable name"
summary = "One concise description"
maintainers = []

[scope]
question = "Which bounded scientific question does this repository maintain?"
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

## Repository identity

`vela init` draws the `repository_id` once, at genesis, from canonical
`vela.repository-genesis-identity.v1` bytes:

```text
schema           vela.repository-genesis-identity.v1
name             exact trimmed profile name
scope            exact trimmed bounded question
genesis_entropy  fresh 256-bit draw from the OS CSPRNG
repository_id    "vrepo_" + sha256(canonical_json(...))[..16 hex chars]
```

The entropy is what makes the identity name one repository. A Repository is one
independently clonable Git repository with a bounded scope, and two groups may
legitimately open repositories on the same question with the same wording; they
are different repositories and must not receive the same identity. The user-local
trust store keys on `vrepo_`, so a shared identity would make one repository's
authority anchor collide with the other's.

The entropy is not retained anywhere, and the derivation is deliberately not
reproducible. A `repository_id` is asserted once and then carried: the Profile
holds it, the origin, manifest, keyset, and Cedar policy bind it, and no reader
recomputes it from the Profile's name and scope. Retaining the nonce would not
make the identity checkable, because whoever creates the repository chooses it.

The identity is a repository handle, not a scientific commitment. It is not a
Git commit, a repository root, or any statement about Standing.

## Repository origin

`.vela/origin.json` has one closed schema: `vela.repository-origin.v1`.
It binds the `repository_id`, Profile root, generation, initial object-set root,
and full origin identity carried by `vela.repository.v4`. Unknown or
substituted origins fail closed.

A native `vela init` writes Profile v2 and scaffolding, then installs the
genesis, manifest, keyset, policy, sequence-1 authority event and record and
creates the initial unsigned Git commit. If signing cannot complete, the exact
Profile remains as a resumable bootstrap and `status` reports the same
`vela.status.v4` document it reports for a replaying repository, with
`integrity.strict` blocked and `actions.work.mode` `authority_uninitialized`;
rerunning `vela init` completes the same lifecycle.

The four compacted pre-release repositories retain one exact predecessor block
inside their origin. It is provenance, not an alternate active schema. There
is no current migration command.

## Current canonical layout

```text
vela.toml
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
                                   them; every repository that has any uses this
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
vela.lock
proof/
```

These are retired without `vela replay` rejecting them. This block is the
list; `conformance/repository_lint.py` in `vela` reads it and fails on a file at
any of these paths, so the retirement is checked without widening what
verification refuses.

<!-- repository-lint:retired-paths -->
```text
SCOPE.md
scripts/write_sources_lock.py
```

A repository carrying either of them verifies clean today, and each is here for
its own reason.

`vela.lock` and `proof/` used to be listed here instead. What kept them out of
the enforced block was not the protocol: two published repositories declared
`.gitattributes` rules for `proof/**` and `vela.lock` against paths neither of
them had, and refusing a path while a dead rule for it was still in the tree
would have made the reason for a failure ambiguous. Both rules are gone
(erdos-frontier `ddf1d291`, quantum-codes-frontier `53ff1e2`), no published
repository names either path anywhere, and the two have moved up into the block
`vela replay` enforces.

`SCOPE.md` restated scope that `vela.toml` already declares and
`profile_root` already commits to, so it could only ever drift out of
agreement with it; `vela init` does not scaffold it. `scripts/write_sources_lock.py`
was one resolver copied into three repositories, replaced by the shared
`vela-source-manifest` package. Neither is protocol state and neither should
be: replay has no opinion about a repository's build scripts, and acquiring one
would put the profile's housekeeping inside the thing that decides Standing.

The check is a worktree walk over files, so it sees untracked files but not an
empty retired directory.

Historical Git commits and predecessor tags retain old bytes. They are not
valid templates for new repositories.

## Conventions the profile does not define

Every published repository carries files this contract has never described. They
are listed here so an author knows they exist. Vela reads none of them and
`vela replay` validates none of them.

```text
.gitignore                         scaffolded by `vela init`
.gitattributes                     scaffolded by `vela init`
.github/workflows/vela-frontier.yml
sources.yaml                       domain-native source declarations
sources.lock.json                  derived; resolved source content hashes
STATEMENT.md                       the repository's question in prose
technique-sheet.md                 domain method notes
witnesses/                         domain-native evidence inputs
artifacts/                         domain evidence in its working form
```

Of that list only `.gitignore` and `.gitattributes` are scaffolded.
`.gitattributes` matters more than its position here suggests. Vela never opens
it, but Git does, and canonical record bytes are content-addressed: a checkout
filter, keyword expansion, working-tree encoding, or merge driver that rewrites
them breaks replay. All four published repositories now carry one, and all four
hold `.vela/**`, `records/**` and `artifacts/**` to `-text`, because
end-of-line normalization is a byte rewrite and a content-addressed blob that
is rewritten no longer hashes to its own name.

That `artifacts/**` rule is the one place the scaffold names a domain path
before it exists. `vela init` creates no `artifacts/` directory and reports
writing six files, none of them under it; what it does is hold the path to
`-text` in advance, so a repository that later puts evidence there gets byte
stability without having to know it needed it. Three of the four repositories
now have one, holding between 1 and 62 tracked files.

The remainder emerged in the repositories rather than in this contract, so the
shapes vary and the coverage is partial: `sources.yaml` and
`sources.lock.json` are in all four repositories, `STATEMENT.md` in all four,
`artifacts/` in three, and `witnesses/` in two. The lock is written by the
shared `vela-source-manifest` package, not by a script any repository owns. A CI
workflow is in all four and `vela init` scaffolds none of it. Treat these as
domain conventions worth copying, not as contract.

## Runtime behavior

Vela repositories do not configure the operator. Runtime behavior stays
explicit and process-local:

- Vela commits its exact local delta; ordinary Git owns network publication;
- `NO_COLOR` disables terminal color;
- `--quiet` or `VELA_ADVICE=0` suppresses advice.

Credentials, keys, commands, hooks, network endpoints, verifier declarations,
dependencies, policy, actors, and accepted-state settings never belong in a
checked-in runtime configuration file.

## Path ownership

Every tracked top-level entry in the four published repositories falls into one of
the rows below, as does every name their four `.gitignore` files keep out. The
table is written to be exhaustive over those repositories rather than
illustrative: a path that lands in a repository and matches no row is either a row
this contract is missing or a file that should not have been committed, and both
of those are worth the argument the omission would have skipped.

Nothing enforces that, deliberately. A domain directory is the repository's own
shape — it may hold tracked evidence and ignored scratch side by side, as
erdos's `lean/` does — and a rule here that refused an unfamiliar name would
redden a repository for a change that was correct. The table classifies; the
repository decides.

| Path | Class | Rule |
| --- | --- | --- |
| `.vela/origin.json`, `.vela/repository.json` | Canonical repository identity | Origin is immutable; manifest changes only through a Vela transaction |
| `.vela/authority/` | Canonical authentication history | Append through repository authority only |
| `records/**/sha256/` | Canonical content-addressed objects | Never hand-edit or rename |
| `vela.toml` | Descriptive profile | Edit deliberately; any root change must be governed before canonical writes continue |
| `targets.json`, `targets/` | Derived work projection | Generate directly and freshness-check; never treat as Standing |
| `artifacts/` | Domain evidence, working copy | The canonical copy of anything that matters is the Artifact under `records/artifacts/sha256/`; this path is the readable form beside it and confers nothing. `vela init` does not create it and holds it to `-text` anyway, so evidence put here is byte-stable from the first commit |
| `witnesses/`, `sources/`, `execution/`, `attack/`, `lean/`, `statements/`, `discoveries/`, `research/`, `reproductions/`, `verification/`, `verifiers/`, `lean-verifications/`, `exports/`, `review/`, and the repository's own `*.yaml` and `*.json` inventories | Source and evidence | Domain-native, one repository's own shape. Keep stable, reviewable identities. Nothing here is read by Vela or validated by replay, so a file that wants a shape needs a reader in the repository that owns it. `exports/` is tracked in two repositories and ignored in two, which is allowed and is why the class is the answer rather than the name |
| `.vela/operation-journals/`, `.vela/work/`, `.vela/tasks/`, `.vela/workspaces/`, `.vela/tmp/`, `.vela/agents/`, `.vela/keys/`, `.vela/source-inbox/`, `.vela/artifact-blobs/`, `.vela/sign-session.json` | Machine-local runtime | Never scientific state, never tracked. The runtime writes these beside the authority chain, so only an explicit ignore rule keeps a routine `git add .vela` from staging a private key |
| `__pycache__/`, `*.pyc`, `.venv/`, `.pytest_cache/`, `.hypothesis/`, `.ruff_cache/`, `target/`, `node_modules/`, `.DS_Store`, `packets/`, `activity/`, `.contract-source/` | Machine-local scratch | Regenerated by a tool or a run; ignored in every repository that can produce it. A pattern here carries nothing after it on the line, because `#` only opens a comment at the start of a line and a same-line annotation is read as part of the pattern |
| `README.md`, `AGENTS.md`, `CLAUDE.md` | Human and agent guidance | Keep aligned with the current product. `AGENTS.md` is the one agent guide; all four repositories carry a `CLAUDE.md` that is the single line `@AGENTS.md`, and a vendor-specific copy with its own content is a second guide that will disagree with the first. Scope is declared once in `vela.toml`, which `profile_root` commits to; a `SCOPE.md` restating it drifts and is not scaffolded |
| `STATEMENT.md`, `technique-sheet.md` | Optional domain guidance | Emerged convention, not contract; Vela reads neither. All four now carry `STATEMENT.md` |
| `CONTRIBUTING.md`, `LICENSE`, `campaigns/`, `SCREENING.md`, `STANDARD_CHECK.md`, and other repository-local prose | Human guidance | Not contract and not state. Answers how to work here, never what is accepted; a claim about Standing belongs in a record, where it can be replayed. A prose file whose content is already in `README.md` is a second copy to keep in step, and belongs in Git history rather than the active tree |
| `pyproject.toml`, `uv.lock`, `scripts/`, `tests/`, top-level `*.py` | Repository build and check | The repository's own tooling, gated by its own CI. Replay has no opinion about it, and acquiring one would put the profile's housekeeping inside the thing that decides Standing |
| `.gitattributes` | Byte stability | Keep canonical paths out of every checkout filter, keyword expansion, encoding, and merge driver |
| `.gitignore` | Working-tree hygiene | Track canonical identity and authority bytes; ignore journals, workspaces, and `.vela/keys/` |
| `sources.yaml`, `sources.lock.json` | Domain source declaration and derived lock | Never hand-write a hash nobody computed. Every value in the lock is derived from bytes and none is read from the clock, so a rerun over an unchanged inventory rewrites it byte for byte and `git diff --exit-code` after a re-resolve is the whole check. A repository gets that when it bumps its pinned `vela-source-manifest` rev, which is also what dates the lock: the commit does, and it dates the record rather than the run |
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
- the enforced retired paths listed above are absent; the rest are not replay's
  to check and are left to `conformance/repository_lint.py`.

No published repository gates this in CI today. The four that did,
`erdos-frontier`, `sidon-frontier`, `quantum-codes-frontier` and
`formal-conjectures-frontier`, were archived on 2026-08-07. They stay public and
clonable and each still carries `.github/workflows/vela-frontier.yml`, pinning
`vela-science/vela` at `c4023f11` (`v0.966.4`) and passing `frontier: .` so the
action installs the pinned release and runs `vela replay <path> --json`. An
archived repository takes no push and no pull request, so none of that runs
again.

The one live repository, `vela-science/math` (`vrepo_56d3fdfcd34ff5c3`), carries
no `.github/` at all. `vela init` scaffolds no workflow, and each of the four
wrote its own copy, so a repository gets this gate only by writing one, and
`math` has not.

The action took `frontier` as a deprecated alias for `repository` until this
was reread. The reason given was that four archived pins spell it and an
archived pin cannot move, and that is the argument backwards: because the pin
cannot move, it also cannot reach the alias. Those pins resolve to `c4023f11`,
which predates the alias — at that commit `frontier` is the action's only input
— so a pinned consumer fetches an action where the old key is native and no
alias is involved. The alias on the current action was reachable by nobody, and
it is gone. `repository` is the only input.

`vela replay` fails until native authority initialization completes.
Git publication transports bytes; it does not create scientific acceptance.
