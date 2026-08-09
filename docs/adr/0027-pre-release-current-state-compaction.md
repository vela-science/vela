# ADR 0027: Pre-release current-state compaction

- Status: Accepted and implemented on `main`; compaction machinery retired
  2026-08-09
- Retirement: the compaction it performed stands, and every repository it
  produced is still readable. What is gone is the ability to perform another
  one: `RepositoryOriginKind`, `RepositoryOriginPredecessorV1`, the
  `compaction()` constructor and the `pre-compaction/` tag rule were deleted in
  the pre-1.0 standards cut, leaving genesis as the only origin. No live
  repository needed the path — `vela-science/math` is generation 1 at a fresh
  genesis and the epoch-1 repositories are archived — and eleven permanent
  fields on the core origin are the wrong home for a continuity claim that any
  future migration should make as its own signed attestation. This record is
  history for the migration it performed.
- Release: `v0.950.0`
- Protocol effect: the two repository-boundary readers were replaced by one
  current `vela.repository-origin.v1` contract
- Scientific effect: none; the accepted assertion, condition, evidence-content,
  provenance, relation, and Standing projection must be exactly equivalent
- Authority effect: one exact repository-authority approval per Frontier,
  callable from one workspace-root plan; no Claim-by-Claim Decision ceremony
- Compatibility: old repositories remain in exact Git bundles and predecessor
  tags and replay with their pinned old Vela binary; Vela `0.950.0` carries no
  Era-0, `va_`, imported-wrapper, or old-origin reader
- Entry gate: all four controlled Frontiers are current, clean, and replayable;
  there are no external users

## Outcome

The protected aggregate plan
`sha256:f768c16acaaa2dcaa562a49b0e111a794f8983f1bff5783871f8c6f288daef8d`
was published and verified:

| Frontier | Current commit | Origin root | Repository root |
| --- | --- | --- | --- |
| Quantum Codes | `6bc3bacc78942d7d36df60794a211d4a4d750aa3` | `sha256:a52c0aea26726a94b7307b7d07a3ba10b4d6bf4ef1b813cadfebbe69cecb78f1` | `sha256:22a0ef52195d713ddc68c271c5a29de51b54e9b62280103f8acb3f3bcd6b8f1b` |
| Erdős | `81e79f008b4fc653888efda810dd8eb48e50cffa` | `sha256:49969ef6059e636718da4f5b7d200ef421ed17fc60b8e490d493ea71f7b2f77d` | `sha256:8a98ff1c632232c7b227d87a0f1015aaa3429d38c83592ca66f8e465b06b0ee5` |
| Sidon | `ec45b155355769b427f5486c617aad4f68b6ee19` | `sha256:32f743244662048879454a80edc2f1ee915276500ae1f488b1fc1f8e819ae1ac` | `sha256:d047416cce0e569145ae38ae73b8a92102d5c5f63bb46602dff80398cada9a0d` |
| Formal Conjectures | `1ea018d2f5be93325c4e3c7f9b5d82d33e5ba142` | `sha256:aabcbd3a660e5992a13485257df4c8e038c75af2a3e68bf37df16980d390e80a` | `sha256:323269c21ce66b1521d00987ddba2442d69eacfb38bd67d1eb7a96e7644516ca` |

For every Frontier, source `HEAD`, remote `main`, predecessor tag, strict
replay, repository verification, independently retained trust pin, and a
fresh finalizer clone agreed. The one-time compactor and publication commands,
repository-epoch and repository-v2 readers, migration-only Claim and Proposal
fields, and retired Artifact aliases were then removed from the current
runtime.

## Context

The first current-repository epoch correctly preserved authenticated history
while the object model was still moving. It left a small but permanent
compatibility surface in the daily binary:

- `vela.artifact-record.v1` wrappers and `va_` identifiers;
- imported Claim and Proposal provenance fields;
- two repository-boundary schemas;
- reader branches, fixtures, schemas, documentation, and explanations for
  predecessor-only objects.

The active repositories contain 61 imported Artifact wrappers:

| Frontier | Wrappers | Wrapper bytes | Locally retained evidence bytes |
| --- | ---: | ---: | ---: |
| Erdős | 34 | 68,710 | 725,885 |
| Formal Conjectures | 3 | 6,792 | 0 |
| Quantum Codes | 1 | 2,103 | 0 |
| Sidon | 23 | 26,584 | 1,717,170 |

The implemented read-only audit passes over all four exact current
repositories without writing:

