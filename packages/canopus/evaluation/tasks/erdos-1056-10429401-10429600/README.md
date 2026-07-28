# Erdős 1056 Stage A task

This source-only task freezes the first uncovered contiguous range after the
accepted `10429201..10429400` result. Its canonical source is the exact
`erdos:1056` work packet at root
`sha256:517c16cc9c59d7f91aeaea4287e0ce49000c7545199e86ea632c0a2e91faf30b`.

The model-visible packet names `10429401..10429600` and the exact output
contract without including the preflight result. The independent verifier is
built from the existing audited search source with exact compile-time bounds
as a deterministic static Linux amd64 binary. Its root is
`sha256:68f64c3dc4bc55e98927f65ba509e5c571944239337864bbf631546ac259cdf4`.
The runtime wrapper invokes it inside the exact pinned, read-only,
network-denied, capability-dropped verifier image. Two replays accepted the
same preflight artifact. That preflight is verifier evidence, not model output,
a Submission, or scientific acceptance.

These files remain outside the public Canopus package.
