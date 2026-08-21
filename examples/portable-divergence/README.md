# Portable Submission, local Decision divergence

Protocol 1 makes authenticated producer input portable without making
Repository authority global. This synthetic example imports the exact same
signed Submission into two freshly initialized, independently governed
Repositories. One local authority accepts after a local passing Verification;
the other rejects without importing that Verification. Each Git history then
replays deterministically to its own Standing and Repository root.

The portable input is the frozen independently emitted fixture
[`conformance/current-objects/submission.json`](../../conformance/current-objects/submission.json),
signed by `agent:independent-js` at root
`sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a`.
Its retained Artifact contains only the synthetic bytes `42\n`. Importing the
same envelope derives Claim
`vcl_cea6cdb3e9fd02fae86886a0edbe51e5c2fe2d5e00dc7f264d4c3de0f9f2c422`
at root
`sha256:e865c5a2aafd459d52d9b1c8a7734104b1e2d8d1c047c5400684f01505f83632`
in both Repositories.

Run the complete disposable demonstration:

```bash
cargo test --locked -p vela-cli --features test-support --test portable_divergence
```

The test-support build uses two exact synthetic device identities, two separate
temporary OpenSSH agents, and two fresh Repository identities. The device
identities make the authenticated local authority principals distinct without
requiring a second operating-system account or changing production identity
resolution. It runs the `vela submit`, `verification record`, `review
accept|reject`, and `replay` command templates and fixed arguments listed in
[`flow.json`](flow.json), substituting only the fresh paths, Proposal ids, and
Inbox roots generated during the run, then
clones both Git histories and requires each clone to reproduce its source
commit, tree, Repository root, and Standing. It also requires different local
keysets, authorization models, sequence-one roots, Decision-record roots, and
terminal Repository roots.

The same test separately reconstructs the retained
[`accept.git.bundle`](accept.git.bundle) and
[`reject.git.bundle`](reject.git.bundle) histories. [`expected.json`](expected.json)
binds their distinct authenticated principals and every terminal Git,
sequence-one authority, Decision, event-log, Event, Repository, projection, and
Standing root. `flow.json` binds the three fixture-file digests so changed bytes
cannot silently redefine those expectations.

The example demonstrates interoperability without consensus: portable producer
bytes and their derived Claim identity agree, while authority, Decision, Event,
and Standing remain local. The example document itself has `authority_effect:
none`; its accept/reject actions occur only inside disposable synthetic test
Repositories. Nothing here establishes scientific truth, transports Standing,
or supplies authority to another Repository.
