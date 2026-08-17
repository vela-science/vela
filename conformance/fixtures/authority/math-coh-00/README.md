# Math Submission v3 authority-chain verification fixture

This verification-only fixture copies the complete six-record public
repository-authority chain for the compact `vela-science/math` Submission v3
candidate at commit `f9b28280881472ccb9c4b1b35d8e741745f0bd99`, tree `875539d8790c0557ebee91ad2e40b22f5fa0c147`. The chain initializes the
Repository, admits the Erdős 321 predecessor and correction, admits the bounded
Erdős 887 Claim, and admits the Erdős 94 predecessor and correction.

`source.json` is the exact allowlist and provenance ledger for
100,629 copied bytes: six records, eleven Events, the
generation-one keyset and authorization model, the origin, and eleven
repository-manifest snapshots. `trust-anchor.json` is a separate verifier
input. `expected.json` freezes all five Standing transitions and the terminal
three-Claim accepted set. `negative-vectors.json` names thirteen in-memory
mutations; no corrupted history is retained.

There is no signing seed, private key, writer, new Decision, or Standing
mutation here. The fixture verifies already-committed public bytes and has
`authority_effect: none`. It does not establish scientific truth, semantic
equivalence, external review, or adoption.

Run the independent reader without Vela, Rust, Git, or network access:

```bash
uv run --project conformance --locked python conformance/verify_authority_chain.py
```
