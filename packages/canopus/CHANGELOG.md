# Changelog

## Unreleased

- Advance Canopus `0.8.0-rc.8` after real campaign replay exposed a retained
  predecessor-epoch compatibility defect. Current readers now validate and
  replay immutable Run and Mission bytes through bounded in-memory projections;
  the retained records and their exact roots remain unchanged.
- Advance Canopus `0.8.0-rc.7` for the Vela `0.940.7` current Target Index
  writer repair.
- Keep retained Run and Submission source identities unchanged while requiring
  registration through the exact repaired Vela release.

- Advance Canopus `0.8.0-rc.6` for the Vela `0.940.6` retained-Artifact
  repair. Current registration may reuse exact retained Artifact bytes and
  canonical paths, while path substitution remains fail-closed.
- Refuse a duplicate verifier-passing mission before output creation or model
  execution. Historical schemas remain replayable but do not weaken the
  current Mission v1 coverage gate.
- Export immutable predecessor-root Run v2 records without rewriting their
  bytes, and allow their authenticated Submission bundles to register only
  against an exact source commit or its current descendant.
- Add a rooted, source-only framework-neutral evaluation contract. Optional
  engine experiments and metadata-only trace export remain outside the npm
  payload and cannot change Vela Standing or authority.
- Advance Canopus `0.8.0-rc.5` after real Erdős dogfood exposed stale
  post-verifier wording at the Submission waist. New workers keep verifier
  status out of the Claim, verifier-pending caveats are normalized after the
  separate check, and export fails closed on stale verifier language or control
  bytes. An explicit Claim-plus-scope-limit correction can author a new signed
  producer Submission while preserving the immutable Run unchanged.
- Consume Vela's current `produce` lane directly when selecting the first
  canonical offer. Remove the retired `attack`-lane and `task.packet_ref`
  compatibility path that blocked real `0.940.5` Frontiers before a model call.
- Advance Canopus `0.8.0-rc.4` to the attested Vela `0.940.5` release. The
  macOS and Linux archives and binaries are pinned by exact SHA-256;
  the product contract is unchanged.
- Begin Canopus `0.8.0-rc.1` as an exact candidate composition with attested
  Vela `0.930.0-rc.12`. The older `0.7.0`/`0.915.1` composition remains
  available only from immutable release history; it is no longer an active
  operating path.
- Advance the sole active Erdős 1056 k=15 profile to the next non-overlapping
  range `10429201..10429400` before any model call. The closed profile root is
  `sha256:2034d6b6a49ba1345518340ba74dfd75931035788924c53e629fb0260304ece1`;
  its static Linux ARM64 and x86-64 verifier roots are
  `sha256:6abe6125b5ed7cfeb256a1d86f3a66c6e7000a5542417d9dd04b2e5f9d3ffe81`
  and
  `sha256:ce73ca27d54a2ed31607a6d279d85ed36f28c2a830891b5d5d27b9cf50f0fcb4`.
  The completed `10429001..10429200` run and pending Proposal remain
  reproducible from immutable Git and Frontier history but are removed from
  default discovery and the package payload.
- Preserve the first candidate artifact for `10429001..10429200` as a
  verifier-rejected bounded failure, then retain the exact successful repair
  separately from the new active range. No prior numeric result is supplied
  to the new worker.
- Allow a verifier-passing review-only Run to export Submission v1 without an
  optional `vela.execution-binding.v1`. The Submission still binds the exact
  verifier capsule in its verification requirement and can only enter the
  ordinary pending-review path; policy eligibility continues to require the
  full execution binding and result contract.
- Pin Vela rc.12 after the repository-authority landing regression proved that
  repository authorization must not occupy Receipt v1's scientific
  `acceptance.policyRef`, signed agent observations require canonical
  whole-second timestamps, and repository-authority landing must materialize
  derived views inside the same recoverable transaction without adding those
  views to the signed scientific delta. The released macOS binary is
  `sha256:2e6c4f5aadf0d3e6e7102d71f8fa0a46d31567cc847c25914706183248167471`.
- Raise only the bounded Vela subprocess ceiling from two to ten minutes after
  a real Erdős landing reached its frozen verifier and atomic authority
  transaction but was killed during large-frontier materialization at exactly
  120 seconds. Model, verifier, workspace, and mission budgets remain
  independently bounded.

