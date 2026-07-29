# Vela conformance

This directory contains the small, implementation-independent corpus for
Vela's current public object boundary.

Run all checks:

```bash
python3 conformance/verify.py
```

The corpus protects six contracts:

1. canonical JSON bytes and SHA-256 roots;
2. retained Attempt content addresses;
3. repository principal and delegated-capability validation;
4. byte-identical Submission and Verification emission from an independent
   JavaScript implementation; and
5. exact witness and bounded-Claim agreement; and
6. a non-authoritative correction-impact projection, including independent
   support-route survival and bounded fail-closed diagnostics.

`current-objects/` contains deterministic signed Submission and Verification
vectors. The seed files are public fixture material only. They are never used
as production identities.

`fixtures/principal-capability-v1.json` and
`fixtures/exact-witness-floor-v1.json` are normative test vectors.

`fixtures/correction/diamond-input.v1.json`,
`diamond-expected.v1.json`, and `diamond-adversarial.v1.json` are synthetic
conformance vectors only. They let the Rust reader in `vela-edge` and the
clean-room Python reader agree on exact bytes before a real correction fixture
exists. They earn no scientific or protocol-breakthrough credit.

Historical reducer cascades, AcceptancePolicy experiments, actor-registration
previews, and their duplicate Python/TypeScript readers remain available in
Git history. They are not current runtime contracts and are intentionally
absent from this corpus.
