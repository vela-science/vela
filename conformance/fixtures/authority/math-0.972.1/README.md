# Math 0.972.1 authority-chain verification fixture

This verification-only fixture copies the complete four-record public
repository-authority chain for `vela-science/math` at commit
`9bdabbcc1f77d0dd60458e3e9d91d2ffa01fd476`, tree
`3c99d1b9c969a8559605a664bdd7280e9729169f`. The copied authority bytes,
origin, and terminal repository manifest remain byte-identical at the later
Erdős 321 carrier commit `a6a31a528ee86ab79c2aaf4e71e43fc63f4a4e98`.

`source.json` is the exact allowlist and provenance ledger for 56,876 copied
bytes: four records, five events, the generation-one keyset and authorization
model, the origin, and five repository-manifest snapshots. `trust-anchor.json`
is a separate verifier input, not a value inferred from the Repository under
test. `expected.json` freezes the supported chain and terminal-state results.
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
also checks signed read-set, authorization, Event, write-set, and terminal-state
cross-links at fixture level; the production history verifier does not enforce
every one of those links, and this fixture does not claim that it does.

Run the clean-room reader without Vela, Rust, Git, or network access:

```bash
uv run --project conformance --locked python conformance/verify_authority_chain.py
```
