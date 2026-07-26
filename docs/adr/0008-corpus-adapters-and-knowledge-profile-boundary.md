# ADR 0008: Source-local workbench adapters and the shared-extraction boundary

- Status: Proposed
- Release posture: Unscheduled; no Canopus version is reserved
- Protocol effect: None
- Source-local experiment gate: one stable, licensed, machine-readable
  workbench export with a real bounded handoff
- Shared-extraction gate: two qualifying formats expose duplicated common code
  that the extraction deletes

## Context

The Vela product story is intentionally short:

```text
produce → preserve → check → decide → reuse
```

Canopus is a removable producer and workbench-adapter layer. A canonical
frontier Git repository preserves the work. Vela is the sole protocol and
authority boundary: it checks replay and verifier evidence, and only signed
policy or a protected human decision decides scientific standing. The
read-only Observatory and other replaceable readers support reuse.

Research tools such as Codex, Entire, OpenScience, OpenResearch, notebooks, and
batch schedulers can expose useful execution records. That does not by itself
justify a shared Canopus schema, adapter SDK, profile revision, CLI family, or
semantic service. Each abstraction would become permanent product surface and
could blur Canopus's narrow role.

The evidence currently supports studying source formats, not shipping a new
interoperability layer. One source-specific path can justify one removable
source-local adapter experiment; it is not a shared platform boundary.

## Proposed decision

Do not implement a shared adapter system yet. A qualifying source-local
experiment does not require a Vela reader GO, protocol change, or Canopus
release.

1. After one format passes the source-format evidence gate, start with one
   small source-local export/import script for its independently useful
   handoff.
2. Keep every script offline, content-addressed, authority-free, and removable.
3. Record the exact licensed source schema, version, exported bytes, missing
   fields, and any duplicated mapping logic.
4. Reopen this ADR for shared extraction only after two real formats require
   the same core.
5. If the gates pass, promote only the fields and code proved common by those
   two implementations. The promotion change must delete the source-local
   duplication rather than retaining three implementations.

The following remain candidate ideas, not approved interfaces:

- `canopus.research-activity.v1`;
- `canopus.profile.v3` and a knowledge binding;
- a shared adapter registry or SDK;
- `canopus adapter ...` and `canopus activity ...` commands; and
- any new semantic repository, daemon, database, or hosted service.

Mission v1, profile v1/v2, run records, and all released evidence remain
unchanged. ADR 0009 separately retires new withdrawal-capability retention
without changing these adapter boundaries.

## Why this boundary is simpler

| Candidate | Product step | Existing capability reused | Duplication it must delete | Exit condition |
| --- | --- | --- | --- | --- |
| Source-local import script | Produce | Source tool export, Canopus canonical JSON and custody checks | One manual or bespoke conversion for that source | Remove if the source format is unstable, unlicensed, network-dependent, or unused |
| Shared activity envelope | Produce-to-preserve handoff | Mission v1 and run v1 fields, full SHA-256 roots | The same mapping code in two maintained source adapters | Reject if fewer than two formats need it or source-specific fields leak into the core |
| Profile v3 binding | Produce | Closed profile v2 and Vela Receipt evidence | Repeated semantic-root binding in two real profiles | Do not implement until two real profiles require the same closed binding |
| Adapter SDK/registry | Produce | Existing native custody scanner and source-boundary validation | Duplicate hostile-input and canonicalization logic in two maintained adapters | Delete or keep source-local if the shared layer is larger than the code removed |
| Adapter/activity CLI | Produce | Existing `doctor`, `run`, `inspect`, and `replay` command patterns | Repeated operator invocation that survives two real uses | Do not add until a stable shared contract exists |

There is no candidate authority step in this table. Imported activity is
research memory unless it enters Vela through the existing Receipt, proposal,
policy, and decision path.

## Source-format evidence gate

A source counts toward the two-format gate only when it has:

1. a stable versioned machine-readable export;
2. a license permitting implementation and fixture distribution;
3. exact retained source bytes and a full content root;
4. deterministic offline parsing from an explicit file or directory;
5. completed, failed, cancelled, partial, and unknown-field fixtures where the
   source format can express them; and
