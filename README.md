<p align="center">
  <img src="assets/brand/vela-readme-hero.jpg" width="960" alt="Vela connects exact scientific evidence, verification, decisions, and standing through Git history.">
</p>

<h1 align="center">Vela</h1>

<p align="center">
  <strong>Version control for scientific state.</strong>
</p>

<p align="center">
  <a href="https://www.vela.space">Website</a> ·
  <a href="https://problems.science">Problems</a> ·
  <a href="docs/QUICKSTART.md">Quickstart</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/REPOSITORY_BOUNDARIES.md">Repository boundaries</a> ·
    <a href="docs/PROTOCOL.md">Protocol 1</a> ·
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
and a source-owning Repository or read product may use the resulting state to
expose an exact next obligation or an explicit blocker. Compounding is a
measured outcome, not an automatic property of acceptance.

The public navigation loop wraps that exact operator loop without adding
authority:

```text
MAP -> ADVANCE -> REMAP
```

**Map** reads Problems, Claims, Standing, dependencies, Corrections, and open
Obligations from exact roots. **Advance** means doing native human or machine
work that may produce a bounded proposed change. **Remap** replays the new
Standing and derives what remains current, affected, blocked, or open. Vela
core owns no work catalogue or planner.

The product hierarchy is deliberate:

```text
protocol                     integrity layer
map                          user-facing product
verified frontier movement   measurable outcome
```

The protocol is useful when it makes a Frontier legible — the derived boundary
of what is still unresolved — and helps the next valid scientific action
improve after a result, correction, or useful failure. Record count, graph size, workflow completion, and model activity are
not product success.

The Vela Protocol is the narrow integrity layer: Claim records, authenticated
Submissions, scoped Verification Records, Proposals, authorized Decisions,
Events, exact roots, replay, and Standing. The wider Vela ecosystem includes
native workbenches, source-owning Repositories, verifiers, and rebuildable read
products. The Problems projection assembles exact scientific state, evidence,
limitations, and next actions without becoming a protocol object or making a
Decision.

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
                                  read-only Problems
```

Vela keeps four boundaries explicit:

1. **Git preserves bytes and ancestry.**
2. **A Submission preserves producer intent and evidence.**
3. **A Verification Record reports one scoped check.**
4. **Only an authorized, attributed Decision changes Standing.** The
   performer may be human or agent; Repository authority and exact replay, not
   actor kind, govern admission.

A verifier pass is not scientific acceptance. Git publication is not
scientific acceptance. A signature proves control of a key over exact bytes;
it does not prove that a Claim is true.

## Quick start

Install the exact signed release:

Vela supports Linux x86-64 and macOS Apple-silicon release bundles.
`v0.977.1` is the current signed release. Both platform manifests bind the
archives and SBOMs and verify with the out-of-band distribution identity before
installation.

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.977.1/install.sh | \
  VELA_VERSION=v0.977.1 bash
vela --version
```

Read a real public Repository. Reading and replay require neither an account
nor a repository-authority key:

```bash
git clone https://github.com/vela-science/math.git math
git -C math checkout f9b28280881472ccb9c4b1b35d8e741745f0bd99
vela replay math --json
vela claims math --json
```

Use a complete clone: exact offline reads refuse shallow, partial, alternate,
or grafted object stores. The pinned Repository replays to three current accepted
Claims at root
`sha256:45640c5eea54693df444eada6dd1a7c1f5a4b4ef266fddf79cf51d083233ebba`.
The corrected Erdős 321 and 94 Claims are accepted and both predecessors remain
retained with current Standing `superseded`; see the
[formal-math reference flow](examples/formal-math/).

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
Work selection and execution stay with the source-owning Repository, a read
product, or a native tool such as Codex, Claude, OpenCode, Harbor, or laboratory
software. Those surfaces may expose exact next obligations and rooted work
packets. Vela core provides no `next`/`start` command pair; Submission and
Verification authenticate their own exact records, and neither reads
repository-authority credentials.

## Command surface

The ordinary CLI is intentionally small:

```text
init status claims submit show why review replay log
```

Current advanced surfaces:

```text
projection verification correction integration recover authority
```

Run `vela help advanced` for the grouped contract.

