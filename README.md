<p align="center">
  <img src="assets/brand/vela-readme-hero.jpg" width="960" alt="Vela connects exact scientific evidence, verification, decisions, and standing through Git history.">
</p>

<h1 align="center">Vela</h1>

<p align="center">
  <strong>Version control for scientific state.</strong>
</p>

<p align="center">
  <a href="https://www.vela.space">Website</a> ·
  <a href="https://app.vela.space">Observatory</a> ·
  <a href="docs/QUICKSTART.md">Quickstart</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/REPOSITORY_BOUNDARIES.md">Repository boundaries</a> ·
  <a href="docs/PROTOCOL.md">Protocol</a> ·
  <a href="docs/THREAT_MODEL.md">Security</a>
</p>

---

Research already has tools for code, papers, data, proofs, and computation.
Vela turns the state between them into a living map: what is known, contested,
missing, and ready to attempt next; what exact evidence bears on each Claim;
what was independently checked; what a named Repository decided; and what a
later researcher can safely inherit.

Vela is a Git-native protocol and CLI for governed, replayable
scientific-state transitions. Its complete operator loop is:

```text
init -> submit -> verify -> decide -> replay
```

Work can happen in any native tool. A Submission retains bounded producer
evidence. Checks remain scoped. Only an authorized Decision changes Standing,
and the resulting map exposes the next valid Target or an explicit blocker.
Compounding is a measured outcome, not an automatic property of acceptance.

Research navigation wraps that exact loop without adding authority:

```text
map -> target -> work -> submit -> verify -> decide -> remap
```

The product hierarchy is deliberate:

```text
protocol     integrity layer
map          user-facing product
movement     measurable outcome
```

The protocol is useful when it makes a Frontier legible — the derived boundary
of what is still unresolved — and helps the next valid scientific action
improve after a result, correction, or useful failure. Record count, graph size, workflow completion, and model activity are
not product success.

Its long-range direction is a federated inheritance layer for science:
different workbenches can produce evidence, different verifiers can report
scoped checks, and each Repository can decide and replay its own state without
a hosted authority or universal ontology.

Native systems remain sovereign. Lean checks Lean proofs, Lake resolves Lean
packages, Git preserves bytes and ancestry, and external activity recorders
may preserve sessions. Vela binds exact native objects and governs only the
bounded transition into locally accepted, correction-aware state.

With Vela you can:

- inspect one scientific state from exact Git bytes and full roots;
- map an unresolved gap into bounded work without receiving authority;
- submit portable, authenticated evidence;
- retain verifier observations without treating them as acceptance;
- make one exact authorized Decision; and
- replay how every Claim reached its current Standing and what can safely
  happen next.

## How it works

```text
workbench
   │
   ├── native run ── Submission ──────────┐
   │                                      │
verifier ── Verification Record ──────────┤
                                          ▼
                                 pending Proposal
                                          │
                                authorized Decision
                                          │
                                          ▼
                                    Event + Standing
                                          │
                                          ▼
                                 read-only Observatory
```

Vela keeps four boundaries explicit:

1. **Git preserves bytes and ancestry.**
2. **A Submission preserves producer intent and evidence.**
3. **A Verification Record reports one scoped check.**
4. **Only an authorized human Decision changes Standing.**

A verifier pass is not scientific acceptance. Git publication is not
scientific acceptance. A signature proves control of a key over exact bytes;
it does not prove that a Claim is true.

## Quick start

Install the signed release:

Vela supports Linux x86-64 and macOS Apple-silicon release bundles.

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.971.0/install.sh | \
  VELA_VERSION=v0.971.0 bash
vela --version
```

Or build the exact repository revision:

```bash
git clone https://github.com/vela-science/vela.git
cd vela
cargo build --release
./target/release/vela --help
```

Create and inspect a bounded Repository:

```bash
./target/release/vela init ../my-repository \
  --name "Bounded question" \
  --scope "Does X hold under Y?"

