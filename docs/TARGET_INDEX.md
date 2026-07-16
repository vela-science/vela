# Native target index

`targets.json` is the optional, generic bridge between a large scientific
atlas and Vela's task-first loop. It lets a frontier expose thousands of
addressable work targets without copying the atlas into protocol authority or
forcing agents to scrape a website, graph, or generated memo.

The index is a derived projection:

- canonical truth remains `.vela/events`;
- each target names one frontier-relative JSON packet and its exact SHA-256;
- `vela next` reads only the bounded index, ranks entries whose state is
  `open`, and skips live leases;
- `vela work <target>` opens and hash-checks only the selected packet, then
  records the index root, packet root, live event root, Git commit, and
  producer-only authority ceiling in the private session;
- `paused`, `blocked`, `done`, and `retired` entries are not ranked, but remain
  explicitly addressable for inspection or reproduction;
- deleting `targets.json` removes the catalogue convenience and changes no
  accepted scientific state.

## Contract

```json
{
  "schema": "vela.target-index.v1",
  "frontier_id": "vfr_...",
  "as_of": {
    "snapshot_hash": "sha256:...",
    "event_log_hash": "sha256:...",
    "proposal_state_hash": "sha256:..."
  },
  "claim_boundary": {
    "derived": true,
    "authoritative": false,
    "deletable": true
  },
  "targets": [
    {
      "id": "erdos:1056",
      "title": "Erdős 1056",
      "why": "11 pinned residual obligations; 9 recorded attempts; upstream open.",
      "state": "open",
      "rank": 17619056,
      "objective": "Produce one decision-relevant artifact without repeating banked routes.",
      "labels": ["erdos", "upstream-open", "residual-obligations"],
      "packet": {
        "path": "site/problems/1056.json",
        "sha256": "sha256:...",
        "schema": "erdos-frontier.problem-work.v1"
      }
    }
  ]
}
```

The reader is deliberately narrow. It rejects unsafe or duplicate target IDs,
absolute or traversing packet paths, oversized indexes and packets, unsupported
states, symlinks, frontier mismatches, packet digest drift, and packet schema
drift. Unknown descriptive fields may be added by producers, but Vela does not
promote them into policy, authority, signing, or decision semantics.

Index staleness is visible rather than silently erased. A lease or another
frontier event may make the index's `as_of` roots historical; the work session
still binds the selected immutable packet and the current live frontier root.
The frontier's reducer should regenerate the index when substantive
non-coordination state changes.
