# Vela roots and identifiers

This document names the commitment domains that Vela already implements. It
adds no schema, event, reducer rule, accepted-state rule, or migration. Its
purpose is to prevent a digest, identifier, Git object, signature, or derived
projection from being used as a substitute for a different one.

The protocol and conformance vectors remain the source of truth for byte-level
behavior. If prose here and a released conformance vector disagree, the vector
wins.

## Rules

1. A trust-boundary comparison needs the **root kind**, the **canonicalization
   profile**, and the **complete digest**. The text `sha256:` identifies only an
   algorithm and encoding; it does not identify what was hashed.
2. `vf_`, `vfr_`, `vev_`, `vpr_`, `va_`, `vrc_`, and similar 16-hex identifiers
   are readable routing handles. They are not full security identities. A
   consumer must reject an ambiguous handle and use the corresponding full root
   when authorization, replay, or cross-repository handoff depends on identity.
3. Roots from different rows in the table below are never equal *as typed
   commitments*, even if their 64 hexadecimal characters happen to match.
4. Git publication is transport history, not scientific acceptance. A
   signature proves control of a registered key over an exact signing input,
   not truth. A verifier pass is evidence, not authority.
5. Derived views and site bundles must name the canonical roots from which they
   were produced. Regeneration may change projection bytes without changing
   canonical scientific or authority history.
6. Missing, malformed, truncated, multiply resolving, or differently typed
   roots fail closed. They are unknown or unresolvable, never an approximate
   match.
7. Repository bytes cannot select their own first administrator. A Profile v1
   boundary chain also needs the consumer's exact out-of-band first-boundary
   pin before it can authorize a canonical write.

## Canonicalization profiles

### Vela canonical JSON

Most Vela objects use `canonical::to_canonical_bytes`: object keys are sorted
lexicographically at every depth, arrays retain their declared order, tokens
have no intervening whitespace, strings use standard JSON escaping while
preserving non-ASCII UTF-8, and numbers use `serde_json`'s round-trip format.
Non-finite values are rejected. This profile is pinned by
`conformance/canonical-hashing.json`; it is deliberately described as Vela
canonical JSON rather than claimed to be universal RFC 8785 JCS.

### Receipt v1 JCS

Receipt v1 uses RFC 8785 JCS through its receipt-specific canonicalizer. The
parser rejects duplicate JSON names and values outside its bounded validity
set. Integers outside the exactly portable binary64 range
`[-(2^53-1), 2^53-1]` are rejected. The Receipt root commits to the complete
validated Receipt, including accepted namespaced extensions.

### Exact bytes, Git objects, and RFC 6962

Some commitments hash exact file or artifact bytes rather than JSON. Git commit
and tree object IDs use Git's own object format and the repository's configured
hash algorithm. Transparency roots use RFC 6962 domain separation and are
meaningful only with their tree size. These profiles are not interchangeable
with either JSON profile.

## Commitment and identifier catalogue