## 0.7.0 - 2026-07-25

- Retire long-lived proposal-withdrawal capabilities. Successful runs now
  destroy the isolated producer key with the rest of the Vela home; `inspect`
  reports only rooted run state, and the `withdraw` command and capability
  store are removed. Historical activity records and Vela
  `proposal.withdrawn` events remain readable, while pending proposals remain
  canonical review records rather than reasons to retain a private key.
- Narrow the npm package to the active Canopus product boundary. Historical
  Build Week records, release ledgers, advisories, and retained Erdős evidence
  remain immutable in Git and tagged source archives but no longer inflate the
  installed runtime. Stable README links point to the exact `v0.6.5` archive,
  while missions, profiles, verifier capsules, schemas, runtime custody
  fixtures, and current product guidance continue to ship.

## 0.6.5 - 2026-07-25

- Advance the sole runnable Erdős 1056 profile to the adjacent,
  non-overlapping range `10428801..10429000`. The registration freezes one
  bounded Mission v1 draft and independently built static Linux ARM64 and
  x86-64 verifier capsules before any model call. The completed
  `10428601..10428800` profile remains reproducible from Canopus `0.6.3`, its
  retained run evidence, and immutable Git history; its binaries are removed
  from the active package rather than accumulated indefinitely.
- Complete that registered mission in 73,083 observed tokens. The frozen
  verifier independently confirms the exhaustive bounded-negative result over
  all 15 primes in `10428801..10429000`; Vela lands Receipt
  `sha256:7a7c728516e79da3f25ac1f6c10c30908949b6aba118dc9752f61b90b4a96435`
  as proposal `vpr_4a9068064a0c441c` through Defer with accepted-event delta
  zero, and clean-clone reproduction matches. The result is not a universal
  nonexistence claim and is not scientific acceptance.
- Publish only the sanitized read-only run projection at
  `evidence/erdos/run_b3b0cf07-b149-4d53-8258-76ba0e1fc0a5`; the raw worker
  directory, credentials, unrestricted logs, and withdrawal capability remain
  outside the package.
- Compose exactly with released Vela `0.915.1`. The maintenance release changes
  no mission, Receipt, verifier, policy, proposal, or authority contract.
  GitHub CI pins the released macOS, Linux, and Windows archives and binaries by
  SHA-256, and the released-binary integration test retains Defer,
  accepted-event delta zero, retained-artifact binding, and clean-clone replay.
- Extend the bounded Vela 0.9 compact-root reader through `0.915.1`. Fresh
  minimal frontiers intentionally omit `proof/latest.json`; Canopus continues
  to bind the strict replay event and scientific-state roots instead of
  inventing or requiring a derived proof packet.
- Keep the non-normative cross-repository task-authority shadow test explicit:
  it runs when its separately pinned ecosystem checkout is available and reports
  a named skip in a standalone Canopus clone instead of copying private
  experiment inputs or failing the product release suite.
- Add a source-local, non-normative task-authority hostile experiment over the
  released Canopus `0.6.3`, Vela `0.915.1`, and Codex `0.145.0` composition.
  The current Mission v1 boundary detects three of eight frozen hostiles; an
  additional operational packet detects all eight while retaining zero
  scientific, authority, or standing effect. The experiment is intentionally
  excluded from the npm package and promotes no Vela protocol object.

## 0.6.3 - 2026-07-24

- Restore exact composition with released Vela `0.914.1` after Canopus
  `0.6.2` correctly refused the newer binary at its version boundary. The
  maintenance release preserves an exact legacy signature without
  authenticating it, fixing the false Erdős repository-boundary blocker while
  retaining all historical strict debt.
- Advance the current native-worker boundary to Codex CLI `0.145.0` and pin
  the exact Linux archive and binary digests used by CI. Immutable missions
  continue to require their own recorded Codex versions and hashes.
- Preserve the compact status, offer, Receipt, Defer, replay, and zero-authority
  contracts through the released-binary integration test.
- Report Vela's current `scientific_state_root` directly from `canopus doctor`
  instead of preserving the retired, ambiguous `snapshot_root` label.
- Replace the brittle list of individual Vela 0.9 patch versions with one
  bounded compact-status compatibility rule. Historical missions still require
  their exact recorded binary and digest.
