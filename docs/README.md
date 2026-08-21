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

## History

[Decision records](adr/README.md) preserve the design sequence. They are
historical records, including ADRs for designs later retired. The current
documents above define the product now.

[Historical documents](history/README.md) retain selected migrations,
qualifications, and rejected designs with dated labels. Git history retains
obsolete campaign and planning material that no current consumer needs.
