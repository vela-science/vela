# External-use example A: failed and corrected formal proposal

This informative fixture uses a 64-line Python-standard-library verifier rather
than Lean so a clean consumer can exhaust all four assignments without a
toolchain download. The verifier accepts only a small closed Boolean AST. It
first finds four counterexamples to a bad equivalence, after which Vela refuses
acceptance and retains an attributed rejection. A new corrected De Morgan
Proposal then passes exhaustive Verification, receives an authorized Decision,
enters Standing, and replays.

The corrected Proposal is a new `claim.add`, not a Protocol correction: the bad
Claim was rejected and never entered Standing, so there is no accepted
predecessor for `--corrects` to target.

## Reproduce from a clean checkout

Build the exact delegated candidate and run the frozen assertions:

```bash
cargo build --locked --release -p vela-cli
VELA_BIN="$PWD/target/release/vela" \
  examples/external-formal-verifier/check.sh
```

Prerequisites are Vela, Git, Python 3, `jq`, and a SHA-256 utility. Replay and
the native verifier need no authority key, SSH agent, machine identity,
campaign checkout, network service, Lean installation, or campaign-local
state. The checker installs the independently recorded sequence-one pin and
removes it only if it created it.

The valid branch must reproduce Git commit
`840702a681adfcc47e0354b07e1cea154157da33`, Repository root
`sha256:792e6fe849303a4da0a7f6a14018b3da5884f1f41311d441215dadf93af31011`,
one accepted corrected Claim, one rejected bad Proposal, and no pending Claim.
[`expected.json`](expected.json) freezes every identifier, root, count, bundle
digest, native-file digest, and negative-path error.

For manual inspection after cloning the `valid` branch:

```bash
vela review show . vpr_5a3dadd961d0b9cc --json
vela why . \
  vcl_991c14480535ef573491e7b8b43d626af5147bc0bcb305633e9e64f0f7005d8b \
  --json
vela why . \
  vcl_36fa33468804142cabd939251c1a328965018565411c2ef51c3ba1211cbb7e09 \
  --json
vela replay . --json
```

The `failed-proposal` branch stops after the failing Verification and reports a
blocked Decision Inbox. The `missing-artifact` branch deletes the corrected
verifier output while retaining its references; strict replay exits 1 without
partial Standing.

This fixture establishes only the retained finite Boolean result and Vela
lifecycle behavior. It is not evidence of external adoption, general theorem
proving, scientific utility, or Protocol 1.0 release readiness.