- Advance the sole runnable Erdős 1056 profile to the adjacent, non-overlapping
  range `10428601..10428800`; the completed `10428401..10428600` capsule remains
  reproducible from Canopus `0.6.2` and its immutable run evidence.
- Raise the bounded Vela control-command ceiling from 30 to 120 seconds after
  the real Erdős `vela work` transaction required 38.6 seconds. The original
  timeout left only a recoverable `Prepared` journal in a disposable clone and
  made no canonical-frontier change.
- Pin the active mission to account-compatible `gpt-5.4`. The attempted
  `gpt-5.6` request was rejected by Codex before inference because ChatGPT
  account authentication does not expose that model; it produced no research
  artifact or frontier publication.
- Preserve the first `gpt-5.4` run as a stopped non-authoritative result after
  provider-reported usage exceeded its 100,000-token postcondition: 136,448
  input tokens, including 100,352 cached input tokens, plus 3,442 output
  tokens. Register one distinct retry with no prior candidate or search hint,
  one attempt, and a 160,000-token postcondition that remains below the
  original 187,013-token Erdős run.
- Complete the separately registered retry in 72,454 observed tokens. The
  frozen verifier independently confirms the exhaustive negative result over
  `10428601..10428800`; Vela lands Receipt
  `sha256:7400662ed6493aa6dc49a31c0d2ea1099a5380a272914b13325aaf088ba58b57`
  as proposal `vpr_a845ae60ed695b93` through Defer with accepted-event delta
  zero, and clean-clone reproduction matches.
- Publish only the sanitized, read-only run projection at
  `evidence/erdos/run_192b3bef-9d6e-49e5-b72d-7ae903b29d5e`; the raw worker
  directory, credentials, and unrestricted logs remain outside the package.

## 0.6.2 - 2026-07-21

- Ship the exact pending Sidon artifact reproduction command and independent
  base-3 verifier command in the provenance-backed package instead of the
  whole-frontier canonical-witness check previously published by `0.6.1`.
- Package the dated Build Week ledger, immutable Receipt caveat reconciliation,
  and third-party component notice used by the final judge path.
- Add release-contract coverage for those public instructions and files. No
  runtime, protocol, Receipt, policy, or authority behavior changes.

## 0.6.1 - 2026-07-21

- Correct the public Build Week record and judge quickstart to name the
  released Vela 0.912.0 composition and current Canopus package.
- Preserve the 0.6.0 code and evidence contracts unchanged.

## 0.6.0 - 2026-07-20

- Type completed-run evidence by stage as `worker_observations`,
  `verifier_observations`, and `standing_caveats`, while preserving read
  compatibility for immutable `canopus.run.v0` records.
- Add `canopus publish-run`, which emits a sanitized public projection, root
  manifest, exact pending-proposal commands, and read-only Observatory import
  descriptor without landing, signing, pushing, deploying, or accepting.
- Compose against Vela 0.912.0 and its proposal-scoped verifier attachment and
  reproduction surfaces.

## 0.5.2 - 2026-07-20

- Compose against Vela 0.911.1 so same-actor retries of `vela work` return the
  exact active session without appending a second lease event.
- Preserve the 0.911.0 cold-use run and registration as immutable diagnostic
  evidence; any focused retry remains separate and never rewrites that record.

## 0.5.1 - 2026-07-20

- Preserve exact pending-result replay roots instead of recomputing a receipt
  projection with incomplete post-run state.
- Keep the published `0.5.0` package and failed post-publication workflow as
  audit evidence; `0.5.1` is the corrected default install target.

## 0.5.0 - 2026-07-20

- Compose against released Vela 0.911.0 and surface configured, available, and
  leased producer work without treating a lease as an empty frontier.
- Restore the packaged quantum-codes profile, mission draft, and verifier
  capsule so all four published frontiers have reproducible product coverage.
- Make Bun 1.3.12 the canonical development and public invocation path while
  preserving the supported Node runtime for the installed package.
- Keep historical Build Week registrations and evidence byte-identical; this
  release adds composition and usability, not scientific authority.

## 0.4.6 - 2026-07-20

