# Changelog

## Unreleased

- Make `authority trust pin` idempotent and allow a migrated Frontier's public
  local pin to advance only through an exact
  `--previous-record-root` compare-and-swap. The new root must still match the
  current sequence-one authority record; no key is read and no Frontier byte
  changes.
- Retire orphan Atlas summaries, derived discovery catalogs, predecessor-era
  research prototypes, the unrefined top-level Lean model suite, and its dead
  manual CI lane. Git history remains the archive.
- Keep independent Python and TypeScript reducers plus the JavaScript current
  object emitter as focused `conformance/` implementations rather than inactive
  client SDK products.
- Keep only the two active frozen-witness example sets used by public
  reproduction CI.
- Remove unconsumed source-adapter samples, predecessor schema experiments,
  unused graph-analysis projections, and the superseded
  `vela-protocol-core` Conjecture/ProofPacket crate. Current Claim, Submission,
  Verification, and repository contracts remain in the protocol waist.
- Remove Vela's abandoned research-trace, packet-export, agent-attestation,
  proposal-embedded Run/tool/permission trace, Diff-Pack attestation link,
  agent-object registry, and MCP-server configuration surfaces. Canopus owns
  optional execution Runs, traces, capsules, and evaluation; Vela retains only
  local producer identity plus exact CLI intake and canonical state.
- Make the deterministic Attempt identifier vector part of the routine
  cross-implementation conformance run instead of leaving it documented but
  unexecuted.

## v0.940.9 — 2026-07-28 — Decision publication verification repair

- Verify an accepted Decision's linked scientific event by its
  signature-independent semantic identity and exact authority transaction,
  without depending on authority-event filename order.
- Verify the transaction-owned Target Index against the exact new repository
  root before publication, then retain the tracked-HEAD check after the Git
  commit exists.
- Recovered the already-signed Erdős Decision without retrying authority,
  rewriting history, or changing any signed bytes.

## v0.940.8 — 2026-07-28 — Completed-journal rolling-head repair

- Fixed the completed-history recovery barrier so a valid later current-object
  transaction may replace the authenticated rolling `.vela/repository.json`
  head without being rejected by an older completed transaction generation.
- Kept active transaction installation and completion byte-exact, and kept
  immutable events, authority records, Proposals, Receipts, and evidence
  postimages fail closed.
- Added a regression reproducing the Erdős Verification import failure and
  proving that unrelated canonical-evidence drift still blocks recovery.

## v0.940.7 — 2026-07-28 — Current Target Index writer repair

- Fixed current Submission and Verification writers so an exact Target Index
  rebind is verified as a transaction-derived postimage before publication and
  strictly against Git only after the publication commit exists.
- Added `targets.json` to the closed public Frontier topology so the exact
  derived rebind is included in the same local commit as its repository
  transition.
- Added a clean-clone regression covering Submission registration in a current
  repository with a sealed Target Index. Tracking, repository roots, accepted
  state, and replay remain fail closed.

## v0.940.6 — 2026-07-28 — Retained artifact reference reuse repair

- Reuse an exact retained Artifact reference when a new current Submission
  binds the same root and canonical path.
- Preserve the predecessor reference bytes instead of treating its historical
  schema label or raw-hex identifier as a repository-identity collision.
- Continue to reject the same Artifact root at a different path.
- No protocol, object, authority, accepted-state, or canonical-history
  semantics change.

## v0.940.5 — 2026-07-28 — Terminal imported Verification replay repair

- Resolve a retained predecessor-scoped Verification Record through the
  Proposal's immutable rooted Claim after rejection removes that Claim from
  the pending index.
- Preserve fail-closed canonical Claim, Proposal, Submission, and full-root
  checks without rewriting imported evidence.
- Report any future post-commit verification failure as an already-committed
  repository-authority transaction so operators do not retry a Decision.
- No protocol, object, authority, or accepted-state semantics change.

## v0.940.4 — 2026-07-28 — Agent Decision boundary guidance repair

- Remove the retired protected-request language from the generated Frontier
  skill and state the current repository contract precisely: agents may
  prepare or explain one exact Decision command, but may not invoke it.
- Keep the authored plugin skill and both generated skill adapters
  byte-identical, with a regression rejecting the stale authority language.
- No protocol, repository, object, authority, or CLI execution semantics
  change.

## v0.940.3 — 2026-07-27 — Migrated Verification lineage repair

- Resolve retained predecessor-scoped Verification Records through the exact
  current Proposal and Claim `imported_from` lineage.
- Include the same exact migrated observations in `review show` and the
  repository-authority Decision Plan without rewriting their signed bytes.
- Reject mismatched Proposal, Claim, or Submission lineage and retain the
  existing strict unique-mapping requirement.

## v0.940.2 — 2026-07-27 — Current Submission integration repair

- Admit current Claim records through the canonical-evidence write boundary
  while preserving the separate Proposal and review classes.
- Verify every mutable repository-manifest root through its signed authority
  delta chain instead of comparing current bytes with the initialization
  record.
- Publish Profile v2 Submission transactions as exact local Git commits.
- Add an end-to-end authenticated Submission regression proving pending
  Standing, zero accepted-state delta, strict replay, and clean-clone
  reproduction.

## v0.940.1 — 2026-07-27 — GitHub Action attestation fix

- Expose the implicit workflow token to the composite action's release
  attestation checks, so consumers can use the documented action without
  duplicating `GH_TOKEN` wiring.
- No protocol, repository, object, authority, or CLI behavior changes.

## v0.940.0 — 2026-07-27 — One current repository contract

- Make Profile v2, Claim Record v1, Submission v1, Registration Record v1,
  Verification Record v1, Proposal v1, and repository authority the only live
  repository contract.
- Verify a signed repository-epoch boundary while removing the Era-0 parser,
  reducer, writers, compatibility projections, migration command, and
  one-time migration machinery from the daily binary.
- Initialize fresh repositories directly in Profile v2 and bind native
  `vela.repository-genesis.v1` through the same sequence-one authority
  transaction used by current repositories.
- Preserve historical audit through immutable predecessor commits, tags, and
  source archives rather than same-binary legacy replay.
- Keep the public product loop object-first:
  `inspect -> attempt -> submit -> verify -> decide -> continue`.

## v0.930.0-rc.13 — 2026-07-26 — Contract authority to standard providers

- Replace the custom protected-signer path with one attributed
  repository-authority decision: local OS principal authentication,
  restricted Cedar authorization, and exact DSSE signing through the standard
  OpenSSH agent.
- Remove the `vela-signer` crate and binary, identity-v2 custody, signer
  sessions, binary/helper rebind, platform prompt adapters, and one-time
  migration writers.
