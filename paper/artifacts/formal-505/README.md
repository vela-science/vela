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
  --canopus-cli ~/.canopus/bin/canopus-formal-505-d0e05094.js \
  --run ~/.canopus/runs/formal-conjectures-frontier/2026-07-28-formal-505-repair-4/run/run.json \
  --output paper/artifacts/formal-505/report.v1.json
```

The required replay-only CLI has SHA-256
`19e4642e5ca165786a6aa7bf8e352b4461935eb42273c19114818b057c71559d`
and is reproducibly built from Vela Git commit
`d0e050944085a2fdc4a11cc4b57dfa28e789a827`. It is historical evidence, not
the current product interface. Build it in a detached worktree, verify the
digest, and install the exact file at the path above. Current source exposes
only `vela agent`.

`verification-draft.v1.json` and `verification.v1.json` bind that report to the
exact pending Claim, Submission, and Proposal. The exact Submission root binds
all three retained Artifact digests. The redundant Verification v1
`artifact_ids` field is empty because that field accepts legacy `va_` object
identifiers while the current repository stores Artifacts by full content
hash. The incompatibility is recorded as a campaign contract gap; it is not
hidden by an invented identifier.

The signed record uses an ephemeral verifier-only key; no human or repository
authority key is embedded. `import.sh` replays the verifier and checks every
file, binary, and Frontier-head pin before asking the human-controlled
repository transaction layer to retain the non-authorizing Verification.