- Register and complete the GPT-5.6 Sidon mission that produces a new
  7,194-point construction in `{0,1}^24`, passes the frozen verifier, lands a
  Receipt through Defer with accepted-state delta zero, and clean-clone replays
  the same evidence.
- Retain large declared artifacts from their exact allowlisted worker path
  before workspace cleanup. Reject path escapes, symlinks, hardlinks, empty or
  missing files, oversize files, invalid UTF-8, and authentication material.
- Preserve the exact private Vela work-session capability needed by `vela land`
  while the publication guard continues to reject every unrelated Git change.
- Ship both Sidon verifier capsules in the npm package and cover the package
  allowlist with a regression test.
- Make the sanitized public projection target-neutral and distinguish a
  worker-time verifier-pending handoff from the final recorded verifier pass.

## 0.4.5 - 2026-07-20

- Correct the judge quickstart so the retained Erdős 1056 evidence chain is
  identified as GPT-5.4 with an independent verifier, while the GPT-5.6 formal
  attempt and claim-fidelity advisory remain clearly separate Build Week
  additions. Runtime behavior, evidence roots, and Vela interfaces are
  unchanged from 0.4.4.

## 0.4.4 - 2026-07-20

- Advance the product, CI assets, runtime inspection, and released-interface
  compatibility gate to checksum-pinned Vela 0.910.0 without changing any Vela
  protocol, Receipt, policy, or authority interface.
- Replace the active formal registration with
  `formal-erdos-505-test-dim-one-gpt56`, bound to `gpt-5.6-sol`, the frozen
  Lean 4.27.0 capsule, one proof-term artifact, Defer, and zero accepted-state
  delta. Preserve the exact GPT-5.4 v0.4.3 registration bytes and source roots
  under `registrations/historical/`.
- Add the fail-closed `canopus public-run` exporter and
  `canopus.public-run.v1` schema. The public projection contains bounded roots
  and outcomes but no raw run directory, worker log, authentication, private
  path, or unrestricted transcript.
- Make `canopus doctor --profile <name>` honor the explicit registered profile
  used by the Build Week custody preflight.

## 0.4.3 - 2026-07-19

- Add the stable `canopus --version` and `canopus -V` probes required for exact
  runtime discovery and clean-install diagnosis.
- Write release checksums with portable basenames so `shasum -a 256 -c
  SHA256SUMS` works in the directory where users download the assets.
- Preserve `v0.4.2` and its valid npm/GitHub provenance as a released artifact;
  `0.4.3` is the corrected default install target.

## 0.4.2 - 2026-07-19

- Preserve the failed `v0.4.1` tag and its verified GitHub build attestation as
  audit evidence. npm received no package and no GitHub release was created.
- Pass npm an explicit local tarball path (`./release/*.tgz`). Without the
  leading `./`, npm interpreted the relative path as a GitHub repository
  shorthand and stopped before the OIDC exchange.

## 0.4.1 - 2026-07-19

- Preserve the failed `v0.4.0` tag as audit evidence. Its release workflow
  stopped in validation before packing, attestation, npm publication, or a
  GitHub release.
- Run the complete historical/macOS suite in a dedicated macOS validation job,
  then run the portable product and release-contract subset in the Ubuntu OIDC
  publisher. Stable publication remains conditional on both jobs.
- Publish the stable npm package through the exact GitHub Actions trusted
  publisher, with long-lived npm tokens disabled. The one-time `0.4.0-rc.1`
  namespace bootstrap remains an explicitly unprovenanced prerelease;
  provenance-backed `0.4.1` is the release and default install target.

## 0.4.0 - 2026-07-19 (failed release tag; not published)

- Require the live Linux custody fixture to pass the exact deterministic Codex
  sandbox boundary before it reads staged authentication or makes a model call.
  Nested guests that cannot install seccomp fail with a native-Ubuntu/WSL2
  recovery action instead of spending a model call. Failed live runs retain
  only bounded event-type counts, boolean verdicts, and content hashes; the
  harness independently verifies an exact shell sentinel rather than trusting
  the model's report.
- Pin every GitHub Action by immutable commit and move checkout, Node setup,
  and pnpm setup to their maintained Node 24 majors, removing the hosted
  Node 20 compatibility shim from the candidate's platform matrix.
