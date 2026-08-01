# Target Index

The Target Index is one optional, tracked `targets.json` file owned by a
Frontier. It is a derived work catalogue, not scientific Standing and not an
authority surface.

Current repositories use `vela.target-index.v5`. A domain adapter writes the
final canonical index directly. Vela does not maintain a candidate, seal,
apply, inspect, or repair lifecycle for this derived file.

## Contract

The index binds:

- the exact Frontier and current repository root;
- one exact Git source commit and tree;
- the sorted tracked input paths and their byte roots;
- each open Target's stable ID, rank, objective, labels, and packet;
- each packet's schema, path, size, and SHA-256 root; and
- an index root over the complete canonical document.

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
repository transaction, and the runtime `check`, `next`, and `start` gates.

## Producer flow

```bash
# The Frontier's domain adapter updates and commits targets.json and packets.
vela check . --json
vela next . --json
vela start <full-target-id> --frontier . --json
```

`next` returns only fresh open entries. `start` binds the exact index, source,
input, packet, repository, and Git read set before producer work begins.
Submission revalidates the retained binding. None of these operations changes
scientific Standing.

## Failure behavior

Vela fails closed when the index is untracked, non-canonical, bound to another
Frontier or repository root, based on unavailable or changed source/input
bytes, or names a packet whose tracked bytes, schema, size, or digest differ.
The repair is deliberately ordinary: fix the domain adapter or source data,
regenerate `targets.json`, review the Git diff, and commit it.
