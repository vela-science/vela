# Vela documentation

Vela `v0.977.4` is the current signed pre-1.0 release. It reads and writes the
Protocol 1 release candidate with Submission v3.

## Start here

- [Quickstart](QUICKSTART.md): install Vela, replay the public Math Repository,
  submit a Result, record a scoped check, and make an attributed Decision.
- [CLI contract](CLI.md): shipped commands, flags, JSON, refusal states, and
  recovery behavior.
- [Current evidence](EVIDENCE.md): demonstrated behavior, the bounded Phase 0
  comparison, and open external-validation gates.
- [Repository boundaries](REPOSITORY_BOUNDARIES.md): what Core owns and what
  remains with source repositories, workbenches, and read products.
- [Standards disposition](STANDARDS_DISPOSITION.md): which generic facts stay
  native and how the five conceptual views map to current protocol objects.
- [Glossary](GLOSSARY.md): canonical current terms and their authority effects.

## Choose your job

### Read and reconstruct scientific state

- [Quickstart](QUICKSTART.md)
- [Current evidence and validation gates](EVIDENCE.md)
- [Roots](ROOTS.md)
- [Continuity and replay](CONTINUITY.md)
- [Repository profile](REPOSITORY_PROFILE.md)

### Produce, check, and decide

- [CLI contract](CLI.md)
- [Verification Records](VERIFICATION.md)
- [Authority and performer attribution](SIGNING.md)
- [Publishing with Git](PUBLISHING.md)
- [Review Method examples](../examples/review-methods/)

### Operate and secure a Repository

- [Threat model](THREAT_MODEL.md)
- [Authority and performer attribution](SIGNING.md)
- [Release, installation, and recovery](RELEASES.md)
- [Repository profile](REPOSITORY_PROFILE.md)

### Integrate another tool or repository

- [Interoperability](INTEROPERABILITY.md)
- [Architecture](ARCHITECTURE.md)
- [Native Repository integration](integrations/native-repository-integration-v0.1.md)
- [Integration-profile template](integrations/integration-profile-template.md)
- [Genesis open-model integration record](integrations/genesis-open-models.md)

### Implement or audit Protocol 1

- [Protocol 1](PROTOCOL.md)
- [Architecture](ARCHITECTURE.md)
- [Protocol 1 conformance profile](interop/scientific-state-profile.md)
- [Roots](ROOTS.md)
- [Continuity and replay](CONTINUITY.md)

## Current boundaries

- A Submission authenticates producer evidence. It does not accept a Claim.
- A Verification Record reports one scoped observation. It does not accept a
  Claim.
- Only an authorized, attributed Decision changes Standing.
- Git publishes exact bytes. A merge or push does not create scientific
  acceptance.
- Vela records scientific inheritance. It does not run domain work, search the
  literature, rank Problems, or allocate research effort.

## Current supervised campaign

- [VELA-COMPOSE-1 campaign index](campaigns/vela-compose-1/README.md)
- [Campaign charter](campaigns/vela-compose-1/CAMPAIGN.md)
- [Repository baseline](campaigns/vela-compose-1/BASELINE.md)
- [Append-only campaign state](campaigns/vela-compose-1/STATE.md)
- [Append-only campaign decisions](campaigns/vela-compose-1/DECISIONS.md)
- [Frozen experiment registry](campaigns/vela-compose-1/EXPERIMENTS.md)
- [Append-only results](campaigns/vela-compose-1/RESULTS.md)
- [Anomaly reopen gate](campaigns/vela-compose-1/REOPEN.md)
- [Campaign risks](campaigns/vela-compose-1/RISKS.md)
- [T1 Kernel contract](campaigns/vela-compose-1/threads/T1_KERNEL.md)
- [T2 Replay contract](campaigns/vela-compose-1/threads/T2_REPLAY.md)
- [T2 replay and receipt qualification](campaigns/vela-compose-1/T2_REPLAY_REPORT.md)
- [T3 Branching contract](campaigns/vela-compose-1/threads/T3_BRANCHING.md)
- [T4 Lean contract](campaigns/vela-compose-1/threads/T4_LEAN.md)
- [T5 Alzheimer contract](campaigns/vela-compose-1/threads/T5_ALZHEIMER.md)
- [T6 Handoff contract](campaigns/vela-compose-1/threads/T6_HANDOFF.md)
- [T7 Release contract](campaigns/vela-compose-1/threads/T7_RELEASE.md)

## History

[Decision records](adr/README.md) preserve the design sequence. They are
historical records, including ADRs for designs later retired. The current
documents above define the product now.

[Historical documents](history/README.md) retain selected migrations,
qualifications, and rejected designs with dated labels. Git history retains
obsolete campaign and planning material that no current consumer needs.