- Remove copied root/time confirmation from `review decide`; the exact command
  is the semantic human action and the recoverable transaction remains the
  write boundary.
- Contract release archives to one `vela` binary and the workspace to six
  crates while retaining byte-identical Era-0 replay.
- Retire obsolete signer-session schemas and rewrite current authority,
  command, verification, repository-profile, quickstart, and threat-model
  guidance around the single live writer.

## v0.930.0-rc.12 — 2026-07-26 — Preserve repository configuration during landing

- Keep `frontier.yaml` and every other repository-configuration surface out of
  repository-authority derived drafts. Transactional landing rematerializes
  only the closed Vela-owned view set (`frontier.json`, `vela.lock`,
  `.vela/proof-state.json`, and `proof/**`), preserving migrated frontier
  configuration byte-for-byte.
- Render those views from canonical repository bytes plus the new pending
  proposal, not from the workflow's effective in-memory lease overlay.
  Detached repository-authority claims remain available for work-session
  validation without leaking into `frontier.json` or `vela.lock`.
- Add the migrated-frontier regression exposed by the retained Erdős landing;
  the invalid rc.11 attempt failed before a transaction marker or proposal
  mutation.

## v0.930.0-rc.11 — 2026-07-26 — Materialize repository-authority landings atomically

- Install `frontier.json`, `vela.lock`, proof exports, and proof-state
  bookkeeping through the same recoverable transaction as a
  repository-authorized Receipt landing.
- Bind the exact derived postimages to the transaction journal and active
  recovery checks without adding them to the signed authority object delta or
  scientific write-set root.
- Reject derived-only authority attempts, unsupported derived paths,
  post-signing derived drift, and canonical/derived path overlap before a
  commit marker can authorize writes.
- Preserve completed-journal rematerialization behavior and every canonical
  event, Receipt, proposal, artifact, authority, and scientific-state
  commitment.

## v0.930.0-rc.10 — 2026-07-26 — Separate repository authorization from acceptance

- Record `urn:vela:policy:none` in a repository-authorized Receipt. Repository
  Cedar policy authorizes the exact canonical write; it does not constitute
  scientific acceptance and cannot occupy Receipt v1's AcceptancePolicy field.
- Keep the signed-agent `receipt_land` permission check fail-closed before
  Receipt authoring.
- Canonicalize signed agent event and activity-record authentication
  observations to whole-second UTC while retaining the full signed object root.
- Preserve every canonical Frontier byte and the independently verified Erdős
  repair artifact while unblocking its deferred, pending-review landing.

## v0.930.0-rc.9 — 2026-07-26 — Repair migrated strict state and producer preflight

- Count only strict blockers in compact status totals. Advisory migration and
  audit signals remain visible, but they no longer inflate the blocker count
  or make a migrated Frontier appear more broken than its exact strict state.
- Preflight repository-authorized producer work before any write. A missing
  routine producer policy now returns the exact protected administrator repair
  command while preserving the closed `vela.offer.v1` machine contract.
- Preserve all canonical Frontier bytes, scientific roots, authority records,
  and the rc.8 migration boundary. This candidate exists to prove the first
  useful repository-authority Receipt landing before the stable `0.930.0` cut.

## v0.930.0-rc.7 — 2026-07-25 — Stabilize generated statistics

- Serialize derived category and link-type counts through ordered maps so
  `frontier.json` is byte-identical across processes.
- Reproduce the defect against the exact Sidon Profile v1 checkout, then prove
  three-process materialization, strict replay, and unchanged canonical roots.
- Preserve the rc.6 protected migration behavior and the completed Formal
  authority history without adding a protocol or authority primitive.

## v0.930.0-rc.6 — 2026-07-25 — Canonicalize protected migration approval time

- Canonicalize the protected helper's authenticated approval observation to
  whole-second UTC before authority preflight, matching the closed
  `vela.authentication-observation.v1` contract.
- Add a regression for the nanosecond timestamp returned by the macOS helper
  and prove that the resulting local authentication observation validates
  without weakening the fail-closed transaction boundary.
- Preserve the rc.5 Target Index compatibility and all pre-migration history.

## v0.930.0-rc.5 — 2026-07-25 — Restore historical Target Index inspection

- Preserve bounded, unique Target Index v1 labels in their historical byte
  order during read-only inspection and migration planning. Target Index v2
  remains strictly sorted and canonical. This restores Quantum Codes' exact
  protected Profile v1 migration path without rewriting its derived v1 index
  or weakening producer-work sealing.
- Remove the unpublished, unconsumed `vela-hub` compatibility crate, its
  database/server dependency graph, deployment files, and active service
  documentation. Historical tags retain the complete implementation; local
  `vela serve` and the optional Observatory remain the supported read paths.
- Include `vela-authority` in release-version checks and crates.io publication
  order so the complete seven-crate `0.930` graph can be published.
- Advertise proposal-scoped reproduction only when the pending proposal retains
  a valid frontier-local frozen witness. Proposals backed by an external
  producer replay bundle remain fully inspectable, but no longer receive a
  broken `vela reproduce --proposal` next action.
- Constrain retained Receipt and witness reads to regular, non-symlink files
  inside the Frontier and verify their exact roots before use.
- Reproduce the real Quantum Codes v1 index at its unchanged Git commit and
  roots with zero writes while rejecting duplicate, oversized, invalid, or
  otherwise ambiguous legacy labels.

## v0.930.0-rc.2 — 2026-07-24 — Restore feature-independent historical replay

- Canonicalize synthetic source commitment preimages before hashing them.
  Ordinary `serde_json` object bytes changed when the CLI dependency graph
  enabled `preserve_order`, causing one historical Git anchor to derive a
  different source registry and snapshot root than the reusable library.
- Pin the same source commitment and source ID under both the narrow protocol
  test graph and the full CLI feature graph.
- Requalify the proposed repository-authority migration seam without changing
  a canonical Frontier byte or weakening repository-boundary verification.
- Supersede `v0.930.0-rc.1`, which remains an immutable failed candidate and
  must not be used for a migration ceremony.

## v0.930.0-rc.1 — 2026-07-24 — Qualify the repository-authority migration seam

- Add the proposed ADR 0020 dual-history verifier, restricted Cedar runtime,
  closed principals and capabilities, DSSE authority records, repository
  keysets, policy bundles, rotation, and terminal close.
- Add `vela authority migrate` as one temporary two-phase migration command:
  preview is key-free and write-free; apply rederives the exact plan under the
  recovery barrier, requests one final protected legacy continuity signature,
  authenticates the local operating-system principal, and records sequence 1
  through a standard OpenSSH-agent repository key.
