# Math COH-00 authority-chain verification fixture

This verification-only fixture copies the complete four-record public
repository-authority chain for `vela-science/math` at commit
`08a0e6d327e1ae9937ab2e0e5002192815eac69a`, tree
`f58de302dcaf96e41e4836732dc5446f4eeb8c61`. The chain initializes the
current compact Repository, accepts an Erdős 321 predecessor, accepts its
explicit correction, and accepts the bounded Erdős 887 Claim.

`source.json` is the exact allowlist and provenance ledger for 54,772 copied
bytes: four records, seven events, the generation-one keyset and authorization
model, the origin, and six repository-manifest snapshots. Each copied file is
pinned to its source commit, tree, Git blob, size, and raw SHA-256 digest.
`trust-anchor.json` is a separate verifier input, not a value inferred from the
Repository under test. `expected.json` freezes the supported chain,
predecessor-to-correction transition, and terminal-state results.
`negative-vectors.json` names thirteen in-memory mutations and their stable
failure codes; no corrupted history is retained.

There is no signing seed, private key, writer, new Decision, or Standing
mutation here. The fixture verifies already-committed public bytes. The source
commit contains no `LICENSE`, `COPYING`, or `NOTICE`, so rights are
`NOASSERTION`; retaining these bytes makes no redistribution-right claim.

The check does not establish key or model rotation, terminal close, forks,
federation, operating-system authentication, human intent, transaction or
read-set preimages, execution-binary identity, full scientific-object replay,
scientific truth, commit signatures, full-tree reconstruction, or the actual
out-of-band distribution of the trust anchor. It also does not claim that the
current CLI read path loads its local authority trust pin. The independent
reader takes the sequence-one anchor as an explicit separate input. The reader
also checks signed read-set, authorization, Event, write-set, correction, and
terminal-state cross-links at fixture level; the production history verifier
does not enforce every one of those links, and this fixture does not claim that
it does.

Run the clean-room reader without Vela, Rust, Git, or network access:

```bash
uv run --project conformance --locked python conformance/verify_authority_chain.py
```
