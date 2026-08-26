# External-use example B: heterogeneous evidence lifecycle

This informative fixture retains four evidence kinds for one deliberately
bounded result: a five-row CSV, a Python-standard-library analysis, its exact
JSON result, and a prose method note. One deterministic Verification checks
exact recomputation. A separate scoped review checks that the Claim does not
extend beyond those bytes. Only after both pass does an attributed authorized
Decision admit the Claim to Standing.

The rows are fixture data, not observations attributed to a real instrument,
site, or population. The accepted Claim is only that the exact median of the
five retained values is 20.5 degrees C. It establishes no instrument accuracy,
measurement provenance, representativeness, causality, or external validity.

## Reproduce from a clean checkout

```bash
cargo build --locked --release -p vela-cli
VELA_BIN="$PWD/target/release/vela" \
  examples/external-heterogeneous-evidence/check.sh
```

Prerequisites are Vela, Git, Python 3, `jq`, and a SHA-256 utility. The checker
recomputes the native result, clones every frozen branch, installs the public
sequence-one pin, exercises scoped review and replay, and removes the pin only
if it created it. It needs no authority key, SSH agent, campaign checkout,
network service, or campaign-local state.

The valid branch must reproduce Git commit
`659107e30ad21c2d1c41f423b043df6646fff399`, Repository root
`sha256:97e508de9e08b272eeb5b1d0fd0a581180adb829fa8c77b59e040bd40d759f58`,
two passing Verification Records, one accepted Claim, and no pending Claim.
[`expected.json`](expected.json) freezes the exact roots, digests, identifiers,
counts, incomplete-review gate, and missing-evidence error.

The `incomplete-review` branch stops after exact recomputation; the Decision
Inbox remains blocked because `evidence_scope_review` is missing. The
`missing-artifact` branch deletes the retained CSV object while keeping its
references; strict replay exits 1 and emits no partial Standing.

This fixture demonstrates a neutral evidence/Verification/Decision lifecycle.
It is not external adoption, empirical validation, a benchmark win, or a broad
scientific claim.
