# Native Target Index v2

`targets.json` is an optional, derived bridge between a scientific atlas and
Vela's task-first producer loop. It makes bounded work addressable without
copying a domain ontology into protocol authority or asking an agent to scrape
a website.

Domain tools own target meaning, target ordering, packet construction, and
packet schemas. Vela owns only the closed seal. A target is work advice, never
standing, verification, policy, or acceptance.

## Closed contract

A sealed index has schema `vela.target-index.v2` and contains:

- the exact Frontier ID;
- source Git object format, commit, and tree;
- a complete declared input manifest and its root;
- event-log root/count and non-lease event-log root;
- scientific-state, proposal, identity, dependency, and observed-profile
  roots;
- the fixed non-authoritative claim boundary;
- the sealing Vela version;
- ordered target records with exact packet path, schema, size, and digest; and
- the full `index_root`.

The complete wire shape is pinned by ADR 0016 and the conformance fixtures.
`index_root` is SHA-256 of Vela-canonical JSON for the complete v2 object with
only `index_root` omitted. The input root is the same construction over the
closed `vela.target-index-input-manifest.v1` value.

`conformance/target-index-v2/` is the single portable fixture source. Its
closed manifest binds the exact candidate, input manifest, packet, sealed
index, task binding, expected roots, and deterministic Git fast-import stream.
The ordinary Rust edge test imports that stream and checks the fixed commits,
trees, fresh/profile-only/lease/non-lease states, task-binding replay, and
shallow-history failure.

Targets are sorted by ascending `(rank, id)` and IDs are unique. State is
exactly `open`, `paused`, `blocked`, `done`, or `retired`. Labels are sorted
and unique. The canonical index is at most 4 MiB, contains at most 16,384
targets, and each packet is at most 1 MiB. Exact numeric and text limits live
in the protocol implementation and conformance vectors.

## Candidate and seal

A domain generator emits a closed `vela.target-index-candidate.v1`, normally
at ignored path `.vela/tmp/target-index-candidate.json`. It declares:

- `frontier_id`;
- the exact source Git commit;
- every tracked input path that influenced target membership, order,
  description, labels, or packets; and
- ordered target semantics and packet paths.

Vela validates the candidate, resolves every declared input from the source
commit's Git tree, reads each packet once, derives the Git tree and all
repository roots, fills packet size/digest, sets its version, and computes the
index root. It never invents or reranks domain semantics.

The advanced setup commands are:

```bash
vela target-index repair . --json
vela target-index seal . --candidate .vela/tmp/target-index-candidate.json \
  --check --json
vela target-index seal . --candidate .vela/tmp/target-index-candidate.json \
  --apply --json
vela target-index inspect . [<full-target-id>] --json
```

`repair` is read-only and reports exact stale codes plus the candidate-seal
command. It never runs the domain generator. `seal --check` is write-free and
returns the complete proposed bytes and read set. `seal --apply` atomically
writes only `targets.json`; it does not stage or commit.

The sealed index and packets grant no offer until they are tracked in `HEAD`
and their working-tree bytes exactly match the Git blobs.

## Freshness

At inspection, offer, and lease time Vela verifies:

- `targets.json` and every relevant packet are tracked regular files, not
  symlinks or submodules;
- the source commit exists, is an ancestor of `HEAD`, and resolves to the
  declared tree;
- the source tree does not contain the sealed index blob;
- every declared input path, Git mode, size, and digest matches that source
  commit;
- the current event history contains the exact sealed prefix;
- only valid `attempt.claimed` lease events may extend that prefix without
  changing the non-lease event root;
- scientific-state, proposal, identity, and dependency roots still match; and
- target, packet, input, and index roots rederive exactly.

A profile-only or documentation-only commit may leave an index fresh when its
source remains an ancestor and all security-bearing inputs still match.
`observed_profile_root` is audit context; its drift alone is not staleness.

The closed stale/error codes include schema, Frontier, Git availability and
ancestry, source-tree, self-reference, event, scientific-state, proposal,
identity, dependency, input, index, packet, tracked-output, duplicate, path,
and target failures. They are emitted as stable typed codes. No command may
downgrade one of them to an advisory.

## Offers and leases

`vela next` validates every open entry before counting or returning it. Its
availability counts mean:

```text
configured = open entries
stale      = open entries excluded by freshness failure
leased     = fresh open entries excluded by a live lease
available  = configured - stale - leased
returned   = offers returned after the caller's limit
```

`vela start` revalidates the index, selected packet, repository roots, and
transaction read set immediately before appending the lease. Failure writes no
session, event, journal marker, or Git commit. There is no `--force`,
non-strict bypass, or Profile v1 compatibility exception.

One successful claim creates a closed `vela.target-task-binding.v1` containing
the target and index root, source Git identity, input root, exact packet, index
roots, claim-time Git/event read set, and its own full binding root. The
private session carries that record. `vela land` copies the same bytes into
the Receipt v1 `vela:target_task_binding` extension, where the Receipt
whole-body binding covers them.

A later valid index change cannot rewrite that historical task binding.
Deleting a private session therefore cannot erase which offer and packet
produced a retained Receipt.

## Inspection and compatibility

`vela target-index inspect` may inspect a stale entry by its full target ID. It
labels the target unactionable and reports exact codes. Packet content appears
only when its binding still matches. Inspection never turns a target into an
offer or lease.

Target Index v1 remains readable for historical inspection, but it grants no
Profile v1 work. Profile v0.1 repositories remain read-compatible and
migratable; their old indexes are sealed to v2 inside the protected repository
migration.

Deleting the index removes only catalogue convenience. It changes no accepted
finding, event authority, or scientific-state root. Structural graph ranking
is separate advice and never replaces this canonical producer queue.

A Frontier Algebra or Discovery Calculus projection may read an exact sealed
index as context or an action catalogue. Its explanations, scores, and ordering
remain separate rooted advisory artifacts. They cannot edit the domain
candidate, rewrite the sealed index, reorder `vela next`, or alter a retained
task binding. A domain may adopt a demonstrated lens only by regenerating its
candidate through the ordinary domain-owned path and resealing it under this
contract.
