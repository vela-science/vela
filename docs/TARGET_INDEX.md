# Target Index

The Target Index is one optional, tracked `targets.json` file owned by a
Frontier. It is a derived work catalogue, not scientific Standing and not an
authority surface.

Current repositories use `vela.target-index.v5`. A domain adapter writes the
final canonical index directly. Vela does not maintain a candidate, seal,
apply, inspect, or repair lifecycle for this derived file.

## Contract

The index binds:

- the exact Frontier and current repository root (`repository_id`, and
  `repository` as `origin_id` plus `repository_root`);
- one exact Git source commit and tree;
- the sorted tracked input paths and their byte roots;
- a `claim_boundary`, which must read exactly
  `derived: true, authoritative: false, deletable: true`, the index restating
  in its own bytes that it carries no Standing and may be thrown away and
  regenerated;
- each open Target's stable ID, title, why, presence, rank, objective,
  labels, and packet;
- each packet's schema, path, size, and SHA-256 root; and
- an index root over the complete canonical document.

Every field above is required except `labels`, which defaults to empty. The
index and its entries reject unknown fields, so an adapter that omits `title`
or `why`, or invents a field, fails validation on its first run.

Every entry has `presence: "open"`. A generator removes unavailable work
instead of preserving paused, blocked, done, or retired pseudo-history in the
current catalogue. Git retains prior catalogues when historical inspection is
needed.

`targets.json`, every declared input, and every packet must be unchanged
tracked regular files at `HEAD`. Declared inputs must also match the exact
source commit. A mismatch makes the affected offer unavailable; it never
falls back to worktree bytes or an older index.

The domain adapter owns ranking and target semantics. It must emit canonical
JSON with targets sorted by ascending `(rank, id)`. Vela owns only validation,
the automatic repository-root/index-root rebind performed by an ordinary
repository transaction, and the runtime `replay`, `next`, and `start` gates.

## Producer flow

```bash
# The Frontier's domain adapter updates and commits targets.json and packets.
vela replay . --json
vela next . --json
vela start <full-target-id> --repo . --json
```

`next` returns only fresh open entries. `start` revalidates the exact index,
source, inputs, packet, and repository, then prints their identities in a
stateless briefing. Submission does not depend on or consume `start` output.
Neither operation changes scientific Standing.

The offer reports both `queue_position` and `rank`. `queue_position` is the
one-based order among currently open, fresh Targets. `rank` is the stable
configured priority, so the first remaining Target can legitimately have rank
two after all rank-one work closes. The `start` briefing labels the bound Git
identity `target_index_source`; it can be an ancestor of the current
`repository_head` because the derived index is rebound without changing its
scientific source inputs.

## Failure behavior

Vela fails closed when the index is untracked, non-canonical, bound to another
Frontier or repository root, based on unavailable or changed source/input
bytes, or names a packet whose tracked bytes, schema, size, or digest differ.
The repair is deliberately ordinary: fix the domain adapter or source data,
regenerate `targets.json`, review the Git diff, and commit it.