./target/release/vela status ../my-repository --json
./target/release/vela replay ../my-repository --json
```

`init` creates the Profile, signs the repository origin with one Ed25519
identity from the normal OpenSSH agent, installs local trust, and commits the
replayable Repository. It creates no scientific Claim or Standing. When the
agent exposes multiple Ed25519 identities, pass `--key SHA256:<fingerprint>`.
If signing is unavailable, the Profile is retained safely; load the key and
rerun the same `vela init` command.

The resulting sequence-one authority-record root must be distributed through
an independent trusted channel. Load the dedicated repository key once for the
current operating-system session. A trusted native agent may execute an exact
Decision the operator authorized; never forward the authority-agent socket to
remote, untrusted, or proposal-supplied code.

## Typical workflow

```bash
vela status . --json
vela next . --limit 1 --json
# Optional: print the exact stateless Target briefing.
vela start <target> --repo . --json

# Produce the bounded artifact and run the declared verifier.

vela submit --repo . \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

# Verification binds method bytes retained at the current Git commit.
git add -- verification/method.json
git commit -m "Retain verification method"
vela verification record . <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --as verifier:<name> \
  --json

vela review show . <vpr_id> --json
vela replay . --json
vela why . <claim_id> --json
```

`submit` registers authenticated producer input and a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted Standing.
When the Submission declares one verification requirement, `verification
record` uses it directly. Additional observations require an explicit
`--property ... --complementary` so they cannot be mistaken for evidence that
satisfies the registered gate.
`start` writes nothing: Codex, Claude, OpenCode, Harbor, laboratory software,
or another native tool owns execution. Submission and Verification authenticate
their own exact records; neither reads repository-authority credentials.

## Command surface

The ordinary CLI is intentionally small:

```text
init status claims next start submit show why review replay reproduce log
```

Current advanced surfaces:

```text
verification authority
```

Run `vela help advanced` for the grouped contract.

Domain adapters write the optional tracked Target Index directly;
`replay`, `next`, and `start` validate it without a maintenance subcommand.

## Repository model

A Repository is an ordinary Git repository, and it is the only authority
boundary. Its canonical state is composed from typed, content-addressed objects
and an append-only repository-authority history. Generated indexes, Web pages,
databases, graphs, and materialized views are disposable readers. A Frontier is
none of those things: it is a derived query over unresolved state, it has no
identifier, and it owns nothing (ADR 0039).

The Vela source repository is not itself a Vela Repository: do not run
`vela init` here and do not add a root `.vela/` directory. A project-local
`.vela/` belongs only to a real Repository and contains that Repository's
repository-control state.
User-local `~/.vela/` contains private configuration and runtime data; it is
never scientific Standing and must not be copied into Git.

The portable interoperability boundary is the Submission, not a Vela-internal
Event. Workbenches can emit Submission bytes without importing the Vela
runtime. Verifiers can emit scoped Verification Records without receiving
review or repository authority.

## Product packages

The public product now develops from one repository while retaining separate
runtime boundaries:

```text
crates/             Vela protocol, replay, repository authority, and CLI
conformance/        Independent Python and JavaScript readers, two clean-room emitters,
                    fixtures, and repository-wide checks