| Name | Derivation and role | Required comparison | Not a substitute for |
| --- | --- | --- | --- |
| Frontier handle `vfr_…` | First 16 hex characters of the SHA-256 commitment to the canonical `frontier.created` event preimage. Legacy frontiers fall back to a canonical metadata preimage. | Exact handle inside a replay-validated frontier; pair with Git and frontier roots at a handoff. | Event-log root, Git identity, or proof that a checkout is current. |
| Profile root | `sha256:` plus SHA-256 of Vela-canonical JSON for the parsed, closed `vela.frontier-profile.v1` value. YAML comments, whitespace, key order, and quoting are erased before hashing; array order remains semantic. | Full root plus successful closed-profile parsing and an independently derived matching Frontier ID. | Frontier identity, scientific state, maintainer authority, or accepted standing. |
| Frontier identity root | `sha256:` plus SHA-256 of the closed `vela.frontier-identity.v1` record derived from the exact Profile v1 genesis or signed legacy boundary. | Full root rederived from the complete valid identity-event chain. | Readable `vfr_` handle, profile root, administrator authentication, or scientific-state root. |
| Dependency root | `sha256:` plus SHA-256 of the Vela-canonical sorted list of closed exact Frontier dependency records. Each entry binds Frontier ID, identity root, scientific-state root, Git object format, commit, and tree. | Full root, exact ordered entries, and successful resolution of every named Git and Vela root. | A remote URL, mutable ref, availability promise, transfer of standing, or scientific support edge. |
| Repository-boundary content root | The full event-content root of one signed `frontier.repository_bound` event. It binds identity/dependencies, administrator, exact Git/event/state anchor, actor registry, artifacts, and retained canonical bytes. | Full root, valid signature and linear boundary chain, complete repository-context checks, and the consumer's first-boundary pin. | Event ID, event-log root, timestamp-based membership, acceptance, or a self-authenticating administrator. |
| Repository trust-anchor root | SHA-256 of the closed local `vela.repository-trust-anchor.v1` record containing Frontier identity, first-boundary content root, administrator actor, and public key. | Full root of the atomically installed, out-of-band reviewed record under the operating-system account home. | Repository content, a secret key, universal trust, claim authority, or scientific state. |
| Scientific-state root v2 | `sha256:` plus SHA-256 of the closed `vela.scientific-state.v2` component-root record. It binds identity, dependencies, and the explicitly named scientific collections; every component binds canonical JSON, including empty arrays. | Full root plus rederivation of every named component from the replayed Project. | Legacy snapshot root, event/proposal/actor/policy roots, Git identity, graph position, or accepted standing. |
| Finding handle `vf_…` | First 16 hex characters of SHA-256 over the normalized assertion type, text, and selected provenance identity. It routes to a claim lineage. | Exact, unique handle plus the full finding revision root when the revision matters. | A full revision commitment, acceptance, or proof. |
| Finding revision root | `sha256:` plus SHA-256 of Vela-canonical `FindingBundle` bytes with mutable relationship `links` cleared. It commits to the current claim-bearing bundle. | Full root in the finding-revision domain. | Stable `vf_` handle, event-log root, or standing. |
| Evidence-atom root | `sha256:` plus SHA-256 of the Vela-canonical evidence atom. | Full root in the evidence-atom domain. | Artifact bytes, a verifier attachment, or a finding revision. |
| Artifact handle `va_…` | First 16 hex characters of a descriptor preimage. Older descriptors use a delimited string; descriptors with explicit reference axes use Vela canonical JSON. | Exact, unique handle only for descriptor routing. | Artifact byte digest, availability, disclosure status, or review standing. |
| Public artifact digest | SHA-256 of the exact retained or retrieved public bytes. Retained local artifacts live under `records/artifacts/sha256/<digest>`. Restricted artifacts intentionally expose no equality digest. | Full digest and the artifact descriptor's media/kind and size or immutable locator constraints. | Artifact handle, Receipt root, or permission to disclose restricted material. |
| Receipt root | `sha256:` plus SHA-256 of the complete Receipt v1 JCS bytes. | Full root after strict Receipt v1 validation. | Landing record, proposal, verifier result, policy verdict, or acceptance. |
| Producer identity-binding handle `vib_…` and full credential root | First 16 hex characters, or the complete lowercase SHA-256 root, over the Vela-canonical `vela.identity_binding.v0.1` object with `binding_id` and `signature` cleared. The self-signature separately proves possession of the embedded Ed25519 key. | `vib_` is an exact routing handle. The complete root is the credential identity used by the accepted AcceptancePolicy v0.3 scoped allowlist. | Actor-registry membership, personhood, expertise, independence, or authority without an exact human-signed policy. |
| Landing record handle `vrc_…` | First 16 hex characters of SHA-256 over the Vela-canonical `ActivityRecord` with its ID and signature cleared. The record points to the full Receipt root. | Exact, unique handle plus retained record bytes and Receipt root. | Receipt root or scientific claim identity. |
| Proposal handle `vpr_…` | First 16 hex characters of SHA-256 over the Vela-canonical logical proposal preimage. `created_at` and mutable review status are excluded so exact retries are idempotent. | Exact, unique handle plus the full proposal root for a decision or withdrawal. | Proposal root, Decision Plan, event, or accepted-state change. |
| Proposal root | `sha256:` plus SHA-256 of the exact Vela-canonical `StateProposal` bytes at the operation boundary. | Full root and exact proposal handle/status. | Aggregate proposal root or Decision Plan root. |
| Proposal aggregate root | `sha256:` plus SHA-256 of the Vela-canonical ordered proposal array. Reported by `vela status`. | Full root for the complete proposal collection and the frontier checkout being inspected. | Any individual proposal root. |
| Event handle `vev_…` | First 16 hex characters of the full event-content digest. Its preimage includes schema, kind, target, actor, timestamp, reason, before/after roots, payload, and caveats; it excludes the event ID and signature. | Exact, unique handle plus the full event-content root for authority inspection or external handoff. | Full event-content root, signature verification, or event-log membership. |
| Event-content root | `sha256:` plus SHA-256 of the same Vela-canonical content preimage used for `vev_…`. | Full root in the event-content domain. | Event-log root, event signature, or proposal root. |
| Event signature | Ed25519 verification over the versioned canonical signing input defined by the event/signing contract. It is deliberately outside event content addressing. | Registered actor, algorithm/version, exact signing input, and signature verification. | Content root, scientific truth, registry membership, or current authority. |
| Event-log root | `sha256:` plus SHA-256 of the Vela-canonical array of all events sorted by `event.id`, with each top-level signature removed. Replay order is derived separately. | Full root and event count; strict replay and signatures are checked independently. | Transparency root, Git tree, snapshot, or proof that every actor signature is valid. |
| Non-lease event-log root | The event-log algorithm after excluding only `attempt.claimed` coordination events. It pins the proof subject while leases change. | Full root explicitly typed as non-lease. | Full event-log root or a general rule for excluding future event kinds. |
| Transparency root | RFC 6962 Merkle tree root over event-content preimage bytes: empty `SHA256("")`, leaf `SHA256(0x00 || leaf)`, node `SHA256(0x01 || left || right)`. | Full root, tree size, and the applicable inclusion or consistency proof. | Event-log root; the trees have different ordering and domain separation. |
| Snapshot root | `sha256:` plus SHA-256 of the Vela-canonical project after removing top-level `events`, `signatures`, and `proof_state`. | Full root and the event-log root from which the snapshot is replayed. | Event-log root, Git tree, or accepted-state authority by itself. |
| Legacy snapshot root | The v0.1 snapshot algorithm above, named explicitly in Profile v1 compatibility and migration output. | Full root under the historical schema and the exact boundary that anchored it. | `scientific_state_root`, even when both describe the same pre-migration history. |
| Actor-registry root | SHA-256 of the exact `.vela/actors.json` bytes. Legacy fallback uses Vela-canonical actor-array bytes when the file is absent. | Full root and the explicitly identified source form. | Proof of key possession, event signature, or temporal registration boundary. |
| Policy bytes/root | Exact policy bytes or the explicitly named Vela-canonical policy/head preimage, depending on the field. Policy observations separately bind policy and signature byte roots. | The exact field's schema/domain, full root, policy ID/version, and signature/authorization checks. | Policy Decision Plan, policy-head event, or permission inferred from a merely present file. |
| Review Decision Plan root | SHA-256 of `"vela.decision-plan.internal.v1\0" || canonical-preimage`, returned as `sha256:…`. It binds the frontier root, ordered answers, consumed facts, policy inputs, and semantic event cores. | Full root, observation/confirmation time, pinned binary, and complete revalidation immediately before protected signing. | Proposal root, signature, or reusable approval. |
| Policy Decision Plan root | SHA-256 of `"vela.policy-decision.v1\0" || canonical-plan-without-self-root`, returned as `sha256:…`. | Full root and the same late revalidation requirements as the protected policy path. | Review Decision Plan or active policy root. |
| Git commit | Git object ID for `HEAD^{commit}`. It binds a commit object, its tree and parents according to Git. | Complete object ID, repository, and ancestry/pin expectations. | Git tree, event-log validity, signatures, or scientific acceptance. |
| Git tree | Git object ID for `HEAD^{tree}`. It binds tracked paths and bytes but not history. | Complete object ID and repository hash format. | Commit ancestry, untracked state, event replay, or currentness. |
| Target input root | `sha256:` plus SHA-256 of the closed `vela.target-index-input-manifest.v1`, whose sorted entries bind source-commit path, mode, size, and digest. | Full root plus resolution of every entry from the exact source Git tree. | Proof that a domain generator disclosed every input, target-index root, or packet digest. |
| Target-index root | `sha256:` plus SHA-256 of the complete canonical `vela.target-index.v2` object with only `index_root` omitted. | Full root, closed-schema validation, exact source/input/packet/repository roots, and freshness at the operation edge. | Work authority, scientific standing, graph rank, or a historical task binding. |
| Target-task binding root | `sha256:` plus SHA-256 of the complete canonical `vela.target-task-binding.v1` with only `binding_root` omitted. It retains the exact target, index, packet, source, repository roots, and claim-time read set. | Full root in both the private session and the byte-identical Receipt v1 extension. | A current offer, verifier result, policy Permit, or accepted state. |
| Projection or site-bundle root | A non-authoritative projection's own documented canonical or exact-byte digest, together with its bound source roots. | Projection schema/version, full projection root, and exact source roots. | Any source root or a claim that the projection is live. |
| Release checksum | SHA-256 of exact binary or archive bytes. | Full checksum, artifact name/platform, release version, and trusted publication source. | Git commit, package integrity metadata, code signature, or build provenance. |
| Build attestation | A signed provenance statement binding a produced artifact digest to a workflow and source identity. | Attestation verification, expected issuer/workflow/source, and the artifact digest. | Artifact checksum, platform code signature, scientific signature, or frontier authority. |

