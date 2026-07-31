# Target Index

`targets.json` is a derived bridge between domain work and Vela's producer
loop. It makes bounded work addressable without making ranking, graphs, or a
domain ontology authoritative.

Domain tools own Target meaning, ordering, packet construction, and packet
schemas. Vela owns the closed seal and freshness checks.

## Current contract

Current repositories use `vela.target-index.v4`. The index binds:

- Frontier ID;
- current repository origin ID and root;
- source Git object format, commit, and tree;
- a complete sorted input manifest and root;
- sealing Vela version;
- ordered Targets;
- exact packet paths, schemas, sizes, and digests; and
- full index root.

Targets sort by ascending `(rank, id)` and use unique full IDs. States are:

```text
open paused blocked done retired
```

The index is derived, deletable, and non-authoritative.

## Candidate and seal

A domain generator emits `vela.target-index-candidate.v1` at an ignored path.
It declares:

- Frontier ID;
- exact source Git commit;
- every tracked input that influenced membership, order, description, labels,
  or packets; and
- ordered Target semantics and packet paths.

Vela validates the candidate, resolves every input from the source commit,
reads each packet once, fills byte lengths and digests, binds the current
repository origin/root, and computes the index root. It never invents or
reranks domain semantics.

```bash
vela target-index repair . --json
vela target-index seal . \
  --candidate .vela/tmp/target-index-candidate.json \
  --check --json
vela target-index seal . \
  --candidate .vela/tmp/target-index-candidate.json \
  --apply --json
vela target-index inspect . [<full-target-id>] --json
```

`repair` is diagnostic. `seal --check` is write-free. `seal --apply` writes
only `targets.json` atomically and does not stage or commit it.

The index and packets yield no Offer until their working bytes exactly match
tracked Git blobs.

## Freshness

At inspection, Offer, and Attempt time Vela verifies:

- the index and relevant packets are tracked regular files;
- the source commit exists, is an ancestor of `HEAD`, and has the declared
  tree;
- the source tree does not contain the sealed output blob;
- every declared input path, mode, size, and digest matches;
- the current origin ID and repository root match;
- Target, packet, input, and index roots rederive; and
- the selected Target is open.

Any mismatch is a typed stale condition, not an advisory. There is no force or
non-strict work bypass.

## Offers and Attempts

`vela next` validates every open entry before counting or returning it:

```text
configured = open entries
stale      = entries excluded by freshness failure
fresh      = configured - stale
returned   = offers after the requested limit
```

`vela next` does not pretend that private local work is shared availability.
`vela start` performs the exact local Attempt arbitration, rechecks the
selected entry, and creates one ignored private Attempt bound to:

- current repository origin and root;
- Target Index and input roots;
- source Git commit/tree;
- exact Target and packet;
- completion contract;
- allowed operations and Artifact classes;
- Submission, Verification, Artifact-count, and Artifact-byte budgets; and
- local expiry.

It writes no canonical object and reads no authority key.

Each `vela submit` and Attempt-bound `vela verification import` revalidates the
exact current Target-task binding and binds it into the repository transaction
read set. A successful registration or import increments the matching private
Attempt counter rather than deleting the Attempt.
Starting roots remain exact; the current private read set advances only when
the same Target source, inputs, and packet remain unchanged. Expiry or
explicit `start --drop` revokes future use.

That binding proves queue identity, packet/read-set continuity, and enforced
local evidence budgets. It does not infer that a Claim is scientifically
responsive to the Target. Semantic fit remains an explicit Verification and
human-review question; Vela does not add a heuristic semantic validator to the
Target Index.

## Inspection

`target-index inspect` may display a stale Target by full ID. It labels it
unactionable and returns exact stale codes. Packet content appears only when
its binding still matches.

Deleting `targets.json` removes catalogue convenience only. It changes no
Claim, Proposal, Decision, Event, or Standing.

Structural graph ranking and Discovery Calculus output remain separate rooted
advice. They cannot reorder `vela next` without a domain generator producing
and sealing a new candidate.
