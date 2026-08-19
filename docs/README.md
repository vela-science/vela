# Vela documentation

Start with the [quickstart](QUICKSTART.md). The current signed release is
`v0.977.3`; Vela Core reads and writes Protocol 1 with Submission v3.

## Current product and protocol

- [Protocol 1](PROTOCOL.md) defines current objects and semantics.
- [Architecture](ARCHITECTURE.md) maps Core components and their consumers.
- [CLI reference](CLI.md) lists the shipped command and JSON contracts.
- [Repository profile](REPOSITORY_PROFILE.md) defines source-owned layout.
- [Roots](ROOTS.md) defines content and repository identity.
- [Continuity](CONTINUITY.md) explains replay across exact Git history.
- [Interoperability](INTEROPERABILITY.md) defines the portable boundary.
- [Protocol 1 conformance profile](interop/scientific-state-profile.md)
  identifies the normative schemas and vectors.

## Authority, verification, and security

- [Authority and performer attribution](SIGNING.md) separates the authority
  principal, signer, and human or agent performer.
- [Verification](VERIFICATION.md) separates scoped checks from Decisions.
- [Repository boundaries](REPOSITORY_BOUNDARIES.md) assigns work to Core,
  source-owning repositories, and read products.
- [Threat model](THREAT_MODEL.md) names the defended trust boundaries.

## Installation and operations

- [Release qualification](RELEASES.md) covers signed manifests and assets.
- [Publishing](PUBLISHING.md) covers Repository publication.
- [Quickstart](QUICKSTART.md) covers install, read, submit, verify, decide,
  replay, and explicit recovery.

## Integrations

- [Native Repository integration](integrations/native-repository-integration-v0.1.md)
- [Integration-profile template](integrations/integration-profile-template.md)
- [Genesis open-model integration record](integrations/genesis-open-models.md)

[Decision records](adr/README.md) preserve the decision sequence. They are
historical records, including ADRs that described later-retired designs; the
current documents above define the product now.

[Historical documents](history/README.md) retain selected migrations,
qualifications, and rejected designs with explicit historical labels. Git
history retains obsolete planning and campaign scaffolding that no current or
reproducibility consumer needs.
