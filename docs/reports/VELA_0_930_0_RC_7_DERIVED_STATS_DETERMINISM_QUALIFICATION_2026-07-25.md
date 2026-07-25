# Vela 0.930.0-rc.7 derived-statistics determinism qualification

- Date: 2026-07-25
- Status: locally qualified source candidate
- Stable released baseline: Vela `v0.915.1`
- Candidate version: Vela `v0.930.0-rc.7`
- Candidate implementation commit:
  `081533d356070d4b877ce96ff2277deba407d598`
- Determinism repair commit:
  `7b91af9bdc591858ed96f740f19c9a305bee61d5`
- Protocol effect: none
- Scientific-state effect: none
- Authority effect: none

## Decision

Qualify `v0.930.0-rc.7` as the source candidate that makes Profile v1
derived-statistics serialization deterministic. Keep `v0.915.1` as the stable
released ecosystem component.

The candidate is qualified for source tagging because:

- serialized statistic categories and link types now use ordered maps;
- repeated materialization in separate processes produces identical bytes;
- strict replay passes for the exact migrated Formal Frontier and the current
  Sidon Frontier;
- Erdős and Quantum retain their exact pre-existing fail-closed
  classifications; and
- the deterministic full release union has no failures.

Hosted portable archives, attestations, and checksums remain a separate
publication gate. A source tag is not an installable or attested release.

## Reproduced defect and repair

`ProjectStats.categories` and `ProjectStats.link_types` were serialized from
`HashMap` values. Their randomized process-local iteration order could change
`frontier.json` bytes after a legitimate materialization even when the event
log and scientific state were unchanged.

The repair changes only those serialized fields and their local collectors to
ordered maps. It adds a regression requiring lexical key order in serialized
statistics. It does not change an event, proposal, Receipt, registration,
artifact, policy, accepted-state rule, authority record, or signature
algorithm.

Focused checks passed:

```text
vela-protocol serialized statistics regression: passed
vela-edge lint tests:                         25 passed
vela-cli server tests:                        8 passed
vela-protocol computed project tests:         10 passed
vela-protocol frontier repository tests:      9 passed
vela-protocol state-integrity tests:           7 passed
```

Three independent pre-release Sidon materializations produced the same
`frontier.json` SHA-256:

```text
064952de322e626de7230e27d94b4d1ef4bd2bf7e70c2ddf08e22c002146b73f
```

## Candidate binary

The locally built locked candidate reports:

```text
vela 0.930.0-rc.7
```

Its macOS development binary SHA-256 is:

```text
061f428be4052ca5b572bd67581d810931ce081d59fc8865dbddaeef0a948232
```

This development binary is evidence for local qualification only. It is not a
portable release archive and carries no hosted build attestation.

## Exact live Frontier replay

All commands were read-only except for materialization inside a fresh
disposable Sidon clone. Every canonical source checkout was clean before and
after qualification.

### Formal Conjectures

```text
commit:                25f3ff2d1dd43fd9ec560f78d96f2aa9c602a16c
tree:                  4182b327f49248d174cdc92d4abd3c023e8af3c6
frontier:              vfr_97d7d25957384f80
event count:           36
event-log root:        sha256:3514fbe88560719ebc4f5c7e63522f99373cd4e457ab586f839597122c8fc8e3
scientific-state root: sha256:4924adbbea6dfe288d14af03cf3d544f73c511df6b6ef8b938c8291685101444
strict result:         pass; 14 of 14 findings valid; zero blockers
```

The new `authority.model_migrated` event remains valid, repository context is
strict-clean, and the scientific-state root is unchanged from the pre-migration
boundary.

### Sidon

```text
commit:                89f7208e74c2e8702c80516a4d9bed29f9975e18
tree:                  fda72f6abe3888e43f05054a5c34ba9c223bcf77
frontier:              vfr_496956067dc5ad79
event count:           107
event-log root:        sha256:388557398acd3828f01e09e522f423efa0194526c26938f6c7b7fc833fc8367f
scientific-state root: sha256:ff5a5810d1a173a67253ac2cf509cfb3518cb7fc0b99c840387a8a4d5fe879b6
strict result:         pass; 40 of 40 findings valid; zero blockers
```

A fresh clone materialized twice with the candidate. Both runs produced
identical command output and identical `frontier.json` bytes:

```text
20033e5d402cad5cd6ef05667a7f7457f05c09ec98e6a1cb9867d53970374f9e
```

The first run changed only the derived Profile files:

```text
frontier.json
proof/hashes.json
proof/latest.json
vela.lock
```

The second run introduced no additional change. Strict replay of the
materialized clone passed with the same event and scientific roots above.

### Erdős

```text
commit:                c96f06866dfb50362812bbd15cc0730a4107f184
tree:                  07ec5a51f8fc6459532ee2da48fa9ad678026c41
frontier:              vfr_0a25edabc16db143
event count:           2,192
event-log root:        sha256:cbfa8ff683e44a0abfef9388d48496f0efee60595ae070d415f013ca8c3129c4
scientific-state root: sha256:540d4967071425f77c693e61f62053208b07d67667490dcb9eeef62ec3f1d316
strict result:         fail closed
```

All 2,770 findings replay as structurally valid. Strict mode retains exactly:

```text
missing_conditions:           1,511 blockers
unsigned_registered_actor:       81 blockers
total:                        1,592 blockers
```

The candidate does not temporalize, waive, rewrite, or otherwise hide this
historical debt.

### Quantum Codes

```text
commit:                be2723fe07d0e218f0370253cff93a8748690683
tree:                  b3362c302ea87fbef798abeb48d03aca7ed92553
repository generation: legacy_v0_1 read-only replay
event count:           7
event-log root:        sha256:7a8d06e9c86b9437fffaa6dac9803827f9ad64ee32c34fb1603af8ca986a17ab
legacy snapshot root:  sha256:0975b1b7fda4c2fee1b5cf6fe312843f3f36425151da75eab389522ee1a73e10
strict result:         fail closed
```

Replay retains five valid findings and the same five immutable proposal
logical-ID conflicts documented by the rc.5 qualification. The candidate
neither masks nor repairs them.

## Release-union verification

The deterministic parent release union ran once for this candidate boundary:

```bash
./scripts/full-conformance.sh --suite full --mode=ci
```

Result:

```text
42 PASS
1 WARN
0 FAIL
7 SKIP
```

The sole warning is the existing active-source-map reconciliation warning for
retained historical embedded Formal and Sidon copies. Standalone public
Frontier repositories remain canonical. The warning is an ecosystem cleanup
item, not a candidate protocol or replay failure.

The union included substrate tests, formatting, clippy, dependency audit,
cross-language conformance, internal Lean build, and axiom audits. External
Lean, Diderot, live-network, site, and unrelated suites were not selected.

## Remaining gates

Before this candidate can be described as an installable hosted prerelease:

1. commit and source-tag the exact qualified tree;
2. run the immutable GitHub release workflow;
3. pass all platform build and fresh-prefix smoke jobs;
4. publish archive, SBOM, trust-record, and checksum subjects; and
5. verify hosted provenance for every subject.

If GitHub Actions cannot start because of an account billing or spending
limit, the candidate remains source-tagged and locally qualified. The absence
of hosted artifacts must not be described as a code failure or as a published
portable release.

## Boundary

This qualification does not authorize another Frontier migration, touch a
human credential, change scientific standing, activate a production read
projection, or alter the stable ecosystem lock. Formal's completed ceremony
is the only active authority migration included in this evidence.