- Bind the initial Cedar policy to one exact local administrator. Human review
  actions target proposals; an unbound human principal is denied.
- Preserve non-canonical historical event file bytes exactly while comparing
  their typed event content. Migration appends new canonical bytes and never
  normalizes or rewrites retained Era-0 history.
- Prove the composed command seam on a disposable Git Frontier, including
  cancellation before custody access, exact helper response validation,
  recoverable installation, a separate repository signature, and network-free
  clean-clone replay.
- Keep ADR 0020 Proposed. This candidate migrates no active Frontier, removes
  no Era-0 verifier, and is not the public `v0.930.0` release.
- Qualification note: live read-only replay exposed a feature-dependent source
  commitment and repository-anchor mismatch after this tag was published.
  The tag is retained as a failed candidate and superseded by `v0.930.0-rc.2`.

## v0.915.1 — 2026-07-24 — Align human and JSON strict-check verdicts

- Make the ordinary `vela check --strict` renderer use the same verified
  repository-context classification as `--json`.
- Report exact Profile v1 proposal identities retained at an independently
  pinned signed boundary as `anchored_immutable_unauthenticated` audit facts
  in both output modes.
- Continue to fail closed for every unanchored, added, altered, native
  Profile v1, legacy v0.1, or invalid-boundary proposal-ID conflict.
- Fix the real Formal Conjectures CI path without changing any event,
  proposal, reducer, scientific root, or authority rule.

## v0.915.0 — 2026-07-24 — Authenticate exact historical dependency states

- Accept ADR 0018 and allow an existing
  `vela.exact-frontier-dependency.v1` record to identify one exact retained
  ancestor of the first independently pinned repository temporalization
  boundary.
- Rederive hardened Git ancestry, the historical tree and replay, stable
  repository identity, event and proposal retention, every canonical retained
  byte, the empty dependency context, and Scientific State Root v2 before
  returning the existing dependency record.
- Continue to fail closed for forks, descendants, missing or shallow history,
  Git replacement or configuration attacks, altered retained objects,
  non-empty dependency contexts, ambiguous or invalid boundaries, wrong trust
  pins, and any expected-root mismatch.
- Pass the real read-only Erdős-to-Formal vector: the later signed Formal
  boundary authenticates Erdős's unchanged historical Formal commit, tree,
  and snapshot pin while preserving all 1,217 producer targets and the exact
  1,511-condition plus 81-actor Erdős strict debt.
- Add no event, object, signature, reducer transition, dependency schema, or
  scientific authority rule. Vela `0.914` histories replay unchanged.

## v0.914.2 — 2026-07-24 — Preserve exact legacy proposal identity debt

- Make `vela check --strict` agree with the Profile v1 repository write gate:
  a verified, externally pinned temporalization boundary may retain only the
  exact proposal logical-ID conflicts present at its Git anchor.
- Report those anchored historical IDs as
  `anchored_immutable_unauthenticated` audit facts. They confer no proposal
  authentication, acceptance, identity, or authority.
- Continue to fail closed for native Profile v1 conflicts, legacy repositories
  without a verified boundary, any new conflict, any changed conflicted
  proposal byte, an invalid boundary or trust pin, and every withdrawal
  conflict.
- Restore strict checking of the signed Formal Conjectures migration after
  ordinary regeneration of its derived `frontier.json`, `vela.lock`, and proof
  readouts. Canonical events, proposals, findings, artifacts, registries,
  accepted state, and scientific roots remain unchanged.

## v0.914.1 — 2026-07-24 — Preserve exact legacy signature debt

- Let a signed repository temporalization retain signature bytes already
  present at its exact Git/Vela anchor when the legacy display actor has no
  anchored public key. The bytes remain unauthenticated and confer no
  identity, standing, or authority.
- Continue to fail closed if such a signature is added, removed, replaced, or
  otherwise differs from the immutable anchor. Registered actors and
  repository-boundary events still require ordinary signature verification.
- Make the target-task Receipt fixture use a stable logical base path so the
  same release source passes from any clean checkout directory.
- Restore exact replay of the released Erdős Profile v1 repository without
  changing any event, proposal, Receipt, artifact, registry, policy, reducer,
  accepted-state, or scientific-state byte.

## v0.914.0 — 2026-07-23 — Bind portable frontier repositories

- Replace the permissive v0.1 repository manifest with closed Profile v1 and
  Settings v1 contracts. Minimal initialization now records the real scope,
  rejects unknown state-carrying fields, and separates human profile data,
  operational preferences, derived views, and canonical protocol bytes.
- Add one signed, non-scientific `frontier.repository_bound` event with exact
  repository identity, dependency, Git, event-prefix, registry, retained-byte,
  and Scientific State Root v2 commitments. The boundary governs repository
  administration and continuity; it grants no scientific acceptance.
- Add protected native binding and legacy temporalization ceremonies with
  proof of actor possession, out-of-band full-root trust pins, exact
  destination binding, recoverable transactions, and byte-preserving replay.
  Invalid, missing, forked, stale, or substituted context fails closed.
- Add Target Index v2 as a sealed, derived producer-work projection with exact
  candidate inputs, canonical rank, availability state, repair commands, and
  strict suppression when repository trust or roots do not verify.
- Make compact status, checks, policy administration, CI, publishing, work,
  and landing share the complete hardened repository/Git read boundary. Add a
  closed JSON command envelope and cross-implementation fixture 19.
- Harden Settings and Target Index writes with descriptor-relative no-follow
  paths, exact preimages, cooperative repository locking, atomic replacement,
  readback, durability barriers, rollback, mode repair, and hostile race/error
  regressions. These files remain non-authoritative.
- Keep native Windows read, check, reproduce, key-free preview, and protected
  signing support. The two standalone exact-preimage maintenance writes fail
  closed on native Windows until a handle-relative exchange/rollback primitive
  is proven; WSL2 with a Linux-filesystem checkout is the supported path.

## v0.913.0 — 2026-07-22 — Keep producer work and review legible

- Restore the retained assertion text in compact `finding.add` review rows so
  a reviewer can choose one proposal before opening its exact Decision Brief.
- Make `vela next` return configured producer work only. Human review remains
  in `vela review`, and advice-only structural opportunities remain in
  `vela frontier rank`; neither can masquerade as a packet-bound work offer.
- Fail closed if the derived producer offer list contains duplicate target IDs
  or disagrees with its available-work count. Change no event, Receipt,
  proposal, policy, signature, reducer, standing, or accepted-state schema.
