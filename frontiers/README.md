# Frontiers

This directory contains only release-critical sample frontier state.

Tracked artifacts:

- `bbb-alzheimer.json` - canonical BBB/Alzheimer frontier sample for the v0 proof path.

Large generated frontiers are not committed by default. Generate them locally with `vela compile`, validate them with `vela check`, and attach heavyweight samples to releases when they are needed for reproducible demos.

Useful commands:

```bash
vela stats frontiers/bbb-alzheimer.json
vela check frontiers/bbb-alzheimer.json
vela proof frontiers/bbb-alzheimer.json --out proof-packet
```
