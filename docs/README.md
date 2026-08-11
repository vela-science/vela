# Vela documentation

## Start here

- [Quickstart](QUICKSTART.md)
- [Producer quickstart](PRODUCER_QUICKSTART.md)
- [Agent quickstart](AGENT_QUICKSTART.md)
- [CLI contract](CLI.md)
- [Terminology](TERMINOLOGY.md)

## Current contracts

- [Protocol](PROTOCOL.md)
- [Ecosystem](ECOSYSTEM.md)
- [Architecture](ARCHITECTURE.md)
- [Authority and attribution](SIGNING.md)
- [Verification](VERIFICATION.md)
- [Roots](ROOTS.md)
- [Repository profile](REPOSITORY_PROFILE.md)
- [Scientific State Profile v1](interop/scientific-state-profile.md)
- [Threat model](THREAT_MODEL.md)
- [Continuity](CONTINUITY.md)
- [Provider-loss qualification, 2026-08-09](PROVIDER_LOSS_QUALIFICATION_2026-08-09.md)
- [Ecosystem completion ledger, 2026-08-09](ECOSYSTEM_COMPLETION_2026-08-09.md)
- [Gittuf authority deletion spike](GITTUF_AUTHORITY_DELETION_SPIKE.md)
- [Current repository origin ADR](adr/0027-pre-release-current-state-compaction.md)
- [Math-first compounding product architecture ADR](adr/0025-math-first-compounding-product-architecture.md)
- [Proposed correction benchmark and whitepaper evidence contract ADR](adr/0026-correction-benchmark-and-whitepaper-evidence-contract.md)
- [Living Frontier map and native-system boundary ADR](adr/0028-living-frontier-map-and-native-system-boundary.md)
- [Closed foreign-reference experiment ADR](adr/0029-derived-foreign-reference-and-local-authority-containment.md)
- [Math Atlas, Math Source Registry, and Target-closure ADR](adr/0030-root-bound-math-source-registry-atlas-and-target-closure.md)
- [One product and removable Agent executor ADR](adr/0031-one-product-and-removable-agent-executor.md)
- [Self-authenticated evidence and human Decision authority ADR](adr/0032-self-authenticated-evidence-and-human-decision-authority.md)
- [Direct Submission lineage and Registration retirement ADR](adr/0033-direct-submission-lineage-and-registration-retirement.md)

## Project documents

- [Current core work](CAMPAIGN.md)
- [Portable-waist campaign](PORTABLE_WAIST_CAMPAIGN.md)
- [Protocol breakthrough benchmark](BREAKTHROUGH_BENCHMARK.md)
- [Whitepaper evidence contract](WHITEPAPER_CONTRACT.md)
- [Genesis: open models and the scientific-state control point](integrations/genesis-open-models.md)
- [Roadmap](ROADMAP.md)
- [Interoperability boundary](INTEROPERABILITY.md)
- [Protocol adoption and interoperability](PROTOCOL_ADOPTION.md)
- [Repository ownership boundaries](REPOSITORY_BOUNDARIES.md)
- [Theory](THEORY.md)
- [Publishing](PUBLISHING.md)

This index covers `docs/` completely: a test in `vela-protocol` walks the tree
and holds every current document to a link here, because an index nobody checks
is how three documents came to be published on the web while this page had
never heard of them. The groups are editorial; the coverage is not.

It walks rather than lists a directory because the first version read the top
level only, and the interoperability profile and the Genesis dossier were then
published one directory down, where nothing could see them and no page in the
repository linked either.

[Decision records](adr/README.md) index every ADR in number order. They are
linked as a directory rather than listed here because an ADR is a decision at a
moment and this page is what holds now.

[Historical documents](history/README.md) retain predecessor language,
rejected designs, and dated assessments outside the current documentation
surface.

Accepted ADRs preserve decisions. Proposed ADRs describe candidates and their
evidence gates. Git history preserves superseded text; active documents do not
repeat it as compatibility behavior.

Case-specific execution plans live with their canonical owners. Vela core
retains no reviewer-recruitment or source-specific campaign surface.