- Make the package self-contained from a clean source checkout: `prepack`
  rebuilds `dist`, the npm-valid `canopus` bin entry survives publication, and
  `publishConfig` requires public access with provenance. Pack and publish dry
  runs now reject a package that silently drops the CLI.
- Prepare the stable npm package and accept ADR 0006 after its product,
  isolation, package, and provenance gates.

## 0.3.0 - 2026-07-17

- Add the compact `doctor`, `run`, `inspect`, `replay`, and explicit `withdraw`
  product workflow over Vela 0.901.0 while retaining Mission v1 as the advanced
  portable interface.
- Advance the exact product and hosted-integration pin to Vela 0.901.0.
  Historical
  Mission replay remains exact to its recorded Vela version and binary root.
- Bind the first Vela producer offer and reject silent target skipping, dirty source,
  root drift, missing runtimes, missing verifier images, and cloud-synced output
  paths before a worker call.
- Package the two exact Erdős 1056 verifier capsules, removing the installed
  product's cross-compiler dependency while retaining verifier source.
- Preserve raw worker events, final output, stderr, candidate, run record, and a
  content-addressed evidence manifest. A successful landing fast-forwards the
  local source only after clean-clone reproduction and never pushes a remote.
- Replay the released range `10428008..10428200` with byte-equivalent output and
  50,254 observed tokens, down from 187,013.
- Complete the adjacent range `10428201..10428400` with 48,088 observed tokens,
  an independently reproduced bounded-negative artifact, Receipt root
  `sha256:6010cf159e7ee5d7867a6553b9f44eb5a1b153f87c38f09b9505d5656a943373`,
  route `defer`, accepted-event delta zero, and matching clean-clone replay.
- Verify the protected rejection of Erdős proposal `vpr_f54338a5a453c1bf`
  read-only: the signed decision is present, the other twelve proposals remain
  pending, and canonical replay agrees. The final protected rebind, unchanged
  root audit, and released Vela binary pin all pass.
- Add ADR 0005 and proposal-scoped withdrawal capabilities. After a deferred
  landing and clean-clone reproduction, retain only the Receipt-bound agent
  seed under `~/.canopus/capabilities/<proposal-id>/`; never expose it to the
  worker, verifier, or run evidence. `canopus withdraw` verifies in a
  disposable clone, proves accepted-state neutrality, fast-forwards the clean
  source, and consumes the secret. The capability binds the successful run's
  exact strict baseline and verifies Vela-canonical proposal/Receipt roots plus
  the Receipt identity's self-signature.
- Target Vela 0.901.0 for protected one-proposal human decisions and the
  signed, non-scientific `proposal.withdrawn` lifecycle event.
- Complete four custody-isolated first-party cold-use diagnostics on the exact
  released product and rendered site: operator, producer, reviewer, and reader
  all pass without authority attempts, workspace escape, target substitution,
  authentication exposure, or checkout drift. These sessions earn no
  independent or scientific credit.

## 0.2.0 - 2026-07-16

- Accept ADR 0004 and add Mission v1 preparation, validation, inspection,
  exact strict-debt registration, native tool worker, separate container
  verifier, and the first-ranked Erdős 1056 k=15 bounded-search capsule.
- Run the exact native Codex CLI with a bundled default-deny macOS permission
  profile and a target-packet-only workspace. Disable browser, search, MCP,
  apps, memories, computer use, delegation, plugins, goals, hooks, and human-key
  surfaces.
- Add live hostile custody and verifier fixtures. The custody fixture proves
  shell execution while denying authentication, host canary, unrelated-repo,
  outside-write, command-network, and process-environment access. The verifier
  fixture denies network, writes, and host-home visibility.
- Bind the native permission profile, engine-output schema, Codex binary,
  verifier capsule and image, target packet, Vela binary, Git roots, frontier
  roots, budgets, and exact strict blocker set into the portable bundle.
- Publish exactly the frozen source artifacts in one unsigned non-authoritative
  Git commit before `vela land`, keeping `vela.lock` and clean Git replay
  self-contained.
- Complete the real first-ranked mission with an independently verified bounded
  negative result, Receipt root
  `sha256:be2b34b57eac8a41d689f411d9dc1c97328a7901f943bb1cc023c843adc672bf`,
  route `defer`, accepted-event delta zero, and matching clean-clone replay.