`vela recover --repo <PATH> <OPERATION_ID> [--json]` is the explicit operator
route out of an interrupted repository transaction. It opens only the named
journal: an exactly Prepared transaction with a definitely absent commit marker
is aborted, while a valid marker authorizes policy-free exact completion.
Completed and Aborted journals are idempotent. Recovery stops after the
repository filesystem transaction; it neither continues the semantic command
nor publishes Git state. A routine completion names Git status as the next
inspection. If the exact operation is a verified native genesis, it instead
names the retained `vela init --key ... --reason ...` continuation; that later
command verifies and creates or idempotently confirms only the deterministic
Git/trust tail without a signer or another authority transaction. If that
advisory proof is unavailable after recovery succeeds, the result stays
successful, reports a closed `blocked` continuation status, and offers no
executable continuation until the retained proof is repaired.

Source-owning repositories and read products may expose exact next obligations
under their own rooted contracts. They are replaceable orientation surfaces,
not Vela replay state, and cannot change Standing.

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
crates/             Protocol, repository runtime, authority, and CLI
conformance/        Independent Python and JavaScript readers, two clean-room emitters,
                    fixtures, and repository-wide checks
scripts/            Core release and signed release-manifest tooling
.github/release/    Binary artifact publication and smoke tooling
```

The immutable public `@vela-science/canopus@0.8.0` and its Git tag remain
historical replay evidence. Current Vela ships no agent runner. Codex, Claude,
OpenCode, laboratory software, and other native tools work directly or from a
source-local packet and register ordinary Submissions or Verification Records.
Vela Web and the canonical Repositories remain separate because they have
independent deployment and scientific-history lifecycles.

Source acquisition and domain inventory tooling stay with their source-owning
repositories. The top-level `scripts/` holds the Core release path:
`release.sh` is the
release, `release_manifest.py` is what it emits, `sign-published-release.sh` is
the operator step that signs what CI published and then publishes it. The root
`install.sh` is the public product installer.

The Rust crates are internal implementation boundaries, tested together and
released as one `vela` binary. `vela-repository` is the policy-neutral durable
transaction runtime below the CLI; it is not a separate product or semantic
kernel. Cross-language conformance uses small standalone readers instead of a
second package. A registry package will exist only after a real external
consumer needs it. The immutable Canopus `0.8.0` package remains historical
evidence from `product-v0.8.0`.

## Security model

Repository authority is a service identity. It records the authenticated
principal, authorization decision, semantic action, exact read set, and
canonical write. It does not replace scientific judgment.

- Producer identities can authenticate bounded work only.
- Authorized Decision actions are direct `review accept` or `review reject`
  commands. `--as` records a human or agent performer separately from the
  Repository authority principal and signer.
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
- [Architecture](docs/ARCHITECTURE.md)
- [Repository boundaries](docs/REPOSITORY_BOUNDARIES.md)
- [Authority and attribution](docs/SIGNING.md)
- [Verification Records](docs/VERIFICATION.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Release, installation, and recovery guidance](docs/RELEASES.md)
- [Complete documentation index](docs/README.md)
- [Historical transfer evidence](paper/artifacts/transfer/README.md)

## Project status

The published Vela binary remains pre-1.0. The current object/kernel contract is
presented as the Protocol 1 release candidate, with one normative specification,
generated schemas, a digest-bound conformance manifest, independent Python and
JavaScript emitters/readers, and three executable examples. This status does not
publish `v1.0.0`, promise compatibility before the final release authorization,
or claim external adoption.

All controlled public Repositories use the current repository object model.
Archived predecessors remain readable through their tags and the binaries of
their era; every Repository the current binary writes starts at a native
genesis. Historical Git revisions preserve earlier contracts, and the current
binary exposes no migration writer.

The signed `v0.977.1` release carries the current Protocol 1 and Submission v3
runtime. Published signed tags retain the binaries and source needed to
reproduce earlier repositories. They do not add predecessor readers or writers
to the current runtime.

## License

Code is dual-licensed under Apache-2.0 OR MIT; see [LICENSE](LICENSE) for the
license boundary and the full [Apache](LICENSE-APACHE) and [MIT](LICENSE-MIT)
texts. The Vela name and marks are trademark rights reserved; see
[`assets/brand/LICENSE`](assets/brand/LICENSE).
