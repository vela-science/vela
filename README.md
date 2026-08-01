<p align="center">
  <img src="assets/brand/vela-readme-hero.jpg" width="960" alt="Vela connects exact scientific evidence, verification, decisions, and standing through Git history.">
</p>

<h1 align="center">Vela</h1>

<p align="center">
  <strong>Version control for living science.</strong>
</p>

<p align="center">
  <a href="https://www.vela.space">Website</a> ·
  <a href="https://app.vela.space">Observatory</a> ·
  <a href="docs/QUICKSTART.md">Quickstart</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/PROTOCOL.md">Protocol</a> ·
  <a href="docs/THREAT_MODEL.md">Security</a>
</p>

---

Research already has tools for code, papers, data, proofs, and computation.
Vela turns the state between them into a living map: what is known, contested,
missing, and ready to attempt next; what exact evidence bears on each Claim;
what was independently checked; what a named Frontier decided; and what a
later researcher can safely inherit.

Vela is a Git-native protocol and CLI for governed, replayable
scientific-state transitions. Its product loop is:

```text
map -> target -> run -> verify -> commit -> compound
```

Work can run anywhere. Checks remain scoped. Only an authorized Decision
changes Standing. The accepted result, correction, or retained failed route
then becomes a stronger starting point for the next person or agent.

The product hierarchy is deliberate:

```text
protocol     integrity layer
map          user-facing product
movement     measurable outcome
```

The protocol is useful when it makes a Frontier legible and helps the next
valid scientific action improve after a result, correction, or useful
failure. Record count, graph size, workflow completion, and model activity are
not product success.

Its long-range direction is a federated inheritance layer for science:
different workbenches can produce evidence, different verifiers can report
scoped checks, and each Frontier can decide and replay its own state without a
hosted authority or universal ontology.

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

Install the GitHub-attested release:

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.960.1/install.sh | \
  VELA_VERSION=v0.960.1 bash
vela --version
```

Or build the exact repository revision:

```bash
git clone https://github.com/vela-science/vela.git
cd vela
cargo build --release
./target/release/vela --help
```

Create and inspect a bounded Frontier:

```bash
./target/release/vela init ../my-frontier \
  --name "Bounded question" \
  --scope "Does X hold under Y?"

./target/release/vela status ../my-frontier --json
./target/release/vela doctor ../my-frontier --all
```

`init` creates a Profile v2 bootstrap, not scientific authority. `status` and
`doctor` report the uninitialized authority boundary without inventing an old
event log. A fresh Frontier administrator can establish the repository writer
with one dedicated Ed25519 identity already loaded in the normal OpenSSH
agent:

```bash
./target/release/vela authority init ../my-frontier \
  --reason "Establish the repository writer for this bounded Frontier." \
  --json
```

The resulting sequence-one authority-record root must be distributed through
an independent trusted channel.

## Typical workflow

```bash
vela status . --json
vela next . --limit 1 --json
# Optional: print the exact Target packet and Submission template.
vela start <target> --frontier . --json

# Produce the bounded artifact and run the declared verifier.

vela submit --frontier . \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

vela verification record . <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --property "Replay the exact retained artifact." \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --as verifier:<name> \
  --json

vela review show . <vpr_id> --json
vela check . --json
vela why . <claim_id> --json
```

`submit` registers authenticated producer input and a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted Standing.
`start` writes nothing: Codex, Claude, OpenCode, Harbor, laboratory software,
or another native tool owns execution. Submission and Verification authenticate
their own exact records; neither reads repository-authority credentials.

## Command surface

The ordinary CLI is intentionally small:

```text
init status next start submit verification show why review check reproduce log doctor
```

Current advanced surfaces:

```text
claim id agents config verification authority repository
```

Run `vela help advanced` for the grouped contract. `repository` is read-only:
it verifies the signed current origin and active repository.

Frontier domain adapters write the optional tracked Target Index directly;
`check`, `next`, and `start` validate it without a maintenance subcommand.

## Repository model

A Frontier is an ordinary Git repository. Its canonical state is composed from
typed, content-addressed objects and an append-only repository-authority
history. Generated indexes, Web pages, databases, graphs, and materialized
views are disposable readers.

The Vela source repository is not itself a Frontier: do not run `vela init`
here and do not add a root `.vela/` directory. A project-local `.vela/` belongs
only to a real Frontier and contains that Frontier's repository-control state.
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
conformance/        Independent Python and JavaScript protocol readers,
                    fixtures, and repository-wide checks
.github/release/    Binary artifact publication and smoke tooling
```

The immutable public `@vela-science/canopus@0.8.0` and its Git tag remain
historical replay evidence. Current Vela ships no agent runner. Codex, Claude,
OpenCode, laboratory software, and other native tools work from a Target
packet and register ordinary Submissions or Verification Records. Vela Web and
canonical Frontier repositories remain separate because they have independent
deployment and scientific-history lifecycles.

Package-local tooling stays with its package. The repository has no catch-all
top-level `scripts/` directory. The root `install.sh` is the public product
installer, not a tooling bucket.

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
- Vela reads no human seed file and ships no custom signer.
- The OpenSSH agent signs the exact repository-authority record.
- Preflight or signing failure creates no committed transaction.
- Consumers pin the sequence-one authority-record root independently.

See [Authority and attribution](docs/SIGNING.md) and the
[threat model](docs/THREAT_MODEL.md).

## Development

Requires a current stable Rust toolchain plus Python and Node for the portable
conformance readers.

```bash
cargo check -p vela-cli
cargo clippy -p vela-cli --all-targets -- -D warnings
python3 conformance/verify.py
VELA_EPHEMERAL_ACCOUNT_HOME=1 ./conformance/check-current-object-waist.sh
```

Use focused tests for ordinary changes. The deterministic release union runs
once per actual release boundary.

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

Vela is pre-1.0. All controlled public Frontiers use the current repository
object model. Existing signed predecessor origins remain exact read-only
origins; newly created Frontiers use native current genesis. Historical Git
revisions preserve earlier contracts, and the current binary exposes no
migration writer.

## License

Code is dual-licensed under Apache-2.0 OR MIT. The Vela name and marks are
trademark rights reserved; see [`assets/brand/LICENSE`](assets/brand/LICENSE).
