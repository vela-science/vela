# Vela organization: target architecture

Status: canonical target-state map for VELA-ORG-1

The target is the smallest coherent operating system supported by current
evidence: Vela Core, Problems, Workbench, Math, and an intentionally tiny www
surface. It preserves Vela `0.977.5` as immutable released history and preserves
Protocol 1. It creates no new protocol object, authority path, scientific
Repository, provider dependency, or speculative product repository.

This target is an ownership map, not permission to migrate. The exact gates
and approvals are in [the migration map](VELA_ORG_MIGRATION.md); observed
reality remains in [the current map](VELA_ORG_CURRENT.md).

## Coherent target

```text
native scientific sources and tools
              |
              v
        Workbench activity
              |
      unsigned Submission v3
              |
              v
      Math authority Repository <---- Vela Core reader and CLI
              |
       exact Git state/root
              |
              v
    Problems projection and hosted Work
              |
       WebMCP permission subset

www: tiny static orientation surface linking to Core and Problems;
     no scientific state, hosted work, projection, or authority
```

The arrows describe data and handoff, not transferred authority. Only an
authorized Decision admitted by Math changes Math Standing. Problems,
Workbench, WebMCP, www, GitHub, and providers cannot perform that transition.

## Canonical components

| Component | Canonical responsibility | Must remain outside |
| --- | --- | --- |
| Vela Core | Protocol 1, canonical objects and encoding, roots, replay, authority evaluation, stable CLI JSON, schemas, conformance, and releases | Scientific campaigns and Decisions, hosted work, projections, source-specific logic, and provider state |
| Problems | Canonical public Problems source, verified root-bound discovery projection, hosted activity product, and the eight-tool WebMCP subset | Repository keys, Verification/Decision/Event issuance, inferred Standing, and duplicate scientific records |
| Workbench | Sovereign local activity, native runs, evidence preparation, and explicit export to the portable Submission boundary | Repository authority, hosted Problems state, and a competing protocol or workflow engine |
| Math | One reference mathematics authority Repository with exact source locks, Claims, Submissions, Verifications, Decisions, Events, correction history, and replay state | Generic Core behavior, upstream statement ownership, proof implementation ownership, or authority over other Repositories |
| www | A minimal, rights-safe, static `vela.space` orientation surface that links to canonical Core documentation and Problems | Projection code, authenticated state, a database, shared component catalogue, scientific authority, WebMCP, or a second product runtime |

The www source placement is **UNKNOWN** until the extraction demonstrates a
rights-safe closure and an existing canonical owner is shown to fit. This map
does not invent a `www` repository or authorize repository creation. The www
surface is a deployment responsibility, not a fifth semantic authority.

## Protocol and scientific authority invariants

The target retains the boundaries in [Protocol 1](../PROTOCOL.md):

- a Submission is authenticated producer input and requests change;
- a Verification Record reports one scoped check and has no Standing effect;
- a Proposal is pending until an authorized Decision covers it;
- only an authorized Decision changes Standing through admitted Events;
- strict replay begins from an independently pinned sequence-one authority
  record and fails closed on invalid roots, signatures, policy, or relations;
- a Git commit, deployment, provider write, successful check, or Web badge is
  not scientific acceptance; and
- projections are disposable, root-bound readers, never sources of Standing.

Protocol 1's normative schemas, release roots, wire names, authority records,
OpenSSH-agent custody boundary, and current Decision model do not change in
this organization migration.

## WebMCP subset authority

The target keeps the current WebMCP boundary because it is already the minimal
safe surface:

| Capability | Target disposition |
| --- | --- |
| Read Problems projection and hosted Work | allowed under hosted Problems permissions |
| Mutate attributed hosted Work | allowed; hosted product effect only |
| Prepare unsigned Submission v3 candidate | allowed; `authority_effect:none` |
| Sign a Submission or Verification | forbidden |
| Issue a Verification Record, Decision, or Event | forbidden |
| Access repository authority credentials | forbidden |
| Mutate or infer Standing | forbidden |

WebMCP remains a strict subset of the human-facing hosted product, not a route
around repository authority and not a Core semantic dependency.

## Data and provider target

Git Repositories retain scientific authority. Problems may keep one operational
PostgreSQL provider for two sharply separated classes of data:

- a disposable, reconstructible projection schema bound to exact source roots;
- canonical hosted Problems activity state that is explicitly non-scientific.

The schema names and physical database arrangement may change only through a
separate operational migration; this target requires the semantic separation,
not a rename. Legacy Observatory data may remain retained until custody and
deletion approval are resolved, but no current product should depend on it.

Vercel deploys public surfaces, WorkOS authenticates hosted accounts, GitHub
stores source and Git history, and Entire may retain optional provenance. Each
provider is replaceable and outside Core replay. No provider becomes a trust
anchor or source of Standing.

## Repository and historical custody target

The active semantic topology is four canonical repositories or repository
roles: `vela`, `problems`, `vela-workbench`, and `math`. This statement does not
order a rename, split, or repository creation. `.github` remains organization
support, and `lean-correspondence` remains reference evidence rather than a
product or authority.

All predecessor and archived repositories remain immutable custody records.
They may stay archived indefinitely. Historical tags, receipts, Pages content,
stashes, local objects, and research directories are not silently folded into
the active topology and are not deleted merely because the target is smaller.

No additional repository is justified for a topic, provider, graph, package
registry, adapter framework, agent transcript store, scheduler, Observatory,
WebMCP, or www until a separate evidence-backed ownership decision passes its
approval gate. Topic boundaries do not create scientific authorities.

## Completion properties

The target is reached only when all of the following are simultaneously true:

1. a qualified current Core reader replays the exact Math commit and root from
   a clean full clone without changing Math authority state;
2. active downstream readers and generators bind that qualified release and
   accepted binary digest while historical receipts remain unchanged;
3. public Problems reconstructs its exact projection release without private
   `vela-web` custody;
4. licensing and asset rights are evidenced in the repositories that publish
   the bytes;
5. www is rights-safe, minimal, and at deployment parity without the old
   private dependency graph;
6. `vela-web` no longer owns a live deployment or required release asset and
   its rollback window and stashes have reviewed dispositions; and
7. every DNS, archive, rename, provider cleanup, and local deletion with
   external or custody impact has explicit user approval.

Until then, [the current architecture](VELA_ORG_CURRENT.md), including its
transitional duplication, remains the operational truth.