- Remove the unshipped standalone Hub projection-indexer experiment. Canonical
  Git frontiers remain the source of truth, and hosted read models stay outside
  the Vela authority and product boundary.

## v0.912.0 — 2026-07-20 — Retain proposal-scoped verification

- Add `vela verify attach` for signed, retry-safe verifier evidence bound to
  one pending proposal, its exact claim root, execution evidence, probe
  evidence, implementation, and declared lineage.
- Add `vela reproduce --proposal` with explicit `pending_proposal` scope and no
  authority effect, plus deterministic `review show` next actions.
- Preserve all legacy attachment IDs by omitting the new optional evidence
  fields from historical records. Verification evidence remains orthogonal to
  human acceptance and cannot change accepted state by itself.

## v0.911.1 — 2026-07-20 — Make work claims retry-safe

- Return the exact active private session when the same actor repeats
  `vela work` for the same still-leased target. The retry appends no lease
  event, changes no root, and reports `idempotent: true`.
- Fail closed when a matching active lease exists but its private session is
  unavailable, instead of silently refreshing the lease and masking an
  in-flight or interrupted claim.
- Change no event, Receipt, proposal, policy, signature, reducer, or accepted
  state schema. Existing frontier history replays unchanged.

## v0.911.0 — 2026-07-20 — Explain work availability

- Preserve the compact `vela.offer.v1` target array while adding configured,
  available, and leased producer-work counts plus bounded lease summaries.
  A live lease no longer makes `vela next` claim that a frontier needs seeds.
- Add `available_work` and `leased_work` to `vela.status.v1` while preserving
  `open_work` as the configured-open compatibility count.
- Mark `vela frontier rank` as advice-only structural opportunity ranking and
  direct producers to `vela next`; graph leverage is not a work queue,
  verifier verdict, or authority decision.
- Change no event, Receipt, proposal, policy, signature, reducer, or accepted
  state schema. Existing canonical frontier history replays unchanged.

## v0.910.0 — 2026-07-19 — Cross-platform public beta

- Make the registered Ed25519 actor the only Vela identity shown in ordinary
  product language. Platform credential stores are hidden custody adapters;
  `vela id protect` uses safe one-command defaults, `vela id show` reports the
  bounded approval session, and `vela id lock` closes only that local session.
- Keep routine agent work prompt-free under exact signed policy. Ordinary help
  now teaches protected `review decide` and `policy decide` flows while hiding
  legacy batch signing, raw key import, key generation, and binary pinning.
- Publish a provenance-first six-crate package graph and prove exact
  `cargo install --locked vela-cli --version 0.910.0` on macOS, Linux, and
  Windows. GitHub bundles carry checksums, SPDX SBOMs, build attestations, and
  explicit portable/native trust metadata rather than pretending an Apple or
  Windows platform signature exists.
- Separate optional native distribution credentials from scientific identity.
  Developer ID/notarization and managed Windows Artifact Signing may add
  no-warning native tiers later; they do not block the universal source and
  package-manager beta.

- Add a Proposed AcceptancePolicy v0.3 candidate for an exact Permit lane
  scoped to one full producer credential root already retained in Receipt v1.
  V0.1/v0.2 registry-backed replay is unchanged; v0.3 requires both the four
  execution roots and an explicit producer root even for globally registered
  actors. Missing, malformed, duplicate, or substituted roots fail closed.
- Add `policy draft search-witness --from-proposal <vpr_id>` so one retained
  pending Receipt supplies all five full roots. Protected previews rederive
  the complete identity binding and display its actor, key fingerprint, and
  root. Draft output now makes the required policy-only Git commit explicit
  before the clean-checkout Decision Plan request.

- Give the unreleased public-beta train a distinct SemVer identity so candidate
  binaries cannot be mistaken for the published `v0.901.0` artifacts.
- Preserve all historical protocol bytes and released tags. This identifier is
  provenance metadata only; it does not accept ADR 0012 or satisfy platform
  signing, notarization, attestation, or fresh-install release gates.
- Make the release workflow reject any Git tag that differs from the one exact
  version shared by all Vela workspace packages, before building artifacts.
- Treat `.vela/proof-state.json` as derived export bookkeeping in new frontier
  transactions, and let completed historical journals survive a legitimate
  proof re-export. Active recovery and transaction completion still require
  exact postimages, while authority, proposals, Receipts, artifacts, and other
  canonical evidence remain fail-closed.
- Make `vela id show` use the same protected-readiness rule as ceremony and
  doctor checks: both the signer helper and the running Vela binary must match
  their authenticated pins. A rebuilt binary is now reported as stale with
  the exact rebind action instead of incorrectly appearing ready.
- Make `vela migrate --to 0.900` add the current `/.vela/work/` ignore rule as
  well as the operation-journal rule. Private work-session coordination no
  longer appears as publishable source dirt on migrated frontiers.

## v0.901.0 — 2026-07-17 — Protected decisions and producer withdrawal

- Replace ordinary batch signing with `vela review decide`: a key-free exact
  preview followed by one root- and time-bound decision card. The protected
  path accepts no key path, batch state, wildcard, `--yes`, or blanket approval.
- Add identity v2 and a one-shot `vela-signer` for macOS, Windows, and Linux.
  It uses the platform credential store, a signed 15-minute-idle/one-hour-total
  authentication session by default, and an optional per-signature mode. It
  verifies key possession and binary identity, removes the plaintext source
  after protected readback, and zeroizes transient seed buffers. Identity v1
  remains readable.
- Add an enforced `VELA_NO_USER_INTERACTION=1` latch for conformance and other
  automated contexts. Any accidental path to platform authentication or a
  decision card now fails before opening desktop UI or touching protected
  custody.
- Prepare checksum-verified paired bundles for macOS, Windows, and Linux so the
  CLI and exact helper cannot drift. The Linux bundle includes a non-caching
  polkit action; no release installs a daemon, listener, or reusable signing
  socket.
- Add the signed, Receipt-bound, accepted-state-neutral
  `proposal.withdrawn` event. Only the exact producer key embedded in Receipt
  v1 can withdraw its own pending proposal; all proposal, Receipt, artifact,
  record, and event evidence remains immutable.
- Move historical batch and detached-file `vela sign` compatibility to
  advanced help and add focused transaction, tamper, duplicate, conflict,
  custody, CLI-surface, and cross-implementation reducer vectors.
- Order the compact review queue newest-first, expose each proposal's exact
  `created_at`, and make the retired `proposals` hint name the replacement
  list/show commands. This closes the first-party cold-review discovery defect
  without changing canonical proposal bytes.