## Comparison and substitution contract

- Full Vela SHA-256 roots are lowercase `sha256:` followed by exactly 64
  hexadecimal characters. Bare hexadecimal digests are normalized only where a
  specific released field says so; callers must not guess.
- A stored or transmitted root must be interpreted using its containing schema
  and field name. Generic `root`, `hash`, or `digest` values need an explicit
  domain before they can cross a trust boundary.
- Handle resolution must produce exactly one object whose full commitment
  rederives. Zero matches are unresolvable; more than one is ambiguous. Prefix
  equality is never a fallback authorization rule.
- Event timestamps and `created_at` metadata do not establish set membership or
  registration eligibility. Membership comes from exact committed history.
- Changing only a valid event signature can leave the `vev_` handle,
  event-content root, event-log root, and transparency leaf preimage unchanged.
  The new signature must still verify independently.
- A re-materialized snapshot or projection may legitimately have different
  bytes while canonical events and authoritative roots remain unchanged. A
  canonical event, proposal, Receipt, registration, artifact, or signed policy
  may not be silently rewritten as “materialization.”
- Git ancestry answers whether one published history descends from another.
  Vela roots and replay answer whether the contained scientific and authority
  history is valid. Claims of continuity need both when both questions matter.
- Profile v0.1 `snapshot_hash` and Profile v1 `scientific_state_root` are
  different typed commitments. Migration records the former as
  `legacy_snapshot_root` and derives the latter; no implementation substitutes
  one for the other in a historical signature or Decision Plan.

