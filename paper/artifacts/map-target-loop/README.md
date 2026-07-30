# Live map-to-target loop

`pre-run.v1.json` freezes the exact current inputs before the next producer
Run. It binds the released Vela binary, compact Erdős repository, derived
scientific map, first ranked Target, accepted predecessor range, current
packet, Canopus profile, and both verifier capsules.

The map is a non-authoritative read projection. The Target is producer advice.
The Run and its mechanical verifier cannot change Standing. A later human
Decision, if any, is a separate event and is not implied by this artifact.

The baseline deliberately records no projection release root because candidate
observation time is not scientific identity. The stable map source and layout
roots, exact Git state, repository root, and Target roots are the comparison
inputs for the remap.

Reproduce the current read-only inputs with the released binary:

```bash
vela status ../erdos-frontier --json
vela next ../erdos-frontier --limit 1 --json
```

Reproduce the map projection from `vela-web` commit `834598bf`:

```bash
VELA_FRONTIERS_ROOT="$HOME/personal" \
VELA_BIN=/path/to/vela-0.950.0 \
VELA_PROJECTION_DRY_RUN=1 \
bun packages/frontier-data/scripts/refresh-neon-projection.mjs
```

`post-verification.v1.json` records the completed producer and Verification
half of the loop. Released Vela `0.950.1` imported
`vvr_eb80b766c730513b` at Erdős commit
`606f2f4b50193b1feccf1df4e1f31d50d3a8dd99`; strict and clean-clone replay
agree on repository root
`sha256:8b1c2bbc99b9e9aade2bfb56d3493be02cdad954eefa3cd98a14ac41128ae0d4`.
Accepted-event delta remains zero.

`decision-packet.v1.json` is a key-free inspection packet. It contains a
suggested bounded reason, but neither selects nor invokes the human Decision.
The post-Decision and remap comparison belongs beside this frozen baseline
only after the human independently accepts, rejects, or cancels. It must
report changed and unchanged roots explicitly rather than replacing any
earlier artifact.
