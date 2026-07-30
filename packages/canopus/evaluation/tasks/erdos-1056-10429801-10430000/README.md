# Held-out Erdős 1056 task

This source-only task freezes the current first non-overlapping range
`10429801..10430000` for a possible future registered evaluation. It does not
register that evaluation and contains no candidate artifact, preflight result,
verifier output, Submission, Verification Record, or Decision.

`packet.json` is the complete model-visible input. It binds:

- the clean public Erdős Frontier checkout, Git tree, and repository root;
- the generated Target Index and exact `erdos:1056` packet;
- accepted coverage through `10429600`;
- producer-complete work pending review through `10429800`; and
- the next exact inclusive range and closed artifact format.

The packet deliberately projects only the information needed to perform the
bounded task. It does not copy the Frontier corpus or expose the verifier.

`capsule.json` binds the existing generic Erdős verifier source, exact
compile-time bounds, pinned build image, Docker client, and expected static
Linux amd64 executable root. `build-verifier.mjs` compiles that capsule without
running the scientific search. `verify.mjs` is retained for a later registered
study and must remain outside every model-readable workspace.

Prepare the packet only from the exact clean Frontier:

```bash
bun evaluation/tasks/erdos-1056-10429801-10430000/prepare.mjs \
  --frontier /path/to/erdos-frontier \
  --output /new/answer-free-task
```

Build the verifier capsule separately:

```bash
bun evaluation/tasks/erdos-1056-10429801-10430000/build-verifier.mjs \
  --source capsules/erdos1056-k15/verifier.cpp \
  --docker /path/to/docker \
  --output /private/verifier
```

A future plan may declare this task held out only if a fresh leakage audit
still finds no candidate answer before registration. Mechanical verifier
passage would be evaluation evidence only, never scientific Standing.
