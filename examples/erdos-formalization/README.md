# Erdős formalization historical replay fixture

This directory preserves eight immutable signed events produced by an earlier
Vela release. It exists to prove that the current read path can still decode and
replay accepted historical bytes; it is not a current frontier scaffold or an
authoring template.

- Authority under test: `.vela/events/*.json` (never hand-edit these files).
- Current replay expectation: eight events replay without conflict and the
  replayed state hash equals the source state hash.
- Retained historical material: `frontier.json`, `vela.lock`, and `proof/` show
  what the earlier release emitted. The old lock uses its release's event-log
  ordering and therefore is not a current strict-release certificate.

Inspect the compatibility result without writing:

```bash
vela check examples/erdos-formalization --json
```

Read the `replay` object: `ok` must be `true`, event count must be `8`, and
`replayed_hash` must equal `source_hash`. Do not "fix" the historical event
bytes or use this fixture as the starting point for a new frontier.
