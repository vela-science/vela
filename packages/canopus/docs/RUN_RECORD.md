# Run, export, and submit records

## Run

A completed current Run writes `canopus.run.v2` beneath its private run root.
Its contract is deliberately nonmutating:

```json
{
  "schema": "canopus.run.v2",
  "effect": "none",
  "authority": "non_authoritative",
  "submission": null
}
```

The run root retains:

- append-only orchestration activity;
- exact worker events, final response, and bounded stderr;
- candidate bytes and content-addressed Artifacts;
- verifier identity and result;
- clean-clone reproduction;
- exact mission, source, runtime, budget, and evidence roots.

The worker workspace is disposable. If a Run fails after worker execution,
Canopus retains only newly created, recognized implementation-source and build
files useful for diagnosis under `failure-evidence/`. Generic text, notes,
transcripts, logs, the target packet, declared Artifacts, runtime files, known
credentials, binaries, and pre-existing inputs are excluded. The capture is
capped at 16 files, 64 KiB per file, and 256 KiB total. Its exact manifest root
is appended to the activity log before the failure record; it is
non-authoritative and is deleted on a successful Run.

For a repair Run, `repair.input_bound` records the exact parent digest, path,
and byte count. The worker receives those root-checked bytes at the contracted
artifact path. Canopus refuses a repair mission without `--repair-from` or
when the supplied bytes do not match `parent_candidate`.

`canopus show` projects these records. `canopus replay` reruns the frozen
verifier. Deleting Canopus or its run directory cannot change Vela replay or
Standing.

## External activity recorders

A Canopus Run may originate from or be compared with work captured by an
external activity recorder. Such records are supplemental and
non-authoritative. They do not replace the exact mission, starting roots,
worker events, frozen Artifacts, verifier result, budgets, or clean-clone
reproduction retained by `canopus.run.v2`.

The current Run schema does not require an external recorder, include raw
external transcripts, or depend on an external checkpoint for replay.
Deleting the recorder or its data cannot change the Run result, a Vela
Verification Record, or Standing. Any future provider experiment begins as a
private, metadata-only sidecar and must pass the measured gate in
[ADR 0012](adr/0012-optional-external-activity-recorders.md) before changing a
public Canopus contract.

## Export

`canopus export` creates a portable export directory:

```text
submission/
  submission.json
  manifest.json
  artifacts/sha256/<full-digest>
```

`submission.json` is a whole-body Ed25519-signed `vela.submission.v1`.
Independent verifier output is named only as a verification requirement; it is
not mislabeled as producer authority or a Vela Verification Record.
`manifest.json` binds the export to its exact Run root, source Git identity,
producer, Submission root, and transport mapping. It is portable lineage
evidence; Vela does not treat it as authority or require it for registration.

The current worker contract keeps verifier status out of the Claim. After a
passing verifier result, the producer may pass `--claim` with `--scope-limit`
to refine that pre-verifier wording into one bounded scientific Claim. Canopus
signs the new Submission, records the refinement, and leaves the Run
byte-for-byte unchanged. A retained older Run that says verification is still
pending fails closed until this explicit correction is supplied. Control
characters remain forbidden.

The producer key is ephemeral. It is not placed in run evidence, the exported
directory, or a retained capability store. A producer that independently keeps
its own key may use Vela's direct withdrawal interface; Canopus does not add a
second key lifecycle.

`canopus export --attempt <vat_id>` writes that exact private Attempt ID into
`Submission.provenance.source_attempt`. The immutable Run remains independent
of the Attempt; only the optional Submission export carries the binding.

## Registration

Canopus has no registration command. Register the exported Submission through
the canonical Vela path:

```sh
vela submit /path/to/submission/submission.json \
  --frontier /path/to/frontier \
  --attempt <vat_id> \
  --as <agent:id> \
  --json
```

Vela verifies the Submission signature, transport artifacts, producer and
Attempt identities, current Target binding, budgets, repository state, and
repository-authority transaction. Registration creates only a pending Proposal
with accepted-event delta zero. It does not create a Verification Record,
Decision, Event, or accepted Standing.

## Budgets

Missions bound prompt bytes, Artifact bytes, attempts, processes, output, wall
time, and observed provider tokens. The same verifier budget covers initial and
clean-clone replay. Provider-reported token totals are verified post hoc; the
subscription CLI does not expose a portable pre-call billing cutoff.
