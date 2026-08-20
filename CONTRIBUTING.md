# Contributing to Vela

Vela is a pre-1.0 scientific-state protocol and CLI. Useful contributions make
one current user journey clearer, safer, or more interoperable without adding
another protocol layer.

## Good first contributions

- Improve a confusing error or recovery message and add a falsifier.
- Add a hostile fixture for an exact parser or authority boundary.
- Correct a quickstart, example, or platform-specific installation path.
- Add a small interoperability test against a real maintained consumer.
- Reduce duplicate current-facing code, documentation, or compatibility logic.
- Report a concrete Result journey that the current CLI cannot complete.

Before starting a larger change, open or join a focused
[GitHub issue](https://github.com/vela-science/vela/issues). Describe the user
journey, the exact blocker, and the smallest contract change that would remove
it.

## Boundaries to preserve

- Core owns Protocol objects, Repository authority, replay, schemas,
  conformance, and the CLI.
- Source repositories own scientific work, evidence, scope, and source status.
- Native tools own execution. Vela is not a shell runner or agent platform.
- Read products may project state. They do not gain authority by displaying it.
- Verification and acceptance stay separate.
- Human and agent performers use the same authority and current-root gates.

Do not add a new object, compatibility reader, planner, registry, adapter
protocol, or hosted service without a demonstrated maintained consumer.

## Development setup

Vela uses Rust, Python through [uv](https://docs.astral.sh/uv/), and JavaScript
for independent conformance readers.

```bash
git clone https://github.com/vela-science/vela.git
cd vela
uv sync --project conformance --locked
cargo check -p vela-cli
```

The Vela source repository is not a scientific Vela Repository. Do not run
`vela init` here or add a root `.vela/` directory.

## Validate the change you made

Use the smallest credible gate for ordinary work.

Rust change:

```bash
cargo fmt --all -- --check
cargo clippy -p <changed-package> --all-targets -- -D warnings
cargo test -p <changed-package> <focused-test-or-module>
```

Schema or conformance change:

```bash
uv run --project conformance --locked python conformance/verify.py
```

Documentation change:

```bash
git diff --check
rg -n "<changed term or version>" README.md docs crates conformance
```

The full deterministic release union is for release boundaries and changes
that cross most of the system. Do not substitute a broad green test run for a
focused falsifier of the behavior you changed.

## Pull requests

Keep the change narrow. Include:

1. the user or interoperability problem;
2. the exact boundary affected;
3. the evidence or maintained consumer that justifies the change;
4. the focused checks run; and
5. any scientific, authority, compatibility, or recovery claim that remains
   unproven.

Do not describe a build, check, merge, or signature as scientific acceptance.
Do not claim external adoption, independence, or productivity lift without
the corresponding external evidence.

## Security

Do not open public issues for undisclosed vulnerabilities or secrets. Follow
the private reporting route in [SECURITY.md](SECURITY.md). Read the
[threat model](docs/THREAT_MODEL.md) before changing signing, authority,
recovery, parsing, Git custody, or process boundaries.

## License

Contributions are accepted under the repository's Apache-2.0 OR MIT license
boundary. The Vela name and marks remain reserved.