| Frontier | Artifact map root | Accepted Claims | Relations | Equivalence report root |
| --- | --- | ---: | ---: | --- |
| Erdős | `sha256:2990e5614323261628bf3ceca39210bb7bc9128d75dcc00d99f04f7af1db4ad2` | 2,771 | 1,282 | `sha256:522529bbd71c35bad244f0691675ec381ad30f434f0c1218e2e4325b82fd0617` |
| Formal Conjectures | `sha256:4aacc14965287f5eb18182c6322c4d0cfa29c30d6bc8e7bce06b44726eefcac5` | 14 | 0 | `sha256:2b96ac51fe3d6d9adca0d2394f43b8e07d294c6c8d5f06de5d843bee93a2b455` |
| Quantum Codes | `sha256:f5307e93f87bf1bf913ca1701848d2035b14253dec537dbd69a645321fd3e269` | 5 | 0 | `sha256:6e82d595254d09e02bd4997d7f4cafcf1849e79a2c8577f2f8a95446e2ee430b` |
| Sidon | `sha256:a03f320140a58c42b39cb39076b658bd964b703264855e7b54133e1a7a8e2f72` | 40 | 0 | `sha256:aaf20dc5162c67d117bc62776a6af6d8ae2d5f36bb8dd5febe84f68503611bbe` |

These are preview roots. They remain non-authoritative and may change when the
candidate-file and repository-origin layers become part of the rooted plan.

The source-only compactor now also materializes and independently re-reads the
complete candidate repository outside each Frontier. It rejects extra,
missing, substituted, non-canonical, legacy-wrapper, or `va_`-referencing
files. Every candidate includes its deterministic predecessor Git archive,
the exact predecessor commit/tree/remote/tag binding, predecessor
object manifest, single origin, v3 repository manifest, rebuilt objects, and
equivalence report. Two independent materializations produced identical plan
roots. The first rooted set is retained under
`~/Desktop/Constellate/Archives/vela-current-compaction-candidates-2026-07-29-final-v2/`:

| Frontier | Candidate plan root | Origin root | Repository root | Files |
| --- | --- | --- | --- | ---: |
| Erdős | `sha256:56499890c7f302575bb1ef411c93145bfad985f4e3abf852e2872cfafdf108c6` | `sha256:49969ef6059e636718da4f5b7d200ef421ed17fc60b8e490d493ea71f7b2f77d` | `sha256:8a98ff1c632232c7b227d87a0f1015aaa3429d38c83592ca66f8e465b06b0ee5` | 2,811 |
| Formal Conjectures | `sha256:c53a6f19accb5f4bcdb6a2d67626c112a809ab159ff2703c0b0cc600c7384922` | `sha256:aabcbd3a660e5992a13485257df4c8e038c75af2a3e68bf37df16980d390e80a` | `sha256:323269c21ce66b1521d00987ddba2442d69eacfb38bd67d1eb7a96e7644516ca` | 23 |
| Quantum Codes | `sha256:1e18fc5bd40f46d126088098175909c4153979c369bb1d6ff2eb48cfea371c0c` | `sha256:a52c0aea26726a94b7307b7d07a3ba10b4d6bf4ef1b813cadfebbe69cecb78f1` | `sha256:22a0ef52195d713ddc68c271c5a29de51b54e9b62280103f8acb3f3bcd6b8f1b` | 12 |
| Sidon | `sha256:f9d083627ac05920d6555c013611ef0c2200f20deaecdf6fbcd0cd74272e0a2c` | `sha256:32f743244662048879454a80edc2f1ee915276500ae1f488b1fc1f8e819ae1ac` | `sha256:d047416cce0e569145ae38ae73b8a92102d5c5f63bb46602dff80398cada9a0d` | 69 |

Materialization writes no source-Frontier byte. Every origin now binds the
final archived authority event-log root rather than the predecessor epoch's
initial root, and the plan and origin share one canonical reason. A
credential-disabled activation rehearsal on Quantum Codes proved that a wrong
confirmation root creates no worktree, while a correct root stages the six
candidate records, removes every predecessor-only path, and fails safely
before signing when `SSH_AUTH_SOCK` is absent. The rehearsal worktree was
removed and the source checkout remained clean. These plan roots still stop
before the repository-authority transaction postimage; they are candidate
evidence, not authorization to publish.

The first protected Erdős activation then failed its postcondition before
commit or publication. The empty archived-predecessor history path returned
the empty legacy event root instead of the exact archived event root, so the
new sequence-1 record and the compact-origin verifier disagreed. The source
checkout and remote remained at the exact predecessor commit. The preserved
worktree contained no authority record, origin, repository manifest, commit
marker, or new Git commit and was removed after that audit. The verifier now
preserves the archived root in its empty-history result, and a deterministic
transaction test signs and re-verifies sequence 1 against that root. All four
candidate packages re-verify unchanged; their plan roots above remain valid.