6. a real use that removes a bespoke handoff or enables a reproducible one.

Current candidates:

- Entire CLI `v0.8.42` checkpoint exports may qualify after a disposable,
  prehydrated, network-denied export proves complete capture without mutating
  the canonical source.
- OpenScience may qualify only after its exact version, Apache-2.0 source,
  stable export, and redistributable fixtures are pinned.
- OpenResearch does not currently qualify: its public surface does not provide
  both a stable versioned machine-readable export contract and a license that
  permits implementation plus fixture redistribution. It remains precedent
  only until both are available and pinned.
- Cursor Origin is precedent only while it lacks a stable public schema,
  implementation surface, and license.

Direct Codex transcript parsing is not a second independent format. Add it only
if a retained fixture proves a released workbench export omits evidence needed
for an existing Canopus mission.

## Candidate contract constraints

If the shared-extraction gate passes, any shared activity record must remain a
closed, authority-free observation. At minimum it would bind:

- producer and adapter identity and full artifact roots;
- objective, declared bounds, process outcome, and explicit missing fields;
- exact repository commit/tree or deterministic dirty-worktree manifest;
- input packet, profile, capsule, artifact, and verifier roots when present;
- resource use, interventions, caveats, capture coverage, and source-export
  root; and
- `authority_effect: none`.

It must not contain chain of thought, scientific standing, an accepted flag, a
mutable `latest` identity, or an inferred claim. Branch names, provider session
IDs, timestamps, run numbers, and short IDs remain locator metadata only.

Unknown source fields, if retention is necessary, belong in one rooted opaque
attachment. Missing, unknown, redacted, and not-applicable values remain
distinct. Redaction produces a declared derivative and never claims byte
identity with the source.

## Safety and replay requirements

Every source-local experiment and any future shared layer must fail closed on:

- changed source bytes, adapter substitution, short roots, or mutable source
  identity;
- path traversal, symlink escape, device files, oversized archives, or archive
  bombs;
- environment variables, tokens, cookies, key directories, private paths, or
  unrelated host files entering output;
- network-dependent conversion or replay;
- dirty worktrees without a deterministic rooted manifest;
- workbench success presented as verifier success or scientific standing;
- a verifier observation without exact subject and verifier roots; or
- any mutation of a canonical frontier.

Re-importing identical source bytes with the same script or future adapter
version must be byte-identical. Changed source bytes create a new record and
never overwrite the first. Permissive inspection may report a rooted opaque
source and diagnostics, but it cannot land a Receipt or supply policy context.

## Evidence required for shared extraction

Reconsider a shared Canopus implementation only when all of the following are
recorded:

1. two qualifying licensed formats have working source-local scripts;
2. their independently written mappings expose a concrete common core;
3. promoting that core deletes the corresponding duplicated code and removes
   more maintained code than it adds;
4. exact replay works offline from retained bytes;
5. hostile custody fixtures show no secret or human-key exposure; and
6. Vela remains the sole authority boundary, with state changes possible only
   through the released Receipt/proposal/policy/decision path.

Passing these gates authorizes a new decision, not an automatic release. The
ADR must then name the smallest exact interface and a release target based on
the implementation actually selected.

## Alternatives rejected for now

- **Reserve a Canopus release and build the abstraction speculatively.** This
  turns a research question into permanent product surface before the shared
  need exists.
- **Make Canopus a general research workbench.** Existing tools already own
  execution UX; duplicating them adds bloat and custody risk.
- **Put workbench adapters in Vela.** Source formats are not protocol or
  authority semantics.
- **Ingest directly into Neon.** The read model is disposable and cannot be the
  only retained or replayable record.
- **Create a `vela-knowledge` service first.** A source-local producer adapter
  does not justify a repository, package, database, protocol, or service.
- **Accept free-form JSON.** Unknown semantics, secrets, and authority language
  would cross the boundary silently.

## Consequences

Canopus stays small and understandable. Useful source-specific experiments can
proceed without reserving a release or creating a framework. A shared contract
will exist only if two real integrations prove that it deletes duplication
while preserving Vela as the sole authority boundary.
