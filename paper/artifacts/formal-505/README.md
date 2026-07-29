# Formal Erdős 505 replay

This artifact replays the exact retained Canopus Run for
`formal:erdos-505-test-dim-one` through its network-denied, read-only verifier
capsule. It binds the Run, Mission, capsule, Canopus CLI, stdout, and stderr
roots and emits a deterministic report.

A passing report establishes only that the retained Lean declaration
elaborates under the frozen Lean 4.27.0 environment and that its axiom audit
contains only `propext`, `Classical.choice`, and `Quot.sound`. It does not
establish informal statement fidelity, solve the general Borsuk conjecture, or
constitute scientific acceptance.

```bash
python3 paper/artifacts/formal-505/verify_replay.py \
  --canopus-cli packages/canopus/dist/src/cli.js \
  --run ~/.canopus/runs/formal-conjectures-frontier/2026-07-28-formal-505-repair-4/run/run.json \
  --output paper/artifacts/formal-505/report.v1.json
```

`verification-draft.v1.json` and `verification.v1.json` bind that report to the
exact pending Claim, Submission, Proposal, and all three retained Artifacts.
The signed record uses an ephemeral verifier-only key; no human or repository
authority key is embedded. `import.sh` replays the verifier and checks every
file, binary, and Frontier-head pin before asking the human-controlled
repository transaction layer to retain the non-authorizing Verification.