- Accept ADR 0011 after the exact release pair passes the protected Erdős
  terminal test, current-platform rebind and semantic-card review, unchanged
  root audit, and clean-clone replay.

## v0.900.2 — 2026-07-17 — Truly read-only product projections

- Make compact status and exact Decision Brief reads verify operation recovery
  before and after projection without creating frontier lock files. A
  read-only checkout now remains byte- and mode-clean while incomplete,
  corrupt, or overlapping transactions still fail closed.
- Update the shipped frontier skill to teach
  `vela finding show <dir> <vf_id> --view standing` instead of the retired
  `vela state` surface.
- Add a focused filesystem-permission regression proving the compact read path
  needs no checkout write access and creates no operational state.
- This compatible patch changes no event, reducer, Receipt, proposal,
  signature, policy, accepted-state, migration, or key-custody contract.

## v0.900.1 — 2026-07-17 — Clean legacy migration locks

- Extend the 0.900 repository migration to add the missing
  `/.vela/operation-journals/` safety rule when a legacy `.gitignore` lacks it.
  Ordinary `status`, `next`, and review reads may create an empty frontier-lock
  file; it is operational coordination, not canonical state or publishable
  evidence.
- Keep the migration preview's own recovery barrier in Git-private storage so
  checking a legacy frontier cannot dirty the checkout before the ignore rule
  is applied.
- Reproduce the defect on the migrated Sidon and formal-conjectures frontiers,
  preserve all canonical roots and scientific debt classifications, and add a
  focused regression for clean preview and exact touched-file reporting.
- This compatible patch changes no event, reducer, Receipt, proposal,
  signature, policy, accepted-state, or key-custody contract.

## v0.900.0 — 2026-07-16 — Everyday product contract

- Cut the default CLI to the twelve-command daily path and moved setup nouns
  and advanced verification into separate help sections.
- Added compact, root-bound status, producer offer, work-session, and review
  projections with explicit output budgets.
- Added minimal bounded initialization and a compact doctor response with one
  next action. Optional MCP, CI, proof, and editor scaffolding no longer ships
  in a fresh frontier.
- Added an isolated `migrate --to 0.900` preview and apply path. Migration
  removes retired manifest fields, regenerates derived views, preserves
  canonical roots and bytes, and reports stale proof pointers and artifact
  debt.
- Retired the core Atlas wrapper, Foundry and external-reproduction paths,
  legacy proposal/state/credit/publication aliases, and the old Hub wrapper.
  `vela-hub` remains a separate binary.
- Added ADR 0010 and focused regressions for help, compact contracts, minimal
  initialization, migration refusal, and old-frontier replay. This release
  adds no protocol primitive or authority surface.

## v0.800.23 — 2026-07-16 — Durable journal boundaries

- Fixed the completed-transaction recovery barrier so a later legitimate
  materialization may replace a historical derived-view postimage without
  making the frontier unavailable.
- Retained exact completed-journal verification for markers, staged blobs,
  event membership and roots, authority, public-review, canonical-evidence,
  and private-coordination postimages. Active installation and completion
  continue to verify every write class exactly.
- Added regressions proving derived re-materialization preserves event bytes
  and roots while authority drift, missing durable postimages, corrupt journal
  material, and incomplete recovery still fail closed.
- Reproduced the defect and repair against the completed Erdős governance
  journal. This patch changes no event, reducer, signature, policy, Receipt,
  scientific authority, or human key-custody contract.

## v0.800.22 — 2026-07-16 — Immutable event transactions

- Fixed repository rendering so an existing event file is retained
  byte-for-byte whenever its decoded event is unchanged. This preserves legacy
  JSON encodings across policy, decision, maintenance, and ordinary derived
  view writes.
- Extended the append-only transaction guard from actor activation to `work`
  and `land`: those commands now fail closed if their candidate removes or
  semantically changes any preexisting event. Deliberate valid signature
  acquisition remains representable through its explicit migration path.
- Added protocol and real CLI regressions proving `work` and deferred `land`
  preserve all preexisting event bytes. The defect was found by the first valid
  direct-Codex cold-use producer cell, which stopped safely after observing
  that `v0.800.21` had reserialized three fixture events.
- This patch changes no event identity, signature algorithm, reducer result,
  actor-registration semantics, policy route, accepted-state authority, or
  human key-custody boundary.

## v0.800.21 — 2026-07-16 — Byte-preserving actor activation

- Fixed the temporal actor-registration ceremony so it preserves every
  preexisting event file byte-for-byte. The recoverable transaction now rejects
  any removal or semantic mutation of an existing event and installs only the
  new signed activation event plus its derived projections.
- Added a CLI regression with deliberately noncanonical legacy bytes, including
  an explicit `signature: null` and trailing newline, and proved those bytes
  survive the real terminal ceremony unchanged.
- The verifier semantics, activation payload, event identity, authority rule,
  and scientific state are unchanged from `v0.800.20`. This patch closes the
  immutable-history migration contract before cold-use registration.

## v0.800.20 — 2026-07-16 — Temporal actor registration

- Added the signed, audit-only `actor.registration_activated` event and closed
  `vela.actor-registration-boundary.v1` payload. A temporal boundary binds one
  actor key to an exact ancestor Git commit and tree, event-log root and count,
  and actor-registry byte root.
- Strict verification now classifies exact unsigned anchor members as legacy
  and unauthenticated without attributing them to the key holder. Matching
  events absent from the anchor still require valid signatures regardless of
  timestamp. Existing actor records without a valid activation preserve the
  timeless rule.
- Added fail-closed checks for missing and shallow history, non-ancestor forks,
  wrong roots, registry tampering, duplicate boundaries, event deletion or
  mutation, signature stripping, backdating, and activation-plus-registry
  deletion from descendant history.
- Added `vela actor activate --preview` and the human-only activation ceremony.
  Scripted use requires `--yes --confirm-root <sha256:...>` from an exact
  key-free preview. The ceremony refuses agent identities, reads the matching
  key only after confirmation and a recovery barrier, installs one signed event
  through the recoverable frontier transaction, and uses exact Git publication.
- Reproduced the Erdős migration preview at its exact registration root: 81
  anchored unsigned events, 131 anchored signed events, zero later unsigned
  events, and one later signed event. No key was read and no frontier byte was
  changed.
- Rust, Python, and TypeScript reducers agree that the activation event is
  state-neutral. This release changes no scientific acceptance rule, signature
  algorithm, object family, registry service, or model authority.

## v0.800.19 — 2026-07-16 — Reload-stable policy retirement

