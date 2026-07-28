# CORE-Bench capsule 1108125

This is the first Stage A scientific-computing task that passes the frozen
selection rules in `docs/CAMPAIGN.md`.

The source archive is not checked into the Vela monorepo. Download it from the
registered CORE-Bench URL, verify its exact SHA-256 through `prepare.mjs`, and
keep the archive outside every model-readable workspace. Preparation projects
only the fourteen allowlisted code and data files into `packet.json`; retained
published results and CORE-Bench answer records are not included.

```bash
bun run eval:task:prepare:scientific -- \
  --archive /path/to/capsule-1108125.tar.gz \
  --output /new/registration/task
```

The worker must produce:

```json
{
  "schema": "canopus.core-bench-1108125-result.v1",
  "task_id": "core-bench:capsule-1108125",
  "forestgroup_mean": 0.34,
  "gender_mean": 0.46,
  "income_mean": 1,
  "eigen_trend": "decrease"
}
```

The model-visible packet specifies the complete closed object contract,
including schema and task identities, the two-decimal rounding rule for each
mean, and the `decrease|increase` enum. It does not contain the values above.
Those values remain here for verifier maintenance. The exact task verifier
must remain outside worker-readable paths. It replays the source with the
registered image, network disabled, a read-only root, read-only source mounts,
and ephemeral output filesystems. It checks the three summary rows and the
stable Figure S5 root, then compares the candidate artifact. The unrelated
Monte Carlo output and PNG container metadata outside Figure S5 are
deliberately outside the result contract.

```bash
bun run eval:task:verify:scientific -- \
  --archive /path/to/capsule-1108125.tar.gz \
  --artifact /path/to/artifacts/result.json
```

The verifier pass is mechanical evaluation evidence. It is not a Vela
Verification Record, Decision, or scientific acceptance.
