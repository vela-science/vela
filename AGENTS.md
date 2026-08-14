# Working on Vela

Vela is the Git-native protocol and CLI for exact scientific state. Keep this
repository small, current, and useful. Optimize for a complete operator loop,
not for protocol surface area.

The product loop is:

```text
init -> submit -> verify -> decide -> replay
```

Read `README.md` first. Use `docs/ARCHITECTURE.md` for component boundaries,
`docs/PROTOCOL.md` for current semantics, and `docs/REPOSITORY_BOUNDARIES.md`
before moving work between repositories. Historical files explain old results;
they do not define the current product.

## Repository boundary

Vela core owns:

- current protocol objects, canonical encoding, roots, and replay;
- repository authority and exact Decision admission;
- the `vela` CLI and its stable JSON contracts;
- portable schemas, conformance vectors, and exact verifiers.

Vela core does not own:

- scientific campaigns, Targets, proofs, computations, or case dossiers;
- a research runner, scheduler, agent transcript store, or workflow engine;
- Web presentation or a hosted package registry;
- source-specific Erdős, Astra, Formal Conjectures, or other domain logic.

Scientific work belongs in its source-owning Repository. Read-only product views
belong in `vela-web`. Reusable machinery moves into Vela only after two real,
maintained consumers demonstrate the same need and the extraction deletes more
code than it adds.

The Vela source repository is not itself a Vela Repository. Never run
`vela init` here and never add a root `.vela/` directory.

## How to work

1. Reproduce a concrete failure or name the exact behavior being improved.
2. Find the smallest owning crate and its direct consumers.
3. Change the current path. Delete superseded code instead of adding another
   mode, alias, fallback, or compatibility branch.
4. Add the narrowest test that proves the behavior and its important failure
   mode.
5. Run focused checks for the affected crate and consumer boundary.
6. Update current docs only when the public contract changed.

Prefer ordinary Rust and maintained standards over local frameworks. Keep
Vela-specific code for Vela-specific semantics: canonical scientific objects,
root construction, replay, authority evaluation, and consequence-complete
Decisions.

Do not preserve retired commands, object generations, operating systems, or
internal APIs without a current user or replay requirement. Use an explicit
migration when persistent current data must change. Do not make new code read
both old and new formats indefinitely.

Do not add abstract extension points for hypothetical consumers. A second
implementation is evidence for an abstraction; a memo is not.

### Removing something does not create a permanent guard

Deleting a command, spelling, path or object name must not leave behind a test
whose only assertion is that the deleted thing is still absent. Git history
already records what was removed, and `conformance/check-core.sh` says the same
thing: routine CI does not spend time proving that deleted source is still
absent.

A guard against reintroduction earns its place only when reintroduction would
corrupt accepted data or create a security downgrade — a retired path a replay
must refuse, a wire field that would be read as valid, a published JSON key a
consumer contracts on, an installer that must not ship a signer. Those are
protocol checks that happen to be phrased as absence.

Everything else is a sunset. A vocabulary sweep is finished when the word is
gone, and a detector that matches only itself and its own allowlist is not
coverage — it is the last thing keeping the word in the tree. Prefer guarding a
live surface a user meets, such as the binary's help and error output, over
sweeping prose that ordinary review already reads.

## Authority and scientific meaning

Keep these distinctions exact:

- a Submission is authenticated producer input;
- a Verification Record reports one scoped check;
- an authorized Decision is the only operation that changes Standing;
- a green check, signature, Git commit, or Web badge is not acceptance.

Vela uses the standard OpenSSH agent for the repository service identity. Do
not add private-key custody, a signer daemon, repeated per-signature prompts,
or a second approval system. Never forward the authority-agent socket to
remote, untrusted, or proposal-supplied code.

Do not invent panels, reviewer counts, enrollment gates, or multi-person
ceremony. One authorized human or agent performer may make an exact Decision
through Repository authority. Record the performer with `--as` and, when
available, a source-owned `--session-ref`. Preserve the
cryptographic, policy, current-root, read-set, and replay checks that make that
Decision attributable and fail closed.

## Focused verification