- Kept legacy-policy retirement replay-stable by recomputing project
  statistics without re-materializing removed source associations during the
  governance-only event. A shared-source regression now proves that a signed
  retirement remains byte-stable after reload.
- Corrected the reusable GitHub Action and installation examples to pin the
  current release. `v0.800.18` published valid binaries, but its conformance
  run exposed the stale `v0.800.17` action example after publication.
- This patch changes no scientific state, authority rule, signer boundary,
  event schema, Receipt contract, or verifier semantics.

## v0.800.17 — 2026-07-16 — Bounded composition profiles

- Completed ADR 0004's removable Phase 1B reference profiles around one exact
  canonical fact manifest: a Vela resolver, correction-aware CI projection,
  accepted-state context pack, independent Reader C, and a competing
  Git/DSSE/in-toto/`science.lock` wrapper.
- Added 54 hostile fact-manifest vectors, seven three-consumer CLI cases,
  independent reader status/root parity, offline same/descendant/stale/fork
  bundle inspection, projection-deletion checks, and 13 hostile
  standards-wrapper vectors.
- Red-team review removed an ambiguous precedence rule. One delivered manifest
  may now carry one later truth-relevant change event; combined finding,
  decision, or verifier changes fail closed and must be delivered as separate
  exact manifests.
- No dependency object, event kind, command family, authority rule, automatic
  child-truth propagation, graph, cache, or hosted service is promoted. These
  are internal research profiles and engineering evidence, not independent
  interoperability, human ceremony, ecosystem adoption, or proof that Vela is
  a scientific foundation.

## v0.800.16 — 2026-07-16 — Native atlas targets

- Added the optional `vela.target-index.v1` projection so a large scientific
  atlas can expose thousands of stable, ranked targets through the ordinary
  `next -> work -> land` loop without creating a second authority store.
- `vela next` reads only the bounded derived index, filters non-open and live
  leased targets, and carries a hash-pinned packet reference plus the live
  producer-only authority ceiling. `vela work` opens and verifies only the
  selected packet, then retains the packet root, index root, current frontier
  root, and Git commit in the private session.
- Hardened the new reader against unsafe or duplicate IDs, traversal,
  symlinks, oversize indexes and packets, frontier mismatches, digest drift,
  and schema drift. Terminal entries remain explicitly addressable but are not
  suggested as open work.
- Validated the interface against the complete 1,217-problem Erdős atlas:
  652 open targets, seven paused targets, 558 completed-status targets, and 38
  frozen witnesses reproduced. This establishes native local integration, not
  acceptance of any scientific claim or completion of the pending human policy
  retirement ceremony.

## v0.800.15 — 2026-07-16 — Replay-equivalent work snapshots

- Fixed the task-first `work` transaction so its in-memory candidate
  recomputes derived project statistics before rendering `frontier.json` and
  `vela.lock`. The signed `attempt.claimed` event, visible state, lock, proof,
  Git commit, and a fresh replay now agree immediately after the claim.
- Added a public regression that runs `work` and then
  `vela check . --strict --json` without an intervening materialization.
- This patch changes no event, Receipt, authority, signer, policy, or human
  ceremony contract.

## v0.800.14 — 2026-07-15 — Task-first closure and inspectable decisions

- Completed the ordinary producer chain through one supported Receipt v1
  surface. Scientific-chain assertions now have bounded typed authoring,
  flag/file canonical-byte parity, exact retry identity, and the same
  structured landing result across CLI and MCP, including a nullable typed
  `original_route` for exact retries.
- Made `vela work` and signed zero-TTL lease release publish their exact Git
  deltas through the existing recoverable publication path. A clean-clone
  training frontier now proves `next -> work -> reproduce -> land`, Deferred
  routing, zero accepted-state delta, and replay from retained artifacts.
- Completed the Decision Brief facet inventory and finite DecisionPlan batch
  matrix. Mixed decisions remain coherent, high-risk items remain isolated,
  and every supported read transport exposes the same bounded review facts
  without adding model judgment or authority.
- Retained each new decision's canonical seven-field DecisionPlan preimage at
  its root-keyed content-addressed evidence path in the same recoverable
  transaction as the decision. A pure named-decision inspector and registered
  adversarial vectors prove exact binding, including every matched semantic
  event's historical reviewer signature, and prove the evidence remains
  non-authoritative: deleting it changes neither replay nor signed events.
- This release adds no scientific event, Receipt family, signer, policy rule,
  dependency object, or automatic truth propagation. Its fixture and local
  engineering evidence do not establish a human ceremony, independent
  adoption, outside interoperability, or a new scientific foundation.

## v0.800.13 — 2026-07-15 — Evidence closure and composition probe

- Froze the reviewed pre-ADR 0003 canonical `.vela` Git tree, every path and
  byte in that tree, and its strict replay result. The test attributes the tree
  to the recorded historical commit and prevents a fixture or manifest
  rebaseline from retaining stale provenance.
- Completed the private frontier-transaction journal failpoint coverage for
  abort and committed-conflict transitions, including operation-ID collision
  refusal, third-party-drift preservation, and exact recovery. Added a
  post-key-read/pre-marker boundary proving one fixture-key read still leaves
  zero decision or journal delta on failure.
- Made the complete Decision Brief byte-equivalent across six read transports
  after normalizing only their independent observation time, and added five
  adversarial fixtures with concrete reviewer questions for statement
  fidelity, vacuity, partial verification, contradiction blast radius, and
  contributor credit.
- Started ADR 0004 only as a removable internal experiment frozen against
  released `v0.800.12`. Exact-checkout vectors reject structural substitutions,
  while the resolver has no success state and stops at
  `unresolvable:authority_snapshot_porcelain_missing`: a derived aggregate
  cannot prove canonical replay, historical signer scope, or which attachment
  set the reviewer consumed. No public wire object, authority rule, human
  ceremony, outside-use claim, or benchmark result is added.

## v0.800.12 — 2026-07-15 — Bounded trust-edge hardening

- Made explicit Receipt v1 files and local public artifacts descriptor-bound,
  symlink-safe, and bounded before the write edge. Receipts and individual
  artifacts are capped at 8 MiB; one landing may retain at most 64 MiB of local
  public artifact bytes. Traversal, symlink, oversize, and identity drift fail
  before scientific or Git state changes. Archive-like artifacts remain opaque
  content-addressed bytes and are never expanded by review.
- Removed duplicate raw claim rendering from proposal preview and state diff.
  Hostile claims and caveats are now regression-tested across `diff`, proposal
  preview, state diff, sign preview, and status with visible escaping, bounded
  output, no command execution, and no scientific delta.
