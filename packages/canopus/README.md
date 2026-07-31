# Private Vela Agent executor

This directory contains the current implementation candidate behind
`vela agent`. It is private workspace source, not a second Vela product,
installable CLI, public library, or independent release train.

The durable boundary is:

```text
Vela        Target, Attempt, Submission registration, Verification intake,
            human Decision, Event, replay, and Standing

Agent helper
            isolated execution, bounded resources, artifact freezing,
            verifier custody, Run retention, replay, and Submission export
```

A Run is nonmutating. Evidence and verifier success do not change scientific
Standing. Only canonical `vela submit` registers a Submission as
`pending_review`, and only a human-authorized Vela Decision can change
Standing.

## Current interface

Use the Vela CLI:

```sh
vela agent doctor
vela agent run --attempt <vat_id>
vela agent show <run.json>
vela agent replay <run.json>
vela agent export <run.json>
```

The Rust CLI invokes an exact, separately built helper process. The helper
receives no repository-authority material or human scientific key. Vela replay
and every Frontier remain valid when the helper is absent.

## Status

[ADR 0031](../../docs/adr/0031-one-product-and-removable-agent-executor.md)
accepts one Vela product and a removable executor. Implementation is still
being reduced. The helper has not yet earned continued distribution: its
survival depends on the registered twelve-hour matched dogfood gate.

The immutable public product is Canopus `0.8.0`, preserved through npm and Git
tag `product-v0.8.0` for historical Runs that bind those exact bytes. Every
other retained Run must use its own exact helper root and source commit. Current
source must not be published under the frozen identity. Historical product
documentation, package contents, and release instructions remain available at
that tag, including the
[Build Week record](https://github.com/vela-science/vela-research-harness/blob/v0.6.5/BUILD_WEEK.md).

## Development

From the repository root:

```sh
bun install --frozen-lockfile
bun run check
```

Package-specific checks:

```sh
bun run --cwd packages/canopus typecheck
bun run --cwd packages/canopus test
```

The directory keeps its historical name during the deletion test so the shrink
diff remains reviewable. If the helper earns survival, it moves under an Agent
implementation name and ships only from the same Vela tag and manifest. If it
does not, it is deleted.

Apache-2.0 OR MIT, at your option.
