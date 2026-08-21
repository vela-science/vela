# Bounded inherited-correction benchmark

This artifact preregisters one cold-successor comparison. It asks whether a
Vela-organized read packet lowers continuation and reassessment cost relative
to Git and ordinary documents containing the same information.

The case is deliberately synthetic. One upstream calibration Claim changes
from factor 10 to factor 12. A bounded chain then contains one directly
affected calculation, one downstream Claim that must be reassessed, one
discovery-only Claim that is unaffected, and one aggregate that is presently
unprovable because an exact required input is absent. This topology tests
classification and safe continuation without pretending that a toy case is a
scientific correction or accepted Standing.

## What is frozen

- `preregistration.json` binds the fixed denominator, timeout, scoring gate,
  candidate-visible inputs, protected adjudication, scorer/capture/test bytes,
  authorization template, and both packet roots.
- `amendment.v1.json` retains the first registration and independent BLOCKED
  review, then records the prospective scorer/custody repair made before any
  experimental session or result access.
- `fixture/public-facts.json` and `fixture/{source,evidence}/` are the common
  source facts and exact bytes.
- `conditions/git-documents/` presents those facts as history, Claim,
  dependency, source, and evidence documents.
- `conditions/vela/` presents the same facts through a correction/replay
  projection and per-Claim `why` records.
- `input-equivalence.json` binds both generated packet roots to one atomic fact
  set and one exact source/evidence set.
- `scoring/adjudication.json` contains protected labels and action rules. It is
  not copied into either candidate packet.
- `result.json` records `not_run`. There is no positive result to report.

The Vela presentation is non-authoritative and adds no protocol object. It
does not invoke Repository authority, create a Decision, change Standing, or
mutate a scientific Repository.

## Deterministic qualification

From the Vela source root:

```bash
python3 paper/artifacts/inherited-correction-benchmark/benchmark.py verify
python3 paper/artifacts/inherited-correction-benchmark/test_benchmark.py
git diff --check
```

`build` deterministically regenerates the two packets, equivalence proof,
preregistration bindings, unrun result, and manifest. Review uses `verify`,
which regenerates those bytes in memory and rejects drift.

## Confirmatory capture protocol

The separate confirmatory registration records the user's bounded execution
authorization, but every permit remains held pending exact independent
prelaunch PASS. The retired `start`/`finish` path cannot create a scorable run.
After one authorized one-shot container attempt is terminal, the only ingest
path is:

```bash
python3 paper/artifacts/inherited-correction-benchmark-execution/confirmatory-custody.py ingest \
  --capture-dir <exact-run-capture> --runs-dir <runs> --run-id <exact-run-id>
```

The bridge requires the assigned atomically consumed permit, launch record,
terminal receipt, provider-event stream, stderr, and any runtime response. It
revalidates their byte digests and every registration, authorization,
assignment, shared-to-condition configuration mapping, prompt, packet, image,
trust, runtime, identity, attempt, timeout, status, time, usage, event, and
response relationship before generating one benchmark run record.

After exactly eight no-retry sessions per arm are complete:

```bash
python3 paper/artifacts/inherited-correction-benchmark/benchmark.py freeze \
  --runs-dir <runs>
python3 paper/artifacts/inherited-correction-benchmark/benchmark.py score \
  --runs-dir <runs> --output <runs>/scored-result.json
```

`freeze` requires exactly the fixed sixteen bridge-generated records and their
complete runtime custody. It writes a content-bound capture manifest without
opening the answer key. `score` recomputes that custody and refuses to access
the key unless the manifest exactly matches every run, consumed permit,
terminal receipt, event stream, response, authorization, packet, and root. It
then buffers each capture-listed run and response exactly once, derives the
scored capture root from those immutable bytes, and never reopens the mutable
paths during scoring.
Wrong, invalid, failed, and timed-out sessions remain in the denominator at the
600-second cost cap. There are no retries, substitutions, post-hoc rescoring,
or manual semantic overrides.

A future passing gate would be a bounded descriptive signal for this one
synthetic packet. It would not establish scientific truth, general
productivity, external adoption, or a Vela authority role.
