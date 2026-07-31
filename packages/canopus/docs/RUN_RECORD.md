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

`canopus export` creates `canopus.submission-bundle.v1`:

```text
submission-bundle/
  submission.json
  manifest.json
  artifacts/sha256/<full-digest>
```

`submission.json` is a whole-body Ed25519-signed `vela.submission.v1`.
Independent verifier output is named only as a verification requirement; it is
not mislabeled as producer authority or a Vela Verification Record.

The current worker contract keeps verifier status out of the Claim. After a
passing verifier result, the producer may pass `--claim` with `--scope-limit`
to refine that pre-verifier wording into one bounded scientific Claim. Canopus
signs the new Submission, records the refinement, and leaves the Run
byte-for-byte unchanged. A retained older Run that says verification is still
pending fails closed until this explicit correction is supplied. Control
characters remain forbidden.

The producer key is ephemeral. It is not placed in run evidence, the portable
bundle, or a retained capability store. A producer that independently keeps its
own key may use Vela's direct withdrawal interface; Canopus does not add a
second key lifecycle.

`canopus export --attempt <vat_id>` writes that exact private Attempt ID into
`Submission.provenance.source_attempt`. The immutable Run remains independent
of the Attempt; only the optional Submission export carries the binding.

## Submit

`canopus submit`:

1. verifies the bundle, Submission signature, Artifacts, source Git roots, and
   exact Vela binary;
2. keeps transport blobs outside the clone and lets Vela create their canonical
   content-addressed paths inside the repository-authority transaction;
3. performs ordinary registration in a disposable exact-head clone, or uses
   the clean source checkout when `--attempt` is present because private
   Attempt state is intentionally absent from Git;
4. requires `vela.submit-result.v1`, `pending_review`, and accepted-event delta
   zero;
5. fast-forwards the clean source checkout only after the registration is
   complete.

Submit does not create a Verification Record, Decision, Event, or accepted
Standing. `canopus submit --attempt <vat_id>` fails unless the ID exactly
matches the Submission, and Vela independently revalidates the active
Attempt's Target binding and budgets.

## Budgets

Missions bound prompt bytes, Artifact bytes, attempts, processes, output, wall
time, and observed provider tokens. The same verifier budget covers initial and
clean-clone replay. Provider-reported token totals are verified post hoc; the
subscription CLI does not expose a portable pre-call billing cutoff.
