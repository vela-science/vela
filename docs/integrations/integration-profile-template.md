# Non-normative integration-profile template

Status: package-plane guidance with authority effect `none`. Protocol 1 has no
corresponding object or wire schema. Capability availability remains
source-owned.

Use this template when a native scientific system emits or verifies evidence at
Vela's portable boundary. Keep source-specific semantics in the owning
repository. Delete sections that do not apply; do not invent facts to fill them.

## Purpose and ownership

- Profile name and version:
- Owning source, repository, or organization:
- Bounded scientific use:
- Native standard or interface reused:
- Exact gap this profile fills:
- Maintained producers and consumers:
- Authority effect: `none`

## Native identity

- Native system and object kind:
- Native identifier:
- Immutable revision, commit, accession, digest, or equivalent:
- Locator and resolution procedure:
- Mutability, tombstone, and deletion behavior:
- Exact interpreter or reader:

## Environment and reproduction contract

- Environment class: `exact`, `bounded`, `best_effort`, or `unavailable`
- Package locks, image digests, derivations, firmware, or calibration:
- Hardware, runtime, or service facts that are not captured:
- Network and credential requirements:
- Resource and safety bounds:
- Expected failure modes:

## Evidence and retention

- Inputs and exact roots:
- Outputs and exact roots:
- Certificates, observations, logs, or reports retained:
- Referenced rather than retained material:
- Artifact custody and retention policy:
- Privacy or access classification:
- License, rights, and permitted redistribution:
- Availability claim and observation time:

## Producer boundary

- Submission assertion and conditions:
- Requested transition:
- Caveats and replayability:
- Optional source-owned run or session reference:
- Exact native manifests, methods, and outputs retained as Artifacts:
- Producer checks, explicitly labeled `producer_reported`:

## Reviewer or verifier scope

- Reviewer kind: `human`, `ai_model`, `organization`, or `deterministic_tool`
- Performer identity, provider, version, and attesting actor where available:
- Named method and exact method root:
- Property checked:
- Exact subject and input roots:
- Independence disclosure and shared dependencies:
- Required and retained output artifacts:
- Outcome vocabulary and failure handling:
- Explicit nonclaims and limitations:

Reviewer kinds are peers. Category alone supplies no quality or authority
ranking. Evaluate evidence by method fitness, exact inputs, independence,
outputs, scope, and limitations. A consolidation or synthesis is another
separately attributed review; it does not overwrite its inputs or create a
Decision.

When a native workbench exposes them, retain links to the separately owned
session or checkpoint evidence: agent, model, provider, runtime, tool versions,
token or resource use, elapsed time, file changes, and exact checkpoint. These
facts remain source-owned activity provenance and confer no acceptance.

## Semantic mapping report

For every consequential translation, list exact source and target identities,
then report:

- `preserved`:
- `approximated`:
- `omitted`:
- `unsupported`:
- `assumed`:
- `unresolved`:

A successful parse, conversion, build, or execution does not establish semantic
equivalence. State which losses block Submission, Verification, or admission and
which are merely disclosed.

## Read and write boundaries

- Source-native authority:
- Vela Repository receiving the portable record:
- Read-only projections produced:
- Writeback, if any, and the authority that permits it:
- Rebuild procedure for hosted indexes:
- Explicitly forbidden actions:

An adapter may emit a Submission or Verification Record. It must not mint a
Decision, Event, accepted Standing, or a global source identity.

## Acceptance checklist

- [ ] The native system still works without Vela.
- [ ] The executor or scheduler can be replaced without rewriting Standing.
- [ ] A producer can emit a Submission without Repository-authority credentials.
- [ ] A verifier can emit one scoped result and nonclaims without changing Standing.
- [ ] Producer, verifier, and Repository authority identities remain distinct.
- [ ] Every consequential input, method, environment, and native object resolves
      to an exact version or fails explicitly.
- [ ] Consequential translations report all six semantic-loss categories.
- [ ] Corrections identify affected, unaffected, unresolved, and
      reassessment-required state within declared coverage.
- [ ] Hosted readers can be deleted and rebuilt from exact roots.
- [ ] Restricted or unavailable evidence is represented without implying public
      reproducibility.
- [ ] Multiple reviewers remain separate Verification Records.
- [ ] A cold successor can recover current Standing, decisive evidence,
      limitations, correction history, and the next valid action without private
      maintainer context.
