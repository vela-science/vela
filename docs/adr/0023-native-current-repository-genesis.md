# ADR 0023: Native current repository genesis

- Status: Accepted
- Target release: Vela `v0.940.0`
- Protocol effect: one native `vela.repository-genesis.v1` origin object
- Product effect: `vela init` creates Profile v2 directly and the ordinary
  bootstrap no longer writes Era-0 events, actors, snapshots, or lock files
- Authority effect: the existing sequence-1 repository-authority transaction
  signs the exact genesis and current repository manifest
- Compatibility: existing `vela.repository-epoch.v1` repositories replay
  byte-for-byte; no predecessor record is rewritten

## Context

ADR 0022 moved all four active Frontiers to the current repository object
model. It also removed the one-time migration writer. The ordinary creation
path remained inconsistent: `vela init` still wrote a Profile v1 repository
with an unsigned `frontier.created` event, empty actor registry, generated
snapshot, and lock file. The current `vela status` command immediately
rejected that repository because it requires Profile v2.

This is both a product defect and a trust-base defect. A repository created
after the current boundary has no predecessor. Inventing an Era-0 history only
so another command can retire it adds bytes, parsers, and explanation without
preserving any real history.

## Decision

### 1. Fresh repositories start in Profile v2

`vela init <path> --name <name> --scope <scope>` writes only:

```text
frontier.yaml             vela.frontier-profile.v2
README.md
SCOPE.md
VELA.md
.gitignore
.gitattributes
.vela/settings.toml
```

It initializes Git on `main` but does not create a commit. It writes no:

```text
.vela/events/
.vela/actors.json
.vela/proof-state.json
frontier.json
vela.lock
.vela/epoch.json
.vela/repository.json
```

The frontier ID is derived from canonical
`vela.frontier-genesis-identity.v1` bytes containing the exact trimmed name
and bounded scope. This gives the profile a stable non-circular identity.

Before authority exists, `status` and `doctor` return a valid bootstrap view:

```text
phase: authority_uninitialized
replay: not_initialized
strict: blocked
blocker: repository_authority_uninitialized
next_action: vela authority init ...
```

`repository verify` continues to fail closed because there is not yet a signed
repository.

### 2. Native genesis is not a predecessor epoch

`vela.repository-genesis.v1` is canonical JSON containing:

```text
epoch_id
frontier_id
epoch = 1
profile_root
initial_object_set_root
initial_event_log_root
initial_actor_registry_root
reason
```

The initial object set is empty. The event-log and actor-registry roots are the
exact SHA-256 roots of empty inputs. The object contains no remote, tag,
commit, archive, imported set, compatibility snapshot, or equivalence report.

`.vela/epoch.json` remains the single origin path. Readers dispatch on the
closed top-level schema:

- `vela.repository-epoch.v1` means a signed predecessor boundary;
- `vela.repository-genesis.v1` means a native current origin.

Unknown schemas fail closed.

### 3. Authority initialization installs the repository atomically

`vela authority init` is the only transition out of bootstrap. Before asking
the repository-authority signer, it revalidates:

- Profile v2 and its root;
- absence of epoch, manifest, authority, and scientific object stores;
- the exact empty history roots;
- the selected Ed25519 public key;
- the generated Cedar policy and keyset;
- the genesis and repository manifest roots;
- the local principal, action, reason, binary, and transaction read set.

One recoverable transaction appends:

```text
.vela/epoch.json
.vela/repository.json
.vela/authority/events/<id>.json
.vela/authority/records/<id>.dsse.json
.vela/authority/keysets/<root>.json
.vela/authority/policies/<root>.json
.vela/authority/cedar/<root>/...
```

The signed sequence-1 record covers the initialization event, genesis,
repository manifest, keyset, policy, and Cedar material. The event carries
null scientific before/after hashes. Repository authentication grants no
scientific standing.

After replay succeeds, Vela creates one unsigned Git commit containing only
the known bootstrap and canonical genesis paths. It never stages unrelated
files. A clean clone must reproduce the same roots.

### 4. Existing predecessor epochs remain exact

The reader retains schema-dispatched support for existing
`vela.repository-epoch.v1` objects because those signed bytes are active
origins of the four published Frontiers. This is not a migration writer or a
dual scientific runtime:

- there is no command that creates another predecessor epoch;
- there is no archive or equivalence-report writer;
- the predecessor object remains read-only;
- native repositories never acquire predecessor fields;
- current Claim, Submission, Verification, Proposal, Decision, and authority
  semantics are identical in both origin modes.

## Adversarial cases

- A bootstrap containing a current canonical object store is invalid.
- A missing, partial, noncanonical, or mismatched genesis/manifest fails.
- A nonempty native event or actor root fails.
- Profile, principal, reason, action, keyset, policy, binary, or read-set drift
  before signing invalidates the transaction.
- An authentication cancellation or signer refusal produces no epoch,
  manifest, authority event, authority record, or Git commit.
- Duplicate initialization is refused.
- Genesis and predecessor schemas cannot be substituted because the manifest
  binds the full epoch ID and root.
- A record that omits either the genesis or repository postimage fails
  sequence-1 verification.
- Git publication does not establish authority or scientific acceptance.

## Conformance contract

Focused tests must prove:

1. Fresh init writes Profile v2 and none of the retired paths.
2. Bootstrap status and doctor are informative while strict verification
   remains blocked.
3. Native genesis has no predecessor, archive, or equivalence fields.
4. Nonempty history roots and altered profile/object-set roots fail.
5. Initialization requires exactly one event, record, genesis, and manifest.
6. Authentication failure, signer refusal, and transaction drift cause zero
   canonical writes.
7. Successful initialization verifies from a clean clone with identical
   epoch, repository, keyset, and policy roots.
8. Existing predecessor-epoch fixtures and all four active Frontiers replay
   unchanged.

Focused commands:

```bash
cargo test -p vela-protocol native_genesis --lib
cargo test -p vela-protocol current_init_writes_profile_only --lib
cargo test -p vela-cli --test current_genesis
cargo test -p vela-cli authority_transaction
cargo test -p vela-cli current_repository
python3 conformance/verify.py
```

No external Lean, Diderot, live-network, or broad release suite is required
for this decision.

## Consequences

The product has one understandable creation story:

```text
init -> authority init -> commit exists -> work and review
```

New repositories begin in the current object model, while real historical
boundaries remain verifiable. The change removes trust-boundary friction by
eliminating a fake migration ceremony; it does not weaken authority because
strict use remains blocked until the exact repository genesis is signed and
replayable.

## Acceptance

Accepted at the Vela `v0.940.0` release gate. The focused native-genesis,
authority-transaction, current-repository, hostile-conformance, clean-clone,
and all-four-Frontier checks passed. The deterministic release union also
passed with zero failures or warnings; external Lean and live-network lanes
remained explicitly outside this decision.
