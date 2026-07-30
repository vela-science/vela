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

The post-Run and post-Decision comparisons belong beside this frozen baseline;
they must report changed and unchanged roots explicitly rather than replacing
this file.
