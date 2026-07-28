# ADR 0024: Repository ownership and integration-repository retirement

- Status: Proposed
- Target release: no Vela release; accept after the repository-decomposition
  gate completes
- Protocol effect: none
- Product effect: public Vela owns architecture and roadmap; each repository
  owns its own tests, release facts, and compatibility boundary
- Authority effect: none
- Compatibility: Git history, tags, releases, package names, Frontier bytes,
  and public interfaces remain intact

## Context

`vela-science/vela-internal` was created to compose exact Vela, Canopus, Web,
and Frontier sources. It now mixes release pins, conformance, cross-language
fixtures, campaign coordination, private research, historical reports, and
duplicated canon. Only Vela is an actual submodule; Canopus and Web are found
through sibling-checkout conventions. The repository produces no installable
distribution.

This creates three failures:

1. Public users must discover a private meta-repository to understand the
   project.
2. The owner of architecture, roadmap, and compatibility is ambiguous.
3. Parent CI can block product work even when the actual owner repositories
   are green.

Canopus has the opposite shape. It is independently packaged, optional,
replaceable, and integrated through released Vela CLI identities. That is a
real repository boundary, but its repository name does not match its product
and npm package.

## Decision

### 1. Use owner repositories

The target active topology is:

```text
vela-science/vela
vela-science/canopus
vela-science/vela-web
vela-science/erdos-frontier
vela-science/formal-conjectures-frontier
vela-science/sidon-frontier
vela-science/quantum-codes-frontier
vela-science/.github
```

Each repository owns the tests and documentation needed to release and use
its component:

| Concern | Owner |
| --- | --- |
| Protocol, schemas, reducer parity, authority, replay, CLI, release qualification | Vela |
| Released-Vela composition, model custody, verifier isolation, Run replay, Submission export, evaluation | Canopus |
| Exact state replay, Target Index, domain verifiers, correction cases | Each Frontier |
| Projection compatibility, manifests, read-only boundary | Vela Web |
| Reusable security, provenance, dependency, and organization policy workflows | `.github` |

### 2. Retire `vela-internal` from the product topology

The current mixed-role repository is frozen with a final
`pre-decomposition/2026-07-28` tag before destructive cleanup. Its
load-bearing scripts, fixtures, and documents are inventoried by owner.

Migration is complete only when:

- every retained check passes in its owner repository;
- no release or Frontier workflow depends on a sibling checkout or the parent;
- public architecture and roadmap live in Vela;
- the organization profile maps the supported repositories;
- historical reports and commits remain reachable;
- and the load-bearing inventory is empty.

Then `vela-internal` receives a final archival README and is archived on
GitHub. It is not renamed to a distribution repository because it assembles
no distribution. It is not renamed to a lab by default because private memos
already have a non-product home.

### 3. Rename the Canopus repository

After Canopus `0.8.0` is released and its provenance is verified:

```text
vela-science/vela-research-harness
    -> vela-science/canopus
```

The npm package remains `@vela-science/canopus`. Source URLs, workflows,
badges, release metadata, organization navigation, and downstream checks are
updated in one bounded migration. The old GitHub name is never recreated, so
Git and web redirects remain available.

### 4. Replace synthetic ecosystem locks with compatibility evidence

Each released component publishes its own immutable identity and provenance.
Organization workflows exercise a compatibility matrix across public
interfaces and exact release artifacts.

An exact Run continues to pin all binary and source identities. A mutable
application or parent lock does not become canonical scientific state.

The current parent `ecosystem.lock.json` remains transitional evidence until
the owner workflows cover its useful assertions. It is then archived with the
parent instead of copied into a new meta-repository.

### 5. Keep boundaries external

Canopus remains separate from Vela. It uses the released executable, version,
digest, Submission, and Verification boundaries. It does not import Vela
internals or become a privileged producer.

Vela Web remains separate because it is a read-only product with an
independent deployment lifecycle. Frontiers remain separate because each is a
canonical scientific repository with its own authority and history.

## Rejected alternatives

- **Keep `vela-internal` unchanged.** Rejected because it has no coherent
  product or distribution responsibility.
- **Merge Canopus into Vela.** Rejected because it weakens replaceability and
  couples Rust protocol work to model execution, Bun, sandboxes, and npm.
- **Create `vela-distribution` now.** Rejected because no multi-component
  distribution exists.
- **Create `vela-lab` automatically.** Rejected because exploratory work does
  not need another active repository.
- **Delete the parent immediately.** Rejected because useful tests and
  historical evidence must first move or be explicitly retired.

## Acceptance gates

1. A checked inventory classifies every active parent script, fixture,
   workflow, and current document as move, retire, or historical.
2. Owner repositories pass their focused and clean-clone checks without the
   parent.
3. Canopus `0.8.0` is released before its repository rename.
4. The organization profile and all current public links name the target
   topology.
5. `vela-internal` has no load-bearing consumer, carries the final transition
   tag and archival README, and is archived without deleting history.
6. No protocol bytes, Frontier history, release tag, or package identity is
   rewritten.

Until all six pass, this ADR remains Proposed and `vela-internal` remains a
transition workspace rather than a supported product dependency.
