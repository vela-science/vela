# Canopus current-branch contraction

**Status:** Active deletion record

**Baseline:** `6f08247b` on `main`

**Owner:** `vela-science/vela-research-harness`

## Decision

Canopus is one optional bounded producer. Its installed product has four
ordinary commands:

```text
doctor
run
inspect
replay
```

The package exposes only stable run-record parsing and projection. Vela owns
Git publication, canonical state, and authority. Historical benchmarks,
profiles, campaigns, release videos, public-run bundling, and prior domain
capsules remain recoverable from Git tags and history; they do not remain live
product surfaces.

## Deletion protocol

| Candidate | Classification | Current consumers | Recovery | Decision |
| --- | --- | --- | --- | --- |
| `benchmarks/` | Historical registered experiment | Historical tests and reports only | Git history and released tags through `v0.7.0` | Remove from current branch |
| `video/` | Historical Build Week presentation source and generated captures | No runtime consumer | Git history and `v0.6.5` release | Remove from current branch |
| non-Erdős profiles, missions, and capsules | Historical domain missions | Historical tests and package payload only | Git history and releases `v0.4.3` through `v0.7.0` | Remove from current branch |
| `public-run` and `publish-run` commands | Superseded derived-publication format | CLI tests and old evidence docs only | Git history and immutable public evidence already retained in Frontier repositories | Remove from current product |
| fake and tool-free engines | Test and historical benchmark support | Tests and Mission v0 replay | Keep only while a retained current test requires them |
| registrations, experiments, advisories | Historical evidence and stopped programs | No current product consumer | Git history and release tags | Remove from package and default navigation; delete current copies after final consumer trace |

Every candidate above is tracked and present on the public remote. No unique
untracked source is included in this deletion set. Git object identity and
release tags are the recovery source; no separate source archive is required.
Local ignored run evidence under `~/.canopus` is out of scope and remains
untouched.

## Measured baseline

| Surface | Files | Bytes |
| --- | ---: | ---: |
| `benchmarks/` | 200 | 5,650,487 |
| `video/` | 76 tracked | 33,946,088 tracked |
| `profiles/` | 6 | 12,311 |
| `missions/` | 6 | 17,980 |
| `capsules/` | 11 | 4,939,518 |
| `registrations/` | 3 | 4,841 |
| `evidence/` | 10 | 13,189 |
| `experiments/` | 8 | 34,197 |
| `advisories/` | 4 | 9,734 |
| `docs/` | 17 | 448,271 |
| `scripts/` | 11 | 140,922 |
| `src/` | 46 | 480,032 |
| `tests/` | 46 | 256,763 |

## Required verification

Each contraction slice must pass:

```bash
bun run typecheck
bun run build
node --test <focused retained tests>
bun pm pack --dry-run
git diff --check
```

The released-Vela integration additionally proves:

- Vela publishes the exact work and landing commits;
- the source Frontier remains unchanged;
- no Canopus-authored Git commit appears in the candidate history;
- the Receipt routes to Defer with zero accepted-event delta; and
- the clean-clone verifier reproduces the same roots and verdict.

## Result

This first contraction slice removes 24 tracked product files and 1,920 lines
while adding 44 lines of current contract and deletion evidence. It leaves:

- four ordinary commands: `doctor`, `run`, `inspect`, and `replay`;
- one active profile and one matching mission draft;
- one Erdős verifier capsule with two portable platform binaries;
- no `public-run`, `publish-run`, direct mission-execution, retained producer
  key, or Canopus-authored Git publication path; and
- 111 packed files totaling 2.89 MB unpacked, of which 2.40 MB is the two
  required verifier binaries.

The full local suite passes 160 tests with three intentional skips. The focused
integration against Vela `0.930.0-rc.13` proves Vela publishes the lease and
landing commits, the source Frontier remains unchanged, the Receipt routes to
Defer with zero accepted-event delta, and a clean clone reproduces the result.
The current npm composition contract remains the attested Vela
`0.930.0-rc.12` release until rc.13 is published.

Historical benchmark, video, registration, experiment, advisory, and release
evidence is still present in source at this checkpoint. Its current-branch
removal is a separate auditable slice so the product contraction is not mixed
with evidence-history cleanup.
