# Decision records

Every ADR in this directory, in number order. `docs/README.md` indexes the
current contracts and links here rather than listing these, because an ADR is a
decision at a moment and a contract is what holds now; the two age differently
and mixing them makes both harder to read.

An ADR is never edited to reflect a later decision. A superseded one stays as it
was and the ADR that replaced it says so, which is why this list only grows.

Held to the directory by `cli_release_contract.rs`: a file here that is missing
from this list fails the build. That test used to skip this directory on the
grounds that it kept its own index, and it did not.

- [ADR 0001: A frontier should be a git repo, not a parallel git](0001-frontier-as-git-repo.md)
- [ADR 0002: Stable-id + revision-digest pairing is deferred; continuity is a back-pointer chain](0002-stable-id-plus-revision-digest-deferred.md)
- [ADR 0003: Preserve the rigorous core behind a task-first workflow](0003-rigorous-core-task-first-workflow.md)
- [ADR 0004: Falsify the need for a scientific dependency primitive](0004-verifiable-scientific-composition-experiment.md)
- [ADR 0005: Temporal actor registration and pre-registration history](0005-temporal-actor-registration-and-pre-registration-history.md)
- [ADR 0006: Independent correction-aware handoff and standards baseline](0006-independent-correction-aware-handoff-and-standards-baseline.md)
- [ADR 0007: Full-digest claim revision references](0007-full-digest-claim-revision-references.md)
- [ADR 0008: Signed frontier checkpoint continuity](0008-signed-frontier-checkpoint-continuity.md)
- [ADR 0009: Exact dependency pins and deterministic standing](0009-exact-dependency-pins-and-deterministic-standing.md)
- [ADR 0010: Everyday product contract and experimental surface retirement](0010-everyday-product-contract-and-experimental-surface-retirement.md)
- [ADR 0011: Human-governed authority and producer withdrawal](0011-protected-single-decision-approval-and-producer-withdrawal.md)
- [ADR 0012: Cross-platform public-beta distribution and protected policy administration](0012-cross-platform-public-beta-distribution-and-protected-policy-administration.md)
- [ADR 0013: Exact verifier and work-binding constraints for Permit](0013-exact-verifier-and-work-binding-constraints-for-permit.md)
- [ADR 0014: Policy-scoped producer credentials for exact Permit](0014-policy-scoped-producer-credentials-for-exact-permit.md)
- [ADR 0015: Optional Erdős knowledge export and reader boundary](0015-optional-erdos-knowledge-export-and-reader-boundary.md)
- [ADR 0016: Frontier Repository Profile v1 and legacy identity migration](0016-frontier-repository-profile-v1-and-legacy-identity-migration.md)
- [ADR 0017: Kernel, Frontier Algebra, and Discovery Calculus boundaries](0017-kernel-frontier-algebra-and-discovery-calculus-boundaries.md)
- [ADR 0018: Authenticated historical dependency states](0018-authenticated-historical-dependency-states.md)
- [ADR 0019: Versioned semantic packages and workbench-adapter boundaries](0019-versioned-semantic-packages-and-workbench-adapter-boundaries.md)
- [ADR 0020: Attributed repository authority](0020-attributed-repository-authority-and-standard-delegation.md)
- [ADR 0021: Scientific Submission and direct-action CLI language](0021-scientific-submission-and-direct-action-cli-language.md)
- [ADR 0022: Current repository epoch and legacy runtime retirement](0022-current-repository-epoch-and-legacy-runtime-retirement.md)
- [ADR 0023: Native current repository genesis](0023-native-current-repository-genesis.md)
- [ADR 0024: Product monorepo and integration-repository retirement](0024-repository-ownership-and-integration-repository-retirement.md)
- [ADR 0025: Math-first compounding product architecture](0025-math-first-compounding-product-architecture.md)
- [ADR 0026: Correction benchmark and whitepaper evidence contract](0026-correction-benchmark-and-whitepaper-evidence-contract.md)
- [ADR 0027: Pre-release current-state compaction](0027-pre-release-current-state-compaction.md)
- [ADR 0028: Living Frontier map and native-system boundary](0028-living-frontier-map-and-native-system-boundary.md)
- [ADR 0029: Derived foreign reference and local authority containment](0029-derived-foreign-reference-and-local-authority-containment.md)
- [ADR 0030: Root-bound Math Source Registry, Atlas, and Target closure](0030-root-bound-math-source-registry-atlas-and-target-closure.md)
- [ADR 0031: One Vela product; native tools remain external](0031-one-product-and-removable-agent-executor.md)
- [ADR 0032: Self-authenticated evidence; human Decision authority](0032-self-authenticated-evidence-and-human-decision-authority.md)
- [ADR 0033: Direct Submission lineage; Registration retirement](0033-direct-submission-lineage-and-registration-retirement.md)
- [ADR 0034: Direct Target Index generation](0034-direct-target-index-generation.md)
- [ADR 0035: Commodity encoding, signing, and wire contracts](0035-commodity-encoding-signing-and-wire-contracts.md)
- [ADR 0036: Flagship mathematical breakthrough campaign](0036-flagship-mathematical-breakthrough-campaign.md)
- [ADR 0037: Session-authenticated local repository authority](0037-session-authenticated-local-repository-authority.md)
- [ADR 0038: Problem map and frontier-to-commons foundry](0038-problem-map-and-frontier-to-commons-foundry.md)
- [ADR 0039: Repository is the authority boundary; Frontier is derived](0039-repository-authority-and-derived-frontiers.md)
- [ADR 0040: A producer-declared dependency on `vela.submission.v1`](0040-producer-declared-claim-dependencies.md)
- [ADR 0041: A language-independent conformance vector for the authority contract](0041-authority-conformance-vector.md)
- [ADR 0042: Policy-bundle rotation, and what it takes to retire Cedar](0042-policy-bundle-rotation-and-cedar-retirement.md)
- [ADR 0043: Experiment first with exact, artifact-backed Claim dependencies](0043-experiment-first-exact-claim-dependencies.md)
- [ADR 0044: Constrain Frontier Calculus to research vocabulary](0044-constrain-frontier-calculus-to-research-vocabulary.md)
- [ADR 0045: Scientific coordination stops at the state boundary](0045-scientific-coordination-boundary.md)
- [ADR 0046: Capability-based, attributed Decisions](0046-attributed-actor-decisions.md)
- [ADR 0047: Native repository integration is non-authoritative](0047-native-repository-integration-boundary.md)
