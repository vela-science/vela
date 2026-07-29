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
   ├── Attempt ── Submission ─────────────┐
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
4. **Only an authorized Decision changes Standing.**

A verifier pass is not scientific acceptance. Git publication is not
scientific acceptance. A signature proves control of a key over exact bytes;
it does not prove that a Claim is true.

## Quick start

Install the released CLI from crates.io:

```bash
cargo install vela-cli --version 0.940.9 --locked
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
vela start <target> --frontier . --as agent:<name> --json

# Produce the bounded artifact and run the declared verifier.

vela submit --frontier . \
  --attempt <vat_id> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --as agent:<name> \
  --json

vela verification import . verification.json \
  --as verifier:<name> \
  --json

vela review show . <vpr_id> --json
vela check . --strict --json
vela why . <claim_id> --json
```

`submit` registers authenticated producer input and a pending Proposal. It
does not create a Verification Record, Decision, Event, or accepted Standing.

## Command surface

The ordinary CLI is intentionally small:

```text
init status next start submit show why review check reproduce log doctor
```

Current advanced surfaces:

```text
claim id agents config verification authority target-index repository
```

Run `vela help advanced` for the grouped contract. `repository` is read-only:
it verifies the signed current origin and active repository.

## Repository model

A Frontier is an ordinary Git repository. Its canonical state is composed from
typed, content-addressed objects and an append-only repository-authority
history. Generated indexes, Web pages, databases, graphs, and materialized
views are disposable readers.

The portable interoperability boundary is the Submission, not a Vela-internal
Event. Workbenches can emit Submission bytes without importing the Vela
runtime. Verifiers can emit scoped Verification Records without receiving
review or repository authority.

## Product packages

The public product now develops from one repository while retaining separate
runtime boundaries:

```text
crates/             Vela protocol, replay, repository authority, and CLI
packages/protocol/  Authority-free TypeScript contracts and validators
packages/canopus/   Optional bounded producer and evaluation harness
conformance/        Shared cross-implementation fixtures
                     plus repository-wide protocol checks
.github/release/    Rust artifact publication and smoke tooling
```

`@vela-science/canopus` may use `@vela-science/protocol`; it cannot import
repository-authority or Decision internals. Vela Web and canonical Frontier
repositories remain separate because they have independent deployment and
scientific-history lifecycles.

Package-local tooling stays with its package. The repository has no catch-all
top-level `scripts/` directory. The root `install.sh` is the public product
installer, not a tooling bucket.

The Protocol package and Canopus publish together, in dependency order, from
one `product-v*` release tag. Routine development does not advance versions or
copy release facts across the workspace; package versions change only when a
public contract ships. Rust and npm artifacts retain independent versions and
immutable tags even when one monorepo release change coordinates them.

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

Requires a current stable Rust toolchain. TypeScript package work uses the
root Bun workspace.

```bash
cargo check -p vela-cli
cargo clippy -p vela-cli --all-targets -- -D warnings
python3 conformance/verify.py
bun install --frozen-lockfile
bun run check
bun run pack:check
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
- [Current repository epoch ADR](docs/adr/0022-current-repository-epoch-and-legacy-runtime-retirement.md)
- [Native current repository genesis ADR](docs/adr/0023-native-current-repository-genesis.md)
- [Product monorepo and transition-repository retirement ADR](docs/adr/0024-repository-ownership-and-integration-repository-retirement.md)
- [Math-first compounding product architecture ADR](docs/adr/0025-math-first-compounding-product-architecture.md)
- [Living Frontier map and native-system boundary ADR](docs/adr/0028-living-frontier-map-and-native-system-boundary.md)
- [Proposed derived foreign reference ADR](docs/adr/0029-derived-foreign-reference-and-local-authority-containment.md)
- [Protocol breakthrough benchmark](docs/BREAKTHROUGH_BENCHMARK.md)
- [Whitepaper evidence contract](docs/WHITEPAPER_CONTRACT.md)

## Project status

Vela is pre-1.0. All controlled public Frontiers use the current repository
object model. Existing signed predecessor epochs remain exact read-only
origins; newly created Frontiers use native current genesis. Historical Git
revisions preserve earlier contracts, and the current binary exposes no
migration writer.

## License

Code is dual-licensed under Apache-2.0 OR MIT. The Vela name and marks are
trademark rights reserved; see [`assets/brand/LICENSE`](assets/brand/LICENSE).