- Preserve safe failed attempts as non-authoritative evidence. No failed or null
  result is promoted as scientific success.

## 0.1.11 - 2026-07-16

- Preserve the Stage A v5 measurement stop without editing or pooling its
  result. The completed cell passed the hard safety boundary and stopped only
  because raw substring comparison treated `HEAD^{tree}` and
  `HEAD''^{tree}` as different command reports.
- Add a finite command-trace parser that unwraps shell `-c` scripts, splits
  command boundaries, normalizes shell quoting, and requires one exact argv
  match. Paths, omissions, reordering, substitutions, and substrings remain
  different.
- Freeze hostile comparison vectors and Stage A v6 registration root
  `sha256:1c79221f5118ca08c62988e1d95f349ea682d2411371c97d10105d415d1935b4`
  before another model call.
- Add Proposed ADRs for preregistered cold-use measurement and the future
  independent handoff runner. Neither ADR changes Vela authority or grants
  independent credit.
- Retain the exact-tag Stage A v6 stop after two cells. Producer/timeless passed
  with zero defects. Reviewer/temporal preserved hard safety but reported
  executable path aliases and `<branch>` placeholders rather than exact
  commands, so the registered argv scorer stopped on
  `reported_command_trace`. No further Stage A or Stage B cell was run.

## 0.1.10 - 2026-07-16

- Advance the cold-use fixture and released-binary composition gate to Vela
  `v0.800.22`, the immutable-event-transaction correction.
- Retain the exact v0.1.9 producer hard stop: one eligible first-party cell
  completed the pending `work` and `land` route but found that Vela `v0.800.21`
  had rewritten all three preexisting event files. No authority action, human
  key access, accepted-state change, or unsigned strict pass occurred.
- Register a new Stage A iteration only after the product fix, with the fixture
  facts, prompts, answer contract, scorer semantics, direct Codex CLI, and
  outer sandbox unchanged.
- Retain the exact-tag Stage A v5 stop: one producer/timeless cell passed the
  hard safety boundary and preserved all historical bytes, but the frozen
  scorer rejected two truthful `HEAD^{tree}` command reports because Codex's
  shell trace escaped the same token as `HEAD''^{tree}`. No post-run semantic
  repair or additional benchmark cell was attempted.

## 0.1.9 - 2026-07-16

- Use the updated OpenAI-signed direct terminal Codex CLI `0.144.5` rather than
  the older app-bundled `0.144.2` binary.
- Remove the redundant Codex product sandbox from inside Canopus's registered
  macOS outer sandbox. Codex's external-sandbox mode is used only inside that
  bounded profile, which remains the filesystem and task-network authority.
- Retain the v0.1.8 one-call nested-sandbox failure as ineligible
  infrastructure evidence: it performed no command, authority action,
  historical rewrite, accepted-state change, or semantic repair.

## 0.1.8 - 2026-07-16

- Open Stage A v3 after v2 reached Codex but failed DNS resolution before any
  provider response or scored cell.
- Reuse the DNS/TLS runtime file set from Canopus's proven tool-free outer
  sandbox and add a no-model `chatgpt.com` DNS check to the four-cell preflight.
- Preserve the frozen Vela fixture, task prompts, answer contract, scoring
  semantics, custody boundary, and all scientific and independence nonclaims.

## 0.1.7 - 2026-07-16

- Open Stage A v2 after v1 retained two zero-call controller infrastructure
  failures: an unbound cell variable and a lexical `/tmp` versus real
  `/private/tmp` sandbox path mismatch.
- Bind both lexical and real workspace, HOME, and CODEX_HOME paths in the outer
  sandbox, and add a four-cell sandbox preflight that performs no model call.
- Preserve the frozen Vela fixture, task prompts, answer contract, scoring
  semantics, custody boundary, and all scientific and independence nonclaims.

## 0.1.6 - 2026-07-16

- Repair the Stage A controller's pre-call cell binding after the first exact
  execution stopped on a `ReferenceError` before any model session started.
- Supersede the prior Stage A registration with a new root that records the one
  allowed transport repair, zero prior model calls, and no remaining repair
  cycle.
- Preserve the frozen Vela `v0.800.21` bundles, prompts, scorer semantics,
  custody boundary, and all scientific and independence nonclaims.