The second protected invocation also failed before the commit marker. The
fresh-authority write gate recognized only the native
`repository-genesis + repository.v2` shape and rejected the planned
`repository-origin(compaction) + repository.v3` shape. The preserved worktree
contained no authority bytes, origin, repository manifest, or new commit; all
2,803 untracked candidate objects matched the already rooted candidate
package byte-for-byte, and the source and remote stayed at the predecessor
commit. The write gate now accepts exactly those two complete initialization
shapes, rejects every mixture, requires a compaction origin for v3, and binds
the Frontier, Profile, origin ID/root, and retained object-set root before a
commit marker can exist.

The third protected invocation reached signing but still stopped before its
commit marker. The initial authorization correctly recognized the empty
post-removal bootstrap. After the compactor materialized and read-bound the
retained records, marker-time revalidation incorrectly reapplied that native
empty-bootstrap predicate and rejected the exact compaction surface. No
origin, repository manifest, canonical commit, source update, or remote update
occurred. One signed sequence-1 record exists only inside an aborted private
journal, retained under
`failed-activations/erdos-vop-b95ad35c6197622e/` in the candidate archive. Its
payload-manifest root is
`sha256:1e4b21bb5ba42d1c90710080bb4aafc6745865f7ef554122179eb2d74b89e750`.
All 2,805 materialized records match the rooted candidate bytes exactly.

Compaction initialization now has its own closed authorization surface. It
accepts only the complete expected `records/` set, binds its canonical
object-set root into the authorization context, requires the compact-v3
transaction delta to carry the same root, and repeats the same exact check
before the marker. Missing, substituted, duplicate, outside-`records/`, or
unexplained files fail closed. The native bootstrap predicate remains
unchanged and continues to reject any pre-authority records. A deterministic
nonempty compact-v3 transaction now exercises prepare, marker-time
reauthorization, exact installation, and completion without a real credential.

The disk cost is immaterial. The cost is a larger protocol vocabulary and
trust base before Vela has external users. Keeping compatibility indefinitely
would optimize for a release history that does not yet exist.

Deleting the wrappers in place is still invalid. Current accepted Claims name
their exact wrapper IDs and roots. Blind deletion would change their support
identity without a deterministic mapping or equivalence proof.

## Decision

### 1. Compact meaning once, then delete compatibility

A source-only compactor builds a new isolated candidate for each controlled
Frontier. It never edits the source checkout and is removed before the final
release.

For every imported Artifact wrapper it emits exactly one current evidence
object:

1. `local_blob`: verify the retained bytes against the declared full digest and
   store the exact bytes at `records/artifacts/sha256/<digest>`;
2. `remote` or `pointer`: retain the exact canonical wrapper bytes as an opaque
   content-addressed Artifact at
   `records/artifacts/sha256/<wrapper-byte-digest>`. This preserves the only
   bytes actually held without pretending that the remotely located source was
   deposited.

Both forms are opaque content-addressed Artifacts to Vela. The repository
manifest uses the full lowercase 64-hex content digest as the Artifact ID.
Vela does not add an Artifact authority schema or parse a retired wrapper.
Historical identifiers that occur inside opaque retained evidence bytes have
no routing or protocol meaning.

Every retained Claim is rebuilt with:

- exact full-hash Artifact IDs, roots, and paths;
- unchanged assertion, conditions, evidence relation, provenance, revision,
  and creation time;
- relations remapped to rebuilt accepted Claim identities; a relation to a
  non-standing historical Claim keeps its full Claim ID only when that exact
  target exists in the bound predecessor archive;
- no `imported_from` field or legacy extension.

The Claim mapping is deterministic. Claim identity excludes relations, so the
compactor computes identities from normalized support first and remaps the
relation graph in a second pass. Missing bytes, digest disagreement, duplicate
maps, a relation absent from both candidate and predecessor, ambiguous remote
identity, or a mapping cycle that cannot be represented fails closed.

### 2. Preserve only live current operations

The compactor retains every accepted Claim and its locally available evidence
Artifacts. It retains only pending Proposal -> signed Submission ->
Verification closures whose exact bytes already reference the normalized
Claim and Artifact identities and remain valid without a predecessor reader.

Everything else is archived with the predecessor:

- terminal accepted or rejected operations already reflected in Standing;
- stale, rejected, withdrawn, or superseded work;
- pending work whose authenticated bytes bind replaced identities.

