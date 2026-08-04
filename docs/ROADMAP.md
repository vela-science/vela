# Vela roadmap

Vela is version control for scientific state. The product is the complete,
local, Git-native loop:

```text
create -> submit -> verify -> decide -> replay
```

## Now

- Make `vela init` a complete, resumable one-command operation.
- Make empty and active Frontiers return useful next actions.
- Keep JSON output stable, closed, and actionable on success and failure.
- Exercise Submission, Verification, Decision, correction, and clean-clone
  replay as one end-to-end CLI product.
- Remove obsolete commands, compatibility paths, studies, and case-specific
  campaign material from Vela core.

## Next

- Fix concrete failures found by sustained use on maintained Frontiers.
- Reduce repeated code where deletion preserves exact roots and fail-closed
  behavior.
- Maintain the portable JSON Schema and conformance waist for existing objects.
- Improve read-only Web projections without adding authority or duplicate state.

## Only after demand

- Add another interoperability transport only after a real producer and a real
  consumer need it.
- Extract a shared package only when two maintained consumers agree on the same
  root and the extraction deletes more maintained code than it adds.
- Add hosted services only when local Git-native operation is insufficient for
  an observed user need.

## Not planned

- A Vela agent runner, scheduler, graph authority, automatic Decision path,
  hosted package registry, or reviewer-recruitment program.
- General usability, productivity, adoption, or scientific-lift claims from
  internal instrumentation.
- Source-specific scientific campaigns in the Vela core repository.

Scientific work belongs to its source-owning Frontier. Vela core changes only
when they remain useful after every named scientific case is removed.