- Consolidated exact Git publication interruption controls into one private
  durability-step harness. Focused regressions cover a nonempty caller index on
  exact retry, caller-index drift after preflight, every pre-ref journal state,
  an actual compare-and-swap loss, post-ref recovery, and successful push
  followed by completion-record failure without minting or moving another
  commit.
- Replaced the Hub's unbounded review JSON response with
  `vela.hub.review.v0.2`: snapshot-bound offset pages default to 25 and cap at
  100, each ledger reports its own exact total and continuation, and policy
  counts are explicitly page-scoped. Materialized rows are loaded with the
  snapshot hash stored in the same database read snapshot, so a concurrent
  promotion can never cache or label new rows under an old continuation hash.
  The scale proof composes the existing 10,000-row typed pending catalog with
  bounded Receipt paging, task-first landing/idempotency regressions, and a
  10,000-row Hub page collector; it does not perform 10,000 redundant Git
  writes.
- Changed no Receipt, event, proposal, signature, policy-authority, or accepted
  scientific-state contract. ADR 0004 remains accepted but inactive behind the
  open ADR 0003 human and independent-adoption gates.

## v0.800.11 — 2026-07-15 — Exact fixture-signature state

- Removed a superseded detached signature from the active fixture-manifest
  slot after `v0.800.10` changed one fixture and regenerated the manifest.
  The current manifest is now honestly unsigned; no key was read and no new
  authenticity claim was created. The prior human signature remains in Git
  history.
- Added a public Rust regression that rejects any present detached signature
  unless it verifies over the exact current manifest bytes. Fixture-change
  guidance now requires retiring the prior active signature and leaves any
  later re-signing to the canonical `vela sign <manifest>` human ceremony.
- Closed the same ceremony's clear-signing gap: detached `vela sign <path>`
  now checks the operator's binary pin before resolving a key or writing a
  signature, with a regression proving stale pins fail closed.

## v0.800.10 — 2026-07-15 — Typed readiness and bounded review

- Replaced the ambiguous open/closed policy lane with one byte-level policy
  state (`absent`, `staged_unsigned`, `active`, or `broken`) and a separate
  Permit readiness (`ready`, `human_only`, or `blocked`). Missing or lifecycle-
  limited authority now routes a Permit to a human; malformed governance blocks;
  an intentional evaluator Deny remains a Deny. Status, check, doctor, policy
  show/test/log/suggest, Decision Briefs, MCP, and landing use the same
  assessment without reading a human key.
- Made exact publication no-ops explicit as `unchanged`, allowed a requested
  push to verify an unchanged tip, and made recovery recognize when an upstream
  ref already contains an earlier publication candidate. Recovery remains
  idempotent and preserves caller index, worktree, local tip, and remote
  descendants.
- Added bounded review-pressure telemetry over durable pending-proposal facts.
  A 10,000-row queue is measured without opening Receipts; queues beyond 16,384
  keep deterministic pagination while pressure becomes typed unavailable.
  Unsupported quality, independence, priority, verifier, effort, correction,
  and downstream-use metrics remain typed missing and never gain authority.
- Bounded the default Receipt parser against excessive JSON depth and 100,000-
  artifact inputs, corrected the canonical stored dependency relation to
  `depends`, and narrowed public reproducibility and adoption claims.
- Recorded accepted ADR 0004 and its experiment plan for verifiable scientific
  composition, queued behind completion of ADR 0003. It starts with four
  Codex-only diagnostic runs; no API spend, new wire primitive, human ceremony,
  or active-goal change begins during this release.

## v0.800.9 — 2026-07-15 — Deterministic hosted regression

- Removed a probabilistic ordering assumption from the legacy-retirement
  regression exposed by hosted conformance for `v0.800.8`. The test now proves
  canonical insertion against an existing proposal on either side of the new
  proposal ID, without searching for a hash with a particular ordering.
- Runtime behavior, transaction postimages, schemas, accepted state, and
  authority boundaries are unchanged from `v0.800.8`.

## v0.800.8 — 2026-07-15 — Release lint closure

- Removed the needless test-only borrow caught by the hosted all-targets Clippy
  gate for `v0.800.7`. Runtime behavior, transaction bytes, schemas, and
  authority boundaries are unchanged.
- Retains the `v0.800.7` canonical proposal-ordering fix and its exact
  prepare-materialize-recovery regression as the release candidate used for
  frontier migration.

## v0.800.7 — 2026-07-15 — Canonical proposal transaction postimages

- Kept every in-memory pending-proposal insertion in the same proposal-ID
  order used by split-repository loading. A prepared transaction's visible
  postimages therefore remain byte-identical after the official materializer
  reloads the same proposal files.
- Added the exact regression found while migrating the Erdős frontier: prepare
  bounded legacy-policy retirement, materialize, compare `frontier.json`,
  `vela.lock`, and `proof/latest.json` byte for byte, then reacquire the
  completed-journal recovery barrier successfully.
- Added a direct protocol ordering regression. Accepted events, Receipt v1,
  scientific schemas, and authority rules are unchanged; this patch reads no
  key and performs no decision.

## v0.800.6 — 2026-07-15 — Bounded legacy-policy retirement

- Added one prepare-only recovery command for unsupported prelaunch policy
  bytes. `vela policy retire-legacy` records a closed, content-addressed
  governance proposal without reading a key, validating the legacy signature,
  or granting policy authority. The existing isolated `vela sign` Decision
  Plan remains the only acceptance path.
- Bound acceptance to the exact raw active pair, fixed internally derived
  paths, the optional byte-identical same-id snapshot pair, an intact replay,
  no current policy head, and no evidence that the legacy policy admitted
  state. Drift aborts before key access; rejection preserves every byte; an
  accepted review and the bounded deletions commit in one recoverable
  transaction.
- Narrowed strict-signal heuristics so typed, non-biomedical Erdős catalogue
  records do not inherit empirical missing-condition failures and a mathematical
  “translation property” is not treated as a missing biological translation
  condition. Empirical biomedical records retain the strict checks.
- Added the `vela.policy-legacy-retirement.v1` governance proposal shape and
  its review/audit regressions. Receipt v1, accepted event, policy-lane, and
  scientific finding schemas are unchanged.

## v0.800.5 — 2026-07-15 — One executable frontier scaffold

- Removed the unlaunched multi-template `vela init --template` branch and its
  orphaned adoption scaffold. New frontiers now have one task-first path and
  one generated command list: `agents sync`, `doctor`, `status`, `next`, and
  strict `check`.
