# Stage A two-provider runtime qualification candidate

Status: **held and blocked; zero provider calls; no permit exists**.

This is a prospective, no-call amendment candidate for the frozen Lean
Correspondence Stage A open pilot. It does not edit the 12 participant packets,
prompts, assignments, or held permits at producer commit
`6e818ecaa8886d3d83856ddb01c4865acdd8b310`. It selects two independently
released model snapshots from different provider organizations and freezes the
common participant-visible tool and custody contracts needed by a later runtime.

The candidate cannot honestly produce the method-required maintained-qualifier
receipt. The exact qualifier blob on reviewed Vela main accepts only
`tools = "none"`, rejects provider event streams containing tool calls, and can
derive a provider schema only by deleting `uniqueItems`. Stage A requires tools,
and its registered response schema also contains `pattern`, `minLength`, and
`minItems` constraints whose provider support is not established by the bound
offline evidence. Claiming qualification would therefore weaken both the tool
boundary and the fail-closed schema boundary.

The local credential check found consumer subscription OAuth sessions for both
providers, but no `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`. The planned raw-HTTP
runner requires those Platform API-key classes through ephemeral descriptor
injection. Subscription credentials are not copied, mounted, persisted, baked,
or treated as API keys.

Because the offline qualifier and schema gates fail, deterministic image builds,
trust-store materialization, absolute assignment mounts, and the two distinct
neutral-calibration permits are deliberately not fabricated. Their roots remain
null and the permit list remains empty. A separately reviewed amendment to the
maintained qualifier must first add a generic tool-using capture contract and a
proved provider-schema transformation vocabulary. After that, a fresh candidate
must build each image twice from empty cache, bind a real trust bundle and
canonical absolute read-only mounts, re-check credential class presence without
revealing values, and obtain a new exact independent PASS before calibration can
be authorized.

Verify the frozen candidate without provider contact:

```bash
uv run --project conformance --locked python \
  paper/artifacts/lean-correspondence-stage-a-runtime-qualification/verify.py

uv run --project conformance --locked python -m unittest discover \
  -s paper/artifacts/lean-correspondence-stage-a-runtime-qualification \
  -p 'test_*.py' -v
```

Passing these checks establishes only that the held blocker is exact and that no
unauthorized substitution or early permit exists. It is not runtime
qualification, calibration authorization, pilot launch, a result, acceptance,
Decision, Standing, release, or deployment.

