# Canopus run records

A completed run writes these local records beneath its run root:

- `activity.jsonl`: append-only orchestration events linked by full SHA-256
  digests, including the exact `vela next` offer digest and selected rank;
- `candidate.json`: the frozen candidate and verifier facts;
- `worker-events.jsonl`: the raw Codex event stream used as the tool trace;
- `worker-final.json`: the exact final worker response;
- `worker-stderr.bin`: the bounded raw worker stderr stream;
- `engine-result.json`: the parsed worker outcome and usage;
- `run.json`: the compact run record, exact roots, landing result, costs,
  clean-clone reproduction result, and stage-typed observations;
- `evidence-manifest.json`: content roots for the worker, run, candidate,
  artifacts, verifier result, Receipt when present, and final frontier roots;
- `projection.json`: a read-only view for another consumer;
- `landing-command.json`: the exact argv, exit status, raw streams, and stream
  digests observed at the potentially effectful `vela land` boundary;
- `landing-observation.json`: the parsed raw Vela result, retained before any
  Canopus interpretation.

If anything fails at or after that boundary, `landing-recovery.json` records
the last command, raw result when available, parsed route when available, and
freshly inspected roots. `failure.json` points to that recovery record. A
command timeout may have changed external state even when no complete stdout
was captured, so recovery roots are authoritative over assumptions.

These files are operational evidence, not Vela authority. The projection says
so in its schema and can be rebuilt with `canopus inspect run.json`. Deleting
the run-record files must not change Vela replay, a policy route, or accepted
state.

Completed runs now use `canopus.run.v1`. Its `observations` object separates
three claims that must not be collapsed:

- `worker_observations`: what the model reported or attempted;
- `verifier_observations`: what the frozen mechanical verifier established;
- `standing_caveats`: limitations that remain true after verification.

Immutable `canopus.run.v0` records remain readable and replayable. Canopus does
not rewrite them to manufacture the new evidence categories.

The installed package exposes the stable run-record parsers and projections for
read-only consumers. Public evidence is assembled by the consuming repository
from those rooted records; Canopus does not maintain a second publication
bundle or Observatory-import format. Never publish a raw run directory,
isolated homes, authentication, private paths, or unrestricted logs.

The run root also contains isolated checkouts and content-addressed artifacts.
The landing clone uses a disposable attached branch so Vela can publish exact
deltas; the immutable input clone remains detached and read-only. Successful
runs delete the isolated Vela home and its agent-only key. Failed runs may
retain local diagnostic state until explicit cleanup. Never publish a run root
or Codex credential directory wholesale. Canopus does not author Git commits.
It supplies the frozen source artifacts to `vela land`; Vela alone commits the
exact Receipt, activity record, proposal, source artifacts, and retained
`records/artifacts/sha256/<digest>` copies. `activity.jsonl` records Vela's
publication facts and the resulting roots.

A `--no-land` run writes `canopus.diagnostic-run.v1`, invokes the same frozen
worker and verifier path, and leaves the source frontier at its starting commit
and tree. It has no Receipt or proposal. A landed product run performs all work
in isolated clones and returns a clean-clone-reproduced local candidate with its
exact repository, commit, and tree. The source checkout remains unchanged.
Publishing or otherwise integrating that candidate is an explicit Frontier
operator action outside Canopus.

Product output must live outside the source frontier and outside known
cloud-synced Desktop or cloud-storage roots. Docker bind mounts over cloud-backed
paths can stall without producing a verifier verdict, so Canopus refuses that
placement rather than weakening the verification timeout.

`route: defer` with `accepted_event_delta: 0` means the Receipt v1 record is
pending human review. It is a successful producer handoff, not scientific
acceptance. `route: permit` is valid only when an already signed Vela policy
admits the exact proposal; Canopus still does not hold a human key.

## Budget semantics

The mission hard-bounds prompt bytes, artifact bytes, attempts, spawned
research-lane processes, and captured research-lane output. The same ledger
covers the first verifier and clean-clone verifier. `max_observed_tokens` is an
honest postcondition over provider-reported usage; the Codex subscription
surface does not expose a portable pre-call token cutoff. Git and Vela are the
control plane and retain their own fixed command timeout/output bounds. The
mission therefore does not advertise a hard end-to-end subprocess or billing
budget it cannot enforce.
