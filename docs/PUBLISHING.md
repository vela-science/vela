# Publishing Vela frontiers

One standalone Git repository owns each published frontier. Publication is an
ordinary commit and push. Vela does not maintain a second registry transport,
Hub write protocol, signed mirror, or special publication database.

## Publish a reviewed frontier

From the standalone repository:

```bash
vela frontier materialize .
vela check . --strict --json
vela reproduce .
git status --short
git commit
git push
```

Use the frontier's exact frozen reproduction command when it differs from
`vela reproduce .`. A verifier pass is evidence about the declared mechanical
property. It does not replace any signed policy or human decision required for
accepted scientific state.

Materialized files are committed only when the frontier contract calls for
them. They remain derived views. The `.vela/events` log and the artifacts it
binds are the replay source; do not repair a release by hand-editing
`frontier.json`, a proof packet, or a Hub row.

## Pull requests

A Git pull request is the collaboration and branch-review mechanism. Vela's
scientific contribution path inside that repository remains:

```text
next -> work -> land -> signed policy route -> Permit | Defer -> protected human decision
```

`land` records the exact Receipt v1 contribution and its routed effect. The Git
host may run `vela check --strict` and selected frozen verifiers, but CI cannot
use a human key or turn a deferred proposal into accepted state. A merge is
publication of the committed bytes, not an independent acceptance mechanism.

## Read-only discovery

The Observatory projector is a read-only consumer. Its checked source registry
binds a repository URL, exact commit, and expected `vfr_id`; it fetches,
replays, and projects that exact Git history into a disposable query cache.

Projection inclusion is not registration or endorsement. The Observatory can
lag, omit a source, or disappear without changing the repository's history.
Consumers use the displayed Git source as a locator and verify an exact clone
locally.

## Frozen release archives

A project may attach an immutable archive to a Git tag or deposit it in an
external archive for citation and long-term access. Preserve the repository
bytes and include:

- the full Git commit and tag;
- the frontier ID and accepted event-log root;
- artifact content digests and retrieval instructions;
- the verifier and environment pins needed for reproduction;
- licenses and `CITATION.cff` when present;
- a checksum manifest for files outside Git object verification.

An archive, dataset card, DOI, or Git bundle is another distribution locator.
It is not a new source of scientific authority. If bytes are copied outside
Git, copy them unchanged and retain their content digests.

For a fully offline transfer:

```bash
git bundle create frontier.bundle --all
git bundle verify frontier.bundle
git clone frontier.bundle frontier-offline
cd frontier-offline
git fsck --full
vela check . --strict --json
vela reproduce .
```

See [`EXIT_AND_EXPORT_DRILL.md`](EXIT_AND_EXPORT_DRILL.md) for the institutional
exit exercise and [`INTEROPERABILITY.md`](INTEROPERABILITY.md) for the stable
wire contracts.

## Citation

Cite the frontier repository and exact commit or release tag. Include the
frontier ID and relevant finding or artifact IDs so another reader can locate
the accepted state and reproduce its checks. An Observatory URL may be included
as a convenience link, but it should not be the only locator.

Describe Vela accurately: it records a replayable frontier and the authority
that admitted its transitions. It does not certify that a claim is important or
scientifically correct merely because the bytes are signed and reproducible.

## Release boundary

Before calling a frontier release reproducible, require:

1. the selected Vela conformance checks for the changed paths;
2. `vela check . --strict --json` from the exact release tree;
3. the frontier's frozen verifier or `vela reproduce .`;
4. a clean reconstruction of every committed derived view;
5. unchanged accepted-event bytes except for intentional, signed additions;
6. an exact commit and successful push.

External Lean, live-network, and platform-pinned checks run only when the
release or changed paths select them. An unrelated Vela release does not wait on
an early external project or service.

## Prelaunch legacy note

Earlier development documents described Hub registration and signed transport,
special frontier packets, an embedded website, package-registry publication,
and profile-specific reviewer writers. Those were prelaunch experiments and are
not compatibility contracts. Immutable historical events remain replayable,
but new publication uses Git and new contributions use Receipt v1 plus `land`.
