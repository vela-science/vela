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

`canopus show` projects these records. `canopus replay` reruns the frozen
verifier. Deleting Canopus or its run directory cannot change Vela replay or
Standing.

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

The producer key is ephemeral. It is not placed in run evidence, the portable
bundle, or a retained capability store. A producer that independently keeps its
own key may use Vela's direct withdrawal interface; Canopus does not add a
second key lifecycle.

## Submit

`canopus submit`:

1. verifies the bundle, Submission signature, Artifacts, source Git roots, and
   exact Vela binary;
2. performs the Vela registration in a disposable exact-head clone;
3. requires `vela.submit-result.v1`, `pending_review`, and accepted-event delta
   zero;
4. fast-forwards the clean source checkout only after the registration is
   complete.

Submit does not create a Verification Record, Decision, Event, or accepted
Standing.

## Budgets

Missions bound prompt bytes, Artifact bytes, attempts, processes, output, wall
time, and observed provider tokens. The same verifier budget covers initial and
clean-clone replay. Provider-reported token totals are verified post hoc; the
subscription CLI does not expose a portable pre-call billing cutoff.
