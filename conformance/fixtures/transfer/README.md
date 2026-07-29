# Current foreign-transfer contract gap

This fixture is a negative conformance inventory, not a transfer schema.

The Rust reader derives current migration-lineage fields from compiled public
types. The clean-room Python reader derives them from the public Rust source.
Both also confirm that the product CLI deliberately exposes no federation
surface.

Agreement qualifies the reproduced B8 gap. It does not authorize an import
command, transfer envelope, resolver, Registry, hosted service, or local
Standing change.

`foreign-reference-input.v1.json` is the language-neutral qualification
fixture for the smallest derived response to that gap. It binds exact source
repository, Claim, Proposal, Decision, authority, and retained-object roots;
declares completeness; and fixes local Standing effect to `none`.
`foreign-reference-expected.v1.json` is the byte-identical assessment expected
from both readers. These synthetic vectors qualify implementations only. They
do not establish real second-Frontier retention or external independence.
