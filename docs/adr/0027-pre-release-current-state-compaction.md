# ADR 0027: Pre-release current-state compaction

- Status: Proposed
- Target release: Vela `v0.950.0`
- Protocol effect: replace the two repository-boundary readers with one
  current `vela.repository-origin.v1` contract after all controlled Frontiers
  are compacted
- Scientific effect: none; the accepted assertion, condition, evidence-content,
  provenance, relation, and Standing projection must be exactly equivalent
- Authority effect: one exact repository-authority approval per Frontier,
  callable from one workspace-root plan; no Claim-by-Claim Decision ceremony
- Compatibility: old repositories remain in exact Git bundles and predecessor
  tags and replay with their pinned old Vela binary; Vela `0.950.0` carries no
  Era-0, `va_`, imported-wrapper, or old-origin reader
- Entry gate: all four controlled Frontiers are current, clean, and replayable;
  there are no external users

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
`~/Desktop/Constellate/Archives/vela-current-compaction-candidates-2026-07-29-tar-a/`:

| Frontier | Candidate plan root | Origin root | Repository root | Files |
| --- | --- | --- | --- | ---: |
| Erdős | `sha256:b6f77828df6e94e14c342b5488eb30540cc2257edb1299086c1d981de7625d23` | `sha256:500845f209cb527587ce88bae5645d2eb3e6fba2b3c80c79e03d007d27a02405` | `sha256:1cd16496c740d71bfe50e3eaf40a6637578eec11846836babeeab05876a64488` | 2,811 |
| Formal Conjectures | `sha256:9e44e062564879d9909fc4cec10a1a54800889578c419fcf3d2daecc865a0d55` | `sha256:decf16565a7cd4c188fb7b12925427b03569733538e2457f8943f34cf735158d` | `sha256:b775b469532b288a71022f2af1b92643442e121fdfdbcc096a582243f9966cf3` | 23 |
| Quantum Codes | `sha256:e3ae75e4527516b9a8c0c48a65309ae51113849f0bf881ebc658b7cf4ee6cfe0` | `sha256:f6a8cd5809d18a31307151767ee364db830afd60420ec44c2664eb4c0ce627dd` | `sha256:794fd809da3ac0e2c04070eebcd441f1072bb238e3f3c32d65e8ae43b80a5b63` | 12 |
| Sidon | `sha256:b5bf3cc4f628615158a271481e02ea49a2af0be977454411a6a2413257cba81b` | `sha256:262c52bf24a07feeef9729d43ff15ccfb7272b7d943f0761045cb5779a01d03c` | `sha256:3ca35241e9de5316e116c84fe9b8a6e1f2385cbacf5dde3efef1b253dc35e8ee` | 69 |

Materialization writes no source-Frontier byte. These plan roots still stop
before the repository-authority transaction postimage; they are candidate
evidence, not authorization to publish.

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

The compactor retains a Submission, Registration Record, Verification Record,
Proposal, or Decision only when its exact bytes already reference the
normalized Claim and Artifact identities and remain valid without a legacy
reader.

Everything else is archived with the predecessor:

- terminal operations already reflected in accepted Standing;
- stale, rejected, withdrawn, or superseded work;
- pending work whose authenticated bytes bind replaced identities.

Still-useful pending work is resubmitted through the ordinary current producer
path after compaction. The compactor never rewrites a producer or verifier
signature.

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

The workspace command prepares all four isolated candidates and one aggregate
plan root before any credential use. The human invokes the exact confirmed
plan once. Publication begins only after every candidate signs and verifies;
partial results remain isolated and recoverable.

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