packages/           The shared source-manifest tooling
scripts/            Release, release manifest, and ecosystem status
.github/release/    Binary artifact publication and smoke tooling
```

The immutable public `@vela-science/canopus@0.8.0` and its Git tag remain
historical replay evidence. Current Vela ships no agent runner. Codex, Claude,
OpenCode, laboratory software, and other native tools work from a Target
packet and register ordinary Submissions or Verification Records. Vela Web and
the canonical Repositories remain separate because they have independent
deployment and scientific-history lifecycles.

Package-local tooling stays with its package. The top-level `scripts/` holds
four files and is not a bucket for a fifth kind of thing: `release.sh` is the
release, `release_manifest.py` is what it emits, `sign-published-release.sh` is
the operator step that signs what CI published and then publishes it, and
`ecosystem-status.py` is what checks this documentation against the tree. The root `install.sh` is the
public product installer.

The Rust crates are internal implementation boundaries, tested together and
released as one `vela` binary. Cross-language conformance uses small standalone
readers instead of a second package. A registry package will exist only after a
real external consumer needs it. The immutable Canopus `0.8.0` package remains
historical evidence from `product-v0.8.0`.

## Security model

Repository authority is a service identity. It records the authenticated
principal, authorization decision, semantic action, exact read set, and
canonical write. It does not replace scientific judgment.

- Producer identities can authenticate bounded work only.
- Human semantic actions are direct `review accept` or `review reject`
  commands.
- Vela reads no human seed file and ships no custom signer daemon.
- The OpenSSH agent signs the exact repository-authority record.
- Preflight or signing failure creates no committed transaction.
- Consumers pin the sequence-one authority-record root independently.

See [Authority and attribution](docs/SIGNING.md) and the
[threat model](docs/THREAT_MODEL.md).

## Development

Requires a current stable Rust toolchain, [uv](https://docs.astral.sh/uv/), and
Node for the portable conformance readers. Sync the locked Python environment
before running the independent reader:

```bash
uv sync --project conformance --locked
cargo check -p vela-cli
cargo clippy -p vela-cli --all-targets -- -D warnings
uv run --project conformance --locked python conformance/verify.py
```

Use focused tests for ordinary changes. The deterministic release union runs
once per actual release boundary.

`conformance/check-current-object-waist.sh` is a CI-only gate and is not in
that workstation set. The CLI resolves the account home through `getpwuid_r`
and ignores `HOME` by design, so the check writes a synthetic authority trust
pin into the real operating-system account home. Its cleanup trap removes the
pin on a normal exit only; an interrupted run leaves it behind. It refuses to
start unless `VELA_EPHEMERAL_ACCOUNT_HOME=1` asserts a disposable account.
`.github/workflows/conformance.yml` runs it on such a runner; run it locally
only on a machine you are willing to treat the same way.

## Documentation

- [Quickstart](docs/QUICKSTART.md)
- [CLI contract](docs/CLI.md)
- [Protocol](docs/PROTOCOL.md)
- [Authority and attribution](docs/SIGNING.md)
- [Verification Records](docs/VERIFICATION.md)
- [Terminology](docs/TERMINOLOGY.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Current repository origin ADR](docs/adr/0027-pre-release-current-state-compaction.md)
- [Native current repository genesis ADR](docs/adr/0023-native-current-repository-genesis.md)
- [Product monorepo and transition-repository retirement ADR](docs/adr/0024-repository-ownership-and-integration-repository-retirement.md)
- [Math-first compounding product architecture ADR](docs/adr/0025-math-first-compounding-product-architecture.md)
- [Living Frontier map and native-system boundary ADR](docs/adr/0028-living-frontier-map-and-native-system-boundary.md)
- [Historical transfer evidence](paper/artifacts/transfer/README.md)
- [Protocol breakthrough benchmark](docs/BREAKTHROUGH_BENCHMARK.md)
- [Whitepaper evidence contract](docs/WHITEPAPER_CONTRACT.md)

## Project status

Vela is pre-1.0. All controlled public Repositories use the current repository
object model. Archived predecessors remain readable through their tags and the
binaries of their era; every Repository the current binary writes starts at a
native genesis. Historical Git revisions preserve earlier contracts, and the
current binary exposes no migration writer.

The final pre-1.0 standards cut has landed in this repository and has not yet
reached `vela-science/math`, which must re-genesis under the new signed
contract before the current binary can read it. That is an operator step, and
until it happens the binary refuses the current `math` head. See
[the 2026-08-08 architecture memo](docs/history/2026-08-08-ideal-ecosystem-and-architecture-memo.md)
and [ADR 0035](docs/adr/0035-commodity-encoding-signing-and-wire-contracts.md).

## License

Code is dual-licensed under Apache-2.0 OR MIT. The Vela name and marks are
trademark rights reserved; see [`assets/brand/LICENSE`](assets/brand/LICENSE).
