# Erdős 505 dimension-one verifier

This target-specific capsule verifies one raw Lean proof term against the exact
statement of `Erdos505.erdos_505.test_dim_one` from
`google-deepmind/formal-conjectures` commit
`c252a41054125b5fd9c8356e2137cd9b55337657`.

The image build verifies that exact upstream commit, tree, and source-file
hash, but the runtime deliberately does not import the admitted upstream
theorem or the `FormalConjecturesUtil` whole-Mathlib umbrella. It retains only
the transitive cache closure of `Mathlib.Analysis.InnerProductSpace.PiL2` and
`Mathlib.Topology.MetricSpace.Bounded`; the reviewed capsule owns an exact copy
of the target signature.

The producer supplies only the term beginning with `by`. The capsule invokes
Lean 4.27.0 inside the pinned verifier image and rejects `sorryAx` or any axiom
outside `propext`, `Classical.choice`, and `Quot.sound`. Kernel success does not
by itself establish statement fidelity or scientific acceptance; the exact
source-bound target packet, capsule review, and Vela review boundary retain
those distinctions.
