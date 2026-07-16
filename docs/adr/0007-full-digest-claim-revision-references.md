# ADR 0007: Full-digest claim revision references

- Status: Proposed
- Candidate release: Vela `v0.801.0`
- Entry gate: ADR 0006 demonstrates an identity gap

## Context

Findings already pair a readable `vf_` handle with a full `finding_hash()`
revision root. ADR 0002 deferred stable identities for other object families.
ADR 0004 requires full roots for every security-relevant handoff comparison.

Current CLI and link surfaces still expose shortened identifiers as convenient
selectors. A cross-frontier consumer needs one closed reference that pairs the
logical finding handle with the exact revision it consumed. The protocol must
not treat a 64-bit display handle as collision-resistant authority.

## Proposed decision

Define the closed reference:

```text
vela.claim-revision-ref.v1 {
  frontier_id
  finding_id
  finding_revision_root
}
```

Rules:

- `frontier_id` and `finding_id` select a candidate finding.
- `finding_revision_root` uses `sha256:<64 lowercase hex>`.
- A consumer re-derives the full root from exact finding bytes.
- Missing or mismatched full roots fail closed.
- Short handles remain display and routing aliases.
- A collision between shortened handles produces ambiguity, never selection by
  iteration order.

The reference creates no new scientific object family, authority, signature,
or accepted-state transition. It is a typed security reference derived from
existing frontier state.

## Read-only resolution

Add:

```bash
vela resolve \
  --repo <path-or-url-or-bundle> \
  --root <git-commit> \
  --claim <vf-id> \
  --json
```

The command:

1. checks out or reads the exact Git root;
2. runs strict replay;
3. locates the finding by handle;
4. derives and verifies the full revision root;
5. renders exact claim text and caveats;
6. lists artifact descriptors and full roots;
7. lists verifier attachment IDs and full canonical roots;
8. identifies the authority event and its full content root;
9. reports current standing at that root; and
10. emits one `ClaimRevisionRef`.

The command does not fetch mutable branches after root selection, execute
producer artifacts, accept state, or discover future corrections.

## Migration and compatibility

Old frontiers replay unchanged. New binaries derive a reference from existing
finding bytes. Historical links, events, proposals, and Receipt records keep
their current identifiers.

New handoff profiles require the full reference. Legacy short-only inputs remain
readable as local selectors but cannot satisfy strict external resolution.

Older binaries may ignore the new command and schema. They continue to replay
frontiers because this ADR adds no event kind.

## Adversarial cases

Conformance covers:

- missing revision root;
- uppercase, truncated, or malformed root;
- correct handle with wrong revision root;
- correct revision root under the wrong frontier;
- two candidates with the same shortened alias;
- changed claim, caveat, evidence, or conditions;
- missing finding bytes;
- stale derived view with correct event log;
- unknown fields in the closed reference; and
- map-order differences across implementations.

## Conformance

```bash
cargo test -p vela-protocol claim_revision_ref
cargo test -p vela-cli --test handoff_workflows resolve
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

The release also requires a clean-clone and Git-bundle resolution fixture.

## Consequences

Consumers gain an injective full-root reference without rewriting Vela's
existing identifiers. ADR 0002 remains in force for proposals, records,
attachments, and events; their handoff references pair readable IDs with full
canonical roots rather than adding stable IDs.