- Replaced retired generated commands (`inbox`, `integrity`, `stats`,
  `source-inbox`, `task`, `claim diff`, and `gate .`) with commands the current
  binary actually exposes. The generated charter now teaches
  `next -> work -> land -> sign`, and a regression keeps first-run guidance on
  the release surface.
- Made a fresh frontier's MCP file byte-identical to `vela agents sync` and
  explicitly limited it to the nonfinalizing draft profile. Agent tooling can
  land a Receipt, but cannot sign or finalize a proposal.
- Made an empty frontier's first `next` useful: it offers one generic
  `seed:first` producer session, without inventing scientific content or
  restoring a template system. Init/doctor command hints now shell-quote the
  frontier path and the generated MCP adapter carries no dead environment.
- Treat a completely absent optional review-policy document set as an explicit
  conservative-default warning. Partially configured or malformed policy
  documents remain release-blocking, including explicitly declared files that
  are missing; declared paths never silently fall back. A fresh frontier no
  longer contradicts its own doctor guidance.
- Made `vela doctor` a local, offline diagnostic. It no longer probes a hosted
  hub, requires a Rust toolchain outside the substrate checkout, or treats an
  occupied optional Workbench port as failure.
- Added a tag-driven two-platform release workflow. Linux x86-64 and Apple
  Silicon binaries plus installer-compatible SHA-256 companions are now built
  from the exact tag and attached to its GitHub Release. Release jobs use fixed
  runner images, exact action commits, least-privilege job permissions, and
  repository-level immutable releases.

## v0.800.4 — 2026-07-15 — Trust-boundary parity hardening

- Consolidated policy-context derivation in the protocol. Landing, replay,
  review, policy testing, and policy suggestion now use one strict builder and
  one caller-supplied observation instant. Missing or incoherent retained
  material fails closed instead of being reconstructed optimistically by the
  CLI; legacy audit paths cannot manufacture credential or assurance facts.
- Added direct regressions for detached-HEAD refusal, publication to an
  un-checked-out branch without touching the caller index, linked-worktree
  rejection, and exact post-ref index-lock recovery. These tests exercise the
  existing Git transaction rather than adding a transport or authority layer.
- Proved that flag authoring and file import retain byte-identical canonical
  Receipt v1 bytes and roots for the same facts. Landing-time activity,
  proposal, and commit identities remain separate provenance; exact retries
  on one frontier remain fully idempotent.
- Receipt-backed finding proposals now retain one typed evidence span per
  explicitly bound artifact, pointing into the canonical Receipt. The normal
  task-first result is therefore review-ready without inventing verifier,
  independence, or acceptance claims.
- No Receipt, event, policy-lane, or Decision Brief schema changed. Existing
  accepted-event bytes are not rewritten, and no human decision or key
  ceremony is part of this release.

## v0.800.3 — 2026-07-15 — Nested-workspace test portability

- Made the frontier-repository integration tests honor the explicit
  `VELA_BIN` contract, matching the release-contract tests. Parent workspaces
  can now reuse the Vela binary they already built instead of requiring a
  duplicate binary under the submodule's private `target/` directory.
- No runtime behavior, protocol schema, Receipt, verifier, accepted event, or
  materialized-frontier bytes changed.

## v0.800.2 — 2026-07-15 — External Lean boundary consolidation

- Removed the unlaunched replay-packet compatibility mode, its packet lineage
  and sealed-environment contract machinery, and the last producer-specific
  Lakefile helper from the installed external Lean verifier.
- Kept one generic external boundary: a full GitHub repository URL, commit,
  and Lean declaration are reconstructed in a Vela-controlled project and
  produce a typed draft result without gaining acceptance authority.
- Added a prelaunch regression guard so packet flags and producer-specific
  Diderot or Krafft assumptions cannot return to the installed verifier.
- Preserved historical Diderot corpus bytes as explicitly inert provenance.
  Diderot remains an early exploratory project, not a Vela partner,
  dependency, verifier, compatibility target, or release gate.
- No protocol schema, Receipt, accepted event, or materialized-frontier bytes
  changed.

## v0.800.1 — 2026-07-15 — Portable prelaunch maintenance

- Made the one-writer-path regression guard depend only on standard Unix
  tools and Git, so a clean GitHub runner checks the same surface as a local
  checkout without requiring ripgrep.
- Made agent-adapter generation use the tracked frontier manifest instead of
  ignored local state, and refreshed the generated task-first skills.
- Made cross-workspace CLI contract tests honor the suite's explicit
  `VELA_BIN`, so a clean parent checkout reuses the binary it already built.
- Moved the broad historical Lean model build to an explicit manual workflow,
  documented its custom assumptions honestly, and removed optional external
  Lean packaging assertions from routine core CI.
- Made the active packet test fixture derive its compiler version from the
  package instead of pinning the prior release.
- No protocol, schema, verifier, Receipt, or materialized-frontier bytes
  changed. Frontiers recorded with Vela `0.800.0` remain exact historical
  artifacts.

## v0.800.0 — 2026-07-14 — Task-first protocol hard cut

Vela's prelaunch protocol candidate is organized around one contribution path:
a producer emits Receipt v1, `vela land` records it, the signed policy routes
it, and a human key holder alone can make an uncovered truth-bearing decision
through `vela sign`.

- Removed unlaunched compatibility aliases and alternate writer paths,
  including direct proposal accept/reject, submit, attempt import, auto-admit,
  legacy finding apply, redundant clients, stale schemas, and obsolete
  examples.
- Removed the unlaunched acceptance-policy compatibility subsystem. Vela now
  accepts only current content-addressed policy IDs, signatures bound to
  `signed_at`, and `vela.policy-lane.v2` replay records.
- Consolidated the portable Python emitter, installed external-verifier core,
  canonical JSON reader, and conformance commands into one crate-owned resource
  bundle. Removed the duplicate checkout-only package and made the whole-body
  `vela:receipt_body` binding mandatory in the single Receipt v1 validator.
- Reduced publication to the exact reviewed Git delta.
- Rebuilt the Hub as a disposable read-only Git index over a versioned source
  catalog. It no longer registers or deprecates sources, signs records, stores
  witness objects, or writes canonical scientific state.
- Removed Carina from live code, schemas, manifests, locks, and documentation.
  Existing immutable event payloads remain readable as opaque historical data.
- Generalized the optional external Lean verifier and removed Diderot-specific
  compatibility and release checks. Diderot is an early exploratory project,
  not a Vela partner, protocol target, or release dependency.
- Added a prelaunch-surface regression gate so retired paths cannot quietly
  return before the protocol is published.
- Removed the duplicate automatic Receipt-draft workflow. Receipt conformance
  stays in the focused core gate; optional external verifier execution remains
  explicit.