## Adversarial examples

- Two objects share the same 16-hex prefix. Selecting either by the short handle
  is an ambiguity and fails; a caller does not choose the first file returned.
- A valid Git commit contains a malformed or forged event. Git proves the bytes
  and ancestry, while Vela replay and strict checks reject their semantics.
- An archive has a valid checksum but is from a different platform or release.
  The checksum does not satisfy the expected artifact identity or attestation.
- The same 64 hexadecimal characters appear in a Receipt-root field and an
  event-log-root field. The typed commitments remain different.
- An event signature is stripped or corrupted while its content root is
  unchanged. Strict signature checks still block where a signature is required.
- `.vela/actors.json` is reserialized without changing its parsed actors. Its
  exact-byte registry root changes, exposing the edit instead of normalizing it
  away.
- A site bundle is internally valid but names an old frontier commit. It is a
  valid historical projection, not a current or live view.
- A checkout contains a valid signed boundary but no independently installed
  consumer pin. Its internal chain may be well formed, but it has not selected
  which first administrator fork the consumer intended and cannot authorize a
  canonical write.
- A valid target index names a source commit that is no longer an ancestor, or
  one packet differs from its tracked blob. The affected work is stale and
  unactionable even if the index's own JSON root still rederives.

## Conformance anchors

Implementations should run the focused contracts that own these domains:

```bash
cargo test -p vela-protocol --test canonical_hashing_conformance
cargo test -p vela-protocol receipt_v1
cargo test -p vela-protocol event_log_hash_is_independent_of_input_order
cargo test -p vela-protocol rfc6962_canonical_ct_vectors
cargo test -p vela-protocol scientific_state_root_v2
cargo test -p vela-protocol --test frontier_profile_loader_v1
cargo test -p vela-edge target_index
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

The canonical JSON fixtures are also checked by
`conformance/verify_canonical_hashing.py`. A new implementation must reproduce
the exact bytes and roots, not merely agree with itself.
