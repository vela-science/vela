# ADR 0024: Product monorepo and integration-repository retirement

- Status: Accepted
- Accepted: 2026-07-28
- Release evidence: Vela `0.940.9`, `@vela-science/protocol@0.1.0`,
  `@vela-science/canopus@0.8.0`
- Protocol effect: none
- Product effect: one public source repository for Vela, the TypeScript
  protocol SDK, Canopus, shared schemas, fixtures, and product CI
- Authority effect: none
- Compatibility: Git history, tags, releases, package names, Frontier bytes,
  and public interfaces remain intact

## Context

`vela-science/vela-internal` was created to compose exact Vela, Canopus, Web,
and Frontier sources. It now mixes release pins, conformance, campaign
coordination, private research, historical reports, and duplicated canon. It
produces no installable distribution.

Canopus is independently packaged, optional, and replaceable, but it changes
with Vela frequently and hand-copies Vela's public protocol types,
canonicalization rules, release versions, binary hashes, fixtures, and CI
inputs. The repository split has not created a stronger trust boundary. It has
created multiple sources of truth for one public contract.

The current topology therefore causes three recurring failures:

1. public users must discover a private meta-repository to understand the
   product;
2. release and compatibility facts drift across Vela, Canopus, Web, CI, and
   the parent; and
3. repository separation is mistaken for authority separation even though the
   real boundary is the public Submission, Verification, and Decision contract.

## Decision

### 1. Use one public product monorepo

The target active topology is:

```text
vela-science/vela
vela-science/vela-web
vela-science/erdos-frontier
vela-science/formal-conjectures-frontier
vela-science/sidon-frontier
vela-science/quantum-codes-frontier
vela-science/.github
```

The public Vela repository contains:

```text
crates/                  Rust protocol, authority, replay, verification, CLI
packages/protocol/       generated TypeScript public contracts and validators
packages/canopus/        optional bounded producer and evaluation harness
schema/                  language-neutral public schemas
conformance/             shared positive, hostile, and mutation fixtures
actions/                 consumer actions
docs/                    architecture, protocol, product, and release guidance
```

The product artifacts release independently:

| Artifact | Version owner | Runtime boundary |
| --- | --- | --- |
| Vela CLI and Rust crates | Cargo workspace | Owns protocol, replay, repository authority, and Decisions |
| `@vela-science/protocol` | Bun/npm package | Public types, canonical encoding, IDs, roots, validators, and conformance |
| `@vela-science/canopus` | Bun/npm package | Optional producer; invokes Vela through the public executable or client contract |

Canopus may depend on `@vela-science/protocol`. It may not import authority
internals, read authority credentials, create Decisions, or mutate Standing.
Tests enforce these import and process boundaries. Runtime authority separation
therefore remains explicit even though source and shared contracts live
together.

Vela Web remains separate because it is a read-only product with an
independent deployment lifecycle. Frontiers remain separate because each is a
canonical scientific repository with independent Git and authority history.

### 2. Retire `vela-internal`

The mixed-role repository is already frozen at
`pre-decomposition/2026-07-28`. Its load-bearing scripts, fixtures, and current
documents are classified by owner:

| Concern | Owner |
| --- | --- |
| Protocol, schemas, reducer parity, authority, replay, CLI, release qualification | Vela product monorepo |
| Model custody, verifier isolation, Run replay, Submission export, evaluation | `packages/canopus` |
| Exact state replay, Target Index, domain verifiers, correction cases | Each Frontier |
| Projection compatibility, manifests, read-only boundary | Vela Web |
| Reusable security, provenance, dependency, and organization policy workflows | `.github` |

Migration is complete only when every retained check passes in its owner,
no supported workflow depends on a sibling checkout or the parent, and the
load-bearing inventory is empty. The parent then receives a final archival
README and is archived without deleting history.

### 3. Preserve Canopus history without collapsing its product boundary

The public `vela-research-harness` history is imported without squashing under
`packages/canopus/`. Canopus `0.8.0` is released from the monorepo so the
stable artifact carries its permanent source and provenance identity.

```text
vela-science/vela-research-harness
    -> vela-science/vela/packages/canopus
```

The npm package remains `@vela-science/canopus`. The old repository carries a
final archival README and is archived. Its tags, Releases, issues, and history
remain available; it is not kept as a writable mirror.

### 4. Separate compatibility, releases, and exact execution

Compatibility is expressed by schema and capability, not a copied exact patch
number. The published package carries:

```text
packages/canopus/compatibility.json
packages/canopus/toolchain.lock.json
```

`compatibility.json` declares required repository epochs, schemas, and Vela
capabilities. `toolchain.lock.json` binds immutable Vela and Codex release
identities for exact execution, CI, and Mission preparation. Source, README
examples, and workflows do not copy a current Vela patch or platform hash
matrix. A separate `vela capabilities` command or release-manifest schema is
deferred until an external consumer demonstrates that the existing public
schemas and compatibility file are insufficient.

Every exact Run continues to pin its Vela binary, source, Codex runtime,
verifier, packet, and artifact identities. Component versions remain
independent; the monorepo does not create an ecosystem version.

### 5. Generate releases and share fixtures

The Cargo workspace and npm packages use owner-local release automation. One
release change may update independently versioned artifacts and changelogs;
publication workflows publish only the components actually released.

One conformance corpus drives Rust, TypeScript, Canopus, Web-reader, and
external-implementation checks. Cross-repository reusable workflows live in
`vela-science/.github` and are pinned by full commit SHA. Repository-specific
tests stay beside their owner.

## Rejected alternatives

- **Keep `vela-internal`.** Rejected because it has no coherent product or
  distribution responsibility.
- **Keep Canopus in a writable second repository.** Rejected because frequent
  coordinated changes and handwritten public-contract duplication outweigh
  Git-level separation, which did not enforce runtime authority.
- **Give Canopus direct Rust-internal access after the merge.** Rejected because
  source colocation is not permission to bypass the public producer boundary.
- **Create `vela-lab`.** Rejected because private memos already have a
  non-product home and exploratory work should not become release
  infrastructure.
- **Delete transition repositories immediately.** Rejected because unique
  history and load-bearing checks must first move or be explicitly retired.

## Acceptance evidence

1. `vela-internal@14e8ebfc2` carries the checked 1,037-path retirement
   inventory and final archival README. Its complete predecessor is retained
   at `pre-decomposition/2026-07-28`.
2. Product packages CI, conformance, and CodeQL passed in the public monorepo
   at `8d35b7278f8eaf9f45e000f6836cc92585d99d87` without the parent.
3. Canopus history was imported without squash under `packages/canopus`
   before the permanent-source `0.8.0` release.
4. Rust and TypeScript consume the public conformance corpus, and the product
   package checks passed from the monorepo.
5. Protocol and Canopus package build, replay, boundary, pack, npm provenance,
   and GitHub release checks passed from `product-v0.8.0` at
   `5df4c0cdd1e049d80c7ea0be00ee96fc4307c681`.
6. `vela-research-harness` and `vela-internal` carry archival tombstones and
   are archived. The 15 private-parent source objects referenced by public
   Erdős records are mirrored byte-for-byte at
   `erdos-frontier@1dd56de918ba35347b5cc3b1a657a27c33f7fbe5`;
   strict repository verification passes without changing scientific or
   authority roots.
7. Vela, Protocol, and Canopus retain independent artifact versions and
   immutable release identities. Exact historical Runs retain their original
   binary roots.
8. No protocol bytes, Frontier Event, Decision, release tag, package identity,
   or historical Git object was rewritten.
