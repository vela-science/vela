# ADR 0012: Optional external activity recorders with no authority effect

- Status: Proposed
- Candidate release: none
- Protocol effect: none
- Run-schema effect: none
- Vela authority effect: none
- Entry gate: one private, noncanonical, preregistered pilot

## Context

Products such as Entire can preserve prompts, responses, file activity,
checkpoints, and session ancestry. That context may help a cold successor or
reviewer understand how work was produced. It does not provide Canopus's
bounded mission, custody, Artifact, verifier, budget, and clean-clone
contracts, and it does not provide Vela's Verification, Decision, Event, or
Standing semantics.

Canopus should not become a generic transcript or checkpoint product. It
should also not ignore a replaceable upstream tool that may measurably improve
scientific continuation.

## Proposed decision

1. External activity recorders are optional activity-plane workbench
   integrations.
2. Vela and published Canopus have no runtime dependency on Entire or another
   recorder.
3. A checkpoint is not a Submission, Verification Record, Decision, Event, or
   accepted state.
4. No raw transcript enters a Frontier or Canopus Run by default.
5. Provider and checkpoint identifiers are opaque locator metadata.
6. Experiments use documented, version-pinned machine-readable CLI or API
   contracts, never private Git layouts.
7. Public-source projects use private checkpoint storage by default.
8. Canonical Frontier writers and controlled Canopus workers run with recorder
   hooks disabled until byte-identity and custody tests prove compatibility.
9. No schema, CLI, package, or Observatory surface is added before a measured
   pilot passes.
10. Failure to show material continuation or review value deletes the
    provider-specific experiment.

## Pilot contract

Compare:

```text
A: Git + Vela state + Canopus Run + ordinary documentation
B: the same material + metadata-only external activity context
```

Freeze one completed scientific artifact, its exact sources, verifier, budget,
and scorer. The only treatment difference is the activity context.

Measure:

- time to the first correct next action;
- evidence-location and reviewer time;
- correction and caveat comprehension;
- tokens, tool calls, wall time, and setup burden;
- incorrect-action rate; and
- secrets, private content, or irrelevant transcript material exposed.

The pilot passes only when all authority, custody, replay, privacy, and
canonical-commit invariants hold and one preregistered primary metric improves
by at least 20 percent without a material regression elsewhere.

## Compatibility gate

Before any recorder is enabled in a canonical Frontier, a disposable matrix
must prove that installed, disabled, enabled, unavailable-remote, and
fully-removed states produce the expected Git commit bytes, Vela roots,
authority records, Events, Standing, strict replay, and push behavior.

An ambient hook that changes a reviewed commit, adds a trailer to a Vela
authority commit, exposes a credential, or makes replay depend on recorder
state fails the experiment.

## Conditional adapter

Only after the pilot and compatibility gate pass may Canopus consider a
provider-neutral, metadata-only sidecar. It must:

- use a documented machine-readable provider boundary;
- bind exact CLI version, Git commit/tree, metadata digest, capture scope, and
  known gaps;
- retain no transcript by default;
- grant no network, repository, verifier, or authority capability;
- remain outside the `canopus.run.v2` root initially; and
- be removable without changing a Run, Submission, Verification, or Standing.

The first provider adapter remains source-only. A shared adapter contract still
requires the two-format and deleted-duplication gate in ADR 0008.

## Rejected alternatives

- replace Git or Canopus with Entire;
- make a recorder mandatory for Vela;
- run recorder hooks inside the native Canopus worker;
- create an `EntireCheckpoint` protocol object;
- turn checkpoints automatically into Submissions or Verifications;
- publish transcripts as canonical Frontier evidence by default; or
- build a generic Vela transcript service.

## Consequences

Canopus can measure whether upstream activity memory improves continuation
without expanding its public contract. Vela remains replayable with every
recorder absent. A failed pilot leaves only the generic architectural
clarification and a rooted negative result.