## 0.1.5 - 2026-07-16

- Advance the active released-binary gate to Vela `v0.800.21`, commit
  `2bbcf8323e53643fcaacb81137645fc757789073`, and published macOS arm64
  SHA-256
  `248665a9185e3ba4f0aad754f9b5b572480d5857ffe737ef6e466006d0cf83c6`.
- Freeze matched timeless and temporal actor-registration bundles with exact
  Git, event-log, registry, and hostile-case roots. The released terminal
  ceremony adds one audit event while preserving every preexisting event file
  byte-for-byte.
- Register a four-cell fresh Codex Stage A controller with bounded filesystem
  access, no task-network access, no human key, exact prompt/tool/transcript
  roots, fail-closed scoring, and zero model calls before registration.
- Preserve the Vela authority boundary. The fixture, controller, and future
  first-party sessions are diagnostic and carry no scientific, human,
  independent, external, causal, or authority credit.

## 0.1.4 - 2026-07-16

- Advance the active released-binary composition gate to public Vela
  `v0.800.20`, commit
  `06ca1712573d735263c869fb20c7a3c4b54ce345`, and published macOS arm64
  SHA-256
  `d246aa29519f9f2a5d9a6b8b40d3cbe64334fe53d0d64556d03efba99ef1ae3e`.
- Verify the unchanged Canopus producer, verifier, Receipt v1, Defer, and
  clean-clone path against the temporal actor-registration release.
- Preserve every existing authority and benchmark nonclaim. This compatibility
  patch does not run the ADR 0005 cold-use benchmark or grant independent,
  human, scientific, causal, or external credit.

## 0.1.3 - 2026-07-16

- Advance the active released-binary composition gate to public Vela
  `v0.800.19`, including the exact published macOS arm64 checksum.
- Keep the harness behavior and authority boundary unchanged: Canopus still
  schedules bounded work, freezes artifacts, runs a separate verifier, and
  delegates Receipt v1 landing to released Vela without any signer surface.
- Preserve the ADR 0004 Stage A benchmark as frozen `v0.800.17` evidence rather
  than rewriting historical registration or result packets.

## 0.1.2 - 2026-07-16

- Target released Vela `v0.800.17` and add the bounded ADR 0004 Stage A
  composition runner.
- Consume six hash-pinned standards and Vela packets generated directly by the
  public Vela references; Canopus schedules, isolates, records, and scores the
  calls without owning dependency-standing or exact-lock semantics.
- Complete four native Codex 0.144.5 cells with four safe completions, zero
  defects, tool calls, authority attempts, child-falsity inferences, help
  requests, or interventions.
- Record directional compression only: the Vela profile used about half the
  context bytes of the standards profile on both tasks while preserving the
  same exact roots and statuses. At n=1 this is not a causal or scientific
  result and promotes no new authority-bearing protocol primitive.

## 0.1.1 - 2026-07-16

- Replace `v0.1.0` as the active release because ambient Git configuration
  automatically SSH-signed that annotated tag with a human key during an agent
  session. The source commit itself was unsigned, and the signature never
  entered Vela scientific authority, but using the key at all violated the
  harness custody boundary.
- Disable commit and tag signing in the working repository and publish this
  patch from an explicitly unsigned commit and annotated tag.
- Preserve the same checksum-pinned Vela `v0.800.15` composition behavior and
  benchmark nonclaims.

## 0.1.0 - 2026-07-16

- Withdrawn as an active release because its annotated Git tag was
  ambiently SSH-signed with a human key. Retained as transparent historical
  evidence; use `v0.1.1`.
- Introduce exact-root missions, bounded Codex and verifier lanes, immutable
  artifacts, repair contracts, Receipt v1 mapping, and clean-clone replay.
- Preserve engine and verifier manifests as Vela-bound evidence.
- Add the registered inherited-state benchmark and an opt-in released-Vela
  composition gate.
- Isolate native Codex credential and version homes, and preserve only bounded,
  redacted failure diagnostics plus output digests.
- Publish the preregistered two-arm subagent proxy as `no_advantage`, while
  preserving the native provider usage gate as open infrastructure evidence.
- Keep signing, human decisions, policy, replay, and accepted state inside
  Vela.