Full-workspace CI is not the default local loop. Use the smallest credible
check for the files changed.

For a Rust crate change:

```bash
cargo fmt --all -- --check
cargo test --locked -p <package>
cargo clippy --locked -p <package> --all-targets -- -D warnings
git diff --check
```

Run a named integration test while iterating when it covers the change:

```bash
cargo test --locked -p vela-cli --test bootstrap_cli_ux
cargo test --locked -p vela-cli --test genesis
cargo test --locked -p vela-protocol --test canonical_hashing_conformance
cargo test --locked -p vela-protocol --test engine_pin
```

If a shared type or behavior changed, test its direct consumer crates as well.
If canonical bytes, roots, schemas, or the current interoperability waist
changed, run the relevant conformance check in addition to crate tests:

```bash
uv run --project conformance --locked python conformance/verify.py
```

`conformance/check-current-object-waist.sh` covers the same waist but is a
CI-only gate: it writes a synthetic authority trust pin into the real
operating-system account home, since the CLI resolves that home through
`getpwuid_r` and ignores `HOME`. It exits 2 unless
`VELA_EPHEMERAL_ACCOUNT_HOME=1` asserts a disposable account. Let
`.github/workflows/conformance.yml` run it.

For documentation-only changes, use targeted consistency searches and:

```bash
git diff --check
```

Run the broad local core union only for a release candidate, a toolchain or
dependency change, a cross-cutting protocol change, or an explicit full
certification request:

```bash
uv run --project conformance --locked ./conformance/check-core.sh
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The object-waist check stays on CI even here, for the account-home reason
above.

GitHub Actions is the normal full-repository certification surface for an
ordinary pull request. `.github/workflows/conformance.yml` is the current full
gate; do not maintain a second command list here or spend hours rerunning
unaffected suites locally.

## Crate map

- `vela-protocol`: current objects, canonicalization, roots, events, replay.
- `vela-authority`: restricted authorization and repository service signing.
- `vela-repository`: policy-neutral durable repository transactions and recovery.
- `vela-cli`: the single user-facing product binary.
- `vela-verify`: frozen package-plane witness compatibility outside the
  protocol and authority kernel.

Workspace crates are implementation boundaries, not separate products. Keep
their versions aligned with the single Vela release identity.

## Release discipline

Do not bump versions, edit release notes, tag, or publish for an ordinary
change. A release is a separate task after the intended product change passes
focused checks and hosted conformance.

Vela currently distributes one binary for Linux x86-64 and macOS Apple
silicon. Do not reintroduce Windows branches or broader platform shims unless a
real supported deployment requires them.

Use `0.RRR.P` for releases. Update every workspace package reference together.
`scripts/release.sh` is the release. `.github/workflows/release.yml` calls it
and owns only what is provider-bound: checkout, toolchain and syft
installation, artifact transport, OIDC build provenance, and `gh release
create`. Do not move a build decision back into the YAML; the entry point has
to keep running on a clean checkout with no CI.

A tagged release is a DRAFT until someone signs it. `release.yml` publishes it
unlisted and uninstallable, and `scripts/sign-published-release.sh` signs each
manifest, checks its digests against the published assets, uploads the sidecars
and then publishes — which is when the release becomes immutable, with the
signatures already inside. A release that fails any check stays a draft. Do not
drop `--draft`: a published release refuses new assets, so publishing first
closes the door on the signature.

Release artifacts must come from the release workflow and include the existing
checksums, SBOMs, provenance attestations, and bundle smoke tests.

## Editing rules

- Preserve unrelated work in a dirty tree.
- Do not hand-edit generated roots or claim that a derived projection is
  authoritative.
- Keep JSON output deterministic and actionable on both success and failure.
- Keep failures fail closed at authority, root, schema, and replay boundaries.
- Prefer deletion to deprecation when the old surface is already retired.
- Keep changelog and public claims proportional to evidence actually produced.
- Do not turn a one-off case result into a general productivity, adoption, or
  scientific-lift claim.

Finish with a concise account of what changed, which focused checks passed,
and any current limitation that remains. Do not report work as accepted,
released, or scientifically established before the corresponding external
state exists.