Valid pending work remains live through its exact direct lineage. Work whose
authenticated bytes bind replaced identities is resubmitted through the
ordinary current producer path after compaction. The compactor never rewrites
a producer, verifier, Decision, or authority signature.

### 3. Prove Standing equivalence

`vela.current-state-equivalence.v1` is a canonical, path-independent report
over the predecessor and candidate. It contains:

- exact predecessor Git commit, tree, repository root, accepted-set root,
  authority head, and archive digest;
- exact candidate Claim and Artifact roots;
- a total predecessor-to-candidate Claim map;
- a total legacy-to-content Artifact map;
- per-Claim comparison of assertion, conditions, evidence content,
  provenance, relations, and Standing;
- archived live-object dispositions;
- before/after accepted counts and relation counts;
- an overall `equivalent` result.

The report passes only when the scientific projection is bijective and every
accepted Claim remains accepted with the same bounded meaning and evidence
content. Text normalization, evidence deletion, standing changes, duplicate
collapse, dangling relations, or unexplained additions fail.

### 4. Use one repository-origin contract

After compaction the shipping reader recognizes only
`vela.repository-origin.v1`:

```text
schema
origin_id
frontier_id
generation
profile_root
initial_object_set_root
kind = genesis | compaction
predecessor = null | {
  remote
  tag
  commit
  tree
  repository_root
  authority_head_root
  archive_sha256
  object_manifest_root
  equivalence_report_root
}
reason
```

Genesis requires a null predecessor, generation 1, and an empty initial object
set. Compaction requires the exact predecessor block, generation greater than
1, and a passing equivalence report.

One sequence-1 repository-authority transaction covers the origin, candidate
repository manifest, current keyset and Cedar policy, exact mapping and
equivalence roots, and complete postimage. Its scientific before/after
projection roots are equal even though canonical object roots differ. It does
not make a new scientific Decision.

The workspace flow prepares all four isolated candidates before credential
use. Each protected activation binds one exact plan and produces only an
isolated signed commit. `repository finalize-compaction --check` then
re-verifies every result, candidate, compacted repository, source checkout,
authority root, and remote precondition and derives one aggregate root.
Publication requires that exact aggregate root. It pushes the predecessor tag
and compacted main update atomically per repository, supports exact resume
after a cross-repository partial failure, fast-forwards clean source
checkouts, advances independently retained authority pins from their exact
preimages, and verifies fresh remote clones. No source or remote is changed
until every supplied activation passes the aggregate preflight.

### 5. Remove the bridge before release

After all four published Frontiers replay from clean clones:

- remove the compactor and all upgrade commands;
- remove `vela.artifact-record.v1` and `CurrentArtifactRecordV1`;
- reject `va_` in Claim, Registration, and Verification records;
- remove `imported_from` and migration-only extensions from active schemas;
- remove repository-epoch v1 and repository-genesis v1 readers;
- remove legacy fixtures, schemas, aliases, tests, help, and active docs;
- retain only the exact source archives, predecessor tags, equivalence reports,
  and pinned historical binary manifest outside the active object model.

The final release must contain no code path that creates or reads the retired
objects.

## Rejection conditions

Reject the compaction if:

- any accepted assertion, condition, evidence content, provenance, relation,
  or Standing changes;
- exact local evidence bytes are missing or fail their declared digest;
- an external reference cannot be made exact without a mutable lookup;
- a producer, verifier, Decision, or authority signature would be rewritten;
- an unrelated tracked byte changes;
- any candidate cannot replay from a clean clone;
- the old reader remains necessary after cutover.

## Verification

Focused tests must prove:

1. all 61 wrappers receive one exact normalized disposition;
2. local bytes and remote-reference bytes are deterministic;
3. Claim identity and relation remapping are order-independent;
4. missing, substituted, or ambiguous evidence fails;
5. the accepted projection is bijective and unchanged;
6. pending authenticated objects are retained only when still exact;
7. a failed candidate never changes a source checkout;
8. the aggregate workspace plan cannot mix candidate roots;
9. each compacted Frontier passes strict and clean-clone replay;
10. the final source tree contains no `va_`, Artifact-wrapper, old-origin, or
    compaction runtime outside the archived release evidence.

## Consequences

Vela takes one deliberate pre-release compatibility break and then has one
current product story:

```text
Target -> Attempt -> Submission -> Verification -> Decision -> Standing
```

Evidence uses full content identities. Old systems remain auditable artifacts,
not permanent runtime dependencies.
